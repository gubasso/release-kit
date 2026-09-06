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
//!
//! A second, pair-keyed table judges what a landed file generates and the
//! payload ships no copy of: no digest records such a file, so nothing
//! else sees it drift away from the configuration it was generated from.
//! That judgment reads the generated text, because the generator is not
//! available to re-run and the text is what the forge executes. It reads
//! the grammar the generator writes and reports what it cannot resolve;
//! a workflow hand-authored in some further YAML presentation is beyond a
//! text reader, and the generator's own check stays the whole-file proof.

use camino::Utf8Path;
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

/// The generated file the cross-file failures name: cargo-dist writes it
/// from `dist-workspace.toml`, the payload ships no copy, and the forge
/// executes it.
const GENERATED_WORKFLOW: &str = ".github/workflows/release.yml";

/// Judge what a landed file generates, keyed by `(technology, forge)`.
///
/// A destination-keyed rule cannot reach such a file: the payload ships
/// no copy and nothing records it, so no digest sees it drift away from
/// the configuration it was generated from. Both files are read off the
/// target's own disk.
#[must_use]
pub fn target_failures(tech: &str, forge: &str, target: &Utf8Path) -> Vec<InvariantFailure> {
    match (tech, forge) {
        ("rust", "github") => generated_release_workflow(target),
        _ => Vec::new(),
    }
}

/// The pair's one generated file. Either file absent reports nothing:
/// `rk init` lands the configuration and writes no workflow, the operator
/// generates it afterwards, and a missing `dist-workspace.toml` is already
/// the record's own `missing` line. An absence is the generator's story.
fn generated_release_workflow(target: &Utf8Path) -> Vec<InvariantFailure> {
    let Ok(config) = std::fs::read_to_string(target.join("dist-workspace.toml")) else {
        return Vec::new();
    };
    let workflow = match std::fs::read_to_string(target.join(GENERATED_WORKFLOW)) {
        Ok(text) => text,
        // Absence alone is silent. A file that is there and cannot be
        // read as text is not an absent one, and a run that cannot read
        // what the forge executes has not judged it.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            return vec![InvariantFailure::new(
                "workflow-file-unreadable",
                GENERATED_WORKFLOW,
                format!("the workflow is present and cannot be read as text: {error}"),
                "repair the file so it reads as UTF-8 text, or regenerate it with dist generate --mode ci",
            )];
        }
    };
    workflow_matches_configuration(&config, &workflow)
}

/// The judgment, over the workflow's own text: `dist` is not available to
/// regenerate and diff — the binding states the devshell carries none —
/// and the text is what the forge executes. It reads in one direction,
/// from the workflow to the configuration: a pin the configuration
/// carries and the workflow never runs is the target's own tuning of its
/// installers and its platforms, not drift. Every reference the workflow
/// executes must be immutable whatever the configuration says about it,
/// because a table entry naming a movable tag pins nothing.
fn workflow_matches_configuration(config: &str, workflow: &str) -> Vec<InvariantFailure> {
    let Ok(table) = config.parse::<toml::Table>() else {
        return Vec::new();
    };
    let dist = table.get("dist").and_then(toml::Value::as_table);
    let pinned = dist
        .and_then(|dist| dist.get("github-action-commits"))
        .and_then(toml::Value::as_table);
    let attested = dist
        .and_then(|dist| dist.get("github-attestations"))
        .and_then(toml::Value::as_bool)
        == Some(true);

    let mut failures = Vec::new();
    let steps = workflow_uses(workflow);
    for step in &steps {
        let (action, reference) = match step {
            // A value this reader cannot resolve is never a pass: an
            // alias, an unfamiliar shape, or a form a later generator
            // emits would otherwise erase a real step from the judgment.
            Step::Opaque(value) => {
                failures.push(InvariantFailure::new(
                    "workflow-step-unreadable",
                    GENERATED_WORKFLOW,
                    format!(
                        "the workflow runs `uses: {value}`, which this check cannot resolve into an action and an immutable reference"
                    ),
                    "write the step as <action>@<full commit SHA>, resolving any alias, so what the workflow runs can be read; regenerating with dist generate --mode ci writes that form",
                ));
                continue;
            }
            Step::Action(action, reference) => (action, reference),
        };
        // A non-string table entry pins nothing, and neither does a
        // string that is not itself immutable, so both fall through to
        // the reference check rather than blessing the step.
        let pin = pinned
            .and_then(|table| table.get(action.as_str()))
            .and_then(toml::Value::as_str);
        if let Some(commit) = pin
            && commit != reference
        {
            failures.push(InvariantFailure::new(
                "workflow-action-stale",
                GENERATED_WORKFLOW,
                format!(
                    "the workflow runs {action}@{reference}, where dist-workspace.toml pins {commit}"
                ),
                "regenerate the workflow from the configuration with dist generate --mode ci and commit it; a hand edit is reverted at the next generate",
            ));
            continue;
        }
        if !is_immutable(reference) {
            failures.push(InvariantFailure::new(
                "workflow-action-unpinned",
                GENERATED_WORKFLOW,
                format!(
                    "the workflow runs {action}@{reference}, which is no immutable reference, so the step runs whatever that name points at today"
                ),
                "pin the action at a full commit SHA in [dist.github-action-commits] in dist-workspace.toml, then regenerate with dist generate --mode ci",
            ));
        }
    }
    if attested
        && !steps.iter().any(|step| match step {
            Step::Action(action, _) => {
                action == "actions/attest" || action.starts_with("actions/attest-")
            }
            Step::Opaque(_) => false,
        })
    {
        failures.push(InvariantFailure::new(
            "workflow-attestation-missing",
            GENERATED_WORKFLOW,
            "dist-workspace.toml sets github-attestations = true, and the workflow carries no attest step, so what this workflow builds ships unattested",
            "regenerate the workflow with dist generate --mode ci and commit it, so the configured attest step is what runs",
        ));
    }
    failures
}

/// One `uses:` value the workflow carries.
enum Step {
    /// The judgeable form: an action and the reference it runs at.
    Action(String, String),
    /// A value this text reader cannot resolve into the pair — a YAML
    /// alias, which GitHub Actions has accepted since September 2025, or
    /// any shape a later generator emits. Carried rather than dropped,
    /// because a step nobody can read is not a step nobody runs.
    Opaque(String),
}

/// Every distinct `uses:` value the workflow carries.
///
/// A step is read in either YAML style, block or flow. A commented line
/// and a value the reader can prove is same-repository —
/// the workspace-relative `./` form and the `$/` self-repository form,
/// which resolves to the running commit — are no movable external
/// reference. Everything else is carried, including a key whose value
/// sits on another line: a trailing comment and surrounding quotes are
/// stripped, so the readable tag kept beside a commit does not read as
/// part of it. One value is reported once however many jobs run it: the
/// operator fixes the pin, not the steps.
fn workflow_uses(workflow: &str) -> Vec<Step> {
    let mut seen: Vec<String> = Vec::new();
    let mut steps = Vec::new();
    for fragment in workflow.lines().flat_map(line_fragments) {
        let fragment = fragment.trim_start();
        // A step may or may not open its list item on the same fragment.
        let fragment = fragment
            .strip_prefix("- ")
            .map_or(fragment, str::trim_start);
        let Some(rest) = uses_value(fragment) else {
            continue;
        };
        let rest = before_comment(rest).trim();
        let rest = rest
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .or_else(|| {
                rest.strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
            })
            .unwrap_or(rest);
        if rest.starts_with("./") || rest.starts_with("$/") {
            continue;
        }
        if seen.iter().any(|value| value == rest) {
            continue;
        }
        seen.push(rest.to_owned());
        steps.push(match rest.split_once('@') {
            Some((action, reference)) => Step::Action(action.to_owned(), reference.to_owned()),
            // An empty value is a scalar continued on a later line, which
            // this line reader does not follow, so it is unreadable
            // rather than absent.
            None if rest.is_empty() => Step::Opaque("a value carried on another line".to_owned()),
            None => Step::Opaque(rest.to_owned()),
        });
    }
    steps
}

/// One line's mapping fragments.
///
/// A step is a mapping in either YAML style. A block line is one
/// fragment, keeping every comma its scalar carries, because a git ref
/// may hold one and splitting there would read a movable ref as the
/// immutable prefix of itself. A line whose item opens a flow collection,
/// or that carries a `uses` key beside a brace, is its delimiters apart —
/// except where it also carries a quote, which can hold a delimiter
/// inside a scalar: the reader does not guess there, and hands on a
/// stand-in that reads as a step it cannot resolve. Every other braced
/// line is an expression in some other key's value, never a step.
fn line_fragments(line: &str) -> Vec<&str> {
    let item = line.trim_start();
    let item = item.strip_prefix("- ").map_or(item, str::trim_start);
    let flow = item.starts_with('{')
        || item.starts_with('[')
        || ((line.contains('{') || line.contains('[')) && line.contains("uses"));
    if !flow {
        return vec![line];
    }
    if line.contains(QUOTES) {
        return vec![UNSPLITTABLE_FLOW_LINE];
    }
    line.split(['{', '}', '[', ']', ',']).collect()
}

/// The scalar before its comment. A hash opens a YAML comment only where
/// a space precedes it, and a git ref may carry one, so the readable tag
/// kept beside a commit is stripped while `<sha>#dev` stays whole.
fn before_comment(value: &str) -> &str {
    let mut previous = ' ';
    for (index, character) in value.char_indices() {
        if character == '#' && (previous == ' ' || previous == '\t') {
            return &value[..index];
        }
        previous = character;
    }
    value
}

/// The two quote characters a YAML scalar is written with, named by code
/// point because the artifact-body scan reads a lone quote in these
/// sources as a literal opening.
const QUOTES: [char; 2] = ['\u{22}', '\u{27}'];

/// The stand-in a quoted flow line becomes: it names no action, so the
/// judgment reads it as a step it cannot resolve.
const UNSPLITTABLE_FLOW_LINE: &str = "uses: a flow-style step carrying a quoted value";

/// The value of a `uses` mapping key, however the key is spelled: bare or
/// quoted, as YAML permits for any implicit key, and padded before its
/// colon. A key whose name merely starts with `uses` is not this key.
fn uses_value(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("\"uses\"")
        .or_else(|| line.strip_prefix("'uses'"))
        .or_else(|| line.strip_prefix("uses"))?;
    rest.trim_start().strip_prefix(':')
}

/// An immutable execution reference: a full commit SHA, or the image
/// digest a `docker://` step pins, which no tag move can swap.
fn is_immutable(reference: &str) -> bool {
    let digest = reference
        .strip_prefix("sha256:")
        .filter(|digest| digest.len() == 64);
    let commit = Some(reference).filter(|reference| reference.len() == 40);
    digest
        .or(commit)
        .is_some_and(|value| value.chars().all(|char| char.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use camino::Utf8Path;

    use super::{failures, target_failures, workflow_matches_configuration};

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

    /// A workflow generated from the clean configuration fails nothing.
    /// Both indentations cargo-dist emits parse, a commented line is no
    /// step, and a readable tag kept beside a commit is not the commit.
    #[test]
    fn the_generated_workflow_at_the_configured_commits_fails_nothing() {
        let workflow = "\
jobs:
  plan:
    steps:
      - uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803
      # - uses: actions/checkout@v4
      - name: Upload
        uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
      - name: Attest
        uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v3
";
        let found = workflow_matches_configuration(CLEAN, workflow);
        assert!(
            found.is_empty(),
            "the generated workflow is clean: {found:?}"
        );
    }

    /// A workflow left behind by a configuration change fails, whether
    /// the reference it kept is a movable tag or a superseded commit: the
    /// forge executes the workflow, never the configuration.
    #[test]
    fn a_workflow_left_at_a_movable_tag_fails() {
        for stale in ["v4", "0000000000000000000000000000000000000000"] {
            let workflow = format!(
                "steps:\n  - uses: actions/checkout@{stale}\n  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n"
            );
            let found = workflow_matches_configuration(CLEAN, &workflow);
            assert!(
                found
                    .iter()
                    .any(|failure| failure.code == "workflow-action-stale"
                        && failure.destination == ".github/workflows/release.yml"
                        && failure.reason.contains("actions/checkout")
                        && failure.reason.contains(stale)
                        && failure
                            .reason
                            .contains("d23441a48e516b6c34aea4fa41551a30e30af803")),
                "{stale} names both sides of the disagreement: {found:?}"
            );
        }
        let twice = "steps:\n  - uses: actions/checkout@v4\n  - uses: actions/checkout@v4\n  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        assert_eq!(
            workflow_matches_configuration(CLEAN, twice).len(),
            1,
            "one reference is one failure, however many jobs run it"
        );
    }

    /// A configured attestation the workflow does not carry fails: the
    /// configuration is the only place the operator reads, and the
    /// release ships unattested. The build-provenance variant satisfies
    /// it, so a cargo-dist version that renames the step is no failure.
    #[test]
    fn a_configured_attestation_with_no_attest_step_fails() {
        let bare = "steps:\n  - uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803\n";
        let found = workflow_matches_configuration(CLEAN, bare);
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "workflow-attestation-missing"),
            "an unattested workflow fails: {found:?}"
        );
        assert!(
            !found
                .iter()
                .any(|failure| failure.code.starts_with("workflow-action-")),
            "the pinned step itself is clean: {found:?}"
        );
        let variant = format!(
            "{bare}  - uses: actions/attest-build-provenance@1e69f48acb82d1966a394da916b4c1698aa569d6\n"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &variant).is_empty(),
            "the build-provenance variant is an attest step"
        );
    }

    /// A reference is judged for itself: an action the configuration
    /// pins with a movable tag, or with a non-string value, fails just as
    /// one the configuration never names, because a table entry naming a
    /// tag pins nothing. A pin the workflow never runs fails nothing:
    /// which actions cargo-dist emits follows the target's own installers
    /// and platforms, which a seeded file leaves the target to tune.
    #[test]
    fn a_movable_reference_fails_whatever_the_configuration_says() {
        let attest = "  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        let movable = format!("steps:\n  - uses: third/party@v1\n{attest}");
        let found = workflow_matches_configuration(CLEAN, &movable);
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("third/party")),
            "an action the configuration never names fails: {found:?}"
        );
        // The configuration agreeing with the movable tag is the hole
        // this case exists to hold shut.
        let agreed = CLEAN.replace(
            "[dist.github-action-commits]",
            "[dist.github-action-commits]\n\"third/party\" = \"v1\"",
        );
        let found = workflow_matches_configuration(&agreed, &movable);
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("third/party")),
            "a table entry naming the same movable tag pins nothing: {found:?}"
        );
        let non_string = CLEAN.replace(
            "[dist.github-action-commits]",
            "[dist.github-action-commits]\n\"third/party\" = 1",
        );
        assert!(
            workflow_matches_configuration(&non_string, &movable)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"),
            "a non-string entry pins nothing either"
        );
        let pinned = format!(
            "steps:\n  - uses: third/party@1111111111111111111111111111111111111111\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &pinned).is_empty(),
            "a commit-pinned action the configuration does not name is the target's own"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &format!("steps:\n{attest}")).is_empty(),
            "a pin no step runs is the target's tuning, not drift"
        );
    }

    /// Every real `uses:` shape reaches the judgment: a padded or quoted
    /// key, a flow mapping, a compact flow sequence, a ref carrying a
    /// comma or a hash. A local `./` or `$/` step is the repository's own
    /// file at the running commit, and a `docker://` image pinned by
    /// digest is immutable.
    #[test]
    fn every_real_step_shape_reaches_the_judgment() {
        let attest = "  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        let padded = format!("steps:\n  - uses : actions/checkout@v4\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &padded)
                .iter()
                .any(|failure| failure.code == "workflow-action-stale"),
            "a padded key is the same mapping"
        );
        let quoted = format!("steps:\n  - \"uses\": actions/checkout@v4\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &quoted)
                .iter()
                .any(|failure| failure.code == "workflow-action-stale"),
            "a quoted key is the same mapping"
        );
        let flow =
            format!("steps:\n  - {{ uses: actions/checkout@v4, with: {{ ref: main }} }}\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &flow)
                .iter()
                .any(|failure| failure.code == "workflow-action-stale"),
            "a flow-style step is the same mapping"
        );
        // A git ref may carry a comma, so a block line is never split on
        // one: the immutable-looking prefix is not the reference.
        let comma = format!(
            "steps:\n  - uses: third/party@1111111111111111111111111111111111111111,dev\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &comma)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains(",dev")),
            "the whole reference is judged, never its prefix"
        );
        // A hash opens a comment only after a space, and a git ref may
        // carry one, so the reference is never read as its prefix.
        let hashed = format!(
            "steps:\n  - uses: third/party@1111111111111111111111111111111111111111#dev\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &hashed)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("#dev")),
            "an adjacent hash is scalar content, not a comment"
        );
        // A compact single-pair mapping is a flow-sequence entry.
        let compact = format!("steps: [ uses: third/party@v1 ]\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &compact)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("third/party")),
            "a compact flow sequence carries its uses key"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &format!("steps:\n  - usesful: no\n{attest}"))
                .is_empty(),
            "a key that merely starts with uses is another key"
        );
        for same_repository in ["./.github/actions/build", "$/.github/actions/build"] {
            let local = format!("steps:\n  - uses: {same_repository}\n{attest}");
            assert!(
                workflow_matches_configuration(CLEAN, &local).is_empty(),
                "{same_repository} is the repository's own file at the running commit"
            );
        }
        let tagged = format!("steps:\n  - uses: docker://alpine:3.8\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &tagged)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"),
            "a docker image with no digest is not immutable"
        );
        let digested = format!(
            "steps:\n  - uses: docker://alpine@sha256:0000000000000000000000000000000000000000000000000000000000000000\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &digested).is_empty(),
            "a docker image pinned by digest is immutable"
        );
    }

    /// A value the reader cannot resolve is reported rather than passed.
    /// GitHub Actions accepts YAML aliases, a scalar may continue on
    /// another line, and a quote may hold a flow delimiter the reader
    /// would otherwise split on: a step nobody can read is not a step
    /// nobody runs. A braced expression in another key's value is no step
    /// at all, and a generated workflow is full of them.
    #[test]
    fn a_step_the_reader_cannot_resolve_is_reported() {
        let attest = "  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        let aliased = format!("steps:\n  - uses: *checkout\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &aliased)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"
                    && failure.reason.contains("*checkout")),
            "an alias is unreadable, never clean"
        );
        let continued = format!("steps:\n  - uses:\n      actions/checkout@v4\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &continued)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"),
            "a value on another line is unreadable, never clean"
        );
        let quoted_flow = format!("steps:\n  - {{ uses: \"third/party@1,dev\" }}\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &quoted_flow)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"),
            "a quoted flow line is not split on a guess"
        );
        let expression = format!(
            "jobs:\n  host:\n    if: ${{{{ fromJson(needs.plan.outputs.val).ci != null && x == 'true' }}}}\n    steps:\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &expression).is_empty(),
            "an expression is not a step this reader cannot resolve"
        );
    }

    /// The cross-file judgment needs both files and its own pair. Either
    /// one absent reports nothing: release-kit writes neither the
    /// workflow nor a record of it, so an absence is the generator's
    /// story and a fresh landing never fails on its first day.
    #[test]
    fn the_cross_file_judgment_needs_both_files() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let target = Utf8Path::from_path(dir.path()).expect("a utf-8 path");
        let broken = "steps:\n  - uses: actions/checkout@v4\n";
        assert!(
            target_failures("rust", "github", target).is_empty(),
            "an empty target"
        );
        std::fs::write(target.join("dist-workspace.toml"), CLEAN).expect("the configuration");
        assert!(
            target_failures("rust", "github", target).is_empty(),
            "a configuration with no generated workflow"
        );
        std::fs::create_dir_all(target.join(".github/workflows")).expect("the workflow directory");
        std::fs::write(target.join(".github/workflows/release.yml"), broken).expect("the workflow");
        assert!(
            !target_failures("rust", "github", target).is_empty(),
            "both files present, and they disagree"
        );
        for (tech, forge) in [("rust", "gitlab"), ("bash", "github")] {
            assert!(
                target_failures(tech, forge, target).is_empty(),
                "{tech}/{forge} generates no artifact workflow"
            );
        }
        // A workflow that is there and cannot be read as text is not an
        // absent one: only absence is silent.
        std::fs::write(
            target.join(".github/workflows/release.yml"),
            [0x66, 0xff, 0xfe],
        )
        .expect("the workflow");
        assert!(
            target_failures("rust", "github", target)
                .iter()
                .any(|failure| failure.code == "workflow-file-unreadable"),
            "a present workflow that does not read as text is reported"
        );
        std::fs::remove_file(target.join("dist-workspace.toml")).expect("the configuration");
        assert!(
            target_failures("rust", "github", target).is_empty(),
            "a workflow with no configuration to judge it against"
        );
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
