//! Local-branch hygiene after squash merges.
//!
//! A squash merge rewrites the branch's work into one trunk commit, so the
//! branch tip never becomes an ancestor of the trunk and git's own
//! `--merged` test cannot see the merge. Once the forge deletes the remote
//! branch and a pruning fetch drops the tracking ref, `[gone]` is the one
//! local signal left — and it proves only that the upstream vanished,
//! never that it merged. This module holds the pure half of `rk branches
//! prune`: parsing what `git for-each-ref` reports, the guard order that
//! keeps a branch out of the candidate set, and the confirmation predicate
//! that turns a forge answer into proof. Spawning stays in the handler.

use std::path::Path;

use serde_json::Value;

use crate::detect::Forge;

/// The prefix naming a release line; a branch under it is never a
/// candidate, whatever its upstream says.
pub const PROTECTED_PREFIX: &str = "release/";

/// The `--format` string the handler passes to `git for-each-ref`, one
/// tab-separated line per local branch.
///
/// `%(upstream:track)` renders
/// `[gone]` verbatim in format strings — plumbing, not the localized
/// porcelain of `git branch -vv` — and `%(worktreepath)` is non-empty for
/// a branch checked out in any worktree, the main one included.
pub const FOR_EACH_REF_FORMAT: &str =
    "%(refname:short)%09%(objectname)%09%(upstream:short)%09%(upstream:track)%09%(worktreepath)";

/// One local branch, as `git for-each-ref` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    /// The short ref name.
    pub name: String,
    /// The full object name at the tip.
    pub tip: String,
    /// The configured upstream's short name, where one is configured.
    pub upstream: Option<String>,
    /// Whether the configured upstream no longer exists.
    pub gone: bool,
    /// The worktree the branch is checked out in, where it is.
    pub worktree: Option<String>,
}

/// Parse the tab-separated `for-each-ref` output into branches, skipping
/// any line that does not carry all five fields.
#[must_use]
pub fn parse_branches(text: &str) -> Vec<Branch> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(5, '\t');
            let name = fields.next()?.to_owned();
            let tip = fields.next()?.to_owned();
            let upstream = fields.next()?;
            let track = fields.next()?;
            let worktree = fields.next()?;
            Some(Branch {
                name,
                tip,
                upstream: (!upstream.is_empty()).then(|| upstream.to_owned()),
                gone: track == "[gone]",
                worktree: (!worktree.is_empty()).then(|| worktree.to_owned()),
            })
        })
        .collect()
}

/// What the report says about one gone branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// Guarded out of the candidate set, with the guard's reason.
    Kept {
        /// Why the branch stays.
        reason: String,
    },
    /// Gone upstream and unguarded: a candidate, not proof.
    Candidate,
    /// Checked out in a worktree: the worktree owns the cleanup, and
    /// the worktree verb is the one that performs it.
    WorktreeBound {
        /// The worktree's path, as `for-each-ref` reports it.
        path: String,
    },
    /// A merged request's recorded head equals this tip.
    Confirmed {
        /// The request, as the forge names it: `#N` or `!N`.
        request: String,
    },
    /// The forge answered and no merged request records this tip.
    Unconfirmed {
        /// What the answer lacked.
        detail: String,
    },
    /// The forge could not answer; an apply keeps the branch.
    Unknown {
        /// Why the answer is missing.
        detail: String,
    },
}

/// Classify one branch: `None` when its upstream is live or unset — the
/// branch never reaches the report — and the guard's verdict otherwise.
///
/// The guards run in order and the first one holds: the current branch
/// stays kept — it is the operator's own seat — a branch checked out in
/// any other worktree is worktree-bound and belongs to the worktree
/// verb, then the trunk and the release lines stay kept. What survives
/// is a candidate.
#[must_use]
pub fn classify(branch: &Branch, current: Option<&str>, trunk: &str) -> Option<Class> {
    if !branch.gone {
        return None;
    }
    if current.is_some_and(|name| name == branch.name) {
        return Some(Class::Kept {
            reason: "the current branch".to_owned(),
        });
    }
    if let Some(worktree) = &branch.worktree {
        return Some(Class::WorktreeBound {
            path: worktree.clone(),
        });
    }
    if branch.name == trunk || branch.name.starts_with(PROTECTED_PREFIX) {
        return Some(Class::Kept {
            reason: "a protected branch".to_owned(),
        });
    }
    Some(Class::Candidate)
}

/// Judge one forge answer against one tip: only a merged request whose
/// recorded head equals the local tip confirms, because a squash merge
/// destroys the ancestry every other proof would rest on.
///
/// A branch
/// advanced after its merge, or one whose upstream was deleted by hand,
/// matches nothing and stays.
#[must_use]
pub fn confirmation(forge: Forge, body: &Value, tip: &str) -> Class {
    let Some(requests) = body.as_array() else {
        return Class::Unknown {
            detail: "the forge answer is not a list of requests".to_owned(),
        };
    };
    let confirmed = requests.iter().find_map(|request| match forge {
        Forge::Github => (!request["merged_at"].is_null()
            && request["head"]["sha"].as_str() == Some(tip))
        .then(|| request["number"].as_u64())
        .flatten()
        .map(|number| format!("#{number}")),
        Forge::Gitlab => (request["state"].as_str() == Some("merged")
            && request["sha"].as_str() == Some(tip))
        .then(|| request["iid"].as_u64())
        .flatten()
        .map(|iid| format!("!{iid}")),
    });
    confirmed.map_or_else(
        || Class::Unconfirmed {
            detail: "no merged request records this tip".to_owned(),
        },
        |request| Class::Confirmed { request },
    )
}

/// Ask the forge for the requests carrying one commit and judge the
/// answer.
///
/// Every failure keeps the branch: a spawn error or an
/// unclassifiable exit is `Unknown`, and a 404 — a tip the forge never
/// saw — is `Unconfirmed`.
#[must_use]
pub fn merged_request_for(cli: &Path, target: &Path, forge: Forge, repo: &str, tip: &str) -> Class {
    let path = match forge {
        Forge::Github => format!("repos/{repo}/commits/{tip}/pulls"),
        Forge::Gitlab => format!(
            "projects/{}/repository/commits/{tip}/merge_requests",
            repo.replace('/', "%2F")
        ),
    };
    let answered = std::process::Command::new(cli)
        .args(["api", &path])
        .current_dir(target)
        .env("GH_PAGER", "")
        .env("GLAB_PAGER", "")
        .output();
    let output = match answered {
        Ok(output) => output,
        Err(source) => {
            return Class::Unknown {
                detail: format!("the forge CLI did not run: {source}"),
            };
        }
    };
    if output.status.success() {
        return serde_json::from_slice::<Value>(&output.stdout).map_or_else(
            |_| Class::Unknown {
                detail: "the forge answer did not parse as JSON".to_owned(),
            },
            |body| confirmation(forge, &body, tip),
        );
    }
    // A definite not-found is proof of absence; anything less specific
    // stays unknown. `gh` renders "HTTP 404", `glab` "404 Not Found" -
    // a bare substring would read an outage message mentioning 404 as
    // an answer.
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("HTTP 404") || stderr.contains("404 Not Found") {
        return Class::Unconfirmed {
            detail: "the forge does not know this commit".to_owned(),
        };
    }
    Class::Unknown {
        detail: last_line(&output.stderr),
    }
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

    use serde_json::json;

    use super::{Branch, Class, classify, confirmation, parse_branches};
    use crate::detect::Forge;

    /// The five tab-separated fields parse, empty ones to `None`, and only
    /// the literal `[gone]` marks a branch gone.
    #[test]
    fn a_for_each_ref_line_parses_into_a_branch() {
        let text = "feat/x\taaaa\torigin/feat/x\t[gone]\t\n\
                    master\tbbbb\torigin/master\t\t/srv/checkouts/repo\n\
                    local-only\tcccc\t\t\t\n\
                    behind\tdddd\torigin/behind\t[behind 2]\t\n\
                    short\tline\n";
        let branches = parse_branches(text);
        assert_eq!(branches.len(), 4, "the short line is skipped");
        assert_eq!(
            branches[0],
            Branch {
                name: "feat/x".into(),
                tip: "aaaa".into(),
                upstream: Some("origin/feat/x".into()),
                gone: true,
                worktree: None,
            }
        );
        assert_eq!(branches[1].worktree.as_deref(), Some("/srv/checkouts/repo"));
        assert!(!branches[1].gone);
        assert_eq!(branches[2].upstream, None);
        assert!(!branches[3].gone, "[behind 2] is tracking, not gone");
    }

    /// The guards hold in order — current, worktree, trunk, release line —
    /// and a live or upstreamless branch never reaches the report.
    #[test]
    fn classification_guards_current_worktree_and_protected_branches() {
        let gone = |name: &str, worktree: Option<&str>| Branch {
            name: name.into(),
            tip: "aaaa".into(),
            upstream: Some(format!("origin/{name}")),
            gone: true,
            worktree: worktree.map(str::to_owned),
        };
        assert_eq!(
            classify(&gone("feat/x", None), Some("feat/x"), "master"),
            Some(Class::Kept {
                reason: "the current branch".into()
            })
        );
        assert_eq!(
            classify(&gone("feat/x", Some("/wt")), Some("master"), "master"),
            Some(Class::WorktreeBound { path: "/wt".into() })
        );
        assert_eq!(
            classify(&gone("feat/x", Some("/wt")), Some("feat/x"), "master"),
            Some(Class::Kept {
                reason: "the current branch".into()
            }),
            "the current branch wins over its own worktree"
        );
        assert_eq!(
            classify(&gone("master", None), None, "master"),
            Some(Class::Kept {
                reason: "a protected branch".into()
            })
        );
        assert_eq!(
            classify(&gone("release/1.2", None), None, "master"),
            Some(Class::Kept {
                reason: "a protected branch".into()
            })
        );
        assert_eq!(
            classify(&gone("feat/x", None), Some("master"), "master"),
            Some(Class::Candidate)
        );
        let live = Branch {
            gone: false,
            ..gone("feat/live", None)
        };
        assert_eq!(classify(&live, None, "master"), None);
    }

    /// Only a merged request whose recorded head equals the tip confirms;
    /// an open request, a mismatched head, and a non-list answer never do.
    #[test]
    fn a_merged_request_confirms_only_on_head_sha_equality() {
        let github = json!([
            {"number": 7, "merged_at": null, "head": {"sha": "aaaa"}},
            {"number": 8, "merged_at": "2026-01-01T00:00:00Z", "head": {"sha": "aaaa"}},
        ]);
        assert_eq!(
            confirmation(Forge::Github, &github, "aaaa"),
            Class::Confirmed {
                request: "#8".into()
            }
        );
        assert_eq!(
            confirmation(Forge::Github, &github, "bbbb"),
            Class::Unconfirmed {
                detail: "no merged request records this tip".into()
            },
            "a merged request for another tip proves nothing about this one"
        );
        let gitlab = json!([
            {"iid": 3, "state": "opened", "sha": "aaaa"},
            {"iid": 4, "state": "merged", "sha": "aaaa"},
        ]);
        assert_eq!(
            confirmation(Forge::Gitlab, &gitlab, "aaaa"),
            Class::Confirmed {
                request: "!4".into()
            }
        );
        assert!(matches!(
            confirmation(Forge::Github, &json!({"message": "rate limited"}), "aaaa"),
            Class::Unknown { .. }
        ));
    }
}
