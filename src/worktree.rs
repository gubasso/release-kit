//! Worktree hygiene after squash merges: the pure half.
//!
//! A linked worktree seats one branch, and the forge's squash merge
//! retires that branch the same way it retires a bare one — so the
//! worktrees need the same post-merge cleanup, resting on the same
//! merged-request proof. This module holds the pure half of the
//! `rk worktree` family: the sibling-path derivation, the fail-closed
//! parser over `git worktree list --porcelain -z`, and the guard order
//! that keeps a worktree out of the candidate set. Spawning stays in the
//! handler, exactly as `crate::branches` declares for the branch half.

use camino::{Utf8Path, Utf8PathBuf};

use crate::branches::{Branch, Class, PROTECTED_PREFIX};

/// The Conventional Commit types the branch grammar's first form admits,
/// mirroring [`crate::landing::BRANCH_GRAMMAR`]'s alternation.
const BRANCH_TYPES: [&str; 11] = [
    "build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test",
];

/// Whether a branch name matches the landed grammar.
///
/// The same anchored
/// language [`crate::landing::BRANCH_GRAMMAR`] states as an extended
/// regular expression, hand-rolled here because the convention admits no
/// regex dependency for one pattern. Necessary, not sufficient: it admits
/// names git itself refuses, so `rk worktree add` follows it with
/// `git check-ref-format --branch`.
#[must_use]
pub fn matches_grammar(branch: &str) -> bool {
    // release[-/].+ — any non-empty remainder, as the regex dot admits.
    if let Some(rest) = branch.strip_prefix("release") {
        if let Some(line) = rest.strip_prefix(['-', '/']) {
            if !line.is_empty() {
                return true;
            }
        }
    }
    // <type>/<slug> with the slug over [A-Za-z0-9._/-]+.
    if let Some((kind, slug)) = branch.split_once('/') {
        if BRANCH_TYPES.contains(&kind)
            && !slug.is_empty()
            && slug
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
        {
            return true;
        }
    }
    issue_form(branch)
}

/// The issue-linked form: `([0-9]+|[A-Z][A-Z0-9]+-[0-9]+)-<slug>` with
/// the slug over `[A-Za-z0-9._-]+`.
fn issue_form(branch: &str) -> bool {
    let slug_ok = |slug: &str| {
        !slug.is_empty()
            && slug
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    // [0-9]+-<slug>: the digit run stops at the first non-digit, which
    // must be the separating hyphen — the classes are disjoint there, so
    // maximal munch is exact.
    let digits = branch
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(branch.len());
    if digits >= 1 {
        if let Some(slug) = branch[digits..].strip_prefix('-') {
            if slug_ok(slug) {
                return true;
            }
        }
    }
    // [A-Z][A-Z0-9]+-[0-9]+-<slug>.
    if !branch.starts_with(|c: char| c.is_ascii_uppercase()) {
        return false;
    }
    let key = branch[1..]
        .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit()))
        .map_or(branch.len(), |offset| offset + 1);
    if key < 2 {
        return false;
    }
    let Some(rest) = branch[key..].strip_prefix('-') else {
        return false;
    };
    let number = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if number < 1 {
        return false;
    }
    rest[number..].strip_prefix('-').is_some_and(slug_ok)
}

/// The branch name flattened for a directory: every `/` becomes `-`.
///
/// Not injective — `feat/a-b` and `feat-a/b` collide — so every caller
/// that creates checks for collision and refuses; none suffixes silently.
#[must_use]
pub fn flatten(branch: &str) -> String {
    branch.replace('/', "-")
}

/// The repository's layout: the main worktree's path, its parent, and
/// its basename as the project name the sibling paths compose with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The main worktree's path.
    pub main: Utf8PathBuf,
    /// The directory the sibling worktrees land in.
    pub parent: Utf8PathBuf,
    /// The main worktree's basename, the project half of a sibling name.
    pub project: String,
}

impl Layout {
    /// The layout of a parsed inventory: the first record is the main
    /// worktree — git documents the ordering — and [`parse_worktrees`]
    /// already refused an inventory whose first record is not one.
    ///
    /// # Errors
    ///
    /// The detail of a main worktree the sibling convention cannot
    /// compose with: no parent directory, or no basename.
    pub fn of(worktrees: &[Worktree]) -> Result<Self, String> {
        let main = worktrees
            .first()
            .ok_or_else(|| "the worktree inventory is empty".to_owned())?;
        let parent = main
            .path
            .parent()
            .ok_or_else(|| format!("the main worktree {} has no parent directory", main.path))?
            .to_owned();
        let project = main
            .path
            .file_name()
            .ok_or_else(|| format!("the main worktree {} has no basename", main.path))?
            .to_owned();
        Ok(Self {
            main: main.path.clone(),
            parent,
            project,
        })
    }
}

/// The canonical worktree path for a branch: `<parent>/<project>-<flat>`.
#[must_use]
pub fn derived_path(layout: &Layout, branch: &str) -> Utf8PathBuf {
    layout
        .parent
        .join(format!("{}-{}", layout.project, flatten(branch)))
}

/// One worktree as `git worktree list --porcelain -z` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    /// The worktree's path.
    pub path: Utf8PathBuf,
    /// The full object name at HEAD.
    pub head: String,
    /// The checked-out branch's short name; `None` when detached.
    pub branch: Option<String>,
    /// Whether the record is the bare repository itself.
    pub bare: bool,
    /// The lock reason, where locked (empty string for a bare lock).
    pub locked: Option<String>,
    /// Git's own prunable note, where the directory is missing.
    pub prunable: Option<String>,
}

/// One record under construction, folded attribute by attribute.
#[derive(Debug, Default)]
struct Partial {
    path: Option<Utf8PathBuf>,
    head: Option<String>,
    branch: Option<String>,
    bare: bool,
    detached: bool,
    locked: Option<String>,
    prunable: Option<String>,
}

impl Partial {
    const fn is_empty(&self) -> bool {
        self.path.is_none()
            && self.head.is_none()
            && self.branch.is_none()
            && !self.bare
            && !self.detached
            && self.locked.is_none()
            && self.prunable.is_none()
    }

    /// Close one record: every required attribute present, or the reason.
    fn close(self) -> Result<Worktree, String> {
        let path = self
            .path
            .ok_or_else(|| "a worktree record carries no path".to_owned())?;
        // A bare record carries no HEAD; every checked-out worktree does.
        let head = match (self.head, self.bare) {
            (Some(head), _) => head,
            (None, true) => String::new(),
            (None, false) => return Err(format!("the record for {path} carries no HEAD")),
        };
        if !self.bare && self.branch.is_none() && !self.detached {
            return Err(format!(
                "the record for {path} names neither a branch nor a detached HEAD"
            ));
        }
        Ok(Worktree {
            path,
            head,
            branch: self.branch,
            bare: self.bare,
            locked: self.locked,
            prunable: self.prunable,
        })
    }
}

/// Parse `git worktree list --porcelain -z`.
///
/// NUL-terminated attribute
/// lines, an empty token closing each record, the attributes `worktree`,
/// `HEAD`, `branch refs/heads/<name>` (shortened here), `bare`,
/// `detached`, `locked [reason]`, and `prunable [reason]`.
///
/// # Errors
///
/// The detail of what could not be trusted: a first record that is not a
/// complete main worktree, a record missing its required attributes, an
/// unknown attribute shape, or a path that is not UTF-8 — each refuses
/// the whole inventory before any verb acts on a partial one. A bare
/// main record is refused by name: the sibling convention has no parent
/// checkout to compose with, and no verb here operates on a bare
/// repository. Destructive verbs sit on this parser, and nothing ever
/// inspects `.git/worktrees/` directly; this is the one reader.
pub fn parse_worktrees(bytes: &[u8]) -> Result<Vec<Worktree>, String> {
    let mut worktrees = Vec::new();
    let mut partial = Partial::default();
    for token in bytes.split(|byte| *byte == 0) {
        if token.is_empty() {
            if !partial.is_empty() {
                worktrees.push(std::mem::take(&mut partial).close()?);
            }
            continue;
        }
        let line = std::str::from_utf8(token)
            .map_err(|_| "a worktree record carries a path that is not UTF-8".to_owned())?;
        let (attribute, value) = line
            .split_once(' ')
            .map_or((line, None), |(attribute, value)| (attribute, Some(value)));
        match (attribute, value) {
            ("worktree", Some(path)) => partial.path = Some(Utf8PathBuf::from(path)),
            ("HEAD", Some(head)) => partial.head = Some(head.to_owned()),
            ("branch", Some(reference)) => {
                partial.branch = Some(
                    reference
                        .strip_prefix("refs/heads/")
                        .unwrap_or(reference)
                        .to_owned(),
                );
            }
            ("bare", None) => partial.bare = true,
            ("detached", None) => partial.detached = true,
            ("locked", reason) => partial.locked = Some(reason.unwrap_or("").to_owned()),
            ("prunable", reason) => partial.prunable = Some(reason.unwrap_or("").to_owned()),
            _ => {
                return Err(format!(
                    "the worktree inventory carries an attribute this binary does not know: {line}"
                ));
            }
        }
    }
    if !partial.is_empty() {
        // A truncated stream: the last record never closed.
        return Err("the worktree inventory ends mid-record".to_owned());
    }
    let Some(main) = worktrees.first() else {
        return Err("the worktree inventory is empty".to_owned());
    };
    if main.bare {
        return Err(
            "the repository is bare; the sibling convention has no main checkout to compose with"
                .to_owned(),
        );
    }
    if main.prunable.is_some() {
        return Err(format!(
            "the first record, {}, is not a complete main worktree",
            main.path
        ));
    }
    Ok(worktrees)
}

/// What `rk worktree prune` says about one linked worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WtClass {
    /// Guarded out, with the reason: the main checkout, a seat in use,
    /// locked, detached, a protected branch, dirty, or a live upstream.
    Kept {
        /// Why the worktree stays.
        reason: String,
    },
    /// Its branch's upstream is gone and no guard held: a candidate.
    Candidate,
    /// Confirmed / Unconfirmed / Unknown — the judgments from
    /// [`crate::branches::Class`], produced by the same predicate.
    Judged(Class),
    /// A registered record whose directory is missing and which is not
    /// locked: `git worktree prune --expire now` territory, never a
    /// removal.
    Stale,
}

/// Classify one worktree for the prune report.
///
/// The guards run in order
/// and the first one holds; the order is load-bearing — a missing
/// directory takes no `status` call and is commonly also detached, so the
/// stale arm precedes the detached one by construction, and a lock is
/// kept unconditionally, missing directory included. The caller applies
/// this within the reportable set (stale records and gone-upstream
/// worktrees); the main-worktree and live-upstream arms stay as
/// belt-and-braces for a caller that hands it anything else.
///
/// `seats` are the paths whose worktrees are in use — the caller's own
/// seat and the target's current worktree, both, independently. `dirty`
/// is the handler's `git status --porcelain` probe, run only for a
/// worktree whose directory exists; untracked files count.
#[must_use]
pub fn classify(
    worktree: &Worktree,
    branch: Option<&Branch>,
    layout: &Layout,
    seats: &[&Utf8Path],
    trunk: &str,
    dirty: bool,
) -> WtClass {
    if worktree.path == layout.main {
        return WtClass::Kept {
            reason: "the main checkout".to_owned(),
        };
    }
    if seats.iter().any(|seat| **seat == worktree.path) {
        return WtClass::Kept {
            reason: "a seat in use".to_owned(),
        };
    }
    if let Some(reason) = &worktree.locked {
        return WtClass::Kept {
            reason: if reason.is_empty() {
                "locked".to_owned()
            } else {
                format!("locked: {reason}")
            },
        };
    }
    if worktree.prunable.is_some() {
        return WtClass::Stale;
    }
    let Some(name) = &worktree.branch else {
        return WtClass::Kept {
            reason: "detached HEAD".to_owned(),
        };
    };
    if name == trunk || name.starts_with(PROTECTED_PREFIX) {
        return WtClass::Kept {
            reason: "a protected branch".to_owned(),
        };
    }
    // The join fails closed, and before the state probes: a worktree
    // whose branch observation is missing is never guessed into a
    // candidate, and its dirt reading is noise — a seat whose ref
    // vanished reads unborn.
    let Some(branch) = branch else {
        return WtClass::Kept {
            reason: format!("no branch observation covers {name}"),
        };
    };
    if dirty {
        return WtClass::Kept {
            reason: "uncommitted changes".to_owned(),
        };
    }
    if !branch.gone {
        return WtClass::Kept {
            reason: "the upstream is live or unset".to_owned(),
        };
    }
    WtClass::Candidate
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use camino::{Utf8Path, Utf8PathBuf};

    use super::{Layout, Worktree, WtClass, classify, derived_path, flatten, parse_worktrees};
    use crate::branches::Branch;

    /// The hand-rolled matcher speaks the one grammar: on a spread of
    /// admitted and refused names it agrees with `grep -E` over
    /// [`crate::landing::BRANCH_GRAMMAR`], the const the hook block
    /// renders — so the two validators cannot drift apart silently.
    #[test]
    fn the_matcher_agrees_with_the_one_branch_grammar() {
        let cases = [
            ("feat/oauth-login", true),
            ("fix/PROJ-412-empty-csv", true),
            ("guides/release", false),
            ("chore/deps/bump", true),
            ("feat/", false),
            ("412-empty-csv", true),
            ("PROJ-412-empty-csv", true),
            ("A-1-x", false),
            ("AB-1-x", true),
            ("412-", false),
            ("release/1.2", true),
            ("release-1.2", true),
            ("release-", false),
            ("release", false),
            ("master", false),
            ("worktree-session", false),
            ("feature/x", false),
            ("123", false),
        ];
        for (name, expected) in cases {
            assert_eq!(
                super::matches_grammar(name),
                expected,
                "matcher disagrees on {name}"
            );
            let grepped = std::process::Command::new("sh")
                .args([
                    "-c",
                    &format!(
                        "printf %s \"$1\" | grep -Eq \"{}\"",
                        crate::landing::BRANCH_GRAMMAR
                    ),
                    "sh",
                    name,
                ])
                .status()
                .expect("grep runs");
            assert_eq!(
                grepped.success(),
                expected,
                "the regex itself disagrees on {name}"
            );
        }
    }

    /// Flattening replaces every slash; the collision pair derives equal —
    /// documented, refused at `add`, never suffixed.
    #[test]
    fn a_branch_flattens_into_a_sibling_directory_name() {
        assert_eq!(flatten("feat/oauth-login"), "feat-oauth-login");
        assert_eq!(flatten("guides/release/x"), "guides-release-x");
        assert_eq!(flatten("plain"), "plain");
        assert_eq!(
            flatten("feat/a-b"),
            flatten("feat-a/b"),
            "flattening is not injective; add refuses the collision by name"
        );
        let layout = Layout {
            main: Utf8PathBuf::from("/srv/checkouts/widget"),
            parent: Utf8PathBuf::from("/srv/checkouts"),
            project: "widget".into(),
        };
        assert_eq!(
            derived_path(&layout, "feat/oauth-login"),
            Utf8PathBuf::from("/srv/checkouts/widget-feat-oauth-login")
        );
    }

    /// A porcelain stream, NUL-separated, with an empty token closing each
    /// record.
    fn stream(records: &[&[&str]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for record in records {
            for line in *record {
                bytes.extend_from_slice(line.as_bytes());
                bytes.push(0);
            }
            bytes.push(0);
        }
        bytes
    }

    /// Complete records parse — main, linked, detached, locked with a
    /// reason, prunable — and each untrustworthy shape refuses with the
    /// reason named.
    #[test]
    fn porcelain_parsing_refuses_what_it_cannot_trust() {
        let parsed = parse_worktrees(&stream(&[
            &[
                "worktree /srv/checkouts/widget",
                "HEAD aaaa",
                "branch refs/heads/master",
            ],
            &[
                "worktree /srv/checkouts/widget-feat-x",
                "HEAD bbbb",
                "branch refs/heads/feat/x",
            ],
            &[
                "worktree /srv/checkouts/widget-probe",
                "HEAD cccc",
                "detached",
            ],
            &[
                "worktree /srv/checkouts/widget-held",
                "HEAD dddd",
                "branch refs/heads/feat/held",
                "locked a running agent",
            ],
            &[
                "worktree /srv/checkouts/widget-gone",
                "HEAD eeee",
                "branch refs/heads/feat/gone",
                "prunable gitdir file points to non-existent location",
            ],
        ]))
        .expect("a complete inventory parses");
        assert_eq!(parsed.len(), 5);
        assert_eq!(parsed[0].branch.as_deref(), Some("master"));
        assert_eq!(parsed[1].branch.as_deref(), Some("feat/x"));
        assert_eq!(parsed[2].branch, None);
        assert_eq!(parsed[3].locked.as_deref(), Some("a running agent"));
        assert!(parsed[4].prunable.is_some());
        let layout = Layout::of(&parsed).expect("the layout resolves");
        assert_eq!(layout.parent, Utf8PathBuf::from("/srv/checkouts"));
        assert_eq!(layout.project, "widget");

        let truncated = stream(&[&["worktree /srv/checkouts/widget", "HEAD aaaa"]]);
        let truncated = &truncated[..truncated.len() - 2];
        assert!(
            parse_worktrees(truncated)
                .expect_err("a truncated stream refuses")
                .contains("mid-record")
        );
        assert!(
            parse_worktrees(&stream(&[&["worktree /srv/x", "branch refs/heads/master"]]))
                .expect_err("a record without a HEAD refuses")
                .contains("no HEAD")
        );
        assert!(
            parse_worktrees(&stream(&[&["worktree /srv/x", "HEAD aaaa"]]))
                .expect_err("neither branch nor detached refuses")
                .contains("neither a branch nor a detached HEAD")
        );
        assert!(
            parse_worktrees(&stream(&[&["worktree /srv/x", "HEAD aaaa", "gitdir /y"]]))
                .expect_err("an unknown attribute refuses")
                .contains("does not know")
        );
        assert!(
            parse_worktrees(&stream(&[&["worktree /srv/bare.git", "bare"]]))
                .expect_err("a bare main record refuses by name")
                .contains("bare")
        );
        assert!(
            parse_worktrees(&stream(&[&[
                "worktree /srv/x",
                "HEAD aaaa",
                "branch refs/heads/x",
                "prunable gone",
            ]]))
            .expect_err("a prunable first record is no main worktree")
            .contains("main worktree")
        );
        let mut invalid = b"worktree /srv/\xff\0HEAD aaaa\0branch refs/heads/x\0\0".to_vec();
        assert!(
            parse_worktrees(&invalid)
                .expect_err("a non-UTF-8 path refuses")
                .contains("not UTF-8")
        );
        invalid.clear();
        assert!(
            parse_worktrees(&invalid).is_err(),
            "an empty inventory refuses"
        );
    }

    fn fixture(path: &str, branch: Option<&str>) -> Worktree {
        Worktree {
            path: Utf8PathBuf::from(path),
            head: "aaaa".into(),
            branch: branch.map(str::to_owned),
            bare: false,
            locked: None,
            prunable: None,
        }
    }

    fn observation(name: &str, gone: bool) -> Branch {
        Branch {
            name: name.into(),
            tip: "aaaa".into(),
            upstream: Some(format!("origin/{name}")),
            gone,
            worktree: None,
        }
    }

    /// The nine guards hold in order: main, seat, locked (missing
    /// directory included), stale before detached, detached, protected,
    /// dirty, live upstream, candidate.
    #[test]
    fn classification_guards_hold_in_order() {
        let layout = Layout {
            main: Utf8PathBuf::from("/srv/widget"),
            parent: Utf8PathBuf::from("/srv"),
            project: "widget".into(),
        };
        let seat = Utf8Path::new("/srv/widget-feat-seat");
        let seats: &[&Utf8Path] = &[seat];
        let gone = observation("feat/x", true);
        let keep = |worktree: &Worktree, branch: Option<&Branch>, dirty: bool| {
            classify(worktree, branch, &layout, seats, "master", dirty)
        };

        assert_eq!(
            keep(&fixture("/srv/widget", Some("master")), None, false),
            WtClass::Kept {
                reason: "the main checkout".into()
            }
        );
        assert_eq!(
            keep(
                &fixture("/srv/widget-feat-seat", Some("feat/x")),
                Some(&gone),
                false
            ),
            WtClass::Kept {
                reason: "a seat in use".into()
            }
        );
        let locked_missing = Worktree {
            locked: Some(String::new()),
            prunable: Some("gone".into()),
            ..fixture("/srv/widget-feat-x", Some("feat/x"))
        };
        assert_eq!(
            keep(&locked_missing, Some(&gone), false),
            WtClass::Kept {
                reason: "locked".into()
            },
            "a lock is kept unconditionally, missing directory included"
        );
        let stale_detached = Worktree {
            prunable: Some("gone".into()),
            ..fixture("/srv/widget-feat-x", None)
        };
        assert_eq!(
            keep(&stale_detached, None, false),
            WtClass::Stale,
            "a missing directory precedes the detached arm by construction"
        );
        assert_eq!(
            keep(&fixture("/srv/widget-probe", None), None, false),
            WtClass::Kept {
                reason: "detached HEAD".into()
            }
        );
        assert_eq!(
            keep(
                &fixture("/srv/widget-release-1.2", Some("release/1.2")),
                Some(&observation("release/1.2", true)),
                false
            ),
            WtClass::Kept {
                reason: "a protected branch".into()
            }
        );
        assert_eq!(
            keep(
                &fixture("/srv/widget-feat-x", Some("feat/x")),
                Some(&gone),
                true
            ),
            WtClass::Kept {
                reason: "uncommitted changes".into()
            }
        );
        assert_eq!(
            keep(&fixture("/srv/widget-feat-x", Some("feat/x")), None, true),
            WtClass::Kept {
                reason: "no branch observation covers feat/x".into()
            },
            "a missing observation keeps by name, before the dirt reading"
        );
        assert_eq!(
            keep(
                &fixture("/srv/widget-feat-x", Some("feat/x")),
                Some(&observation("feat/x", false)),
                false
            ),
            WtClass::Kept {
                reason: "the upstream is live or unset".into()
            }
        );
        assert_eq!(
            keep(
                &fixture("/srv/widget-feat-x", Some("feat/x")),
                Some(&gone),
                false
            ),
            WtClass::Candidate
        );
    }
}
