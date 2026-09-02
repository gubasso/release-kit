//! The invariants a seeded file still carries.
//!
//! A `seeded` file is the target's to tune — nothing here rewrites one —
//! but the narrow part the invariants own is judged: a target may choose
//! its platforms, its installers, and its install path; it may not choose
//! to ship unattested. The judgment reads the effective configuration,
//! never the text: a commented key, a `false` value, or an unpaired phase
//! must fail, and whitespace or key order must not matter. The table is
//! keyed by `(technology, forge, destination)` — the kind table is
//! destination-keyed, and a second pair sharing a destination would
//! otherwise silently inherit the wrong rule.

use serde::Serialize;

use crate::embedded;

/// One invariant a landed file's effective configuration violates: a
/// stable code, the destination, why, and exactly what to write — the
/// operator is told the remediation, never just what was not found.
#[derive(Debug, Clone, Serialize)]
pub struct InvariantFailure {
    /// The stable machine code of the failed rule.
    pub code: &'static str,
    /// The landed destination the failure is about.
    pub destination: String,
    /// Why the configuration violates the invariant.
    pub reason: String,
    /// Exactly what to write to satisfy the rule.
    pub remediation: &'static str,
}

impl InvariantFailure {
    fn new(
        code: &'static str,
        destination: &str,
        reason: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self {
            code,
            destination: destination.to_owned(),
            reason: reason.into(),
            remediation,
        }
    }
}

/// Judge one landed file against the rules its `(tech, forge,
/// destination)` key owns. A destination no rule owns fails nothing.
#[must_use]
pub fn failures(tech: &str, forge: &str, destination: &str, bytes: &[u8]) -> Vec<InvariantFailure> {
    match (tech, forge, destination) {
        ("rust", "github", "dist-workspace.toml") => dist_workspace(destination, bytes),
        _ => Vec::new(),
    }
}

/// The rust/github attestation configuration: attestations on, minted in
/// the `host` phase where every hosted asset is gathered before the
/// release page exists, the release creation paired with that phase, and
/// no narrowing filter — the default `["*"]` covers every hosted file,
/// where an enumerated list goes quiet when an archive format moves.
fn dist_workspace(destination: &str, bytes: &[u8]) -> Vec<InvariantFailure> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return vec![InvariantFailure::new(
            "unparsable-configuration",
            destination,
            "the file is not UTF-8, so its configuration cannot be judged",
            "repair the file so it parses as TOML",
        )];
    };
    let table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(error) => {
            return vec![InvariantFailure::new(
                "unparsable-configuration",
                destination,
                format!("the file does not parse as TOML: {error}"),
                "repair the file so it parses as TOML",
            )];
        }
    };
    let dist = table.get("dist").and_then(toml::Value::as_table);
    let mut failures = Vec::new();
    let value = |key: &str| dist.and_then(|dist| dist.get(key));
    if value("github-attestations").and_then(toml::Value::as_bool) != Some(true) {
        failures.push(InvariantFailure::new(
            "attestations-disabled",
            destination,
            "github-attestations is not effectively true, so no release artifact is attested",
            "set github-attestations = true in [dist]",
        ));
    }
    let phase = value("github-attestations-phase").and_then(toml::Value::as_str);
    if phase != Some("host") {
        failures.push(InvariantFailure::new(
            "attestation-phase-not-host",
            destination,
            phase.map_or_else(
                || "github-attestations-phase is unset, so the default phase attests only the per-platform archives and the curled installers ship unattested".to_owned(),
                |other| format!(
                    "github-attestations-phase is \"{other}\"; only the host phase attests every asset before the release page exists"
                ),
            ),
            "set github-attestations-phase = \"host\" in [dist]",
        ));
    }
    if value("github-release").and_then(toml::Value::as_str) != Some("host") {
        failures.push(InvariantFailure::new(
            "release-phase-unpaired",
            destination,
            "github-release is not \"host\", leaving the release creation unpaired with the attest phase",
            "set github-release = \"host\" in [dist], pairing the release creation with the phase that attests",
        ));
    }
    if value("github-attestations-filters").is_some() {
        failures.push(InvariantFailure::new(
            "attestation-filters-narrowed",
            destination,
            "github-attestations-filters narrows what is attested below the whole release payload",
            "remove github-attestations-filters from [dist]; the default [\"*\"] attests every hosted file",
        ));
    }
    // The build that signs is itself pinned by digest: the seed's
    // [dist.github-action-commits] table pins the actions cargo-dist
    // injects — the attest step among them — and a landed target must
    // carry the same effective table, or its signer runs code a moved
    // tag can swap.
    let expected = seed_action_commits();
    let found = value("github-action-commits").and_then(toml::Value::as_table);
    for (action, commit) in &expected {
        let remediation = "bring the [dist.github-action-commits] table to the payload seed's (rk snippet rust/github/dist-workspace.toml) and regenerate with dist generate --mode ci";
        // Three distinct states, each with its own true reason: an
        // absent entry falls back to the movable tag, a non-string value
        // is invalid configuration, and a mismatched string executes an
        // immutable commit that is just not the payload's.
        match found.and_then(|table| table.get(action)) {
            Some(value) => match value.as_str() {
                Some(pinned) if pinned == commit.as_str() => {}
                Some(pinned) => failures.push(InvariantFailure::new(
                    "action-commit-stale",
                    destination,
                    format!(
                        "[dist.github-action-commits] pins {action} at {pinned}, where the payload pins {commit}"
                    ),
                    remediation,
                )),
                None => failures.push(InvariantFailure::new(
                    "action-commit-invalid",
                    destination,
                    format!(
                        "[dist.github-action-commits] pins {action} with a non-string value; a pin is a full commit SHA string"
                    ),
                    remediation,
                )),
            },
            None => failures.push(InvariantFailure::new(
                "action-commit-missing",
                destination,
                format!(
                    "[dist.github-action-commits] does not pin {action}, so the workflow runs whatever the movable tag names"
                ),
                remediation,
            )),
        }
    }
    failures
}

/// The action commits the payload's own seed pins, read from the
/// embedded snippet so the judgment and the seed cannot drift apart.
fn seed_action_commits() -> Vec<(String, String)> {
    let Some(text) = embedded::SNIPPETS
        .get_file("rust/github/dist-workspace.toml")
        .and_then(|file| file.contents_utf8())
    else {
        return Vec::new();
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    table
        .get("dist")
        .and_then(toml::Value::as_table)
        .and_then(|dist| dist.get("github-action-commits"))
        .and_then(toml::Value::as_table)
        .map(|commits| {
            commits
                .iter()
                .filter_map(|(action, commit)| {
                    commit
                        .as_str()
                        .map(|commit| (action.clone(), commit.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::failures;

    const CLEAN: &str = r#"
[dist]
github-attestations = true
github-attestations-phase = "host"
github-release = "host"

[dist.github-action-commits]
"actions/checkout" = "d23441a48e516b6c34aea4fa41551a30e30af803"
"actions/download-artifact" = "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c"
"actions/upload-artifact" = "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
"actions/attest" = "1e69f48acb82d1966a394da916b4c1698aa569d6"
"#;

    /// The correct configuration fails nothing, whatever the whitespace
    /// and key order, and the payload's own seed is the exemplar: the
    /// judgment is over the effective TOML, and the seed must satisfy
    /// the rule it seeds.
    #[test]
    fn the_seeded_configuration_is_judged_effectively() {
        assert!(failures("rust", "github", "dist-workspace.toml", CLEAN.as_bytes()).is_empty());
        let seed = crate::embedded::SNIPPETS
            .get_file("rust/github/dist-workspace.toml")
            .and_then(|file| file.contents_utf8())
            .expect("the seed is embedded");
        assert!(
            failures("rust", "github", "dist-workspace.toml", seed.as_bytes()).is_empty(),
            "the payload's own seed satisfies the invariants it seeds"
        );
    }

    /// A missing or stale action-commit table fails: the signer's own
    /// steps would otherwise run whatever a movable tag names.
    #[test]
    fn a_missing_or_stale_action_commit_table_fails() {
        let missing = "[dist]\ngithub-attestations=true\ngithub-attestations-phase='host'\ngithub-release='host'\n";
        let found = failures("rust", "github", "dist-workspace.toml", missing.as_bytes());
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "action-commit-missing"),
            "a missing entry falls back to the movable tag: {found:?}"
        );
        let stale = CLEAN.replace(
            "d23441a48e516b6c34aea4fa41551a30e30af803",
            "0000000000000000000000000000000000000000",
        );
        let found = failures("rust", "github", "dist-workspace.toml", stale.as_bytes());
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "action-commit-stale"
                    && failure.reason.contains("actions/checkout")
                    && failure
                        .reason
                        .contains("0000000000000000000000000000000000000000")),
            "a mismatch names the found and expected commits: {found:?}"
        );
        let invalid = CLEAN.replace("\"d23441a48e516b6c34aea4fa41551a30e30af803\"", "123");
        let found_invalid = failures("rust", "github", "dist-workspace.toml", invalid.as_bytes());
        assert!(
            found_invalid
                .iter()
                .any(|failure| failure.code == "action-commit-invalid"
                    && failure.reason.contains("actions/checkout")),
            "a non-string value is invalid configuration, not an absent pin: {found_invalid:?}"
        );
        assert!(
            !found
                .iter()
                .any(|failure| failure.reason.contains("actions/attest")),
            "only the stale action is named: {found:?}"
        );
    }

    /// Every degraded form fails with its own code: a commented key, a
    /// false value, the default phase, an unpaired release phase, a
    /// narrowing filter, and malformed TOML.
    #[test]
    fn each_degraded_form_fails_with_its_code() {
        let cases: &[(&str, &str)] = &[
            (
                "[dist]\n# github-attestations = true\ngithub-attestations-phase='host'\ngithub-release='host'\n",
                "attestations-disabled",
            ),
            (
                "[dist]\ngithub-attestations = false\ngithub-attestations-phase='host'\ngithub-release='host'\n",
                "attestations-disabled",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-release='host'\n",
                "attestation-phase-not-host",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-attestations-phase='build-local-artifacts'\ngithub-release='host'\n",
                "attestation-phase-not-host",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-attestations-phase='host'\ngithub-release='announce'\n",
                "release-phase-unpaired",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-attestations-phase='host'\ngithub-release='host'\ngithub-attestations-filters=['*.tar.gz']\n",
                "attestation-filters-narrowed",
            ),
            ("not toml at [all", "unparsable-configuration"),
        ];
        for (text, code) in cases {
            let found = failures("rust", "github", "dist-workspace.toml", text.as_bytes());
            assert!(
                found.iter().any(|failure| failure.code == *code),
                "{text:?} must fail with {code}, got {found:?}"
            );
        }
    }

    /// The key is the pair plus the destination: the same bytes under
    /// another pair or another destination fail nothing, so a second pair
    /// sharing a destination cannot silently inherit this rule.
    #[test]
    fn the_rule_is_keyed_by_pair_and_destination() {
        let broken = b"[dist]\ngithub-attestations = false\n";
        assert!(failures("rust", "gitlab", "dist-workspace.toml", broken).is_empty());
        assert!(failures("bash", "github", "dist-workspace.toml", broken).is_empty());
        assert!(failures("rust", "github", "release-plz.toml", broken).is_empty());
    }
}
