//! The pinned-tool registry, parsed from the embedded `versions.toml`.
//!
//! One registry serves three readers: `rk versions` prints it raw, a
//! landing copies the relevant pins into the record, and `rk status`
//! compares a record's pins against it offline. Parsing happens at
//! runtime over the embedded bytes, so what the readers see is
//! necessarily what the binary carries.

use serde::Deserialize;

use crate::embedded;

/// One pinned tool, with the fields the binary's readers use; the
/// registry's prose fields stay in the raw print.
#[derive(Debug, Clone, Deserialize)]
pub struct Pin {
    /// The tool's name, the key a record's `pins` map uses.
    pub name: String,
    /// The pinned version.
    pub version: String,
    /// The workflow reference — `owner/action@ref` — where the tool is a
    /// GitHub Action. The ref here is the discovery ref a freshness check
    /// reads; the commit below is what the workflows execute.
    #[serde(default)]
    pub action: Option<String>,
    /// The immutable execution commit the workflows pin, where the tool
    /// is an action.
    #[serde(default)]
    pub commit: Option<String>,
    /// How the discovery ref moves: a moving major or minor tag, an
    /// exact tag, or a maintained branch. Movement is an update signal,
    /// never evidence of an attack.
    #[serde(default)]
    pub ref_class: Option<String>,
    /// The capabilities that use the tool: a capability id, the id
    /// qualified by the release driver as `release.automation/rust` where
    /// the capability has that dimension, or that qualified again by the
    /// provider as `supply-chain.code-scanning/rust/codeql` where its
    /// parameter names one. A pin declares the narrowest of the three it
    /// can, so a target records the tools its own landing runs.
    #[serde(default)]
    pub used_by: Vec<String>,
    /// The URL a freshness check queries, where one exists.
    #[serde(default)]
    pub check: Option<String>,
}

/// The registry's parsed shape; only the fields named here are read.
#[derive(Debug, Deserialize)]
struct Registry {
    /// Every `[[tool]]` entry.
    tool: Vec<Pin>,
}

/// Every pin the embedded registry declares, in authored order.
///
/// The embedded registry is authored in this repository and held valid by
/// a test, so a parse failure is a build defect; this resolves it to an
/// empty list rather than panicking, and the test is what catches it.
#[must_use]
pub fn pins() -> Vec<Pin> {
    parse(embedded::VERSIONS)
}

/// The pins the selected capabilities use, keyed for a landing record.
///
/// Every pin whose `used_by` names a selected capability's id, bare or
/// qualified by its driver and its provider, in authored order and each
/// once.
#[must_use]
pub fn pins_for(selected: &[crate::profile::catalog::Selection]) -> Vec<Pin> {
    let keys: Vec<String> = selected
        .iter()
        .filter(|selection| selection.lands())
        .flat_map(crate::profile::catalog::pin_keys)
        .collect();
    pins()
        .into_iter()
        .filter(|pin| pin.used_by.iter().any(|user| keys.contains(user)))
        .collect()
}

/// The pinned version of one tool, where the registry names it.
#[must_use]
pub fn version_of(name: &str) -> Option<String> {
    pins()
        .into_iter()
        .find(|pin| pin.name == name)
        .map(|pin| pin.version)
}

fn parse(text: &str) -> Vec<Pin> {
    toml::from_str::<Registry>(text)
        .map(|registry| registry.tool)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{pins, pins_for};

    /// The embedded registry parses, and every entry carries the fields
    /// the readers depend on; a `versions.toml` edit that breaks parsing
    /// fails here instead of silently emptying every reader.
    #[test]
    fn the_embedded_registry_parses_with_every_field() {
        let pins = pins();
        assert!(!pins.is_empty(), "the registry parsed to nothing");
        for pin in &pins {
            assert!(!pin.version.is_empty(), "{}: no version", pin.name);
            assert!(!pin.used_by.is_empty(), "{}: no used_by", pin.name);
            assert!(
                pin.check.is_some() || (pin.action.is_some() && pin.commit.is_some()),
                "{}: no check URL and no ref to resolve",
                pin.name
            );
        }
    }

    /// Every `used_by` entry names a capability this binary catalogs,
    /// qualified by a driver the sources know and a provider the landing
    /// parameter admits, where it carries either.
    #[test]
    fn every_used_by_entry_names_a_catalogued_capability() {
        let drivers = crate::profile::catalog::known_drivers();
        for pin in pins() {
            for user in &pin.used_by {
                let mut parts = user.split('/');
                let id = parts.next().unwrap_or_default();
                let driver = parts.next();
                let provider = parts.next();
                assert!(
                    parts.next().is_none(),
                    "{}: used_by names {user}, which carries more than a capability, a driver, and a provider",
                    pin.name
                );
                assert!(
                    crate::profile::catalog::ALL.contains(&id),
                    "{}: used_by names {user}, which is no capability",
                    pin.name
                );
                if let Some(driver) = driver {
                    assert!(
                        drivers.iter().any(|known| known == driver),
                        "{}: used_by names the driver {driver}, which no binding ships",
                        pin.name
                    );
                }
                if let Some(provider) = provider {
                    assert!(
                        crate::landing::manifest::Provider::parse(provider)
                            .is_ok_and(|parsed| parsed.is_some()),
                        "{}: used_by names the provider {provider}, which no parameter admits",
                        pin.name
                    );
                }
            }
        }
    }

    /// A pin keyed on one provider reaches that provider's landing and no
    /// other, so a target records the tools its own workflow runs.
    #[test]
    fn a_provider_keyed_pin_reaches_that_provider_alone() {
        let names = |provider| {
            let mut params =
                crate::landing::Params::for_test("acme/widget", Some(crate::landing::Style::Trunk));
            params.set_code_scanning_for_test(Some(provider));
            let selected = crate::profile::catalog::select(
                &params,
                &crate::profile::catalog::Availability::embedded(),
            );
            pins_for(&selected)
                .into_iter()
                .map(|pin| pin.name)
                .collect::<Vec<String>>()
        };
        let codeql = names(crate::landing::manifest::Provider::CodeQl);
        let semgrep = names(crate::landing::manifest::Provider::Semgrep);
        assert!(
            codeql.iter().any(|name| name == "codeql-analyze"),
            "{codeql:?}"
        );
        assert!(
            !codeql.iter().any(|name| name == "semgrep-image"),
            "a codeql landing records no semgrep tool: {codeql:?}"
        );
        assert!(
            semgrep.iter().any(|name| name == "semgrep-image"),
            "{semgrep:?}"
        );
        assert!(
            !semgrep.iter().any(|name| name == "codeql-init"),
            "a semgrep landing records no codeql tool: {semgrep:?}"
        );
        for pins in [&codeql, &semgrep] {
            assert!(
                pins.iter().any(|name| name == "checkout"),
                "either provider checks out: {pins:?}"
            );
        }
    }

    #[test]
    fn pins_filter_by_selected_capability() {
        let params =
            crate::landing::Params::for_test("acme/widget", Some(crate::landing::Style::Trunk));
        let selected = crate::profile::catalog::select(
            &params,
            &crate::profile::catalog::Availability::embedded(),
        );
        let rust: Vec<String> = pins_for(&selected)
            .into_iter()
            .map(|pin| pin.name)
            .collect();
        assert!(rust.contains(&"release-plz".to_owned()));
        assert!(rust.contains(&"cargo-dist".to_owned()));
        assert!(rust.contains(&"conventional-pre-commit".to_owned()));
        assert!(!rust.contains(&"git-cliff".to_owned()));
        assert!(!rust.contains(&"scorecard-action".to_owned()));
        // A guards-only target records the hook pins and nothing else.
        let guards = crate::landing::Params::for_test_release_less(
            &[],
            None,
            crate::profile::ReleaseMode::None,
        );
        let selected = crate::profile::catalog::select(
            &guards,
            &crate::profile::catalog::Availability::embedded(),
        );
        let names: Vec<String> = pins_for(&selected)
            .into_iter()
            .map(|pin| pin.name)
            .collect();
        assert_eq!(names, ["conventional-pre-commit", "pre-commit-hooks"]);
    }
}
