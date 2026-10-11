//! `rk status`: a target describes itself from its own disk.
//!
//! Read-only and offline: the receipt supplies what landed, this binary's
//! projection supplies what an upgrade would offer, the embedded registry
//! supplies the pin comparison, and no network is ever touched: a fetch
//! is a way for a status command to hang, fail on a network it should not
//! need, or leak a repository's existence. Status compares the current
//! target with this binary's projection and the receipt and nothing else;
//! it reads no stage and resolves no other release. Plain `rk status`
//! reports and exits 0 for every reportable state, drift and no-landing
//! included; `--check` computes the identical report and changes only
//! the final judgment, the one sanctioned bare exit 1.
//!
//! SATISFIES landing:status-judges-only-under-check

use serde::Serialize;

use crate::cli::status::StatusArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::invariants::{self, InvariantFailure};
use crate::landing::manifest::{self, Alignment, Manifest};
use crate::landing::{self, Kind};
use crate::output::Output;
use crate::profile::{CapabilityRequests, GitWorkflow, ProfileSnapshot, ReleaseMode};
use crate::projection::{Candidate, Projection, ProjectionInput, TargetEvidence};
use crate::stage::CapabilityNote;
use crate::{assess, embedded, registry};

/// Drift counts by owned kind; `state` files are never compared.
#[derive(Debug, Serialize)]
struct Drift {
    /// Edits to files release-kit owns — the violation class.
    rendered: usize,
    /// Edits to files the target owns — expected and informational.
    seeded: usize,
}

/// One recorded pin that is behind this binary's registry.
#[derive(Debug, Serialize)]
struct StalePin {
    /// The tool's registry name.
    tool: String,
    /// The version the landing recorded.
    landed: String,
    /// The version this binary's registry pins.
    available: String,
}

/// One reportable condition that is not a violation: the target is not
/// broken and the decision behind it is the operator's.
///
/// The `code` is stable, so a machine reader branches on it rather than on
/// the prose.
#[derive(Debug, Serialize)]
struct Warning {
    /// The stable reason code.
    code: &'static str,
    /// What the condition is, in the target's own terms.
    reason: String,
}

/// The code a lapsed code scanning licence reports under.
const CODE_SCANNING_LICENCE: &str = "code-scanning-licence";

/// Where a target stands for the setup skill's gate: one word computed by
/// the CLI's own rule, so no reader branches on five fields by hand.
///
/// SATISFIES landing:a-landing-classifies-its-target-first
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// No record, no release mechanism, and no release history.
    Setup,
    /// No record and another release mechanism in place.
    Migration,
    /// No record and release activity no mechanism explains.
    NeedsDecision,
    /// No record, and release-kit's own landing is present.
    Adopt,
    /// A record older than this binary.
    Upgrade,
    /// A record at this binary's version that `--check` fails.
    Drift,
    /// A record at this binary's version that `--check` passes.
    Current,
}

impl Classification {
    /// The kebab-case word, as the JSON serializes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Migration => "migration",
            Self::NeedsDecision => "needs-decision",
            Self::Adopt => "adopt",
            Self::Upgrade => "upgrade",
            Self::Drift => "drift",
            Self::Current => "current",
        }
    }
}

/// The classification of a target with no record, from the assessment's
/// rule and the adoption detector.
const fn unrecorded(class: assess::Unrecorded) -> Classification {
    match class {
        assess::Unrecorded::Adopt => Classification::Adopt,
        assess::Unrecorded::Assessed(assess::Classification::Greenfield) => Classification::Setup,
        assess::Unrecorded::Assessed(assess::Classification::Brownfield) => {
            Classification::Migration
        }
        assess::Unrecorded::Assessed(assess::Classification::NeedsDecision) => {
            Classification::NeedsDecision
        }
    }
}

/// The classification of a recorded target. A record newer than this
/// binary has none: that is an observation the setup skill routes to
/// provisioning, not a state this binary can judge.
const fn recorded(alignment: Alignment, clean: bool) -> Option<Classification> {
    match alignment {
        Alignment::TargetNewer => None,
        Alignment::BinaryNewer => Some(Classification::Upgrade),
        Alignment::Aligned if clean => Some(Classification::Current),
        Alignment::Aligned => Some(Classification::Drift),
    }
}

/// One unfilled judgment sentinel, located.
#[derive(Debug, Clone, Serialize)]
struct SentinelAt {
    /// The landed file that carries it.
    file: String,
    /// Its line, counted from one.
    line: usize,
    /// The line's text, trimmed.
    text: String,
}

/// One path a 0.4.x release left under the state root.
#[derive(Debug, Serialize)]
struct LegacyPath {
    /// The absolute path.
    path: String,
    /// `plan` for a stored plan, `release-cache` for the release cache.
    kind: &'static str,
    /// Whether it holds an operation the old binary could still apply.
    active: bool,
}

/// The state a 0.4.x release stored under the state root, each path with
/// whether an operation is active. No later release writes `plans/` or
/// `release/`, so what is there is the old binary's.
///
/// A stored plan reads as active while its own document says an apply
/// may run it, and while it cannot be read at all, because only the
/// operator can retire an operation the old binary owns. The release
/// cache holds no operation.
fn legacy_state() -> Vec<LegacyPath> {
    let Some(root) = crate::applog::state_root() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join(LEGACY_PLANS)) {
        let mut plans: Vec<std::path::PathBuf> = entries
            .flatten()
            .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        plans.sort();
        for plan in plans {
            let active = std::fs::read(plan.join("plan.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_none_or(|document| document["readiness"].as_str() == Some("ready"));
            found.push(LegacyPath {
                path: plan.display().to_string(),
                kind: "plan",
                active,
            });
        }
    }
    let cache = root.join(LEGACY_RELEASE_CACHE);
    if cache.is_dir() {
        found.push(LegacyPath {
            path: cache.display().to_string(),
            kind: "release-cache",
            active: false,
        });
    }
    found
}

/// Where a 0.4.x release stored its plans, under the state root.
const LEGACY_PLANS: &str = "plans";

/// Where a 0.4.x release cached fetched releases, under the state root.
const LEGACY_RELEASE_CACHE: &str = "release";

/// The status report's shape version.
const SCHEMA: &str = "rk.status/13";

/// Configuration is informational, independent of every drift comparison.
#[derive(Debug, serde::Serialize)]
struct ConfigState {
    state: &'static str,
    pending: Vec<String>,
}

fn config_state(config: Option<&crate::config::Config>, record: Option<&Manifest>) -> ConfigState {
    let pending = config
        .zip(record)
        .map_or_else(Vec::new, |(config, record)| {
            crate::config::pending(config, record)
        });
    ConfigState {
        state: if config.is_none() {
            "absent"
        } else if pending.is_empty() {
            "aligned"
        } else {
            "pending"
        },
        pending,
    }
}

/// The machine form of a status report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// Whether a landing record exists; most other fields need one.
    landed: bool,
    /// Where the target stands for the setup skill's gate; null where the
    /// record is newer than this binary.
    classification: Option<Classification>,
    /// Whether the target sits in a non-bare Git work tree.
    git_work_tree: bool,
    config: ConfigState,
    /// What the record says the project is.
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<ProfileSnapshot>,
    /// The recorded Git workflow.
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitWorkflow>,
    /// The recorded capability requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    capabilities: Option<CapabilityRequests>,
    /// Every capability the catalog answers for the recorded values, in
    /// catalog order; absent where this binary cannot project the record.
    #[serde(skip_serializing_if = "Option::is_none")]
    selection: Option<Vec<CapabilityNote>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rk_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    binary_version: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alignment: Option<Alignment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drift: Option<Drift>,
    /// Recorded destinations absent from the disk.
    #[serde(skip_serializing_if = "Option::is_none")]
    missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stale_pins: Option<Vec<StalePin>>,
    /// Unresolved judgment sentinels across the landed files.
    #[serde(skip_serializing_if = "Option::is_none")]
    sentinels: Option<usize>,
    /// Each unresolved sentinel, by file and line.
    #[serde(skip_serializing_if = "Option::is_none")]
    sentinel_locations: Option<Vec<SentinelAt>>,
    /// Record-set disagreements between the recorded parameters'
    /// projection and the recorded destinations — its own count, because
    /// no file was edited and the kind counts must stay honest.
    #[serde(skip_serializing_if = "Option::is_none")]
    record_drift: Option<usize>,
    /// Invariants a landed file's effective configuration violates —
    /// judged, never rewritten, because the file stays the target's.
    #[serde(skip_serializing_if = "Option::is_none")]
    invariant_failures: Option<Vec<InvariantFailure>>,
    /// Conditions that are reportable and not violations: `--check` exits 0
    /// on them, because the target is not broken and the decision is the
    /// operator's.
    #[serde(skip_serializing_if = "Option::is_none")]
    warnings: Option<Vec<Warning>>,
    /// How many destinations an upgrade would change: the count this
    /// binary's sources project under the recorded parameters against
    /// what the record names. Zero means nothing to take, whatever the two
    /// versions say. Absent on a landed target means this binary carries
    /// no files for the recorded pair and cannot answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pending: Option<usize>,
    /// Present only under `--check`: what the judgment failed on.
    #[serde(skip_serializing_if = "Option::is_none")]
    violations: Option<Vec<String>>,
    /// The state a 0.4.x release left under the state root.
    legacy_state: Vec<LegacyPath>,
    /// The open migration, where the configuration names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    migration: Option<OpenMigration>,
}

/// An open migration, as the status reports it.
#[derive(Debug, Serialize)]
struct OpenMigration {
    /// The identifier the configuration names.
    id: String,
    /// The workspace on this host, absent where this host holds none.
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace: Option<String>,
    /// How many sources are neither verified nor dropped, absent where
    /// this host holds no registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    unfinished: Option<usize>,
}

/// The open migration the configuration names, read from this host's
/// workspace where it holds one.
///
/// SATISFIES migration:the-configuration-names-the-open-migration
fn open_migration(
    target: &camino::Utf8Path,
    config: Option<&crate::config::Config>,
) -> Option<OpenMigration> {
    let id = config.and_then(|config| config.migration.clone())?;
    let workspace = crate::migrate::open_workspace(target).ok().flatten();
    let unfinished = workspace
        .as_ref()
        .filter(|workspace| workspace.join(crate::migrate::REGISTRY_FILE).is_file())
        .and_then(|workspace| crate::migrate::registry::load(workspace).ok())
        .map(|registry| {
            registry
                .sources
                .iter()
                .filter(|source| !source.status.finished())
                .count()
        });
    Some(OpenMigration {
        id,
        workspace: workspace.map(|workspace| workspace.to_string()),
        unfinished,
    })
}

/// The line a human report carries for an open migration.
fn render_migration(out: Output, migration: Option<&OpenMigration>) {
    if let Some(migration) = migration {
        out.result_line(format!(
            "migration {} is open; rk migrate status reads it",
            migration.id
        ));
    }
}

fn report_absent(
    out: Output,
    args: &StatusArgs,
    config: Option<&crate::config::Config>,
    git_work_tree: bool,
) -> Result<(), RkError> {
    let migration = open_migration(&args.target, config);
    let classification = match assess::standing(&args.target, config)? {
        assess::Standing::Migrating => Classification::Migration,
        assess::Standing::Unrecorded(class) => unrecorded(class),
    };
    let legacy = legacy_state();
    out.result_line(format!("config: {}", config_state(config, None).state));
    out.result_line(format!("no landing at {}", args.target));
    out.result_line(format!("classification: {}", classification.as_str()));
    render_migration(out, migration.as_ref());
    render_context(out, git_work_tree, &legacy);
    out.next(&[
        format!(
            "rk profile --target {} reports what a landing would select",
            args.target
        ),
        format!("rk init --target {} lands the workflow", args.target),
        format!(
            "rk adopt --target {} records a landing made before the receipt existed",
            args.target
        ),
        format!(
            "rk stage --target {} stages this binary's candidate; the rk-setup skill carries the best-effort migration",
            args.target
        ),
    ]);
    out.emit(&Report {
        schema: SCHEMA,
        landed: false,
        classification: Some(classification),
        git_work_tree,
        config: config_state(config, None),
        profile: None,
        git: None,
        capabilities: None,
        selection: None,
        rk_version: None,
        binary_version: None,
        alignment: None,
        drift: None,
        missing: None,
        stale_pins: None,
        sentinels: None,
        sentinel_locations: None,
        record_drift: None,
        invariant_failures: None,
        warnings: None,
        pending: None,
        violations: args.check.then(|| vec!["no landing".to_owned()]),
        legacy_state: legacy,
        migration,
    })?;
    if args.check {
        return Err(RkError::check_failed(
            Diagnostic::new(
                Reason::StateDrift,
                format!("no landing at {}, and --check requires one", args.target),
            )
            .expected("a target carrying .release-kit/manifest.json")
            .action("rk init lands the workflow; rk adopt records an existing landing; rk stage and the rk-setup skill carry the migration"),
        ));
    }
    Ok(())
}

/// What one pass over the record and the disk observed.
struct Observed {
    drift_rendered: Vec<String>,
    drift_seeded: Vec<String>,
    /// Recorded block destinations whose recorded digest the record's own
    /// parameters do not reproduce: the record was edited, not the file.
    parameter_drift: Vec<String>,
    /// Set differences between what the recorded parameters project —
    /// the withhold judgment applied — and the destinations the record
    /// names: a record whose parameters and file list disagree, whichever
    /// of the two was edited or outgrown.
    record_drift: Vec<String>,
    missing: Vec<String>,
    stale: Vec<StalePin>,
    sentinels: Vec<(String, usize, String)>,
    invariants: Vec<InvariantFailure>,
    /// Reportable conditions that are not violations.
    warnings: Vec<Warning>,
    /// Recorded intents this target cannot honour, counted in
    /// `record_drift` and kept separately because a plain upgrade cannot
    /// repair them: it re-reads the same recorded answer and refuses. An
    /// unavailable optional capability is not one of these; it is reported
    /// in `selection` and omitted, and an upgrade runs over it unharmed.
    incompatible: Vec<String>,
    /// The destinations an upgrade would change, or `None` where this
    /// binary carries no projection for the recorded pair and so cannot
    /// say.
    pending: Option<Vec<String>>,
    /// Every capability the catalog answers for the recorded values.
    selection: Option<Vec<CapabilityNote>>,
}

/// Report the target's landing.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory, the
/// record's own failure taxonomy for an unreadable or unknown record, and
/// [`RkError::CheckFailed`] under `--check` when the report holds a
/// violation.
pub fn run(args: &StatusArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    if !args.target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {} is not a directory", args.target),
            )
            .expected("an existing repository to report on"),
        ));
    }
    let config = crate::config::load(args.target.as_std_path())?;
    let git_work_tree = assess::git_work_tree(&args.target)?;
    let Some(manifest) = manifest::load(&args.target)? else {
        return report_absent(out, args, config.as_ref(), git_work_tree);
    };

    let migration = open_migration(&args.target, config.as_ref());
    let config = config_state(config.as_ref(), Some(&manifest));
    out.result_line(format!("config: {}", config.state));
    for key in &config.pending {
        out.result_line(format!(
            "config pending: {key}; rk upgrade --apply takes it up"
        ));
    }
    let observed = observe(args, &manifest)?;
    let alignment = manifest::alignment(&manifest.rk_version, env!("CARGO_PKG_VERSION"));
    render_human(out, args, &manifest, alignment, &observed);

    let violations = violations_of(&observed);
    // A record newer than this binary is no state it can judge, an open
    // migration included; below that, the open migration outranks the
    // record until it closes.
    let classification = if migration.is_some() && alignment != Alignment::TargetNewer {
        Some(Classification::Migration)
    } else {
        recorded(alignment, violations.is_empty())
    };
    out.result_line(format!(
        "classification: {}",
        classification.map_or(
            "none: the record is newer than this binary",
            Classification::as_str
        )
    ));
    render_migration(out, migration.as_ref());
    let legacy = legacy_state();
    render_context(out, git_work_tree, &legacy);
    out.emit(&Report {
        schema: SCHEMA,
        landed: true,
        classification,
        git_work_tree,
        config,
        profile: Some(manifest.profile.clone()),
        git: Some(manifest.git.clone()),
        capabilities: Some(manifest.capabilities.clone()),
        selection: observed.selection.clone(),
        rk_version: Some(manifest.rk_version),
        binary_version: Some(env!("CARGO_PKG_VERSION")),
        alignment: Some(alignment),
        drift: Some(Drift {
            rendered: observed.drift_rendered.len() + observed.parameter_drift.len(),
            seeded: observed.drift_seeded.len(),
        }),
        record_drift: Some(observed.record_drift.len()),
        missing: Some(observed.missing.clone()),
        stale_pins: Some(observed.stale),
        sentinels: Some(observed.sentinels.len()),
        sentinel_locations: Some(
            observed
                .sentinels
                .iter()
                .map(|(file, line, text)| SentinelAt {
                    file: file.clone(),
                    line: *line,
                    text: text.clone(),
                })
                .collect(),
        ),
        invariant_failures: Some(observed.invariants),
        warnings: Some(observed.warnings),
        pending: observed.pending.as_ref().map(Vec::len),
        violations: args.check.then(|| violations.clone()),
        legacy_state: legacy,
        migration,
    })?;

    if args.check && !violations.is_empty() {
        return Err(RkError::check_failed(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "the landing is not clean: {} violation{}",
                    violations.len(),
                    if violations.len() == 1 { "" } else { "s" }
                ),
            )
            .expected(
                "no rendered drift, no missing recorded file, no unresolved sentinel, no invariant failure",
            ),
        ));
    }
    Ok(())
}

/// The human lines every report carries, landed or not: whether the
/// target is a work tree the method can land in, and each path a 0.4.x
/// release left under the state root.
fn render_context(out: Output, git_work_tree: bool, legacy: &[LegacyPath]) {
    if !git_work_tree {
        out.result_line("not a non-bare Git work tree: the method lands files a work tree commits");
    }
    for entry in legacy {
        out.result_line(format!(
            "LEGACY {} ({}, {})",
            entry.path,
            entry.kind,
            if entry.active { "active" } else { "inactive" }
        ));
    }
}

/// The check-mode violation lines: rendered drift, missing recorded
/// files, unresolved sentinels, and invariant failures — the closed set
/// `landing:status-judges-only-under-check` names.
fn violations_of(observed: &Observed) -> Vec<String> {
    observed
        .drift_rendered
        .iter()
        .map(|path| format!("rendered drift: {path}"))
        .chain(
            observed
                .parameter_drift
                .iter()
                .map(|path| format!("parameter drift: {path}")),
        )
        .chain(
            observed
                .record_drift
                .iter()
                .map(|reason| format!("record drift: {reason}")),
        )
        .chain(
            observed
                .missing
                .iter()
                .map(|path| format!("missing: {path}")),
        )
        .chain(
            observed
                .sentinels
                .iter()
                .map(|(path, line, _)| format!("sentinel: {path}:{line}")),
        )
        .chain(
            observed
                .invariants
                .iter()
                .map(|failure| format!("invariant: {}: {}", failure.destination, failure.code)),
        )
        .collect()
}

/// One pass over the record and the disk: drift, missing files, stale
/// pins, and sentinels.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the record and the disk answers every comparison the report states"
)]
fn observe(args: &StatusArgs, manifest: &Manifest) -> Result<Observed, RkError> {
    let mut observed = Observed {
        drift_rendered: Vec::new(),
        drift_seeded: Vec::new(),
        parameter_drift: Vec::new(),
        record_drift: Vec::new(),
        missing: Vec::new(),
        stale: Vec::new(),
        sentinels: Vec::new(),
        invariants: Vec::new(),
        warnings: Vec::new(),
        incompatible: Vec::new(),
        pending: None,
        selection: None,
    };
    // One projection serves every reader below, because each asks what
    // this binary makes of the recorded parameters at this target. A pair
    // this binary does not carry cannot be projected at all: under a
    // receipt this binary wrote that is a defect and still fails, and
    // under a landing from another rk it is a fact to report.
    let aligned =
        manifest::alignment(&manifest.rk_version, env!("CARGO_PKG_VERSION")) == Alignment::Aligned;
    let projected = match project(args, manifest) {
        Ok(projection) => Some(projection),
        Err(err) if aligned => return Err(err),
        Err(_) => None,
    };
    for file in &manifest.files {
        let Some(bytes) = landing::read_recorded(&args.target, &file.destination)? else {
            observed.missing.push(file.destination.clone());
            continue;
        };
        if Digest::of(&bytes) != file.sha256 {
            match file.kind {
                Kind::Rendered => observed.drift_rendered.push(file.destination.clone()),
                Kind::Seeded => observed.drift_seeded.push(file.destination.clone()),
                Kind::State => {}
            }
        }
        observed.invariants.extend(invariants::failures(
            manifest.profile.release.driver.as_deref().unwrap_or(""),
            manifest.profile.forge.as_deref().unwrap_or(""),
            &file.destination,
            &bytes,
        ));
        let text = String::from_utf8_lossy(&bytes);
        for (idx, line) in text.lines().enumerate() {
            if line.contains(embedded::SENTINEL) {
                observed.sentinels.push((
                    file.destination.clone(),
                    idx + 1,
                    line.trim().to_owned(),
                ));
            }
        }
        // A marked document's markers must be well formed even when its
        // first block matches the receipt: a duplicate hook block still
        // executes, so an ill-formed document reads as rendered drift,
        // never as clean.
        let defective = projected.as_ref().is_some_and(|projection| {
            projection
                .collisions
                .iter()
                .any(|collision| collision.destination == file.destination)
        });
        if defective && !observed.drift_rendered.contains(&file.destination) {
            observed.drift_rendered.push(file.destination.clone());
        }
    }
    // The cross-file step: a landed file can generate the artifact the
    // forge actually executes, and the seeds ship no copy of it, so no
    // recorded digest sees the two disagree. The pair's own rule reads
    // both off the target's disk.
    observed.invariants.extend(invariants::target_failures(
        manifest.profile.release.driver.as_deref().unwrap_or(""),
        manifest.profile.forge.as_deref().unwrap_or(""),
        &args.target,
    ));
    observed.selection = projected.as_ref().map(|projection| {
        projection
            .capabilities
            .iter()
            .map(|selection| CapabilityNote::of(selection, projection))
            .collect()
    });
    if let Some(reason) = projected
        .as_ref()
        .and_then(|projection| projection.licence_refusal.clone())
    {
        observed.warnings.push(Warning {
            code: CODE_SCANNING_LICENCE,
            reason,
        });
    }
    // A receipt naming a release intent nothing can honour is judged at
    // every alignment rather than only where this binary wrote the record.
    // An unavailable optional capability is not one of these: it reports
    // through `selection` and lands nothing, and no verb refuses on it.
    if let Some(projection) = projected.as_ref() {
        observed.incompatible.clone_from(&projection.record_defects);
        observed
            .record_drift
            .extend(projection.record_defects.iter().cloned());
    }
    if aligned && let Some(projection) = projected.as_ref() {
        observe_parameter_drift(manifest, projection, &mut observed);
        observe_record_set(manifest, projection, &mut observed.record_drift);
    }
    // What an upgrade would change, which is the only honest ground for
    // telling an operator to run one. The recorded version says who wrote
    // the receipt, and two releases apart can carry identical bytes for
    // this pair, so it answers a different question and prompts nothing.
    observed.pending = projected
        .as_ref()
        .map(|projection| pending_of(manifest, &projection.candidates));
    // Stale means behind, not merely different: a landing from a newer rk
    // can carry pins ahead of this binary's registry, and that is the
    // alignment line's story, not a freshness complaint.
    for (tool, landed) in &manifest.pins {
        if let Some(available) = registry::version_of(tool)
            && manifest::version_is_newer(&available, landed)
        {
            observed.stale.push(StalePin {
                tool: tool.clone(),
                landed: landed.clone(),
                available,
            });
        }
    }
    Ok(observed)
}

/// The receipt-consistency step over every rendered destination.
///
/// Recorded digests alone cannot see a receipt edited only at its
/// parameters, since every file still matches its own record, so every
/// rendered candidate, whole file or region, as this projection renders
/// it from the receipt's own parameters, is compared against the digest
/// the receipt stores for it. Called only where this binary wrote the
/// receipt: an older landing's files legitimately differ from this
/// projection, which is the alignment line's story and the upgrade's job,
/// not parameter drift. A destination already reported as rendered drift
/// is the file's own story, not the receipt's, and is skipped too.
fn observe_parameter_drift(manifest: &Manifest, projection: &Projection, observed: &mut Observed) {
    for candidate in &projection.candidates {
        if candidate.kind != Kind::Rendered {
            continue;
        }
        let Some(record) = manifest.file(&candidate.destination) else {
            continue;
        };
        if observed.drift_rendered.contains(&candidate.destination)
            || observed.missing.contains(&candidate.destination)
        {
            continue;
        }
        if Digest::of(recorded_form(candidate)) != record.sha256 {
            observed
                .parameter_drift
                .push(format!("{} (parameters)", candidate.destination));
        }
    }
}

/// What this binary projects under the receipt's own parameters at this
/// target, with the same withhold judgment a landing applies, so the
/// comparison stands against what an upgrade would actually offer.
fn project(args: &StatusArgs, manifest: &Manifest) -> Result<Projection, RkError> {
    let evidence = TargetEvidence::gather(&args.target, Some(manifest))?;
    Projection::compute(&ProjectionInput {
        params: landing::Params::from_record(manifest),
        evidence,
    })
}

/// The bytes the receipt digests for a candidate: the whole file, or the
/// marked region alone.
fn recorded_form(candidate: &Candidate) -> &[u8] {
    candidate.region.as_deref().unwrap_or(&candidate.bytes)
}

/// The destinations an upgrade would change, read off the record alone.
///
/// A destination the projection adds or drops changes the record either
/// way, and a `rendered` one whose candidate digest differs from the
/// recorded digest is rewritten. A `seeded` or `state` destination the
/// record already names is never rewritten, so only a change of kind
/// counts for it. Disk drift is a separate story, told by its own lines:
/// an edited file is the target's doing, not a newer binary's.
fn pending_of(manifest: &Manifest, projected: &[Candidate]) -> Vec<String> {
    let mut pending = Vec::new();
    for entry in projected {
        let changed = manifest.file(&entry.destination).is_none_or(|record| {
            record.kind != entry.kind
                || (entry.kind == Kind::Rendered
                    && record.sha256 != Digest::of(recorded_form(entry)))
        });
        if changed {
            pending.push(entry.destination.clone());
        }
    }
    for file in &manifest.files {
        if !projected
            .iter()
            .any(|entry| entry.destination == file.destination)
        {
            pending.push(file.destination.clone());
        }
    }
    pending.sort();
    pending.dedup();
    pending
}

/// The receipt-set consistency step: the recorded digests judge each
/// named file, and the region re-render judges the region records, but
/// neither can see a receipt whose parameters and file list disagree, a
/// nix flag flipped in the receipt with no file landed, or a
/// once-withheld capability whose target grew into the supported shape.
/// So the projection is computed from the receipt's own parameters, the
/// same withhold judgment applied, and the two destination sets compared
/// both ways. A destination the projection withholds at this target is
/// absent because withheld, which is not drift. Called only where this
/// binary wrote the receipt: an older landing's set legitimately differs,
/// and that is the alignment line's story.
fn observe_record_set(
    manifest: &Manifest,
    projection: &Projection,
    record_drift: &mut Vec<String>,
) {
    for entry in &projection.candidates {
        if manifest.file(&entry.destination).is_none() {
            record_drift.push(format!(
                "the recorded parameters project {}, which the receipt does not name",
                entry.destination
            ));
        }
    }
    for file in &manifest.files {
        let produced = projection
            .candidates
            .iter()
            .any(|entry| entry.destination == file.destination)
            || projection
                .omissions
                .iter()
                .any(|omission| omission.destination == file.destination);
        if !produced {
            record_drift.push(format!(
                "the receipt names {}, which the recorded parameters do not project",
                file.destination
            ));
        }
    }
}

/// The human lines, identical with and without `--check`.
#[allow(
    clippy::too_many_lines,
    reason = "one pass prints every reportable condition in the order the report states them"
)]
fn render_human(
    out: Output,
    args: &StatusArgs,
    manifest: &Manifest,
    alignment: Alignment,
    observed: &Observed,
) {
    out.result_line(format!(
        "release-kit {} at {}: {}",
        manifest.rk_version,
        args.target,
        crate::commands::profile::describe(
            &manifest.profile,
            &manifest.git,
            &manifest.capabilities,
            &manifest.parameters.repo
        )
    ));
    // Every capability the record does not select is information, never
    // a violation: a product nobody asked for, or one this target's
    // dimensions cannot take.
    for note in observed.selection.iter().flatten() {
        if note.status != "selected" {
            out.result_line(format!(
                "capability {}: {}{}",
                note.id,
                note.status,
                note.reason
                    .as_deref()
                    .map_or_else(String::new, |reason| format!(" ({reason})"))
            ));
        }
    }
    if alignment == Alignment::TargetNewer {
        out.result_line(format!(
            "binary {} is older than this landing; install the matching rk",
            env!("CARGO_PKG_VERSION")
        ));
    }
    match observed.pending.as_deref() {
        None => out.result_line(format!(
            "this binary carries no projection for the recorded {} release on {}, so what an upgrade would change is unknown",
            manifest
                .profile
                .release
                .driver
                .as_deref()
                .unwrap_or("release-less"),
            manifest.profile.forge.as_deref().unwrap_or("no forge")
        )),
        Some(paths) => {
            for path in paths {
                out.result_line(format!("PENDING {path} (this binary would change it)"));
            }
        }
    }
    for path in &observed.drift_rendered {
        out.result_line(format!("DRIFT {path} (rendered, release-kit-owned)"));
    }
    for path in &observed.parameter_drift {
        out.result_line(format!(
            "DRIFT {path}: the recorded parameters do not render the recorded bytes"
        ));
    }
    for reason in &observed.record_drift {
        out.result_line(format!("DRIFT record: {reason}"));
    }
    for path in &observed.drift_seeded {
        out.result_line(format!("DRIFT {path} (seeded, target-owned)"));
    }
    for path in &observed.missing {
        out.result_line(format!("MISSING {path}"));
    }
    for pin in &observed.stale {
        out.result_line(format!(
            "STALE {} {} landed, {} in this binary",
            pin.tool, pin.landed, pin.available
        ));
    }
    for (path, line, text) in &observed.sentinels {
        out.result_line(format!("SENTINEL {path}:{line}: {text}"));
    }
    for failure in &observed.invariants {
        out.result_line(format!(
            "INVARIANT {} ({}): {}",
            failure.destination, failure.code, failure.reason
        ));
    }
    for warning in &observed.warnings {
        out.result_line(format!("WARNING ({}): {}", warning.code, warning.reason));
    }
    let mut next = Vec::new();
    for failure in &observed.invariants {
        next.push(format!("{}: {}", failure.destination, failure.remediation));
    }
    // The incompatible ones first, and with the override: a plain upgrade
    // reads the same recorded intent and refuses, so advertising it alone
    // would send the operator into a loop.
    if !observed.incompatible.is_empty() {
        next.push(format!(
            "rk upgrade --release-mode none --target {} retires the release intent this target cannot run; name a driver and forge this release carries to keep one",
            args.target
        ));
    }
    if observed.record_drift.len() > observed.incompatible.len() {
        next.push(format!(
            "rk upgrade --target {} rewrites the receipt from its parameters",
            args.target
        ));
    }
    if observed
        .pending
        .as_deref()
        .is_none_or(|paths| !paths.is_empty())
    {
        next.push(format!(
            "rk upgrade --target {} takes this landing to {}",
            args.target,
            env!("CARGO_PKG_VERSION")
        ));
    }
    next.push(format!(
        "rk status --check --target {} exits 1 on a violation",
        args.target
    ));
    if manifest.profile.release.mode != ReleaseMode::Automatic {
        next.retain(|line| !line.contains("rk method operate"));
    }
    out.next(&next);
}

#[cfg(test)]
mod tests {
    use super::{
        Classification, Drift, InvariantFailure, LegacyPath, OpenMigration, Report, SentinelAt,
        StalePin,
    };
    use crate::assess::{self, Facts, OwnLanding, Unrecorded};
    use crate::landing::CheckoutMode;
    use crate::landing::Integration;
    use crate::landing::manifest::Alignment;
    use crate::profile::{
        CapabilityRequests, GitWorkflow, ProfileSnapshot, ReleaseIntent, ReleaseMode,
    };

    /// The complete `rk.status/13` shape, held by snapshot in both the
    /// landed and absent forms.
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the snapshot states every field of both forms in one place"
    )]
    fn the_status_report_schema_snapshot_holds() {
        let landed = Report {
            schema: super::SCHEMA,
            landed: true,
            classification: Some(Classification::Upgrade),
            git_work_tree: true,
            config: super::ConfigState {
                state: "pending",
                pending: vec!["profile.release.style".into()],
            },
            profile: Some(ProfileSnapshot {
                technologies: vec!["rust".into()],
                forge: Some("github".into()),
                release: ReleaseIntent {
                    mode: ReleaseMode::Automatic,
                    driver: Some("rust".into()),
                    style: Some(crate::landing::Style::Trunk),
                    line_prefix: Some("release/".into()),
                },
            }),
            git: Some(GitWorkflow {
                trunk: "master".into(),
                checkout_mode: CheckoutMode::LinkedWorktree,
                integration: Integration::Local,
            }),
            capabilities: Some(CapabilityRequests {
                nix_packaging: true,
                reporting_policy: true,
                scorecard: false,
                code_scanning: Some(crate::landing::Provider::Semgrep),
                workflow_guards: Some(false),
            }),
            selection: Some(vec![]),
            rk_version: Some("0.1.0".into()),
            binary_version: Some("0.2.0"),
            alignment: Some(crate::landing::manifest::Alignment::BinaryNewer),
            drift: Some(Drift {
                rendered: 0,
                seeded: 1,
            }),
            missing: Some(vec![]),
            stale_pins: Some(vec![StalePin {
                tool: "release-plz".into(),
                landed: "0.3.160".into(),
                available: "0.3.170".into(),
            }]),
            sentinels: Some(1),
            sentinel_locations: Some(vec![SentinelAt {
                file: "release-plz.toml".into(),
                line: 18,
                text: "# TODO(release-kit): keep false for a binary-only crate".into(),
            }]),
            record_drift: Some(0),
            invariant_failures: Some(vec![InvariantFailure {
                code: "attestations-disabled",
                destination: "dist-workspace.toml".into(),
                reason: "github-attestations is not effectively true".into(),
                remediation: "set github-attestations = true in [dist]",
            }]),
            warnings: Some(vec![super::Warning {
                code: super::CODE_SCANNING_LICENCE,
                reason: "the target's license, LicenseRef-proprietary, does not state a codebase this release recognizes as open source".into(),
            }]),
            pending: Some(2),
            violations: None,
            legacy_state: vec![LegacyPath {
                path: "/state/release-kit/plans/0123456789abcdef".into(),
                kind: "plan",
                active: true,
            }],
            migration: Some(OpenMigration {
                id: "20261009T140205Z".into(),
                workspace: Some(
                    "/state/release-kit/migrations/widget-0123456789ab/20261009T140205Z".into(),
                ),
                unfinished: Some(2),
            }),
        };
        assert_eq!(
            serde_json::to_string(&landed).expect("a report serializes"),
            r##"{"schema":"rk.status/13","landed":true,"classification":"upgrade","git_work_tree":true,"config":{"state":"pending","pending":["profile.release.style"]},"profile":{"technologies":["rust"],"forge":"github","release":{"mode":"automatic","driver":"rust","style":"trunk","line_prefix":"release/"}},"git":{"trunk":"master","checkout_mode":"linked-worktree","integration":"local"},"capabilities":{"nix_packaging":true,"reporting_policy":true,"scorecard":false,"code_scanning":"semgrep","workflow_guards":false},"selection":[],"rk_version":"0.1.0","binary_version":"0.2.0","alignment":"binary-newer","drift":{"rendered":0,"seeded":1},"missing":[],"stale_pins":[{"tool":"release-plz","landed":"0.3.160","available":"0.3.170"}],"sentinels":1,"sentinel_locations":[{"file":"release-plz.toml","line":18,"text":"# TODO(release-kit): keep false for a binary-only crate"}],"record_drift":0,"invariant_failures":[{"code":"attestations-disabled","destination":"dist-workspace.toml","reason":"github-attestations is not effectively true","remediation":"set github-attestations = true in [dist]"}],"warnings":[{"code":"code-scanning-licence","reason":"the target's license, LicenseRef-proprietary, does not state a codebase this release recognizes as open source"}],"pending":2,"legacy_state":[{"path":"/state/release-kit/plans/0123456789abcdef","kind":"plan","active":true}],"migration":{"id":"20261009T140205Z","workspace":"/state/release-kit/migrations/widget-0123456789ab/20261009T140205Z","unfinished":2}}"##
        );
        let absent = Report {
            landed: false,
            classification: Some(Classification::Setup),
            git_work_tree: false,
            config: super::ConfigState {
                state: "absent",
                pending: vec![],
            },
            profile: None,
            git: None,
            capabilities: None,
            selection: None,
            rk_version: None,
            binary_version: None,
            alignment: None,
            drift: None,
            missing: None,
            stale_pins: None,
            sentinels: None,
            sentinel_locations: None,
            record_drift: None,
            invariant_failures: None,
            warnings: None,
            pending: None,
            violations: None,
            legacy_state: vec![],
            migration: None,
            ..landed
        };
        assert_eq!(
            serde_json::to_string(&absent).expect("a report serializes"),
            r#"{"schema":"rk.status/13","landed":false,"classification":"setup","git_work_tree":false,"config":{"state":"absent","pending":[]},"legacy_state":[]}"#,
            "an absent landing reports the fields a caller can branch on"
        );
        let newer = Report {
            classification: None,
            ..absent
        };
        assert!(
            serde_json::to_string(&newer)
                .expect("a report serializes")
                .contains(r#""classification":null"#),
            "no classification is stated as null, never omitted"
        );
    }

    fn facts() -> Facts {
        Facts {
            technologies: Vec::new(),
            forge: None,
            repo: None,
            release_markers: Vec::new(),
            collisions: Vec::new(),
            git: true,
            tags: 0,
            long_lived_branches: Vec::new(),
        }
    }

    /// Each value the gate reads has one rule that earns it: the
    /// assessment's three for a target with no record, the adoption
    /// detector before them, and the alignment and the check for a
    /// recorded one.
    #[test]
    fn every_classification_has_its_rule() {
        let none = OwnLanding::default();
        let classify = |facts: &Facts, own: &OwnLanding| {
            super::unrecorded(assess::classify_unrecorded(facts, own))
        };
        assert_eq!(classify(&facts(), &none), Classification::Setup);
        let marked = Facts {
            release_markers: vec![".goreleaser.yaml".into()],
            ..facts()
        };
        assert_eq!(classify(&marked, &none), Classification::Migration);
        let tagged = Facts { tags: 3, ..facts() };
        assert_eq!(classify(&tagged, &none), Classification::NeedsDecision);
        // release-kit's own regions read as collisions to the assessment,
        // and the detector outranks them.
        let own = OwnLanding {
            regions: vec!["AGENTS.md".into()],
            rendered: Vec::new(),
        };
        let colliding = Facts {
            collisions: vec!["AGENTS.md".into()],
            release_markers: vec!["CHANGELOG.md".into()],
            ..facts()
        };
        assert_eq!(classify(&colliding, &own), Classification::Adopt);
        assert_eq!(
            assess::classify_unrecorded(&colliding, &own),
            Unrecorded::Adopt
        );
        assert_eq!(
            super::recorded(Alignment::BinaryNewer, false),
            Some(Classification::Upgrade)
        );
        assert_eq!(
            super::recorded(Alignment::Aligned, false),
            Some(Classification::Drift)
        );
        assert_eq!(
            super::recorded(Alignment::Aligned, true),
            Some(Classification::Current)
        );
        assert_eq!(super::recorded(Alignment::TargetNewer, true), None);
        for (class, word) in [
            (Classification::Setup, "setup"),
            (Classification::Migration, "migration"),
            (Classification::NeedsDecision, "needs-decision"),
            (Classification::Adopt, "adopt"),
            (Classification::Upgrade, "upgrade"),
            (Classification::Drift, "drift"),
            (Classification::Current, "current"),
        ] {
            assert_eq!(class.as_str(), word);
            assert_eq!(
                serde_json::to_string(&class).expect("serializes"),
                format!("\"{word}\"")
            );
        }
    }
}
