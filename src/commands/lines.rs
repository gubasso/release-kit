//! `rk lines`: the release-line lifecycle — inventory, open, candidates,
//! and retirement.
//!
//! A line is `release/<major>.<minor>`, a second trunk with three rules
//! this module holds: it is cut from an explicit base, never the tip by
//! default (`maintenance:a-line-is-cut-from-an-explicit-base`); its
//! candidates and releases are tags automation minted, which `rc` only
//! reads; and it retires only behind its tags, seat before branch, with
//! the remote deletion left to the operator
//! (`maintenance:a-line-is-never-retired-before-its-tags`). Every
//! mutating verb previews by default.

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::lines::{LinesAction, LinesArgs};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, Workflow};
use crate::maintenance::{self, Deletion};
use crate::output::Output;
use crate::worktree::{Worktree, parse_worktrees};

/// Route one `rk lines` invocation.
///
/// # Errors
///
/// Each verb's own refusals; every failure is typed.
pub fn run(args: &LinesArgs) -> Result<(), RkError> {
    match &args.action {
        LinesAction::List { target, json } => list(target, Output::new(*json)),
        LinesAction::Open {
            line,
            base,
            target,
            apply,
            json,
        } => open(line, base.as_deref(), target, *apply, Output::new(*json)),
        LinesAction::Rc { line, target, json } => rc(line, target, Output::new(*json)),
        LinesAction::Retire {
            line,
            target,
            apply,
            json,
        } => retire(line, target, *apply, Output::new(*json)),
    }
}

/// One line's row in the inventory.
#[derive(Debug, Serialize)]
struct LineRow {
    /// The `<major>.<minor>` name.
    line: String,
    /// The branch, `release/<line>`.
    branch: String,
    /// Where the branch exists: `local`, `remote`, or `both`.
    presence: &'static str,
    /// The newest `v<line>.*` release tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    newest_release: Option<String>,
    /// The newest `v<line>.*-rc.*` candidate tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    newest_candidate: Option<String>,
    /// Whether every commit the local branch holds beyond the trunk is
    /// reachable from a tag — what makes a retirement safe. Absent for a
    /// remote-only line, which this clone cannot judge.
    #[serde(skip_serializing_if = "Option::is_none")]
    tag_covered: Option<bool>,
    /// The worktree seating the branch, where one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    seat: Option<String>,
}

/// The machine form of `rk lines list`.
#[derive(Debug, Serialize)]
struct ListReport {
    /// The shape version of this document.
    schema: &'static str,
    /// Every line, sorted by name.
    lines: Vec<LineRow>,
}

fn list(target: &Utf8Path, out: Output) -> Result<(), RkError> {
    let local = ref_names(target, "refs/heads/release/")?;
    let remote: Vec<String> = ref_names(target, "refs/remotes/origin/release/")?
        .into_iter()
        .filter_map(|name| name.strip_prefix("origin/").map(str::to_owned))
        .collect();
    let mut names: Vec<String> = local.iter().chain(remote.iter()).cloned().collect();
    names.sort();
    names.dedup();
    let seats = inventory(target)?;
    let mut rows = Vec::new();
    for branch in names {
        let Some(line) = branch.strip_prefix("release/") else {
            continue;
        };
        let is_local = local.contains(&branch);
        let presence = match (is_local, remote.contains(&branch)) {
            (true, true) => "both",
            (true, false) => "local",
            _ => "remote",
        };
        rows.push(LineRow {
            line: line.to_owned(),
            branch: branch.clone(),
            presence,
            newest_release: newest_tag(target, line, false)?,
            newest_candidate: newest_tag(target, line, true)?,
            tag_covered: if is_local {
                Some(uncovered_commits(target, &branch)?.is_empty())
            } else {
                None
            },
            seat: seats
                .iter()
                .find(|seat| seat.branch.as_deref() == Some(branch.as_str()))
                .map(|seat| seat.path.to_string()),
        });
    }
    if rows.is_empty() {
        out.result_line("no release lines; the trunk is the only line alive");
    }
    for row in &rows {
        let mut parts = vec![format!("{} ({})", row.branch, row.presence)];
        if let Some(tag) = &row.newest_release {
            parts.push(format!("newest release {tag}"));
        }
        if let Some(tag) = &row.newest_candidate {
            parts.push(format!("newest candidate {tag}"));
        }
        match row.tag_covered {
            Some(true) => parts.push("tag-covered".to_owned()),
            Some(false) => parts.push("commits beyond the tags; not retirable".to_owned()),
            None => {}
        }
        if let Some(seat) = &row.seat {
            parts.push(format!("seated at {seat}"));
        }
        out.result_line(parts.join(" — "));
    }
    out.emit(&ListReport {
        schema: "rk.lines-list/1",
        lines: rows,
    })
}

/// The machine form of `rk lines open` in the branches mode; the worktree
/// mode delegates to `rk worktree add`, whose report is its own.
#[derive(Debug, Serialize)]
struct OpenReport {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview`, `created`, or `satisfied`.
    mode: &'static str,
    /// The branch the verb acted on.
    branch: String,
    /// What follows, in order.
    next: Vec<String>,
}

fn open(
    line: &str,
    base: Option<&str>,
    target: &Utf8Path,
    apply: bool,
    out: Output,
) -> Result<(), RkError> {
    let branch = line_branch(line)?;
    let Some(base) = base else {
        return Err(RkError::Usage(format!(
            "a line is a snapshot of a chosen commit, so {branch} takes no default base; pass --base \"v<version>\", the tag it patches"
        )));
    };
    // The worktree mode's open is the seat verb it already has: the add
    // requires a base for a release line, adopts an existing one, and
    // derives the sibling path — one implementation, one behavior.
    let workflow = manifest::load(target)
        .ok()
        .flatten()
        .map_or(Workflow::Branches, |record| record.parameters.workflow);
    if workflow == Workflow::Worktree {
        return crate::commands::worktree::run(&crate::cli::worktree::WorktreeArgs {
            action: crate::cli::worktree::WorktreeAction::Add {
                branch,
                target: target.to_owned(),
                base: Some(base.to_owned()),
                apply,
                json: out.is_json(),
            },
        });
    }
    if branch_exists(target, &branch)? {
        out.result_line(format!(
            "satisfied: {branch} already exists; the open adopts it"
        ));
        let next = vec![format!("git checkout {branch} works in it")];
        return out.emit(&OpenReport {
            schema: "rk.lines-open/1",
            mode: "satisfied",
            branch,
            next,
        });
    }
    let resolved = resolve_commit(target, base)?;
    if !apply {
        out.result_line(format!(
            "DRY RUN: would create {branch} at {base} ({resolved})"
        ));
        let next = vec![format!(
            "rk lines open {line} --base \"{base}\" --target {target} --apply"
        )];
        out.next(&next);
        return out.emit(&OpenReport {
            schema: "rk.lines-open/1",
            mode: "preview",
            branch,
            next,
        });
    }
    let created = git(target, &["branch", &branch, &resolved])?;
    if !created.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::StateDrift, last_line(&created.stderr))
                .expected("a branch git can create at the named base")
                .target_state("unchanged"),
        ));
    }
    out.result_line(format!("created {branch} at {base} ({resolved})"));
    let next = vec![
        format!("git checkout {branch} && git push -u origin {branch} publishes it"),
        "rk setup step protect-release-lines --apply protects every line, once per repository"
            .to_owned(),
    ];
    out.next(&next);
    out.emit(&OpenReport {
        schema: "rk.lines-open/1",
        mode: "created",
        branch,
        next,
    })
}

/// The machine form of `rk lines rc`.
#[derive(Debug, Serialize)]
struct RcReport {
    /// The shape version of this document.
    schema: &'static str,
    /// The line the verb read.
    line: String,
    /// The newest `v<line>.*` release tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    newest_release: Option<String>,
    /// The newest candidate tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    newest_candidate: Option<String>,
    /// The number the next candidate takes, absent while none exists yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    next_candidate: Option<u64>,
}

fn rc(line: &str, target: &Utf8Path, out: Output) -> Result<(), RkError> {
    line_branch(line)?;
    let newest_release = newest_tag(target, line, false)?;
    let newest_candidate = newest_tag(target, line, true)?;
    let next_candidate = newest_candidate
        .as_deref()
        .and_then(|tag| tag.rsplit_once("-rc.")?.1.parse::<u64>().ok())
        .map(|n| n + 1);
    match &newest_candidate {
        Some(tag) => {
            out.result_line(format!("newest candidate {tag}"));
            if let Some(next) = next_candidate {
                out.result_line(format!(
                    "a finding would mint rc.{next}; an rc number is single-use"
                ));
            }
        }
        None => out.result_line(
            "no candidate is tagged on the line yet; the line's pipeline mints one when its release path runs",
        ),
    }
    if let Some(tag) = &newest_release {
        out.result_line(format!("newest release {tag}"));
    }
    out.emit(&RcReport {
        schema: "rk.lines-rc/1",
        line: line.to_owned(),
        newest_release,
        newest_candidate,
        next_candidate,
    })
}

/// The machine form of `rk lines retire`.
#[derive(Debug, Serialize)]
struct RetireReport {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The branch the verb acted on.
    branch: String,
    /// The seat that stood — removed under apply — where one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    seat: Option<String>,
    /// What stays the operator's, in order.
    next: Vec<String>,
}

fn retire(line: &str, target: &Utf8Path, apply: bool, out: Output) -> Result<(), RkError> {
    let branch = line_branch(line)?;
    if !branch_exists(target, &branch)? {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("no local {branch} to retire"),
            )
            .expected("a local release line")
            .action(format!(
                "the remote half stays yours either way: git push origin --delete {branch}"
            ))
            .target_state("unchanged"),
        ));
    }
    // The tag gate: every commit the line holds beyond the trunk must be
    // reachable from a tag, or the deletion garbage-collects the line.
    let uncovered = uncovered_commits(target, &branch)?;
    if !uncovered.is_empty() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::DestructiveRefusal,
                format!(
                    "{branch} holds {} commit(s) no tag reaches, {} first",
                    uncovered.len(),
                    uncovered[0]
                ),
            )
            .expected("every line-only commit reachable from a tag")
            .action("tag what the line still owes — the release automation mints tags — or accept losing the commits is not offered")
            .target_state("unchanged"),
        ));
    }
    let tip = resolve_commit(target, &branch)?;
    let seats = inventory(target)?;
    let seat = seats
        .iter()
        .find(|seat| seat.branch.as_deref() == Some(branch.as_str()))
        .map(|seat| seat.path.clone());
    if !apply {
        if let Some(path) = &seat {
            out.result_line(format!(
                "would remove the seat {path}, then delete {branch}"
            ));
        } else {
            out.result_line(format!("would delete {branch} ({tip})"));
        }
        let next = vec![format!("rk lines retire {line} --target {target} --apply")];
        out.next(&next);
        return out.emit(&RetireReport {
            schema: "rk.lines-retire/1",
            mode: "preview",
            branch,
            seat: seat.map(|path| path.to_string()),
            next,
        });
    }
    // Seat before branch: a worktree holds the checkout, so the branch
    // deletion below would otherwise refuse — and git itself refuses a
    // dirty or locked seat, which is the guard this verb wants.
    if let Some(path) = &seat {
        let removed = git(target, &["worktree", "remove", path.as_str()])?;
        if !removed.status.success() {
            return Err(RkError::refusal(
                Diagnostic::new(Reason::DestructiveRefusal, last_line(&removed.stderr))
                    .expected("a clean, unlocked seat")
                    .action(format!(
                        "resolve what the seat holds, then rerun; the branch {branch} survives"
                    ))
                    .target_state("unchanged"),
            ));
        }
        out.result_line(format!("removed the seat {path}"));
    }
    match maintenance::delete_branch(target, &branch, &tip) {
        Deletion::Deleted => out.result_line(format!("deleted {branch} ({tip})")),
        Deletion::ConfigSurvived { detail } => {
            out.result_line(format!("deleted {branch} ({tip}); {detail}"));
        }
        Deletion::Refused { detail } => {
            return Err(RkError::refusal(
                Diagnostic::new(Reason::StateDrift, detail)
                    .expected("a tip that did not move after verification")
                    .target_state("the branch survives"),
            ));
        }
    }
    let next = vec![format!(
        "git push origin --delete {branch} retires the remote half; the tags keep the line recoverable"
    )];
    out.next(&next);
    out.emit(&RetireReport {
        schema: "rk.lines-retire/1",
        mode: "apply",
        branch,
        seat: seat.map(|path| path.to_string()),
        next,
    })
}

/// `release/<line>` for a well-formed `<major>.<minor>`.
fn line_branch(line: &str) -> Result<String, RkError> {
    let well_formed = line.split_once('.').is_some_and(|(major, minor)| {
        !major.is_empty()
            && !minor.is_empty()
            && major.bytes().all(|b| b.is_ascii_digit())
            && minor.bytes().all(|b| b.is_ascii_digit())
    });
    if !well_formed {
        return Err(RkError::Usage(format!(
            "'{line}' is not a line; a line is <major>.<minor>, as in 1.1"
        )));
    }
    Ok(format!("release/{line}"))
}

/// The short names under one ref prefix.
fn ref_names(target: &Utf8Path, prefix: &str) -> Result<Vec<String>, RkError> {
    let output = git(
        target,
        &["for-each-ref", "--format=%(refname:short)", prefix],
    )?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, last_line(&output.stderr))
                .expected("a git repository at the target"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// The newest `v<line>.*` tag, candidates or releases.
fn newest_tag(target: &Utf8Path, line: &str, candidates: bool) -> Result<Option<String>, RkError> {
    let pattern = format!("v{line}.*");
    let output = git(target, &["tag", "-l", &pattern, "--sort=-v:refname"])?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, last_line(&output.stderr))
                .expected("a git repository at the target"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|tag| tag.contains("-rc.") == candidates)
        .map(str::to_owned))
}

/// The commits the branch holds that no tag and no trunk ref reaches.
fn uncovered_commits(target: &Utf8Path, branch: &str) -> Result<Vec<String>, RkError> {
    let mut args = vec![
        "rev-list".to_owned(),
        branch.to_owned(),
        "--not".to_owned(),
        "--tags".to_owned(),
    ];
    for trunk in ["master", "origin/master"] {
        if resolve_commit(target, trunk).is_ok() {
            args.push(trunk.to_owned());
        }
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git(target, &arg_refs)?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::StateDrift, last_line(&output.stderr))
                .expected("a readable branch history"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Whether a local branch exists.
fn branch_exists(target: &Utf8Path, branch: &str) -> Result<bool, RkError> {
    let ref_name = format!("refs/heads/{branch}");
    Ok(
        git(target, &["show-ref", "--verify", "--quiet", &ref_name])?
            .status
            .success(),
    )
}

/// One commit-ish resolved to its commit, or a refusal naming it.
fn resolve_commit(target: &Utf8Path, name: &str) -> Result<String, RkError> {
    let spec = format!("{name}^{{commit}}");
    let output = git(target, &["rev-parse", "--verify", "--quiet", &spec])?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::Usage,
                format!("'{name}' does not resolve to a commit"),
            )
            .expected("a base git can resolve — fetch the tags first")
            .target_state("unchanged"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The worktree inventory, for the seat lookups.
fn inventory(target: &Utf8Path) -> Result<Vec<Worktree>, RkError> {
    let output = git(target, &["worktree", "list", "--porcelain", "-z"])?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, last_line(&output.stderr))
                .expected("a git repository at the target"),
        ));
    }
    parse_worktrees(&output.stdout).map_err(|detail| {
        RkError::refusal(
            Diagnostic::new(Reason::StateDrift, detail).expected("a parseable worktree inventory"),
        )
    })
}

/// Run one git command against the target, spawn failure typed.
fn git(target: &Utf8Path, args: &[&str]) -> Result<std::process::Output, RkError> {
    let mut command = std::process::Command::new("git");
    for var in maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
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
        .unwrap_or("git reported no detail")
        .trim()
        .to_owned()
}
