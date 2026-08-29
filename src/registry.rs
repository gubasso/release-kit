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
    /// The bindings that use the tool.
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

/// The pins a technology's snippets use, keyed for a landing record.
#[must_use]
pub fn pins_for(tech: &str) -> Vec<Pin> {
    pins()
        .into_iter()
        .filter(|pin| pin.used_by.iter().any(|user| user == tech))
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
            assert!(pin.check.is_some(), "{}: no check URL", pin.name);
        }
    }

    #[test]
    fn pins_filter_by_technology() {
        let rust: Vec<String> = pins_for("rust").into_iter().map(|pin| pin.name).collect();
        assert!(rust.contains(&"release-plz".to_owned()));
        assert!(rust.contains(&"cargo-dist".to_owned()));
        assert!(!rust.contains(&"git-cliff".to_owned()));
    }
}
