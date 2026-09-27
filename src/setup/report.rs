//! One complete setup observation, typed.
//!
//! A check walks every step once and classifies it twice: where the step
//! stands at this target, which the target configuration answers, and
//! what the forge answered, where the step applies. The two stay separate
//! types, and a command failure is neither: it is an error the walk
//! returns. The human report, the event stream, the check's verdict, and
//! the committed setup proof all read this one classification, so none of
//! them can disagree about what a run found.
//!
//! SATISFIES setup-proof:one-report-owns-every-classification

use super::context::Ctx;
use super::observe::StepState;
use super::steps::StepSpec;

/// Where one step stands at this target, before anything runs.
///
/// Applicability is the profile's answer and carries no operator reason;
/// an exclusion is the operator's own statement about a step that does
/// apply. An exclusion declared for a step that does not apply is
/// redundant: it is reported as such and turns the step into no work.
///
/// SATISFIES forge-setup:applicability-follows-the-target-configuration
#[derive(Debug, Clone)]
pub enum Stance {
    /// The step applies and the run acts on it.
    Applies,
    /// The target configuration does not select it, with the value that
    /// decided so.
    NotApplicable(String),
    /// The target declared it does not run it, with its stated reason.
    Excluded(String),
    /// The target excluded a step that does not apply here.
    Redundant {
        /// The reason the target stated.
        reason: String,
        /// Why the step does not apply either way.
        inapplicable: String,
    },
}

impl Stance {
    /// The word a report and an event use.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Applies => "applicable",
            Self::NotApplicable(_) => "not-applicable",
            Self::Excluded(_) => "excluded",
            Self::Redundant { .. } => "redundant",
        }
    }

    /// Whether the run acts on the step.
    #[must_use]
    pub const fn acts(&self) -> bool {
        matches!(self, Self::Applies)
    }

    /// The reason alone, as an event and a check line carry it.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Applies => String::new(),
            Self::NotApplicable(reason) | Self::Excluded(reason) => reason.clone(),
            Self::Redundant {
                reason,
                inapplicable,
            } => format!("{inapplicable}; the stated reason was {reason}"),
        }
    }

    /// The same, framed by what decided it, as a preview and an apply
    /// name it.
    #[must_use]
    pub fn framed(&self) -> String {
        match self {
            Self::Applies => String::new(),
            Self::NotApplicable(reason) => format!("not applicable: {reason}"),
            Self::Excluded(reason) => {
                format!("excluded by {}: {reason}", crate::config::CONFIG_PATH)
            }
            Self::Redundant { .. } => format!(
                "{} excludes a step that does not apply here: {}",
                crate::config::CONFIG_PATH,
                self.detail()
            ),
        }
    }
}

/// Where `step` stands at this target.
#[must_use]
pub fn stance(ctx: &Ctx, step: &StepSpec) -> Stance {
    let inapplicable = (step.applies)(ctx);
    match (ctx.excluded(step.name).map(str::to_owned), inapplicable) {
        (Some(reason), Some(inapplicable)) => Stance::Redundant {
            reason,
            inapplicable,
        },
        (Some(reason), None) => Stance::Excluded(reason),
        (None, Some(reason)) => Stance::NotApplicable(reason),
        (None, None) => Stance::Applies,
    }
}

/// What the forge answered for a step that applies, without the words a
/// person reads: the classification alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// The desired state holds, with the weaker guarantee named where the
    /// forge enforces less than the step's proof claims.
    Satisfied {
        /// The stable limitation text, where one exists.
        limitation: Option<String>,
    },
    /// An optional step's condition does not hold: nothing is wrong and
    /// nothing is proven.
    Skipped,
    /// The desired state does not hold.
    Unsatisfied,
    /// The observation could not decide.
    Unknown,
}

impl Observed {
    /// The classification of one observer answer.
    #[must_use]
    pub fn of(state: &StepState) -> Self {
        match state {
            StepState::Satisfied { limitation, .. } => Self::Satisfied {
                limitation: limitation.clone(),
            },
            StepState::Inapplicable { .. } => Self::Skipped,
            StepState::Unsatisfied { .. } => Self::Unsatisfied,
            StepState::Unknown { .. } => Self::Unknown,
        }
    }

    /// The word a human report line opens with.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Satisfied { .. } => "ok",
            Self::Skipped => "skipped",
            Self::Unsatisfied => "unsatisfied",
            Self::Unknown => "unknown",
        }
    }

    /// The word an event and a machine record carry.
    #[must_use]
    pub const fn wire(&self) -> &'static str {
        match self {
            Self::Satisfied {
                limitation: Some(_),
            } => "satisfied-with-limitation",
            Self::Satisfied { limitation: None } => "satisfied",
            Self::Skipped => "skipped",
            Self::Unsatisfied => "unsatisfied",
            Self::Unknown => "unknown",
        }
    }
}

/// One step's row in a complete observation.
#[derive(Debug, Clone)]
pub struct Row {
    /// The step, from the step table.
    pub name: &'static str,
    /// Where the step stands at this target.
    pub stance: Stance,
    /// What the forge answered, where the step applies; `None` where the
    /// run did not act on it, so no observation is ever fabricated.
    pub observed: Option<Observed>,
    /// What the observer found, one line, for the report a person reads
    /// now. It is never recorded: it can carry a forge's own words.
    pub detail: String,
}

impl Row {
    /// A row the run did not act on.
    #[must_use]
    pub const fn stated(step: &StepSpec, stance: Stance) -> Self {
        Self {
            name: step.name,
            stance,
            observed: None,
            detail: String::new(),
        }
    }

    /// A row the run observed.
    #[must_use]
    pub fn observed(step: &StepSpec, stance: Stance, state: &StepState) -> Self {
        Self {
            name: step.name,
            stance,
            observed: Some(Observed::of(state)),
            detail: state.detail().to_owned(),
        }
    }

    /// The human report line.
    #[must_use]
    pub fn line(&self) -> String {
        let Some(observed) = &self.observed else {
            return format!(
                "{} {} — {}",
                self.stance.word(),
                self.name,
                self.stance.detail()
            );
        };
        let mut line = format!("{} {} — {}", observed.label(), self.name, self.detail);
        if let Observed::Satisfied {
            limitation: Some(limit),
        } = observed
        {
            use std::fmt::Write as _;
            let _ = write!(line, " (limitation: {limit})");
        }
        line
    }

    /// The status and the detail an event carries.
    #[must_use]
    pub fn event_fields(&self) -> (String, String) {
        match &self.observed {
            None => (self.stance.word().to_owned(), self.stance.detail()),
            Some(Observed::Satisfied { .. }) => ("satisfied".to_owned(), self.detail.clone()),
            Some(observed) => (observed.wire().to_owned(), self.detail.clone()),
        }
    }
}

/// Every step's row, in step-table order.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// One row per step.
    pub rows: Vec<Row>,
}

impl Report {
    fn count(&self, wanted: &Observed) -> usize {
        self.rows
            .iter()
            .filter(|row| row.observed.as_ref() == Some(wanted))
            .count()
    }

    /// The applicable steps whose desired state does not hold.
    #[must_use]
    pub fn unsatisfied(&self) -> usize {
        self.count(&Observed::Unsatisfied)
    }

    /// The applicable steps the observation could not decide.
    #[must_use]
    pub fn unknown(&self) -> usize {
        self.count(&Observed::Unknown)
    }

    /// The steps the run acted on.
    #[must_use]
    pub fn judged(&self) -> usize {
        self.rows.iter().filter(|row| row.stance.acts()).count()
    }

    /// Whether this observation may become a setup proof: every
    /// applicable step holds or was skipped by its own condition, and
    /// nothing is unknown.
    ///
    /// SATISFIES setup-proof:a-checkpoint-records-only-a-complete-observation
    #[must_use]
    pub fn checkpointable(&self) -> bool {
        self.unsatisfied() == 0 && self.unknown() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::steps::STEPS;

    fn row(observed: Option<Observed>) -> Row {
        let stance = if observed.is_some() {
            Stance::Applies
        } else {
            Stance::Excluded("the target runs it elsewhere".into())
        };
        Row {
            name: STEPS[0].name,
            stance,
            observed,
            detail: String::new(),
        }
    }

    /// Only a complete observation may become a proof: one unknown or one
    /// unsatisfied row refuses it, and every other state is accepted.
    #[test]
    fn only_a_complete_observation_is_checkpointable() {
        let accepted = Report {
            rows: vec![
                row(Some(Observed::Satisfied { limitation: None })),
                row(Some(Observed::Satisfied {
                    limitation: Some("weaker".into()),
                })),
                row(Some(Observed::Skipped)),
                row(None),
            ],
        };
        assert!(accepted.checkpointable());
        for refused in [Observed::Unknown, Observed::Unsatisfied] {
            let mut report = accepted.clone();
            report.rows.push(row(Some(refused.clone())));
            assert!(!report.checkpointable(), "{refused:?} was accepted");
        }
    }

    /// Every observer answer keeps its distinction through the
    /// classification.
    #[test]
    fn an_observation_keeps_its_distinctions() {
        let limited = StepState::Satisfied {
            detail: "found".into(),
            limitation: Some("weaker".into()),
        };
        assert_eq!(Observed::of(&limited).wire(), "satisfied-with-limitation");
        let plain = StepState::Satisfied {
            detail: "found".into(),
            limitation: None,
        };
        assert_eq!(Observed::of(&plain).wire(), "satisfied");
        let skipped = StepState::Inapplicable {
            detail: "no line".into(),
        };
        assert_eq!(Observed::of(&skipped).wire(), "skipped");
        let unknown = StepState::Unknown {
            detail: "unreadable".into(),
        };
        assert_eq!(Observed::of(&unknown).wire(), "unknown");
        let not = StepState::Unsatisfied {
            detail: "absent".into(),
        };
        assert_eq!(Observed::of(&not).wire(), "unsatisfied");
    }
}
