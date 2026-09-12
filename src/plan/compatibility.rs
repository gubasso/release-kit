//! The compatibility axes beyond the payload schema, each evaluated into
//! a precondition with a requirement.
//!
//! The seam settles engine-against-payload through `payload_schema`. Four
//! axes remain, and each one has stranded someone: the engine floor a
//! bundle names where the schema is too coarse, the generator a committed
//! artifact needs at its pin, the manager a pin move would go through,
//! and the forge floor the landed files rest on, plus the intermediate
//! release a landing must pass through. The facts land in the plan's
//! `release.compatibility` section; the policy lands in the preconditions.

use std::collections::BTreeMap;

use crate::release::declared::{self, version_below};

use super::readiness::{Evaluation, Precondition, Requirement};
use super::{Choice, Compatibility, Decision, ForgeFloor, GeneratorFact, IntermediateFact};

/// The generator a technology's committed artifact needs, as
/// `versions.toml` names it, and the artifact it regenerates.
#[must_use]
pub fn generator_for(tech: &str) -> Option<(&'static str, &'static str)> {
    match tech {
        "rust" => Some(("cargo-dist", "dist-workspace.toml")),
        _ => None,
    }
}

/// The decision id for a pin move through a manager the target does not
/// wire.
pub const PIN_MANAGER_DECISION: &str = "pin-manager";

/// Everything the evaluation reads.
pub struct Inputs<'a> {
    /// What the candidate bundle declares.
    pub declared: &'a declared::Compatibility,
    /// The engine computing the plan.
    pub engine_version: &'a str,
    /// The engine's protocol version.
    pub engine_schema: u32,
    /// The candidate's protocol version.
    pub bundle_schema: u32,
    /// The candidate release.
    pub candidate_version: &'a str,
    /// The recorded release, where a record exists.
    pub recorded_version: Option<&'a str>,
    /// The resolved technology, where one resolved.
    pub tech: Option<&'a str>,
    /// The resolved forge, where one resolved.
    pub forge: Option<&'a str>,
    /// The candidate's pins, by tool name.
    pub pins: &'a BTreeMap<String, String>,
    /// The generator's version on the host, where one was read.
    pub host_generator: Option<&'a str>,
    /// Every path the operations write.
    pub writes: &'a [String],
    /// The paths the operations rewrite: a file the target already holds.
    /// A first write generates nothing yet, so only a rewrite of the
    /// generator's artifact needs the generator on the host.
    pub rewrites: &'a [String],
    /// Whether a manager file names release-kit.
    pub pin_wired: bool,
    /// The managers whose file is present and names no release-kit.
    pub unwired_managers: &'a [String],
    /// The forge's version, where the forge was asked.
    pub forge_version: Option<&'a str>,
    /// The decisions the operator selected, by id.
    pub selected: &'a BTreeMap<String, String>,
    /// The evidence the axes cite.
    pub evidence_refs: Vec<String>,
}

/// What the evaluation produces.
pub struct Evaluated {
    /// The facts, for the plan's release section.
    pub facts: Compatibility,
    /// One precondition per axis that applies.
    pub preconditions: Vec<Precondition>,
    /// The decision the manager axis asks, where it asks one.
    pub decisions: Vec<Decision>,
}

/// Evaluate every axis.
#[must_use]
pub fn evaluate(inputs: &Inputs<'_>) -> Evaluated {
    let mut preconditions = Vec::new();
    let mut decisions = Vec::new();
    let refs = inputs.evidence_refs.clone();
    let engine_minimum = engine_axis(inputs, &refs, &mut preconditions);
    let generator = generator_axis(inputs, &refs, &mut preconditions);
    manager_axis(inputs, &refs, &mut preconditions, &mut decisions);
    let forge_floor = forge_axis(inputs, &refs, &mut preconditions);
    let intermediate = intermediate_axis(inputs, &refs, &mut preconditions);
    Evaluated {
        facts: Compatibility {
            engine_schema: inputs.engine_schema,
            bundle_schema: inputs.bundle_schema,
            readable: inputs.bundle_schema <= inputs.engine_schema,
            engine_minimum,
            generator,
            forge_floor,
            intermediate,
            evidence_refs: refs,
        },
        preconditions,
        decisions,
    }
}

/// The engine axis: the bundle names an engine floor the schema alone
/// does not express.
fn engine_axis(
    inputs: &Inputs<'_>,
    refs: &[String],
    preconditions: &mut Vec<Precondition>,
) -> Option<String> {
    let engine_minimum = inputs.declared.engine.minimum.clone();
    if let Some(minimum) = &engine_minimum {
        let below = version_below(inputs.engine_version, minimum);
        preconditions.push(Precondition {
            id: "engine-meets-minimum".into(),
            requirement: Requirement::Required,
            evaluation: if below {
                Evaluation::Unsatisfied {
                    reason: format!(
                        "the bundle for release-kit {} needs engine {minimum} or newer, and this engine is {}; install release-kit {minimum} or newer",
                        inputs.candidate_version, inputs.engine_version
                    ),
                }
            } else {
                Evaluation::Satisfied
            },
            decision: None,
            evidence_refs: refs.to_vec(),
        });
    }
    engine_minimum
}

/// The generator axis: a committed artifact that predates the landed
/// configuration needs the binding's generator at its pin.
fn generator_axis(
    inputs: &Inputs<'_>,
    refs: &[String],
    preconditions: &mut Vec<Precondition>,
) -> Option<GeneratorFact> {
    let generator = inputs
        .tech
        .and_then(generator_for)
        .and_then(|(name, artifact)| {
            let pin = inputs.pins.get(name)?;
            Some(GeneratorFact {
                name: name.to_owned(),
                pin: pin.clone(),
                artifact: artifact.to_owned(),
                host: inputs.host_generator.map(str::to_owned),
            })
        });
    if let Some(fact) = &generator {
        let planned = inputs.rewrites.iter().any(|path| path == &fact.artifact);
        let evaluation = match &fact.host {
            None => Evaluation::NotObserved {
                reason: format!(
                    "{} is not on PATH; the landed {} is regenerated with {} {}",
                    fact.name, fact.artifact, fact.name, fact.pin
                ),
            },
            Some(host) if version_below(host, &fact.pin) => Evaluation::Unsatisfied {
                reason: format!(
                    "{} {host} is on PATH and the landed {} needs {}; install {} {}",
                    fact.name, fact.artifact, fact.pin, fact.name, fact.pin
                ),
            },
            Some(_) => Evaluation::Satisfied,
        };
        preconditions.push(Precondition {
            id: "generator-at-pin".into(),
            requirement: if planned {
                Requirement::Required
            } else {
                Requirement::Advisory
            },
            evaluation,
            decision: None,
            evidence_refs: refs.to_vec(),
        });
    }
    generator
}

/// The manager axis: a pin move through a manager the target does not
/// wire is not an operation, it is a decision. It is asked where a move
/// is wanted, which is a recorded target behind the candidate.
fn manager_axis(
    inputs: &Inputs<'_>,
    refs: &[String],
    preconditions: &mut Vec<Precondition>,
    decisions: &mut Vec<Decision>,
) {
    let move_wanted = inputs
        .recorded_version
        .is_some_and(|recorded| recorded != inputs.candidate_version);
    if !move_wanted || inputs.pin_wired || inputs.unwired_managers.is_empty() {
        return;
    }
    let answered = inputs.selected.get(PIN_MANAGER_DECISION).cloned();
    let managers = inputs.unwired_managers.join(", ");
    decisions.push(Decision {
        id: PIN_MANAGER_DECISION.into(),
        question: format!(
            "{managers} is present and names no release-kit; does the rk pin move through it?"
        ),
        choices: vec![
            Choice {
                answer: "wire".into(),
                consequence: format!(
                    "run rk self-depend add --manager <{}> --apply first, then plan again so the move is an update-pin operation",
                    inputs.unwired_managers.join("|")
                ),
            },
            Choice {
                answer: "host".into(),
                consequence: "rk stays a host install here, nothing pins it, and the plan carries no pin move".into(),
            },
        ],
        selected: answered.clone(),
    });
    preconditions.push(Precondition {
        id: "pin-manager-answered".into(),
        requirement: Requirement::DecisionRequired,
        evaluation: if answered.is_some() {
            Evaluation::Satisfied
        } else {
            Evaluation::NotObserved {
                reason: format!(
                    "{managers} could pin rk and the operator has not said whether it does"
                ),
            }
        },
        decision: Some(PIN_MANAGER_DECISION.into()),
        evidence_refs: refs.to_vec(),
    });
}

/// The forge axis: the landed pipeline rests on a forge floor, and the
/// floor is a fact the plan carries rather than a discovery at setup.
fn forge_axis(
    inputs: &Inputs<'_>,
    refs: &[String],
    preconditions: &mut Vec<Precondition>,
) -> Option<ForgeFloor> {
    let forge_floor = inputs.forge.and_then(|forge| {
        let minimum = inputs.declared.forge.get(forge)?.minimum.clone()?;
        Some(ForgeFloor {
            forge: forge.to_owned(),
            minimum,
            observed: inputs.forge_version.map(str::to_owned),
        })
    });
    if let Some(floor) = &forge_floor {
        let pipeline_planned = inputs
            .writes
            .iter()
            .any(|path| path == ".gitlab-ci.yml" || path.starts_with(".gitlab/ci/"));
        let below = floor
            .observed
            .as_deref()
            .is_some_and(|observed| version_below(observed, &floor.minimum));
        preconditions.push(Precondition {
            id: "forge-meets-floor".into(),
            requirement: if below && pipeline_planned {
                Requirement::Required
            } else {
                Requirement::Advisory
            },
            evaluation: match &floor.observed {
                None => Evaluation::NotObserved {
                    reason: format!(
                        "the {} version was not read; --observe forge reads it, and rk setup refuses an instance below {}",
                        floor.forge, floor.minimum
                    ),
                },
                Some(observed) if below => Evaluation::Unsatisfied {
                    reason: format!(
                        "this {} instance reports {observed}, and the landed files need {} or newer",
                        floor.forge, floor.minimum
                    ),
                },
                Some(_) => Evaluation::Satisfied,
            },
            decision: None,
            evidence_refs: refs.to_vec(),
        });
    }
    forge_floor
}

/// The intermediate axis: a release the landing must pass through,
/// between the record and the candidate.
fn intermediate_axis(
    inputs: &Inputs<'_>,
    refs: &[String],
    preconditions: &mut Vec<Precondition>,
) -> Vec<IntermediateFact> {
    let intermediate: Vec<IntermediateFact> = inputs
        .recorded_version
        .map(|recorded| {
            inputs
                .declared
                .intermediate
                .iter()
                .filter(|step| {
                    version_below(recorded, &step.version)
                        && version_below(&step.version, inputs.candidate_version)
                })
                .map(|step| IntermediateFact {
                    version: step.version.clone(),
                    reason: step.reason.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    for step in &intermediate {
        preconditions.push(Precondition {
            id: format!("intermediate-release:{}", step.version),
            requirement: Requirement::Required,
            evaluation: Evaluation::Unsatisfied {
                reason: format!(
                    "the landing must pass through release-kit {} first: {}; plan with --to {}",
                    step.version, step.reason, step.version
                ),
            },
            decision: None,
            evidence_refs: refs.to_vec(),
        });
    }
    intermediate
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Evaluated, Inputs, evaluate};
    use crate::plan::readiness::{self, Evaluation, Precondition, Readiness, Requirement};
    use crate::release::declared::{Compatibility, parse_compatibility};

    /// Owned inputs, so a test tunes fields and evaluates.
    struct Fixture {
        declared: Compatibility,
        engine_version: &'static str,
        candidate_version: &'static str,
        recorded_version: Option<&'static str>,
        tech: Option<&'static str>,
        forge: Option<&'static str>,
        pins: BTreeMap<String, String>,
        host_generator: Option<&'static str>,
        writes: Vec<String>,
        rewrites: Vec<String>,
        pin_wired: bool,
        unwired_managers: Vec<String>,
        forge_version: Option<&'static str>,
        selected: BTreeMap<String, String>,
    }

    impl Fixture {
        fn new(declared: Compatibility) -> Self {
            Self {
                declared,
                engine_version: "0.3.18",
                candidate_version: "0.3.21",
                recorded_version: Some("0.3.18"),
                tech: Some("rust"),
                forge: Some("gitlab"),
                pins: BTreeMap::from([("cargo-dist".to_owned(), "0.32.0".to_owned())]),
                host_generator: None,
                writes: Vec::new(),
                rewrites: Vec::new(),
                pin_wired: false,
                unwired_managers: Vec::new(),
                forge_version: None,
                selected: BTreeMap::new(),
            }
        }

        fn evaluate(&self) -> Evaluated {
            evaluate(&Inputs {
                declared: &self.declared,
                engine_version: self.engine_version,
                engine_schema: 1,
                bundle_schema: 1,
                candidate_version: self.candidate_version,
                recorded_version: self.recorded_version,
                tech: self.tech,
                forge: self.forge,
                pins: &self.pins,
                host_generator: self.host_generator,
                writes: &self.writes,
                rewrites: &self.rewrites,
                pin_wired: self.pin_wired,
                unwired_managers: &self.unwired_managers,
                forge_version: self.forge_version,
                selected: &self.selected,
                evidence_refs: vec!["candidate-bundle".into()],
            })
        }
    }

    fn precondition<'a>(evaluated: &'a Evaluated, id: &str) -> &'a Precondition {
        evaluated
            .preconditions
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("precondition {id} exists"))
    }

    #[test]
    fn a_bundle_without_compatibility_requires_only_its_schema() {
        let mut fixture = Fixture::new(Compatibility::default());
        fixture.forge = Some("github");
        let evaluated = fixture.evaluate();
        let ids: Vec<&str> = evaluated
            .preconditions
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(
            ids,
            ["generator-at-pin"],
            "the schema and the binding's generator are the only axes an empty declaration leaves"
        );
        assert_eq!(
            precondition(&evaluated, "generator-at-pin").requirement,
            Requirement::Advisory
        );
        assert!(evaluated.facts.engine_minimum.is_none());
        assert!(evaluated.facts.forge_floor.is_none());
        assert!(evaluated.facts.intermediate.is_empty());
        assert!(evaluated.decisions.is_empty());
        assert_eq!(
            readiness::derive(&evaluated.preconditions),
            Readiness::Ready
        );
    }

    #[test]
    fn an_engine_below_requirement_is_blocked_naming_the_engine() {
        let declared = parse_compatibility("[engine]\nminimum = \"0.4.0\"\n").expect("parses");
        let mut fixture = Fixture::new(declared);
        let evaluated = fixture.evaluate();
        let p = precondition(&evaluated, "engine-meets-minimum");
        assert_eq!(p.requirement, Requirement::Required);
        let Evaluation::Unsatisfied { reason } = &p.evaluation else {
            panic!(
                "an engine below the floor is unsatisfied: {:?}",
                p.evaluation
            );
        };
        assert!(reason.contains("install release-kit 0.4.0"), "{reason}");
        assert_eq!(
            readiness::derive(&evaluated.preconditions),
            Readiness::Blocked
        );
        fixture.engine_version = "0.4.0";
        let ok = fixture.evaluate();
        assert!(precondition(&ok, "engine-meets-minimum").evaluation.holds());
    }

    #[test]
    fn a_missing_generator_blocks_only_when_a_generated_artifact_is_planned() {
        let mut fixture = Fixture::new(Compatibility::default());
        let idle = fixture.evaluate();
        let p = precondition(&idle, "generator-at-pin");
        assert_eq!(p.requirement, Requirement::Advisory);
        assert!(matches!(p.evaluation, Evaluation::NotObserved { .. }));
        assert_eq!(readiness::derive(&idle.preconditions), Readiness::Ready);
        // A first landing writes the artifact and generates nothing yet.
        fixture.writes = vec!["dist-workspace.toml".to_owned()];
        let first = fixture.evaluate();
        assert_eq!(
            precondition(&first, "generator-at-pin").requirement,
            Requirement::Advisory
        );
        assert_eq!(readiness::derive(&first.preconditions), Readiness::Ready);
        // A rewrite of an artifact the target already holds regenerates.
        fixture.rewrites = vec!["dist-workspace.toml".to_owned()];
        let planned = fixture.evaluate();
        let p = precondition(&planned, "generator-at-pin");
        assert_eq!(p.requirement, Requirement::Required);
        assert_eq!(
            readiness::derive(&planned.preconditions),
            Readiness::Blocked
        );
        fixture.host_generator = Some("0.31.0");
        let old = fixture.evaluate();
        let Evaluation::Unsatisfied { reason } = &precondition(&old, "generator-at-pin").evaluation
        else {
            panic!("a generator below its pin is unsatisfied");
        };
        assert!(reason.contains("install cargo-dist 0.32.0"), "{reason}");
        fixture.host_generator = Some("0.32.0");
        let current = fixture.evaluate();
        assert!(
            precondition(&current, "generator-at-pin")
                .evaluation
                .holds()
        );
        assert_eq!(
            current
                .facts
                .generator
                .as_ref()
                .map(|g| g.artifact.as_str()),
            Some("dist-workspace.toml")
        );
    }

    #[test]
    fn an_unwired_manager_is_a_decision() {
        let mut fixture = Fixture::new(Compatibility::default());
        fixture.unwired_managers = vec!["mise".to_owned()];
        let asked = fixture.evaluate();
        let p = precondition(&asked, "pin-manager-answered");
        assert_eq!(p.requirement, Requirement::DecisionRequired);
        assert_eq!(p.decision.as_deref(), Some("pin-manager"));
        assert_eq!(
            readiness::derive(&asked.preconditions),
            Readiness::NeedsDecision
        );
        assert_eq!(asked.decisions[0].id, "pin-manager");
        fixture.selected = BTreeMap::from([("pin-manager".to_owned(), "host".to_owned())]);
        let answered = fixture.evaluate();
        assert!(
            precondition(&answered, "pin-manager-answered")
                .evaluation
                .holds()
        );
        assert_eq!(readiness::derive(&answered.preconditions), Readiness::Ready);
        fixture.selected.clear();
        fixture.pin_wired = true;
        assert!(
            fixture.evaluate().decisions.is_empty(),
            "a wired pin asks nothing"
        );
        fixture.pin_wired = false;
        fixture.recorded_version = Some("0.3.21");
        assert!(
            fixture.evaluate().decisions.is_empty(),
            "no move wanted, no question"
        );
        fixture.recorded_version = None;
        assert!(
            fixture.evaluate().decisions.is_empty(),
            "a first landing wires later"
        );
    }

    #[test]
    fn a_forge_below_floor_blocks_only_when_a_forge_fact_affects_an_operation() {
        let declared = parse_compatibility("[forge.gitlab]\nminimum = \"18.2\"\n").expect("parses");
        let mut fixture = Fixture::new(declared);
        let unread = fixture.evaluate();
        let p = precondition(&unread, "forge-meets-floor");
        assert_eq!(p.requirement, Requirement::Advisory);
        assert!(matches!(p.evaluation, Evaluation::NotObserved { .. }));
        fixture.forge_version = Some("18.1.0");
        let below_idle = fixture.evaluate();
        let p = precondition(&below_idle, "forge-meets-floor");
        assert_eq!(p.requirement, Requirement::Advisory);
        assert!(matches!(p.evaluation, Evaluation::Unsatisfied { .. }));
        assert_eq!(
            readiness::derive(&below_idle.preconditions),
            Readiness::Ready
        );
        fixture.writes = vec![".gitlab-ci.yml".to_owned()];
        let below_planned = fixture.evaluate();
        assert_eq!(
            precondition(&below_planned, "forge-meets-floor").requirement,
            Requirement::Required
        );
        assert_eq!(
            readiness::derive(&below_planned.preconditions),
            Readiness::Blocked
        );
        fixture.forge_version = Some("18.2.0-ee");
        let ok = fixture.evaluate();
        assert!(precondition(&ok, "forge-meets-floor").evaluation.holds());
        fixture.forge = Some("github");
        assert!(
            fixture.evaluate().facts.forge_floor.is_none(),
            "no floor is declared for github"
        );
    }

    #[test]
    fn a_skipped_intermediate_version_is_blocked_naming_it() {
        let declared = parse_compatibility(
            "[[intermediate]]\nversion = \"0.3.20\"\nreason = \"the record changed shape\"\n[[intermediate]]\nversion = \"0.1.0\"\nreason = \"already behind the record\"\n",
        )
        .expect("parses");
        let mut fixture = Fixture::new(declared);
        let skipped = fixture.evaluate();
        let p = precondition(&skipped, "intermediate-release:0.3.20");
        assert_eq!(p.requirement, Requirement::Required);
        let Evaluation::Unsatisfied { reason } = &p.evaluation else {
            panic!("a skipped release is unsatisfied");
        };
        assert!(
            reason.contains("0.3.20") && reason.contains("--to 0.3.20"),
            "{reason}"
        );
        assert_eq!(
            skipped.facts.intermediate.len(),
            1,
            "a release behind the record is not in the way"
        );
        assert_eq!(
            readiness::derive(&skipped.preconditions),
            Readiness::Blocked
        );
        fixture.recorded_version = Some("0.3.20");
        assert!(fixture.evaluate().facts.intermediate.is_empty());
        fixture.recorded_version = None;
        assert!(
            fixture.evaluate().facts.intermediate.is_empty(),
            "no record, nothing to pass through"
        );
    }
}
