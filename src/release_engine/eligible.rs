//! Whether a release is worth a request: a packed change, and a releasing
//! commit among the commits that made it.
//!
//! The packed change says what ships; the type of a commit says whether
//! the change is worth a release. A request opens only where the packed
//! set or a packed file changed since the last release and at least one
//! commit that touched the packed set is a releasing commit, by the one
//! classifier [`crate::integrate::release_intent`]: `feat`, `fix`, `perf`,
//! or any type a `!` or a breaking footer marks. The commits that do not
//! release still count for the changelog. With no release tag the whole
//! history is read.
//!
//! One function, [`eligible`], judges every driver. The bash and python
//! request calls it after the packed-change check and before git-cliff
//! bumps; `rk release eligible` answers it for every driver, and a rust
//! target's request job runs that verb before release-plz.
//!
//! SATISFIES release-engine:a-request-needs-a-releasing-commit

use std::collections::BTreeSet;

use camino::Utf8Path;
use serde::Serialize;

use crate::cargo_package::{self, Released};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::requirements::Tool;

use super::Done;
use super::packed::{self, Scratch};

/// The report's shape version.
pub const SCHEMA: &str = "rk.release-eligible/1";

/// The releasing types, as the reasons name them.
pub const RELEASING: &str = "feat, fix, perf, or a breaking change";

/// One commit that touched the packed set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Commit {
    /// The commit's full id.
    pub sha: String,
    /// Its subject line.
    pub subject: String,
    /// Whether its message states a releasing commit.
    pub releasing: bool,
}

/// What [`eligible`] answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// Whether the change is worth a release.
    pub eligible: bool,
    /// Why, in one sentence.
    pub reason: String,
    /// The commits that touched the packed set, newest first.
    pub commits: Vec<Commit>,
}

impl Verdict {
    /// A refusal to judge further: no packed change, or nothing packed.
    #[must_use]
    pub const fn no(reason: String) -> Self {
        Self {
            eligible: false,
            reason,
            commits: Vec::new(),
        }
    }
}

/// The words for the range a verdict read.
fn range_words(since: Option<&str>) -> String {
    since.map_or_else(
        || "in the whole history".to_owned(),
        |tag| format!("since {tag}"),
    )
}

/// Judge the commits that touched the packed set, pure.
#[must_use]
pub fn judge(since: Option<&str>, commits: Vec<Commit>) -> Verdict {
    let range = range_words(since);
    let reason = match commits.iter().find(|commit| commit.releasing) {
        Some(commit) => format!(
            "{} \"{}\" touched the packed files {range} and is a releasing commit",
            crate::integrate::short(&commit.sha),
            commit.subject
        ),
        None if commits.is_empty() => {
            format!("no commit touched the packed files {range}")
        }
        None => format!(
            "none of the {} commit{} that touched the packed files {range} is a releasing commit: {RELEASING}",
            commits.len(),
            if commits.len() == 1 { "" } else { "s" }
        ),
    };
    Verdict {
        eligible: commits.iter().any(|commit| commit.releasing),
        reason,
        commits,
    }
}

/// The commits of `<since>..HEAD`, or of the whole history with no
/// release tag, that touched any of `paths`, newest first.
///
/// # Errors
///
/// git's failure to read the history.
pub fn commits(
    root: &Utf8Path,
    since: Option<&str>,
    paths: &[String],
) -> Result<Vec<Commit>, RkError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let range = since.map_or_else(|| "HEAD".to_owned(), |tag| format!("{tag}..HEAD"));
    let mut args = vec![
        "--literal-pathspecs",
        "log",
        "--format=%x1e%H%x00%B",
        range.as_str(),
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    let output = super::git(root, &args)?;
    if !output.status.success() {
        return Err(RkError::subprocess(
            Diagnostic::new(Reason::SubprocessFailed, super::last_line(&output.stderr))
                .expected("git to read the commits that touched the packed files"),
        ));
    }
    Ok(parse_log(&String::from_utf8_lossy(&output.stdout)))
}

/// The commits of one `git log --format=%x1e%H%x00%B` listing.
#[must_use]
pub fn parse_log(text: &str) -> Vec<Commit> {
    text.split('\u{1e}')
        .filter_map(|record| {
            let (sha, message) = record.split_once('\0')?;
            let sha = sha.trim();
            (!sha.is_empty()).then(|| Commit {
                sha: sha.to_owned(),
                subject: message.lines().next().unwrap_or_default().trim().to_owned(),
                releasing: crate::integrate::release_intent(message.trim_start()).is_some(),
            })
        })
        .collect()
}

/// Whether the commits of `<since>..HEAD`, or of the whole history with no
/// release tag, that touched `paths` hold a releasing commit.
///
/// `paths` are the eligibility inputs: the packed files of both
/// revisions, and the declaration of what ships where its change altered
/// the packed set. The caller has already found the packed change.
///
/// # Errors
///
/// git's failure to read the history.
pub fn eligible(
    root: &Utf8Path,
    since: Option<&str>,
    paths: &[String],
) -> Result<Verdict, RkError> {
    Ok(judge(since, commits(root, since, paths)?))
}

/// The engine's judgment for a bash or python target.
///
/// It compares the packed files with the release `since`, or reads those
/// of `HEAD` with no release tag, then judges the commits that touched
/// them. It answers the comparison where one ran.
///
/// # Errors
///
/// A refusal of the comparison, and a build or git failure.
pub fn engine(
    context: &super::Context,
    since: Option<&str>,
) -> Result<(Verdict, Option<super::packed::Comparison>), RkError> {
    let Some(tag) = since else {
        let paths = super::packed::at_head(context)?;
        // A declaration that selects no file yet opens nothing, and says
        // so by the key that would select one.
        if paths.is_empty() {
            return Ok((Verdict::no(context.adapter.nothing_packed()), None));
        }
        return Ok((eligible(&context.target, None, &paths)?, None));
    };
    let compared = super::packed::compare(context, tag)?;
    if compared.paths.is_empty() {
        return Ok((
            Verdict::no(context.adapter.nothing_packed()),
            Some(compared),
        ));
    }
    if !compared.changed {
        let reason = format!("no packed file changed since {}", compared.tag);
        return Ok((Verdict::no(reason), Some(compared)));
    }
    let verdict = eligible(&context.target, Some(tag), &compared.inputs)?;
    Ok((verdict, Some(compared)))
}

/// The machine form of `rk release eligible`.
#[derive(Debug, Serialize)]
pub struct EligibleReport {
    /// The shape version of this document.
    pub schema: &'static str,
    /// The release driver the record names.
    pub driver: String,
    /// The package judged, where the driver releases a named one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Whether the change since the last release is worth a release.
    pub eligible: bool,
    /// Why, in one sentence.
    pub reason: String,
    /// The last release tag compared with, where one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compared: Option<String>,
    /// How many files the release packs at `HEAD` or at that tag, where
    /// the verb listed them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packed: Option<usize>,
    /// The commits that touched the packed files, newest first.
    pub commits: Vec<Commit>,
}

/// Answer whether the change since the last release is worth a release,
/// for the driver the record names. It writes nothing and reads no forge:
/// the last release is the release tag git holds.
///
/// # Errors
///
/// A target with no landing, a driver with no release, a rust workspace
/// that releases more than one package, a missing tool, and a build or git
/// failure.
pub fn run(target: &Utf8Path) -> Result<Done<EligibleReport>, RkError> {
    let driver = crate::landing::manifest::load(target)
        .ok()
        .flatten()
        .and_then(|record| record.profile.release.driver);
    if driver.as_deref() == Some("rust") {
        return rust(target);
    }
    let context = super::Context::load(target)?;
    let mut tools = vec![Tool::Git];
    if context.driver() == "python" {
        tools.push(Tool::Uv);
    }
    crate::requirements::require_in(
        "rk release eligible",
        &tools,
        Some(context.target.as_std_path()),
    )?;
    let current = context.adapter.read_version(target)?;
    let report = |verdict: Verdict, compared: Option<packed::Comparison>| EligibleReport {
        schema: SCHEMA,
        driver: context.driver().to_owned(),
        package: None,
        eligible: verdict.eligible,
        reason: verdict.reason,
        compared: compared.as_ref().map(|compared| compared.tag.clone()),
        packed: compared.as_ref().map(|compared| compared.paths.len()),
        commits: verdict.commits,
    };
    if current.is_candidate() {
        let reason = match super::guard::propose(current, false, None) {
            super::guard::Proposal::Nothing(reason) | super::guard::Proposal::Refuse(reason) => {
                reason
            }
            super::guard::Proposal::Propose(_) => String::new(),
        };
        return Ok(done(report(Verdict::no(reason), None)));
    }
    // A version with no release tag, set by hand or promoted from a
    // candidate, is judged since the last stable release.
    let tag = context.adapter.tag_of(current);
    let since = if tag_exists(target, &tag)? {
        Some(tag)
    } else {
        packed::stable_tag_below(target, Some(&tag))?
    };
    let (verdict, compared) = engine(&context, since.as_deref())?;
    Ok(done(report(verdict, compared)))
}

/// The human answer and the report.
fn done(report: EligibleReport) -> Done<EligibleReport> {
    let word = if report.eligible {
        "eligible"
    } else {
        "not eligible"
    };
    let mut lines = vec![format!("{word}: {}", report.reason)];
    if let (Some(tag), Some(packed)) = (&report.compared, report.packed) {
        lines.push(format!(
            "compared the {packed} files the release packs with {tag}"
        ));
    }
    Done {
        report,
        lines,
        next: Vec::new(),
    }
}

/// Whether git holds the tag.
fn tag_exists(root: &Utf8Path, tag: &str) -> Result<bool, RkError> {
    let at = format!("refs/tags/{tag}^{{commit}}");
    Ok(super::git(root, &["rev-parse", "--verify", "-q", &at])?
        .status
        .success())
}

/// One cargo call in `dir`, which must succeed.
fn cargo(dir: &Utf8Path, args: &[&str], expected: &str) -> Result<Vec<u8>, RkError> {
    let mut command = std::process::Command::new(Tool::Cargo.name());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let output = command
        .current_dir(dir.as_std_path())
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("cargo did not run: {error}"),
                )
                .expected(expected.to_owned()),
            )
        })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(RkError::subprocess(
        Diagnostic::new(Reason::SubprocessFailed, super::last_line(&output.stderr))
            .expected(expected.to_owned()),
    ))
}

/// Where the package and the workspace lock sit below the checkout.
struct Layout {
    /// The package directory, empty for the checkout's root.
    dir: String,
    /// The workspace lock.
    lock: String,
}

/// The path of `path` below `root`, `/`-separated, or `None` outside it.
fn below(root: &Utf8Path, path: &std::path::Path) -> Option<String> {
    let root = std::fs::canonicalize(root).ok()?;
    let path = std::fs::canonicalize(path).ok()?;
    let rest = path.strip_prefix(&root).ok()?;
    Some(
        rest.components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// Where a released package and its workspace lock sit below `root`,
/// from cargo metadata read at `root`.
fn layout(root: &Utf8Path, metadata: &[u8], package: &Released) -> Result<Layout, RkError> {
    let workspace_root = serde_json::from_slice::<serde_json::Value>(metadata)
        .ok()
        .and_then(|document| document["workspace_root"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let manifest_dir = std::path::Path::new(&package.manifest_path)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let (Some(dir), Some(workspace)) = (
        below(root, &manifest_dir),
        below(root, std::path::Path::new(&workspace_root)),
    ) else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!(
                    "the package {} or its workspace lies outside {root}",
                    package.name
                ),
            )
            .expected("a released package inside the checkout")
            .target_state("unchanged"),
        ));
    };
    let lock = if workspace.is_empty() {
        "Cargo.lock".to_owned()
    } else {
        format!("{workspace}/Cargo.lock")
    };
    Ok(Layout { dir, lock })
}

/// The workspace members release-plz releases at `root`: its cargo
/// metadata and its own release-plz configuration.
fn released_at(root: &Utf8Path) -> Result<(Vec<u8>, Vec<Released>), RkError> {
    let manifest = root.join("Cargo.toml");
    let metadata = cargo(
        root,
        &[
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            manifest.as_str(),
        ],
        "cargo metadata to read the workspace",
    )?;
    let config = cargo_package::RELEASE_PLZ_CONFIGS
        .iter()
        .find_map(|name| std::fs::read_to_string(root.join(name)).ok());
    let released = cargo_package::released_packages(&metadata, config.as_deref());
    Ok((metadata, released))
}

/// The files the package `name` packs at `rev`: `cargo package --list` in
/// a temporary worktree of the revision, each a tracked file of it, and
/// the workspace lock where the package has a binary target there.
///
/// The package is resolved by name in that revision, from its own cargo
/// metadata and release-plz configuration, so a package that moved since
/// is listed where it stood. A revision where release-plz releases no
/// package of that name packs nothing.
fn rust_side(root: &Utf8Path, rev: &str, name: &str) -> Result<BTreeSet<String>, RkError> {
    let scratch = Scratch::new(root, rev)?;
    let tree = scratch.tree();
    if !tree.join("Cargo.toml").is_file() {
        return Ok(BTreeSet::new());
    }
    let (metadata, released) = released_at(&tree)?;
    let Some(package) = released.into_iter().find(|package| package.name == name) else {
        return Ok(BTreeSet::new());
    };
    let layout = layout(&tree, &metadata, &package)?;
    let dir = if layout.dir.is_empty() {
        tree.clone()
    } else {
        tree.join(&layout.dir)
    };
    let manifest = dir.join("Cargo.toml");
    let listed = cargo(
        &tree,
        &[
            "package",
            "--list",
            "--allow-dirty",
            "--manifest-path",
            manifest.as_str(),
        ],
        "cargo to list the package at the revision",
    )?;
    let prefix = if layout.dir.is_empty() {
        String::new()
    } else {
        format!("{}/", layout.dir)
    };
    let members: Vec<String> = cargo_package::Listing::parse(&listed)
        .paths()
        .filter(|path| !cargo_package::LISTED_NOT_TRACKED.contains(path))
        .map(|path| format!("{prefix}{path}"))
        .collect();
    let tracked = packed::tracked(root, rev)?;
    let mut set = packed::keep_tracked(members, &tracked);
    if package.binary && tracked.contains(&layout.lock) {
        set.insert(layout.lock);
    }
    Ok(set)
}

/// The judgment for a rust target: the one package release-plz releases,
/// its packed files at `HEAD` and at the last release tag, and the commits
/// that touched them.
///
/// SATISFIES release-engine:a-rust-target-releases-one-package
fn rust(target: &Utf8Path) -> Result<Done<EligibleReport>, RkError> {
    crate::requirements::require_in(
        "rk release eligible",
        &[Tool::Git, Tool::Cargo],
        Some(target.as_std_path()),
    )?;
    let (_, released) = released_at(target)?;
    let report = |package: Option<String>, verdict: Verdict, compared, packed| EligibleReport {
        schema: SCHEMA,
        driver: "rust".to_owned(),
        package,
        eligible: verdict.eligible,
        reason: verdict.reason,
        compared,
        packed,
        commits: verdict.commits,
    };
    let package = match released.as_slice() {
        [] => {
            let reason = "release-plz releases no member of this workspace".to_owned();
            return Ok(done(report(None, Verdict::no(reason), None, None)));
        }
        [one] => one.clone(),
        many => return Err(more_than_one(many)),
    };
    let name = Some(package.name.clone());
    let since = packed::stable_tag_below(target, None)?;
    let mut paths = rust_side(target, "HEAD", &package.name)?;
    let changed = if let Some(tag) = since.as_deref() {
        let released_side = rust_side(target, tag, &package.name)?;
        let moved = paths != released_side;
        paths.extend(released_side);
        let listed: Vec<String> = paths.iter().cloned().collect();
        moved || packed::differs(target, tag, &listed)?
    } else {
        true
    };
    let listed: Vec<String> = paths.into_iter().collect();
    let count = since.as_ref().map(|_| listed.len());
    if !changed {
        let reason = format!(
            "no packed file changed since {}",
            since.as_deref().unwrap_or_default()
        );
        return Ok(done(report(name, Verdict::no(reason), since, count)));
    }
    let verdict = eligible(target, since.as_deref(), &listed)?;
    Ok(done(report(name, verdict, since, count)))
}

/// The refusal of a workspace whose release-plz releases more than one
/// package: the tag shape `v{{ version }}` names one.
fn more_than_one(many: &[Released]) -> RkError {
    let names: Vec<&str> = many.iter().map(|package| package.name.as_str()).collect();
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!(
                "release-plz releases {} packages of this workspace, {}, and rk release eligible judges a target that releases one, as the tag shape v{{{{ version }}}} assumes",
                many.len(),
                names.join(", ")
            ),
        )
        .expected("one released package: set release = false in release-plz.toml for every other member, or publish = false in its Cargo.toml where release-plz is not git_only")
        .target_state("unchanged"),
    )
}

#[cfg(test)]
mod tests {
    use super::{Commit, EligibleReport, SCHEMA, judge, parse_log};

    fn commit(sha: &str, subject: &str, releasing: bool) -> Commit {
        Commit {
            sha: sha.to_owned(),
            subject: subject.to_owned(),
            releasing,
        }
    }

    /// SATISFIES release-engine:a-request-needs-a-releasing-commit
    ///
    /// The log parses into commits classified by the one classifier, and
    /// the judgment answers yes only where one of them releases.
    #[test]
    fn the_judgment_needs_one_releasing_commit() {
        let log = "\u{1e}aaaaaaaaaa\0build: tidy the makefile\n\n\u{1e}bbbbbbbbbb\0refactor(cli): move it\n\nBREAKING CHANGE: gone\n\u{1e}cccccccccc\0perf: speed it\n";
        let parsed = parse_log(log);
        assert_eq!(
            parsed,
            vec![
                commit("aaaaaaaaaa", "build: tidy the makefile", false),
                commit("bbbbbbbbbb", "refactor(cli): move it", true),
                commit("cccccccccc", "perf: speed it", true),
            ]
        );
        let yes = judge(Some("v0.1.0"), parsed);
        assert!(yes.eligible);
        assert_eq!(
            yes.reason,
            "bbbbbbb \"refactor(cli): move it\" touched the packed files since v0.1.0 and is a releasing commit"
        );
        let no = judge(None, vec![commit("aaaaaaaaaa", "build: tidy", false)]);
        assert!(!no.eligible);
        assert_eq!(
            no.reason,
            "none of the 1 commit that touched the packed files in the whole history is a releasing commit: feat, fix, perf, or a breaking change"
        );
        assert_eq!(
            judge(Some("v1.0.0"), Vec::new()).reason,
            "no commit touched the packed files since v1.0.0"
        );
    }

    /// SATISFIES release-engine:the-output-declares-its-schema
    ///
    /// The verb's report, as a rust target's job and an agent read it.
    #[test]
    fn the_eligible_report_holds_its_schema() {
        let report = EligibleReport {
            schema: SCHEMA,
            driver: "rust".to_owned(),
            package: Some("widget".to_owned()),
            eligible: true,
            reason: "abcdef1 \"fix(deps): move dep\" touched the packed files since v0.1.0 and is a releasing commit".to_owned(),
            compared: Some("v0.1.0".to_owned()),
            packed: Some(4),
            commits: vec![commit("abcdef1234", "fix(deps): move dep", true)],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("serializes"),
            r#"{"schema":"rk.release-eligible/1","driver":"rust","package":"widget","eligible":true,"reason":"abcdef1 \"fix(deps): move dep\" touched the packed files since v0.1.0 and is a releasing commit","compared":"v0.1.0","packed":4,"commits":[{"sha":"abcdef1234","subject":"fix(deps): move dep","releasing":true}]}"#
        );
        let first = EligibleReport {
            schema: SCHEMA,
            driver: "bash".to_owned(),
            package: None,
            eligible: false,
            reason: "no commit touched the packed files in the whole history".to_owned(),
            compared: None,
            packed: None,
            commits: Vec::new(),
        };
        assert_eq!(
            serde_json::to_string(&first).expect("serializes"),
            r#"{"schema":"rk.release-eligible/1","driver":"bash","eligible":false,"reason":"no commit touched the packed files in the whole history","commits":[]}"#
        );
    }
}
