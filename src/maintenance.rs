//! The shared process-side discipline of local-resource cleanup.
//!
//! `rk branches prune` and `rk worktree prune` retire the same resource
//! pair — a branch, and the worktree that seats one — so the deletion
//! discipline has one implementation here, and exactly two callers invoke
//! it: `crate::commands::branches` and `crate::commands::worktree`. The
//! module spawns git, which is why it sits beside the pure `branches` and
//! `worktree` modules rather than inside either: both declare themselves
//! parsing and classification only. The report-closing rule the two prune
//! verbs share lives here too, so the pair cannot fork.

use camino::Utf8Path;

/// The outcome of deleting one branch at a verified tip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deletion {
    /// The ref and its configuration section are gone.
    Deleted,
    /// The ref is gone and a `branch.<name>` configuration section
    /// survives; something is still owed, and the detail names the move.
    ConfigSurvived {
        /// What survived and the command that clears it.
        detail: String,
    },
    /// The compare-and-swap refused: the tip moved, or git did not run.
    Refused {
        /// Git's own reason, last line.
        detail: String,
    },
}

/// Delete one branch whose tip verification authorized, compare-and-swap.
///
/// `git update-ref -d` carries the verified tip, so a ref that moved
/// after verification is refused, never lost. A deleted ref then drops
/// its `branch.<name>` configuration section — what `git branch -d`
/// would have removed beside it — so a later branch under the reused
/// name inherits nothing stale.
#[must_use]
pub fn delete_branch(target: &Utf8Path, branch: &str, verified_tip: &str) -> Deletion {
    let ref_name = format!("refs/heads/{branch}");
    let deleted = match git(target, &["update-ref", "-d", &ref_name, verified_tip]) {
        Ok(output) => output,
        Err(detail) => return Deletion::Refused { detail },
    };
    if !deleted.status.success() {
        return Deletion::Refused {
            detail: last_line(&deleted.stderr),
        };
    }
    // A section that was never written makes the removal fail, which is
    // the common clean case; entries that survive the attempt are the
    // reportable residue.
    let section = format!("branch.{branch}");
    let survives = match git(target, &["config", "--remove-section", &section]) {
        Ok(removed) if removed.status.success() => false,
        Ok(_) => {
            // Enumerate rather than pattern-match: a branch name can carry
            // regex metacharacters, so the filter is an exact prefix test
            // over the fixed-pattern listing.
            let prefix = format!("branch.{branch}.");
            match git(target, &["config", "--get-regexp", "^branch\\."]) {
                Ok(leftover) => {
                    leftover.status.success()
                        && String::from_utf8_lossy(&leftover.stdout)
                            .lines()
                            .any(|line| line.starts_with(&prefix))
                }
                Err(_) => true,
            }
        }
        Err(_) => true,
    };
    if survives {
        return Deletion::ConfigSurvived {
            detail: format!(
                "the branch configuration survives: git config --remove-section branch.{branch}"
            ),
        };
    }
    Deletion::Deleted
}

/// Whether one report row still names a move the operator may make.
///
/// The closing operator line of both prune reports rides this predicate,
/// never the mode: a preview's candidates, every kept and judged row, and
/// every failure row owe — each failure's `detail` is required to carry
/// its recovery, which is why it owes despite the exit code — and so does
/// a `deleted` row whose detail reports surviving configuration. Done is
/// done: `deleted` with no residue and `pruned` owe nothing.
#[must_use]
pub fn row_owes(status: &str, detail: Option<&str>) -> bool {
    match status {
        "deleted" | "pruned" => detail.is_some(),
        _ => true,
    }
}

/// The variables a running git hook exports; a child inheriting them
/// would resolve the hook's repository instead of the `-C` target, so
/// every git this crate spawns against a named target scrubs them.
pub(crate) const GIT_HOOK_VARS: [&str; 4] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
];

/// Run one git command against the target; a spawn failure is the detail.
fn git(target: &Utf8Path, args: &[&str]) -> Result<std::process::Output, String> {
    let mut command = std::process::Command::new("git");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .arg("-C")
        .arg(target.as_std_path())
        .args(args)
        .output()
        .map_err(|source| format!("git did not run: {source}"))
}

/// The last non-empty stderr line, for a one-line detail.
pub(crate) fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::row_owes;

    /// The `(status, detail)` matrix behind the closing line: every row
    /// that still names a move owes, and only finished rows do not.
    #[test]
    fn a_row_owes_until_nothing_is_left_to_ask() {
        for status in [
            "candidate",
            "kept",
            "stale",
            "confirmed",
            "unconfirmed",
            "unknown",
            "worktree-bound",
            "delete-failed",
            "remove-failed",
            "branch-delete-failed",
        ] {
            assert!(row_owes(status, None), "{status} names a move");
            assert!(row_owes(status, Some("detail")), "{status} names a move");
        }
        for finished in ["deleted", "pruned"] {
            assert!(
                row_owes(finished, Some("the branch configuration survives")),
                "surviving residue is still owed"
            );
            assert!(!row_owes(finished, None), "done is done");
        }
    }
}
