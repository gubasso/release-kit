//! Local-branch hygiene after merges.
//!
//! A squash merge rewrites the branch's work into one trunk commit, so the
//! branch tip never becomes an ancestor of the trunk and git's own
//! `--merged` test cannot see the merge. Once the forge deletes the remote
//! branch and a pruning fetch drops the tracking ref, `[gone]` is the one
//! local signal left — and it proves only that the upstream vanished,
//! never that it merged. A merge that keeps the commits — a local rebase
//! integration, a fast-forward merge, a hand `git merge --ff-only` —
//! leaves the tip on the trunk, and the branch's own reflog says whether
//! the tip is work or only where the branch was created. This module
//! holds the pure half of `rk branches prune`: parsing what
//! `git for-each-ref` and the reflog report, the guard order that keeps a
//! branch out of the candidate set, and the confirmation predicates that
//! turn a forge answer or a reachable tip into proof. Spawning stays in
//! the handler and in `crate::maintenance`.

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
    /// One of the three proofs covers this tip.
    Confirmed {
        /// Which proof, and what it names.
        proof: Proof,
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

/// What authorized a branch's retirement.
///
/// Exactly three proofs are admissible, and none stands in for another.
/// The first two are records, one per integration authority, and both
/// rest on the same predicate: the tip the proof recorded equals the tip
/// observed now. The third is no record at all: the trunk reaches the tip
/// observed now, and the branch moved since its reflog's creation entry,
/// so the tip is work the trunk carries rather than the commit a fresh
/// branch started at.
///
/// SATISFIES maintenance:gone-is-a-candidate-not-proof
/// SATISFIES maintenance:one-merge-proof-authorizes-both-removals
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    /// A merged request whose recorded head equals this tip.
    Request(String),
    /// A local integration whose recorded branch tip equals this tip.
    LocalIntegration(String),
    /// A trunk that reaches this tip, carrying the commit the branch's
    /// creation entry names, which differs from the tip.
    ReachableTip(String),
}

impl Proof {
    /// What the report prints beside the branch.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Request(request) => request.clone(),
            Self::LocalIntegration(commit) => {
                format!("locally integrated as {}", crate::integrate::short(commit))
            }
            Self::ReachableTip(created) => format!(
                "reachable tip: the trunk carries it, and the branch moved from {} since its creation",
                crate::integrate::short(created)
            ),
        }
    }

    /// Which proof this is, for the machine report.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Request(_) => "request",
            Self::LocalIntegration(_) => "local-integration",
            Self::ReachableTip(_) => "reachable-tip",
        }
    }

    /// Whether this proof rests on the clone alone, with no forge answer:
    /// the prune reports name these as proof rather than candidates.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::LocalIntegration(_) | Self::ReachableTip(_))
    }
}

/// The commit a branch's latest creation entry names, from the raw reflog
/// text git keeps for the ref: one `<old> <new> <identity>\t<message>`
/// line per update, oldest first.
///
/// The creation entry is the one whose old object ID is all zeros. A
/// reflog with none — expired in part, or rewritten — answers `None`, and
/// so does one that does not parse: an absent creation proves nothing,
/// which keeps the branch.
#[must_use]
pub fn creation_entry(reflog: &str) -> Option<String> {
    reflog
        .lines()
        .rev()
        .find_map(|line| {
            let mut fields = line.split(' ');
            let old = fields.next()?;
            let new = fields.next()?;
            let is_id = |id: &str| {
                matches!(id.len(), 40 | 64) && id.bytes().all(|byte| byte.is_ascii_hexdigit())
            };
            (is_id(old) && is_id(new) && old.bytes().all(|byte| byte == b'0'))
                .then(|| new.to_owned())
        })
        .filter(|new| !new.bytes().all(|byte| byte == b'0'))
}

/// The third proof, judged: the trunk reaches `tip`, and the branch's
/// creation entry names a commit other than `tip`.
///
/// A fresh branch still at the commit it was created at is new, not
/// merged, so equality is no proof; and with no creation entry there is
/// nothing to compare against, so the proof is unavailable.
#[must_use]
pub fn reachable_tip(created: Option<&str>, tip: &str, trunk_reaches: bool) -> Option<Proof> {
    let created = created?;
    (trunk_reaches && created != tip).then(|| Proof::ReachableTip(created.to_owned()))
}

/// Classify one branch: `None` when its upstream is live or unset and no
/// local proof covers it — the branch never reaches the report — and the
/// guard's verdict otherwise.
///
/// `local` is the proof the clone holds on its own: a recorded local
/// integration, or a tip the trunk reaches.
///
/// The guards run in order and the first one holds: the current branch
/// stays kept — it is the operator's own seat — a branch checked out in
/// any other worktree is worktree-bound and belongs to the worktree
/// verb, then the trunk and the release lines stay kept. What survives
/// is a candidate.
#[must_use]
pub fn classify(
    branch: &Branch,
    current: Option<&str>,
    trunk: &str,
    local: Option<Proof>,
) -> Option<Class> {
    // A locally integrated branch never reached the forge, so it has no
    // upstream to go gone. Its own evidence is what puts it in the
    // report, and a branch that advanced after its integration matches
    // no evidence and stays invisible, exactly as an advanced
    // forge-merged branch does. A branch the trunk reaches is reported
    // on the same footing.
    if !branch.gone && local.is_none() {
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
    if let Some(proof) = local {
        return Some(Class::Confirmed { proof });
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
        |request| Class::Confirmed {
            proof: Proof::Request(request),
        },
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
    use serde_json::json;

    use super::{
        Branch, Class, Proof, classify, confirmation, creation_entry, parse_branches, reachable_tip,
    };
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
            classify(&gone("feat/x", None), Some("feat/x"), "master", None),
            Some(Class::Kept {
                reason: "the current branch".into()
            })
        );
        assert_eq!(
            classify(&gone("feat/x", Some("/wt")), Some("master"), "master", None),
            Some(Class::WorktreeBound { path: "/wt".into() })
        );
        assert_eq!(
            classify(&gone("feat/x", Some("/wt")), Some("feat/x"), "master", None),
            Some(Class::Kept {
                reason: "the current branch".into()
            }),
            "the current branch wins over its own worktree"
        );
        assert_eq!(
            classify(&gone("master", None), None, "master", None),
            Some(Class::Kept {
                reason: "a protected branch".into()
            })
        );
        assert_eq!(
            classify(&gone("release/1.2", None), None, "master", None),
            Some(Class::Kept {
                reason: "a protected branch".into()
            })
        );
        assert_eq!(
            classify(&gone("feat/x", None), Some("master"), "master", None),
            Some(Class::Candidate)
        );
        let live = Branch {
            gone: false,
            ..gone("feat/live", None)
        };
        assert_eq!(classify(&live, None, "master", None), None);
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
                proof: Proof::Request("#8".into())
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
                proof: Proof::Request("!4".into())
            }
        );
        assert!(matches!(
            confirmation(Forge::Github, &json!({"message": "rate limited"}), "aaaa"),
            Class::Unknown { .. }
        ));
    }

    /// The creation entry is the one whose old object ID is all zeros, and
    /// the latest such entry answers. A reflog whose creation entry is gone
    /// — expired in part, or never written — answers nothing, and so does
    /// text that is no reflog.
    #[test]
    fn reachable_tip_reads_the_creation_entry_of_a_raw_reflog() {
        let zero = "0".repeat(40);
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let c = "c".repeat(40);
        let whole = format!(
            "{zero} {a} Dev <dev@example.com> 1700000000 +0000\tbranch: Created from master\n\
             {a} {b} Dev <dev@example.com> 1700000100 +0000\tcommit: one\n\
             {b} {c} Dev <dev@example.com> 1700000200 +0000\tcommit: two\n"
        );
        assert_eq!(creation_entry(&whole), Some(a));
        let expired = whole.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert_eq!(creation_entry(&expired), None, "the creation entry expired");
        assert_eq!(creation_entry(""), None, "no reflog at all");
        assert_eq!(creation_entry("not a reflog\n"), None);
        let sha256 = format!(
            "{} {} Dev <dev@example.com> 1700000000 +0000\tbranch: Created\n",
            "0".repeat(64),
            "d".repeat(64)
        );
        assert_eq!(creation_entry(&sha256), Some("d".repeat(64)));
    }

    /// The third proof needs all three facts: a creation entry, a tip that
    /// moved from it, and a trunk that reaches the tip. A fresh branch at
    /// its creation commit is new rather than merged.
    #[test]
    fn reachable_tip_needs_a_moved_branch_the_trunk_reaches() {
        assert_eq!(
            reachable_tip(Some("aaaa"), "bbbb", true),
            Some(Proof::ReachableTip("aaaa".into()))
        );
        assert_eq!(reachable_tip(Some("bbbb"), "bbbb", true), None, "fresh");
        assert_eq!(reachable_tip(Some("aaaa"), "bbbb", false), None, "unmerged");
        assert_eq!(reachable_tip(None, "bbbb", true), None, "no creation entry");
        let proof = Proof::ReachableTip("aaaa".into());
        assert_eq!(proof.kind(), "reachable-tip");
        assert!(proof.detail().starts_with("reachable tip"));
        assert!(proof.is_local() && Proof::LocalIntegration("x".into()).is_local());
        assert!(!Proof::Request("#8".into()).is_local());
        // The proof confirms the branch the way local evidence does, after
        // every guard.
        let branch = Branch {
            name: "feat/x".into(),
            tip: "bbbb".into(),
            upstream: None,
            gone: false,
            worktree: None,
        };
        assert_eq!(
            classify(&branch, Some("master"), "master", Some(proof.clone())),
            Some(Class::Confirmed { proof })
        );
    }
}
