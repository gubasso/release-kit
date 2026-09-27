//! The committed setup proof: what the last complete observation proved.
//!
//! Four records answer four different questions, and none substitutes for
//! another. The target configuration is the desired state, the landing
//! record is what landed, the host journal is what one run did on one
//! machine, and `rk setup check` is what the forge answers now. This file
//! is the fifth: a portable, non-secret statement that a run able to see
//! every step saw each one hold, against a named setup contract.
//!
//! The record never becomes current truth. A reader judges whether it
//! still describes this target's contract, and a live check that reads
//! the forge still decides what holds now.

use std::collections::BTreeMap;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use super::context::Ctx;
use super::report::{Observed, Report, stance};
use super::steps::STEPS;
use crate::digest::Digest;

/// Where the proof lives, relative to the target.
pub const PROOF_PATH: &str = ".release-kit/setup-proof.json";

/// The record's schema.
pub const SCHEMA: &str = "rk.setup-proof/1";

/// The schema family, so a newer record reads as newer rather than foreign.
const SCHEMA_FAMILY: &str = "rk.setup-proof/";

/// The setup contract a proof is judged against: the answers that decide
/// which steps run and what each one asserts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    /// The resolved target answers, by name, each in its canonical text.
    pub target: BTreeMap<String, String>,
    /// Every step's contract, in step-table order.
    pub steps: Vec<SubjectStep>,
}

/// One step's contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectStep {
    /// The step.
    pub name: String,
    /// What the step proves.
    pub proves: String,
    /// The steps that must hold before this one applies.
    pub prerequisites: Vec<String>,
    /// Where the step stands at this target.
    pub stance: String,
    /// Why, where it does not apply or the target excludes it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// One step's normalized result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenStep {
    /// The step.
    pub name: String,
    /// `satisfied`, `satisfied-with-limitation`, `skipped`, `excluded`,
    /// `not-applicable`, or `redundant`.
    pub state: String,
    /// The stable limitation, where the forge enforces less than the step
    /// claims.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limitation: Option<String>,
}

/// The committed record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    /// Always [`SCHEMA`].
    pub schema: String,
    /// The binary that ran the observation: provenance, not a judgment.
    pub rk_version: String,
    /// When the observation completed, UTC.
    pub verified_at: String,
    /// The contract the observation proved.
    pub subject: Subject,
    /// The SHA-256 of the subject's canonical bytes.
    pub subject_digest: String,
    /// Every step's normalized result, in step-table order.
    pub steps: Vec<ProvenStep>,
}

/// Where a committed proof stands against this target now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// No proof is committed.
    Absent,
    /// The proof describes this target's current contract.
    Compatible(Proof),
    /// The proof is whole, but the contract moved, by the fields named.
    Stale {
        /// The record.
        proof: Proof,
        /// Each changed field, by name.
        differences: Vec<String>,
    },
    /// The file is not a proof this binary can read.
    Invalid {
        /// `unreadable`, `malformed`, `unsupported-schema`,
        /// `future-schema`, or `digest-mismatch`.
        reason: &'static str,
        /// What was found, one line.
        detail: String,
    },
}

impl Standing {
    /// The word a report leads with.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Compatible(_) => "compatible",
            Self::Stale { .. } => "stale",
            Self::Invalid { .. } => "invalid",
        }
    }
}

/// The contract this target states now.
#[must_use]
pub fn subject(ctx: &Ctx) -> Subject {
    let steps = STEPS
        .iter()
        .map(|step| {
            let stance = stance(ctx, step);
            SubjectStep {
                name: step.name.to_owned(),
                proves: step.proves.to_owned(),
                prerequisites: step.prereqs.iter().map(|&name| name.to_owned()).collect(),
                stance: stance.word().to_owned(),
                reason: stance.detail(),
            }
        })
        .collect();
    Subject {
        target: ctx.proof_fields(),
        steps,
    }
}

/// The subject's digest over its canonical bytes: compact JSON, whose maps
/// are ordered by key and whose lists keep step-table order.
#[must_use]
pub fn digest(subject: &Subject) -> String {
    let bytes = serde_json::to_vec(subject).unwrap_or_default();
    Digest::of(&bytes).to_string()
}

/// A proof of `report`, or `None` where the report is not a complete
/// observation.
///
/// Only a normalized word and a stable limitation enter the record per
/// step: a forge's own answer, a process's output, and every local
/// coordinate stay out.
///
/// SATISFIES setup-proof:a-checkpoint-records-only-a-complete-observation
/// SATISFIES setup-proof:the-proof-carries-no-secret-or-machine-coordinate
#[must_use]
pub fn of(ctx: &Ctx, report: &Report, verified_at: String) -> Option<Proof> {
    if !report.checkpointable() {
        return None;
    }
    let steps = report
        .rows
        .iter()
        .map(|row| {
            let (state, limitation) = match &row.observed {
                None => (row.stance.word().to_owned(), None),
                Some(Observed::Satisfied { limitation }) => (
                    Observed::Satisfied {
                        limitation: limitation.clone(),
                    }
                    .wire()
                    .to_owned(),
                    limitation.clone(),
                ),
                Some(other) => (other.wire().to_owned(), None),
            };
            ProvenStep {
                name: row.name.to_owned(),
                state,
                limitation,
            }
        })
        .collect();
    let subject = subject(ctx);
    Some(Proof {
        schema: SCHEMA.to_owned(),
        rk_version: env!("CARGO_PKG_VERSION").to_owned(),
        verified_at,
        subject_digest: digest(&subject),
        subject,
        steps,
    })
}

/// The record's bytes: pretty JSON with one trailing newline.
#[must_use]
pub fn render(proof: &Proof) -> String {
    let mut text = serde_json::to_string_pretty(proof).unwrap_or_default();
    text.push('\n');
    text
}

/// Write the proof in one rename, so an interrupted write leaves the
/// previous record whole.
///
/// # Errors
///
/// The underlying I/O failure.
pub fn write(target: &Utf8Path, proof: &Proof) -> std::io::Result<()> {
    let path = target.join(PROOF_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::atomic::write(path.as_std_path(), render(proof).as_bytes())
}

/// Read the committed proof and judge it against `current`, offline.
///
/// SATISFIES setup-proof:the-status-reads-the-proof-offline
#[must_use]
pub fn judge(target: &Utf8Path, current: &Subject) -> Standing {
    let path = target.join(PROOF_PATH);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Standing::Absent,
        Err(err) => {
            return Standing::Invalid {
                reason: "unreadable",
                detail: format!("{PROOF_PATH} is unreadable: {err}"),
            };
        }
    };
    let proof = match parse(&text) {
        Ok(proof) => proof,
        Err((reason, detail)) => return Standing::Invalid { reason, detail },
    };
    let differences = differences(&proof.subject, current);
    if differences.is_empty() {
        Standing::Compatible(proof)
    } else {
        Standing::Stale { proof, differences }
    }
}

/// Parse and verify one record: its schema first, then its shape, then
/// that its subject is the one its digest names.
fn parse(text: &str) -> Result<Proof, (&'static str, String)> {
    /// The one field read before the schema is known.
    #[derive(Deserialize)]
    struct Envelope {
        schema: String,
    }
    let malformed = |err: serde_json::Error| {
        (
            "malformed",
            format!("{PROOF_PATH} is not a setup proof: {err}"),
        )
    };
    let envelope: Envelope = serde_json::from_str(text).map_err(malformed)?;
    if envelope.schema != SCHEMA {
        let newer = envelope
            .schema
            .strip_prefix(SCHEMA_FAMILY)
            .and_then(|version| version.parse::<u32>().ok())
            .is_some_and(|version| version > 1);
        return Err(if newer {
            (
                "future-schema",
                format!(
                    "{PROOF_PATH} is {}, newer than the {SCHEMA} this binary reads; a newer rk reads it",
                    envelope.schema
                ),
            )
        } else {
            (
                "unsupported-schema",
                format!(
                    "{PROOF_PATH} declares {}, and this binary reads {SCHEMA}",
                    envelope.schema
                ),
            )
        });
    }
    let proof: Proof = serde_json::from_str(text).map_err(malformed)?;
    if digest(&proof.subject) != proof.subject_digest {
        return Err((
            "digest-mismatch",
            format!(
                "{PROOF_PATH} names a subject digest its own subject does not produce, so the record was edited after it was written"
            ),
        ));
    }
    Ok(proof)
}

/// Every field where the recorded contract and the current one differ,
/// by name: `target.<answer>`, `step <name> <part>`, or a step that one
/// side lacks.
#[must_use]
pub fn differences(recorded: &Subject, current: &Subject) -> Vec<String> {
    let mut found = Vec::new();
    let keys: std::collections::BTreeSet<&String> = recorded
        .target
        .keys()
        .chain(current.target.keys())
        .collect();
    for key in keys {
        if recorded.target.get(key) != current.target.get(key) {
            found.push(format!("target.{key}"));
        }
    }
    for step in &recorded.steps {
        match current.steps.iter().find(|now| now.name == step.name) {
            None => found.push(format!("step {} is no longer in the step table", step.name)),
            Some(now) => {
                for (part, changed) in [
                    ("proves", step.proves != now.proves),
                    ("prerequisites", step.prerequisites != now.prerequisites),
                    (
                        "stance",
                        step.stance != now.stance || step.reason != now.reason,
                    ),
                ] {
                    if changed {
                        found.push(format!("step {} {part}", step.name));
                    }
                }
            }
        }
    }
    for now in &current.steps {
        if !recorded.steps.iter().any(|step| step.name == now.name) {
            found.push(format!("step {} is new", now.name));
        }
    }
    found
}

/// The `rk.setup-status/1` document.
#[derive(Debug, Serialize)]
pub struct StatusDocument {
    /// Always `rk.setup-status/1`.
    pub schema: &'static str,
    /// `absent`, `compatible`, `stale`, or `invalid`.
    pub state: &'static str,
    /// The record's provenance, where one was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<Checkpoint>,
    /// Each changed contract field, where the proof is stale.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub differences: Vec<String>,
    /// Each step's recorded result, where one was read.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<ProvenStep>,
    /// Why the file is not readable as a proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid: Option<InvalidProof>,
}

/// A record's provenance.
#[derive(Debug, Serialize)]
pub struct Checkpoint {
    /// The binary that proved it.
    pub rk_version: String,
    /// When.
    pub verified_at: String,
    /// The subject it proved.
    pub subject_digest: String,
}

/// Why a file is not a readable proof.
#[derive(Debug, Serialize)]
pub struct InvalidProof {
    /// The closed reason word.
    pub reason: &'static str,
    /// What was found.
    pub detail: String,
}

impl StatusDocument {
    /// The document for one standing.
    #[must_use]
    pub fn of(standing: &Standing) -> Self {
        let mut document = Self {
            schema: "rk.setup-status/1",
            state: standing.word(),
            checkpoint: None,
            differences: Vec::new(),
            steps: Vec::new(),
            invalid: None,
        };
        let proof = match standing {
            Standing::Absent => None,
            Standing::Compatible(proof) => Some(proof),
            Standing::Stale { proof, differences } => {
                document.differences.clone_from(differences);
                Some(proof)
            }
            Standing::Invalid { reason, detail } => {
                document.invalid = Some(InvalidProof {
                    reason,
                    detail: detail.clone(),
                });
                None
            }
        };
        if let Some(proof) = proof {
            document.checkpoint = Some(Checkpoint {
                rk_version: proof.rk_version.clone(),
                verified_at: proof.verified_at.clone(),
                subject_digest: proof.subject_digest.clone(),
            });
            document.steps.clone_from(&proof.steps);
        }
        document
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_subject() -> Subject {
        Subject {
            target: BTreeMap::from([
                ("forge".to_owned(), "github".to_owned()),
                ("repo".to_owned(), "acme/widget".to_owned()),
            ]),
            steps: vec![SubjectStep {
                name: "default-branch".to_owned(),
                proves: "the trunk is the default branch".to_owned(),
                prerequisites: Vec::new(),
                stance: "applicable".to_owned(),
                reason: String::new(),
            }],
        }
    }

    fn sample() -> Proof {
        let subject = sample_subject();
        Proof {
            schema: SCHEMA.to_owned(),
            rk_version: "0.0.0".to_owned(),
            verified_at: "2026-09-27T00:00:00Z".to_owned(),
            subject_digest: digest(&subject),
            subject,
            steps: vec![
                ProvenStep {
                    name: "default-branch".to_owned(),
                    state: "satisfied".to_owned(),
                    limitation: None,
                },
                ProvenStep {
                    name: "auto-merge".to_owned(),
                    state: "satisfied-with-limitation".to_owned(),
                    limitation: Some("weaker".to_owned()),
                },
            ],
        }
    }

    /// The record's shape, held by snapshot: a renamed field is a schema
    /// bump, never a silent parser break.
    ///
    /// SATISFIES distribution:machine-output-declares-its-schema
    #[test]
    fn the_setup_proof_shape_is_held() {
        let text = serde_json::to_string(&sample()).expect("serializes");
        assert_eq!(
            text,
            format!(
                r#"{{"schema":"rk.setup-proof/1","rk_version":"0.0.0","verified_at":"2026-09-27T00:00:00Z","subject":{{"target":{{"forge":"github","repo":"acme/widget"}},"steps":[{{"name":"default-branch","proves":"the trunk is the default branch","prerequisites":[],"stance":"applicable"}}]}},"subject_digest":"{}","steps":[{{"name":"default-branch","state":"satisfied"}},{{"name":"auto-merge","state":"satisfied-with-limitation","limitation":"weaker"}}]}}"#,
                digest(&sample_subject())
            )
        );
    }

    /// The status document's shape, held by snapshot in each state.
    ///
    /// SATISFIES distribution:machine-output-declares-its-schema
    #[test]
    fn the_setup_status_shape_is_held() {
        let absent = serde_json::to_string(&StatusDocument::of(&Standing::Absent)).expect("ok");
        assert_eq!(absent, r#"{"schema":"rk.setup-status/1","state":"absent"}"#);
        let stale = serde_json::to_string(&StatusDocument::of(&Standing::Stale {
            proof: sample(),
            differences: vec!["target.repo".to_owned()],
        }))
        .expect("ok");
        assert_eq!(
            stale,
            format!(
                r#"{{"schema":"rk.setup-status/1","state":"stale","checkpoint":{{"rk_version":"0.0.0","verified_at":"2026-09-27T00:00:00Z","subject_digest":"{}"}},"differences":["target.repo"],"steps":[{{"name":"default-branch","state":"satisfied"}},{{"name":"auto-merge","state":"satisfied-with-limitation","limitation":"weaker"}}]}}"#,
                digest(&sample_subject())
            )
        );
        let invalid = serde_json::to_string(&StatusDocument::of(&Standing::Invalid {
            reason: "malformed",
            detail: "not json".to_owned(),
        }))
        .expect("ok");
        assert_eq!(
            invalid,
            r#"{"schema":"rk.setup-status/1","state":"invalid","invalid":{"reason":"malformed","detail":"not json"}}"#
        );
    }

    /// Each way a file fails to be a proof reads as its own reason.
    #[test]
    fn each_unreadable_proof_names_its_own_reason() {
        let reason = |text: &str| match parse(text) {
            Err((reason, _)) => reason,
            Ok(proof) => panic!("{proof:?}"),
        };
        assert_eq!(reason("not json"), "malformed");
        assert_eq!(reason(r#"{"schema":"rk.setup-proof/1"}"#), "malformed");
        assert_eq!(reason(r#"{"schema":"rk.setup-proof/2"}"#), "future-schema");
        assert_eq!(reason(r#"{"schema":"rk.status/12"}"#), "unsupported-schema");
        let mut edited = sample();
        edited
            .subject
            .target
            .insert("repo".into(), "acme/other".into());
        assert_eq!(reason(&render(&edited)), "digest-mismatch");
        assert_eq!(parse(&render(&sample())), Ok(sample()));
    }

    /// A changed contract names what changed; an equal one names nothing.
    #[test]
    fn a_changed_contract_names_each_field() {
        let recorded = sample_subject();
        assert!(differences(&recorded, &recorded).is_empty());
        let mut current = recorded.clone();
        current.target.insert("repo".into(), "acme/other".into());
        current.steps[0].proves = "something else".into();
        current.steps[0].stance = "excluded".into();
        assert_eq!(
            differences(&recorded, &current),
            [
                "target.repo",
                "step default-branch proves",
                "step default-branch stance"
            ]
        );
        current.steps.clear();
        assert!(
            differences(&recorded, &current)
                .contains(&"step default-branch is no longer in the step table".to_owned())
        );
    }
}
