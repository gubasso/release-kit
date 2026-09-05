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

use crate::error::RkError;
use crate::landing::{self, manifest};
use crate::setup::context::TRUNK_BRANCH;

/// Files that mark a release mechanism, whichever tool owns it. The
/// payload's own destinations are judged separately, as collisions; this
/// list is what other tools leave behind.
pub const RELEASE_MARKERS: [&str; 15] = [
    ".release-plz.toml",
    ".releaserc",
    ".releaserc.js",
    ".releaserc.json",
    ".releaserc.yaml",
    ".releaserc.yml",
    "release.config.js",
    ".goreleaser.yaml",
    ".goreleaser.yml",
    ".github/workflows/publish.yml",
    ".github/workflows/publish.yaml",
    ".github/workflows/release.yaml",
    ".github/workflows/release-drafter.yml",
    "CHANGELOG.md",
    "CHANGES.md",
];

/// Branch names that conventionally outlive a topic: a second one beside
/// the trunk is the retired two-branch flow, or a trunk under another
/// name, and either is a migration step.
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
/// landing record — a broken record must not silently classify — and
/// [`RkError::Io`] for a disk read that fails for a reason other than
/// absence.
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
    release_markers.sort();
    let mut collisions = Vec::new();
    for destination in landing::destinations() {
        if landing::read_recorded(target, destination)?.is_some() {
            collisions.push(destination.to_owned());
        }
    }
    collisions.sort();
    let (git, tags, long_lived_branches) = git_evidence(target);
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

/// The git-borne evidence: whether the target is a repository, how many
/// tags it holds, and which long-lived branches stand beside the trunk.
/// A target git cannot read answers `false` and empty — an observation,
/// never a failure, because a plain directory is a legitimate greenfield.
fn git_evidence(target: &Utf8Path) -> (bool, usize, Vec<String>) {
    let Some(tags) = git_lines(target, &["tag", "--list"]) else {
        return (false, 0, Vec::new());
    };
    let refs = git_lines(
        target,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
    )
    .unwrap_or_default();
    (true, tags.len(), long_lived_among(&refs))
}

/// The long-lived branch names among `refs`, the trunk excluded and the
/// remote prefix stripped, each name once, in the catalog's order.
#[must_use]
pub fn long_lived_among(refs: &[String]) -> Vec<String> {
    let names: Vec<&str> = refs
        .iter()
        .map(|name| name.split_once('/').map_or(name.as_str(), |(_, rest)| rest))
        .collect();
    LONG_LIVED_BRANCHES
        .iter()
        .filter(|candidate| **candidate != TRUNK_BRANCH)
        .filter(|candidate| names.contains(candidate))
        .map(|candidate| (*candidate).to_owned())
        .collect()
}

/// The non-empty stdout lines of one git call, or `None` where git did
/// not run or refused — a directory that is not a repository.
fn git_lines(target: &Utf8Path, args: &[&str]) -> Option<Vec<String>> {
    let out = Command::new(crate::probes::git_bin())
        .arg("-C")
        .arg(target)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
    )
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

    /// The trunk is never evidence against itself; a remote prefix is
    /// stripped; a topic branch is not long-lived; a name appears once.
    #[test]
    fn long_lived_branches_are_read_from_the_refs() {
        let refs: Vec<String> = [
            "master",
            "origin/master",
            "origin/HEAD",
            "develop",
            "origin/develop",
            "feat/x",
            "origin/main",
        ]
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
        assert_eq!(long_lived_among(&refs), vec!["main", "develop"]);
        assert!(long_lived_among(&["master".to_owned()]).is_empty());
    }

    #[test]
    fn the_verdict_words_are_the_wire_form() {
        assert_eq!(Classification::Greenfield.as_str(), "greenfield");
        assert_eq!(Classification::Brownfield.as_str(), "brownfield");
        assert_eq!(Classification::NeedsDecision.as_str(), "needs-decision");
    }
}
