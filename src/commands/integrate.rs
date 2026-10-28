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
//! Under the `rebase` merge form the local path builds no commit at all:
//! it publishes the rebased branch tip with the same compare-and-swap, so
//! every commit of the branch reaches the trunk with its own message, and
//! each of those messages is judged first.
//!
//! SATISFIES git:a-local-integration-is-a-transaction
//! SATISFIES git:a-rebase-integration-publishes-the-branch-tip
//! SATISFIES git:the-manual-stage-is-the-pre-integrate-contract
//! SATISFIES git:a-local-integration-warns-of-an-uncounted-release

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::cli::integrate::IntegrateArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::integrate::{self, Entry, Ledger};
use crate::landing::{Integration, MergeForm};
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
    /// The recorded merge form: `squash` or `rebase`.
    merge_form: &'static str,
    /// The branch integrated.
    branch: String,
    /// The trunk it integrated onto.
    trunk: String,
    /// The seat the gate ran in.
    seat: String,
    /// The trunk tip before this integration.
    trunk_before: String,
    /// The commit the trunk now carries, on an applied local integration
    /// alone: the squash commit, or the branch tip under `rebase`.
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
    let merge_form = recorded_merge_form(target)?;
    let scope_required = recorded_scope_rule(target)?;
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

    let run = LocalRun {
        target,
        git_dir: &git_dir,
        seat: &seat,
        seats: &seats,
        trunk: &trunk,
        authority_source,
        merge_form,
        scope_required,
    };
    match (integration, merge_form) {
        (Integration::Forge, _) => forge_path(
            args,
            out,
            &seat,
            &trunk,
            authority_source,
            merge_form,
            scope_required,
        ),
        (Integration::Local, MergeForm::Squash) => local_path(args, out, &run),
        (Integration::Local, MergeForm::Rebase) => rebase_path(args, out, &run),
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
    merge_form: MergeForm,
    /// Whether a trunk message must name a scope: wherever the record
    /// names a release, because the changelog reads it.
    scope_required: bool,
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

/// The merge form this execution uses: the record's, as for the authority,
/// and `squash` where no record exists or the record predates the form.
///
/// No flag overrides it. The authority flags exist because the same
/// branch can reach the trunk either way, and the form decides which
/// messages the trunk keeps, which the landed forge settings and gates
/// were rendered for.
fn recorded_merge_form(target: &Utf8Path) -> Result<MergeForm, RkError> {
    Ok(crate::landing::manifest::load(target)?
        .map_or_else(crate::landing::manifest::merge_form_squash, |record| {
            record.git.merge_form
        }))
}

/// Whether a trunk message must name a scope, as the record says: a
/// release reads the scope, so every mode but `none` requires one, and a
/// target with no record keeps the requirement it always had.
///
/// SATISFIES project-profile:a-release-less-landing-carries-no-release-text
fn recorded_scope_rule(target: &Utf8Path) -> Result<bool, RkError> {
    Ok(crate::landing::manifest::load(target)?
        .is_none_or(|record| record.profile.release.mode != crate::profile::ReleaseMode::None))
}

/// The forge path: gate through the push, then name the request command.
fn forge_path(
    args: &IntegrateArgs,
    out: Output,
    seat: &Utf8Path,
    trunk: &str,
    authority_source: &'static str,
    merge_form: MergeForm,
    scope_required: bool,
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
    let next = request_commands(
        &origin_url(seat),
        trunk,
        &args.branch,
        merge_form,
        scope_required,
    );
    for line in &steps {
        out.result_line(line);
    }
    out.emit(&Report {
        schema: "rk.integrate/1",
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        integration: Integration::Forge.as_str(),
        authority_source,
        merge_form: merge_form.as_str(),
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

/// The URL of the seat's `origin`, or empty where none answers.
fn origin_url(seat: &Utf8Path) -> String {
    git(seat, &["remote", "get-url", "origin"])
        .ok()
        .filter(|answer| answer.status.success())
        .map(|answer| String::from_utf8_lossy(&answer.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// The request command for the forge the remote names, and what reaches
/// the trunk from the request under the recorded merge form.
///
/// Under `squash` the request's title becomes the trunk's one commit, and
/// GitLab is asked to squash. Under `rebase` every commit of the branch
/// reaches the trunk with its own message, so no squash is asked for and
/// each message is the one the commit-msg judgment holds. Where the record
/// names no release, the guidance asks for a Conventional Commit whose
/// scope is preferred and not required, because no changelog reads it.
///
/// SATISFIES git:the-merge-form-is-a-landing-parameter
/// SATISFIES project-profile:a-release-less-landing-carries-no-release-text
fn request_commands(
    remote: &str,
    trunk: &str,
    branch: &str,
    form: MergeForm,
    scope_required: bool,
) -> Vec<String> {
    let squash = form == MergeForm::Squash;
    let create = if remote.contains("gitlab") {
        format!(
            "glab mr create --source-branch {branch} --target-branch {trunk}{}",
            if squash { " --squash-before-merge" } else { "" }
        )
    } else {
        format!("gh pr create --base {trunk} --head {branch}")
    };
    let reaches = match (squash, scope_required) {
        (true, true) => {
            "the title is the trunk's commit message, so it is a scoped Conventional Commit"
        }
        (true, false) => {
            "the title is the trunk's commit message, so it is a Conventional Commit, whose scope is preferred and not required"
        }
        (false, true) => {
            "every commit of the branch reaches the trunk with its own message, so each one passes the commit-msg judgment: a scoped Conventional Commit with no guarded content"
        }
        (false, false) => {
            "every commit of the branch reaches the trunk with its own message, so each one passes the commit-msg judgment: a Conventional Commit, whose scope is preferred and not required, with no guarded content"
        }
    };
    vec![
        create,
        reaches.to_owned(),
        "the body is yours; rk message --check --kind body judges it before you post it".to_owned(),
    ]
}

/// Rebase the branch in its seat onto `onto`, with the integration's own
/// semantics rather than the clone's preferences.
///
/// Both local merge forms run this one rebase. A merge in the branch is
/// flattened, never recreated, whatever `rebase.rebaseMerges` says. No
/// other branch moves, whatever `rebase.updateRefs` says, because the
/// trunk is the one ref an integration publishes. No `fixup!` commit is
/// folded and nothing is stashed, so the commits the gate judges are the
/// commits the branch carries. A conflict refuses and leaves every ref
/// where it found it.
///
/// SATISFIES git:a-rebase-integration-publishes-the-branch-tip
fn rebase_onto(run: &LocalRun<'_>, branch: &str, onto: &str) -> Result<(), RkError> {
    let rebased = git(
        run.seat,
        &REBASE_ARGS
            .iter()
            .copied()
            .chain([onto])
            .collect::<Vec<_>>(),
    )?;
    if !rebased.status.success() {
        let _ = git(run.seat, &["rebase", "--abort"]);
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "{branch} does not rebase onto {} cleanly: {}",
                integrate::short(onto),
                last_line(&rebased.stderr)
            ),
        ));
    }
    Ok(())
}

/// The rebase every local integration runs, before the commit it rebases
/// onto: each option overrides the clone's configured preference of the
/// same name.
const REBASE_ARGS: [&str; 5] = [
    "rebase",
    "--no-rebase-merges",
    "--no-update-refs",
    "--no-autosquash",
    "--no-autostash",
];

/// Refuse a range to publish that holds a merge commit.
///
/// The rebase flattens merges, and this does not trust it to: the trunk
/// takes no merge commit from any actor, so the range is read back before
/// the publication, and a merge in it refuses with the trunk unmoved.
///
/// SATISFIES git:a-rebase-integration-publishes-the-branch-tip
fn refuse_merges_in_range(run: &LocalRun<'_>, from: &str, tip: &str) -> Result<(), RkError> {
    let listed = git(
        run.seat,
        &["rev-list", "--merges", &format!("{from}..{tip}")],
    )?;
    if !listed.status.success() {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "the range {}..{} could not be read for merge commits: {}",
                integrate::short(from),
                integrate::short(tip),
                last_line(&listed.stderr)
            ),
        ));
    }
    let merges: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(integrate::short)
        .collect();
    if merges.is_empty() {
        return Ok(());
    }
    Err(refuse(
        Reason::StateDrift,
        format!(
            "the rebased range {}..{} still holds the merge commit {}, and the trunk takes no merge commit, so nothing was published",
            integrate::short(from),
            integrate::short(tip),
            merges.join(", ")
        ),
    ))
}

/// The dry run, which refreshes nothing.
///
/// No lock and no fetch: a preview that moved remote-tracking refs and
/// wrote `FETCH_HEAD` would make its own promise false, and it would
/// also report a plan built on state it changed while reporting it.
fn preview(args: &IntegrateArgs, out: Output, run: &LocalRun<'_>) -> Result<(), RkError> {
    let mut steps = Vec::new();
    let trunk_before = rev_parse(run.seat, run.trunk)?;
    warn_if_uncounted(
        out,
        run,
        args.message.as_deref().unwrap_or_default(),
        &[&format!("{trunk_before}...{}", args.branch)],
        true,
    );
    steps.push("no fetch and no lock: a preview refreshes nothing".to_owned());
    steps.push(format!(
        "would rebase {} onto {}, or onto whatever the fetch brings",
        args.branch,
        integrate::short(&trunk_before)
    ));
    steps.push("would run the manual stage in the seat".to_owned());
    steps.push(format!("would write one squash commit onto {}", run.trunk));
    report(
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
    )
}

/// The local transaction.
///
/// It reads its evidence ledger before it builds anything, so a ledger
/// this binary cannot parse refuses while the trunk still stands where it
/// stood, and it stages the evidence before it publishes, so every
/// fallible part of writing it happens with the trunk unmoved. The one
/// ref this moves is the trunk, through [`trunk_move`]'s publication.
fn local_path(args: &IntegrateArgs, out: Output, run: &LocalRun<'_>) -> Result<(), RkError> {
    let message = args.message.as_deref().unwrap_or_default();
    refuse_message(run.seat, message, run.scope_required)?;
    let mut steps = Vec::new();

    if !args.apply {
        return preview(args, out, run);
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
    rebase_onto(run, &args.branch, &trunk_before)?;
    steps.push(format!(
        "rebased {} onto {}",
        args.branch,
        integrate::short(&trunk_before)
    ));

    // The authoritative observation of the branch, taken before the gate
    // so the tree the gate judges is the tree that reaches the trunk. It
    // is re-observed below: a commit landing in the seat under the gate
    // would otherwise be squashed without having passed it.
    let branch_tip = rev_parse(run.seat, &args.branch)?;

    // Before the gate, so the operator reads it while the trunk still
    // stands where it stood and can stop to retype the message.
    warn_if_uncounted(out, run, message, &[&trunk_before, &branch_tip], false);

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

    // The re-observation. One object feeds the tree that is integrated
    // and the tip the evidence certifies, and that object is the one the
    // gate judged. The lock bounds this binary's own runs and bounds no
    // hand at the desk, so both directions are checked rather than
    // assumed.
    let now = rev_parse(run.seat, &args.branch)?;
    if now != branch_tip {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "{} moved from {} to {} while the gate ran, so the gate judged a tree that is no longer the branch's",
                args.branch,
                integrate::short(&branch_tip),
                integrate::short(&now)
            ),
        ));
    }
    let commit = build_squash(run, &branch_tip, &trunk_before, message)?;
    // Publish it against the tip observed before the gate ran. A trunk
    // that moved under the gate refuses here and nothing was written.
    // The evidence is rendered and staged before the publication, so
    // every fallible part of writing it happens while the trunk still
    // stands where it stood. Evidence naming a commit no ref carries
    // proves nothing: both prune verbs require the trunk to reach the
    // recorded commit, so a staged entry whose publication then fails is
    // inert rather than dangerous.
    ledger.record(Entry {
        branch: args.branch.clone(),
        branch_tip,
        trunk_commit: commit.clone(),
        at: crate::landing::manifest::now(),
    });
    write_ledger(&ledger_path, &ledger)?;

    let drift = publish(run, &commit, &trunk_before)?;
    steps.push(format!(
        "{} now carries {}",
        run.trunk,
        integrate::short(&commit)
    ));
    steps.push("recorded the integration".to_owned());
    if let Some(drift) = &drift {
        steps.push(drift.step(run.trunk));
    }

    report(
        args,
        out,
        run,
        &trunk_before,
        Some(commit.clone()),
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
    )?;
    drift.map_or(Ok(()), |drift| Err(drift.into_error(run.trunk, &commit)))
}

/// The local transaction under the `rebase` merge form.
///
/// It is the squash transaction with the build step removed: the same
/// lock, ledger read, refresh, rebase, gate, and re-observation, and the
/// same publication, which moves the trunk to the rebased branch tip
/// itself once the range is read back with no merge commit in it.
/// Every commit between the trunk and that tip becomes a trunk commit, so
/// each message is judged before the gate, and none is written here.
///
/// SATISFIES git:a-rebase-integration-publishes-the-branch-tip
fn rebase_path(args: &IntegrateArgs, out: Output, run: &LocalRun<'_>) -> Result<(), RkError> {
    refuse_message_under_rebase(args)?;
    if !args.apply {
        return rebase_preview(args, out, run);
    }
    let mut steps = Vec::new();

    let _held = crate::landing::lock::acquire(run.git_dir)?;
    let ledger_path = run.git_dir.join(integrate::LEDGER_PATH);
    let mut ledger = read_ledger(&ledger_path)?;

    refresh_trunk(run, &mut steps)?;

    let trunk_before = rev_parse(run.seat, run.trunk)?;

    rebase_onto(run, &args.branch, &trunk_before)?;
    steps.push(format!(
        "rebased {} onto {}",
        args.branch,
        integrate::short(&trunk_before)
    ));

    // The one observation the judgment, the gate, the evidence, and the
    // publication all rest on; it is re-observed after the gate.
    let branch_tip = rev_parse(run.seat, &args.branch)?;
    let commits = branch_commits(run.seat, &trunk_before, &branch_tip, &args.branch)?;
    refuse_commit_messages(run.seat, &commits, run.scope_required)?;
    steps.push(format!(
        "judged the message of each of {} commit{}",
        commits.len(),
        if commits.len() == 1 { "" } else { "s" }
    ));

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

    let now = rev_parse(run.seat, &args.branch)?;
    if now != branch_tip {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "{} moved from {} to {} while the gate ran, so the gate judged a tree that is no longer the branch's",
                args.branch,
                integrate::short(&branch_tip),
                integrate::short(&now)
            ),
        ));
    }

    // The range to publish holds no merge commit, whatever the rebase
    // above did with the clone's preferences.
    refuse_merges_in_range(run, &trunk_before, &branch_tip)?;

    // The evidence names the branch tip as the integrated commit, staged
    // before the publication like the squash form's, and inert in the
    // same way where the publication then refuses: no prune reads an
    // entry whose commit the trunk does not reach.
    ledger.record(Entry {
        branch: args.branch.clone(),
        branch_tip: branch_tip.clone(),
        trunk_commit: branch_tip.clone(),
        at: crate::landing::manifest::now(),
    });
    write_ledger(&ledger_path, &ledger)?;

    let drift = publish(run, &branch_tip, &trunk_before)?;
    steps.push(format!(
        "{} now carries {} at {}, with no new commit",
        run.trunk,
        args.branch,
        integrate::short(&branch_tip)
    ));
    steps.push("recorded the integration".to_owned());
    if let Some(drift) = &drift {
        steps.push(drift.step(run.trunk));
    }

    report(
        args,
        out,
        run,
        &trunk_before,
        Some(branch_tip.clone()),
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
    )?;
    drift.map_or(
        Ok(()),
        |drift| Err(drift.into_error(run.trunk, &branch_tip)),
    )
}

/// The dry run under `rebase`, which refreshes nothing and writes nothing.
///
/// It judges the messages the branch carries now: a rebase keeps every
/// message, so a refusal the apply would reach is named here first.
fn rebase_preview(args: &IntegrateArgs, out: Output, run: &LocalRun<'_>) -> Result<(), RkError> {
    let trunk_before = rev_parse(run.seat, run.trunk)?;
    let branch_tip = rev_parse(run.seat, &args.branch)?;
    let commits = branch_commits(run.seat, &trunk_before, &branch_tip, &args.branch)?;
    refuse_commit_messages(run.seat, &commits, run.scope_required)?;
    let steps = vec![
        "no fetch and no lock: a preview refreshes nothing".to_owned(),
        format!(
            "would rebase {} onto {}, or onto whatever the fetch brings",
            args.branch,
            integrate::short(&trunk_before)
        ),
        "would run the manual stage in the seat".to_owned(),
        format!(
            "would move {} to the tip of {}, carrying its {} commit{} with their own messages and writing no new commit",
            run.trunk,
            args.branch,
            commits.len(),
            if commits.len() == 1 { "" } else { "s" }
        ),
    ];
    report(
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
    )
}

/// Refuse a trunk message under `rebase`, where no commit takes one.
///
/// A message given here would reach nothing: every commit of the branch
/// keeps its own, and the integration writes no commit to carry it. A
/// silent drop would let the operator believe it landed.
fn refuse_message_under_rebase(args: &IntegrateArgs) -> Result<(), RkError> {
    if args.message.is_none() {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::MergeFormMismatch,
            "-m names a trunk message, and the recorded merge form is rebase: every commit of the branch reaches the trunk with its own message, and no commit is written to carry this one",
        )
        .expected("no -m under git.merge_form = \"rebase\"")
        .action(format!(
            "rk integrate {} --apply integrates without it; git rebase -i on the branch rewords a commit",
            args.branch
        ))
        .target_state("unchanged; the trunk stands where it stood"),
    ))
}

/// One commit the rebase form publishes, with its message.
struct BranchCommit {
    id: String,
    message: String,
}

/// Every commit between the trunk and the branch tip, oldest first.
///
/// A branch with no commit of its own has nothing to integrate, and a
/// publication that moved nothing would still write evidence naming it.
fn branch_commits(
    seat: &Utf8Path,
    trunk: &str,
    tip: &str,
    branch: &str,
) -> Result<Vec<BranchCommit>, RkError> {
    let listed = git(seat, &["rev-list", "--reverse", &format!("{trunk}..{tip}")])?;
    if !listed.status.success() {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "the commits of {branch} could not be listed: {}",
                last_line(&listed.stderr)
            ),
        ));
    }
    let ids: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    if ids.is_empty() {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "{branch} carries no commit {} does not already carry, so there is nothing to integrate",
                integrate::short(trunk)
            ),
        ));
    }
    ids.into_iter()
        .map(|id| {
            let shown = git(seat, &["log", "-1", "--format=%B", &id])?;
            if !shown.status.success() {
                return Err(refuse(
                    Reason::StateDrift,
                    format!(
                        "the message of {} could not be read: {}",
                        integrate::short(&id),
                        last_line(&shown.stderr)
                    ),
                ));
            }
            Ok(BranchCommit {
                message: String::from_utf8_lossy(&shown.stdout).into_owned(),
                id,
            })
        })
        .collect()
}

/// Refuse the first branch commit whose message the landed `commit-msg`
/// stage would refuse.
///
/// The move that publishes these commits fires no hook, so the whole
/// stage is applied here through the one owner the squash form uses: the
/// scoped Conventional Commit shape, then every finding
/// `rk message --check` reports. `at` is the seat, for the same reason as
/// the squash form's: the ignore rules a message is judged against are
/// the branch's.
fn refuse_commit_messages(
    at: &Utf8Path,
    commits: &[BranchCommit],
    scope_required: bool,
) -> Result<(), RkError> {
    for commit in commits {
        let subject = commit.message.lines().next().unwrap_or("").trim();
        let named = format!("commit {}", integrate::short(&commit.id));
        if subject.is_empty() {
            return Err(refuse(
                Reason::Usage,
                format!(
                    "{named} carries no message, and every commit reaches the trunk with its own"
                ),
            ));
        }
        if let Some(reason) = misshapen_subject(subject, scope_required) {
            return Err(refuse(Reason::Usage, format!("{named}: {reason}")));
        }
        let (findings, _) = crate::commands::message::judge(
            &commit.message,
            crate::cli::message::MessageKind::Commit,
            at,
            subject,
            crate::commands::message::Judgment {
                exempt: crate::commands::message::exempt_title(subject),
                scope_required,
            },
        );
        if findings.is_empty() {
            continue;
        }
        let listed: Vec<String> = findings
            .iter()
            .map(|finding| format!("{}:{} {}", finding.class, finding.line, finding.detail))
            .collect();
        return Err(refuse(
            Reason::Usage,
            format!(
                "{named} '{subject}' carries {} finding{} the landed commit-msg stage would refuse, and the trunk move fires no hook: {}",
                findings.len(),
                if findings.len() == 1 { "" } else { "s" },
                listed.join("; ")
            ),
        ));
    }
    Ok(())
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

/// Move the trunk ref forward to its remote tip, through the seat that
/// holds it where one does, so no working tree is left behind its own HEAD.
///
/// This runs before the gate, and nothing is integrated yet, so a refresh
/// that completed and then could not be confirmed, or was raced, refuses
/// the integration. Its diagnostic describes the completed refresh: the
/// trunk moved to its remote tip, and no integration was published.
fn fast_forward_trunk(run: &LocalRun<'_>, to: &str, from: &str) -> Result<(), RkError> {
    let PostPublication::Unsettled(unsettled) = trunk_move(run, to, from, "fast-forward")? else {
        return Ok(());
    };
    Err(unsettled.into_refresh_error(run.trunk, from, to))
}

/// Publish the integrated commit: the squash commit, or the branch tip
/// under `rebase`.
///
/// `Some` is a publication the trunk's read-back did not confirm as the
/// planned move; the caller reports the publication and then fails with
/// [`Unsettled::into_error`].
fn publish(run: &LocalRun<'_>, commit: &str, expected: &str) -> Result<Option<Unsettled>, RkError> {
    Ok(match trunk_move(run, commit, expected, "integration")? {
        PostPublication::Confirmed => None,
        PostPublication::Unsettled(unsettled) => Some(unsettled),
    })
}

/// What a trunk move came to once it was made.
///
/// Every refusal of a move is an `Err` of [`trunk_move`] and happens before
/// anything is written. Once the held trunk's fast-forward succeeded, the
/// trunk is published, and what follows is this type and never an error:
/// [`confirm_move`] returns it and cannot fail, so no read-back problem can
/// travel the refusal path and claim the trunk stands where it stood.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PostPublication {
    /// The move was the planned one: a compare-and-swap of an unheld
    /// trunk, or a held fast-forward whose start and end were confirmed.
    Confirmed,
    /// The trunk moved, and the read-back did not confirm the plan.
    Unsettled(Unsettled),
}

/// A completed move of the held trunk that is not confirmed as planned.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Unsettled {
    /// A confirmation source shows the move started elsewhere: a writer
    /// outside rk moved the trunk during the publication.
    Raced(Drift),
    /// No source confirms where the move started, for the reason given.
    Unconfirmed {
        /// The commit the observation found before the gate.
        expected: String,
        /// Why no source confirmed the move.
        reason: String,
    },
}

impl Unsettled {
    /// The report line naming the uncertainty beside the publication.
    fn step(&self, trunk: &str) -> String {
        match self {
            Self::Raced(drift) => drift.step(trunk),
            Self::Unconfirmed { expected, reason } => format!(
                "{trunk}'s move from the observed {} is not confirmed: {reason}",
                integrate::short(expected)
            ),
        }
    }

    /// The diagnostic of a publication that is not confirmed as planned:
    /// the trunk is published, nothing is rolled back, and the operator
    /// reads what moved it.
    fn into_error(self, trunk: &str, published: &str) -> RkError {
        match self {
            Self::Raced(drift) => drift.into_error(trunk, published),
            Self::Unconfirmed { expected, reason } => {
                unconfirmed_error(trunk, &expected, published, &reason)
            }
        }
    }

    /// The diagnostic of a pre-gate refresh that completed and is not
    /// confirmed as planned: the trunk moved to its remote tip, and no
    /// integration was published, so the integration refuses.
    fn into_refresh_error(self, trunk: &str, from: &str, to: &str) -> RkError {
        let message = match &self {
            Self::Raced(drift) => format!(
                "{trunk} was fast-forwarded to its remote tip {}, and its last move went from {} to {} where this run observed {}: a writer outside rk moved it at the same time, so no integration was published",
                integrate::short(to),
                integrate::short(&drift.found_old),
                integrate::short(&drift.found_new),
                integrate::short(from)
            ),
            Self::Unconfirmed { reason, .. } => format!(
                "{trunk} was fast-forwarded to its remote tip {}, but rk could not confirm that the move started at the observed {}: {reason}; no integration was published",
                integrate::short(to),
                integrate::short(from)
            ),
        };
        RkError::refusal(
            Diagnostic::new(Reason::StateDrift, message)
                .action(format!(
                    "read git reflog {trunk} and git log {}..{}, then run the integration again",
                    integrate::short(from),
                    integrate::short(to)
                ))
                .target_state(format!(
                    "{trunk} was refreshed to its remote tip {}, with no rollback; no integration was published",
                    integrate::short(to)
                )),
        )
    }
}

/// The diagnostic of a publication no source confirmed.
fn unconfirmed_error(trunk: &str, expected: &str, published: &str, reason: &str) -> RkError {
    let expected = integrate::short(expected);
    let published = integrate::short(published);
    RkError::check_failed(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "{trunk} now stands at the published {published}, but rk could not confirm that the move started at the observed {expected}: {reason}"
            ),
        )
        .expected(format!("a confirmed move of {trunk} from {expected} to {published}"))
        .action(format!(
            "read git reflog {trunk} and git log {expected}..{published}, and decide whether anything else moved {trunk}; rk moves nothing back"
        ))
        .target_state(format!(
            "{trunk} is published at {published}, with no rollback and no reset; the staged evidence names {published} and the trunk reaches it"
        )),
    )
}

/// A held trunk's move, read back, that did not start at the observed tip.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Drift {
    /// The commit the observation found before the gate.
    expected: String,
    /// The commit the move started at.
    found_old: String,
    /// The commit the move ended at.
    found_new: String,
}

impl Drift {
    /// The report line naming the drift beside the publication.
    fn step(&self, trunk: &str) -> String {
        format!(
            "{trunk}'s last move went from {} to {}, where this run observed {}",
            integrate::short(&self.found_old),
            integrate::short(&self.found_new),
            integrate::short(&self.expected)
        )
    }

    /// The diagnostic of a publication a writer outside rk raced: the
    /// trunk is published, nothing is rolled back, and the operator reads
    /// the commits that writer added.
    fn into_error(self, trunk: &str, published: &str) -> RkError {
        RkError::check_failed(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "{trunk} now stands at the published {}, and its last move went from {} to {}, not from the observed {}: a writer outside rk moved it during the publication",
                    integrate::short(published),
                    integrate::short(&self.found_old),
                    integrate::short(&self.found_new),
                    integrate::short(&self.expected)
                ),
            )
            .expected(format!(
                "a move of {trunk} from the observed {} to the published {}",
                integrate::short(&self.expected),
                integrate::short(published)
            ))
            .action(format!(
                "read the commits between the observed and the found starting commit, git log {}..{}, and decide whether they belong on {trunk}; rk moves nothing back",
                integrate::short(&self.expected),
                integrate::short(&self.found_old)
            ))
            .target_state(format!(
                "{trunk} is published at {}, with no rollback and no reset; the staged evidence names {} and the trunk reaches it",
                integrate::short(published),
                integrate::short(published)
            )),
        )
    }
}

/// One trunk move from `expected` to `to`.
///
/// The worktrees and the trunk are read again immediately before the
/// move: a trunk that moved since the observation refuses with the
/// `moved` refusal, and a trunk that became checked out, or stopped being
/// checked out, since then refuses too, because the form of the move
/// depends on who holds it. Nothing was written in either case.
///
/// Where no worktree holds the trunk, the ref moves by `git update-ref`
/// carrying `expected` as its old value, a compare-and-swap. Where a
/// worktree holds it, Git offers no such swap that also moves the
/// worktree, so the move is that worktree's own fast-forward merge, which
/// keeps its index and working tree with HEAD. A writer outside rk can
/// still move the trunk between the re-observation and the merge. A merge
/// that refuses writes nothing and is an `Err`. A merge that succeeds has
/// published, and [`confirm_move`] then reads where the move started.
fn trunk_move(
    run: &LocalRun<'_>,
    to: &str,
    expected: &str,
    what: &str,
) -> Result<PostPublication, RkError> {
    let observed = run
        .seats
        .iter()
        .find(|seat| seat.branch.as_deref() == Some(run.trunk))
        .map(|seat| seat.path.clone());
    let live_seats = seats(run.target)?;
    let holder = live_seats
        .iter()
        .find(|seat| seat.branch.as_deref() == Some(run.trunk));
    if let Some(changed) = holder_change(
        run.trunk,
        observed.as_deref(),
        holder.map(|seat| seat.path.as_path()),
        what,
    ) {
        return Err(refuse(Reason::StateDrift, changed));
    }
    let reference = format!("refs/heads/{}", run.trunk);
    let live = rev_parse(run.seat, &reference)?;
    if live != expected {
        return Err(moved(run.trunk, expected, &live));
    }
    let Some(holder) = holder else {
        let swapped = git(run.seat, &["update-ref", &reference, to, expected])?;
        if !swapped.status.success() {
            let now =
                rev_parse(run.seat, &reference).unwrap_or_else(|_| "an unreadable tip".to_owned());
            return Err(moved(run.trunk, expected, &now));
        }
        return Ok(PostPublication::Confirmed);
    };
    if is_dirty(&holder.path)? {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "{} has the trunk checked out and carries uncommitted changes, so the {what} would leave it behind its own HEAD",
                holder.path
            ),
        ));
    }
    // The reflog is written whatever the clone says, and the merge reports
    // its own move in English with full object names, so the move can be
    // read back from either.
    let merged = git_in_c_locale(
        &holder.path,
        &[
            "-c",
            "core.logAllRefUpdates=always",
            "-c",
            "core.abbrev=no",
            "merge",
            "--ff-only",
            to,
        ],
    )?;
    if !merged.status.success() {
        return Err(refuse(
            Reason::StateDrift,
            format!(
                "the {what} is not a fast-forward of {}, so nothing was published: {}",
                holder.path,
                last_line(&merged.stderr)
            ),
        ));
    }
    let report = String::from_utf8_lossy(&merged.stdout).into_owned();
    Ok(confirm_move(run, &reference, expected, to, &report))
}

/// Where the held trunk's completed fast-forward started and ended, read
/// back, and what that makes of it. It cannot fail: every problem is an
/// unconfirmed publication.
///
/// The sources, in order: the files backend's raw reflog entry, which
/// records the transition as written; else the newest two entries of
/// `git reflog show`, where two exist; and the merge's own `Updating
/// <old>..<new>` line, with full object names. A reflog source and the
/// merge's line that disagree confirm nothing. Only exact full-name
/// equality confirms: the start equals the observed tip and the end the
/// published object. A source that shows another start, with the planned
/// end, is the race; anything else is unconfirmed.
fn confirm_move(
    run: &LocalRun<'_>,
    reference: &str,
    expected: &str,
    to: &str,
    merge_report: &str,
) -> PostPublication {
    let reflog = reflog_transition(run, reference);
    let reported = parse_updating(merge_report, expected.len());
    judge_move(expected, to, reflog, reported)
}

/// The judgment over the two read-backs; see [`confirm_move`].
fn judge_move(
    expected: &str,
    to: &str,
    reflog: Result<(String, String), String>,
    reported: Result<(String, String), String>,
) -> PostPublication {
    let unconfirmed = |reason: String| {
        PostPublication::Unsettled(Unsettled::Unconfirmed {
            expected: expected.to_owned(),
            reason,
        })
    };
    let transition = match (reflog, reported) {
        (Ok(logged), Ok(said)) if logged != said => {
            return unconfirmed(format!(
                "the reflog shows {}..{} and the fast-forward reported {}..{}",
                integrate::short(&logged.0),
                integrate::short(&logged.1),
                integrate::short(&said.0),
                integrate::short(&said.1)
            ));
        }
        (Ok(logged), _) => logged,
        (Err(_), Ok(said)) => said,
        (Err(logged), Err(said)) => {
            return unconfirmed(format!("{logged}, and {said}"));
        }
    };
    let (old, new) = transition;
    if new != to {
        return unconfirmed(format!(
            "the move read back ends at {}, not at the published {}",
            integrate::short(&new),
            integrate::short(to)
        ));
    }
    if old == expected {
        return PostPublication::Confirmed;
    }
    PostPublication::Unsettled(Unsettled::Raced(Drift {
        expected: expected.to_owned(),
        found_old: old,
        found_new: new,
    }))
}

/// Whether `name` is a full object name of `len` hexadecimal characters.
fn full_name(name: &str, len: usize) -> bool {
    name.len() == len && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The old and new value of the trunk's newest reflog entry, or why they
/// cannot be read.
///
/// The files backend keeps the raw transition in the ref's log, read as
/// written. A backend with no such file answers through the reflog
/// command, whose second entry carries the value the newest one started
/// from; with one entry, as after an expiry, it answers nothing.
fn reflog_transition(run: &LocalRun<'_>, reference: &str) -> Result<(String, String), String> {
    let path = run.git_dir.join("logs").join(reference);
    match std::fs::read_to_string(path.as_std_path()) {
        Ok(text) => {
            let line = text.lines().last().unwrap_or_default();
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next()) {
                (Some(old), Some(new))
                    if full_name(old, new.len()) && full_name(new, old.len()) =>
                {
                    Ok((old.to_owned(), new.to_owned()))
                }
                _ => Err(format!("the reflog file {path} holds no readable entry")),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let listed = git(
                run.seat,
                &["reflog", "show", "--format=%H", "-n", "2", reference],
            )
            .map_err(|error| format!("git reflog did not run: {error}"))?;
            if !listed.status.success() {
                return Err(format!(
                    "git reflog show {reference} failed: {}",
                    last_line(&listed.stderr)
                ));
            }
            let text = String::from_utf8_lossy(&listed.stdout).into_owned();
            let mut lines = text.lines();
            match (lines.next(), lines.next()) {
                (Some(new), Some(old))
                    if full_name(new, old.len()) && full_name(old, new.len()) =>
                {
                    Ok((old.to_owned(), new.to_owned()))
                }
                _ => Err(format!(
                    "the reflog of {reference} holds fewer than two readable entries"
                )),
            }
        }
        Err(error) => Err(format!("the reflog file {path} could not be read: {error}")),
    }
}

/// The `<old>` and `<new>` of the fast-forward's own `Updating
/// <old>..<new>` line, each a full object name of `len` characters, or why
/// the report confirms nothing: no such line, more than one, a malformed
/// one, or an abbreviated name.
fn parse_updating(report: &str, len: usize) -> Result<(String, String), String> {
    let lines: Vec<&str> = report
        .lines()
        .filter_map(|line| line.strip_prefix("Updating "))
        .collect();
    let [line] = lines.as_slice() else {
        return Err(if lines.is_empty() {
            "the fast-forward reported no Updating line".to_owned()
        } else {
            "the fast-forward reported more than one Updating line".to_owned()
        });
    };
    let Some((old, new)) = line.trim().split_once("..") else {
        return Err(format!(
            "the fast-forward's Updating line is malformed: {line}"
        ));
    };
    if !full_name(old, len) || !full_name(new, len) {
        return Err(format!(
            "the fast-forward's Updating line names no full object names: {line}"
        ));
    }
    Ok((old.to_owned(), new.to_owned()))
}

/// Why the trunk's holder differs from the one observed, or `None`.
fn holder_change(
    trunk: &str,
    observed: Option<&Utf8Path>,
    live: Option<&Utf8Path>,
    what: &str,
) -> Option<String> {
    match (observed, live) {
        (None, None) => None,
        (Some(before), Some(now)) if before == now => None,
        (None, Some(now)) => Some(format!(
            "{trunk} was checked out in {now} after this run observed it, so the {what} would leave that worktree behind its own HEAD; nothing was published"
        )),
        (Some(before), None) => Some(format!(
            "{trunk} stopped being checked out in {before} after this run observed it, so the {what} no longer has the worktree it was planned for; nothing was published"
        )),
        (Some(before), Some(now)) => Some(format!(
            "{trunk} moved from being checked out in {before} to {now} after this run observed it; nothing was published"
        )),
    }
}

/// The refusal for a trunk that moved between the observation and the write.
///
/// This one refusal comes after the evidence was staged, so it says what
/// is actually on disk rather than repeating the general claim: the trunk
/// stands where it stood, and the staged entry names a commit the trunk
/// does not reach, which both prune verbs ignore.
fn moved(trunk: &str, expected: &str, now: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            integrate::refuse_moved_trunk(expected, now).unwrap_or_else(|| {
                format!(
                    "{trunk} could not be moved and stands at {}",
                    integrate::short(now)
                )
            }),
        )
        .target_state(format!(
            "{trunk} stands where it stood; the staged evidence names a commit it does not reach, which every prune ignores"
        )),
    )
}

/// Warn where the message states release intent and the squash changes no
/// file the crate ships, under a record whose release bot is release-plz.
///
/// release-plz attributes a commit to a package only where the commit
/// changes a file `cargo package --list` prints, so such a commit reaches
/// neither the changelog nor a release. The checks run cheapest first, so a
/// message stating no intent or a target another driver releases spawns no
/// cargo. Anything that cannot answer leaves the integration silent: a
/// warning built on an unproved listing teaches the operator to ignore it.
/// The listing runs `--locked --offline`, so it writes nothing into the
/// tree the gate judges.
///
/// SATISFIES git:a-local-integration-warns-of-an-uncounted-release
fn warn_if_uncounted(
    out: Output,
    run: &LocalRun<'_>,
    message: &str,
    range: &[&str],
    previewed: bool,
) {
    let Some(intent) = integrate::release_intent(message) else {
        return;
    };
    let rust_release = crate::landing::manifest::load(run.target)
        .ok()
        .flatten()
        .is_some_and(|record| {
            let release = &record.profile.release;
            release.mode == crate::profile::ReleaseMode::Automatic
                && release.driver.as_deref() == Some("rust")
        });
    if !rust_release {
        return;
    }
    let mut diff = vec!["diff", "--name-only", "--no-renames", "-z"];
    diff.extend_from_slice(range);
    let Ok(changed) = git(run.seat, &diff) else {
        return;
    };
    if !changed.status.success() {
        return;
    }
    let changed = String::from_utf8_lossy(&changed.stdout).into_owned();
    let changed: Vec<&str> = changed
        .split('\0')
        .filter(|path| !path.is_empty())
        .collect();
    let root_manifest = run.seat.as_std_path().join("Cargo.toml");
    let probe = crate::cargo_package::probe(&root_manifest, &["--locked", "--offline"], |args| {
        cargo_in(run.seat, args)
    });
    let Ok(crate::cargo_package::Probe::Listed(listing)) = probe else {
        return;
    };
    if !listing.touched_by(changed.iter().copied()) {
        out.warn(integrate::uncounted_release(intent, previewed));
    }
}

/// One cargo call in the seat, for the listing the warning reads.
fn cargo_in(seat: &Utf8Path, args: &[&str]) -> Result<crate::cargo_package::Answer, ()> {
    let mut command = std::process::Command::new("cargo");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .current_dir(seat.as_std_path())
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map(|output| crate::cargo_package::Answer {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
        .map_err(|_| ())
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
        merge_form: run.merge_form.as_str(),
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

/// The shape a trunk commit message states where the record names no
/// release, and a scope is preferred rather than required.
const SHAPE_SCOPE_PREFERRED: &str =
    "a Conventional Commit: <type>(<scope>): <description>, or <type>: <description>";

/// The Conventional Commit types the branch grammar admits, which is the
/// same set the landed `rk-branch-name` hook tests.
const TYPES: [&str; 11] = [
    "build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test",
];

/// Refuse a trunk commit message the landed guards would refuse.
///
/// `at` is the seat, never the trunk checkout: whether a path is ignored
/// is answered by the ignore rules standing where the implementation was
/// written, and a branch that adds one is exactly the case the landed
/// `commit-msg` stage would have judged there.
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
fn refuse_message(target: &Utf8Path, text: &str, scope_required: bool) -> Result<(), RkError> {
    let subject = text.lines().next().unwrap_or("").trim();
    if subject.is_empty() {
        return Err(refuse(
            Reason::Usage,
            "a local integration writes the trunk's commit message, so --message is required",
        ));
    }
    if let Some(reason) = misshapen_subject(subject, scope_required) {
        return Err(refuse(Reason::Usage, reason));
    }
    let (findings, _) = crate::commands::message::judge(
        text,
        crate::cli::message::MessageKind::Commit,
        target,
        subject,
        crate::commands::message::Judgment {
            exempt: crate::commands::message::exempt_title(subject),
            scope_required,
        },
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

/// Why a subject is not the Conventional Commit the record asks for, or
/// `None`: a scoped one wherever `scope_required`, and one whose scope may
/// be absent where the record names no release.
#[must_use]
fn misshapen_subject(subject: &str, scope_required: bool) -> Option<String> {
    let shape = if scope_required {
        SHAPE
    } else {
        SHAPE_SCOPE_PREFERRED
    };
    let Some((head, description)) = subject.split_once(": ") else {
        return Some(format!("'{subject}' is not {shape}"));
    };
    if description.trim().is_empty() {
        return Some(format!(
            "'{subject}' is not {shape}: it states no description"
        ));
    }
    // A breaking `!` sits after the scope, so it comes off the head
    // before the scope is read out of it.
    let head = head.strip_suffix('!').unwrap_or(head);
    let (kind, scope) = match head.split_once('(') {
        None if scope_required => {
            return Some(format!("'{subject}' is not {shape}: it names no scope"));
        }
        None => (head, None),
        Some((kind, scope)) => {
            let Some(scope) = scope.strip_suffix(')') else {
                return Some(format!("'{subject}' is not {shape}: its scope is unclosed"));
            };
            (kind, Some(scope))
        }
    };
    if !TYPES.contains(&kind) {
        return Some(format!(
            "'{kind}' is not a Conventional Commit type; the types are: {}",
            TYPES.join(", ")
        ));
    }
    if let Some(scope) = scope
        && !crate::projection::scope_is_shaped(scope)
    {
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

/// Run one git command against a directory under the C locale, so its
/// human report reads in English; a spawn failure refuses.
fn git_in_c_locale(at: &Utf8Path, args: &[&str]) -> Result<std::process::Output, RkError> {
    let mut command = std::process::Command::new(crate::probes::git_bin());
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .env("LC_ALL", "C")
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

    use super::{
        BranchCommit, misshapen_subject, refuse_commit_messages, refuse_message, request_commands,
    };
    use crate::landing::MergeForm;

    /// SATISFIES git:the-merge-form-is-a-landing-parameter
    #[test]
    fn merge_form_rebase_asks_gitlab_for_no_squash() {
        let gitlab = "git@gitlab.com:acme/widget.git";
        let squash = request_commands(gitlab, "master", "feat/x", MergeForm::Squash, true);
        assert_eq!(
            squash[0],
            "glab mr create --source-branch feat/x --target-branch master --squash-before-merge"
        );
        assert!(squash[1].contains("the title is the trunk's commit message"));
        let rebase = request_commands(gitlab, "master", "feat/x", MergeForm::Rebase, true);
        assert_eq!(
            rebase[0],
            "glab mr create --source-branch feat/x --target-branch master"
        );
        assert!(
            !rebase.iter().any(|line| line.contains("squash")),
            "{rebase:?}"
        );
        assert!(
            rebase[1].contains("every commit of the branch reaches the trunk with its own message"),
            "{rebase:?}"
        );
        assert!(!rebase[1].contains("title"), "{rebase:?}");
    }

    #[test]
    fn merge_form_keeps_the_github_request_command() {
        let github = "git@github.com:acme/widget.git";
        for form in [MergeForm::Squash, MergeForm::Rebase] {
            assert_eq!(
                request_commands(github, "master", "feat/x", form, true)[0],
                "gh pr create --base master --head feat/x"
            );
        }
    }

    #[test]
    fn a_trunk_subject_is_held_to_the_landed_convention() {
        assert_eq!(
            misshapen_subject("feat(integrate): land the verb", true),
            None
        );
        assert_eq!(misshapen_subject("feat(a/b)!: break it", true), None);
        assert!(
            misshapen_subject("land the verb", true)
                .expect("an unscoped subject refuses")
                .contains("Conventional Commit")
        );
        assert!(
            misshapen_subject("feat: land the verb", true)
                .expect("a missing scope refuses")
                .contains("names no scope")
        );
        assert!(
            misshapen_subject("feat(integrate):   ", true)
                .expect("an empty description refuses")
                .contains("no description")
        );
        assert!(
            misshapen_subject("wat(integrate): land it", true)
                .expect("an unknown type refuses")
                .contains("not a Conventional Commit type")
        );
        assert!(
            misshapen_subject("feat(Integrate): land it", true)
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
        refuse_message(
            target,
            "feat(integrate): land the verb\n\nThe context.\n",
            true,
        )
        .expect("a clean message passes");
        let error = refuse_message(
            target,
            "feat(integrate): land the verb\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n",
            true,
        )
        .expect_err("agent attribution refuses")
        .to_string();
        assert!(error.contains("attribution"), "{error}");
        assert!(error.contains("commit-tree fires no hook"), "{error}");
        let error = refuse_message(
            target,
            "feat(integrate): land the verb\n\nSee .draft/plan.md for the rest.\n",
            true,
        )
        .expect_err("an internal path refuses")
        .to_string();
        assert!(error.contains("internal-path"), "{error}");
        let error = refuse_message(target, "", true)
            .expect_err("an empty message refuses")
            .to_string();
        assert!(error.contains("--message is required"), "{error}");
    }

    /// Under `rebase` every commit of the branch becomes a trunk commit,
    /// so each message takes the whole landed commit-msg judgment and the
    /// refusal names the commit that failed it.
    #[test]
    fn rebase_integrate_judges_every_commit_message() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let target = Utf8Path::from_path(dir.path()).expect("utf-8");
        let commit = |id: &str, message: &str| BranchCommit {
            id: id.to_owned(),
            message: message.to_owned(),
        };
        refuse_commit_messages(
            target,
            &[
                commit("1111111aaaa", "feat(greeting): add it\n"),
                commit("2222222bbbb", "fix(greeting): mend it\n\nThe context.\n"),
            ],
            true,
        )
        .expect("clean messages pass");
        let error = refuse_commit_messages(
            target,
            &[
                commit("1111111aaaa", "feat(greeting): add it\n"),
                commit(
                    "2222222bbbb",
                    "fix(greeting): mend it\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n",
                ),
            ],
            true,
        )
        .expect_err("agent attribution refuses")
        .to_string();
        assert!(error.contains("commit 2222222"), "{error}");
        assert!(error.contains("attribution"), "{error}");
        let error = refuse_commit_messages(target, &[commit("3333333cccc", "mend it\n")], true)
            .expect_err("an unscoped subject refuses")
            .to_string();
        assert!(error.contains("commit 3333333"), "{error}");
        assert!(error.contains("Conventional Commit"), "{error}");
        let error = refuse_commit_messages(target, &[commit("4444444dddd", "\n")], true)
            .expect_err("an empty message refuses")
            .to_string();
        assert!(error.contains("carries no message"), "{error}");
    }

    /// Where the record names no release, a scope is preferred and not
    /// required: a header with none passes, while every other part of the
    /// shape, the type and a present scope's shape included, still holds.
    ///
    /// SATISFIES project-profile:a-release-less-landing-carries-no-release-text
    #[test]
    fn workflow_only_a_trunk_subject_may_name_no_scope() {
        assert_eq!(misshapen_subject("fix: drop the stale path", false), None);
        assert_eq!(misshapen_subject("feat!: break it", false), None);
        assert_eq!(misshapen_subject("feat(a/b): add it", false), None);
        assert!(
            misshapen_subject("fix: drop the stale path", true)
                .expect("a release requires the scope")
                .contains("names no scope")
        );
        assert!(
            misshapen_subject("wat: drop it", false)
                .expect("an unknown type refuses")
                .contains("not a Conventional Commit type")
        );
        assert!(
            misshapen_subject("drop the stale path", false)
                .expect("a subject with no type refuses")
                .contains("<type>: <description>")
        );
        assert!(
            misshapen_subject("fix(Bad): mend it", false)
                .expect("a misshapen scope refuses")
                .contains("outside")
        );
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let target = Utf8Path::from_path(dir.path()).expect("utf-8");
        refuse_message(target, "fix: drop the stale path\n", false)
            .expect("a release-less trunk message names no scope");
        refuse_commit_messages(
            target,
            &[BranchCommit {
                id: "5555555eeee".to_owned(),
                message: "fix: drop the stale path\n".to_owned(),
            }],
            false,
        )
        .expect("a release-less branch commit names no scope");
    }

    /// Under `release.mode = "none"` the forge request guidance asks for a
    /// Conventional Commit whose scope is preferred, never a scoped one, in
    /// either merge form.
    ///
    /// SATISFIES project-profile:a-release-less-landing-carries-no-release-text
    #[test]
    fn workflow_only_request_guidance_prefers_the_scope() {
        for remote in [
            "git@gitlab.com:acme/widget.git",
            "git@github.com:acme/widget.git",
        ] {
            for form in [MergeForm::Squash, MergeForm::Rebase] {
                let lines = request_commands(remote, "master", "feat/x", form, false);
                assert!(!lines[1].contains("scoped"), "{lines:?}");
                assert!(
                    lines[1].contains("scope is preferred and not required"),
                    "{lines:?}"
                );
                let releasing = request_commands(remote, "master", "feat/x", form, true);
                assert!(
                    releasing[1].contains("scoped Conventional Commit"),
                    "{releasing:?}"
                );
                assert_eq!(lines[0], releasing[0], "the command itself does not change");
            }
        }
    }

    /// The publication a writer outside rk raced reads as a publication:
    /// a check failure with the reason `state-drift`, naming the published
    /// object, the observed tip, and the move the reflog shows, telling the
    /// operator what to read, and never claiming the trunk stands where it
    /// stood.
    ///
    /// SATISFIES git:a-local-integration-is-a-transaction
    #[test]
    fn integrate_reports_a_raced_publication_as_published() {
        let drift = super::Drift {
            expected: "a".repeat(40),
            found_old: "b".repeat(40),
            found_new: "c".repeat(40),
        };
        assert!(drift.step("master").contains("bbbbbbb to ccccccc"));
        let error = drift.into_error("master", &"c".repeat(40));
        assert_eq!(error.exit_code(), 1);
        let diagnostic = error.diagnostic();
        assert_eq!(diagnostic.reason, crate::diagnostic::Reason::StateDrift);
        for named in [
            "published ccccccc",
            "bbbbbbb",
            "observed aaaaaaa",
            "outside rk",
        ] {
            assert!(
                diagnostic.message.contains(named),
                "{named}: {}",
                diagnostic.message
            );
        }
        assert!(
            !diagnostic.message.contains("nothing"),
            "{}",
            diagnostic.message
        );
        let state = diagnostic.target_state.unwrap_or_default();
        assert!(state.contains("published at ccccccc"), "{state}");
        assert!(state.contains("no rollback"), "{state}");
        assert!(!state.contains("where it stood"), "{state}");
        let action = diagnostic.action.unwrap_or_default();
        assert!(action.contains("git log aaaaaaa..bbbbbbb"), "{action}");
    }

    /// The holder observed before the gate must be the holder at the
    /// publication: a trunk checked out since, or no longer, refuses.
    ///
    /// SATISFIES git:a-local-integration-is-a-transaction
    #[test]
    fn integrate_refuses_a_changed_trunk_holder() {
        let main = Utf8Path::new("/repo");
        let other = Utf8Path::new("/repo@trunk");
        assert_eq!(
            super::holder_change("master", None, None, "integration"),
            None
        );
        assert_eq!(
            super::holder_change("master", Some(main), Some(main), "integration"),
            None
        );
        for (observed, live) in [
            (None, Some(other)),
            (Some(main), None),
            (Some(main), Some(other)),
        ] {
            let reason = super::holder_change("master", observed, live, "integration")
                .expect("a changed holder refuses");
            assert!(reason.contains("checked out"), "{reason}");
            assert!(reason.contains("nothing was published"), "{reason}");
        }
    }

    /// A publication no source confirmed reads as a publication: a check
    /// failure with the reason `state-drift`, naming the published object
    /// and the observed tip, telling the operator what to read, and never
    /// claiming the trunk stands where it stood. The pre-gate refresh of a
    /// held trunk, completed and unconfirmed or raced, says the trunk was
    /// refreshed and that no integration was published.
    ///
    /// SATISFIES git:a-local-integration-is-a-transaction
    #[test]
    fn integrate_reports_an_unconfirmed_move_as_published() {
        let expected = "a".repeat(40);
        let published = "c".repeat(40);
        let unsettled = super::Unsettled::Unconfirmed {
            expected: expected.clone(),
            reason: "the reflog holds one entry".to_owned(),
        };
        assert!(unsettled.step("master").contains("is not confirmed"));
        let error = unsettled.clone().into_error("master", &published);
        assert_eq!(error.exit_code(), 1);
        let diagnostic = error.diagnostic();
        assert_eq!(diagnostic.reason, crate::diagnostic::Reason::StateDrift);
        assert_eq!(
            diagnostic.message,
            "master now stands at the published ccccccc, but rk could not confirm that the move started at the observed aaaaaaa: the reflog holds one entry"
        );
        assert_eq!(
            diagnostic.expected.as_deref(),
            Some("a confirmed move of master from aaaaaaa to ccccccc")
        );
        assert_eq!(
            diagnostic.action.as_deref(),
            Some(
                "read git reflog master and git log aaaaaaa..ccccccc, and decide whether anything else moved master; rk moves nothing back"
            )
        );
        assert_eq!(
            diagnostic.target_state.as_deref(),
            Some(
                "master is published at ccccccc, with no rollback and no reset; the staged evidence names ccccccc and the trunk reaches it"
            )
        );

        let raced = super::Unsettled::Raced(super::Drift {
            expected: expected.clone(),
            found_old: "b".repeat(40),
            found_new: published.clone(),
        });
        for refresh in [unsettled, raced] {
            let diagnostic = refresh
                .into_refresh_error("master", &expected, &published)
                .diagnostic();
            assert!(
                diagnostic
                    .message
                    .contains("was fast-forwarded to its remote tip ccccccc")
            );
            assert!(diagnostic.message.contains("no integration was published"));
            let state = diagnostic.target_state.unwrap_or_default();
            assert!(
                state.contains("refreshed to its remote tip ccccccc"),
                "{state}"
            );
            for unchanged in ["unchanged", "where it stood", "now stands at the published"] {
                assert!(!diagnostic.message.contains(unchanged), "{unchanged}");
                assert!(!state.contains(unchanged), "{unchanged}");
            }
        }
    }

    /// The fast-forward's own report confirms only through one well-formed
    /// `Updating <old>..<new>` line of full object names.
    ///
    /// SATISFIES git:a-local-integration-is-a-transaction
    #[test]
    fn integrate_parses_only_a_full_updating_line() {
        let old = "a".repeat(40);
        let new = "b".repeat(40);
        assert_eq!(
            super::parse_updating(&format!("Updating {old}..{new}\nFast-forward\n"), 40),
            Ok((old.clone(), new.clone()))
        );
        for report in [
            "Fast-forward\n".to_owned(),
            format!("Updating {}..{}\n", &old[..7], &new[..7]),
            format!("Updating {old}..{}\n", &new[..12]),
            format!("Updating {old} {new}\n"),
            format!("Updating {old}..{new}\nUpdating {new}..{old}\n"),
            format!("Updating {}..{new}\n", "g".repeat(40)),
            format!("Updating {old}..{new}\n").replace("Updating", "Mise à jour"),
        ] {
            assert!(super::parse_updating(&report, 40).is_err(), "{report}");
        }
    }

    /// The read-backs confirm only an exact full-name match; a start
    /// elsewhere is the race; an end elsewhere, two sources that disagree,
    /// and no source at all are unconfirmed.
    ///
    /// SATISFIES git:a-local-integration-is-a-transaction
    #[test]
    fn integrate_judges_the_read_backs_of_a_held_move() {
        use super::{PostPublication, Unsettled, judge_move};
        let expected = "a".repeat(40);
        let other = "b".repeat(40);
        let to = "c".repeat(40);
        let pair = |old: &str, new: &str| Ok((old.to_owned(), new.to_owned()));
        let none = || Err::<(String, String), String>("unavailable".to_owned());
        assert_eq!(
            judge_move(&expected, &to, pair(&expected, &to), none()),
            PostPublication::Confirmed
        );
        assert_eq!(
            judge_move(&expected, &to, none(), pair(&expected, &to)),
            PostPublication::Confirmed
        );
        assert_eq!(
            judge_move(&expected, &to, pair(&expected, &to), pair(&expected, &to)),
            PostPublication::Confirmed
        );
        assert!(matches!(
            judge_move(&expected, &to, pair(&other, &to), none()),
            PostPublication::Unsettled(Unsettled::Raced(_))
        ));
        for judged in [
            judge_move(&expected, &to, pair(&expected, &to), pair(&other, &to)),
            judge_move(&expected, &to, none(), pair(&expected, &other)),
            judge_move(&expected, &to, none(), none()),
        ] {
            assert!(
                matches!(
                    judged,
                    PostPublication::Unsettled(Unsettled::Unconfirmed { .. })
                ),
                "{judged:?}"
            );
        }
    }
}
