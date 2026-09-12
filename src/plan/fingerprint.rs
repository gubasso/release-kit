//! The input fingerprint: one canonical digest over the semantic inputs
//! of a plan, and nothing else.
//!
//! Approval binds to this digest. Apply regenerates the inputs,
//! recomputes it, and refuses on any difference, naming what moved. The
//! inputs are the candidate bundle's digests, the record and
//! configuration digests, every operation's path, kind, and before and
//! after digests, every required precondition's evaluation, and every
//! selected decision. Excluded on purpose: timestamps, presentation
//! text, advisory evaluations, and whether a view showed bytes inline or
//! by digest.

use crate::digest::Digest;

use super::readiness::Requirement;
use super::{ConfigurationState, Plan, RecordState};

/// The canonical text the fingerprint digests, one input per line, in a
/// fixed order, so two plans over the same inputs produce the same text.
#[must_use]
pub fn canonical(plan: &Plan) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "candidate\t{}\t{}",
        plan.release.candidate.payload_sha256, plan.release.candidate.payload_schema
    ));
    lines.push(format!(
        "record\t{}",
        match &plan.observed_state.installation.record {
            RecordState::Absent => "absent".to_owned(),
            RecordState::Present { sha256, .. } => sha256.to_string(),
            RecordState::Invalid { reason } => format!("invalid\t{reason}"),
        }
    ));
    lines.push(format!(
        "configuration\t{}",
        match &plan.observed_state.installation.configuration {
            ConfigurationState {
                sha256: Some(digest),
                ..
            } => digest.to_string(),
            ConfigurationState {
                invalid: Some(reason),
                ..
            } => format!("invalid\t{reason}"),
            ConfigurationState { .. } => "absent".to_owned(),
        }
    ));
    let mut operations: Vec<String> = plan
        .operations
        .iter()
        .map(|operation| format!("operation\t{}", operation.canonical()))
        .collect();
    operations.sort();
    lines.extend(operations);
    let mut required: Vec<String> = plan
        .preconditions
        .iter()
        .filter(|precondition| precondition.requirement == Requirement::Required)
        .map(|precondition| {
            format!(
                "precondition\t{}\t{}",
                precondition.id,
                precondition.evaluation.word()
            )
        })
        .collect();
    required.sort();
    lines.extend(required);
    let mut selected: Vec<String> = plan
        .decisions
        .iter()
        .filter_map(|decision| {
            decision
                .selected
                .as_ref()
                .map(|answer| format!("decision\t{}\t{answer}", decision.id))
        })
        .collect();
    selected.sort();
    lines.extend(selected);
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// The fingerprint over the canonical text.
#[must_use]
pub fn compute(plan: &Plan) -> Digest {
    Digest::of(canonical(plan).as_bytes())
}

/// The plan id: the first sixteen hex digits of a digest over the
/// fingerprint and the creation instant.
#[must_use]
pub fn plan_id(fingerprint: &Digest, created_at: &str) -> String {
    let digest = Digest::of(format!("{fingerprint}\t{created_at}").as_bytes());
    digest.to_string().chars().take(16).collect()
}
