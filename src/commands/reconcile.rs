//! `rk reconcile`: the plan, computed and printed.
//!
//! `plan` is read-only and offline by default: it observes the target,
//! reads the embedded bundle, computes the plan, and prints it. `--to`
//! with a version this binary does not carry reads the crates venue and
//! says so. `--observe forge` opts into the one forge read. Nothing is
//! persisted and nothing is written into the target.

use std::collections::BTreeMap;

use camino::Utf8Path;

use crate::cli::reconcile::{Observe, PlanArgs, ReconcileAction, ReconcileArgs};
use crate::error::RkError;
use crate::landing::manifest;
use crate::output::Output;
use crate::plan::gather::{self, Flags, RecordRead, Request};
use crate::plan::planner::{self, Baseline, Candidate};
use crate::plan::{Classification, Operation, Plan, Planned, Readiness, Verification};
use crate::release::{CrateReleaseSource, EmbeddedReleaseSource, ReleaseSource};

/// Dispatch one reconcile action.
///
/// # Errors
///
/// The planner's and the gathering's own failures.
pub fn run(args: &ReconcileArgs) -> Result<(), RkError> {
    match &args.action {
        ReconcileAction::Plan(plan_args) => plan(plan_args),
    }
}

/// Compute and print one plan.
fn plan(args: &PlanArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let decisions = parse_decisions(&args.decide)?;
    let nix = match args.nix.as_deref() {
        None => None,
        Some("on") => Some(true),
        Some("off") => Some(false),
        Some(other) => {
            return Err(RkError::Usage(format!(
                "unknown --nix value '{other}'; the values are: on, off"
            )));
        }
    };
    let flags = Flags {
        tech: args.tech.clone(),
        forge: args.forge.clone(),
        repo: args.repo.clone(),
        workflow: args.workflow.clone(),
        style: args.style.clone(),
        nix,
    };
    let clock = manifest::now();
    let planned = compute(
        &args.target,
        &args.to,
        args.fetch,
        args.observe.contains(&Observe::Forge),
        &flags,
        &decisions,
        &clock,
    )?;
    render(out, &planned.plan);
    out.emit(&planned.plan)
}

/// The whole computation, shared with the fronts that print a plan's
/// classification and readiness beside their own report.
///
/// # Errors
///
/// A selector the crates venue cannot resolve, a bundle the engine
/// cannot read, and the gathering's own failures.
pub fn compute(
    target: &Utf8Path,
    selector: &str,
    fetch: bool,
    observe_forge: bool,
    flags: &Flags,
    decisions: &BTreeMap<String, String>,
    clock: &str,
) -> Result<Planned, RkError> {
    let embedded = EmbeddedReleaseSource;
    let crate_source = if selector == "embedded" {
        None
    } else {
        Some(CrateReleaseSource::new(selector)?)
    };
    let (candidate_source, venue, verification): (&dyn ReleaseSource, &str, Verification) =
        match &crate_source {
            None => (&embedded, "embedded", Verification::Embedded),
            Some(source) => {
                let resolved = source.resolve()?;
                (
                    source,
                    "crates",
                    Verification::RegistryChecksum {
                        cksum: resolved.cksum.clone(),
                    },
                )
            }
        };
    let candidate_manifest = candidate_source.manifest()?;
    let request = Request {
        target,
        flags,
        decisions,
        observe_forge,
        clock,
        source: candidate_source,
    };
    let observation = gather::observe(&request)?;
    let resolution = gather::resolve(&request, &observation)?;

    // The recorded release's bundle: the embedded one where the record
    // names its payload, the cache where it holds the recorded version,
    // the venue where `--fetch` allows it, and not observed otherwise.
    let recorded = match &observation.record {
        RecordRead::Present { manifest, .. } => {
            Some((manifest.rk_version.clone(), manifest.payload_sha256.clone()))
        }
        RecordRead::Absent | RecordRead::Invalid { .. } => None,
    };
    let baseline_crate = match &recorded {
        Some((version, digest))
            if *digest != EmbeddedReleaseSource::manifest_ref().payload_sha256
                && *digest != candidate_manifest.payload_sha256 =>
        {
            let source = CrateReleaseSource::new(version)?;
            if fetch || source.is_cached() {
                Some(source)
            } else {
                None
            }
        }
        _ => None,
    };
    let baseline = match &recorded {
        None => Baseline::NotNeeded,
        Some((_, digest)) if *digest == EmbeddedReleaseSource::manifest_ref().payload_sha256 => {
            Baseline::Embedded(&embedded)
        }
        Some((version, digest)) if *digest == candidate_manifest.payload_sha256 => {
            Baseline::Cached {
                version: version.clone(),
                source: candidate_source,
            }
        }
        Some((version, _)) => match &baseline_crate {
            Some(source) => {
                source.resolve()?;
                Baseline::Cached {
                    version: version.clone(),
                    source,
                }
            }
            None => Baseline::NotObserved {
                reason: format!(
                    "the recorded release {version} is not in the release cache; --fetch reads it through the crates venue"
                ),
            },
        },
    };
    planner::plan(planner::Inputs {
        clock,
        engine_version: env!("CARGO_PKG_VERSION"),
        selector,
        candidate: Candidate {
            source: candidate_source,
            manifest: candidate_manifest,
            venue,
            verification,
        },
        baseline,
        observation,
        resolution,
        selected: decisions,
    })
}

/// `<id>=<answer>` pairs into a map, refusing a malformed one.
fn parse_decisions(raw: &[String]) -> Result<BTreeMap<String, String>, RkError> {
    let mut decisions = BTreeMap::new();
    for item in raw {
        let Some((id, answer)) = item.split_once('=') else {
            return Err(RkError::Usage(format!(
                "--decide takes <id>=<answer>; '{item}' has no '='"
            )));
        };
        if id.is_empty() || answer.is_empty() {
            return Err(RkError::Usage(format!(
                "--decide takes <id>=<answer>; '{item}' leaves one side empty"
            )));
        }
        decisions.insert(id.to_owned(), answer.to_owned());
    }
    Ok(decisions)
}

/// The human lines: the routing word, the readiness, the operations by
/// kind, every precondition that does not hold, every decision that
/// waits, and the fingerprint.
fn render(out: Output, plan: &Plan) {
    out.result_line(format!(
        "plan {} for {} toward release-kit {} ({})",
        plan.identity.plan_id,
        plan.observed_state.repository.target,
        plan.desired_state.release.version,
        plan.desired_state.release.venue
    ));
    out.result_line(format!("classification: {}", plan.classification.as_str()));
    for finding in &plan.findings {
        out.result_line(format!("  {}: {}", finding.code, finding.detail));
    }
    out.result_line(format!("readiness: {}", plan.readiness.as_str()));
    let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
    for operation in &plan.operations {
        *by_kind.entry(operation.kind()).or_default() += 1;
    }
    if by_kind.is_empty() {
        out.result_line("operations: none");
    } else {
        out.result_line(format!(
            "operations: {}",
            by_kind
                .iter()
                .map(|(kind, count)| format!("{count} {kind}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        for operation in &plan.operations {
            out.result_line(format!("  {}", describe(operation)));
        }
    }
    for precondition in plan.preconditions.iter().filter(|p| !p.evaluation.holds()) {
        let reason = match &precondition.evaluation {
            crate::plan::Evaluation::Satisfied => String::new(),
            crate::plan::Evaluation::NotObserved { reason }
            | crate::plan::Evaluation::Unsatisfied { reason } => reason.clone(),
        };
        out.result_line(format!(
            "precondition {} ({}): {}: {reason}",
            precondition.id,
            precondition.requirement.as_str(),
            precondition.evaluation.word()
        ));
    }
    for decision in plan.decisions.iter().filter(|d| d.selected.is_none()) {
        out.result_line(format!("decision {}: {}", decision.id, decision.question));
        for choice in &decision.choices {
            out.result_line(format!("  {}: {}", choice.answer, choice.consequence));
        }
    }
    out.result_line(format!("fingerprint: {}", plan.input_fingerprint));
    out.next(&next_lines(plan));
}

/// One operation as a human line.
fn describe(operation: &Operation) -> String {
    match operation {
        Operation::WriteFile { path, kind, .. } => format!("write-file {path} ({})", kind.as_str()),
        Operation::SpliceBlock { path, .. } => format!("splice-block {path}"),
        Operation::RemoveOwnedFile { path, .. } => format!("remove-owned-file {path}"),
        Operation::WriteRecord { .. } => format!("write-record {}", manifest::MANIFEST_PATH),
        Operation::UpdatePin {
            manager,
            before,
            after,
        } => format!("update-pin {manager} {before} -> {after}"),
    }
}

/// What plausibly follows, from the readiness.
fn next_lines(plan: &Plan) -> Vec<String> {
    let target = &plan.observed_state.repository.target;
    match plan.readiness {
        Readiness::Blocked => {
            vec!["resolve each unsatisfied required precondition above, then plan again".to_owned()]
        }
        Readiness::NeedsDecision => plan
            .decisions
            .iter()
            .filter(|d| d.selected.is_none())
            .map(|d| {
                format!(
                    "rk reconcile plan --target {target} --decide {}=<{}> selects an answer",
                    d.id,
                    d.choices
                        .iter()
                        .map(|c| c.answer.as_str())
                        .collect::<Vec<_>>()
                        .join("|")
                )
            })
            .collect(),
        Readiness::Ready => match plan.classification {
            Classification::Setup | Classification::Migration => vec![format!(
                "rk init --target {target} --apply lands what this plan names"
            )],
            Classification::Upgrade if plan.operations.is_empty() => {
                vec!["nothing to take: the target is at this release".to_owned()]
            }
            Classification::Upgrade => vec![format!(
                "rk upgrade --target {target} --apply takes what this plan names"
            )],
            Classification::Drift | Classification::Invalid => {
                vec!["resolve the findings above, then plan again".to_owned()]
            }
        },
    }
}
