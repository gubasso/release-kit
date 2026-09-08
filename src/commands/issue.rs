//! `rk issue start`: one command from an issue to the branch the forge
//! named, seated the way the recorded mode says.
//!
//! Not a worktree verb, because the worktree verbs are mode-free by
//! design and starting from an issue is not: branches mode has no
//! worktree to add. The order of the run is what makes the refusals
//! cheap — the local reads and the reference check come first, the
//! forge-CLI gate next, and the network last — so every failure but a
//! forge outage costs one local process and leaves the clone untouched.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::cli::issue::{IssueAction, IssueArgs};
use crate::detect::{self, Forge};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::issue::{self, Resolved};
use crate::landing::manifest::{self, Workflow};
use crate::output::Output;
use crate::probes;
use crate::setup::context::{TRUNK_BRANCH, resolve_cli};

/// One `rk issue start` report.
#[derive(Debug, Serialize)]
struct StartReport {
    /// The shape version of this JSON.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The forge acted on.
    forge: &'static str,
    /// The project path.
    repo: String,
    /// The issue, as the forge numbers it.
    issue: u64,
    /// The issue's title.
    title: String,
    /// The branch, where one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    /// Where the name came from: `already`, `forge`, or `pending`.
    origin: &'static str,
    /// The recorded workflow mode this run seated by.
    workflow: &'static str,
    /// The worktree path, under worktree mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// The branch checked out in place, under branches mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    checkout: Option<String>,
    /// Every other branch the forge links to this issue.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    others: Vec<String>,
    /// A state the operator must see rather than one rk decided quietly.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// What to run next.
    next: Vec<String>,
}

/// Dispatch the issue surface.
///
/// # Errors
///
/// Refuses a target that is not a repository, a reference that names
/// another project, an undetected forge, a forge CLI below its floor, a
/// GitLab template rendering a name the landed grammar refuses, and every
/// seating refusal `rk worktree add` already carries.
pub fn run(args: &IssueArgs) -> Result<(), RkError> {
    match &args.action {
        IssueAction::Start {
            issue,
            target,
            forge,
            repo,
            workflow,
            base,
            apply,
            json,
        } => start(
            target,
            issue,
            &Overrides {
                forge: forge.as_deref(),
                repo: repo.as_deref(),
                workflow: workflow.as_deref(),
                base: base.as_deref(),
            },
            *apply,
            Output::new(*json),
        ),
    }
}

/// What the operator overrode, where detection or the record would
/// otherwise decide.
struct Overrides<'a> {
    /// The forge, over the detected one.
    forge: Option<&'a str>,
    /// The project path, over the detected one.
    repo: Option<&'a str>,
    /// The workflow mode, over the recorded one.
    workflow: Option<&'a str>,
    /// The commit-ish a new branch starts from.
    base: Option<&'a str>,
}

/// Everything the local reads settled, before the first forge call.
struct Ground {
    /// The forge to act on.
    forge: Forge,
    /// The project path.
    repo: String,
    /// The mode the seat follows.
    workflow: Workflow,
    /// Where the mode came from, for the report.
    workflow_source: &'static str,
}

/// Refuse a coordinate that disagrees with one the clone already knows.
///
/// A coordinate the clone does not know is absent rather than
/// contradicted, which is what leaves an override its real job.
fn contradicts(what: &str, chosen: Option<&str>, known: Option<&str>) -> Result<(), RkError> {
    let (Some(chosen), Some(known)) = (chosen, known) else {
        return Ok(());
    };
    if chosen == known {
        return Ok(());
    }
    Err(RkError::Usage(format!(
        "the {what} to act on is {chosen} and this clone's is {known}; the branch would be minted on one project and seated in another"
    )))
}

/// The mode the seat follows, and what decided it.
///
/// The mode is a landing parameter, changed through the landing verbs
/// alone, so a runtime flag states it rather than sets it. Where it
/// disagrees with the record, one clone would work in a mode the
/// committed project policy does not carry.
fn mode_of(target: &Utf8Path, named: Option<&str>) -> Result<(Workflow, &'static str), RkError> {
    let recorded = manifest::load(target)?.map(|held| held.parameters.workflow);
    match (named, recorded) {
        (Some(raw), Some(held)) => {
            if Workflow::parse(raw)? != held {
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::StateDrift,
                        format!(
                            "--workflow {raw} disagrees with the landing record, which states {}",
                            held.as_str()
                        ),
                    )
                    .expected("a flag that states the recorded mode, or no flag at all")
                    .action("rk upgrade --workflow <mode> --apply changes the recorded mode")
                    .target_state("unchanged"),
                ));
            }
            Ok((held, "the landing record, restated by --workflow"))
        }
        (Some(raw), None) => Ok((Workflow::parse(raw)?, "the --workflow flag")),
        (None, Some(held)) => Ok((held, "the landing record")),
        // A target with no record is treated as the convention's own
        // mode rather than as branches: the record's serde default exists
        // for records written before the parameter, not for targets that
        // never landed.
        (None, None) => Ok((Workflow::Worktree, "the default, with no landing record")),
    }
}

/// Refuse a forge on a host this verb's calls would not reach.
///
/// Every GitHub call here goes to the CLI's default host. An enterprise
/// remote reaches this verb only through `--forge`, and acting on the
/// wrong host is worse than saying plainly that this verb carries one.
fn reachable(forge: Forge, host: Option<&str>) -> Result<(), RkError> {
    let Some(host) = host else { return Ok(()) };
    if forge != Forge::Github || host.eq_ignore_ascii_case("github.com") {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::ForgeUnsupported,
            format!("this clone's origin is {host}, and rk issue start reaches github.com alone"),
        )
        .expected("a github.com remote, or a GitLab project")
        .action(
            "start the branch with gh issue develop --repo <host>/<owner>/<name>, then rk worktree add it",
        )
        .target_state("unchanged"),
    ))
}

/// Read the target, the reference, and the recorded mode. Nothing here
/// touches the network, so every refusal below costs one local read.
fn ground(
    target: &Utf8Path,
    reference: &issue::Reference,
    overrides: &Overrides<'_>,
) -> Result<Ground, RkError> {
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory"),
            )
            .expected("an existing repository to act on"),
        ));
    }
    let named = overrides
        .forge
        .map(|name| {
            Forge::parse(name).ok_or_else(|| {
                RkError::Usage(format!(
                    "unknown forge '{name}'; the forges are: github, gitlab"
                ))
            })
        })
        .transpose()?;
    let detected = detect::detect(target.as_std_path());
    // The reference is held to the clone before anything else: an agent
    // pasting a URL while sitting in another checkout would otherwise
    // mint on one project and seat in another.
    issue::agrees(reference, &detected).map_err(RkError::Usage)?;
    let Some(forge) = named.or(detected.forge) else {
        let diagnostic = detected
            .host
            .as_ref()
            .map_or_else(
                || {
                    Diagnostic::new(
                        Reason::ForgeUndetected,
                        "no forge detected: the target has no origin remote",
                    )
                },
                |host| {
                    Diagnostic::new(
                        Reason::ForgeUndetected,
                        format!("no forge detected: the host {host} is not recognized"),
                    )
                },
            )
            .expected("a github.com or gitlab remote, or an override")
            .action("pass --forge <github|gitlab>, and --repo <path> if the remote is absent");
        return Err(if detected.host.is_some() {
            RkError::refusal(diagnostic)
        } else {
            RkError::missing(diagnostic)
        });
    };
    // The reference names a host too, and it is authoritative where the
    // clone has none: an issue URL for another host must not be acted on
    // at the CLI's default one.
    reachable(
        forge,
        detected.host.as_deref().or(reference.host.as_deref()),
    )?;
    // An override supplies a coordinate detection could not, and never
    // replaces one it could: minting on the project the operator named
    // and seating the branch in the clone they are standing in is the
    // same cross-project mistake the reference check refuses.
    contradicts(
        "forge",
        named.map(Forge::as_str),
        detected.forge.map(Forge::as_str),
    )?;
    let Some(repo) = overrides
        .repo
        .map(str::to_owned)
        .or_else(|| reference.repo.clone())
        .or_else(|| detected.repo.clone())
    else {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::ForgeUndetected,
                "no repository detected: the target has no origin remote",
            )
            .expected("an origin remote naming the project")
            .action("pass --repo <owner/name>"),
        ));
    };
    contradicts("repository", Some(repo.as_str()), detected.repo.as_deref())?;
    contradicts("repository", Some(repo.as_str()), reference.repo.as_deref())?;
    let (workflow, workflow_source) = mode_of(target, overrides.workflow)?;
    Ok(Ground {
        forge,
        repo,
        workflow,
        workflow_source,
    })
}

/// Mint the issue's branch at the forge and seat it.
fn start(
    target: &Utf8Path,
    reference: &str,
    overrides: &Overrides<'_>,
    apply: bool,
    out: Output,
) -> Result<(), RkError> {
    let reference = issue::parse_reference(reference).map_err(RkError::Usage)?;
    let ground = ground(target, &reference, overrides)?;
    // The target is a repository before anything reaches the network.
    // Without this, a mint could succeed and the run then fail on a
    // local prerequisite, leaving a remote branch no report accounts for.
    let main = crate::commands::worktree::main_checkout(target)?;
    // The gate before the first forge call, so a stale CLI costs one
    // local process rather than a half-finished remote change.
    probes::require_forge_cli(ground.forge)?;
    let cli = resolve_cli(ground.forge)?;
    // Where the forge lets rk know the name before it writes, every
    // local refusal the seat carries runs first.
    let seatable = |branch: &str| -> Result<(), RkError> {
        match ground.workflow {
            Workflow::Worktree => {
                crate::commands::worktree::plan_seat(target, branch, overrides.base, false)
                    .map(|_| ())
            }
            Workflow::Branches => branch_seatable(&main, branch),
        }
    };
    let resolved = issue::resolve(
        &cli,
        target.as_std_path(),
        &issue::Ask {
            forge: ground.forge,
            repo: &ground.repo,
            reference: &reference,
            base: overrides.base,
            apply,
            seatable: &seatable,
        },
    )?;
    match ground.workflow {
        Workflow::Worktree => seat_worktree(target, &ground, &resolved, overrides.base, apply, out),
        Workflow::Branches => seat_branch(&main, &ground, &resolved, apply, out),
    }
}

/// Worktree mode: the derived sibling path, through the same planning and
/// the same refusals `rk worktree add` uses.
fn seat_worktree(
    target: &Utf8Path,
    ground: &Ground,
    resolved: &Resolved,
    base: Option<&str>,
    apply: bool,
    out: Output,
) -> Result<(), RkError> {
    let Some(branch) = resolved.branch.as_deref() else {
        return report(out, ground, resolved, None, None, apply);
    };
    let seat = crate::commands::worktree::plan_seat(target, branch, base, apply)?;
    let mut note = None;
    let path = match seat {
        crate::commands::worktree::Seat::Satisfied { path } => path,
        crate::commands::worktree::Seat::Fresh {
            path,
            source,
            detail,
        } => {
            // An apply refreshes first, and `detail` is set only where
            // that refresh failed. A remote-tracking ref left over from
            // an older fetch is not the tip the forge holds now, so it
            // is not something to seat from and call success.
            if apply {
                if let Some(why) = detail {
                    return Err(stale_refs(branch, resolved, &why));
                }
            }
            // The forge holds this branch, so the seat comes from its
            // real tip — an adopted local branch, or the remote-tracking
            // ref. Anything else would build a same-named branch sharing
            // none of the forge's history. A preview says so, because it
            // does not fetch and the refs it reads may simply be stale;
            // an apply fetched first, so there it is a refusal.
            if !matches!(source.kind, "adopted" | "remote") {
                if apply {
                    return Err(unreachable_tip(branch, resolved));
                }
                note = Some(format!(
                    "origin/{branch} is not in this clone yet; the apply fetches first, and refuses rather than seat a branch from the trunk"
                ));
            }
            if apply {
                crate::commands::worktree::create_seat(target, &source)?;
            }
            path
        }
    };
    report_with(out, ground, resolved, Some(path), None, apply, note)
}

/// The branch the forge holds is not reachable locally, so no seat is
/// made from something else that happens to share its name.
fn unreachable_tip(branch: &str, resolved: &Resolved) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!("the forge carries {branch} and this clone cannot reach its tip"),
        )
        .expected(format!(
            "origin/{branch} present, or {branch} already local"
        ))
        .action("git fetch origin, then rerun")
        .target_state(format!(
            "unchanged; issue #{} keeps its branch at the forge",
            resolved.number
        )),
    )
}

/// What the branches-mode checkout needs, checked before the forge is
/// written to.
///
/// `git switch` refuses a branch another worktree has checked out, and it
/// refuses to move a working tree whose changes it would lose. Both are
/// knowable here, and a branch created at the forge and then refused
/// locally is a remote change no report accounts for.
///
/// The second check is deliberately stricter than git, which permits a
/// switch whose changes do not conflict. Whether they conflict is not
/// knowable without doing the switch, and this mode seats a branch the
/// operator is about to start work on: a clean checkout is what that
/// asks for, and the remedy is one command.
fn branch_seatable(main: &Utf8Path, branch: &str) -> Result<(), RkError> {
    if let Some(seat) = crate::commands::worktree::seat_of(main, branch)? {
        if seat != main {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!(
                        "branch {branch} is checked out at {seat}, and one branch has one seat"
                    ),
                )
                .expected("the branch free, or already in the main checkout")
                .action(format!("git -C {seat} switch {TRUNK_BRANCH}, then rerun"))
                .target_state("unchanged"),
            ));
        }
        // The branch is already seated here, so nothing is checked out
        // over anything: the switch is a no-op and carries no risk.
        return Ok(());
    }
    let held = crate::commands::worktree::git(main, &["status", "--porcelain"])?;
    // A probe that cannot answer counts as dirty: this runs before a
    // remote write, so the closed direction is the safe one.
    if !held.status.success() || !held.stdout.is_empty() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!("{main} carries uncommitted work, and this mode checks {branch} out there"),
            )
            .expected("a clean main checkout to seat the branch in")
            .action("commit or stash the work, then rerun")
            .target_state("unchanged"),
        ));
    }
    Ok(())
}

/// The refresh failed, so no local ref is proof of what the forge holds.
fn stale_refs(branch: &str, resolved: &Resolved, why: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "this clone could not refresh from the forge, so its {branch} may be stale: {why}"
            ),
        )
        .expected("a fetch that answered, so the seat starts from the tip the forge holds")
        .action("git fetch origin, then rerun")
        .target_state(format!(
            "unchanged; issue #{} keeps its branch at the forge",
            resolved.number
        )),
    )
}

/// Branches mode: the branch checked out in the main checkout.
///
/// The main checkout is where this mode works branches, so the switch
/// runs there whichever of the repository's worktrees `--target` named.
///
/// Both forges create the branch on the remote, so an apply always sees
/// the same case — a remote tip with no local branch — unless a previous
/// run already made one.
fn seat_branch(
    main: &Utf8Path,
    ground: &Ground,
    resolved: &Resolved,
    apply: bool,
    out: Output,
) -> Result<(), RkError> {
    let Some(branch) = resolved.branch.as_deref() else {
        return report(out, ground, resolved, None, None, apply);
    };
    if !apply {
        return report(out, ground, resolved, None, Some(branch.to_owned()), false);
    }
    // Through the shared runner, which scrubs the hook variables: a run
    // from inside a git hook must act on the named checkout and never on
    // the hook's own repository.
    let git = |args: &[&str]| crate::commands::worktree::git(main, args);
    let fetched = git(&["fetch", "origin"])?;
    if !fetched.status.success() {
        return Err(stale_refs(branch, resolved, &last_line(&fetched.stderr)));
    }
    let local = git(&[
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &format!("refs/heads/{branch}^{{commit}}"),
    ])?;
    let switched = if local.status.success() {
        git(&["switch", branch])?
    } else {
        git(&[
            "switch",
            "--track",
            "-c",
            branch,
            &format!("refs/remotes/origin/{branch}"),
        ])?
    };
    if !switched.status.success() {
        // git refuses a dirty switch itself, so its own last line is the
        // honest reason rather than one rk invents.
        return Err(RkError::subprocess(
            Diagnostic::new(
                Reason::SubprocessFailed,
                format!(
                    "git refused to check out {branch}: {}",
                    last_line(&switched.stderr)
                ),
            )
            .expected("a working tree the checkout can move")
            .target_state("the branch exists on the forge and is not checked out here"),
        ));
    }
    report(out, ground, resolved, None, Some(branch.to_owned()), true)
}

/// One report, in either mode.
fn report(
    out: Output,
    ground: &Ground,
    resolved: &Resolved,
    path: Option<Utf8PathBuf>,
    checkout: Option<String>,
    apply: bool,
) -> Result<(), RkError> {
    report_with(out, ground, resolved, path, checkout, apply, None)
}

/// [`report`], carrying a note the seating step raised.
fn report_with(
    out: Output,
    ground: &Ground,
    resolved: &Resolved,
    path: Option<Utf8PathBuf>,
    checkout: Option<String>,
    apply: bool,
    note: Option<String>,
) -> Result<(), RkError> {
    let mode = if apply { "apply" } else { "preview" };
    out.result_line(format!("issue:  #{} {}", resolved.number, resolved.title));
    out.result_line(format!(
        "branch: {}  ({})",
        resolved.branch.as_deref().unwrap_or("named by the forge"),
        match resolved.origin {
            "already" => "already linked at the forge",
            "forge" => "minted at the forge",
            _ => "not minted yet",
        }
    ));
    out.result_line(format!(
        "seat:   {} ({} says so)",
        path.as_ref().map_or_else(
            || checkout.as_deref().map_or_else(
                || "unknown".to_owned(),
                |branch| format!("checkout {branch}")
            ),
            ToString::to_string
        ),
        ground.workflow_source
    ));
    if !resolved.others.is_empty() {
        out.warn(format!(
            "the issue carries other linked branches, and the first was taken: {}",
            resolved.others.join(", ")
        ));
    }
    let detail = match (resolved.detail.clone(), note) {
        (Some(had), Some(note)) => Some(format!("{had}; {note}")),
        (Some(one), None) | (None, Some(one)) => Some(one),
        (None, None) => None,
    };
    if let Some(detail) = &detail {
        out.warn(detail);
    }
    let next = next_lines(ground, resolved, path.as_ref(), apply);
    out.next(&next);
    out.emit(&StartReport {
        schema: "rk.issue-start/1",
        mode,
        forge: ground.forge.as_str(),
        repo: ground.repo.clone(),
        issue: resolved.number,
        title: resolved.title.clone(),
        branch: resolved.branch.clone(),
        origin: resolved.origin,
        workflow: ground.workflow.as_str(),
        path: path.map(|path| path.to_string()),
        checkout,
        others: resolved.others.clone(),
        detail,
        next,
    })
}

/// What to run next, which differs by mode and by whether this ran.
fn next_lines(
    ground: &Ground,
    resolved: &Resolved,
    path: Option<&Utf8PathBuf>,
    apply: bool,
) -> Vec<String> {
    if !apply {
        return vec![format!(
            "rk issue start {} --apply mints the branch and seats it",
            resolved.number
        )];
    }
    match (ground.workflow, path) {
        (Workflow::Worktree, Some(path)) => vec![
            format!("cd {path}"),
            "rk worktree list reports every seat".to_owned(),
        ],
        _ => vec!["rk status reports what this target carries".to_owned()],
    }
}

/// The last non-empty stderr line, for a one-line reason.
fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .to_owned()
}
