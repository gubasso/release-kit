//! The plan: one typed, immutable document that is the input to every
//! landing write.
//!
//! The document keeps five kinds apart, and an addition inside one kind
//! is additive. Evidence is what was observed. Analysis is what the
//! engine derived from it: the operations, the compatibility, the
//! guidance. Policy is the requirement each precondition carries.
//! Decisions are workflow state the operator owns. Postconditions are
//! what proves completion. `rk reconcile plan` computes one and prints
//! it; nothing here writes into a target.

pub mod apply;
pub mod classify;
pub mod evidence;
pub mod fingerprint;
pub mod gather;
pub mod operation;
pub mod planner;
pub mod readiness;
pub mod store;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use classify::{Classification, Finding, Verdict};
pub use evidence::{EvidenceItem, EvidenceKind};
pub use operation::Operation;
pub use readiness::{Evaluation, Precondition, Readiness, Requirement};

use crate::digest::Digest;
use crate::landing::Kind;

/// The version of the plan's shape.
pub const PLAN_SCHEMA: &str = "rk.plan/2";

/// What the caller asked the plan to be: the open reconciliation, or one
/// of the three fronts, each of which fixes what the plan may contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Intent {
    /// `rk reconcile plan`: the classification decides.
    Reconcile,
    /// `rk init`: a first landing, refused over a record.
    Setup,
    /// `rk upgrade`: a recorded target takes the candidate, refused
    /// without a record.
    Upgrade,
    /// `rk adopt`: the record and the configuration alone, every
    /// destination verified and none written.
    Adopt,
}

impl Intent {
    /// The wire form, identical to the serde rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reconcile => "reconcile",
            Self::Setup => "setup",
            Self::Upgrade => "upgrade",
            Self::Adopt => "adopt",
        }
    }
}

/// The plan, whole.
#[derive(Debug, Serialize, Deserialize)]
pub struct Plan {
    /// The shape version of this document.
    pub schema: std::borrow::Cow<'static, str>,
    /// Who computed it, when, under which id.
    pub identity: Identity,
    /// Which procedure this plan is.
    pub classification: Classification,
    /// What the classification compresses.
    pub findings: Vec<Finding>,
    /// What the target is asked to converge toward.
    pub desired_state: DesiredState,
    /// What the target was found to be.
    pub observed_state: ObservedState,
    /// The candidate bundle and what is known about it.
    pub release: Release,
    /// The typed changes, in apply order.
    pub operations: Vec<Operation>,
    /// Each with its requirement and its evaluation.
    pub preconditions: Vec<Precondition>,
    /// The questions the operator owns, with their selected answers.
    pub decisions: Vec<Decision>,
    /// The typed checks an apply runs at the end and reports.
    pub postconditions: Vec<Postcondition>,
    /// Every observed value, cited by the fields above.
    pub evidence: Vec<EvidenceItem>,
    /// Whether the plan may be applied.
    pub readiness: Readiness,
    /// One canonical digest over the semantic inputs.
    pub input_fingerprint: Digest,
}

/// Who computed the plan, when, and under which id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// Derived from the fingerprint and the creation instant, so two
    /// plans over the same inputs are distinguishable and one plan is
    /// not stored twice by accident.
    pub plan_id: String,
    /// The instant the plan was computed, RFC 3339.
    pub created_at: String,
    /// The engine that computed it.
    pub engine_version: String,
}

/// What the target is asked to converge toward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    /// What the caller asked the plan to be.
    pub intent: Intent,
    /// The selector as the operator gave it: `embedded`, `latest`, or an
    /// exact version.
    pub selector: String,
    /// What the selector resolved to, once, frozen here.
    pub release: ResolvedRelease,
    /// The landing configuration the projection renders under, or the
    /// reason none resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Configuration>,
    /// Why the configuration did not resolve, where it did not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<String>,
}

/// One exact release, resolved at plan time and never again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRelease {
    /// The exact version.
    pub version: String,
    /// Where it was read from: `embedded`, `crates`, or `directory`.
    pub venue: String,
    /// The bundle's aggregate digest.
    pub payload_sha256: Digest,
    /// The bundle's protocol version.
    pub payload_schema: u32,
}

/// The landing parameters, resolved, with the layer each one came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    /// The payload binding.
    pub tech: String,
    /// The forge.
    pub forge: String,
    /// The project path on the forge.
    pub repo: String,
    /// The working-copy mode.
    pub workflow: String,
    /// The release style, where one is answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Whether the landing carries the Nix capability.
    pub nix: bool,
    /// The one permanent branch.
    pub trunk: String,
    /// The release-line prefix.
    pub line_prefix: String,
    /// The security contact the policy names, empty for the forge's own.
    pub security_contact: String,
    /// The acknowledgment window the policy promises.
    pub security_response: String,
    /// Which layer answered each parameter: `flag`, `configuration`,
    /// `record`, `detected`, or `default`.
    pub sources: BTreeMap<String, String>,
    /// The evidence the resolution read.
    pub evidence_refs: Vec<String>,
}

/// What the target was found to be.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedState {
    /// The repository's own state.
    pub repository: Repository,
    /// What release-kit landed there, as far as the disk says.
    pub installation: Installation,
    /// The engine and the host.
    pub host: Host,
    /// What the forge said, where it was asked.
    pub forge: ForgeState,
}

/// The repository's own state, read off the disk and git.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    /// The target directory.
    pub target: String,
    /// Whether the target is a git repository.
    pub git: bool,
    /// How many tags it holds.
    pub tags: usize,
    /// Long-lived branches beside the trunk.
    pub long_lived_branches: Vec<String>,
    /// Other tools' release markers present.
    pub release_markers: Vec<String>,
    /// Payload destinations already present.
    pub collisions: Vec<String>,
    /// The technology the version file names, where one is found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech: Option<String>,
    /// The forge the origin remote maps to, where one is recognized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forge: Option<String>,
    /// The project path from the origin remote, where one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// The corpus verdict the facts above earn.
    pub verdict: Verdict,
    /// The evidence these facts rest on.
    pub evidence_refs: Vec<String>,
}

/// What release-kit landed at the target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installation {
    /// The landing record.
    pub record: RecordState,
    /// The committed configuration.
    pub configuration: ConfigurationState,
    /// Every destination the candidate or the record names, as found.
    pub destinations: Vec<Destination>,
    /// The evidence the installation rests on.
    pub evidence_refs: Vec<String>,
}

/// The landing record, as found.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RecordState {
    /// No record at the target.
    Absent,
    /// A record this engine read.
    Present {
        /// The binary that wrote it.
        rk_version: String,
        /// The payload that landed.
        payload_sha256: Digest,
        /// The record's schema.
        schema_version: u64,
        /// How the record came to exist.
        origin: String,
        /// The digest of the record's bytes.
        sha256: Digest,
    },
    /// A record this engine could not read.
    Invalid {
        /// Why.
        reason: String,
    },
}

/// The committed configuration, as found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationState {
    /// Whether `.release-kit/config.toml` exists.
    pub present: bool,
    /// The digest of its bytes, where present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<Digest>,
    /// Why it did not read, where it did not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid: Option<String>,
    /// Keys whose configured answers the record has yet to take up.
    pub pending: Vec<String>,
}

/// One destination, as found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Destination {
    /// The destination, relative to the target.
    pub path: String,
    /// Whether the file, or the marked block, is present.
    pub present: bool,
    /// The digest of what is there, where present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<Digest>,
    /// The kind the record declares for it, where the record names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_kind: Option<Kind>,
}

/// The engine and the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    /// This engine's version.
    pub engine_version: String,
    /// The pin the wired manager records for `rk`, where one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<PinState>,
    /// The evidence the host facts rest on.
    pub evidence_refs: Vec<String>,
}

/// The `rk` pin a tool manager records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinState {
    /// The manager.
    pub manager: String,
    /// The file that records it.
    pub file: String,
    /// The version, as the manager records it.
    pub version: String,
}

/// What the forge said, where it was asked.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum ForgeState {
    /// The forge was not asked, and the reason says why.
    NotObserved {
        /// Why.
        reason: String,
    },
    /// The forge was asked.
    Observed {
        /// The trunk the read asked about.
        trunk: String,
        /// The trunk's tip at the remote, where it has one.
        #[serde(skip_serializing_if = "Option::is_none")]
        remote_tip: Option<String>,
        /// The evidence the read produced.
        evidence_refs: Vec<String>,
    },
}

/// The candidate bundle and what is known about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    /// The candidate's identity.
    pub candidate: BundleIdentity,
    /// How the candidate was verified.
    pub verification: Verification,
    /// The recorded release's bundle, for the three-way comparison.
    pub baseline: BaselineState,
    /// What the engine can say about reading this bundle.
    pub compatibility: Compatibility,
    /// The guidance the bundle carries for this target.
    pub guidance: Guidance,
}

/// One bundle's identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleIdentity {
    /// The release's version.
    pub version: String,
    /// The aggregate digest.
    pub payload_sha256: Digest,
    /// The protocol version.
    pub payload_schema: u32,
    /// How many artifacts the bundle carries.
    pub artifacts: usize,
    /// The evidence the identity rests on.
    pub evidence_refs: Vec<String>,
}

/// How a bundle was verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum Verification {
    /// The bundle is the one compiled into this engine.
    Embedded,
    /// The archive digested to the registry's checksum.
    RegistryChecksum {
        /// The checksum the index named.
        cksum: Digest,
    },
    /// A directory laid out as a bundle, read as is.
    Directory,
}

/// The recorded release's bundle, as read for the baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum BaselineState {
    /// No record, so no baseline is needed.
    NotNeeded,
    /// The recorded payload is the one compiled into this engine.
    Embedded,
    /// The recorded release's bundle was read from the release cache.
    Cached {
        /// The recorded version.
        version: String,
    },
    /// The recorded release's bundle could not be read, and the reason
    /// says why.
    NotObserved {
        /// Why.
        reason: String,
    },
}

/// What the engine can say about reading this bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compatibility {
    /// The engine's protocol version.
    pub engine_schema: u32,
    /// The bundle's protocol version.
    pub bundle_schema: u32,
    /// Whether the engine reads the bundle.
    pub readable: bool,
}

/// The guidance the bundle carries for this target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guidance {
    /// `not-shipped` until a bundle carries guidance; the coverage words
    /// grow when one does.
    pub coverage: std::borrow::Cow<'static, str>,
}

/// One question the operator owns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// A stable id that survives re-planning.
    pub id: String,
    /// The question, one line.
    pub question: String,
    /// The answers, each with its consequence.
    pub choices: Vec<Choice>,
    /// The answer selected, where one is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
}

/// One answer to a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// The answer word.
    pub answer: String,
    /// What selecting it means.
    pub consequence: String,
}

/// One typed check an apply runs at the end and reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "check", rename_all = "kebab-case")]
pub enum Postcondition {
    /// The record reads back at the planned digest.
    RecordReadsBack {
        /// The planned digest.
        sha256: Digest,
    },
    /// A destination holds the planned bytes.
    DestinationHolds {
        /// The destination.
        path: String,
        /// The planned digest.
        sha256: Digest,
    },
    /// `rk status --check` exits 0.
    StatusCheckClean,
    /// The wired manager records the planned version.
    PinReads {
        /// The manager.
        manager: String,
        /// The planned version.
        version: String,
    },
}

/// One request to compute a plan, as the store keeps it beside the plan
/// so an apply can compute the same plan again and compare.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRequest {
    /// The target, as given.
    pub target: camino::Utf8PathBuf,
    /// What the caller asked the plan to be.
    pub intent: Intent,
    /// The selector as given: `embedded`, `latest`, or an exact version.
    pub selector: String,
    /// Whether a recorded release the cache does not hold is fetched.
    pub fetch: bool,
    /// Whether the forge is read.
    pub observe_forge: bool,
    /// The explicit answers.
    pub flags: gather::Flags,
    /// The decisions selected, by id.
    pub decisions: BTreeMap<String, String>,
}

/// A computed plan with the bytes its operations name, and what the
/// three-way comparison decided per destination, for the fronts that
/// render a per-file report.
#[derive(Debug)]
pub struct Planned {
    /// The plan.
    pub plan: Plan,
    /// Every byte the plan names, by digest: what an operation writes,
    /// what a destination holds now, and the baseline where it was read.
    pub blobs: BTreeMap<Digest, Vec<u8>>,
    /// What the comparison decided for each projected destination, in
    /// projection order.
    pub outcomes: Vec<DestinationOutcome>,
    /// The configuration an apply writes, where the parameters resolved.
    pub config: Option<crate::config::Plan>,
    /// The Nix destinations withheld at this target, each with why.
    pub withheld: Vec<crate::landing::Withheld>,
}

/// What the three-way comparison decided for one projected destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationOutcome {
    /// The destination, relative to the target.
    pub path: String,
    /// The kind the candidate declares.
    pub kind: Kind,
    /// Whether the record names it.
    pub recorded: bool,
    /// What happens to it.
    pub disposition: Disposition,
}

/// The closed set of things the comparison decides for a destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// The candidate's bytes are written.
    Write,
    /// The destination already holds what the candidate would write, or
    /// what the record left there.
    Unchanged,
    /// The target's own bytes stay: a seeded or state file it tuned.
    Kept,
    /// A recorded seeded file moved away from its baseline and stays.
    Drift,
    /// A recorded state file, never compared.
    State,
    /// The target edited a file release-kit owns.
    Conflict,
    /// The record names a file the disk does not hold.
    Missing,
}

impl Disposition {
    /// The word a report prints.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Unchanged => "unchanged",
            Self::Kept => "kept",
            Self::Drift => "drift",
            Self::State => "state",
            Self::Conflict => "conflict",
            Self::Missing => "missing",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        BaselineState, BundleIdentity, Choice, Classification, Compatibility, Configuration,
        ConfigurationState, Decision, DesiredState, Destination, Evaluation, ForgeState, Guidance,
        Host, Identity, Installation, Intent, Operation, PLAN_SCHEMA, PinState, Plan,
        Postcondition, Precondition, Readiness, RecordState, Release, Repository, Requirement,
        ResolvedRelease, Verdict, Verification,
    };
    use crate::digest::Digest;
    use crate::landing::Kind;
    use crate::plan::classify::Finding;
    use crate::plan::evidence::{EvidenceItem, EvidenceKind};

    /// The complete `rk.plan/1` shape, every section present, held by
    /// snapshot: a field rename or removal fails here and becomes a
    /// deliberate schema bump.
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the snapshot builds every section of the plan once, and cutting it would hide a section from the one test that holds the shape"
    )]
    fn the_plan_schema_is_versioned_and_snapshot_tested() {
        let a = Digest::of(b"a");
        let b = Digest::of(b"b");
        let plan = Plan {
            schema: PLAN_SCHEMA.into(),
            identity: Identity {
                plan_id: "0123456789abcdef".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                engine_version: "0.0.0".into(),
            },
            classification: Classification::Upgrade,
            findings: vec![Finding {
                code: "payload-collision".into(),
                detail: "SECURITY.md".into(),
            }],
            desired_state: DesiredState {
                intent: Intent::Reconcile,
                selector: "embedded".into(),
                release: ResolvedRelease {
                    version: "0.0.0".into(),
                    venue: "embedded".into(),
                    payload_sha256: a.clone(),
                    payload_schema: 1,
                },
                configuration: Some(Configuration {
                    tech: "rust".into(),
                    forge: "github".into(),
                    repo: "acme/widget".into(),
                    workflow: "worktree".into(),
                    style: Some("trunk".into()),
                    nix: false,
                    trunk: "master".into(),
                    line_prefix: "release/".into(),
                    security_contact: String::new(),
                    security_response: "best-effort".into(),
                    sources: BTreeMap::from([("tech".to_owned(), "record".to_owned())]),
                    evidence_refs: vec!["record".into()],
                }),
                unresolved: None,
            },
            observed_state: super::ObservedState {
                repository: Repository {
                    target: "/tmp/t".into(),
                    git: true,
                    tags: 0,
                    long_lived_branches: vec![],
                    release_markers: vec![],
                    collisions: vec!["SECURITY.md".into()],
                    tech: Some("rust".into()),
                    forge: Some("github".into()),
                    repo: Some("acme/widget".into()),
                    verdict: Verdict::Brownfield,
                    evidence_refs: vec!["repository".into()],
                },
                installation: Installation {
                    record: RecordState::Present {
                        rk_version: "0.0.0".into(),
                        payload_sha256: a.clone(),
                        schema_version: 6,
                        origin: "init".into(),
                        sha256: b.clone(),
                    },
                    configuration: ConfigurationState {
                        present: true,
                        sha256: Some(b.clone()),
                        invalid: None,
                        pending: vec![],
                    },
                    destinations: vec![Destination {
                        path: "SECURITY.md".into(),
                        present: true,
                        sha256: Some(a.clone()),
                        recorded_kind: Some(Kind::Rendered),
                    }],
                    evidence_refs: vec!["record".into(), "configuration".into()],
                },
                host: Host {
                    engine_version: "0.0.0".into(),
                    pin: Some(PinState {
                        manager: "mise".into(),
                        file: "mise.toml".into(),
                        version: "0.0.0".into(),
                    }),
                    evidence_refs: vec!["host".into()],
                },
                forge: ForgeState::NotObserved {
                    reason: "not requested".into(),
                },
            },
            release: Release {
                candidate: BundleIdentity {
                    version: "0.0.0".into(),
                    payload_sha256: a.clone(),
                    payload_schema: 1,
                    artifacts: 1,
                    evidence_refs: vec!["candidate-bundle".into()],
                },
                verification: Verification::Embedded,
                baseline: BaselineState::Embedded,
                compatibility: Compatibility {
                    engine_schema: 1,
                    bundle_schema: 1,
                    readable: true,
                },
                guidance: Guidance {
                    coverage: "not-shipped".into(),
                },
            },
            operations: vec![Operation::WriteRecord {
                before: Some(b.clone()),
                after: a.clone(),
            }],
            preconditions: vec![Precondition {
                id: "record-readable".into(),
                requirement: Requirement::Required,
                evaluation: Evaluation::Satisfied,
                decision: None,
                evidence_refs: vec!["record".into()],
            }],
            decisions: vec![Decision {
                id: "workflow-mode".into(),
                question: "which working-copy mode".into(),
                choices: vec![Choice {
                    answer: "worktree".into(),
                    consequence: "every branch in a linked worktree".into(),
                }],
                selected: Some("worktree".into()),
            }],
            postconditions: vec![Postcondition::RecordReadsBack { sha256: a.clone() }],
            evidence: vec![EvidenceItem {
                id: "record".into(),
                kind: EvidenceKind::Record,
                producer: "rk".into(),
                observed_at: "2026-01-01T00:00:00Z".into(),
                sha256: Some(b.clone()),
                method: "read".into(),
            }],
            readiness: Readiness::Ready,
            input_fingerprint: a.clone(),
        };
        let json = serde_json::to_string(&plan).expect("a plan serializes");
        let expected = format!(
            r#"{{"schema":"rk.plan/2","identity":{{"plan_id":"0123456789abcdef","created_at":"2026-01-01T00:00:00Z","engine_version":"0.0.0"}},"classification":"upgrade","findings":[{{"code":"payload-collision","detail":"SECURITY.md"}}],"desired_state":{{"intent":"reconcile","selector":"embedded","release":{{"version":"0.0.0","venue":"embedded","payload_sha256":"{a}","payload_schema":1}},"configuration":{{"tech":"rust","forge":"github","repo":"acme/widget","workflow":"worktree","style":"trunk","nix":false,"trunk":"master","line_prefix":"release/","security_contact":"","security_response":"best-effort","sources":{{"tech":"record"}},"evidence_refs":["record"]}}}},"observed_state":{{"repository":{{"target":"/tmp/t","git":true,"tags":0,"long_lived_branches":[],"release_markers":[],"collisions":["SECURITY.md"],"tech":"rust","forge":"github","repo":"acme/widget","verdict":"brownfield","evidence_refs":["repository"]}},"installation":{{"record":{{"state":"present","rk_version":"0.0.0","payload_sha256":"{a}","schema_version":6,"origin":"init","sha256":"{b}"}},"configuration":{{"present":true,"sha256":"{b}","pending":[]}},"destinations":[{{"path":"SECURITY.md","present":true,"sha256":"{a}","recorded_kind":"rendered"}}],"evidence_refs":["record","configuration"]}},"host":{{"engine_version":"0.0.0","pin":{{"manager":"mise","file":"mise.toml","version":"0.0.0"}},"evidence_refs":["host"]}},"forge":{{"state":"not-observed","reason":"not requested"}}}},"release":{{"candidate":{{"version":"0.0.0","payload_sha256":"{a}","payload_schema":1,"artifacts":1,"evidence_refs":["candidate-bundle"]}},"verification":{{"method":"embedded"}},"baseline":{{"state":"embedded"}},"compatibility":{{"engine_schema":1,"bundle_schema":1,"readable":true}},"guidance":{{"coverage":"not-shipped"}}}},"operations":[{{"op":"write-record","before":"{b}","after":"{a}"}}],"preconditions":[{{"id":"record-readable","requirement":"required","evaluation":{{"state":"satisfied"}},"evidence_refs":["record"]}}],"decisions":[{{"id":"workflow-mode","question":"which working-copy mode","choices":[{{"answer":"worktree","consequence":"every branch in a linked worktree"}}],"selected":"worktree"}}],"postconditions":[{{"check":"record-reads-back","sha256":"{a}"}}],"evidence":[{{"id":"record","kind":"record","producer":"rk","observed_at":"2026-01-01T00:00:00Z","sha256":"{b}","method":"read"}}],"readiness":"ready","input_fingerprint":"{a}"}}"#
        );
        assert_eq!(json, expected);
    }
}
