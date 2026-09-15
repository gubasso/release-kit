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
/// The record answers, because the record is what landed: the hook block
/// that admits or refuses these writes, the routing block an agent reads,
/// and the forge protections the setup installed all render from it. A
/// configuration edit is pending input to the next landing, never a
/// runtime override — taking it here would run a local integration in a
/// target whose installed controls still say forge, which is the split
/// authority `target-config:the-config-is-input-and-the-record-is-the-record`
/// exists to prevent. `--local` and `--forge` override one execution and
/// write nothing.
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
    Ok(recorded.map_or(
        (crate::landing::manifest::integration_forge(), "default"),
        |mode| (mode, "record"),
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
///
/// The preview comes first and takes no lock, runs no fetch, and touches
/// no ref: a dry run that refreshed remote-tracking refs and wrote
/// `FETCH_HEAD` would make its own promise false.
///
/// The apply reads its evidence ledger before it builds anything, so a
/// ledger this binary cannot parse refuses while the trunk still stands
/// where it stood. Publishing is the last failure point that moves a ref.
fn local_path(args: &IntegrateArgs, out: Output, run: &LocalRun<'_>) -> Result<(), RkError> {
    let message = args.message.as_deref().unwrap_or_default();
    refuse_message(run.target, message)?;
    let mut steps = Vec::new();

    if !args.apply {
        let trunk_before = rev_parse(run.seat, run.trunk)?;
        steps.push("no fetch and no lock: a preview refreshes nothing".to_owned());
        steps.push(format!(
            "would rebase {} onto {}, or onto whatever the fetch brings",
            args.branch,
            integrate::short(&trunk_before)
        ));
        steps.push("would run the manual stage in the seat".to_owned());
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

    // The lock is the common git directory, so two seats of one clone
    // collide on it: they write one trunk ref.
    let _held = crate::landing::lock::acquire(run.git_dir)?;

    // The evidence ledger is read and judged before anything is built, so
    // an unreadable one refuses with the trunk untouched rather than
    // after the squash is published.
    let ledger_path = run.git_dir.join(integrate::LEDGER_PATH);
    let mut ledger = read_ledger(&ledger_path)?;

    refresh_trunk(run, &mut steps)?;

    let trunk_before = rev_parse(run.seat, run.trunk)?;

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

    // One observation of the branch feeds both the tree that is
    // integrated and the tip the evidence certifies. Reading them
    // separately would let a commit landing between the two write
    // evidence for a tip whose work never reached the trunk, and the
    // prune predicate would then accept it and delete that work. The
    // lock bounds this binary's own runs and bounds no hand at the desk.
    let branch_tip = rev_parse(run.seat, &args.branch)?;
    let commit = build_squash(run, &branch_tip, &trunk_before, message)?;
    // Publish it with one compare-and-swap carrying the tip observed
    // before the gate ran. A trunk that moved under the gate refuses here
    // and nothing was written.
    publish(run, &commit, &trunk_before)?;
    steps.push(format!(
        "{} now carries {}",
        run.trunk,
        integrate::short(&commit)
    ));

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
    branch_tip: &str,
    parent: &str,
    message: &str,
) -> Result<String, RkError> {
    let tree = rev_parse(run.seat, &format!("{branch_tip}^{{tree}}"))?;
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
fn refresh_trunk(run: &LocalRun<'_>, steps: &mut Vec<String>) -> Result<(), RkError> {
    // An absent origin and an origin that will not answer are different
    // facts. A project with no remote integrates against its local trunk
    // alone; a project whose remote exists and cannot be reached may have
    // a newer or divergent trunk behind that failure, so proceeding from
    // stale local state could publish an integration nobody can push.
    let named = git(run.seat, &["remote", "get-url", "origin"])?;
    if !named.status.success() {
        steps.push("no origin remote; the local trunk stands alone".to_owned());
        return Ok(());
    }
    let fetched = git(run.seat, &["fetch", "--quiet", "origin"])?;
    if !fetched.status.success() {
        return Err(refuse(
            Reason::ForgeTemporary,
            format!(
                "origin is configured and did not answer, so the trunk could not be refreshed: {}",
                last_line(&fetched.stderr)
            ),
        ));
    }
    steps.push("fetched origin".to_owned());
    let Some(remote) = rev_parse(run.seat, &format!("refs/remotes/origin/{}", run.trunk)).ok()
    else {
        steps.push(format!(
            "origin carries no {} yet; the local trunk stands alone",
            run.trunk
        ));
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
        integrate::TrunkState::Behind => {
            fast_forward_trunk(run, &remote, &local)?;
            steps.push(format!(
                "fast-forwarded {} to origin/{}",
                run.trunk, run.trunk
            ));
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

/// Refuse a trunk commit message the landed guards would refuse.
///
/// `git commit-tree` fires no hook, so the whole `commit-msg` stage is
/// applied here instead, and it is applied through the one owner rather
/// than a second, weaker copy: the Conventional Commit shape the
/// `conventional-pre-commit` hook holds, and then every finding
/// `rk message --check` reports — agent attribution, a reference to a
/// path the target ignores, and a scope outside the title check's shape.
/// A message that reaches the trunk here reaches a permanent history and
/// a forge-facing changelog, so the two paths judge one set.
///
/// # Errors
///
/// Returns [`RkError::Refusal`] naming what a landed guard would have
/// refused, with the target untouched.
fn refuse_message(target: &Utf8Path, text: &str) -> Result<(), RkError> {
    let subject = text.lines().next().unwrap_or("").trim();
    if subject.is_empty() {
        return Err(refuse(
            Reason::Usage,
            "a local integration writes the trunk's commit message, so --message is required",
        ));
    }
    if let Some(reason) = misshapen_subject(subject) {
        return Err(refuse(Reason::Usage, reason));
    }
    let (findings, _) = crate::commands::message::judge(
        text,
        crate::cli::message::MessageKind::Commit,
        target,
        subject,
        crate::commands::message::exempt_title(subject),
    );
    if findings.is_empty() {
        return Ok(());
    }
    let named: Vec<String> = findings
        .iter()
        .map(|finding| format!("{}:{} {}", finding.class, finding.line, finding.detail))
        .collect();
    Err(refuse(
        Reason::Usage,
        format!(
            "the trunk message carries {} finding{} the landed commit-msg stage would refuse, and git commit-tree fires no hook: {}",
            findings.len(),
            if findings.len() == 1 { "" } else { "s" },
            named.join("; ")
        ),
    ))
}

/// Why a subject is not a scoped Conventional Commit, or `None`.
#[must_use]
fn misshapen_subject(subject: &str) -> Option<String> {
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
    use camino::Utf8Path;

    use super::{misshapen_subject, refuse_message};

    #[test]
    fn a_trunk_subject_is_held_to_the_landed_convention() {
        assert_eq!(misshapen_subject("feat(integrate): land the verb"), None);
        assert_eq!(misshapen_subject("feat(a/b)!: break it"), None);
        assert!(
            misshapen_subject("land the verb")
                .expect("an unscoped subject refuses")
                .contains("Conventional Commit")
        );
        assert!(
            misshapen_subject("feat: land the verb")
                .expect("a missing scope refuses")
                .contains("names no scope")
        );
        assert!(
            misshapen_subject("feat(integrate):   ")
                .expect("an empty description refuses")
                .contains("no description")
        );
        assert!(
            misshapen_subject("wat(integrate): land it")
                .expect("an unknown type refuses")
                .contains("not a Conventional Commit type")
        );
        assert!(
            misshapen_subject("feat(Integrate): land it")
                .expect("a misshapen scope refuses")
                .contains("outside")
        );
    }

    /// The whole landed commit-msg stage runs here, not the subject
    /// alone: `git commit-tree` fires no hook, so a body the landed
    /// guard refuses must refuse here or it reaches a permanent history.
    #[test]
    fn a_trunk_body_is_judged_by_the_one_message_owner() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let target = Utf8Path::from_path(dir.path()).expect("utf-8");
        refuse_message(target, "feat(integrate): land the verb\n\nThe context.\n")
            .expect("a clean message passes");
        let error = refuse_message(
            target,
            "feat(integrate): land the verb\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n",
        )
        .expect_err("agent attribution refuses")
        .to_string();
        assert!(error.contains("attribution"), "{error}");
        assert!(error.contains("commit-tree fires no hook"), "{error}");
        let error = refuse_message(
            target,
            "feat(integrate): land the verb\n\nSee .draft/plan.md for the rest.\n",
        )
        .expect_err("an internal path refuses")
        .to_string();
        assert!(error.contains("internal-path"), "{error}");
        let error = refuse_message(target, "")
            .expect_err("an empty message refuses")
            .to_string();
        assert!(error.contains("--message is required"), "{error}");
    }
}
