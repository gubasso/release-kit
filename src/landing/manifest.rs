//! The landing record: `.release-kit/manifest.json`.
//!
//! The record is a manifest, not a stamp: `rk status` and `rk upgrade`
//! make decisions from it, so it earns a parser that can fail and a
//! stated schema version — an unknown shape refuses naming the record,
//! never a best-effort read. It is written last, after every file has
//! landed, through the temp-plus-rename writer, and it is committed:
//! every reader it exists for sees only committed files, and it carries
//! digests of committed files, nothing secret and nothing
//! machine-specific.

use std::collections::BTreeMap;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::atomic;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::Kind;

/// Where the record lives, relative to the target root.
pub const MANIFEST_PATH: &str = ".release-kit/manifest.json";

/// The schema this binary writes.
///
/// It also reads schema 1 — the pre-mode record, whose absent `workflow`
/// parameter reads as `branches` — schema 2 — the pre-style record,
/// whose absent `style` parameter reads as none and holds an upgrade
/// until `--style` names one — and schema 3 — the pre-nix record, whose
/// absent `nix` parameter reads as opt-out, so an existing target's
/// upgrade never sprouts files nobody requested — and refuses anything
/// else by name.
pub const SCHEMA_VERSION: u64 = 4;

/// The oldest schema this binary still reads.
const OLDEST_READABLE_SCHEMA: u64 = 1;

/// The working-copy mode a landing records: a project decision, rendered
/// into the landed blocks and changed only through the landing verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Workflow {
    /// Every code-changing branch lives in a linked worktree and the main
    /// checkout commits nothing.
    Worktree,
    /// Branches are worked in the main checkout; worktrees stay available
    /// beside them and nothing refuses either form.
    Branches,
}

impl Workflow {
    /// The flag, wire, and report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Worktree => "worktree",
            Self::Branches => "branches",
        }
    }

    /// Parse a `--workflow` flag value.
    ///
    /// # Errors
    ///
    /// Returns [`RkError::Usage`] naming the two values.
    pub fn parse(raw: &str) -> Result<Self, RkError> {
        match raw {
            "worktree" => Ok(Self::Worktree),
            "branches" => Ok(Self::Branches),
            other => Err(RkError::Usage(format!(
                "unknown workflow '{other}'; the modes are: worktree, branches"
            ))),
        }
    }
}

/// The serde default for a record from before the parameter existed.
const fn workflow_branches() -> Workflow {
    Workflow::Branches
}

/// The release style a landing records.
///
/// Whether the bot's release request stands armed to merge itself: a
/// project decision, rendered into the landed release workflow and
/// changed only through the landing verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// The trunk style: the release request carries auto-merge from
    /// creation, so a green trunk ships itself.
    Trunk,
    /// The lines style: every request waits for a human's merge, because
    /// a line's candidate is validated by hand.
    Lines,
}

impl Style {
    /// The flag, wire, and report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trunk => "trunk",
            Self::Lines => "lines",
        }
    }

    /// Parse a `--style` flag value.
    ///
    /// # Errors
    ///
    /// Returns [`RkError::Usage`] naming the two values.
    pub fn parse(raw: &str) -> Result<Self, RkError> {
        match raw {
            "trunk" => Ok(Self::Trunk),
            "lines" => Ok(Self::Lines),
            other => Err(RkError::Usage(format!(
                "unknown style '{other}'; the styles are: trunk, lines"
            ))),
        }
    }
}

/// The record a landing writes and every target-side verb reads.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    /// An integer this binary either knows or refuses on.
    pub schema_version: u64,
    /// The binary that produced the landing.
    pub rk_version: String,
    /// The aggregate payload digest from `rk payload`: which payload
    /// actually landed, where the version alone is ambiguous.
    pub payload_sha256: Digest,
    /// `init` or `adopt` — how the record came to exist.
    pub origin: String,
    /// The technology that selected the payload.
    pub tech: String,
    /// The forge that selected the payload.
    pub forge: String,
    /// When the first landing happened; an upgrade preserves it.
    pub landed_at: String,
    /// Every value substituted into a `rendered` file, so a re-render is
    /// reproducible without asking again.
    pub parameters: Parameters,
    /// Every landed destination with its kind and digests.
    pub files: Vec<FileRecord>,
    /// The registry pins the landed technology uses, copied at landing
    /// time; `rk status` compares them offline.
    pub pins: BTreeMap<String, String>,
}

/// The landing parameters, recorded whole.
#[derive(Debug, Serialize, Deserialize)]
pub struct Parameters {
    /// The project path on the forge, recorded whole because a GitLab
    /// project may nest below its group.
    pub repo: String,
    /// The Conventional Commit scopes the project accepts, rendered into
    /// the title checks, the commit hook, and the routing block. Defaults
    /// empty for a record from before the parameter existed; an upgrade of
    /// such a record asks for `--scopes` once and records the answer.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// The working-copy mode the project chose: every code-changing branch
    /// in a linked worktree (`worktree`), or branches worked in the main
    /// checkout with worktrees optional beside them (`branches`). A record
    /// predating the field reads as `branches`, so an upgrade never imposes
    /// a guard the project did not choose.
    #[serde(default = "workflow_branches")]
    pub workflow: Workflow,
    /// The release style the project chose: the bot's request armed to
    /// merge itself (`trunk`), or every merge a human's (`lines`). A
    /// record predating the field carries none, and an upgrade refuses
    /// until `--style` names one: neither value is a compatibility-safe
    /// reading of a target nobody asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    /// Whether the landing carries the Nix capability: the seeded package
    /// expression, the flake pair where the target had none, and the
    /// workflow that proves the build. A record predating the field reads
    /// as opt-out, so an upgrade adds nothing unrequested; the projection
    /// stays reproducible from the record because this field is part of
    /// it.
    #[serde(default)]
    pub nix: bool,
}

/// One landed destination.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileRecord {
    /// The destination, relative to the target root.
    pub destination: String,
    /// The declared ownership kind.
    pub kind: Kind,
    /// The digest of what was written — after substitution for a
    /// `rendered` file, of the marked block for `AGENTS.md`.
    pub sha256: Digest,
    /// The digest of the bytes this file's comparisons start from — what
    /// makes the three-way comparison at upgrade possible. For a
    /// `rendered` file, the payload as it stood at landing, before
    /// substitution; for a `seeded` file, the starting point the target
    /// tunes away from — the seeding payload, or, where a later payload
    /// reclassified the file from `rendered`, the rendered bytes
    /// release-kit last wrote. Absent for `state` files, which are never
    /// compared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_sha256: Option<Digest>,
}

impl Manifest {
    /// The recorded entry for one destination, where the record names it.
    #[must_use]
    pub fn file(&self, destination: &str) -> Option<&FileRecord> {
        self.files
            .iter()
            .find(|file| file.destination == destination)
    }
}

/// Read the record at `target`, or `None` where no landing exists.
///
/// # Errors
///
/// The record's stated failure taxonomy: an unreadable record is a
/// refusal naming it, a record at an unknown `schema_version` is a
/// refusal naming the record, and one that does not parse at a known
/// schema is a defect-class failure.
pub fn load(target: &Utf8Path) -> Result<Option<Manifest>, RkError> {
    let path = target.join(MANIFEST_PATH);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(RkError::refusal(
                Diagnostic::new(Reason::Io, format!("cannot read {path}: {e}"))
                    .expected("a readable landing record")
                    .target_state("unchanged"),
            ));
        }
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| anyhow::anyhow!("{path} is not a landing record: {e}"))?;
    // Schema 1 is the pre-mode record: it parses through the same
    // `Parameters`, whose serde default reads the absent `workflow` as
    // `branches`. Anything past this binary's schema refuses by name —
    // the record decides whether a guard is landed, and an older binary
    // must never silently ignore that.
    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64);
    if !schema.is_some_and(|version| (OLDEST_READABLE_SCHEMA..=SCHEMA_VERSION).contains(&version)) {
        let found = schema.map_or_else(|| "none".to_owned(), |version| version.to_string());
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::UnsupportedSchema,
                format!(
                    "{path} declares schema_version {found}, and this binary knows only {OLDEST_READABLE_SCHEMA} through {SCHEMA_VERSION}"
                ),
            )
            .expected("a record this binary can read")
            .action("run the rk release that wrote this record, or a newer one")
            .target_state("unchanged"),
        ));
    }
    let declared = schema.unwrap_or(SCHEMA_VERSION);
    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|e| anyhow::anyhow!("{path} does not parse at schema_version {declared}: {e}"))?;
    Ok(Some(manifest))
}

/// Write the record, last, through the temp-plus-rename writer.
///
/// # Errors
///
/// Any write failure; the destination then holds what it held.
pub fn write(target: &Utf8Path, manifest: &Manifest) -> Result<(), RkError> {
    let text = serde_json::to_string_pretty(manifest).map_err(anyhow::Error::from)?;
    let path = target.join(MANIFEST_PATH);
    atomic::write(path.as_std_path(), format!("{text}\n").as_bytes())?;
    Ok(())
}

/// The current instant in the record's RFC 3339 form.
#[must_use]
pub fn now() -> String {
    humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string()
}

/// How a record's `rk_version` stands against this binary's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alignment {
    /// The landing came from this binary's version.
    Aligned,
    /// The binary is newer; `rk upgrade` takes the target forward.
    BinaryNewer,
    /// The landing came from a newer `rk` than this one, which an upgrade
    /// refuses rather than downgrading.
    TargetNewer,
}

impl Alignment {
    /// The wire form, identical to the serde rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::BinaryNewer => "binary-newer",
            Self::TargetNewer => "target-newer",
        }
    }
}

/// Compare a record's version against this binary's.
#[must_use]
pub fn alignment(recorded: &str, binary: &str) -> Alignment {
    // Build metadata after `+` carries no precedence.
    let recorded = recorded
        .split_once('+')
        .map_or(recorded, |(version, _)| version);
    let binary = binary
        .split_once('+')
        .map_or(binary, |(version, _)| version);
    let recorded_core = numeric_core(recorded);
    let binary_core = numeric_core(binary);
    match binary_core.cmp(&recorded_core) {
        std::cmp::Ordering::Greater => Alignment::BinaryNewer,
        std::cmp::Ordering::Less => Alignment::TargetNewer,
        std::cmp::Ordering::Equal => {
            // Equal numeric cores: a pre-release is older than the plain
            // release it precedes, and two pre-releases compare by semver
            // precedence — dot-separated identifiers, numeric ones
            // numerically and below alphanumeric ones.
            let recorded_pre = recorded.split_once('-').map(|(_, pre)| pre);
            let binary_pre = binary.split_once('-').map(|(_, pre)| pre);
            match (recorded_pre, binary_pre) {
                (Some(_), None) => Alignment::BinaryNewer,
                (None, Some(_)) => Alignment::TargetNewer,
                (None, None) => Alignment::Aligned,
                (Some(r), Some(b)) => match prerelease_cmp(b, r) {
                    std::cmp::Ordering::Greater => Alignment::BinaryNewer,
                    std::cmp::Ordering::Less => Alignment::TargetNewer,
                    std::cmp::Ordering::Equal => Alignment::Aligned,
                },
            }
        }
    }
}

/// Whether `candidate` is ahead of `pinned`, by the same ordering the
/// alignment uses.
#[must_use]
pub fn version_is_newer(candidate: &str, pinned: &str) -> bool {
    alignment(pinned, candidate) == Alignment::BinaryNewer
}

/// Semver pre-release precedence: identifier by identifier, numeric ones
/// numerically and below any alphanumeric one, and — all preceding
/// identifiers equal — the longer list wins. An all-digit identifier
/// compares by digit count and then lexically, which is numeric order at
/// any length — semver forbids leading zeroes — so no integer parse can
/// overflow into a wrong answer.
fn prerelease_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let numeric = |identifier: &str| identifier.bytes().all(|byte| byte.is_ascii_digit());
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => {
                let ordering = match (numeric(x), numeric(y)) {
                    (true, true) => x.len().cmp(&y.len()).then_with(|| x.cmp(y)),
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (false, false) => x.cmp(y),
                };
                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

/// The dotted numeric components before any pre-release suffix.
fn numeric_core(version: &str) -> Vec<u64> {
    let core = version.split_once('-').map_or(version, |(core, _)| core);
    core.split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Alignment, FileRecord, Manifest, Parameters, Style, Workflow, alignment};
    use crate::digest::Digest;
    use crate::landing::Kind;

    /// The complete record shape at schema 4, held by snapshot: a field
    /// rename or removal fails here and becomes a schema-version bump
    /// instead of a silent break at every reader.
    #[test]
    fn the_manifest_schema_snapshot_holds() {
        let manifest = Manifest {
            schema_version: 4,
            rk_version: "0.1.0".into(),
            payload_sha256: Digest::of(b""),
            origin: "init".into(),
            tech: "rust".into(),
            forge: "github".into(),
            landed_at: "2026-08-29T00:00:00Z".into(),
            parameters: Parameters {
                repo: "acme/widget".into(),
                scopes: vec!["api".into(), "cli".into()],
                workflow: Workflow::Worktree,
                style: Some(Style::Trunk),
                nix: true,
            },
            files: vec![
                FileRecord {
                    destination: "release-plz.toml".into(),
                    kind: Kind::Seeded,
                    sha256: Digest::of(b""),
                    baseline_sha256: Some(Digest::of(b"")),
                },
                FileRecord {
                    destination: "VERSION".into(),
                    kind: Kind::State,
                    sha256: Digest::of(b""),
                    baseline_sha256: None,
                },
            ],
            pins: std::iter::once(("release-plz".to_owned(), "0.3.160".to_owned())).collect(),
        };
        let empty = Digest::of(b"").to_string();
        assert_eq!(
            serde_json::to_string(&manifest).expect("a manifest serializes"),
            format!(
                r#"{{"schema_version":4,"rk_version":"0.1.0","payload_sha256":"{empty}","origin":"init","tech":"rust","forge":"github","landed_at":"2026-08-29T00:00:00Z","parameters":{{"repo":"acme/widget","scopes":["api","cli"],"workflow":"worktree","style":"trunk","nix":true}},"files":[{{"destination":"release-plz.toml","kind":"seeded","sha256":"{empty}","baseline_sha256":"{empty}"}},{{"destination":"VERSION","kind":"state","sha256":"{empty}"}}],"pins":{{"release-plz":"0.3.160"}}}}"#
            ),
            "a state file must omit baseline_sha256 rather than serializing null"
        );
    }

    /// A record written before the mode existed reads as `branches`; a
    /// record past this binary's schema refuses by name, because the field
    /// it cannot see decides whether a guard is landed.
    #[test]
    fn a_schema_1_record_reads_as_branches_and_a_newer_schema_refuses() {
        let dir = tempfile::tempdir().expect("a scratch target exists");
        let target = camino::Utf8Path::from_path(dir.path()).expect("utf-8 path");
        std::fs::create_dir_all(target.join(".release-kit")).expect("the record dir writes");
        let record = |schema: u64| {
            format!(
                r#"{{"schema_version":{schema},"rk_version":"0.1.0","payload_sha256":"0000000000000000000000000000000000000000000000000000000000000000","origin":"init","tech":"rust","forge":"github","landed_at":"2026-08-29T00:00:00Z","parameters":{{"repo":"acme/widget","scopes":["api"]}},"files":[],"pins":{{}}}}"#
            )
        };
        std::fs::write(target.join(super::MANIFEST_PATH), record(1)).expect("the record writes");
        let manifest = super::load(target)
            .expect("a schema-1 record loads")
            .expect("the record exists");
        assert_eq!(manifest.parameters.workflow, Workflow::Branches);
        assert_eq!(
            manifest.parameters.style, None,
            "a pre-style record carries no style; the upgrade demands one"
        );
        assert!(
            !manifest.parameters.nix,
            "a pre-nix record reads as opt-out, so an upgrade adds nothing unrequested"
        );

        std::fs::write(target.join(super::MANIFEST_PATH), record(5)).expect("the record writes");
        let refused = super::load(target).expect_err("a schema-5 record refuses");
        let message = refused.to_string();
        assert!(message.contains('5'), "{message}");
    }

    #[test]
    fn alignment_orders_versions_numerically() {
        assert_eq!(alignment("0.1.0", "0.1.0"), Alignment::Aligned);
        assert_eq!(alignment("0.1.0", "0.2.0"), Alignment::BinaryNewer);
        assert_eq!(alignment("0.10.0", "0.9.9"), Alignment::TargetNewer);
        assert_eq!(alignment("0.1.0-rc.1", "0.1.0"), Alignment::BinaryNewer);
        assert_eq!(alignment("0.1.0", "0.1.0-rc.1"), Alignment::TargetNewer);
    }

    /// Pre-release identifiers order by semver precedence, not by text:
    /// `rc.10` is newer than `rc.2`, so a binary at `rc.2` must refuse a
    /// landing from `rc.10` rather than downgrade it — at any identifier
    /// length, so no integer width bounds the protection.
    #[test]
    fn alignment_orders_numeric_prerelease_identifiers_numerically() {
        assert_eq!(
            alignment("0.1.0-rc.10", "0.1.0-rc.2"),
            Alignment::TargetNewer
        );
        assert_eq!(
            alignment("0.1.0-rc.2", "0.1.0-rc.10"),
            Alignment::BinaryNewer
        );
        assert_eq!(alignment("0.1.0-rc.1", "0.1.0-rc.1"), Alignment::Aligned);
        assert_eq!(
            alignment("0.1.0-alpha", "0.1.0-alpha.1"),
            Alignment::BinaryNewer
        );
        assert_eq!(alignment("0.1.0-1", "0.1.0-alpha"), Alignment::BinaryNewer);
        assert_eq!(
            alignment("1.0.0-100000000000000000000", "1.0.0-99999999999999999999"),
            Alignment::TargetNewer,
            "identifiers past the u64 range still compare numerically"
        );
        assert_eq!(
            alignment("1.0.0-99999999999999999999", "1.0.0-100000000000000000000"),
            Alignment::BinaryNewer
        );
    }

    /// Build metadata carries no precedence: it never corrupts a numeric
    /// component and never separates two otherwise-equal versions.
    #[test]
    fn alignment_ignores_build_metadata() {
        assert_eq!(alignment("1.2.10+build", "1.2.9"), Alignment::TargetNewer);
        assert_eq!(alignment("1.2.9", "1.2.10+build"), Alignment::BinaryNewer);
        assert_eq!(alignment("1.0.0+alpha", "1.0.0+beta"), Alignment::Aligned);
        assert_eq!(
            alignment("1.2.10-rc.1+build", "1.2.10-rc.1"),
            Alignment::Aligned
        );
        assert_eq!(
            alignment("1.2.10-rc.1+build", "1.2.10"),
            Alignment::BinaryNewer
        );
    }
}
