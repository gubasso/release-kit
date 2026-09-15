//! The capability catalog: which complete release-kit product each
//! embedded source belongs to, which dimensions select it, and whether it
//! is available at the dimensions a target resolved to.
//!
//! Availability belongs to a capability at its dimensions and never to a
//! category alone: `release.automation` at `(python, github)` is available
//! while the same capability at `(python, gitlab)` is unavailable. Every
//! omission is recorded with one of the five reasons the vocabulary
//! names, so a reader can tell a product nobody asked for from one this
//! release cannot land.
//!
//! The catalog is pure: it reads the snippet paths it is given and the
//! resolved parameters, and nothing else.
//!
//! SATISFIES project-profile:availability-belongs-to-a-capability-at-its-dimensions

use serde::Serialize;

use super::{Params, ReleaseMode};
use crate::detect::Forge;
use crate::projection::{CODE_SCANNING_DESTINATIONS, NIX_DESTINATIONS};

/// The local Git workflow guards: the routing block, the glossary, and the
/// hook block. Every valid target selects it.
pub const GUARDS: &str = "git.guards";
/// The complete, active title gate at the forge.
pub const TITLE_CHECK: &str = "git.title-check";
/// The landed vulnerability reporting policy.
pub const REPORTING_POLICY: &str = "security.reporting-policy";
/// The release automation at one driver and one forge: the version
/// source, the bot configuration, and the release workflow.
pub const RELEASE_AUTOMATION: &str = "release.automation";
/// The seeded package expression and the seed flake pair.
pub const PACKAGING_NIX: &str = "packaging.nix";
/// The `OpenSSF` Scorecard workflow.
pub const SCORECARD: &str = "supply-chain.scorecard";
/// The code scanning workflow, by provider.
pub const CODE_SCANNING: &str = "supply-chain.code-scanning";

/// Every capability, in the order a report lists them.
pub const ALL: [&str; 7] = [
    GUARDS,
    TITLE_CHECK,
    REPORTING_POLICY,
    RELEASE_AUTOMATION,
    PACKAGING_NIX,
    SCORECARD,
    CODE_SCANNING,
];

/// The zone below `snippets/` that carries the release-less GitLab root
/// pipeline: the minimal `.gitlab-ci.yml` that activates the title
/// fragment where no release automation ships a root pipeline.
pub const TITLE_GATE_ZONE: &str = "_title-gate";

/// The one owner of every embedded snippet: which capability lands the
/// file at `path`, the path relative to `snippets/`.
///
/// `None` is a source defect: a snippet no capability claims would land
/// under no selection, and a test holds every embedded file to one
/// owner.
#[must_use]
pub fn owner_of(path: &str) -> Option<&'static str> {
    let mut segments = path.splitn(3, '/');
    let (zone, _forge, destination) = (segments.next()?, segments.next()?, segments.next()?);
    if zone == "_shared" {
        return match destination {
            "SECURITY.md" => Some(REPORTING_POLICY),
            ".github/workflows/pr-title.yml" | ".gitlab/ci/mr-title.yml" => Some(TITLE_CHECK),
            ".github/workflows/scorecard.yml" => Some(SCORECARD),
            _ => None,
        };
    }
    if zone == TITLE_GATE_ZONE {
        return (destination == ".gitlab-ci.yml").then_some(TITLE_CHECK);
    }
    if zone.starts_with('_') {
        return None;
    }
    if NIX_DESTINATIONS.contains(&destination) {
        return Some(PACKAGING_NIX);
    }
    if CODE_SCANNING_DESTINATIONS
        .iter()
        .any(|(name, _)| *name == destination)
    {
        return Some(CODE_SCANNING);
    }
    Some(RELEASE_AUTOMATION)
}

/// The status of one capability for one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// The catalog has the complete contribution, and the target selects
    /// it.
    Selected,
    /// The target did not request the optional capability.
    NotRequested,
    /// The capability lacks a dimension the target does not have, a forge
    /// or a release driver.
    NotApplicable,
    /// The catalog knows the capability but not at these dimensions.
    Unavailable,
    /// A category name the catalog does not know.
    Unknown,
    /// Selected, but a target-owned destination blocks its activation:
    /// the safe prerequisite files still land, and the omission names
    /// the operator's one remaining edit.
    Withheld,
}

impl Status {
    /// The report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::NotRequested => "not-requested",
            Self::NotApplicable => "not-applicable",
            Self::Unavailable => "unavailable",
            Self::Unknown => "unknown",
            Self::Withheld => "withheld",
        }
    }
}

/// One capability's answer for one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Selection {
    /// The capability id.
    pub id: &'static str,
    /// Its status.
    pub status: Status,
    /// Why, for every status but selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The one edit the operator makes, for a withheld capability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The embedded sources it lands, relative to `snippets/`, empty for
    /// a capability the blocks land or one that lands nothing.
    #[serde(skip)]
    pub sources: Vec<String>,
    /// The release driver the selection is keyed on, where the capability
    /// has that dimension; the registry pins are keyed on it too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
}

impl Selection {
    /// Whether the capability contributes destinations.
    #[must_use]
    pub const fn lands(&self) -> bool {
        matches!(self.status, Status::Selected | Status::Withheld)
    }

    fn selected(id: &'static str, sources: Vec<String>, driver: Option<&str>) -> Self {
        Self {
            id,
            status: Status::Selected,
            reason: None,
            action: None,
            sources,
            driver: driver.map(str::to_owned),
        }
    }

    fn omitted(id: &'static str, status: Status, reason: impl Into<String>) -> Self {
        Self {
            id,
            status,
            reason: Some(reason.into()),
            action: None,
            sources: Vec::new(),
            driver: None,
        }
    }
}

/// What the embedded sources can land: every snippet path, relative to
/// `snippets/`, sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    files: Vec<String>,
}

impl Availability {
    /// The availability over an explicit snippet list, each path relative
    /// to `snippets/`.
    #[must_use]
    pub fn over(files: Vec<String>) -> Self {
        let mut files = files;
        files.sort();
        Self { files }
    }

    /// The availability the installed binary embeds.
    #[must_use]
    pub fn embedded() -> Self {
        Self::over(
            crate::embedded::walk(&crate::embedded::SNIPPETS)
                .into_iter()
                .map(|(path, _)| path)
                .collect(),
        )
    }

    /// Every embedded path, relative to `snippets/`.
    #[must_use]
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// The files under one zone and forge that `capability` owns.
    fn owned(&self, zone: &str, forge: &str, capability: &str) -> Vec<String> {
        let prefix = format!("{zone}/{forge}/");
        self.files
            .iter()
            .filter(|path| path.starts_with(&prefix) && owner_of(path) == Some(capability))
            .cloned()
            .collect()
    }

    /// The drivers the sources know: every non-underscore zone.
    #[must_use]
    pub fn drivers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for path in &self.files {
            if let Some((zone, _)) = path.split_once('/')
                && !zone.starts_with('_')
                && !out.iter().any(|known| known == zone)
            {
                out.push(zone.to_owned());
            }
        }
        out
    }

    /// The `(driver, forge)` tuples at which the release automation is
    /// available, in path order, as `driver, forge` strings.
    #[must_use]
    pub fn automation_tuples(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for path in &self.files {
            let mut segments = path.splitn(3, '/');
            if let (Some(driver), Some(forge), Some(_)) =
                (segments.next(), segments.next(), segments.next())
                && !driver.starts_with('_')
                && owner_of(path) == Some(RELEASE_AUTOMATION)
            {
                let entry = format!("{driver}, {forge}");
                if !out.contains(&entry) {
                    out.push(entry);
                }
            }
        }
        out
    }
}

/// The release drivers this binary's sources know.
#[must_use]
pub fn known_drivers() -> Vec<String> {
    Availability::embedded().drivers()
}

/// Whether `name` is a forge this binary has an adapter for.
fn known_forge(name: &str) -> bool {
    Forge::parse(name).is_some()
}

/// Select every capability for `params` against `availability`.
///
/// The order is [`ALL`]. The local guards are always selected; every
/// other capability answers by its dimensions and its request.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one pass answers every capability, and splitting it would separate a selection from the dimensions that decided it"
)]
pub fn select(params: &Params, availability: &Availability) -> Vec<Selection> {
    let forge = params.forge();
    let driver = params.driver();
    let known_drivers = availability.drivers();
    let driver_known = driver.is_some_and(|d| known_drivers.iter().any(|k| k == d));
    let forge_known = forge.is_some_and(known_forge);

    let mut out = Vec::with_capacity(ALL.len());
    out.push(Selection::selected(GUARDS, Vec::new(), None));

    // The release automation first, because the title gate's root
    // pipeline depends on whether it lands.
    let automation = match (params.release_mode(), driver, forge) {
        (ReleaseMode::External, _, _) => Selection::omitted(
            RELEASE_AUTOMATION,
            Status::NotRequested,
            "the release is external: the target releases through a process release-kit does not drive",
        ),
        (ReleaseMode::None, _, _) => Selection::omitted(
            RELEASE_AUTOMATION,
            Status::NotRequested,
            "the release mode is none",
        ),
        (ReleaseMode::Automatic, None, _) => Selection::omitted(
            RELEASE_AUTOMATION,
            Status::NotApplicable,
            "an automatic release names no driver",
        ),
        (ReleaseMode::Automatic, _, None) => Selection::omitted(
            RELEASE_AUTOMATION,
            Status::NotApplicable,
            "the profile names no forge",
        ),
        (ReleaseMode::Automatic, Some(driver), Some(forge)) => {
            if !driver_known {
                Selection::omitted(
                    RELEASE_AUTOMATION,
                    Status::Unknown,
                    format!(
                        "the driver {driver} is not one this release knows; the bindings are: {}",
                        known_drivers.join(", ")
                    ),
                )
            } else if !forge_known {
                Selection::omitted(
                    RELEASE_AUTOMATION,
                    Status::Unknown,
                    format!(
                        "the forge {forge} is not one this release drives; the forges are: github, gitlab"
                    ),
                )
            } else {
                let sources = availability.owned(driver, forge, RELEASE_AUTOMATION);
                if sources.is_empty() {
                    Selection::omitted(
                        RELEASE_AUTOMATION,
                        Status::Unavailable,
                        format!(
                            "the release automation at ({driver}, {forge}) has no landable files; the available tuples are: {}",
                            availability.automation_tuples().join("; ")
                        ),
                    )
                } else {
                    Selection::selected(RELEASE_AUTOMATION, sources, Some(driver))
                }
            }
        }
    };
    let automation_lands = automation.status == Status::Selected;

    // The title gate: complete where the forge is known.
    out.push(match forge {
        None => Selection::omitted(
            TITLE_CHECK,
            Status::NotApplicable,
            "the profile names no forge",
        ),
        Some(forge) if !forge_known => Selection::omitted(
            TITLE_CHECK,
            Status::Unknown,
            format!(
                "the forge {forge} is not one this release drives; the forges are: github, gitlab"
            ),
        ),
        Some(forge) => {
            let mut sources = availability.owned("_shared", forge, TITLE_CHECK);
            if !automation_lands {
                sources.extend(availability.owned(TITLE_GATE_ZONE, forge, TITLE_CHECK));
            }
            if sources.is_empty() {
                Selection::omitted(
                    TITLE_CHECK,
                    Status::Unavailable,
                    format!("this release ships no title gate for {forge}"),
                )
            } else {
                Selection::selected(TITLE_CHECK, sources, None)
            }
        }
    });

    // The reporting policy.
    out.push(match forge {
        _ if !params.reporting_policy() => Selection::omitted(
            REPORTING_POLICY,
            Status::NotRequested,
            "capabilities.reporting_policy is false",
        ),
        None => Selection::omitted(
            REPORTING_POLICY,
            Status::NotApplicable,
            "the profile names no forge, and the policy names the forge's private channel",
        ),
        Some(forge) if !forge_known => Selection::omitted(
            REPORTING_POLICY,
            Status::Unknown,
            format!(
                "the forge {forge} is not one this release drives; the forges are: github, gitlab"
            ),
        ),
        Some(forge) => {
            let sources = availability.owned("_shared", forge, REPORTING_POLICY);
            if sources.is_empty() {
                Selection::omitted(
                    REPORTING_POLICY,
                    Status::Unavailable,
                    format!("this release ships no reporting policy for {forge}"),
                )
            } else {
                Selection::selected(REPORTING_POLICY, sources, None)
            }
        }
    });

    out.push(automation);

    // The Nix packaging, keyed on the release driver and the forge.
    out.push(match (params.nix_packaging(), driver, forge) {
        (false, _, _) => Selection::omitted(
            PACKAGING_NIX,
            Status::NotRequested,
            "capabilities.nix_packaging is false",
        ),
        (true, None, _) => Selection::omitted(
            PACKAGING_NIX,
            Status::NotApplicable,
            "the seed is keyed on an automatic release driver, and the profile names none",
        ),
        (true, _, None) => Selection::omitted(
            PACKAGING_NIX,
            Status::NotApplicable,
            "the profile names no forge",
        ),
        (true, Some(driver), Some(forge)) => {
            if !driver_known || !forge_known {
                Selection::omitted(
                    PACKAGING_NIX,
                    Status::Unknown,
                    format!("({driver}, {forge}) names a category this release does not know"),
                )
            } else {
                let sources = availability.owned(driver, forge, PACKAGING_NIX);
                if sources.is_empty() {
                    Selection::omitted(
                        PACKAGING_NIX,
                        Status::Unavailable,
                        format!("the {driver} binding ships no Nix seed on {forge}"),
                    )
                } else {
                    Selection::selected(PACKAGING_NIX, sources, Some(driver))
                }
            }
        }
    });

    // The Scorecard workflow, GitHub's alone.
    out.push(match forge {
        _ if !params.scorecard() => Selection::omitted(
            SCORECARD,
            Status::NotRequested,
            "capabilities.scorecard is false",
        ),
        None => Selection::omitted(
            SCORECARD,
            Status::NotApplicable,
            "the profile names no forge",
        ),
        Some(forge) if !forge_known => Selection::omitted(
            SCORECARD,
            Status::Unknown,
            format!(
                "the forge {forge} is not one this release drives; the forges are: github, gitlab"
            ),
        ),
        Some(forge) => {
            let sources = availability.owned("_shared", forge, SCORECARD);
            if sources.is_empty() {
                Selection::omitted(
                    SCORECARD,
                    Status::Unavailable,
                    format!(
                        "the Scorecard workflow is GitHub's alone, and the {forge} zone ships none"
                    ),
                )
            } else {
                Selection::selected(SCORECARD, sources, None)
            }
        }
    });

    // The code scanning workflow, keyed on the driver, the forge, and the
    // provider.
    out.push(match (params.code_scanning(), driver, forge) {
        (None, _, _) => Selection::omitted(
            CODE_SCANNING,
            Status::NotRequested,
            "capabilities.code_scanning is off",
        ),
        (Some(_), None, _) => Selection::omitted(
            CODE_SCANNING,
            Status::NotApplicable,
            "a scanner reads the release driver's language, and the profile names no driver",
        ),
        (Some(_), _, None) => Selection::omitted(
            CODE_SCANNING,
            Status::NotApplicable,
            "the profile names no forge",
        ),
        (Some(provider), Some(driver), Some(forge)) => {
            if !driver_known || !forge_known {
                Selection::omitted(
                    CODE_SCANNING,
                    Status::Unknown,
                    format!("({driver}, {forge}) names a category this release does not know"),
                )
            } else if let Some(reason) = crate::projection::code_scanning_incompatibility(
                Some(provider),
                Some(driver),
                Some(forge),
            ) {
                Selection::omitted(CODE_SCANNING, Status::Unavailable, reason)
            } else {
                let sources: Vec<String> = availability
                    .owned(driver, forge, CODE_SCANNING)
                    .into_iter()
                    .filter(|path| {
                        CODE_SCANNING_DESTINATIONS
                            .iter()
                            .any(|(name, owner)| path.ends_with(name) && *owner == provider)
                    })
                    .collect();
                if sources.is_empty() {
                    Selection::omitted(
                        CODE_SCANNING,
                        Status::Unavailable,
                        format!(
                            "the {driver} binding ships no {} workflow on {forge}",
                            provider.as_str()
                        ),
                    )
                } else {
                    Selection::selected(CODE_SCANNING, sources, Some(driver))
                }
            }
        }
    });
    debug_assert_eq!(out.len(), ALL.len());
    out
}

/// The pin keys one selection matches in the registry's `used_by`: the
/// bare capability id, and the id qualified by the driver where the
/// capability has that dimension.
#[must_use]
pub fn pin_keys(selection: &Selection) -> Vec<String> {
    let mut keys = vec![selection.id.to_owned()];
    if let Some(driver) = &selection.driver {
        keys.push(format!("{}/{driver}", selection.id));
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::{
        ALL, Availability, CODE_SCANNING, GUARDS, PACKAGING_NIX, RELEASE_AUTOMATION,
        REPORTING_POLICY, SCORECARD, Status, TITLE_CHECK, owner_of, select,
    };
    use crate::landing::Params;
    use crate::landing::manifest::{Provider, Style};
    use crate::profile::ReleaseMode;

    fn status_of(selections: &[super::Selection], id: &str) -> Status {
        selections
            .iter()
            .find(|s| s.id == id)
            .expect("every capability answers")
            .status
    }

    /// Every embedded snippet has exactly one owner, and no shared path
    /// falls to the release automation by default.
    #[test]
    fn every_embedded_snippet_has_one_owner() {
        let availability = Availability::embedded();
        assert!(!availability.files().is_empty());
        for path in availability.files() {
            let owner = owner_of(path);
            assert!(owner.is_some(), "{path}: no capability owns it");
            assert!(
                ALL.contains(&owner.unwrap_or_default()),
                "{path}: an unlisted owner"
            );
        }
        assert_eq!(
            owner_of("_shared/github/SECURITY.md"),
            Some(REPORTING_POLICY)
        );
        assert_eq!(
            owner_of("_shared/github/.github/workflows/scorecard.yml"),
            Some(SCORECARD)
        );
        assert_eq!(owner_of("rust/github/flake.nix"), Some(PACKAGING_NIX));
        assert_eq!(
            owner_of("rust/github/.github/workflows/code-scanning-codeql.yml"),
            Some(CODE_SCANNING)
        );
        assert_eq!(
            owner_of("rust/github/release-plz.toml"),
            Some(RELEASE_AUTOMATION)
        );
        assert_eq!(owner_of("_shared/github/unknown.txt"), None);
    }

    /// SATISFIES project-profile:availability-belongs-to-a-capability-at-its-dimensions
    #[test]
    fn the_catalog_answers_availability_per_tuple() {
        let availability = Availability::embedded();
        let mut github = Params::for_test("acme/widget", Some(Style::Trunk));
        github.set_pair_for_test("python", "github");
        let mut gitlab = github.clone();
        gitlab.set_pair_for_test("python", "gitlab");
        let on_github = select(&github, &availability);
        let on_gitlab = select(&gitlab, &availability);
        assert_eq!(status_of(&on_github, RELEASE_AUTOMATION), Status::Selected);
        assert_eq!(
            status_of(&on_gitlab, RELEASE_AUTOMATION),
            Status::Unavailable
        );
        let reason = on_gitlab
            .iter()
            .find(|s| s.id == RELEASE_AUTOMATION)
            .and_then(|s| s.reason.clone())
            .expect("an unavailable capability states why");
        assert!(reason.contains("rust, gitlab"), "{reason}");
        assert!(reason.contains("python, github"), "{reason}");
        // The title gate still lands on GitLab, with the release-less root
        // pipeline beside the fragment.
        let title = on_gitlab
            .iter()
            .find(|s| s.id == TITLE_CHECK)
            .expect("the title gate answers");
        assert_eq!(title.status, Status::Selected);
        assert!(
            title
                .sources
                .iter()
                .any(|s| s == "_title-gate/gitlab/.gitlab-ci.yml"),
            "{:?}",
            title.sources
        );
    }

    /// A no-forge, no-release input selects the local guards alone and
    /// marks every forge capability not applicable.
    #[test]
    fn a_no_forge_input_selects_the_guards_alone() {
        let params = Params::for_test_release_less(&[], None, ReleaseMode::None);
        let selections = select(&params, &Availability::embedded());
        assert_eq!(status_of(&selections, GUARDS), Status::Selected);
        for id in [
            TITLE_CHECK,
            REPORTING_POLICY,
            RELEASE_AUTOMATION,
            PACKAGING_NIX,
        ] {
            assert_ne!(status_of(&selections, id), Status::Selected, "{id}");
        }
        assert_eq!(status_of(&selections, TITLE_CHECK), Status::NotApplicable);
        assert_eq!(
            status_of(&selections, RELEASE_AUTOMATION),
            Status::NotRequested
        );
        assert_eq!(status_of(&selections, PACKAGING_NIX), Status::NotRequested);
    }

    /// An unknown forge keeps the guards and reports the rest unknown.
    #[test]
    fn an_unknown_forge_reads_as_unknown_and_lands_the_guards() {
        let mut params = Params::for_test_release_less(&[], Some("codeberg"), ReleaseMode::None);
        params.set_scorecard_for_test(true);
        let selections = select(&params, &Availability::embedded());
        assert_eq!(status_of(&selections, GUARDS), Status::Selected);
        assert_eq!(status_of(&selections, TITLE_CHECK), Status::Unknown);
        assert_eq!(status_of(&selections, SCORECARD), Status::Unknown);
    }

    /// The scanner follows the provider and the pair, and the reasons
    /// stay the ones resolution refuses with.
    #[test]
    fn code_scanning_selects_the_providers_own_file() {
        let availability = Availability::embedded();
        let mut params = Params::for_test("acme/widget", Some(Style::Trunk));
        params.set_code_scanning_for_test(Some(Provider::Semgrep));
        let selections = select(&params, &availability);
        let scanning = selections
            .iter()
            .find(|s| s.id == CODE_SCANNING)
            .expect("answers");
        assert_eq!(scanning.status, Status::Selected);
        assert_eq!(
            scanning.sources,
            vec!["rust/github/.github/workflows/code-scanning-semgrep.yml".to_owned()]
        );
        params.set_pair_for_test("bash", "github");
        let selections = select(&params, &availability);
        assert_eq!(status_of(&selections, CODE_SCANNING), Status::Unavailable);
    }
}
