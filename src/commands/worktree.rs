//! `rk worktree list | add | prune`: the worktree half of the local
//! cleanup pair.
//!
//! Mode-free by design: the verbs behave identically under the worktree
//! and branches workflows, and the recorded mode gates only what the
//! landed blocks render. The landed branches surface's idioms hold
//! throughout — preview by default, `--apply` to act, `--json` with a
//! versioned schema, `--quiet` for the hook path, exit 0 for any report,
//! and refusals through the existing matrix. Every git spawn lives here;
//! `crate::worktree` stays pure.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::branches::{Branch, Class, FOR_EACH_REF_FORMAT, merged_request_for};
use crate::cli::worktree::{WorktreeAction, WorktreeArgs};
use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::maintenance;
use crate::output::Output;
use crate::setup::context::{TRUNK_BRANCH, resolve_cli};
use crate::worktree::{Layout, Worktree, WtClass, classify, derived_path, matches_grammar};

/// The closing line a prune report ends with while some reported row
/// still names a move the operator may make; [`maintenance::row_owes`]
/// is the shared predicate.
const OPERATOR_LINE: &str = "Removing a worktree and deleting its branch are the operator's action: an agent reading this states the command and waits to be asked.";

/// Dispatch the worktree surface.
///
/// # Errors
///
/// Refuses a target that is not a directory or not a git repository, an
/// inventory the porcelain parser cannot trust, and the `add` refusals
/// each documented below; propagates subprocess failures through the
/// matrix after the report has named every outcome.
pub fn run(args: &WorktreeArgs) -> Result<(), RkError> {
    match &args.action {
        WorktreeAction::List { target, json } => list(target, Output::new(*json)),
        WorktreeAction::Add {
            branch,
            target,
            base,
            apply,
            json,
        } => add(target, branch, base.as_deref(), *apply, Output::new(*json)),
        WorktreeAction::Prune {
            target,
            repo,
            forge,
            verify,
            apply,
            quiet,
            json,
        } => prune(
            target,
            repo.as_deref(),
            forge.as_deref(),
            *verify,
            *apply,
            *quiet,
            Output::new(*json),
        ),
    }
}

// ---------------------------------------------------------------------------
// The common gate

/// The parsed worktree inventory, behind the common gate: the target is a
/// directory, git lists it, and the porcelain parser trusts every record.
fn inventory(target: &Utf8Path) -> Result<Vec<Worktree>, RkError> {
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory"),
            )
            .expected("an existing repository to read"),
        ));
    }
    let listed = git(target, &["worktree", "list", "--porcelain", "-z"])?;
    if !listed.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!("target {target} is not a git repository"),
            )
            .expected("a repository whose worktrees git can list"),
        ));
    }
    crate::worktree::parse_worktrees(&listed.stdout).map_err(|detail| {
        RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!("the worktree inventory cannot be trusted: {detail}"),
            )
            .expected("a worktree inventory this binary can parse whole")
            .target_state("unchanged"),
        )
    })
}

/// The repository layout, from the inventory the gate already parsed.
fn layout_of(worktrees: &[Worktree]) -> Result<Layout, RkError> {
    Layout::of(worktrees).map_err(|detail| {
        RkError::refusal(
            Diagnostic::new(Reason::PrerequisiteUnmet, detail)
                .expected("a main worktree the sibling convention composes with"),
        )
    })
}

/// The seats in use: the caller's own worktree, resolved from the process
/// working directory, and the target's current worktree — both,
/// independently, so invoking from worktree A with `--target` naming the
/// main checkout still keeps A.
fn seats(target: &Utf8Path) -> Vec<Utf8PathBuf> {
    let toplevel = |output: std::io::Result<std::process::Output>| {
        output
            .ok()
            .filter(|answer| answer.status.success())
            .map(|answer| String::from_utf8_lossy(&answer.stdout).trim().to_owned())
            .filter(|path| !path.is_empty())
            .map(Utf8PathBuf::from)
    };
    let mut seats = Vec::new();
    if let Some(seat) = toplevel(
        std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output(),
    ) {
        seats.push(seat);
    }
    if let Some(seat) = toplevel(
        std::process::Command::new("git")
            .arg("-C")
            .arg(target.as_std_path())
            .args(["rev-parse", "--show-toplevel"])
            .output(),
    ) {
        if !seats.contains(&seat) {
            seats.push(seat);
        }
    }
    seats
}

/// Whether one existing worktree holds uncommitted work; untracked files
/// count, and a probe that cannot answer counts too — fail closed.
fn is_dirty(path: &Utf8Path) -> bool {
    git(path, &["status", "--porcelain"]).map_or(true, |probed| {
        !probed.status.success() || !probed.stdout.is_empty()
    })
}

/// The local branches, parsed; [`crate::branches::parse_branches`] skips
/// a malformed line, so the caller judges absence against the worktrees.
fn branch_inventory(target: &Utf8Path) -> Result<Vec<Branch>, RkError> {
    let listed = git(
        target,
        &[
            "for-each-ref",
            "refs/heads",
            "--format",
            FOR_EACH_REF_FORMAT,
        ],
    )?;
    if !listed.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!("target {target} is not a git repository"),
            )
            .expected("a repository whose branches git can list"),
        ));
    }
    Ok(crate::branches::parse_branches(&String::from_utf8_lossy(
        &listed.stdout,
    )))
}

// ---------------------------------------------------------------------------
// list

/// One worktree in the list report.
#[derive(Debug, Serialize)]
struct ListRow {
    /// The worktree's path.
    path: String,
    /// The checked-out branch; absent when detached.
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    /// The full object name at HEAD.
    head: String,
    /// `main` or `linked`.
    kind: &'static str,
    /// One state by fixed precedence: locked over missing over detached
    /// over dirty over clean.
    state: &'static str,
    /// Whether a linked worktree sits at its derived sibling path.
    canonical: bool,
}

/// The machine form of a list report.
#[derive(Debug, Serialize)]
struct ListReport {
    /// The shape version of this document.
    schema: &'static str,
    /// Every worktree, the main one first.
    worktrees: Vec<ListRow>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// The offline inventory: every worktree, one deterministic state each.
fn list(target: &Utf8Path, out: Output) -> Result<(), RkError> {
    let worktrees = inventory(target)?;
    let layout = layout_of(&worktrees)?;
    let rows: Vec<ListRow> = worktrees
        .iter()
        .enumerate()
        .map(|(index, worktree)| {
            // The precedence is deterministic and pinned: a locked record
            // whose directory is missing reports locked, an unlocked one
            // reports missing, and no status probe runs on an absent path.
            let state = if worktree.locked.is_some() {
                "locked"
            } else if worktree.prunable.is_some() {
                "missing"
            } else if worktree.branch.is_none() {
                "detached"
            } else if is_dirty(&worktree.path) {
                "dirty"
            } else {
                "clean"
            };
            let canonical = index == 0
                || worktree
                    .branch
                    .as_deref()
                    .is_none_or(|branch| derived_path(&layout, branch) == worktree.path);
            ListRow {
                path: worktree.path.to_string(),
                branch: worktree.branch.clone(),
                head: worktree.head.clone(),
                kind: if index == 0 { "main" } else { "linked" },
                state,
                canonical,
            }
        })
        .collect();
    let next = vec![
        "rk worktree add <branch> creates or adopts a branch's worktree".to_owned(),
        "rk worktree prune reports the worktrees a squash merge retired".to_owned(),
    ];
    out.result_line(format!(
        "{} worktree{} of {}:",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" },
        layout.main
    ));
    let width = rows.iter().map(|row| row.path.len()).max().unwrap_or(0);
    for row in &rows {
        let head = row.head.get(..8).unwrap_or(&row.head);
        let mut line = format!(
            "  {:width$}  {head}  {}  {}",
            row.path,
            row.branch.as_deref().unwrap_or("(detached)"),
            row.state
        );
        if !row.canonical {
            if let Some(branch) = &row.branch {
                use std::fmt::Write as _;
                let expected = derived_path(&layout, branch);
                let _ = write!(
                    line,
                    "  off-path: expected ../{}",
                    expected.file_name().unwrap_or_default()
                );
            }
        }
        out.result_line(line);
    }
    out.next(&next);
    out.emit(&ListReport {
        schema: "rk.worktree-list/1",
        worktrees: rows,
        next,
    })
}

// ---------------------------------------------------------------------------
// add

/// The machine form of an add report.
#[derive(Debug, Serialize)]
struct AddReport {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The branch the worktree seats.
    branch: String,
    /// The worktree's path, absolute.
    path: String,
    /// What the run creates: `branch` (a new branch and its worktree),
    /// `worktree` (an existing branch adopted), or `nothing` (the
    /// canonical worktree already stands).
    created: &'static str,
    /// Where the branch comes from: `adopted`, `remote`, `base`, or
    /// `trunk`.
    source: &'static str,
    /// The commit-ish a created branch starts from, where one is created.
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<String>,
    /// The upstream a tracking branch was created against, where one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    upstream: Option<String>,
    /// What the run wants said beside the result.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// One resolved source for the branch.
struct Source {
    /// `adopted`, `remote`, `base`, or `trunk`.
    kind: &'static str,
    /// What creates: `branch`, `worktree`, or `nothing`.
    created: &'static str,
    /// The commit-ish shown as the base, where a branch is created.
    base: Option<String>,
    /// The upstream of a created tracking branch.
    upstream: Option<String>,
    /// The exact git invocation, argv after `git`.
    command: Vec<String>,
}

/// Create or adopt one branch's worktree at its derived sibling path.
#[allow(clippy::too_many_lines)]
fn add(
    target: &Utf8Path,
    branch: &str,
    base: Option<&str>,
    apply: bool,
    out: Output,
) -> Result<(), RkError> {
    let worktrees = inventory(target)?;
    let layout = layout_of(&worktrees)?;

    // The convention's grammar first — necessary, not sufficient — then
    // git's own ref rules, so a name that would fail at `git worktree
    // add` fails at the preview instead, with git's reason.
    if !matches_grammar(branch) {
        return Err(RkError::Usage(format!(
            "branch '{branch}' is none of the three forms — <type>/<slug>, <issue-id>-<slug>, or release/<line> — the landed grammar admits"
        )));
    }
    let checked = git(target, &["check-ref-format", "--branch", branch])?;
    if !checked.status.success() {
        return Err(RkError::Usage(format!(
            "git refuses the branch name '{branch}': {}",
            last_line(&checked.stderr)
        )));
    }
    if branch == TRUNK_BRANCH {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!("{TRUNK_BRANCH} takes no worktree; the main checkout is its seat"),
            )
            .expected("a short-lived branch to seat")
            .target_state("unchanged"),
        ));
    }
    if let Some(base) = base {
        if base.starts_with('-') {
            return Err(RkError::Usage(format!(
                "--base '{base}' is option-shaped; pass a commit-ish"
            )));
        }
    }
    let path = derived_path(&layout, branch);

    // Refusals before any mutation: the collision, and the non-canonical
    // seat. The already-standing canonical worktree is satisfied instead.
    let registered = worktrees
        .iter()
        .find(|worktree| worktree.branch.as_deref() == Some(branch));
    if let Some(seat) = registered {
        if seat.path == path {
            return report_satisfied(out, branch, &path, apply);
        }
        let move_hint = if seat.path == layout.main {
            format!("; git switch {TRUNK_BRANCH} there, then re-run")
        } else {
            String::new()
        };
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "branch {branch} is checked out at {}, and one branch has one seat{move_hint}",
                    seat.path
                ),
            )
            .expected("the branch free, or already at its derived path")
            .target_state("unchanged"),
        ));
    }
    if path.exists() {
        let occupant = worktrees
            .iter()
            .find(|worktree| worktree.path == path)
            .and_then(|worktree| worktree.branch.clone())
            .map_or_else(
                || "a directory this repository does not register".to_owned(),
                |other| format!("the worktree of branch {other}"),
            );
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "{path} already exists as {occupant}; flattening is not injective and nothing is suffixed silently"
                ),
            )
            .expected("the derived path free, or registered to this branch")
            .target_state("unchanged"),
        ));
    }

    // Under apply, refresh through the remote's configured refmap first —
    // best-effort, because a missing remote must not block a local
    // branch — then resolve; a preview decides from the local refs as
    // they stand and says so.
    let mut detail = None;
    if apply {
        let fetched = git(target, &["fetch", "origin"])?;
        if !fetched.status.success() {
            detail = Some(format!(
                "the fetch failed ({}); the run proceeded on local refs",
                last_line(&fetched.stderr)
            ));
        }
    }
    let source = resolve_source(target, branch, base, &path)?;

    if !apply {
        out.result_line(format!(
            "branch: {branch}  ({})",
            match source.kind {
                "adopted" => "existing, adopted".to_owned(),
                "remote" => format!(
                    "remote, from {}",
                    source.upstream.as_deref().unwrap_or("origin")
                ),
                _ => format!("new, from {}", source.base.as_deref().unwrap_or("?")),
            }
        ));
        out.result_line(format!(
            "path:   ../{}",
            path.file_name().unwrap_or_default()
        ));
        if let Some(base) = &source.base {
            out.result_line(format!("base:   {base}"));
        }
        out.result_line(format!("would run: git {}", source.command.join(" ")));
        let next = vec![format!(
            "rk worktree add {branch} --target {target} --apply creates it; the apply refreshes the remote refs and re-resolves"
        )];
        out.next(&next);
        return out.emit(&AddReport {
            schema: "rk.worktree-add/1",
            mode: "preview",
            branch: branch.to_owned(),
            path: path.to_string(),
            created: source.created,
            source: source.kind,
            base: source.base,
            upstream: source.upstream,
            detail: Some(
                "a preview decides from the local refs as they stand; apply refreshes and re-resolves"
                    .to_owned(),
            ),
            next,
        });
    }

    let argv: Vec<&str> = source.command.iter().map(String::as_str).collect();
    let created = git(target, &argv)?;
    if !created.status.success() {
        return Err(RkError::subprocess(
            Diagnostic::new(
                Reason::SubprocessFailed,
                format!("git worktree add refused: {}", last_line(&created.stderr)),
            )
            .expected("the worktree created at the derived path")
            .target_state("unchanged"),
        ));
    }
    out.result_line(&path);
    let next = vec![
        format!("cd {path}"),
        "rk worktree list reports every seat".to_owned(),
    ];
    out.next(&next);
    out.emit(&AddReport {
        schema: "rk.worktree-add/1",
        mode: "apply",
        branch: branch.to_owned(),
        path: path.to_string(),
        created: source.created,
        source: source.kind,
        base: source.base,
        upstream: source.upstream,
        detail,
        next,
    })
}

/// The idempotent outcome: the canonical worktree already stands.
fn report_satisfied(
    out: Output,
    branch: &str,
    path: &Utf8Path,
    apply: bool,
) -> Result<(), RkError> {
    out.result_line(format!("{path} already seats {branch}; nothing to create"));
    let next = vec![format!("cd {path}")];
    out.next(&next);
    out.emit(&AddReport {
        schema: "rk.worktree-add/1",
        mode: if apply { "apply" } else { "preview" },
        branch: branch.to_owned(),
        path: path.to_string(),
        created: "nothing",
        source: "adopted",
        base: None,
        upstream: None,
        detail: None,
        next,
    })
}

/// The source precedence, in order: adopt a local branch, create a
/// tracking branch from a lone matching remote tip, else create from
/// `--base` or the refreshed trunk — so a forge-minted branch or the
/// bot's release branch is seated from its real tip, never silently
/// recreated from the trunk. Everything is resolved to an exact object
/// name behind `--end-of-options` before any mutation, and the one name
/// passed onward — the remote ref a tracking branch needs — sits in a
/// documented value position, fully qualified.
fn resolve_source(
    target: &Utf8Path,
    branch: &str,
    base: Option<&str>,
    path: &Utf8Path,
) -> Result<Source, RkError> {
    let resolve = |name: &str| -> Result<Option<String>, RkError> {
        let resolved = git(
            target,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &format!("{name}^{{commit}}"),
            ],
        )?;
        Ok(resolved
            .status
            .success()
            .then(|| String::from_utf8_lossy(&resolved.stdout).trim().to_owned()))
    };

    // Arm 1: the branch exists locally — adopt it. The caller already
    // handled a branch seated elsewhere; here it is free.
    if resolve(&format!("refs/heads/{branch}"))?.is_some() {
        return Ok(Source {
            kind: "adopted",
            created: "worktree",
            base: None,
            upstream: None,
            command: vec![
                "worktree".into(),
                "add".into(),
                path.to_string(),
                branch.to_owned(),
            ],
        });
    }

    // Arm 2: exactly origin/<branch> exists — create the local tracking
    // branch from that remote tip.
    let remote_ref = format!("refs/remotes/origin/{branch}");
    if resolve(&remote_ref)?.is_some() {
        return Ok(Source {
            kind: "remote",
            created: "branch",
            base: Some(format!("origin/{branch}")),
            upstream: Some(format!("origin/{branch}")),
            command: vec![
                "worktree".into(),
                "add".into(),
                "--track".into(),
                "-b".into(),
                branch.to_owned(),
                path.to_string(),
                remote_ref,
            ],
        });
    }

    // Arm 3: --base where given, else the refreshed trunk; a release
    // line requires the explicit base — a line is cut from a tag, never
    // the tip.
    if branch.starts_with(crate::branches::PROTECTED_PREFIX) && base.is_none() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!(
                    "release line {branch} takes an explicit --base; a line is cut from a tag, never the tip"
                ),
            )
            .expected("--base \"v<version>\" naming the tag the line patches")
            .target_state("unchanged"),
        ));
    }
    let (kind, shown) = base.map_or_else(
        || ("trunk", format!("origin/{TRUNK_BRANCH}")),
        |base| ("base", base.to_owned()),
    );
    let resolved = match resolve(&shown)? {
        Some(oid) => Some(oid),
        // A clone with no remote still creates from its own trunk.
        None if kind == "trunk" => resolve(TRUNK_BRANCH)?,
        None => None,
    };
    let oid = resolved.ok_or_else(|| {
        RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!("{shown} does not resolve to a commit"),
            )
            .expected("a commit-ish the new branch can start from")
            .target_state("unchanged"),
        )
    })?;
    Ok(Source {
        kind,
        created: "branch",
        base: Some(shown),
        upstream: None,
        command: vec![
            "worktree".into(),
            "add".into(),
            path.to_string(),
            "-b".into(),
            branch.to_owned(),
            oid,
        ],
    })
}

// ---------------------------------------------------------------------------
// prune

/// One worktree in the prune report.
#[derive(Debug, Serialize)]
struct PruneRow {
    /// The worktree's path.
    path: String,
    /// The branch it seats, where one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    /// The branch tip the judgment rests on, where one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    tip: Option<String>,
    /// The judgment: kept, candidate, stale, confirmed, unconfirmed,
    /// unknown, pruned, remove-failed, or branch-delete-failed.
    status: &'static str,
    /// The merged request that proved the tip, where one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<String>,
    /// Why the worktree stays, or what is still owed.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl PruneRow {
    /// The human tail of a row line.
    fn describe(&self) -> String {
        match self.status {
            "kept" => format!("kept: {}", self.detail.as_deref().unwrap_or("")),
            "stale" => {
                "stale: the registered directory is missing; apply clears the record".to_owned()
            }
            "confirmed" => format!(
                "confirmed: merged request {} matches this tip",
                self.request.as_deref().unwrap_or("")
            ),
            "unconfirmed" => format!("unconfirmed: {}", self.detail.as_deref().unwrap_or("")),
            "unknown" => format!("unknown: {}", self.detail.as_deref().unwrap_or("")),
            "pruned" => {
                let mut line = self.request.as_deref().map_or_else(
                    || "pruned".to_owned(),
                    |request| format!("pruned (merged request {request})"),
                );
                if let Some(detail) = &self.detail {
                    line.push_str("; ");
                    line.push_str(detail);
                }
                line
            }
            "remove-failed" => format!("remove failed: {}", self.detail.as_deref().unwrap_or("")),
            "branch-delete-failed" => format!(
                "branch delete failed: {}",
                self.detail.as_deref().unwrap_or("")
            ),
            _ => "candidate".to_owned(),
        }
    }
}

/// The machine form of a prune report.
#[derive(Debug, Serialize)]
struct PruneReport {
    /// The shape version of this document.
    schema: &'static str,
    /// Which mode produced it: preview, verify, or apply.
    mode: &'static str,
    /// Every reportable worktree, judged; empty when the clone is clean.
    worktrees: Vec<PruneRow>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// One reportable worktree, carried from classification to the report.
struct Judged {
    worktree: Worktree,
    /// The branch observation, where the join found one.
    tip: Option<String>,
    class: WtClass,
}

/// The ordered cleanup: report stale records and gone-upstream worktrees
/// — never the main checkout or a healthy linked one — confirm against
/// the forge under `--verify`, and remove worktree before branch under
/// `--apply`, re-observing at the moment of action.
#[allow(clippy::too_many_lines)]
fn prune(
    target: &Utf8Path,
    repo_flag: Option<&str>,
    forge_flag: Option<&str>,
    verify: bool,
    apply: bool,
    quiet: bool,
    out: Output,
) -> Result<(), RkError> {
    let worktrees = inventory(target)?;
    let layout = layout_of(&worktrees)?;
    let branches = branch_inventory(target)?;
    // The join fails closed as a whole: no branch lines where the
    // inventory names checked-out branches means the observation itself
    // cannot be trusted, and no judgment is made over it.
    if branches.is_empty() && worktrees.iter().any(|worktree| worktree.branch.is_some()) {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                "the branch inventory did not parse, and no worktree is judged without its branch observation",
            )
            .expected("a branch listing covering the checked-out branches")
            .target_state("unchanged"),
        ));
    }
    let seat_paths = seats(target);
    let seat_refs: Vec<&Utf8Path> = seat_paths.iter().map(Utf8PathBuf::as_path).collect();

    // The reportable set: stale-eligible records, and linked worktrees
    // whose branch observation is gone — or missing, which is kept by
    // name, never guessed. A healthy seat is never a row.
    let mut judged: Vec<Judged> = Vec::new();
    for worktree in worktrees.iter().skip(1) {
        let observation = worktree
            .branch
            .as_deref()
            .and_then(|name| branches.iter().find(|branch| branch.name == name));
        let reportable = worktree.prunable.is_some()
            || worktree
                .branch
                .as_deref()
                .is_some_and(|_| observation.is_none_or(|branch| branch.gone));
        if !reportable {
            continue;
        }
        let dirty = worktree.prunable.is_none() && is_dirty(&worktree.path);
        let class = classify(
            worktree,
            observation,
            &layout,
            &seat_refs,
            TRUNK_BRANCH,
            dirty,
        );
        judged.push(Judged {
            worktree: worktree.clone(),
            tip: observation.map(|branch| branch.tip.clone()),
            class,
        });
    }

    // The forge is asked only where a candidate exists to confirm.
    if (verify || apply)
        && judged
            .iter()
            .any(|row| matches!(row.class, WtClass::Candidate))
    {
        let resolved = crate::landing::resolve(target, forge_flag, repo_flag)?;
        let forge = Forge::parse(&resolved.forge)
            .ok_or_else(|| RkError::Usage(format!("unknown forge '{}'", resolved.forge)))?;
        let repo = resolved.repo.ok_or_else(crate::landing::repo_unresolved)?;
        let cli = resolve_cli(forge)?;
        for row in &mut judged {
            if matches!(row.class, WtClass::Candidate) {
                let Some(tip) = row.tip.as_deref() else {
                    continue;
                };
                row.class = WtClass::Judged(merged_request_for(
                    &cli,
                    target.as_std_path(),
                    forge,
                    &repo,
                    tip,
                ));
            }
        }
    }

    let mut rows: Vec<PruneRow> = judged
        .iter()
        .map(|row| {
            let (status, request, detail) = match &row.class {
                WtClass::Kept { reason } => ("kept", None, Some(reason.clone())),
                WtClass::Candidate => ("candidate", None, None),
                WtClass::Stale => ("stale", None, None),
                WtClass::Judged(Class::Confirmed { request }) => {
                    ("confirmed", Some(request.clone()), None)
                }
                WtClass::Judged(Class::Unconfirmed { detail }) => {
                    ("unconfirmed", None, Some(detail.clone()))
                }
                WtClass::Judged(Class::Unknown { detail }) => {
                    ("unknown", None, Some(detail.clone()))
                }
                WtClass::Judged(_) => ("kept", None, Some("guarded".to_owned())),
            };
            PruneRow {
                path: row.worktree.path.to_string(),
                branch: row.worktree.branch.clone(),
                tip: row.tip.clone(),
                status,
                request,
                detail,
            }
        })
        .collect();

    let mut failures = 0usize;
    if apply {
        for row in &mut rows {
            if row.status != "confirmed" {
                continue;
            }
            if let Err(count) = retire(target, row) {
                failures += count;
            }
        }
        failures += sweep_stale(target, &mut rows)?;
    }

    let mode = if apply {
        "apply"
    } else if verify {
        "verify"
    } else {
        "preview"
    };
    let next = next_lines(mode);
    render(out, &rows, &next, quiet);
    out.emit(&PruneReport {
        schema: "rk.worktree-prune/1",
        mode,
        worktrees: rows,
        next,
    })?;
    if failures > 0 {
        return Err(RkError::subprocess(
            Diagnostic::new(
                Reason::SubprocessFailed,
                format!("git refused {failures} cleanup actions"),
            )
            .expected("every confirmed worktree removed; the report names each outcome"),
        ));
    }
    Ok(())
}

/// Retire one confirmed worktree, ordered, each outcome independent:
/// re-observe at the last moment — verification authorizes only the
/// state it saw — then remove the worktree, then delete its branch
/// through the shared compare-and-swap helper. A failed remove leaves
/// the branch and its configuration untouched.
fn retire(target: &Utf8Path, row: &mut PruneRow) -> Result<(), usize> {
    let Some(branch) = row.branch.clone() else {
        return Ok(());
    };
    let Some(tip) = row.tip.clone() else {
        return Ok(());
    };
    let keep = |row: &mut PruneRow, moved: &str| {
        row.status = "kept";
        row.detail = Some(format!(
            "{moved} after verification; rk worktree prune --verify re-confirms"
        ));
    };
    let reread = git(
        target,
        &[
            "for-each-ref",
            &format!("refs/heads/{branch}"),
            "--format",
            "%(objectname)",
        ],
    )
    .map_err(|_| 1usize)?;
    let fresh_tip = String::from_utf8_lossy(&reread.stdout).trim().to_owned();
    if !reread.status.success() || fresh_tip != tip {
        keep(row, "the tip moved");
        return Ok(());
    }
    // The fresh inventory fails closed: an unobservable state clears no
    // removal, and the record must still be the same resource — the very
    // branch the merge proof named, unlocked, its directory standing.
    let path = Utf8PathBuf::from(&row.path);
    let fresh = git(target, &["worktree", "list", "--porcelain", "-z"]).map_err(|_| 1usize)?;
    if !fresh.status.success() {
        keep(row, "the worktree inventory could not be re-read");
        return Ok(());
    }
    let Ok(inventory) = crate::worktree::parse_worktrees(&fresh.stdout) else {
        keep(row, "the worktree inventory could not be re-read");
        return Ok(());
    };
    let seat = inventory.iter().find(|worktree| worktree.path == path);
    if let Some(reason) = crate::worktree::reobservation(seat, &branch) {
        keep(row, &reason);
        return Ok(());
    }
    if is_dirty(&path) {
        keep(row, "uncommitted changes arrived");
        return Ok(());
    }
    let removed = git(target, &["worktree", "remove", row.path.as_str()]).map_err(|_| 1usize)?;
    if !removed.status.success() {
        row.status = "remove-failed";
        row.detail = Some(format!(
            "{}; clear what holds it — the dirt, the lock, the process in the directory — and re-run rk worktree prune --apply",
            last_line(&removed.stderr)
        ));
        return Err(1);
    }
    match maintenance::delete_branch(target, &branch, &tip) {
        maintenance::Deletion::Deleted => {
            row.status = "pruned";
            Ok(())
        }
        maintenance::Deletion::ConfigSurvived { detail } => {
            row.status = "pruned";
            row.detail = Some(detail);
            Ok(())
        }
        maintenance::Deletion::Refused { detail } => {
            // Reported truthfully: the worktree is already gone, the
            // branch and its work survive, and the recovery is named.
            row.status = "branch-delete-failed";
            row.detail = Some(format!(
                "{detail}; the worktree is removed and the branch survives with its work: rk worktree add {branch} --apply re-seats it"
            ));
            Err(1)
        }
    }
}

/// Clear the stale records, once, after the loop: plain `git worktree
/// prune` is expiration-gated, so `--expire now` is the form that
/// guarantees the missing-directory records go — and only those; a
/// locked record is never touched and was never a stale row. Because it
/// is one command over many rows, its outcome is read per row from a
/// fresh inventory rather than assumed.
fn sweep_stale(target: &Utf8Path, rows: &mut [PruneRow]) -> Result<usize, RkError> {
    if !rows.iter().any(|row| row.status == "stale") {
        return Ok(0);
    }
    let mut failures = 0usize;
    let swept = git(target, &["worktree", "prune", "--expire", "now"])?;
    // Fail closed: only an inventory that was actually re-read proves a
    // record gone, so an unreadable one marks every stale row failed
    // rather than claiming a sweep nothing observed.
    let survivors: Option<Vec<Utf8PathBuf>> =
        git(target, &["worktree", "list", "--porcelain", "-z"])
            .ok()
            .filter(|fresh| fresh.status.success())
            .and_then(|fresh| crate::worktree::parse_worktrees(&fresh.stdout).ok())
            .map(|inventory| {
                inventory
                    .into_iter()
                    .map(|worktree| worktree.path)
                    .collect()
            });
    for row in rows.iter_mut().filter(|row| row.status == "stale") {
        let survived = survivors
            .as_ref()
            .is_none_or(|paths| paths.iter().any(|path| *path == row.path));
        if survived {
            row.status = "remove-failed";
            row.detail = Some(if survivors.is_none() {
                "the record's fate could not be observed; re-run rk worktree prune --apply"
                    .to_owned()
            } else if swept.status.success() {
                "the record survived the sweep; re-run rk worktree prune --apply".to_owned()
            } else {
                format!(
                    "{}; re-run rk worktree prune --apply",
                    last_line(&swept.stderr)
                )
            });
            failures += 1;
        } else {
            row.status = "pruned";
        }
    }
    if !swept.status.success() && failures == 0 {
        failures = 1;
    }
    Ok(failures)
}

/// What plausibly follows each mode; an apply is its own conclusion.
fn next_lines(mode: &str) -> Vec<String> {
    let verify = "rk worktree prune --verify confirms each candidate against the forge";
    let apply = "rk worktree prune --apply verifies, then removes each worktree before its branch";
    match mode {
        "preview" => vec![verify.to_owned(), apply.to_owned()],
        "verify" => vec![apply.to_owned()],
        _ => Vec::new(),
    }
}

/// The human report: silent under `--quiet` when nothing is reportable —
/// the clean-clone guarantee the reminder hook rests on — one judged line
/// per reportable worktree otherwise, closed by who owns the removal only
/// while some row still names a move.
fn render(out: Output, rows: &[PruneRow], next: &[String], quiet: bool) {
    if quiet && rows.is_empty() {
        return;
    }
    if rows.is_empty() {
        out.result_line("no worktree needs cleanup");
    } else {
        out.result_line(header(rows.len()));
        let width = rows.iter().map(|row| row.path.len()).max().unwrap_or(0);
        for row in rows {
            let tip = row
                .tip
                .as_deref()
                .map_or("        ", |tip| tip.get(..8).unwrap_or(tip));
            out.result_line(format!("  {:width$}  {tip}  {}", row.path, row.describe()));
        }
    }
    out.next(next);
    if rows
        .iter()
        .any(|row| maintenance::row_owes(row.status, row.detail.as_deref()))
    {
        out.result_line(OPERATOR_LINE);
    }
}

/// The count-bearing first line.
fn header(count: usize) -> String {
    if count == 1 {
        "1 worktree reports cleanup (a candidate, not proof):".to_owned()
    } else {
        format!("{count} worktrees report cleanup (a candidate, not proof):")
    }
}

/// Run one git command against the target, spawn failure typed.
fn git(target: &Utf8Path, args: &[&str]) -> Result<std::process::Output, RkError> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(target.as_std_path())
        .args(args)
        .output()
        .map_err(|source| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("git did not run: {source}"),
                )
                .expected("git installed and on PATH"),
            )
        })
}

/// The last non-empty stderr line, for a one-line detail.
fn last_line(bytes: &[u8]) -> String {
    maintenance::last_line(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{ListReport, ListRow, PruneReport, PruneRow};

    /// The complete `rk.worktree-list/1` shape, held by snapshot in the
    /// populated and empty-next forms.
    #[test]
    fn the_worktree_list_schema_snapshot_holds() {
        let populated = ListReport {
            schema: "rk.worktree-list/1",
            worktrees: vec![
                ListRow {
                    path: "/srv/widget".into(),
                    branch: Some("master".into()),
                    head: "aaaabbbbccccddddaaaabbbbccccddddaaaabbbb".into(),
                    kind: "main",
                    state: "clean",
                    canonical: true,
                },
                ListRow {
                    path: "/srv/elsewhere".into(),
                    branch: None,
                    head: "bbbbccccddddaaaabbbbccccddddaaaabbbbcccc".into(),
                    kind: "linked",
                    state: "detached",
                    canonical: true,
                },
            ],
            next: vec!["rk worktree prune reports the worktrees a squash merge retired".into()],
        };
        assert_eq!(
            serde_json::to_string(&populated).expect("a report serializes"),
            r#"{"schema":"rk.worktree-list/1","worktrees":[{"path":"/srv/widget","branch":"master","head":"aaaabbbbccccddddaaaabbbbccccddddaaaabbbb","kind":"main","state":"clean","canonical":true},{"path":"/srv/elsewhere","head":"bbbbccccddddaaaabbbbccccddddaaaabbbbcccc","kind":"linked","state":"detached","canonical":true}],"next":["rk worktree prune reports the worktrees a squash merge retired"]}"#,
            "a detached row must omit branch rather than serializing null"
        );
    }

    /// The complete `rk.worktree-prune/1` shape, held by snapshot in the
    /// populated and clean forms.
    #[test]
    fn the_worktree_prune_schema_snapshot_holds() {
        let populated = PruneReport {
            schema: "rk.worktree-prune/1",
            mode: "verify",
            worktrees: vec![
                PruneRow {
                    path: "/srv/widget-feat-x".into(),
                    branch: Some("feat/x".into()),
                    tip: Some("aaaabbbbccccddddaaaabbbbccccddddaaaabbbb".into()),
                    status: "confirmed",
                    request: Some("#8".into()),
                    detail: None,
                },
                PruneRow {
                    path: "/srv/widget-fix-y".into(),
                    branch: None,
                    tip: None,
                    status: "stale",
                    request: None,
                    detail: None,
                },
            ],
            next: vec![
                "rk worktree prune --apply verifies, then removes each worktree before its branch"
                    .into(),
            ],
        };
        assert_eq!(
            serde_json::to_string(&populated).expect("a report serializes"),
            r##"{"schema":"rk.worktree-prune/1","mode":"verify","worktrees":[{"path":"/srv/widget-feat-x","branch":"feat/x","tip":"aaaabbbbccccddddaaaabbbbccccddddaaaabbbb","status":"confirmed","request":"#8"},{"path":"/srv/widget-fix-y","status":"stale"}],"next":["rk worktree prune --apply verifies, then removes each worktree before its branch"]}"##
        );
        let clean = PruneReport {
            schema: "rk.worktree-prune/1",
            mode: "preview",
            worktrees: vec![],
            next: vec![
                "rk worktree prune --verify confirms each candidate against the forge".into(),
            ],
        };
        assert_eq!(
            serde_json::to_string(&clean).expect("a report serializes"),
            r#"{"schema":"rk.worktree-prune/1","mode":"preview","worktrees":[],"next":["rk worktree prune --verify confirms each candidate against the forge"]}"#,
            "a clean clone reports one empty list a caller can branch on"
        );
    }
}
