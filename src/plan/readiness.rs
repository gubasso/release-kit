//! The readiness policy: what a plan's preconditions require, what each
//! one was found to be, and the one derivation that turns the two into a
//! verdict.
//!
//! A gap is honest and is not permission. Every precondition carries a
//! requirement, and the plan's readiness is the worst precondition: a
//! required one not satisfied blocks, a decision-required one not yet
//! answered asks, and an advisory one never counts. Apply proceeds on
//! `ready` alone, and there is no flag that makes a gap into a pass.

use serde::{Deserialize, Serialize};

/// What a precondition demands of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Requirement {
    /// Reported, never blocking: a fact the operator wants in front of
    /// them and nothing in the operations depends on.
    Advisory,
    /// The operator owns the answer: the plan waits until a decision with
    /// the matching id is selected.
    DecisionRequired,
    /// Nothing proceeds until it holds.
    Required,
}

impl Requirement {
    /// The wire form, identical to the serde rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::DecisionRequired => "decision-required",
            Self::Required => "required",
        }
    }
}

/// What one precondition was found to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Evaluation {
    /// The condition holds.
    Satisfied,
    /// Nothing could say either way, and the reason names why.
    NotObserved {
        /// Why the observation is missing.
        reason: String,
    },
    /// The condition does not hold, and the reason names what was found.
    Unsatisfied {
        /// What was found instead.
        reason: String,
    },
}

impl Evaluation {
    /// The state word, for the fingerprint's canonical form.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::NotObserved { .. } => "not-observed",
            Self::Unsatisfied { .. } => "unsatisfied",
        }
    }

    /// Whether the condition holds.
    #[must_use]
    pub const fn holds(&self) -> bool {
        matches!(self, Self::Satisfied)
    }
}

/// One precondition of a plan: a stable id, what it requires, what it
/// was found to be, and the decision that resolves it where one does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Precondition {
    /// A stable id, the same across re-plans of the same target.
    pub id: String,
    /// The policy on this precondition.
    pub requirement: Requirement,
    /// What was found.
    pub evaluation: Evaluation,
    /// The decision whose selected answer satisfies this precondition,
    /// where the requirement is decision-required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    /// The evidence this evaluation rests on.
    pub evidence_refs: Vec<String>,
}

/// Whether the plan may be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Readiness {
    /// Every required precondition holds and every decision is answered.
    Ready,
    /// A decision the operator owns is not yet selected.
    NeedsDecision,
    /// A required precondition does not hold.
    Blocked,
}

impl Readiness {
    /// The wire form, identical to the serde rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NeedsDecision => "needs-decision",
            Self::Blocked => "blocked",
        }
    }
}

/// The one derivation: the worst precondition decides.
///
/// A required precondition that does not hold, observed or not, blocks.
/// A decision-required one that does not hold asks. An advisory one is
/// reported and never counts.
#[must_use]
pub fn derive(preconditions: &[Precondition]) -> Readiness {
    let mut readiness = Readiness::Ready;
    for precondition in preconditions {
        if precondition.evaluation.holds() {
            continue;
        }
        match precondition.requirement {
            Requirement::Required => return Readiness::Blocked,
            Requirement::DecisionRequired => readiness = Readiness::NeedsDecision,
            Requirement::Advisory => {}
        }
    }
    readiness
}

#[cfg(test)]
mod tests {
    use super::{Evaluation, Precondition, Readiness, Requirement, derive};

    fn precondition(requirement: Requirement, evaluation: Evaluation) -> Precondition {
        Precondition {
            id: "one".into(),
            requirement,
            evaluation,
            decision: None,
            evidence_refs: Vec::new(),
        }
    }

    /// The table: three requirements against three evaluations, and the
    /// worst one wins when several stand together.
    #[test]
    fn readiness_is_the_worst_precondition() {
        let not_observed = || Evaluation::NotObserved {
            reason: "unread".into(),
        };
        let unsatisfied = || Evaluation::Unsatisfied {
            reason: "found otherwise".into(),
        };
        let table: Vec<(Requirement, Evaluation, Readiness)> = vec![
            (
                Requirement::Advisory,
                Evaluation::Satisfied,
                Readiness::Ready,
            ),
            (Requirement::Advisory, not_observed(), Readiness::Ready),
            (Requirement::Advisory, unsatisfied(), Readiness::Ready),
            (
                Requirement::DecisionRequired,
                Evaluation::Satisfied,
                Readiness::Ready,
            ),
            (
                Requirement::DecisionRequired,
                not_observed(),
                Readiness::NeedsDecision,
            ),
            (
                Requirement::DecisionRequired,
                unsatisfied(),
                Readiness::NeedsDecision,
            ),
            (
                Requirement::Required,
                Evaluation::Satisfied,
                Readiness::Ready,
            ),
            (Requirement::Required, not_observed(), Readiness::Blocked),
            (Requirement::Required, unsatisfied(), Readiness::Blocked),
        ];
        for (requirement, evaluation, expected) in table {
            let got = derive(&[precondition(requirement, evaluation.clone())]);
            assert_eq!(got, expected, "{requirement:?} {evaluation:?}");
        }
        assert_eq!(derive(&[]), Readiness::Ready);
        let mixed = [
            precondition(Requirement::Advisory, unsatisfied()),
            precondition(Requirement::DecisionRequired, not_observed()),
            precondition(Requirement::Required, Evaluation::Satisfied),
        ];
        assert_eq!(derive(&mixed), Readiness::NeedsDecision);
        let blocked = [
            precondition(Requirement::DecisionRequired, not_observed()),
            precondition(Requirement::Required, unsatisfied()),
        ];
        assert_eq!(derive(&blocked), Readiness::Blocked);
    }

    #[test]
    fn the_words_are_the_wire_form() {
        for (requirement, word) in [
            (Requirement::Advisory, "advisory"),
            (Requirement::DecisionRequired, "decision-required"),
            (Requirement::Required, "required"),
        ] {
            assert_eq!(requirement.as_str(), word);
            assert_eq!(
                serde_json::to_string(&requirement).expect("serializes"),
                format!("\"{word}\"")
            );
        }
        for (readiness, word) in [
            (Readiness::Ready, "ready"),
            (Readiness::NeedsDecision, "needs-decision"),
            (Readiness::Blocked, "blocked"),
        ] {
            assert_eq!(readiness.as_str(), word);
            assert_eq!(
                serde_json::to_string(&readiness).expect("serializes"),
                format!("\"{word}\"")
            );
        }
        assert_eq!(
            serde_json::to_string(&Evaluation::NotObserved {
                reason: "unread".into()
            })
            .expect("serializes"),
            r#"{"state":"not-observed","reason":"unread"}"#
        );
    }
}
