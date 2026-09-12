//! Classification: the corpus verdict a target earns before anything
//! lands, and the plan classification derived from it and the record.
//!
//! Two words answer two questions. The verdict — `greenfield`,
//! `brownfield`, or `needs-decision` — is what `rk assess` has always
//! answered and reads the repository alone. The plan classification —
//! `setup`, `migration`, `upgrade`, `drift`, or `invalid` — names which
//! procedure a plan is, and it reads the verdict beside the record and
//! the comparison. One enum carries the routing word, and the findings
//! beside it carry the detail the word compresses: the marker that made
//! a target brownfield, the file that made a landing drifted, the reason
//! a record is invalid. A reader routes on the word and reads the
//! findings, and neither needs the other to change shape when a new
//! finding appears.

use serde::Serialize;

/// The corpus verdict, computed from the repository's evidence alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No release mechanism and no release history: land the workflow.
    Greenfield,
    /// A release mechanism is in place: migrate, never land beside it.
    Brownfield,
    /// Release activity no mechanism explains: the operator decides.
    NeedsDecision,
}

impl Verdict {
    /// The kebab-case verdict word, as the JSON serializes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Greenfield => "greenfield",
            Self::Brownfield => "brownfield",
            Self::NeedsDecision => "needs-decision",
        }
    }
}

/// The repository facts the verdict reads: a projection of the
/// assessment's evidence, so the rule stays pure and testable.
#[derive(Debug, Clone, Default)]
pub struct RepositoryFacts {
    /// Release-mechanism files of other tools found at the target.
    pub release_markers: Vec<String>,
    /// Payload destinations already present.
    pub collisions: Vec<String>,
    /// How many tags the repository holds.
    pub tags: usize,
    /// Long-lived branches found besides the trunk.
    pub long_lived_branches: Vec<String>,
}

/// Compute the verdict from the facts. Pure, so the rule is testable
/// without a repository.
#[must_use]
pub fn verdict(facts: &RepositoryFacts) -> Verdict {
    if !facts.release_markers.is_empty() || !facts.collisions.is_empty() {
        return Verdict::Brownfield;
    }
    if facts.tags > 0 || !facts.long_lived_branches.is_empty() {
        return Verdict::NeedsDecision;
    }
    Verdict::Greenfield
}

/// Which procedure a plan is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// No record and a greenfield verdict: a first landing.
    Setup,
    /// No record and a mechanism or a history in the way: a migration,
    /// gated by what the findings name.
    Migration,
    /// A record, and every owned file as the record left it: a newer
    /// payload taken, or nothing to take.
    Upgrade,
    /// A record, and an owned file the target edited or removed: blocked
    /// until reconciled.
    Drift,
    /// A record this engine cannot read: nothing is planned over it.
    Invalid,
}

impl Classification {
    /// The wire form, identical to the serde rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Migration => "migration",
            Self::Upgrade => "upgrade",
            Self::Drift => "drift",
            Self::Invalid => "invalid",
        }
    }
}

/// One fact the classification compresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// A stable code: `release-marker`, `payload-collision`, `tag`,
    /// `long-lived-branch`, `owned-drift`, `owned-missing`,
    /// `record-invalid`, `record-newer`.
    pub code: &'static str,
    /// What was found, one line.
    pub detail: String,
}

/// What the record was found to be, for the classification alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordState {
    /// No record at the target.
    Absent,
    /// A record this engine read.
    Present,
    /// A record this engine could not read.
    Invalid,
}

/// Derive the classification from the record's state, the verdict, and
/// whether the comparison found owned drift.
#[must_use]
pub const fn classify(record: RecordState, verdict: Verdict, owned_drift: bool) -> Classification {
    match record {
        RecordState::Invalid => Classification::Invalid,
        RecordState::Absent => match verdict {
            Verdict::Greenfield => Classification::Setup,
            Verdict::Brownfield | Verdict::NeedsDecision => Classification::Migration,
        },
        RecordState::Present => {
            if owned_drift {
                Classification::Drift
            } else {
                Classification::Upgrade
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Classification, RecordState, RepositoryFacts, Verdict, classify, verdict};

    #[test]
    fn nothing_is_greenfield() {
        assert_eq!(verdict(&RepositoryFacts::default()), Verdict::Greenfield);
    }

    #[test]
    fn a_release_marker_or_a_collision_is_brownfield() {
        let with_marker = RepositoryFacts {
            release_markers: vec!["CHANGELOG.md".into()],
            ..RepositoryFacts::default()
        };
        assert_eq!(verdict(&with_marker), Verdict::Brownfield);
        let with_collision = RepositoryFacts {
            collisions: vec!["release-plz.toml".into()],
            ..RepositoryFacts::default()
        };
        assert_eq!(verdict(&with_collision), Verdict::Brownfield);
    }

    /// A mechanism outranks unexplained activity: tags beside a marker
    /// are a history the mechanism made, not a question.
    #[test]
    fn a_mechanism_beside_activity_is_still_brownfield() {
        let both = RepositoryFacts {
            release_markers: vec!["CHANGELOG.md".into()],
            tags: 7,
            long_lived_branches: vec!["develop".into()],
            ..RepositoryFacts::default()
        };
        assert_eq!(verdict(&both), Verdict::Brownfield);
    }

    #[test]
    fn activity_with_no_mechanism_needs_a_decision() {
        let tagged = RepositoryFacts {
            tags: 1,
            ..RepositoryFacts::default()
        };
        assert_eq!(verdict(&tagged), Verdict::NeedsDecision);
        let branched = RepositoryFacts {
            long_lived_branches: vec!["develop".into()],
            ..RepositoryFacts::default()
        };
        assert_eq!(verdict(&branched), Verdict::NeedsDecision);
    }

    /// The five classifications over the six target states: empty,
    /// brownfield, landed-current and landed-old (both upgrade: the
    /// operations tell them apart), drifted, and invalid.
    #[test]
    fn the_classification_table_covers_the_six_states() {
        assert_eq!(
            classify(RecordState::Absent, Verdict::Greenfield, false),
            Classification::Setup
        );
        assert_eq!(
            classify(RecordState::Absent, Verdict::Brownfield, false),
            Classification::Migration
        );
        assert_eq!(
            classify(RecordState::Absent, Verdict::NeedsDecision, false),
            Classification::Migration
        );
        assert_eq!(
            classify(RecordState::Present, Verdict::Brownfield, false),
            Classification::Upgrade
        );
        assert_eq!(
            classify(RecordState::Present, Verdict::Brownfield, true),
            Classification::Drift
        );
        assert_eq!(
            classify(RecordState::Invalid, Verdict::Brownfield, true),
            Classification::Invalid
        );
    }

    #[test]
    fn the_words_are_the_wire_form() {
        for (classification, word) in [
            (Classification::Setup, "setup"),
            (Classification::Migration, "migration"),
            (Classification::Upgrade, "upgrade"),
            (Classification::Drift, "drift"),
            (Classification::Invalid, "invalid"),
        ] {
            assert_eq!(classification.as_str(), word);
            assert_eq!(
                serde_json::to_string(&classification).expect("serializes"),
                format!("\"{word}\"")
            );
        }
        assert_eq!(Verdict::Greenfield.as_str(), "greenfield");
        assert_eq!(Verdict::Brownfield.as_str(), "brownfield");
        assert_eq!(Verdict::NeedsDecision.as_str(), "needs-decision");
    }
}
