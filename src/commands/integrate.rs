//! `rk integrate`: move one implementation onto the trunk through the
//! authority the target recorded.
//!
//! The local path is a transaction. It creates the squash commit as an
//! unreferenced object, so nothing is published until one compare-and-swap
//! carries the trunk tip observed before the gate ran. Every refusal
//! therefore leaves the trunk at the tip it started from, and there is no
//! half-integrated trunk to undo.
//!
//! The forge path stops where this repository's boundary already sits.
//! It runs the project's own pre-push stage by pushing the branch, and it
//! names the request command for the detected forge. Opening a request is
//! an operator-named action in the landed routing block, and no verb here
//! authors a request body: `rk message --check --kind body` is what
//! judges one.
//!
//! SATISFIES git:a-local-integration-is-a-transaction
//! SATISFIES git:the-manual-stage-is-the-pre-integrate-contract

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::cli::integrate::IntegrateArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::integrate::{self, Entry, Ledger};
use crate::landing::Integration;
use crate::maintenance::{GIT_HOOK_VARS, last_line};
use crate::output::Output;

/// The machine form of a report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target this ran against.
    target: String,
    /// The authority this execution used.
    integration: &'static str,
    /// Where the authority came from: `record` or `flag`.
    authority_source: &'static str,
    /// The branch integrated.
    branch: String,
    /// The trunk it integrated onto.
    trunk: String,
    /// The seat the gate ran in.
    seat: String,
    /// The trunk tip before this integration.
    trunk_before: String,
    /// The squash commit, on an applied local integration alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    trunk_commit: Option<String>,
    /// The steps that ran, in order.
    steps: Vec<String>,
    /// What the operator does next.
    next: Vec<String>,
}

/// Integrate one branch; preview unless `--apply`.
///
/// # Errors
///
/// Returns [`RkError::Refusal`] for every judgment that stops the
/// transaction, [`RkError::Missing`] for a target that is not a
/// repository, and [`RkError::Io`] where the ledger cannot be written.
pub fn run(args: &IntegrateArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let target = &args.target;
    let trunk = crate::config::trunk_of(target.as_std_path())?;

    let (integration, authority_source) = authority(args, target)?;
    if let Some(reason) = integrate::refuse_branch_name(&args.branch, &trunk) {
        return Err(refuse(Reason::Usage, reason));
    }

    let git_dir = common_git_dir(target)?;
    let seats = seats(target)?;
    let seat = seat_for(&seats, &args.branch, target)?;
    let dirty = is_dirty(&seat)?;
    if dirty {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "{seat} has uncommitted changes, so the gate would judge a tree nobody reviewed"
            ),
        ));
    }

    match integration {
        Integration::Forge => forge_path(args, out, &seat, &trunk, authority_source),
        Integration::Local => local_path(
            args,
            out,
            &LocalRun {
                target,
                git_dir: &git_dir,
                seat: &seat,
                seats: &seats,
                trunk: &trunk,
                authority_source,
            },
        ),
    }
}

/// Everything the local transaction resolved before it acts.
struct LocalRun<'a> {
    target: &'a Utf8Path,
    git_dir: &'a Utf8Path,
    seat: &'a Utf8Path,
    seats: &'a [crate::worktree::Worktree],
    trunk: &'a str,
    authority_source: &'static str,
}

/// The authority this execution uses, and where it came from.
///
/// The record answers unless a flag overrides it for this one execution,
/// which is the per-execution choice the method states. An override
/// writes nothing.
fn authority(
    args: &IntegrateArgs,
    target: &Utf8Path,
) -> Result<(Integration, &'static str), RkError> {
    if args.local {
        return Ok((Integration::Local, "flag"));
    }
    if args.forge {
        return Ok((Integration::Forge, "flag"));
    }
    let recorded = crate::landing::manifest::load(target)?.map(|record| record.git.integration);
    let configured = crate::config::load(target.as_std_path())?.and_then(|c| c.git.integration);
    Ok((
        configured
            .or(recorded)
            .unwrap_or(crate::landing::manifest::integration_forge()),
        if configured.is_some() {
            "config"
        } else if recorded.is_some() {
            "record"
        } else {
            "default"
        },
    ))
}

/// The forge path: gate through the push, then name the request command.
fn forge_path(
    args: &IntegrateArgs,
    out: Output,
    seat: &Utf8Path,
    trunk: &str,
    authority_source: &'static str,
) -> Result<(), RkError> {
    let mut steps = Vec::new();
    if args.apply {
        // The push is the boundary under forge integration, and the
        // project's own pre-push stage is what git fires on it. Nothing
        // here passes --no-verify.
        let pushed = git(seat, &["push", "--set-upstream", "origin", &args.branch])?;
        if !pushed.status.success() {
            return Err(refuse(
                Reason::RemoteConflict,
                format!(
                    "pushing {} refused: {}",
                    args.branch,
                    last_line(&pushed.stderr)
                ),
            ));
        }
        steps.push(format!("pushed {} to origin", args.branch));
    } else {
        steps.push(format!("would push {} to origin", args.branch));
    }
    let next = request_commands(seat, trunk, &args.branch);
    for line in &steps {
        out.result_line(line);
    }
    out.emit(&Report {
        schema: "rk.integrate/1",
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        integration: Integration::Forge.as_str(),
        authority_source,
        branch: args.branch.clone(),
        trunk: trunk.to_owned(),
        seat: seat.to_string(),
        trunk_before: String::new(),
        trunk_commit: None,
        steps,
        next: next.clone(),
    })?;
    out.next(&next);
    Ok(())
}

/// The request command for the forge the remote names, and the rule that
/// the body is the operator's.
fn request_commands(seat: &Utf8Path, trunk: &str, branch: &str) -> Vec<String> {
    let remote = git(seat, &["remote", "get-url", "origin"])
        .ok()
        .filter(|answer| answer.status.success())
        .map(|answer| String::from_utf8_lossy(&answer.stdout).trim().to_owned())
        .unwrap_or_default();
    let create = if remote.contains("gitlab") {
        format!(
            "glab mr create --source-branch {branch} --target-branch {trunk} --squash-before-merge"
        )
    } else {
        format!("gh pr create --base {trunk} --head {branch}")
    };
    vec![
        create,
        "the title is the trunk's commit message, so it is a scoped Conventional Commit".to_owned(),
        "the body is yours; rk message --check --kind body judges it before you post it".to_owned(),
    ]
}

/// The local transaction.
fn local_path(args: &IntegrateArgs, out: Output, run: &LocalRun<'_>) -> Result<(), RkError> {
    let message = args.message.as_deref().unwrap_or_default();
    if let Some(reason) = refuse_message(message) {
        return Err(refuse(Reason::Usage, reason));
    }
    let mut steps = Vec::new();

    // The lock is the common git directory, so two seats of one clone
    // collide on it: they write one trunk ref.
    let _held = crate::landing::lock::acquire(run.git_dir)?;

    refresh_trunk(args, run, &mut steps)?;

    let trunk_before = rev_parse(run.seat, run.trunk)?;

    if !args.apply {
        steps.push(format!(
            "would rebase {} onto {}",
            args.branch,
            integrate::short(&trunk_before)
        ));
        steps.push("would run pre-commit run --hook-stage manual --all-files".to_owned());
        steps.push(format!("would write one squash commit onto {}", run.trunk));
        return report(
            args,
            out,
            run,
            &trunk_before,
            None,
            steps,
            &[format!(
                "rk integrate {} --target {} --apply performs it",
                args.branch, run.target
            )],
        );
    }

    // Bring the branch onto the trunk. A conflict refuses and leaves both
    // refs where it found them.
    let rebased = git(run.seat, &["rebase", &trunk_before])?;
    if !rebased.status.success() {
        let _ = git(run.seat, &["rebase", "--abort"]);
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "{} does not rebase onto {} cleanly: {}",
                args.branch,
                integrate::short(&trunk_before),
                last_line(&rebased.stderr)
            ),
        ));
    }
    steps.push(format!(
        "rebased {} onto {}",
        args.branch,
        integrate::short(&trunk_before)
    ));

    // The gate. One line, one stage, no hook identifier read.
    let gate = gate(run.seat)?;
    if !gate.status.success() {
        out.child_passthrough(crate::events::ChildStream::Stderr, &gate.stderr);
        out.child_passthrough(crate::events::ChildStream::Stdout, &gate.stdout);
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "the manual stage failed in {}, so nothing reached {}",
                run.seat, run.trunk
            ),
        ));
    }
    steps.push("the manual stage passed".to_owned());

    let commit = build_squash(run, &args.branch, &trunk_before, message)?;
    // Publish it with one compare-and-swap carrying the tip observed
    // before the gate ran. A trunk that moved under the gate refuses here
    // and nothing was written.
    publish(run, &commit, &trunk_before)?;
    let branch_tip = rev_parse(run.seat, &args.branch)?;
    steps.push(format!(
        "{} now carries {}",
        run.trunk,
        integrate::short(&commit)
    ));

    // The evidence the prune verbs read, written last.
    let ledger_path = run.git_dir.join(integrate::LEDGER_PATH);
    let mut ledger = read_ledger(&ledger_path)?;
    ledger.record(Entry {
        branch: args.branch.clone(),
        branch_tip,
        trunk_commit: commit.clone(),
        at: crate::landing::manifest::now(),
    });
    write_ledger(&ledger_path, &ledger)?;
    steps.push("recorded the integration".to_owned());

    report(
        args,
        out,
        run,
        &trunk_before,
        Some(commit),
        steps,
        &[
            format!(
                "git -C {} push origin {} pushes the trunk; it is a fast-forward and never a force",
                run.target, run.trunk
            ),
            format!(
                "rk worktree prune --target {} retires the seat, then its branch",
                run.target
            ),
        ],
    )
}

/// The squash commit, as an object nothing references yet.
///
/// `git commit-tree` takes the rebased branch's tree and the trunk tip as
/// its one parent, so the object is a squash by construction and no ref
/// moves. Publishing it is a separate compare-and-swap, which is what
/// makes every refusal before that point leave nothing behind.
fn build_squash(
    run: &LocalRun<'_>,
    branch: &str,
    parent: &str,
    message: &str,
) -> Result<String, RkError> {
    let tree = rev_parse(run.seat, &format!("{branch}^{{tree}}"))?;
    let built = git(
        run.seat,
        &["commit-tree", &tree, "-p", parent, "-m", message],
    )?;
    if !built.status.success() {
        return Err(refuse(
            Reason::Internal,
            format!(
                "the squash commit could not be built: {}",
                last_line(&built.stderr)
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&built.stdout).trim().to_owned())
}

/// Bring the local trunk level with its remote, where a remote answers.
///
/// Only a divergence refuses. A trunk behind its remote fast-forwards,
/// and a trunk ahead of it is the ordinary state under local
/// integration: integrations accumulate and the operator pushes when
/// they decide to.
fn refresh_trunk(
    args: &IntegrateArgs,
    run: &LocalRun<'_>,
    steps: &mut Vec<String>,
) -> Result<(), RkError> {
    let fetched = git(run.seat, &["fetch", "--quiet", "origin"]);
    let Some(remote) = (match fetched {
        Ok(answer) if answer.status.success() => {
            steps.push("fetched origin".to_owned());
            rev_parse(run.seat, &format!("refs/remotes/origin/{}", run.trunk)).ok()
        }
        _ => {
            steps.push("no remote answered; the local trunk stands alone".to_owned());
            None
        }
    }) else {
        return Ok(());
    };
    let local = rev_parse(run.seat, run.trunk)?;
    let state = integrate::trunk_state(
        local == remote,
        is_ancestor(run.seat, &local, &remote),
        is_ancestor(run.seat, &remote, &local),
    );
    if let Some(reason) = integrate::refuse_trunk_state(state) {
        return Err(refuse(Reason::RemoteConflict, reason));
    }
    match state {
        integrate::TrunkState::Behind if args.apply => {
            fast_forward_trunk(run, &remote, &local)?;
            steps.push(format!(
                "fast-forwarded {} to origin/{}",
                run.trunk, run.trunk
            ));
        }
        integrate::TrunkState::Behind => {
            steps.push(format!("would fast-forward {} to its remote", run.trunk));
        }
        integrate::TrunkState::Ahead => {
            steps.push(format!(
                "{} carries integrations nobody pushed yet",
                run.trunk
            ));
        }
        integrate::TrunkState::Level | integrate::TrunkState::Diverged => {}
    }
    Ok(())
}

/// Move the trunk ref forward, through the seat that holds it where one
/// does, so no working tree is left behind its own HEAD.
fn fast_forward_trunk(run: &LocalRun<'_>, to: &str, from: &str) -> Result<(), RkError> {
    trunk_move(run, to, from, "fast-forward")
}

/// Publish the squash commit.
fn publish(run: &LocalRun<'_>, commit: &str, expected: &str) -> Result<(), RkError> {
    trunk_move(run, commit, expected, "integration")
}

/// One trunk move, compare-and-swap against `expected`.
///
/// Where a worktree has the trunk checked out, the move goes through that
/// worktree's own fast-forward merge, so its index and working tree move
/// with HEAD. Where none does, the ref moves directly. Neither form
/// discards anything: a merge that is not a fast-forward refuses, and the
/// direct move refuses on a tip that is not `expected`.
fn trunk_move(run: &LocalRun<'_>, to: &str, expected: &str, what: &str) -> Result<(), RkError> {
    let holder = run
        .seats
        .iter()
        .find(|seat| seat.branch.as_deref() == Some(run.trunk));
    if let Some(holder) = holder {
        if holder.head != expected {
            return Err(moved(run.trunk, expected, &holder.head));
        }
        if is_dirty(&holder.path)? {
            return Err(refuse(
                Reason::StateDrift,
                format!(
                    "{} has the trunk checked out and carries uncommitted changes, so the {what} would leave it behind its own HEAD",
                    holder.path
                ),
            ));
        }
        let merged = git(&holder.path, &["merge", "--ff-only", to])?;
        if !merged.status.success() {
            return Err(refuse(
                Reason::StateDrift,
                format!(
                    "the {what} is not a fast-forward of {}: {}",
                    holder.path,
                    last_line(&merged.stderr)
                ),
            ));
        }
        return Ok(());
    }
    let reference = format!("refs/heads/{}", run.trunk);
    let swapped = git(run.seat, &["update-ref", &reference, to, expected])?;
    if !swapped.status.success() {
        let now = rev_parse(run.seat, run.trunk).unwrap_or_else(|_| "an unreadable tip".to_owned());
        return Err(moved(run.trunk, expected, &now));
    }
    Ok(())
}

/// The refusal for a trunk that moved between the observation and the write.
fn moved(trunk: &str, expected: &str, now: &str) -> RkError {
    refuse(
        Reason::StateDrift,
        integrate::refuse_moved_trunk(expected, now).unwrap_or_else(|| {
            format!(
                "{trunk} could not be moved and stands at {}",
                integrate::short(now)
            )
        }),
    )
}

/// Run the pre-integrate gate: one stage, one command.
fn gate(seat: &Utf8Path) -> Result<std::process::Output, RkError> {
    let mut command = std::process::Command::new("pre-commit");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .current_dir(seat.as_std_path())
        .args(["run", "--hook-stage", "manual", "--all-files"])
        .output()
        .map_err(|source| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("pre-commit did not run in {seat}: {source}"),
                )
                .expected(
                    "pre-commit on PATH, which is what installs and runs this project's hooks",
                )
                .action("install pre-commit, or enter the project's devshell, and run it again")
                .target_state("unchanged"),
            )
        })
}

/// Emit one report.
fn report(
    args: &IntegrateArgs,
    out: Output,
    run: &LocalRun<'_>,
    trunk_before: &str,
    trunk_commit: Option<String>,
    steps: Vec<String>,
    next: &[String],
) -> Result<(), RkError> {
    for step in &steps {
        out.result_line(step);
    }
    out.emit(&Report {
        schema: "rk.integrate/1",
        mode: if args.apply { "apply" } else { "preview" },
        target: run.target.to_string(),
        integration: Integration::Local.as_str(),
        authority_source: run.authority_source,
        branch: args.branch.clone(),
        trunk: run.trunk.to_owned(),
        seat: run.seat.to_string(),
        trunk_before: trunk_before.to_owned(),
        trunk_commit,
        steps,
        next: next.to_vec(),
    })?;
    out.next(next);
    Ok(())
}

/// The shape a trunk commit message states, named once.
const SHAPE: &str = "a scoped Conventional Commit: <type>(<scope>): <description>";

/// The Conventional Commit types the branch grammar admits, which is the
/// same set the landed `rk-branch-name` hook tests.
const TYPES: [&str; 11] = [
    "build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test",
];

/// Why a trunk commit message cannot be written, or `None`.
///
/// `git commit-tree` fires no hook, so the guards the landed commit-msg
/// stage would have applied are applied here instead: the subject is a
/// Conventional Commit, its type is one the branch grammar admits, and
/// its scope is present and shaped the way the forge's title check reads.
#[must_use]
fn refuse_message(text: &str) -> Option<String> {
    let subject = text.lines().next().unwrap_or("").trim();
    if subject.is_empty() {
        return Some(
            "a local integration writes the trunk's commit message, so --message is required"
                .to_owned(),
        );
    }
    let Some((head, description)) = subject.split_once(": ") else {
        return Some(format!("'{subject}' is not {SHAPE}"));
    };
    if description.trim().is_empty() {
        return Some(format!(
            "'{subject}' is not {SHAPE}: it states no description"
        ));
    }
    // A breaking `!` sits after the scope, so it comes off the head
    // before the scope is read out of it.
    let head = head.strip_suffix('!').unwrap_or(head);
    let Some((kind, scope)) = head.split_once('(') else {
        return Some(format!("'{subject}' is not {SHAPE}: it names no scope"));
    };
    let Some(scope) = scope.strip_suffix(')') else {
        return Some(format!("'{subject}' is not {SHAPE}: its scope is unclosed"));
    };
    if !TYPES.contains(&kind) {
        return Some(format!(
            "'{kind}' is not a Conventional Commit type; the types are: {}",
            TYPES.join(", ")
        ));
    }
    if !crate::projection::scope_is_shaped(scope) {
        return Some(format!(
            "the scope '{scope}' is outside {}: lowercase letters, digits, and _ . / -",
            crate::projection::SCOPE_SHAPE
        ));
    }
    None
}

/// Every worktree this clone registers.
fn seats(target: &Utf8Path) -> Result<Vec<crate::worktree::Worktree>, RkError> {
    let listed = git(target, &["worktree", "list", "--porcelain", "-z"])?;
    if !listed.status.success() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a git repository"),
            )
            .expected("a repository whose worktrees git can list"),
        ));
    }
    crate::worktree::parse_worktrees(&listed.stdout)
        .map_err(|detail| refuse(Reason::PrerequisiteUnmet, detail))
}

/// The worktree seating one branch.
fn seat_for(
    seats: &[crate::worktree::Worktree],
    branch: &str,
    target: &Utf8Path,
) -> Result<Utf8PathBuf, RkError> {
    seats
        .iter()
        .find(|seat| seat.branch.as_deref() == Some(branch))
        .map(|seat| seat.path.clone())
        .ok_or_else(|| {
            refuse(
                Reason::StateDrift,
                format!(
                    "no worktree of {target} has {branch} checked out; rk worktree add {branch} --apply seats it"
                ),
            )
        })
}

/// Whether a working tree carries uncommitted changes, untracked included.
fn is_dirty(seat: &Utf8Path) -> Result<bool, RkError> {
    let status = git(seat, &["status", "--porcelain"])?;
    Ok(!status.stdout.is_empty())
}

/// One revision's full object name.
fn rev_parse(seat: &Utf8Path, revision: &str) -> Result<String, RkError> {
    let answer = git(seat, &["rev-parse", "--verify", "--quiet", revision])?;
    if !answer.status.success() {
        return Err(refuse(
            Reason::StateDrift,
            format!("{revision} does not resolve in {seat}"),
        ));
    }
    Ok(String::from_utf8_lossy(&answer.stdout).trim().to_owned())
}

/// Whether `ancestor` is reachable from `descendant`.
fn is_ancestor(seat: &Utf8Path, ancestor: &str, descendant: &str) -> bool {
    git(seat, &["merge-base", "--is-ancestor", ancestor, descendant])
        .is_ok_and(|answer| answer.status.success())
}

/// The clone's common git directory, which every linked seat shares.
fn common_git_dir(target: &Utf8Path) -> Result<Utf8PathBuf, RkError> {
    let answer = git(
        target,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    if !answer.status.success() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a git repository"),
            )
            .expected("a repository whose common git directory git can name"),
        ));
    }
    let path = String::from_utf8_lossy(&answer.stdout).trim().to_owned();
    Utf8PathBuf::from_path_buf(std::path::PathBuf::from(path)).map_err(|path| {
        refuse(
            Reason::PrerequisiteUnmet,
            format!("the common git directory {} is not UTF-8", path.display()),
        )
    })
}

/// Read the ledger, or an empty one where none exists.
fn read_ledger(path: &Utf8Path) -> Result<Ledger, RkError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            Ledger::parse(&text).map_err(|detail| refuse(Reason::UnsupportedSchema, detail))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::default()),
        Err(error) => Err(RkError::Io(error)),
    }
}

/// Write the ledger, atomically.
fn write_ledger(path: &Utf8Path, ledger: &Ledger) -> Result<(), RkError> {
    let text = ledger
        .render()
        .map_err(|detail| refuse(Reason::Internal, detail))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::atomic::write(path.as_std_path(), text.as_bytes())?;
    Ok(())
}

/// One refusal that states the target was left alone.
fn refuse(reason: Reason, message: impl Into<String>) -> RkError {
    RkError::refusal(
        Diagnostic::new(reason, message).target_state("unchanged; the trunk stands where it stood"),
    )
}

/// Run one git command against a directory; a spawn failure refuses.
fn git(at: &Utf8Path, args: &[&str]) -> Result<std::process::Output, RkError> {
    let mut command = std::process::Command::new(crate::probes::git_bin());
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .arg("-C")
        .arg(at.as_std_path())
        .args(args)
        .output()
        .map_err(|source| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("git did not run in {at}: {source}"),
                )
                .target_state("unchanged"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::refuse_message;

    #[test]
    fn a_trunk_message_is_held_to_the_landed_convention() {
        assert_eq!(refuse_message("feat(integrate): land the verb"), None);
        assert_eq!(refuse_message("feat(a/b)!: break it"), None);
        assert!(
            refuse_message("")
                .expect("an empty message refuses")
                .contains("--message is required")
        );
        assert!(
            refuse_message("land the verb")
                .expect("an unscoped subject refuses")
                .contains("Conventional Commit")
        );
        assert!(
            refuse_message("feat: land the verb")
                .expect("a missing scope refuses")
                .contains("Conventional Commit")
        );
        assert!(
            refuse_message("wat(integrate): land it")
                .expect("an unknown type refuses")
                .contains("not a Conventional Commit type")
        );
        assert!(
            refuse_message("feat(Integrate): land it")
                .expect("a misshapen scope refuses")
                .contains("outside")
        );
    }
}
