//! Classify a target repository before anything lands.
//!
//! The assessment is read-only evidence plus one classification computed
//! from it by an explicit rule: `greenfield` when the target carries no
//! release mechanism and no release history, `brownfield` when a release
//! mechanism is already in place — a tool's configuration, a payload
//! destination, a landed block — and `needs-decision` when the target
//! shows release activity that no recognized mechanism explains: tags
//! with no tool behind them, or a second long-lived branch. The rule
//! lives here so a routing skill reads a verdict it can cite instead of
//! judging "some release setup" by feel. The gathering spawns git and
//! reads the disk; the rule itself is pure and unit-tested.

use std::process::Command;

use camino::Utf8Path;
use serde::Serialize;

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::{self, manifest};
use crate::setup::context::TRUNK_BRANCH;

/// The prefix of the convention's own long-lived branch form.
const RELEASE_LINE_PREFIX: &str = "release/";

/// Files that mark a release mechanism, whichever tool owns it.
///
/// The payload's own destinations are judged separately, as collisions;
/// this list is what other tools leave behind: every configuration name
/// semantic-release and `GoReleaser` document, release-plz's dotted form,
/// the workflow names a hand-rolled publish commonly takes, and a
/// changelog. `package.json` joins the list only when it carries the
/// top-level `release` key semantic-release reads, judged in [`gather`].
pub const RELEASE_MARKERS: [&str; 23] = [
    ".release-plz.toml",
    ".releaserc",
    ".releaserc.cjs",
    ".releaserc.js",
    ".releaserc.json",
    ".releaserc.mjs",
    ".releaserc.yaml",
    ".releaserc.yml",
    "release.config.cjs",
    "release.config.js",
    "release.config.mjs",
    ".config/goreleaser.yaml",
    ".config/goreleaser.yml",
    ".goreleaser.yaml",
    ".goreleaser.yml",
    "goreleaser.yaml",
    "goreleaser.yml",
    ".github/workflows/publish.yml",
    ".github/workflows/publish.yaml",
    ".github/workflows/release.yaml",
    ".github/workflows/release-drafter.yml",
    "CHANGELOG.md",
    "CHANGES.md",
];

/// Branch names that conventionally outlive a topic.
///
/// A second one beside the trunk is the retired two-branch flow, or a
/// trunk under another name, and either is a migration step. A
/// `release/<line>` branch — the convention's own long-lived form — is
/// recognized by its prefix.
pub const LONG_LIVED_BRANCHES: [&str; 11] = [
    "master",
    "main",
    "trunk",
    "develop",
    "development",
    "dev",
    "staging",
    "next",
    "release",
    "production",
    "prod",
];

/// What the target is, for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// No release mechanism and no release history: land the workflow.
    Greenfield,
    /// A release mechanism is in place: migrate, never land beside it.
    Brownfield,
    /// Release activity no mechanism explains: the operator decides.
    NeedsDecision,
}

impl Classification {
    /// The kebab-case verdict word, as the JSON serializes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Greenfield => "greenfield",
            Self::Brownfield => "brownfield",
            Self::NeedsDecision => "needs-decision",
        }
    }
}

/// The landing record's presence, the one fact `rk status` owns that the
/// routing needs before it reads the full report.
#[derive(Debug, Serialize)]
pub struct Landing {
    /// Whether `.release-kit/manifest.json` exists and reads.
    pub recorded: bool,
    /// The release-kit version the record names, where one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rk_version: Option<String>,
}

/// The evidence the classification is computed from.
#[derive(Debug, Serialize)]
pub struct Evidence {
    /// The landing record, present or not.
    pub landing: Landing,
    /// The technology the version file names, where one is found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech: Option<&'static str>,
    /// The forge the origin remote maps to, where one is recognized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forge: Option<&'static str>,
    /// The project path from the origin remote, where one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Release-mechanism files of other tools found at the target.
    pub release_markers: Vec<String>,
    /// Payload destinations already present: a whole file that exists, or
    /// a block destination whose marked block is present.
    pub collisions: Vec<String>,
    /// Whether the target is a git repository the evidence below reads.
    pub git: bool,
    /// How many tags the repository holds.
    pub tags: usize,
    /// Long-lived branches found besides the trunk, local or remote.
    pub long_lived_branches: Vec<String>,
}

/// Compute the verdict from the evidence. Pure, so the rule is testable
/// without a repository.
#[must_use]
pub fn classify(evidence: &Evidence) -> Classification {
    if !evidence.release_markers.is_empty() || !evidence.collisions.is_empty() {
        return Classification::Brownfield;
    }
    if evidence.tags > 0 || !evidence.long_lived_branches.is_empty() {
        return Classification::NeedsDecision;
    }
    Classification::Greenfield
}

/// Gather the evidence at `target`, reading and never writing.
///
/// # Errors
///
/// Returns the record's own failure taxonomy for an unreadable or unknown
/// landing record — a broken record must not silently classify —
/// [`RkError::Io`] for a disk read that fails for a reason other than
/// absence, and [`RkError::Subprocess`] where git runs but cannot answer
/// for a repository, because an observation that cannot be read is not a
/// pass and must never read as an absent release history.
pub fn gather(target: &Utf8Path) -> Result<Evidence, RkError> {
    let record = manifest::load(target)?;
    let landing = Landing {
        recorded: record.is_some(),
        rk_version: record.map(|manifest| manifest.rk_version),
    };
    let detected = crate::detect::detect(target.as_std_path());
    let mut release_markers: Vec<String> = RELEASE_MARKERS
        .iter()
        .filter(|marker| target.join(marker).is_file())
        .map(|marker| (*marker).to_owned())
        .collect();
    if package_json_names_a_release(target)? {
        release_markers.push("package.json".to_owned());
    }
    release_markers.sort();
    let mut collisions = Vec::new();
    for destination in landing::destinations() {
        if landing::read_recorded(target, destination)?.is_some() {
            collisions.push(destination.to_owned());
        }
    }
    collisions.sort();
    let (git, tags, long_lived_branches) = git_evidence(target)?;
    Ok(Evidence {
        landing,
        tech: crate::detect::tech_of(target.as_std_path()),
        forge: detected.forge.map(crate::detect::Forge::as_str),
        repo: detected.repo,
        release_markers,
        collisions,
        git,
        tags,
        long_lived_branches,
    })
}

/// Whether `package.json` carries the top-level `release` key
/// semantic-release reads its configuration from. An ordinary Node
/// project's manifest is not a release marker; only that key is.
fn package_json_names_a_release(target: &Utf8Path) -> Result<bool, RkError> {
    let path = target.join("package.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(RkError::Io(e)),
    };
    // A manifest that does not parse is not evidence of a release
    // mechanism; the tool that would read it fails on it too.
    Ok(serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| value.get("release").map(|_| ()))
        .is_some())
}

/// The git-borne evidence: whether the target is a repository, how many
/// tags it holds, and which long-lived branches stand beside the trunk.
///
/// A directory git positively reports as no repository answers `false`
/// and empty — an observation, never a failure, because a plain
/// directory is a legitimate greenfield. Every other refusal — a
/// corrupt repository, an ownership refusal, a git that does not run —
/// is an error, because an unreadable history must not read as none.
fn git_evidence(target: &Utf8Path) -> Result<(bool, usize, Vec<String>), RkError> {
    match git_lines(target, &["rev-parse", "--git-dir"]) {
        Ok(_) => {}
        Err(GitFailure::NotARepository) => return Ok((false, 0, Vec::new())),
        Err(GitFailure::Other(error)) => return Err(error),
    }
    let tags = git_lines(target, &["tag", "--list"]).map_err(GitFailure::into_error)?;
    let refs = git_lines(
        target,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
    )
    .map_err(GitFailure::into_error)?;
    Ok((true, tags.len(), long_lived_among(&refs)))
}

/// The long-lived branch names among `refs`, given as full ref names.
///
/// `refs/heads/<name>` keeps its whole name, `refs/remotes/<remote>/<name>`
/// drops the remote alone, and a remote `HEAD` pointer is skipped. A
/// name is long-lived when it is a catalog entry other than the trunk or
/// carries the release-line prefix; each appears once, sorted.
#[must_use]
pub fn long_lived_among(refs: &[String]) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    for reference in refs {
        let name = if let Some(local) = reference.strip_prefix("refs/heads/") {
            local
        } else if let Some(remote) = reference.strip_prefix("refs/remotes/") {
            match remote.split_once('/') {
                Some((_, "HEAD")) | None => continue,
                Some((_, name)) => name,
            }
        } else {
            continue;
        };
        let catalogued = name != TRUNK_BRANCH && LONG_LIVED_BRANCHES.contains(&name);
        if catalogued || name.starts_with(RELEASE_LINE_PREFIX) {
            names.insert(name.to_owned());
        }
    }
    names.into_iter().collect()
}

/// Why one git call gave no answer.
enum GitFailure {
    /// Git ran and said the target is not a repository.
    NotARepository,
    /// Git did not run, or ran and refused for another reason.
    Other(RkError),
}

impl GitFailure {
    /// After the target is known to be a repository, every failure is
    /// the same kind: a history that cannot be read.
    fn into_error(self) -> RkError {
        match self {
            Self::NotARepository => RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessFailed,
                    "git stopped answering for a repository it had just recognized",
                )
                .expected("a readable repository"),
            ),
            Self::Other(error) => error,
        }
    }
}

/// The non-empty stdout lines of one git call.
///
/// The call answers for the `-C` target alone: the variables a running
/// hook exports are scrubbed, so an inherited `GIT_DIR` cannot redirect
/// the probe at another repository, and the locale is pinned to `C`, so
/// the one diagnostic this module reads — git's own "not a git
/// repository" — arrives untranslated.
fn git_lines(target: &Utf8Path, args: &[&str]) -> Result<Vec<String>, GitFailure> {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .env("LC_ALL", "C")
        .env_remove("LANGUAGE")
        .arg("-C")
        .arg(target)
        .args(args)
        .output()
        .map_err(|error| {
            GitFailure::Other(RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("git could not be spawned: {error}"),
                )
                .expected("git on PATH, or RK_GIT_BIN naming it"),
            ))
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("not a git repository") {
            return Err(GitFailure::NotARepository);
        }
        return Err(GitFailure::Other(RkError::subprocess(
            Diagnostic::new(
                Reason::SubprocessFailed,
                format!(
                    "git {} failed at {target}: {}",
                    args.join(" "),
                    stderr.trim()
                ),
            )
            .expected("git answering for the target, or a target that is not a repository")
            .action("an unreadable history is not an absent one; repair the repository or its ownership before classifying"),
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{Classification, Evidence, Landing, classify, long_lived_among};

    fn evidence() -> Evidence {
        Evidence {
            landing: Landing {
                recorded: false,
                rk_version: None,
            },
            tech: Some("rust"),
            forge: Some("github"),
            repo: Some("acme/widget".into()),
            release_markers: Vec::new(),
            collisions: Vec::new(),
            git: true,
            tags: 0,
            long_lived_branches: Vec::new(),
        }
    }

    #[test]
    fn nothing_is_greenfield() {
        assert_eq!(classify(&evidence()), Classification::Greenfield);
    }

    #[test]
    fn a_release_marker_or_a_collision_is_brownfield() {
        let mut with_marker = evidence();
        with_marker.release_markers.push("CHANGELOG.md".into());
        assert_eq!(classify(&with_marker), Classification::Brownfield);
        let mut with_collision = evidence();
        with_collision.collisions.push("release-plz.toml".into());
        assert_eq!(classify(&with_collision), Classification::Brownfield);
    }

    /// A mechanism outranks unexplained activity: tags beside a marker
    /// are a history the mechanism made, not a question.
    #[test]
    fn a_mechanism_beside_activity_is_still_brownfield() {
        let mut both = evidence();
        both.release_markers.push("CHANGELOG.md".into());
        both.tags = 7;
        both.long_lived_branches.push("develop".into());
        assert_eq!(classify(&both), Classification::Brownfield);
    }

    #[test]
    fn activity_with_no_mechanism_needs_a_decision() {
        let mut tagged = evidence();
        tagged.tags = 1;
        assert_eq!(classify(&tagged), Classification::NeedsDecision);
        let mut branched = evidence();
        branched.long_lived_branches.push("develop".into());
        assert_eq!(classify(&branched), Classification::NeedsDecision);
    }

    /// The trunk is never evidence against itself; only the remote
    /// segment is stripped, so a topic branch whose last segment is a
    /// catalog name stays a topic branch; a release line is recognized
    /// by its prefix; a remote HEAD pointer is skipped; each name once.
    #[test]
    fn long_lived_branches_are_read_from_the_full_ref_names() {
        let refs: Vec<String> = [
            "refs/heads/master",
            "refs/remotes/origin/master",
            "refs/remotes/origin/HEAD",
            "refs/heads/develop",
            "refs/remotes/origin/develop",
            "refs/heads/feat/x",
            "refs/heads/feat/develop",
            "refs/remotes/origin/main",
            "refs/heads/release/1.2",
            "refs/remotes/upstream/release/1.2",
        ]
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
        assert_eq!(
            long_lived_among(&refs),
            vec!["develop", "main", "release/1.2"]
        );
        assert!(long_lived_among(&["refs/heads/master".to_owned()]).is_empty());
        assert!(long_lived_among(&["refs/heads/feat/develop".to_owned()]).is_empty());
    }

    #[test]
    fn the_verdict_words_are_the_wire_form() {
        assert_eq!(Classification::Greenfield.as_str(), "greenfield");
        assert_eq!(Classification::Brownfield.as_str(), "brownfield");
        assert_eq!(Classification::NeedsDecision.as_str(), "needs-decision");
    }
}
