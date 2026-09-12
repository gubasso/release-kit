//! The manager × venue matrix for `rk` itself: which pairs land as a
//! fragment, seeded where the manager file is absent, and which are a
//! hand edit with a named reason.
//!
//! The matrix guesses nothing it cannot read offline. release-kit's
//! crate name, flake output, and release archives are known, so the
//! pairs that consume one of those render; the pairs that need a nixpkgs
//! attribute, a source hash, or an asdf plugin nobody publishes are
//! `manual` with that reason, and the report says so instead of
//! inventing one. The dependency matrix in `src/depend/matrix.rs` has
//! the same shape over an arbitrary source, and its reasons are about
//! that source, so nothing is imported from it.

use serde::Serialize;

use super::Observed;
use super::manager::Manager;
use super::venue::Venue;
use crate::error::RkError;

/// Whether a pair renders, and why not where it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// The pair renders fragments, and a seed where the file is absent.
    Fragment,
    /// The pair needs knowledge the binary does not have offline.
    Manual(&'static str),
}

/// How a pair lands in one target: the closed `support` vocabulary of
/// the add report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// The manager file is present: the fragments are applied by hand.
    Fragment,
    /// The manager file is absent: `--apply` seeds it.
    Seed,
    /// A hand edit the report describes; nothing is written.
    Manual,
}

/// One manager and one venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Pair {
    /// The manager.
    pub manager: Manager,
    /// The venue.
    pub venue: Venue,
}

/// Every manual reason the matrix can name, closed: a test holds each
/// pair's reason to this list.
pub const REASONS: [&str; 4] = [
    "nixpkgs-attribute-unknown",
    "network-hash-needed",
    "no-mise-flake-backend",
    "asdf-plugin-unknown",
];

/// The support of one pair.
#[must_use]
pub const fn support(manager: Manager, venue: Venue) -> Support {
    match (manager, venue) {
        (Manager::Flake | Manager::Devbox, Venue::Flake)
        | (Manager::Mise, Venue::Crates | Venue::GithubRelease) => Support::Fragment,
        (Manager::Flake, Venue::GithubRelease) => Support::Manual("network-hash-needed"),
        (Manager::Flake | Manager::Devbox, _) => Support::Manual("nixpkgs-attribute-unknown"),
        (Manager::Mise, Venue::Flake) => Support::Manual("no-mise-flake-backend"),
        (Manager::Asdf, _) => Support::Manual("asdf-plugin-unknown"),
    }
}

/// The venues a manager prefers, first first.
#[must_use]
pub const fn preference(manager: Manager) -> [Venue; 3] {
    match manager {
        Manager::Flake | Manager::Devbox => [Venue::Flake, Venue::Crates, Venue::GithubRelease],
        Manager::Mise | Manager::Asdf => [Venue::Crates, Venue::GithubRelease, Venue::Flake],
    }
}

/// The first venue a manager renders a fragment for, or its first
/// preference where every venue is manual.
#[must_use]
pub fn default_venue(manager: Manager) -> Venue {
    let order = preference(manager);
    order
        .into_iter()
        .find(|venue| support(manager, *venue) == Support::Fragment)
        .unwrap_or(order[0])
}

/// The pair one `add` or `sync` serves, from the flags and the target.
///
/// A manager the flag names wins. Otherwise the one present manager is
/// chosen, and a target with no manager file takes the flake, the pair
/// the seeds were first written for. Two present managers need the flag.
///
/// # Errors
///
/// Returns [`RkError::Usage`] where the manager is ambiguous.
pub fn choose(
    observed: &Observed,
    manager: Option<Manager>,
    venue: Option<Venue>,
) -> Result<Pair, RkError> {
    let manager = match manager {
        Some(manager) => manager,
        None => detected_manager(observed)?,
    };
    let venue = venue.unwrap_or_else(|| default_venue(manager));
    Ok(Pair { manager, venue })
}

/// The one manager the target carries, the flake where it carries none,
/// or the usage error naming why `--manager` is needed.
fn detected_manager(observed: &Observed) -> Result<Manager, RkError> {
    let present: Vec<Manager> = observed
        .managers
        .iter()
        .filter(|entry| entry.present.is_present())
        .map(|entry| entry.manager)
        .collect();
    match present.as_slice() {
        [one] => Ok(*one),
        [] => Ok(Manager::Flake),
        many => {
            let names: Vec<&str> = many.iter().map(|m| m.as_str()).collect();
            Err(RkError::Usage(format!(
                "the target carries {}; pass --manager to choose",
                names.join(" and ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Manager, Mode, REASONS, Support, Venue, default_venue, preference, support};

    /// SATISFIES packaging:the-venue-and-the-manager-cross-in-one-matrix
    #[test]
    fn every_pair_in_the_matrix_is_classified_once() {
        let mut fragment_pairs = 0;
        for manager in Manager::ALL {
            let mut seen = Vec::new();
            for venue in preference(manager) {
                assert!(!seen.contains(&venue), "{manager:?} lists {venue:?} once");
                seen.push(venue);
                if support(manager, venue) == Support::Fragment {
                    fragment_pairs += 1;
                }
            }
            assert_eq!(
                seen.len(),
                Venue::ALL.len(),
                "{manager:?} covers every venue"
            );
        }
        assert_eq!(
            fragment_pairs, 4,
            "flake, devbox, and the two mise pairs render"
        );
        assert_eq!(default_venue(Manager::Flake), Venue::Flake);
        assert_eq!(default_venue(Manager::Mise), Venue::Crates);
        assert_eq!(default_venue(Manager::Devbox), Venue::Flake);
        assert_eq!(default_venue(Manager::Asdf), Venue::Crates);
        assert_eq!(Mode::Seed, Mode::Seed);
    }

    /// SATISFIES packaging:the-venue-and-the-manager-cross-in-one-matrix
    #[test]
    fn every_manual_pair_names_a_closed_reason() {
        let mut named = Vec::new();
        for manager in Manager::ALL {
            for venue in Venue::ALL {
                if let Support::Manual(reason) = support(manager, venue) {
                    assert!(
                        REASONS.contains(&reason),
                        "{manager:?}/{venue:?}: {reason} is not in the closed set"
                    );
                    named.push(reason);
                }
            }
        }
        for reason in REASONS {
            assert!(named.contains(&reason), "{reason} is named by no pair");
        }
    }

    #[test]
    fn the_asdf_rows_are_manual_with_their_reason() {
        for venue in Venue::ALL {
            assert_eq!(
                support(Manager::Asdf, venue),
                Support::Manual("asdf-plugin-unknown"),
                "{venue:?}"
            );
        }
    }
}
