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
use crate::setup::context::resolve_cli;

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
    let Some(repo) = overrides
        .repo
        .map(str::to_owned)
        .or_else(|| reference.repo.clone())
        .or(detected.repo)
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
    let recorded = manifest::load(target)?.map(|held| held.parameters.workflow);
    let (workflow, workflow_source) = match (overrides.workflow, recorded) {
        (Some(raw), _) => (Workflow::parse(raw)?, "the --workflow flag"),
        (None, Some(held)) => (held, "the landing record"),
        // A target with no record is treated as the convention's own
        // mode rather than as branches: the record's serde default exists
        // for records written before the parameter, not for targets that
        // never landed.
        (None, None) => (Workflow::Worktree, "the default, with no landing record"),
    };
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
    // The gate before the first forge call, so a stale CLI costs one
    // local process rather than a half-finished remote change.
    probes::require_forge_cli(ground.forge)?;
    let cli = resolve_cli(ground.forge)?;
    let resolved = issue::resolve(
        &cli,
        target.as_std_path(),
        ground.forge,
        &ground.repo,
        &reference,
        overrides.base,
        apply,
    )?;
    match ground.workflow {
        Workflow::Worktree => seat_worktree(target, &ground, &resolved, overrides.base, apply, out),
        Workflow::Branches => seat_branch(target, &ground, &resolved, apply, out),
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
    let path = match seat {
        crate::commands::worktree::Seat::Satisfied { path } => path,
        crate::commands::worktree::Seat::Fresh { path, source, .. } => {
            if apply {
                crate::commands::worktree::create_seat(target, &source)?;
            }
            path
        }
    };
    report(out, ground, resolved, Some(path), None, apply)
}

/// Branches mode: the branch checked out in the main checkout.
///
/// Both forges create the branch on the remote, so an apply always sees
/// the same case — a remote tip with no local branch — unless a previous
/// run already made one.
fn seat_branch(
    target: &Utf8Path,
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
    let git = |args: &[&str]| -> Result<std::process::Output, RkError> {
        std::process::Command::new(probes::git_bin())
            .args(["-C"])
            .arg(target)
            .args(args)
            .output()
            .map_err(|source| {
                RkError::subprocess(
                    Diagnostic::new(
                        Reason::SubprocessSpawn,
                        format!("git did not run: {source}"),
                    )
                    .target_state("unchanged"),
                )
            })
    };
    let _ = git(&["fetch", "origin"])?;
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
    if let Some(detail) = &resolved.detail {
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
        detail: resolved.detail.clone(),
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
