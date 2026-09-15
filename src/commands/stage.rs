//! `rk stage`: write the candidate this binary would land into a
//! disposable stage, and `rk stage clean`: remove exactly one such stage.
//!
//! The create verb reads the target the way the landing verbs do, resolves
//! the same parameters, computes the one pure projection, and materializes
//! it under the resolved output. It writes nothing inside the target, and
//! nothing about it changes what a production landing does: `rk init`,
//! `rk upgrade`, `rk adopt`, and `rk status` never read a stage. The
//! clean verb removes one directory whose own receipt names it, through
//! held descriptors, and refuses everything else.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::cli::stage::{StageAction, StageArgs};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, Provider};
use crate::landing::{self, Params};
use crate::output::Output;
use crate::projection::{Projection, ProjectionInput, TargetEvidence};
use crate::stage::{self, OutputSource, Receipt, clean};

/// The machine form of a staging: the receipt as written, plus where the
/// output path came from and what follows.
#[derive(Debug, Serialize)]
struct Report<'a> {
    /// The receipt, its `schema` first.
    #[serde(flatten)]
    receipt: &'a Receipt,
    /// `--output`, `RK_STAGE_ROOT`, or `state root`.
    output_source: &'static str,
    /// What plausibly follows.
    next: Vec<String>,
}

/// The machine form of a cleanup.
#[derive(Debug, Serialize)]
struct CleanReport {
    /// The shape version of this document.
    schema: &'static str,
    /// The stage that was removed.
    stage_root: String,
    /// The target its receipt named.
    target: String,
    /// Whether the directory is gone.
    removed: bool,
    /// The command that recreates it.
    recovery: String,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Stage the candidate, or remove a stage.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory, the
/// parameter resolution's own failures, a `destructive-refusal` for a
/// stage root inside the target, a `state-drift` refusal for an existing
/// nonempty output, and [`RkError::Io`] for a write that fails;
/// `clean` returns the refusals `crate::stage::clean::validate` names.
pub fn run(args: &StageArgs) -> Result<(), RkError> {
    match &args.action {
        Some(StageAction::Clean { path, json }) => remove(Output::new(*json), path),
        None => create(args),
    }
}

fn create(args: &StageArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    if !args.target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {} is not a directory", args.target),
            )
            .expected("an existing repository to stage the candidate for"),
        ));
    }
    let target = canonical(&args.target)?;
    let config = crate::config::load(target.as_std_path())?;
    // The record explains the target to the projection; a record this
    // binary cannot read is reported and stands aside, because a stage is
    // evidence and refusing to explain would leave the agent with less.
    let record = match manifest::load(&target) {
        Ok(record) => record,
        Err(error) => {
            out.warn(format!(
                "the landing record was not read, so the candidate is computed as a first landing: {error}"
            ));
            None
        }
    };
    let receipt_schema_version = stage::recorded_schema_version(&target);
    let params = Params::resolve(
        &target,
        &landing::Inputs {
            nix: args.nix_packaging.then_some(true),
            reporting_policy: args.reporting_policy.then_some(true),
            scorecard: args.scorecard.then_some(true),
            code_scanning: args
                .code_scanning
                .as_deref()
                .map(Provider::parse)
                .transpose()?,
            ..args.profile.inputs()?
        },
        config.as_ref(),
        record.as_ref(),
        if record.is_some() {
            landing::Purpose::Upgrade
        } else {
            landing::Purpose::Init
        },
    )?;
    let evidence = TargetEvidence::gather(&target, record.as_ref())?;
    let projection = Projection::compute(&ProjectionInput {
        params: params.clone(),
        evidence,
    })?;
    let (output, source) = stage::resolve_output(args.output.as_deref(), target.as_std_path())?;
    let prepared = stage::prepare(&output, source, target.as_std_path())?;
    let composed = stage::compose(
        &projection,
        &params,
        target.as_std_path(),
        prepared.resolved(),
        record.as_ref(),
        receipt_schema_version,
    );
    stage::write(&prepared, &composed)?;
    let receipt = &composed.receipt;
    let next = vec![
        format!(
            "read {}/{} for the candidate inventory and its reasons",
            receipt.stage_root,
            stage::RECEIPT_NAME
        ),
        format!(
            "compare {}/{} with {} before anything lands",
            receipt.stage_root,
            stage::ARTIFACTS_DIR,
            receipt.target
        ),
        format!(
            "rk stage clean {} removes the stage once the landing is verified",
            receipt.stage_root
        ),
    ];
    render(out, receipt, source);
    out.next(&next);
    out.emit(&Report {
        receipt,
        output_source: source.as_str(),
        next,
    })
}

/// The human report, one fact per line.
fn render(out: Output, receipt: &Receipt, source: OutputSource) {
    out.result_line(format!("stage: {}", receipt.stage_root));
    out.result_line(format!("output: from {}", source.as_str()));
    out.result_line(format!("target: {}", receipt.target));
    let parameters = &receipt.parameters;
    out.result_line(format!(
        "parameters: {}",
        crate::commands::profile::describe(
            &parameters.profile,
            &parameters.git,
            &parameters.capabilities,
            &parameters.repo
        )
    ));
    for note in &receipt.capabilities {
        out.result_line(format!(
            "capability {}: {}{}",
            note.id,
            note.status,
            note.reason
                .as_deref()
                .map_or_else(String::new, |reason| format!(" ({reason})"))
        ));
    }
    out.result_line(format!(
        "landing record: {}",
        receipt.receipt_schema_version.map_or_else(
            || "none".to_owned(),
            |version| format!("schema_version {version}")
        )
    ));
    out.result_line(format!("candidates: {}", receipt.candidates.len()));
    for candidate in &receipt.candidates {
        out.result_line(format!(
            "  {}/{} ({}, {})",
            stage::ARTIFACTS_DIR,
            candidate.destination,
            candidate.kind.as_str(),
            candidate.placement
        ));
    }
    for note in &receipt.omissions {
        out.result_line(format!("omitted {}: {}", note.destination, note.reason));
        if let Some(action) = &note.action {
            out.result_line(format!("  action: {action}"));
        }
    }
    for note in &receipt.collisions {
        out.result_line(format!("collision {}: {}", note.destination, note.reason));
    }
    for destination in &receipt.retired {
        out.result_line(format!(
            "retired {destination}: recorded, no longer produced; target-owned from the next landing"
        ));
    }
    for destination in &receipt.seeded_present {
        out.result_line(format!("seeded present {destination}: a landing keeps it"));
    }
    for destination in &receipt.state_present {
        out.result_line(format!("state present {destination}: a landing keeps it"));
    }
    out.result_line(format!(
        "reference: {}",
        receipt
            .reference
            .iter()
            .map(|root| format!("{}/{root}", stage::REFERENCE_DIR))
            .collect::<Vec<_>>()
            .join(", ")
    ));
}

fn remove(out: Output, path: &Utf8Path) -> Result<(), RkError> {
    let validated = clean::validate(path.as_std_path())?;
    clean::remove(&validated)?;
    let stage_root = validated.resolved().display().to_string();
    let target = validated.target().to_owned();
    let recovery = format!("rk stage --target {target} --output {stage_root}");
    out.result_line(format!("removed {stage_root}"));
    out.result_line(format!("recovery: {recovery} stages the candidate again"));
    let next = vec![
        format!("rk status --target {target} reports what the target holds"),
        format!("rk stage --target {target} stages this binary's candidate again"),
    ];
    out.next(&next);
    out.emit(&CleanReport {
        schema: "rk.stage-clean/1",
        stage_root,
        target,
        removed: true,
        recovery,
        next,
    })
}

/// The canonical absolute target, as text.
fn canonical(target: &Utf8Path) -> Result<Utf8PathBuf, RkError> {
    let path = std::fs::canonicalize(target)?;
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| RkError::Usage(format!("the target path is not UTF-8: {}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::{CleanReport, Report};
    use crate::stage::{Parameters, Receipt, STAGE_SCHEMA};

    /// The `rk stage --json` report is the receipt with the output source
    /// and the next lines beside it, held by snapshot.
    #[test]
    fn the_stage_report_schema_snapshot_holds() {
        let receipt = Receipt {
            schema: STAGE_SCHEMA.to_owned(),
            rk_version: "0.0.0".into(),
            target: "/tmp/t".into(),
            stage_root: "/tmp/s".into(),
            parameters: Parameters {
                profile: crate::profile::ProfileSnapshot {
                    technologies: vec!["rust".into()],
                    forge: Some("github".into()),
                    release: crate::profile::ReleaseIntent {
                        mode: crate::profile::ReleaseMode::Automatic,
                        driver: Some("rust".into()),
                        style: None,
                        line_prefix: Some("release/".into()),
                    },
                },
                git: crate::profile::GitWorkflow {
                    trunk: "main".into(),
                    checkout_mode: crate::landing::CheckoutMode::MainWorktree,
                },
                capabilities: crate::profile::CapabilityRequests {
                    nix_packaging: true,
                    reporting_policy: true,
                    scorecard: false,
                    code_scanning: None,
                },
                repo: "acme/widget".into(),
                security_contact: String::new(),
                security_response: "best-effort".into(),
            },
            capabilities: vec![],
            receipt_schema_version: None,
            candidates: vec![],
            omissions: vec![],
            collisions: vec![],
            retired: vec![],
            seeded_present: vec![],
            state_present: vec![],
            reference: vec!["CHANGELOG.md".into()],
        };
        let report = Report {
            receipt: &receipt,
            output_source: "--output",
            next: vec![
                "rk stage clean /tmp/s removes the stage once the landing is verified".into(),
            ],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.stage/4","rk_version":"0.0.0","target":"/tmp/t","stage_root":"/tmp/s","parameters":{"profile":{"technologies":["rust"],"forge":"github","release":{"mode":"automatic","driver":"rust","line_prefix":"release/"}},"git":{"trunk":"main","checkout_mode":"main-worktree"},"capabilities":{"nix_packaging":true,"reporting_policy":true,"scorecard":false},"repo":"acme/widget","security_contact":"","security_response":"best-effort"},"capabilities":[],"receipt_schema_version":null,"candidates":[],"omissions":[],"collisions":[],"retired":[],"seeded_present":[],"state_present":[],"reference":["CHANGELOG.md"],"output_source":"--output","next":["rk stage clean /tmp/s removes the stage once the landing is verified"]}"#
        );
    }

    /// The complete `rk.stage-clean/1` shape, held by snapshot.
    #[test]
    fn the_stage_clean_report_schema_snapshot_holds() {
        let report = CleanReport {
            schema: "rk.stage-clean/1",
            stage_root: "/tmp/s".into(),
            target: "/tmp/t".into(),
            removed: true,
            recovery: "rk stage --target /tmp/t --output /tmp/s".into(),
            next: vec!["rk status --target /tmp/t reports what the target holds".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.stage-clean/1","stage_root":"/tmp/s","target":"/tmp/t","removed":true,"recovery":"rk stage --target /tmp/t --output /tmp/s","next":["rk status --target /tmp/t reports what the target holds"]}"#
        );
    }
}
