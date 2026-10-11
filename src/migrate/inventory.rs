//! The one detector of predecessor files.
//!
//! A predecessor file is one another release tool owns: its release
//! configuration, its workflow, its manifest, a bump script, or a hook.
//! Each is named here with its tool and its kind, so the archive moves
//! exactly what this table lists and nothing a guess would add. Four root
//! files serve the project whatever releases it, so they stay in place
//! and are listed as such: the changelog, the readme, the licence, and the
//! agent entry document.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::digest::Digest;
use crate::error::RkError;

/// What a predecessor file is to its tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A CI workflow that runs the tool.
    Workflow,
    /// The tool's own configuration.
    ReleaseConfig,
    /// A script that moves a version by hand.
    BumpScript,
    /// A hook or the configuration a hook reads.
    Hook,
    /// A file the tool writes to remember a version.
    Manifest,
    /// A root file that stays in place.
    Root,
}

impl Kind {
    /// The kebab-case word, as the JSON serializes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workflow => "workflow",
            Self::ReleaseConfig => "release-config",
            Self::BumpScript => "bump-script",
            Self::Hook => "hook",
            Self::Manifest => "manifest",
            Self::Root => "root",
        }
    }
}

/// Where a listed file stays through the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// The archive moves it into the workspace.
    Archived,
    /// It stays at its path in the target.
    InPlace,
}

impl Placement {
    /// The kebab-case word, as the JSON serializes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Archived => "archived",
            Self::InPlace => "in-place",
        }
    }
}

/// Every predecessor file this release recognizes: its path, its tool,
/// and its kind.
pub const PREDECESSORS: [(&str, &str, Kind); 52] = [
    (
        "release-please-config.json",
        "release-please",
        Kind::ReleaseConfig,
    ),
    (
        ".release-please-manifest.json",
        "release-please",
        Kind::Manifest,
    ),
    (
        ".github/workflows/release-please.yml",
        "release-please",
        Kind::Workflow,
    ),
    (
        ".github/workflows/release-please.yaml",
        "release-please",
        Kind::Workflow,
    ),
    (".releaserc", "semantic-release", Kind::ReleaseConfig),
    (".releaserc.json", "semantic-release", Kind::ReleaseConfig),
    (".releaserc.yaml", "semantic-release", Kind::ReleaseConfig),
    (".releaserc.yml", "semantic-release", Kind::ReleaseConfig),
    (".releaserc.js", "semantic-release", Kind::ReleaseConfig),
    (".releaserc.cjs", "semantic-release", Kind::ReleaseConfig),
    (".releaserc.mjs", "semantic-release", Kind::ReleaseConfig),
    ("release.config.js", "semantic-release", Kind::ReleaseConfig),
    (
        "release.config.cjs",
        "semantic-release",
        Kind::ReleaseConfig,
    ),
    (
        "release.config.mjs",
        "semantic-release",
        Kind::ReleaseConfig,
    ),
    (
        ".github/workflows/semantic-release.yml",
        "semantic-release",
        Kind::Workflow,
    ),
    (".goreleaser.yaml", "goreleaser", Kind::ReleaseConfig),
    (".goreleaser.yml", "goreleaser", Kind::ReleaseConfig),
    ("goreleaser.yaml", "goreleaser", Kind::ReleaseConfig),
    ("goreleaser.yml", "goreleaser", Kind::ReleaseConfig),
    (".config/goreleaser.yaml", "goreleaser", Kind::ReleaseConfig),
    (".config/goreleaser.yml", "goreleaser", Kind::ReleaseConfig),
    (
        ".github/workflows/goreleaser.yml",
        "goreleaser",
        Kind::Workflow,
    ),
    ("release-plz.toml", "release-plz", Kind::ReleaseConfig),
    (".release-plz.toml", "release-plz", Kind::ReleaseConfig),
    (
        ".github/workflows/release-plz.yml",
        "release-plz",
        Kind::Workflow,
    ),
    ("cliff.toml", "git-cliff", Kind::ReleaseConfig),
    ("dist-workspace.toml", "cargo-dist", Kind::ReleaseConfig),
    (
        ".github/workflows/release.yml",
        "cargo-dist",
        Kind::Workflow,
    ),
    (".changeset/config.json", "changesets", Kind::ReleaseConfig),
    (
        ".github/workflows/changesets.yml",
        "changesets",
        Kind::Workflow,
    ),
    (".bumpversion.cfg", "bump-my-version", Kind::ReleaseConfig),
    (".bumpversion.toml", "bump-my-version", Kind::ReleaseConfig),
    (".cz.toml", "commitizen", Kind::ReleaseConfig),
    (".cz.json", "commitizen", Kind::ReleaseConfig),
    (".cz.yaml", "commitizen", Kind::ReleaseConfig),
    (".versionrc", "standard-version", Kind::ReleaseConfig),
    (".versionrc.json", "standard-version", Kind::ReleaseConfig),
    (
        ".github/release-drafter.yml",
        "release-drafter",
        Kind::ReleaseConfig,
    ),
    (
        ".github/workflows/release-drafter.yml",
        "release-drafter",
        Kind::Workflow,
    ),
    ("commitlint.config.js", "commitlint", Kind::Hook),
    ("commitlint.config.cjs", "commitlint", Kind::Hook),
    (".commitlintrc", "commitlint", Kind::Hook),
    (".commitlintrc.json", "commitlint", Kind::Hook),
    (".commitlintrc.yml", "commitlint", Kind::Hook),
    (".husky/commit-msg", "husky", Kind::Hook),
    (
        ".github/workflows/publish.yml",
        "hand-rolled",
        Kind::Workflow,
    ),
    (
        ".github/workflows/publish.yaml",
        "hand-rolled",
        Kind::Workflow,
    ),
    (
        ".github/workflows/release.yaml",
        "hand-rolled",
        Kind::Workflow,
    ),
    ("release.sh", "hand-rolled", Kind::BumpScript),
    ("bump.sh", "hand-rolled", Kind::BumpScript),
    ("scripts/release.sh", "hand-rolled", Kind::BumpScript),
    ("scripts/bump-version.sh", "hand-rolled", Kind::BumpScript),
];

/// The root files that stay in place and are listed as such.
pub const ROOT_FILES: [&str; 4] = ["CHANGELOG.md", "README.md", "LICENSE", "AGENTS.md"];

/// One listed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// The target-relative path.
    pub path: String,
    /// The tool that owns it, or `project` for a root file.
    pub tool: &'static str,
    /// What it is to its tool.
    pub kind: Kind,
    /// Whether the archive moves it.
    pub placement: Placement,
    /// The sha256 of its bytes.
    pub sha256: String,
}

/// The tool, kind, and placement of one path, or `None` for a path this
/// release does not list.
#[must_use]
pub fn classify(path: &str) -> Option<(&'static str, Kind, Placement)> {
    if ROOT_FILES.contains(&path) {
        return Some(("project", Kind::Root, Placement::InPlace));
    }
    PREDECESSORS
        .iter()
        .find(|(known, _, _)| *known == path)
        .map(|(_, tool, kind)| (*tool, *kind, Placement::Archived))
}

/// Every listed path among `paths`, predecessor files first in table
/// order, then the root files.
#[must_use]
pub fn listed<'a>(paths: &[&'a str]) -> Vec<&'a str> {
    PREDECESSORS
        .iter()
        .map(|(known, _, _)| *known)
        .chain(ROOT_FILES)
        .filter_map(|known| paths.iter().find(|path| **path == known).copied())
        .collect()
}

/// Every listed file present at `target`, read from the disk.
///
/// # Errors
///
/// [`RkError::Io`] where a present file cannot be read.
pub fn scan(target: &Utf8Path) -> Result<Vec<Entry>, RkError> {
    let present: Vec<&str> = PREDECESSORS
        .iter()
        .map(|(path, _, _)| *path)
        .chain(ROOT_FILES)
        .filter(|path| {
            let full = target.join(path);
            full.is_file() && !full.is_symlink()
        })
        .collect();
    let mut out = Vec::new();
    for path in listed(&present) {
        let bytes = std::fs::read(target.join(path))?;
        let (tool, kind, placement) =
            classify(path).unwrap_or(("project", Kind::Root, Placement::InPlace));
        out.push(Entry {
            path: path.to_owned(),
            tool,
            kind,
            placement,
            sha256: Digest::of(&bytes).to_string(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{Kind, PREDECESSORS, Placement, ROOT_FILES, classify, listed};

    #[test]
    fn every_predecessor_path_is_listed_once_and_no_root_file_is_archived() {
        let mut seen = std::collections::BTreeSet::new();
        for (path, _, kind) in PREDECESSORS {
            assert!(seen.insert(path), "{path} is listed twice");
            assert_ne!(kind, Kind::Root, "{path}");
            assert!(!ROOT_FILES.contains(&path), "{path}");
        }
        for root in ROOT_FILES {
            assert_eq!(
                classify(root),
                Some(("project", Kind::Root, Placement::InPlace))
            );
        }
        assert_eq!(
            classify("release-please-config.json"),
            Some(("release-please", Kind::ReleaseConfig, Placement::Archived))
        );
        assert_eq!(classify("src/main.rs"), None);
    }

    #[test]
    fn the_listing_keeps_the_table_order() {
        let paths = [
            "README.md",
            "cliff.toml",
            "release-please-config.json",
            "src/lib.rs",
        ];
        assert_eq!(
            listed(&paths),
            ["release-please-config.json", "cliff.toml", "README.md"]
        );
    }
}
