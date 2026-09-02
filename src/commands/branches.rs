//! `rk branches prune`: report the branches a squash merge retired.
//!
//! Preview by default and fully offline: the post-merge hook runs it on
//! every pull, so the read path costs no network and no forge CLI. Only
//! `--verify` and `--apply` resolve the forge, and only `--apply` deletes
//! — each branch on the strength of a merged request whose recorded head
//! equals the local tip, never on `[gone]` alone.

use camino::Utf8Path;
use serde::Serialize;

use crate::branches::{Branch, Class, FOR_EACH_REF_FORMAT, classify, merged_request_for};
use crate::cli::branches::{BranchesAction, BranchesArgs};
use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::output::Output;
use crate::setup::context::{TRUNK_BRANCH, resolve_cli};

/// The closing line every non-quiet report ends with; it states who owns
/// the deletion, in the same voice as the landed routing block.
const OPERATOR_LINE: &str = "Deleting a branch is the operator's action: an agent reading this states the command and waits to be asked.";

/// The machine form of a prune report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// Which mode produced it: preview, verify, or apply.
    mode: &'static str,
    /// Every gone branch, judged; empty when the clone is clean.
    branches: Vec<Row>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// One gone branch in the report.
#[derive(Debug, Serialize)]
struct Row {
    /// The branch name.
    name: String,
    /// The full object name at the tip.
    tip: String,
    /// The judgment: candidate, kept, confirmed, unconfirmed, unknown,
    /// deleted, or delete-failed.
    status: &'static str,
    /// The merged request that proved the tip, where one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<String>,
    /// Why the branch was kept or the answer is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl Row {
    /// Map one judgment onto its wire form.
    fn from(branch: &Branch, class: Class) -> Self {
        let (status, request, detail) = match class {
            Class::Kept { reason } => ("kept", None, Some(reason)),
            Class::Candidate => ("candidate", None, None),
            Class::Confirmed { request } => ("confirmed", Some(request), None),
            Class::Unconfirmed { detail } => ("unconfirmed", None, Some(detail)),
            Class::Unknown { detail } => ("unknown", None, Some(detail)),
        };
        Self {
            name: branch.name.clone(),
            tip: branch.tip.clone(),
            status,
            request,
            detail,
        }
    }

    /// The human tail of a row line.
    fn describe(&self) -> String {
        match self.status {
            "kept" => format!("kept: {}", self.detail.as_deref().unwrap_or("")),
            "confirmed" => format!(
                "confirmed: merged request {} matches this tip",
                self.request.as_deref().unwrap_or("")
            ),
            "unconfirmed" => format!("unconfirmed: {}", self.detail.as_deref().unwrap_or("")),
            "unknown" => format!("unknown: {}", self.detail.as_deref().unwrap_or("")),
            "deleted" => format!(
                "deleted (merged request {})",
                self.request.as_deref().unwrap_or("")
            ),
            "delete-failed" => format!("delete failed: {}", self.detail.as_deref().unwrap_or("")),
            _ => "candidate".to_owned(),
        }
    }
}

/// Dispatch the branches surface.
///
/// # Errors
///
/// Refuses when the target is not a git repository, propagates a git or
/// forge-CLI resolution failure, and — under `--apply` — returns the
/// subprocess failure of a deletion git itself refused, after the report
/// has named every branch's outcome.
pub fn run(args: &BranchesArgs) -> Result<(), RkError> {
    match &args.action {
        BranchesAction::Prune {
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

/// The whole verb: enumerate, guard, optionally confirm, optionally
/// delete, and report.
fn prune(
    target: &Utf8Path,
    repo_flag: Option<&str>,
    forge_flag: Option<&str>,
    verify: bool,
    apply: bool,
    quiet: bool,
    out: Output,
) -> Result<(), RkError> {
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory"),
            )
            .expected("an existing repository to read"),
        ));
    }
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
    let branches = crate::branches::parse_branches(&String::from_utf8_lossy(&listed.stdout));
    let current = git(target, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .filter(|answer| answer.status.success())
        .map(|answer| String::from_utf8_lossy(&answer.stdout).trim().to_owned())
        .filter(|name| !name.is_empty());
    let mut judged: Vec<(&Branch, Class)> = branches
        .iter()
        .filter_map(|branch| {
            classify(branch, current.as_deref(), TRUNK_BRANCH).map(|class| (branch, class))
        })
        .collect();

    // The forge is asked only where a candidate exists to confirm: the
    // clean path stays offline in every mode.
    if (verify || apply)
        && judged
            .iter()
            .any(|(_, class)| matches!(class, Class::Candidate))
    {
        let resolved = crate::landing::resolve(target, forge_flag, repo_flag)?;
        let forge = Forge::parse(&resolved.forge)
            .ok_or_else(|| RkError::Usage(format!("unknown forge '{}'", resolved.forge)))?;
        let repo = resolved.repo.ok_or_else(crate::landing::repo_unresolved)?;
        let cli = resolve_cli(forge)?;
        for (branch, class) in &mut judged {
            if matches!(class, Class::Candidate) {
                *class = merged_request_for(&cli, target.as_std_path(), forge, &repo, &branch.tip);
            }
        }
    }

    let mut rows: Vec<Row> = judged
        .iter()
        .map(|(branch, class)| Row::from(branch, class.clone()))
        .collect();

    let mut failed_deletes = 0usize;
    if apply {
        for row in &mut rows {
            if row.status != "confirmed" {
                continue;
            }
            let deleted = git(target, &["branch", "-D", "--", &row.name])?;
            if deleted.status.success() {
                row.status = "deleted";
            } else {
                row.status = "delete-failed";
                row.detail = Some(last_line(&deleted.stderr));
                failed_deletes += 1;
            }
        }
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
    out.emit(&Report {
        schema: "rk.branches-prune/1",
        mode,
        branches: rows,
        next,
    })?;
    if failed_deletes > 0 {
        return Err(RkError::subprocess(
            Diagnostic::new(
                Reason::SubprocessFailed,
                format!("git refused to delete {failed_deletes} confirmed branches"),
            )
            .expected("every confirmed branch deleted; the report names each outcome"),
        ));
    }
    Ok(())
}

/// What plausibly follows each mode; an apply is its own conclusion.
fn next_lines(mode: &str) -> Vec<String> {
    let verify = "rk branches prune --verify confirms each candidate against the forge";
    let apply = "rk branches prune --apply verifies, then deletes the confirmed branches";
    match mode {
        "preview" => vec![verify.to_owned(), apply.to_owned()],
        "verify" => vec![apply.to_owned()],
        _ => Vec::new(),
    }
}

/// The human report: silent under `--quiet` when nothing is reportable,
/// one judged line per gone branch otherwise, closed by who owns the
/// deletion.
fn render(out: Output, rows: &[Row], next: &[String], quiet: bool) {
    if quiet && rows.is_empty() {
        return;
    }
    if rows.is_empty() {
        out.result_line("no local branch tracks a gone remote branch");
    } else {
        out.result_line(header(rows.len()));
        let width = rows.iter().map(|row| row.name.len()).max().unwrap_or(0);
        for row in rows {
            let tip = row.tip.get(..8).unwrap_or(&row.tip);
            out.result_line(format!("  {:width$}  {tip}  {}", row.name, row.describe()));
        }
    }
    out.next(next);
    out.result_line(OPERATOR_LINE);
}

/// The count-bearing first line.
fn header(count: usize) -> String {
    if count == 1 {
        "1 local branch tracks a remote branch that is gone (a candidate, not proof):".to_owned()
    } else {
        format!(
            "{count} local branches track a remote branch that is gone (a candidate, not proof):"
        )
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
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .to_owned()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Report, Row};

    /// The complete `rk.branches-prune/1` shape, held by snapshot in both
    /// the populated and the clean forms.
    #[test]
    fn the_branches_prune_schema_snapshot_holds() {
        let populated = Report {
            schema: "rk.branches-prune/1",
            mode: "verify",
            branches: vec![
                Row {
                    name: "feat/x".into(),
                    tip: "aaaabbbbccccddddaaaabbbbccccddddaaaabbbb".into(),
                    status: "confirmed",
                    request: Some("#8".into()),
                    detail: None,
                },
                Row {
                    name: "fix/y".into(),
                    tip: "bbbbccccddddaaaabbbbccccddddaaaabbbbcccc".into(),
                    status: "kept",
                    request: None,
                    detail: Some("checked out at /wt".into()),
                },
            ],
            next: vec![
                "rk branches prune --apply verifies, then deletes the confirmed branches".into(),
            ],
        };
        assert_eq!(
            serde_json::to_string(&populated).expect("a report serializes"),
            r##"{"schema":"rk.branches-prune/1","mode":"verify","branches":[{"name":"feat/x","tip":"aaaabbbbccccddddaaaabbbbccccddddaaaabbbb","status":"confirmed","request":"#8"},{"name":"fix/y","tip":"bbbbccccddddaaaabbbbccccddddaaaabbbbcccc","status":"kept","detail":"checked out at /wt"}],"next":["rk branches prune --apply verifies, then deletes the confirmed branches"]}"##
        );
        let clean = Report {
            schema: "rk.branches-prune/1",
            mode: "preview",
            branches: vec![],
            next: vec![
                "rk branches prune --verify confirms each candidate against the forge".into(),
                "rk branches prune --apply verifies, then deletes the confirmed branches".into(),
            ],
        };
        assert_eq!(
            serde_json::to_string(&clean).expect("a report serializes"),
            r#"{"schema":"rk.branches-prune/1","mode":"preview","branches":[],"next":["rk branches prune --verify confirms each candidate against the forge","rk branches prune --apply verifies, then deletes the confirmed branches"]}"#,
            "a clean clone reports one empty list a caller can branch on"
        );
    }
}
