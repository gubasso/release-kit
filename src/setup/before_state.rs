//! The before-state a setup step saves ahead of a change it cannot undo.
//!
//! A step that removes or replaces forge state reads what it is about to
//! change, through the same read-only calls the observers make, and
//! writes it to the run's snapshot directory before its script runs: the
//! repository settings behind the default branch and the merge settings,
//! the rulesets, each ruleset a step replaces read in full, and the
//! classic protections, and every branch
//! `single-trunk` deletes, by name and commit, with a bundle where this
//! clone holds the commit. A read that fails refuses the step, because a
//! change nobody can restore is not one this setup makes. A saved state
//! is a backup: no later run reads it to decide anything.
//!
//! SATISFIES forge-setup:a-step-is-idempotent

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use super::context::Ctx;
use super::observe::{self, Api, Runner};
use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::snapshot::{FORGE_DIR, REFS_DIR, Snapshot};

/// Every step that removes or replaces forge state.
pub const DESTRUCTIVE: [&str; 7] = [
    "default-branch",
    "single-trunk",
    "merge-cleanup",
    "protect-trunk",
    "protect-tags",
    "protect-release-lines",
    "auto-merge",
];

/// Whether `step` removes or replaces forge state.
#[must_use]
pub fn destructive(step: &str) -> bool {
    DESTRUCTIVE.contains(&step)
}

/// The read-only calls that capture what `step` changes.
#[must_use]
pub fn reads(ctx: &Ctx, step: &str) -> Vec<String> {
    let Some(forge) = ctx.forge else {
        return Vec::new();
    };
    let trunk = ctx.trunk();
    match forge {
        Forge::Github => {
            let repo = &ctx.repo;
            match step {
                "default-branch" | "merge-cleanup" | "auto-merge" => vec![format!("repos/{repo}")],
                // protect-trunk also rewrites the squash settings.
                "protect-trunk" => vec![
                    format!("repos/{repo}"),
                    format!("repos/{repo}/rulesets"),
                    format!("repos/{repo}/branches/{trunk}/protection"),
                ],
                "protect-tags" | "protect-release-lines" => vec![format!("repos/{repo}/rulesets")],
                "single-trunk" => candidates(ctx)
                    .iter()
                    .map(|branch| format!("repos/{repo}/git/ref/heads/{branch}"))
                    .collect(),
                _ => Vec::new(),
            }
        }
        Forge::Gitlab => {
            let project = ctx.repo.replace('/', "%2F");
            match step {
                "default-branch" | "merge-cleanup" | "auto-merge" => {
                    vec![format!("projects/{project}")]
                }
                // protect-trunk also rewrites the merge settings.
                "protect-trunk" => vec![
                    format!("projects/{project}"),
                    format!("projects/{project}/protected_branches"),
                ],
                "protect-release-lines" => {
                    vec![format!("projects/{project}/protected_branches")]
                }
                "protect-tags" => vec![format!("projects/{project}/protected_tags")],
                "single-trunk" => candidates(ctx)
                    .iter()
                    .map(|branch| format!("projects/{project}/repository/branches/{branch}"))
                    .collect(),
                _ => Vec::new(),
            }
        }
    }
}

/// The rulesets `step` replaces in place, by name. The listing names
/// them only, without their rules, conditions, or bypass actors, so each
/// one the step finds is read in full by its id.
fn replaced_rulesets<'c>(ctx: &'c Ctx, step: &str) -> Vec<&'c str> {
    match (ctx.forge, step) {
        (Some(Forge::Github), "protect-trunk") => vec![ctx.safety_ruleset(), ctx.trunk_ruleset()],
        (Some(Forge::Github), "protect-tags") => vec![ctx.tag_ruleset()],
        (Some(Forge::Github), "protect-release-lines") => vec![ctx.lines_ruleset()],
        _ => Vec::new(),
    }
}

/// The refusal of a step whose before-state this run cannot read.
fn cannot_read(step: &str, path: &str, reason: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::ObservationIncomplete,
            format!(
                "{step} changes forge state this run cannot read first ({path}: {reason}), so it did not run"
            ),
        )
        .expected("the state the step changes, read and saved before it changes")
        .action("rerun where the forge CLI can read this project's settings")
        .target_state("unchanged")
        .step(step),
    )
}

/// The long-lived branches `single-trunk` may delete: every retired name
/// but the trunk itself.
fn candidates(ctx: &Ctx) -> Vec<String> {
    ctx.retired_branches()
        .iter()
        .filter(|branch| branch.as_str() != ctx.trunk())
        .cloned()
        .collect()
}

/// Read and save the before-state of `step` into `snapshot`, returning
/// where it went.
///
/// # Errors
///
/// An `observation-incomplete` refusal where a read fails for a reason
/// other than a definite not-found, and the snapshot's own write
/// failures.
pub fn save(
    ctx: &Ctx,
    step: &str,
    snapshot: &Snapshot,
    run: &mut Runner,
) -> Result<PathBuf, RkError> {
    let mut answers = serde_json::Map::new();
    let mut branches = Vec::new();
    for path in reads(ctx, step) {
        let answer = match observe::api_get(ctx, run, &path)? {
            Api::Ok(body) => body,
            Api::Missing => Value::Null,
            Api::Failed(reason) => return Err(cannot_read(step, &path, &reason)),
        };
        if step == "single-trunk"
            && let Some(commit) = answer
                .pointer("/object/sha")
                .or_else(|| answer.pointer("/commit/id"))
                .and_then(Value::as_str)
        {
            let name = path.rsplit('/').next().unwrap_or_default().to_owned();
            branches.push((name, commit.to_owned()));
        }
        answers.insert(path, answer);
    }
    let listing = format!("repos/{}/rulesets", ctx.repo);
    for name in replaced_rulesets(ctx, step) {
        // The script replaces the first ruleset of that name, and creates
        // one where none is listed, which replaces nothing.
        let Some(id) = answers
            .get(&listing)
            .and_then(Value::as_array)
            .and_then(|list| list.iter().find(|ruleset| ruleset["name"] == name))
            .and_then(|ruleset| ruleset["id"].as_u64())
        else {
            continue;
        };
        let path = format!("{listing}/{id}");
        let answer = match observe::api_get(ctx, run, &path)? {
            Api::Ok(body) => body,
            // A ruleset the listing names is no absence when it reads as
            // missing, and its definition is what the step replaces.
            Api::Missing => {
                return Err(cannot_read(
                    step,
                    &path,
                    "the listed ruleset reads as missing",
                ));
            }
            Api::Failed(reason) => return Err(cannot_read(step, &path, &reason)),
        };
        answers.insert(path, answer);
    }
    let mut bundled = Vec::new();
    for (name, commit) in &branches {
        if let Some(bundle) = bundle(ctx, snapshot, name, commit)? {
            bundled.push(bundle.display().to_string());
        }
    }
    let document = json!({
        "step": step,
        "taken_at": crate::applog::now_utc(),
        "reads": answers,
        "branches": branches
            .iter()
            .map(|(name, commit)| json!({"name": name, "commit": commit}))
            .collect::<Vec<_>>(),
        "bundles": bundled,
    });
    let bytes = serde_json::to_vec_pretty(&document).map_err(anyhow::Error::from)?;
    snapshot.save(&Path::new(FORGE_DIR).join(format!("{step}.json")), &bytes)
}

/// Bundle `branch` at `commit` where this clone holds the commit; `None`
/// where it does not, in which case the saved name and commit are the
/// record, because `single-trunk` deletes only a branch the trunk
/// already contains.
fn bundle(
    ctx: &Ctx,
    snapshot: &Snapshot,
    branch: &str,
    commit: &str,
) -> Result<Option<PathBuf>, RkError> {
    let held = git(ctx)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .status()
        .is_ok_and(|status| status.success());
    if !held {
        return Ok(None);
    }
    let scratch = std::env::temp_dir().join(format!(
        "rk-bundle-{}-{}",
        std::process::id(),
        branch.replace('/', "-")
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch)?;
    let outcome = bundle_in(ctx, &scratch, snapshot, branch, commit);
    let _ = std::fs::remove_dir_all(&scratch);
    outcome
}

/// The bundle steps inside one scratch directory.
fn bundle_in(
    ctx: &Ctx,
    scratch: &Path,
    snapshot: &Snapshot,
    branch: &str,
    commit: &str,
) -> Result<Option<PathBuf>, RkError> {
    let clone = scratch.join("objects.git");
    let file = scratch.join("branch.bundle");
    // A bundle names refs, so the commit rides under the branch's own
    // name in a throwaway clone that shares the objects, never as a ref
    // of the operator's clone.
    let cloned = git(ctx)
        .args(["clone", "--quiet", "--bare", "--shared", "."])
        .arg(&clone)
        .status()
        .is_ok_and(|status| status.success());
    let named = cloned
        && bare(&clone)
            .args(["update-ref", &format!("refs/heads/{branch}"), commit])
            .status()
            .is_ok_and(|status| status.success());
    let bundled = named
        && bare(&clone)
            .args(["bundle", "create", "--quiet"])
            .arg(&file)
            .arg(format!("refs/heads/{branch}"))
            .status()
            .is_ok_and(|status| status.success());
    if !bundled {
        return Ok(None);
    }
    let bytes = std::fs::read(&file)?;
    snapshot
        .save(
            &Path::new(REFS_DIR).join(format!("{branch}.bundle")),
            &bytes,
        )
        .map(Some)
}

/// A git command against the throwaway clone.
fn bare(clone: &Path) -> Command {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command.arg("-C").arg(clone);
    command
}

/// A git command against the target alone.
fn git(ctx: &Ctx) -> Command {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command.arg("-C").arg(ctx.target.as_std_path());
    command
}

#[cfg(test)]
mod tests {
    use super::{DESTRUCTIVE, destructive};

    #[test]
    fn the_destructive_steps_are_named_once() {
        for step in DESTRUCTIVE {
            assert!(destructive(step));
        }
        for step in ["package-check", "install-bot", "protections-check"] {
            assert!(!destructive(step), "{step}");
        }
    }
}
