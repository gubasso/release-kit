//! `rk reconcile`: the plan computed, stored, shown, and applied.
//!
//! `plan` observes the target, reads the embedded bundle, computes the
//! plan, stores it under the state root, and prints it; it writes
//! nothing into the target. `--to` with a version this binary does not
//! carry reads the crates venue and says so. `--observe forge` opts into
//! the one forge read. `show` renders a stored plan. `apply` executes
//! one: it computes the same plan again over the same request, refuses
//! on any difference in the fingerprint, writes through one staged
//! transaction, runs the postconditions, and journals the run.

use std::collections::BTreeMap;

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::reconcile::{
    ApplyArgs, ListArgs, Observe, PlanArgs, ReconcileAction, ReconcileArgs, ShowArgs,
};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest;
use crate::output::Output;
use crate::plan::apply::{self, Applied};
use crate::plan::gather::{self, Flags, RecordRead, Request};
use crate::plan::planner::{self, Baseline, Candidate};
use crate::plan::store;
use crate::plan::{
    Classification, Intent, Operation, Plan, PlanRequest, Planned, Readiness, ResolvedRelease,
    Verification,
};
use crate::release::declared;
use crate::release::{CrateReleaseSource, EmbeddedReleaseSource, ReleaseSource};
use crate::setup::journal::Journal;

/// Dispatch one reconcile action.
///
/// # Errors
///
/// The planner's, the store's, and the apply's own failures.
pub fn run(args: &ReconcileArgs) -> Result<(), RkError> {
    match &args.action {
        ReconcileAction::Plan(plan_args) => plan(plan_args),
        ReconcileAction::Show(show_args) => show(show_args),
        ReconcileAction::Apply(apply_args) => apply_stored(apply_args),
        ReconcileAction::List(list_args) => list(list_args),
    }
}

/// Compute, store, and print one plan.
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
    let request = PlanRequest {
        target: args.target.clone(),
        intent: Intent::Reconcile,
        selector: args.to.clone(),
        fetch: args.fetch,
        observe_forge: args.observe.contains(&Observe::Forge),
        flags: Flags {
            tech: args.tech.clone(),
            forge: args.forge.clone(),
            repo: args.repo.clone(),
            workflow: args.workflow.clone(),
            style: args.style.clone(),
            nix,
        },
        decisions,
    }
    .canonicalized()?;
    let planned = compute(&request, &manifest::now())?;
    store::persist(&planned, &request)?;
    render(out, &planned.plan, true);
    out.emit(&planned.plan)
}

/// Render a stored plan.
fn show(args: &ShowArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let stored = store::load(&args.plan_id)?;
    render(out, &stored.plan, false);
    out.emit(&stored.plan)
}

/// Execute a stored plan.
fn apply_stored(args: &ApplyArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let stored = store::load(&args.plan_id)?;
    // The target is taken before it is observed, so the world the fresh
    // plan describes is the world this apply goes on to write: another
    // run committing between the observation and the first rename is
    // what the lock exists to stop.
    let _lock = crate::plan::lock::acquire(&stored.request.target)?;
    // The same request at the same instant: a fresh landing's record
    // carries the plan's instant, so recomputing at another one would
    // read as the record moving when nothing did.
    // The candidate is the release the plan froze, served from the cache:
    // the selector was resolved once at plan time and is never resolved
    // again.
    let fresh = compute_frozen(
        &stored.request,
        &stored.plan.identity.created_at,
        Some(&stored.plan.desired_state.release),
    )?;
    let journal = open_journal("reconcile apply", &stored.plan).map_err(|error| {
        RkError::refusal(
            Diagnostic::new(
                Reason::JournalUnavailable,
                format!("the run journal could not be created, and nothing was written: {error}"),
            )
            .expected("a writable state root for the journal")
            .target_state("unchanged"),
        )
    })?;
    let applied = apply::run_locked(
        &stored.request.target,
        &stored.plan,
        &stored.blobs,
        &fresh.plan,
        Some(journal),
    )?;
    render_applied(out, &applied);
    out.emit(&applied)?;
    applied.failure().map_or(Ok(()), Err)
}

/// The listing of stored plans.
#[derive(Debug, Serialize)]
struct ListReport {
    /// The shape version of this document.
    schema: &'static str,
    /// Every stored plan, oldest first.
    plans: Vec<ListRow>,
}

/// One stored plan's row.
#[derive(Debug, Serialize)]
struct ListRow {
    /// The plan id.
    plan_id: String,
    /// The instant it was computed.
    created_at: String,
}

fn list(args: &ListArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let rows: Vec<ListRow> = store::list()
        .into_iter()
        .map(|(created_at, plan_id)| ListRow {
            plan_id,
            created_at,
        })
        .collect();
    for row in &rows {
        out.result_line(format!("{}  {}", row.plan_id, row.created_at));
    }
    if rows.is_empty() {
        out.result_line("no plans are stored");
    }
    out.emit(&ListReport {
        schema: "rk.reconcile-list/1",
        plans: rows,
    })
}

/// The trace a front's report carries of the plan it applied.
#[derive(Debug, Serialize)]
pub struct Trace {
    /// The plan that was applied.
    pub plan_id: String,
    /// The fingerprint the apply revalidated against.
    pub input_fingerprint: crate::digest::Digest,
    /// Whether the store took the plan.
    pub stored: bool,
    /// The journal entry, where the journal took one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

impl FrontApplied {
    /// The trace for a front's report.
    #[must_use]
    pub fn trace(&self) -> Trace {
        Trace {
            plan_id: self.applied.plan_id.clone(),
            input_fingerprint: self.applied.input_fingerprint.clone(),
            stored: self.stored,
            run_id: self.applied.run_id.clone(),
        }
    }

    /// The human line a front prints for the plan it applied.
    #[must_use]
    pub fn line(&self) -> String {
        self.applied.run_id.as_ref().map_or_else(
            || format!("applied plan {}", self.applied.plan_id),
            |run_id| format!("applied plan {} (run {run_id})", self.applied.plan_id),
        )
    }

    /// The bytes an operation wrote at `path`, where one did.
    #[must_use]
    pub fn written<'a>(planned: &'a Planned, path: &str) -> Option<&'a [u8]> {
        planned
            .plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                Operation::WriteFile { path: p, after, .. }
                | Operation::SpliceBlock { path: p, after, .. }
                    if p == path =>
                {
                    planned.blobs.get(after).map(Vec::as_slice)
                }
                _ => None,
            })
    }
}

/// What a front's apply came back with: the engine's report and whether
/// the store took the plan.
#[derive(Debug)]
pub struct FrontApplied {
    /// The engine's report.
    pub applied: Applied,
    /// Whether the plan was stored; a front is one process with no review
    /// window, so a store that cannot be written costs the record alone.
    pub stored: bool,
}

/// One computed plan applied in the same process, which is what the
/// fronts do on `--apply`: the store and the journal are best effort,
/// and the execution path is the one `rk reconcile apply` takes.
///
/// # Errors
///
/// The apply's own refusals and failures.
pub fn apply_in_process(
    planned: &Planned,
    request: &PlanRequest,
    command: &str,
) -> Result<FrontApplied, RkError> {
    let stored = store::persist(planned, request).is_ok();
    let journal = open_journal(command, &planned.plan).ok();
    let applied = apply::run(
        &request.target,
        &planned.plan,
        &planned.blobs,
        &planned.plan,
        journal,
    )?;
    Ok(FrontApplied { applied, stored })
}

fn open_journal(command: &str, plan: &Plan) -> std::io::Result<Journal> {
    let (forge, repo) = plan
        .desired_state
        .configuration
        .as_ref()
        .map_or(("", ""), |c| (c.forge.as_str(), c.repo.as_str()));
    Journal::create(command, &plan.observed_state.repository.target, forge, repo)
}

/// The whole computation, shared with the fronts.
///
/// # Errors
///
/// A selector the crates venue cannot resolve, a bundle the engine
/// cannot read, and the gathering's own failures.
pub fn compute(request: &PlanRequest, clock: &str) -> Result<Planned, RkError> {
    compute_frozen(request, clock, None)
}

/// The same computation over a release a stored plan froze: the exact
/// version is served from the release cache and the selector is never
/// resolved again, so an apply is offline once its plan exists.
///
/// # Errors
///
/// [`compute`]'s failures, and a `bundle-unverified` refusal when the
/// cache no longer holds the frozen release.
#[allow(
    clippy::too_many_lines,
    reason = "one computation is one linear sequence from the selector to the planner's inputs, and cutting it would separate a bundle from the observation it is read against"
)]
pub fn compute_frozen(
    request: &PlanRequest,
    clock: &str,
    frozen: Option<&ResolvedRelease>,
) -> Result<Planned, RkError> {
    let target: &Utf8Path = &request.target;
    let selector = request.selector.as_str();
    let embedded = EmbeddedReleaseSource;
    let crate_source = match frozen {
        _ if selector == "embedded" => None,
        Some(release) => {
            let source = CrateReleaseSource::new(&release.version)?;
            if !source.is_cached() {
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::BundleUnverified,
                        format!(
                            "the plan froze release-kit {} ({}), and the release cache no longer holds that bundle; nothing was written",
                            release.version, release.payload_sha256
                        ),
                    )
                    .expected("the frozen release's verified bundle in the release cache")
                    .action("rk reconcile plan --to <version> resolves and caches it again")
                    .target_state("unchanged"),
                ));
            }
            Some(source)
        }
        None => Some(CrateReleaseSource::new(selector)?),
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
    let declared = declared::compatibility(candidate_source, &candidate_manifest)?;
    let guidance_files = declared::guidance(candidate_source, &candidate_manifest)?;
    let carries_guidance = declared::carries_guidance(&candidate_manifest);
    let extra_paths: Vec<String> = guidance_files
        .iter()
        .flat_map(|file| file.destinations.iter().cloned())
        .collect();
    let gather_request = Request {
        target,
        flags: &request.flags,
        decisions: &request.decisions,
        observe_forge: request.observe_forge,
        clock,
        source: candidate_source,
        extra_paths: &extra_paths,
    };
    let mut observation = gather::observe(&gather_request)?;
    let resolution = gather::resolve(&gather_request, &observation)?;
    gather::observe_host_tools(
        &mut observation,
        resolution.params.as_ref().map(crate::landing::Params::tech),
        resolution
            .params
            .as_ref()
            .map(crate::landing::Params::forge),
        clock,
    );

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
            if request.fetch || source.is_cached() {
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
        Some((version, _)) => cached_baseline(version, baseline_crate.as_ref()),
    };
    planner::plan(planner::Inputs {
        intent: request.intent,
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
        selected: &request.decisions,
        declared: &declared,
        guidance_files: &guidance_files,
        carries_guidance,
    })
}

/// The baseline for a recorded release the cache may hold.
///
/// A baseline that will not verify is an evidence gap, never a refusal.
/// The candidate is what an apply writes and it refuses unverified; the
/// recorded release only says what the target started from, so a plan
/// that cannot read it says so and lets the readiness policy decide. A
/// cache written before the seal existed is exactly this case, and it
/// must still be able to plan.
fn cached_baseline<'a>(version: &str, source: Option<&'a CrateReleaseSource>) -> Baseline<'a> {
    let Some(source) = source else {
        return Baseline::NotObserved {
            reason: format!(
                "the recorded release {version} is not in the release cache; --fetch reads it through the crates venue"
            ),
        };
    };
    match source.resolve() {
        Ok(_) => Baseline::Cached {
            version: version.to_owned(),
            source,
        },
        Err(error) => Baseline::NotObserved {
            reason: format!(
                "the recorded release {version} is in the release cache and did not verify: {}",
                error.diagnostic().message
            ),
        },
    }
}

/// `<id>=<answer>` pairs into a map, refusing a malformed one.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for an item without `=` or with an empty
/// side.
pub fn parse_decisions(raw: &[String]) -> Result<BTreeMap<String, String>, RkError> {
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
        // A decision's choices are the whole of what answers it, so an
        // unrecognized id or answer is refused where the operator typed
        // it rather than read as a decision taken.
        let Some(choices) = crate::plan::decision_choices(id) else {
            let ids: Vec<&str> = crate::plan::DECISION_CHOICES
                .iter()
                .map(|(id, _)| *id)
                .collect();
            return Err(RkError::Usage(format!(
                "--decide names no decision '{id}'; the decisions are: {}",
                ids.join(", ")
            )));
        };
        if !choices.contains(&answer) {
            return Err(RkError::Usage(format!(
                "--decide {id}={answer} is not an answer it takes; the answers are: {}",
                choices.join(", ")
            )));
        }
        decisions.insert(id.to_owned(), answer.to_owned());
    }
    Ok(decisions)
}

/// The human lines: the routing word, the readiness, the operations by
/// kind, every precondition that does not hold, every decision that
/// waits, and the fingerprint.
fn render(out: Output, plan: &Plan, fresh: bool) {
    out.result_line(format!(
        "plan {} for {} toward release-kit {} ({}){}",
        plan.identity.plan_id,
        plan.observed_state.repository.target,
        plan.desired_state.release.version,
        plan.desired_state.release.venue,
        if fresh { ", stored" } else { "" }
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
    match &plan.release.guidance.coverage {
        crate::plan::Coverage::NotNeeded => {}
        coverage => {
            let word = match coverage {
                crate::plan::Coverage::Covered => "covered".to_owned(),
                crate::plan::Coverage::Partial { since } => format!("partial above {since}"),
                crate::plan::Coverage::Unavailable => "unavailable".to_owned(),
                crate::plan::Coverage::NotNeeded => String::new(),
            };
            out.result_line(format!(
                "guidance: {word}, {} step(s) for this target, {} excluded",
                plan.release.guidance.steps.len(),
                plan.release.guidance.excluded
            ));
            for step in &plan.release.guidance.steps {
                out.result_line(format!(
                    "  {} ({}): {} [{}]",
                    step.version,
                    step.action,
                    step.title,
                    step.destinations.join(", ")
                ));
            }
        }
    }
    out.result_line(format!("fingerprint: {}", plan.input_fingerprint));
    out.next(&next_lines(plan));
}

/// The human lines of an apply.
fn render_applied(out: Output, applied: &Applied) {
    out.result_line(format!(
        "applied plan {} to {}",
        applied.plan_id, applied.target
    ));
    for result in &applied.operations {
        out.result_line(format!(
            "  {} {}",
            result.op,
            result.path.as_deref().unwrap_or_default()
        ));
    }
    for result in &applied.postconditions {
        out.result_line(result.detail.as_ref().map_or_else(
            || format!("postcondition {}: {}", result.check, result.status),
            |detail| {
                format!(
                    "postcondition {}: {} ({detail})",
                    result.check, result.status
                )
            },
        ));
    }
    if let Some(run_id) = &applied.run_id {
        out.result_line(format!("journal: run {run_id}"));
    }
    out.next(&applied.next);
}

/// One operation as a human line.
pub(crate) fn describe(operation: &Operation) -> String {
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
            Classification::Upgrade if plan.operations.is_empty() => {
                vec!["nothing to take: the target is at this release".to_owned()]
            }
            Classification::Setup | Classification::Migration | Classification::Upgrade => {
                vec![format!(
                    "rk reconcile apply {} executes exactly these operations",
                    plan.identity.plan_id
                )]
            }
            Classification::Drift | Classification::Invalid => {
                vec!["resolve the findings above, then plan again".to_owned()]
            }
        },
    }
}
