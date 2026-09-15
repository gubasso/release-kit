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
/// Schema 8 is the receipt of a direct landing: the producing
/// `rk_version`, the origin, the resolved parameters, and per destination
/// the path, the kind, the placement where the destination is a marked
/// region, and the digest of the bytes or region now present. It carries
/// no bundle digest and no baseline digest, because the landing renders
/// afresh from this binary and compares against no earlier release.
///
/// Schemas 1 through 7 read through one bounded conversion in
/// [`legacy`]: the retired `payload_sha256`, per-file `baseline_sha256`,
/// and `parameters.scopes` fields are dropped, and the parameters a
/// record predates take the defaults such a landing wrote. The next
/// successful landing rewrites schema 8. Anything past this schema
/// refuses by name.
///
/// SATISFIES landing:a-record-states-its-schema
pub const SCHEMA_VERSION: u64 = 8;

/// The oldest schema this binary still reads.
const OLDEST_READABLE_SCHEMA: u64 = 1;

/// The first schema that states a destination's placement. A record below
/// it carried none, and the block destinations were regions by their names
/// alone.
const PLACEMENT_SCHEMA: u64 = 7;

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

/// The code scanning provider a landing records.
///
/// A project decision: which analyzer the landed workflow runs, and with it
/// whether the landing carries a licence condition at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// GitHub's own analyzer. Free under terms that cover an open-source
    /// codebase alone, so a landing reads the binding's declared licence
    /// first and refuses the pair where it is not OSI-approved.
    CodeQl,
    /// Semgrep Community Edition, which carries no licence condition on the
    /// codebase it scans and runs on either forge.
    Semgrep,
}

impl Provider {
    /// The flag, wire, and report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CodeQl => "codeql",
            Self::Semgrep => "semgrep",
        }
    }

    /// Parse a `--code-scanning` flag value.
    ///
    /// # Errors
    ///
    /// Returns [`RkError::Usage`] naming the providers and the word that
    /// turns the capability off.
    pub fn parse(raw: &str) -> Result<Option<Self>, RkError> {
        match raw {
            "codeql" => Ok(Some(Self::CodeQl)),
            "semgrep" => Ok(Some(Self::Semgrep)),
            "off" => Ok(None),
            other => Err(RkError::Usage(format!(
                "unknown code scanning provider '{other}'; the providers are: codeql, semgrep, and off turns the capability off"
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
    /// `init` or `adopt` — how the record came to exist.
    pub origin: String,
    /// The technology that selected the files.
    pub tech: String,
    /// The forge that selected the files.
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
    /// Whether the landing carries the Scorecard capability: the workflow
    /// that computes an `OpenSSF` Scorecard result and publishes it. A record
    /// predating the field reads as opt-out, so an upgrade adds nothing
    /// unrequested; the projection stays reproducible from the record
    /// because this field is part of it.
    #[serde(default)]
    pub scorecard: bool,
    /// The code scanning provider the landing carries, or none where the
    /// project did not opt in. A record predating the field carries none,
    /// so an upgrade adds no workflow unrequested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_scanning: Option<Provider>,
    /// The one permanent branch, rendered into every landed artifact that
    /// names it. A record predating the field reads as `master`, which is
    /// what such a landing wrote, so the projection stays reproducible.
    #[serde(default = "trunk_master")]
    pub trunk: String,
    /// The release-line branch prefix, rendered into the release triggers
    /// and branch guards. A record predating the field reads as
    /// `release/`, which is what such a landing wrote.
    #[serde(default = "line_prefix_release")]
    pub line_prefix: String,
    /// The contact the landed policy names where the forge's own channel
    /// is unavailable, empty for the forge's authored wording. A record
    /// predating the field reads as empty, which is what such a landing
    /// wrote.
    #[serde(default, deserialize_with = "read_contact")]
    pub security_contact: String,
    /// The acknowledgment window the landed policy promises. A record
    /// predating the field reads as `best-effort`, which is what such a
    /// landing wrote.
    #[serde(default = "response_best_effort", deserialize_with = "read_response")]
    pub security_response: String,
}

/// The trunk a record predating the field carries.
fn trunk_master() -> String {
    crate::config::TRUNK_DEFAULT.to_owned()
}

/// The prefix a record predating the field carries.
fn line_prefix_release() -> String {
    crate::config::LINE_PREFIX_DEFAULT.to_owned()
}

/// The stance a record predating the field carries.
fn response_best_effort() -> String {
    crate::config::RESPONSE_DEFAULT.to_owned()
}

/// A recorded contact, refused where the configuration reader would refuse
/// it or where it is not already canonical.
///
/// The record is the one input a re-render reads, so a hand-edited record
/// must not reach bytes the configured path could never have produced.
fn read_contact<'de, D: serde::Deserializer<'de>>(reader: D) -> Result<String, D::Error> {
    canonical(reader, "security_contact", crate::config::canonical_contact)
}

/// A recorded response stance, held to the same grammar as the key.
fn read_response<'de, D: serde::Deserializer<'de>>(reader: D) -> Result<String, D::Error> {
    canonical(
        reader,
        "security_response",
        crate::config::canonical_response,
    )
}

/// One recorded string held to its canonical form.
fn canonical<'de, D: serde::Deserializer<'de>>(
    reader: D,
    field: &str,
    judge: impl Fn(&str) -> Result<String, String>,
) -> Result<String, D::Error> {
    let raw = String::deserialize(reader)?;
    let canonical = judge(&raw)
        .map_err(|reason| serde::de::Error::custom(format!("parameters.{field}: {reason}")))?;
    if canonical == raw {
        Ok(canonical)
    } else {
        Err(serde::de::Error::custom(format!(
            "parameters.{field} is not canonical: the record carries {raw:?} where a landing writes {canonical:?}"
        )))
    }
}

/// One landed destination.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileRecord {
    /// The destination, relative to the target root.
    pub destination: String,
    /// The declared ownership kind.
    pub kind: Kind,
    /// The digest of what the destination holds: the bytes now present
    /// for a whole file, the marked region alone for a region destination.
    pub sha256: Digest,
    /// How the landing occupies the destination: the whole file, which
    /// the record omits, or one marked region inside a document the
    /// target owns.
    #[serde(default, skip_serializing_if = "Placement::is_whole")]
    pub placement: Placement,
}

/// How a recorded destination is occupied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    /// The landing owns the whole file.
    #[default]
    Whole,
    /// The landing owns the one marked region; every byte outside it is
    /// the target's.
    Region,
}

impl Placement {
    /// Whether this is the default the record omits.
    #[must_use]
    pub const fn is_whole(&self) -> bool {
        matches!(self, Self::Whole)
    }

    /// The report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Whole => "whole",
            Self::Region => "region",
        }
    }
}

/// The one bounded conversion from a record at schemas 1 through 7 to the
/// current shape.
///
/// It reads no other release and interprets no other release's sources: it drops the
/// fields the direct landing retired and lets the serde defaults on
/// [`Parameters`] answer what an older record left unsaid.
pub mod legacy {
    /// Drop every retired field from a record value at a schema before
    /// this binary's, so it deserializes as the current shape.
    ///
    /// `payload_sha256` named a bundle digest no comparison reads any
    /// more; per-file `baseline_sha256` fed a three-way comparison that
    /// no longer exists; `parameters.scopes` was a vocabulary this binary
    /// renders nowhere.
    pub fn convert(mut value: serde_json::Value) -> serde_json::Value {
        if let Some(record) = value.as_object_mut() {
            record.remove("payload_sha256");
            if let Some(parameters) = record
                .get_mut("parameters")
                .and_then(serde_json::Value::as_object_mut)
            {
                parameters.remove("scopes");
            }
            if let Some(files) = record
                .get_mut("files")
                .and_then(serde_json::Value::as_array_mut)
            {
                for file in files
                    .iter_mut()
                    .filter_map(serde_json::Value::as_object_mut)
                {
                    file.remove("baseline_sha256");
                }
            }
        }
        value
    }
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
    // A record at an earlier schema converts through the one legacy
    // conversion. Anything past this binary's schema refuses by the
    // record schema alone: the record decides whether a guard is landed,
    // and an older binary must never silently ignore that.
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
            .expected("a landing record at a schema this binary knows")
            .action("install the rk release that wrote this record, or a newer one")
            .target_state("unchanged"),
        ));
    }
    let declared = schema.unwrap_or(SCHEMA_VERSION);
    let value = if declared < SCHEMA_VERSION {
        legacy::convert(value)
    } else {
        value
    };
    let mut manifest: Manifest = serde_json::from_value(value)
        .map_err(|e| anyhow::anyhow!("{path} does not parse at schema_version {declared}: {e}"))?;
    // A record below the placement schema stated none: the block
    // destinations were regions by their names alone, and the loaded shape
    // says so.
    for file in &mut manifest.files {
        if declared < PLACEMENT_SCHEMA && crate::landing::block_markers(&file.destination).is_some()
        {
            file.placement = Placement::Region;
        }
    }
    Ok(Some(manifest))
}

/// Write the record, last, through the temp-plus-rename writer.
///
/// # Errors
///
/// Any write failure; the destination then holds what it held.
pub fn write(target: &Utf8Path, manifest: &Manifest) -> Result<(), RkError> {
    let path = target.join(MANIFEST_PATH);
    atomic::write(path.as_std_path(), &render(manifest)?)?;
    Ok(())
}

/// The bytes [`write`] puts on disk for a record.
///
/// # Errors
///
/// A serialization failure, which is a defect in this binary.
pub fn render(manifest: &Manifest) -> Result<Vec<u8>, RkError> {
    let text = serde_json::to_string_pretty(manifest).map_err(anyhow::Error::from)?;
    Ok(format!("{text}\n").into_bytes())
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
    use super::{
        Alignment, FileRecord, Manifest, Parameters, Placement, Provider, Style, Workflow,
        alignment,
    };
    use crate::digest::Digest;
    use crate::landing::Kind;

    /// The complete record shape at schema 8, held by snapshot: a field
    /// rename or removal fails here and becomes a schema-version bump
    /// instead of a silent break at every reader.
    #[test]
    fn the_manifest_schema_snapshot_holds() {
        let manifest = Manifest {
            schema_version: 8,
            rk_version: "0.1.0".into(),
            origin: "init".into(),
            tech: "rust".into(),
            forge: "github".into(),
            landed_at: "2026-08-29T00:00:00Z".into(),
            parameters: Parameters {
                repo: "acme/widget".into(),
                workflow: Workflow::Worktree,
                style: Some(Style::Trunk),
                nix: true,
                scorecard: true,
                code_scanning: Some(Provider::Semgrep),
                trunk: crate::config::TRUNK_DEFAULT.to_owned(),
                line_prefix: crate::config::LINE_PREFIX_DEFAULT.to_owned(),
                security_contact: String::new(),
                security_response: crate::config::RESPONSE_DEFAULT.to_owned(),
            },
            files: vec![
                FileRecord {
                    destination: "release-plz.toml".into(),
                    kind: Kind::Seeded,
                    sha256: Digest::of(b""),
                    placement: Placement::Whole,
                },
                FileRecord {
                    destination: "AGENTS.md".into(),
                    kind: Kind::Rendered,
                    sha256: Digest::of(b""),
                    placement: Placement::Region,
                },
            ],
            pins: std::iter::once(("release-plz".to_owned(), "0.3.160".to_owned())).collect(),
        };
        let empty = Digest::of(b"").to_string();
        let text = serde_json::to_string(&manifest).expect("a manifest serializes");
        assert_eq!(
            text,
            format!(
                r#"{{"schema_version":8,"rk_version":"0.1.0","origin":"init","tech":"rust","forge":"github","landed_at":"2026-08-29T00:00:00Z","parameters":{{"repo":"acme/widget","workflow":"worktree","style":"trunk","nix":true,"scorecard":true,"code_scanning":"semgrep","trunk":"master","line_prefix":"release/","security_contact":"","security_response":"best-effort"}},"files":[{{"destination":"release-plz.toml","kind":"seeded","sha256":"{empty}"}},{{"destination":"AGENTS.md","kind":"rendered","sha256":"{empty}","placement":"region"}}],"pins":{{"release-plz":"0.3.160"}}}}"#
            ),
            "a whole file omits its placement, and no retired digest field survives"
        );
        assert!(!text.contains("payload_sha256") && !text.contains("baseline_sha256"));
    }

    /// A record written before the mode existed reads as `branches`, and
    /// its scope vocabulary drops, because this binary renders none. Every
    /// earlier schema converts through the one legacy path with its
    /// retired digests ignored, and a record past this binary's schema
    /// refuses by the record schema alone, naming no other schema.
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
        assert_eq!(
            manifest.parameters.security_contact, "",
            "a pre-policy record names no contact, which is what its policy landed"
        );
        assert_eq!(
            manifest.parameters.security_response,
            crate::config::RESPONSE_DEFAULT,
            "a pre-policy record promises no window, which is what its policy landed"
        );

        for schema in 2..=6 {
            std::fs::write(
                target.join(super::MANIFEST_PATH),
                format!(
                    r#"{{"schema_version":{schema},"rk_version":"0.1.0","payload_sha256":"0000000000000000000000000000000000000000000000000000000000000000","origin":"init","tech":"rust","forge":"github","landed_at":"2026-08-29T00:00:00Z","parameters":{{"repo":"acme/widget"}},"files":[{{"destination":"AGENTS.md","kind":"rendered","sha256":"0000000000000000000000000000000000000000000000000000000000000000","baseline_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}],"pins":{{}}}}"#
                ),
            )
            .expect("the record writes");
            let manifest = super::load(target)
                .expect("an earlier record loads")
                .expect("the record exists");
            assert_eq!(manifest.schema_version, schema);
            assert_eq!(
                manifest.files[0].placement,
                Placement::Region,
                "a block destination reads as a region"
            );
            let rewritten = super::render(&manifest).expect("renders");
            let text = String::from_utf8(rewritten).expect("text");
            assert!(!text.contains("baseline_sha256"), "{text}");
        }

        std::fs::write(target.join(super::MANIFEST_PATH), record(999)).expect("the record writes");
        let refused = super::load(target).expect_err("a schema-999 record refuses");
        assert_eq!(
            refused.reason(),
            crate::diagnostic::Reason::UnsupportedSchema
        );
        let message = refused.to_string();
        assert!(message.contains("999"), "{message}");
        assert!(message.contains(super::MANIFEST_PATH), "{message}");
        assert!(
            !message.to_lowercase().contains("bundle"),
            "the record schema stands alone: {message}"
        );
    }

    /// The record is the one input a re-render reads, so a hand-edited
    /// record must not reach bytes the configured path could never write:
    /// a value the configuration reader refuses, and a value it would
    /// canonicalize, both refuse at deserialization.
    #[test]
    fn a_record_carrying_an_uncanonical_security_parameter_refuses() {
        let dir = tempfile::tempdir().expect("a scratch target exists");
        let target = camino::Utf8Path::from_path(dir.path()).expect("utf-8 path");
        std::fs::create_dir_all(target.join(".release-kit")).expect("the record dir writes");
        for (field, value) in [
            // A JSON escape, so the record parses and the value it decodes
            // to is the line feed the policy could never carry.
            ("security_contact", "team@acme.example\\nsecond line"),
            ("security_contact", "  team@acme.example  "),
            ("security_response", "90d"),
            ("security_response", "0 days"),
            ("security_response", "07 days"),
            ("security_response", "1 days"),
            ("security_response", ""),
        ] {
            let record = format!(
                r#"{{"schema_version":8,"rk_version":"0.1.0","origin":"init","tech":"rust","forge":"github","landed_at":"2026-08-29T00:00:00Z","parameters":{{"repo":"acme/widget","{field}":"{value}"}},"files":[],"pins":{{}}}}"#
            );
            std::fs::write(target.join(super::MANIFEST_PATH), record).expect("the record writes");
            let refused = super::load(target).expect_err("an uncanonical record refuses");
            assert!(refused.to_string().contains(field), "{field}: {refused}");
        }
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
