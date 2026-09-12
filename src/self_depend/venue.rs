//! The venue axis: where a release of `rk` itself is published, and the
//! reference form a manager records for each.
//!
//! One enum owns the list, in the order the reports list them. A venue
//! joins the list only where this repository's own release proves the
//! publication in CI, the same claim `packaging:an-advertised-system-is-a-
//! proven-system` holds for a system. A system package venue has no
//! variant: no per-project manager pins a system package to a version
//! inside a repository, so every pair on it would be manual, and the
//! reason string alone is recorded here.

use clap::ValueEnum;
use serde::Serialize;

use super::pin::PIN_PREFIX;

/// Where a release of `rk` is published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum Venue {
    /// The crate on crates.io, built from source by the consumer.
    Crates,
    /// The flake this repository serves at every tag.
    Flake,
    /// The prebuilt archives attached to a GitHub release.
    GithubRelease,
}

/// The reason every pair on a system package venue would carry: a
/// distribution package is pinned by the host, never inside a
/// repository, so no manager here can record or move it.
pub const SYSTEM_PACKAGE_REASON: &str = "system-package-unpinnable";

impl Venue {
    /// The wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crates => "crates",
            Self::Flake => "flake",
            Self::GithubRelease => "github-release",
        }
    }

    /// Every venue, in the order the reports list them.
    pub const ALL: [Self; 3] = [Self::Crates, Self::Flake, Self::GithubRelease];

    /// The forge path the flake and the release archives are served
    /// from, derived from the pin grammar so the project path has one
    /// owner.
    #[must_use]
    pub fn owner_repo() -> &'static str {
        PIN_PREFIX
            .trim_start_matches("github:")
            .trim_end_matches('/')
    }

    /// The version as a manager records it for this venue: the flake
    /// venue records the `v` tag inside a flake reference, the crates
    /// venue the bare version, and the release venue the bare version a
    /// release archive is fetched by.
    #[must_use]
    pub fn reference(self, tag: &str) -> String {
        let bare = tag.strip_prefix('v').unwrap_or(tag);
        match self {
            Self::Crates | Self::GithubRelease => bare.to_owned(),
            Self::Flake => format!("{PIN_PREFIX}v{bare}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SYSTEM_PACKAGE_REASON, Venue};

    #[test]
    fn the_venue_list_is_closed_and_ordered() {
        assert_eq!(Venue::ALL.len(), 3);
        let names: Vec<&str> = Venue::ALL.iter().map(|v| v.as_str()).collect();
        assert_eq!(names, ["crates", "flake", "github-release"]);
        assert_eq!(Venue::owner_repo(), "gubasso/release-kit");
        assert!(!SYSTEM_PACKAGE_REASON.is_empty());
    }

    #[test]
    fn each_venue_renders_its_reference_form() {
        assert_eq!(Venue::Crates.reference("v0.2.16"), "0.2.16");
        assert_eq!(Venue::GithubRelease.reference("0.2.16"), "0.2.16");
        assert_eq!(
            Venue::Flake.reference("0.2.16"),
            "github:gubasso/release-kit/v0.2.16"
        );
        assert_eq!(
            Venue::Flake.reference("v0.2.16"),
            "github:gubasso/release-kit/v0.2.16",
            "the tag takes exactly one v"
        );
    }
}
