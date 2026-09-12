//! The guidance the bundle carries for one target.
//!
//! The interval from the recorded release to the candidate, the files
//! inside it, the filter against the destinations the target has, and
//! the coverage judgment. "No applicable steps" and "history unavailable" are different
//! outcomes, and the code keeps them apart: `covered` with no step is the
//! first, `unavailable` is the second. Partial coverage is a decision the
//! operator takes knowingly, and unavailable guidance for a rendered file
//! the plan writes blocks, because the operator cannot reconcile what
//! nobody described.

use std::collections::{BTreeMap, BTreeSet};

use crate::release::declared::{GuidanceFile, version_below};

use super::readiness::{Evaluation, Precondition, Requirement};
use super::{Choice, Coverage, Decision, Guidance, GuidanceStep, Interval};

/// The decision id partial coverage asks.
pub const PARTIAL_GUIDANCE_DECISION: &str = "partial-guidance";

/// Everything the selection reads.
pub struct Inputs<'a> {
    /// The recorded release, where a record exists.
    pub recorded_version: Option<&'a str>,
    /// The candidate release.
    pub candidate_version: &'a str,
    /// Whether the candidate bundle carries a guidance root at all.
    pub carries_root: bool,
    /// The release above which the bundle describes every release.
    pub since: Option<&'a str>,
    /// Every guidance file the bundle carries, in version order.
    pub files: &'a [GuidanceFile],
    /// Every destination present at the target.
    pub present: &'a BTreeSet<String>,
    /// Whether the operations write a rendered file.
    pub rendered_write: bool,
    /// The decisions the operator selected, by id.
    pub selected: &'a BTreeMap<String, String>,
    /// The evidence the selection cites.
    pub evidence_refs: Vec<String>,
}

/// What the selection produces.
pub struct Selected {
    /// The section, for the plan.
    pub guidance: Guidance,
    /// The coverage precondition, where coverage is a condition at all.
    pub precondition: Option<Precondition>,
    /// The decision partial coverage asks, where it asks one.
    pub decision: Option<Decision>,
}

/// Select and judge.
#[must_use]
pub fn select(inputs: &Inputs<'_>) -> Selected {
    let refs = inputs.evidence_refs.clone();
    let Some(recorded) = inputs.recorded_version else {
        return Selected {
            guidance: Guidance {
                coverage: Coverage::NotNeeded,
                interval: None,
                steps: Vec::new(),
                excluded: 0,
                evidence_refs: refs,
            },
            precondition: None,
            decision: None,
        };
    };
    let interval = Interval {
        from: recorded.to_owned(),
        to: inputs.candidate_version.to_owned(),
    };
    let in_interval = |version: &str| {
        version_below(recorded, version) && !version_below(inputs.candidate_version, version)
    };
    let mut steps = Vec::new();
    let mut excluded = 0;
    for file in inputs
        .files
        .iter()
        .filter(|file| in_interval(&file.version))
    {
        if file
            .destinations
            .iter()
            .any(|destination| inputs.present.contains(destination))
        {
            steps.push(GuidanceStep {
                version: file.version.clone(),
                title: file.title.clone(),
                destinations: file.destinations.clone(),
                action: file.action.clone(),
                body: file.body.clone(),
            });
        } else {
            excluded += 1;
        }
    }
    let empty_interval = !version_below(recorded, inputs.candidate_version);
    let coverage = if empty_interval {
        Coverage::Covered
    } else if !inputs.carries_root {
        Coverage::Unavailable
    } else {
        match inputs.since {
            Some(since) if version_below(recorded, since) => Coverage::Partial {
                since: since.to_owned(),
            },
            Some(_) => Coverage::Covered,
            None => Coverage::Unavailable,
        }
    };
    let (precondition, decision) = judge(&coverage, recorded, inputs, &refs);
    Selected {
        guidance: Guidance {
            coverage,
            interval: Some(interval),
            steps,
            excluded,
            evidence_refs: refs,
        },
        precondition,
        decision,
    }
}

/// The coverage judged: partial asks, unavailable blocks a rendered
/// write, and the rest is no condition at all.
fn judge(
    coverage: &Coverage,
    recorded: &str,
    inputs: &Inputs<'_>,
    refs: &[String],
) -> (Option<Precondition>, Option<Decision>) {
    match coverage {
        Coverage::NotNeeded | Coverage::Covered => (None, None),
        Coverage::Partial { since } => {
            let answered = inputs.selected.get(PARTIAL_GUIDANCE_DECISION).cloned();
            let decision = Decision {
                id: PARTIAL_GUIDANCE_DECISION.into(),
                question: format!(
                    "the bundle describes every release above {since}, and the record is at {recorded}; proceed with the releases between them undescribed?"
                ),
                choices: vec![Choice {
                    answer: "accept".into(),
                    consequence: "the plan carries the steps the bundle does have, and the operator reads the changelog for the rest".into(),
                }],
                selected: answered.clone(),
            };
            let precondition = Precondition {
                id: "guidance-covered".into(),
                requirement: Requirement::DecisionRequired,
                evaluation: if answered.as_deref() == Some("accept") {
                    Evaluation::Satisfied
                } else {
                    Evaluation::NotObserved {
                        reason: format!(
                            "guidance is partial: the releases from {recorded} up to {since} are not described"
                        ),
                    }
                },
                decision: Some(PARTIAL_GUIDANCE_DECISION.into()),
                evidence_refs: refs.to_vec(),
            };
            (Some(precondition), Some(decision))
        }
        Coverage::Unavailable => (
            Some(Precondition {
                id: "guidance-covered".into(),
                requirement: if inputs.rendered_write {
                    Requirement::Required
                } else {
                    Requirement::Advisory
                },
                evaluation: Evaluation::Unsatisfied {
                    reason: format!(
                        "the bundle for release-kit {} carries no guidance, so no release between {recorded} and it is described",
                        inputs.candidate_version
                    ),
                },
                decision: None,
                evidence_refs: refs.to_vec(),
            }),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{Inputs, Selected, select};
    use crate::plan::Coverage;
    use crate::plan::readiness::{self, Evaluation, Readiness, Requirement};
    use crate::release::declared::GuidanceFile;

    fn file(version: &str, destinations: &[&str]) -> GuidanceFile {
        GuidanceFile {
            version: version.to_owned(),
            title: format!("release-kit {version}"),
            destinations: destinations.iter().map(|d| (*d).to_owned()).collect(),
            action: "operator-step".into(),
            body: "## What to do\n\nEdit the line.".into(),
        }
    }

    /// Owned inputs, so a test tunes fields and selects.
    struct Fixture {
        recorded_version: Option<&'static str>,
        candidate_version: &'static str,
        carries_root: bool,
        since: Option<&'static str>,
        files: Vec<GuidanceFile>,
        present: BTreeSet<String>,
        rendered_write: bool,
        selected: BTreeMap<String, String>,
    }

    impl Fixture {
        fn new(files: Vec<GuidanceFile>) -> Self {
            Self {
                recorded_version: Some("0.3.18"),
                candidate_version: "0.3.21",
                carries_root: true,
                since: Some("0.3.18"),
                files,
                present: [".envrc", "SECURITY.md"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                rendered_write: false,
                selected: BTreeMap::new(),
            }
        }

        fn select(&self) -> Selected {
            select(&Inputs {
                recorded_version: self.recorded_version,
                candidate_version: self.candidate_version,
                carries_root: self.carries_root,
                since: self.since,
                files: &self.files,
                present: &self.present,
                rendered_write: self.rendered_write,
                selected: &self.selected,
                evidence_refs: vec!["candidate-bundle".into()],
            })
        }
    }

    #[test]
    fn a_release_with_no_steps_reports_no_applicable_steps() {
        let mut fixture = Fixture::new(Vec::new());
        let covered = fixture.select();
        assert_eq!(covered.guidance.coverage, Coverage::Covered);
        assert!(covered.guidance.steps.is_empty());
        assert!(
            covered.precondition.is_none(),
            "no applicable steps is not a condition"
        );
        fixture.carries_root = false;
        let unavailable = fixture.select();
        assert_eq!(unavailable.guidance.coverage, Coverage::Unavailable);
        assert!(
            unavailable.precondition.is_some(),
            "unavailable is a condition"
        );
        fixture.carries_root = true;
        fixture.since = None;
        assert_eq!(fixture.select().guidance.coverage, Coverage::Unavailable);
        fixture.since = Some("0.3.18");
        fixture.recorded_version = None;
        let fresh = fixture.select();
        assert_eq!(fresh.guidance.coverage, Coverage::NotNeeded);
        assert!(fresh.guidance.interval.is_none());
        let mut current = Fixture::new(vec![file("0.3.19", &[".envrc"])]);
        current.recorded_version = Some("0.3.21");
        let current = current.select();
        assert_eq!(current.guidance.coverage, Coverage::Covered);
        assert!(
            current.guidance.steps.is_empty(),
            "nothing above the record"
        );
    }

    #[test]
    fn guidance_is_filtered_against_the_targets_destinations_with_a_count() {
        let fixture = Fixture::new(vec![
            file("0.3.19", &[".envrc"]),
            file("0.3.20", &["nix/package.nix"]),
            file("0.3.21", &["SECURITY.md", "nix/package.nix"]),
            file("0.3.22", &[".envrc"]),
            file("0.3.18", &[".envrc"]),
        ]);
        let selected = fixture.select();
        let versions: Vec<&str> = selected
            .guidance
            .steps
            .iter()
            .map(|s| s.version.as_str())
            .collect();
        assert_eq!(
            versions,
            ["0.3.19", "0.3.21"],
            "in the interval, and concerning a present destination"
        );
        assert_eq!(
            selected.guidance.excluded, 1,
            "the nix-only step on a target without nix"
        );
        let interval = selected
            .guidance
            .interval
            .expect("a record bounds the interval");
        assert_eq!(
            (interval.from.as_str(), interval.to.as_str()),
            ("0.3.18", "0.3.21")
        );
        assert_eq!(
            selected.guidance.steps[1].destinations,
            ["SECURITY.md", "nix/package.nix"]
        );
    }

    #[test]
    fn partial_guidance_is_a_decision_and_a_selected_decision_resolves_it() {
        let mut fixture = Fixture::new(vec![file("0.3.19", &[".envrc"])]);
        fixture.recorded_version = Some("0.3.10");
        let partial = fixture.select();
        assert_eq!(
            partial.guidance.coverage,
            Coverage::Partial {
                since: "0.3.18".into()
            }
        );
        assert_eq!(
            partial.guidance.steps.len(),
            1,
            "the steps it does have still ride"
        );
        let p = partial
            .precondition
            .as_ref()
            .expect("partial is a condition");
        assert_eq!(p.requirement, Requirement::DecisionRequired);
        assert_eq!(p.decision.as_deref(), Some("partial-guidance"));
        assert!(matches!(p.evaluation, Evaluation::NotObserved { .. }));
        assert_eq!(
            readiness::derive(std::slice::from_ref(p)),
            Readiness::NeedsDecision
        );
        assert_eq!(
            partial.decision.as_ref().map(|d| d.id.as_str()),
            Some("partial-guidance")
        );
        fixture.selected = BTreeMap::from([("partial-guidance".to_owned(), "accept".to_owned())]);
        let accepted = fixture.select();
        let p = accepted.precondition.as_ref().expect("still a condition");
        assert!(p.evaluation.holds());
        assert_eq!(readiness::derive(std::slice::from_ref(p)), Readiness::Ready);
        assert_eq!(
            accepted
                .decision
                .as_ref()
                .and_then(|d| d.selected.as_deref()),
            Some("accept")
        );
    }

    #[test]
    fn unavailable_guidance_for_a_planned_rendered_file_blocks() {
        let mut fixture = Fixture::new(Vec::new());
        fixture.carries_root = false;
        let idle = fixture.select();
        let p = idle.precondition.as_ref().expect("a condition");
        assert_eq!(p.requirement, Requirement::Advisory);
        assert_eq!(readiness::derive(std::slice::from_ref(p)), Readiness::Ready);
        fixture.rendered_write = true;
        let writing = fixture.select();
        let p = writing.precondition.as_ref().expect("a condition");
        assert_eq!(p.requirement, Requirement::Required);
        assert!(matches!(p.evaluation, Evaluation::Unsatisfied { .. }));
        assert_eq!(
            readiness::derive(std::slice::from_ref(p)),
            Readiness::Blocked
        );
        assert!(writing.decision.is_none(), "unavailable is not a decision");
    }
}
