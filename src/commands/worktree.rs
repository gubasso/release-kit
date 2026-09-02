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

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::worktree::{WorktreeAction, WorktreeArgs};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::maintenance;
use crate::output::Output;
use crate::setup::context::TRUNK_BRANCH;
use crate::worktree::{Layout, Worktree, derived_path, matches_grammar};

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

/// Whether one existing worktree holds uncommitted work; untracked files
/// count, and a probe that cannot answer counts too — fail closed.
fn is_dirty(path: &Utf8Path) -> bool {
    git(path, &["status", "--porcelain"]).map_or(true, |probed| {
        !probed.status.success() || !probed.stdout.is_empty()
    })
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

    use super::{ListReport, ListRow};

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
}
