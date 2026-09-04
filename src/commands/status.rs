//! `rk status`: a target describes itself from its own disk.
//!
//! Read-only and offline: the record supplies what landed, the binary's
//! embedded registry supplies the pin comparison, and no network is ever
//! touched — a fetch is a way for a status command to hang, fail on a
//! network it should not need, or leak a repository's existence. Plain
//! `rk status` reports and exits 0 for every reportable state, drift and
//! no-landing included; `--check` computes the identical report and
//! changes only the final judgment, the one sanctioned bare exit 1.

use serde::Serialize;

use crate::cli::status::StatusArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::invariants::{self, InvariantFailure};
use crate::landing::manifest::{self, Alignment, Manifest};
use crate::landing::{self, Kind};
use crate::output::Output;
use crate::{embedded, registry};

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

/// The machine form of a status report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// Whether a landing record exists; every other field needs one.
    landed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tech: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    forge: Option<String>,
    /// The recorded working-copy mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow: Option<&'static str>,
    /// The recorded release style; absent on a record predating it.
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'static str>,
    /// Whether the landing carries the Nix capability; a record predating
    /// the parameter reads as opt-out.
    #[serde(skip_serializing_if = "Option::is_none")]
    nix: Option<bool>,
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
    /// Invariants a landed file's effective configuration violates —
    /// judged, never rewritten, because the file stays the target's.
    #[serde(skip_serializing_if = "Option::is_none")]
    invariant_failures: Option<Vec<InvariantFailure>>,
    /// Present only under `--check`: what the judgment failed on.
    #[serde(skip_serializing_if = "Option::is_none")]
    violations: Option<Vec<String>>,
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
    let Some(manifest) = manifest::load(&args.target)? else {
        out.result_line(format!("no landing at {}", args.target));
        out.next(&[
            format!(
                "rk init --tech <tech> --target {} lands the workflow",
                args.target
            ),
            format!(
                "rk adopt --target {} records a landing made before the record existed",
                args.target
            ),
        ]);
        out.emit(&Report {
            schema: "rk.status/5",
            landed: false,
            tech: None,
            forge: None,
            workflow: None,
            style: None,
            nix: None,
            rk_version: None,
            binary_version: None,
            alignment: None,
            drift: None,
            missing: None,
            stale_pins: None,
            sentinels: None,
            invariant_failures: None,
            violations: args.check.then(|| vec!["no landing".to_owned()]),
        })?;
        if args.check {
            return Err(RkError::check_failed(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!("no landing at {}, and --check requires one", args.target),
                )
                .expected("a target carrying .release-kit/manifest.json")
                .action("rk init lands the workflow; rk adopt records an existing landing"),
            ));
        }
        return Ok(());
    };

    let observed = observe(args, &manifest)?;
    let alignment = manifest::alignment(&manifest.rk_version, env!("CARGO_PKG_VERSION"));
    render_human(out, args, &manifest, alignment, &observed);

    let violations = violations_of(&observed);
    out.emit(&Report {
        schema: "rk.status/5",
        landed: true,
        tech: Some(manifest.tech),
        forge: Some(manifest.forge),
        workflow: Some(manifest.parameters.workflow.as_str()),
        style: manifest.parameters.style.map(manifest::Style::as_str),
        nix: Some(manifest.parameters.nix),
        rk_version: Some(manifest.rk_version),
        binary_version: Some(env!("CARGO_PKG_VERSION")),
        alignment: Some(alignment),
        drift: Some(Drift {
            rendered: observed.drift_rendered.len()
                + observed.parameter_drift.len()
                + observed.record_drift.len(),
            seeded: observed.drift_seeded.len(),
        }),
        missing: Some(observed.missing.clone()),
        stale_pins: Some(observed.stale),
        sentinels: Some(observed.sentinels.len()),
        invariant_failures: Some(observed.invariants),
        violations: args.check.then(|| violations.clone()),
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
            &manifest.tech,
            &manifest.forge,
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
        // The hook file's markers must be well formed even when its first
        // block matches the record: a duplicate block still executes, so
        // an ill-formed file reads as rendered drift, never as clean.
        if file.destination == landing::HOOKS_DESTINATION
            && !observed.drift_rendered.contains(&file.destination)
            && landing::hooks_file_defect(&args.target)?.is_some()
        {
            observed.drift_rendered.push(file.destination.clone());
        }
    }
    // The record-consistency step: recorded digests alone cannot see a
    // manifest edited only at its parameters — every file still matches
    // its own record — so the two mode-bearing block destinations are
    // re-rendered from the record's own parameters and compared against
    // the digest the record stores for each. Only where the recorded
    // payload is this binary's: an older landing's blocks legitimately
    // differ from this payload's candidate — that is the alignment line's
    // story and the upgrade's job, not parameter drift. A destination
    // already reported as rendered drift is the file's own story, not the
    // record's, and is skipped too.
    let same_payload = manifest.payload_sha256 == crate::commands::payload::report().payload_sha256;
    for (destination, template) in [
        (
            landing::AGENTS_DESTINATION,
            landing::routing_block(manifest.parameters.workflow),
        ),
        (
            landing::HOOKS_DESTINATION,
            landing::hooks_block(manifest.parameters.workflow),
        ),
    ] {
        if !same_payload {
            break;
        }
        let Some(record) = manifest.file(destination) else {
            continue;
        };
        if observed
            .drift_rendered
            .iter()
            .any(|path| path == destination)
            || observed.missing.iter().any(|path| path == destination)
        {
            continue;
        }
        let candidate = landing::render(
            template.as_bytes(),
            &manifest.parameters.repo,
            &manifest.parameters.scopes,
            manifest.parameters.style,
        );
        if Digest::of(&candidate) != record.sha256 {
            observed
                .parameter_drift
                .push(format!("{destination} (parameters.workflow)"));
        }
    }
    if same_payload {
        observe_record_set(args, manifest, &mut observed.record_drift)?;
    }
    // Stale means behind, not merely different: a landing from a newer rk
    // can carry pins ahead of this binary's registry, and that is the
    // alignment line's story, not a freshness complaint.
    for (tool, landed) in &manifest.pins {
        if let Some(available) = registry::version_of(tool) {
            if manifest::version_is_newer(&available, landed) {
                observed.stale.push(StalePin {
                    tool: tool.clone(),
                    landed: landed.clone(),
                    available,
                });
            }
        }
    }
    Ok(observed)
}

/// The record-set consistency step: the recorded digests judge each
/// named file, and the block re-render judges the two block records, but
/// neither can see a record whose parameters and file list disagree — a
/// nix flag flipped in the record with no file landed, or a once-withheld
/// capability whose target grew into the supported shape. So the
/// projection is reconstructed from the record's own parameters, the same
/// withhold judgment applied, and the two destination sets compared both
/// ways. Called only under this binary's own payload: an older landing's
/// set legitimately differs, and that is the alignment line's story.
fn observe_record_set(
    args: &StatusArgs,
    manifest: &Manifest,
    record_drift: &mut Vec<String>,
) -> Result<(), RkError> {
    let mut projected = landing::projection(
        &manifest.tech,
        &manifest.forge,
        &manifest.parameters.repo,
        &manifest.parameters.scopes,
        manifest.parameters.workflow,
        manifest.parameters.style,
        manifest.parameters.nix,
    )?;
    landing::withhold_nix(
        &args.target,
        manifest.parameters.nix,
        Some(manifest),
        &mut projected,
    )?;
    for entry in &projected {
        if manifest.file(&entry.destination).is_none() {
            record_drift.push(format!(
                "the recorded parameters project {}, which the record does not name",
                entry.destination
            ));
        }
    }
    for file in &manifest.files {
        if !projected
            .iter()
            .any(|entry| entry.destination == file.destination)
        {
            record_drift.push(format!(
                "the record names {}, which the recorded parameters do not project",
                file.destination
            ));
        }
    }
    Ok(())
}

/// The human lines, identical with and without `--check`.
fn render_human(
    out: Output,
    args: &StatusArgs,
    manifest: &Manifest,
    alignment: Alignment,
    observed: &Observed,
) {
    out.result_line(format!(
        "release-kit {} ({}, {}, {} workflow, {} style{}) at {}",
        manifest.rk_version,
        manifest.tech,
        manifest.forge,
        manifest.parameters.workflow.as_str(),
        manifest
            .parameters
            .style
            .map_or("unrecorded", manifest::Style::as_str),
        if manifest.parameters.nix { ", nix" } else { "" },
        args.target
    ));
    match alignment {
        Alignment::BinaryNewer => out.result_line(format!(
            "binary {} is newer; run 'rk upgrade'",
            env!("CARGO_PKG_VERSION")
        )),
        Alignment::TargetNewer => out.result_line(format!(
            "binary {} is older than this landing; install the matching rk",
            env!("CARGO_PKG_VERSION")
        )),
        Alignment::Aligned => {}
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
    let mut next = Vec::new();
    for failure in &observed.invariants {
        next.push(format!("{}: {}", failure.destination, failure.remediation));
    }
    if !observed.record_drift.is_empty() {
        next.push(format!(
            "rk upgrade --target {} reconciles the record with its parameters",
            args.target
        ));
    }
    if alignment == Alignment::BinaryNewer {
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
    out.next(&next);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Drift, InvariantFailure, Report, StalePin};

    /// The complete `rk.status/5` shape, held by snapshot in both the
    /// landed and absent forms.
    #[test]
    fn the_status_report_schema_snapshot_holds() {
        let landed = Report {
            schema: "rk.status/5",
            landed: true,
            tech: Some("rust".into()),
            forge: Some("github".into()),
            workflow: Some("worktree"),
            style: Some("trunk"),
            nix: Some(true),
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
            invariant_failures: Some(vec![InvariantFailure {
                code: "attestations-disabled",
                destination: "dist-workspace.toml".into(),
                reason: "github-attestations is not effectively true".into(),
                remediation: "set github-attestations = true in [dist]",
            }]),
            violations: None,
        };
        assert_eq!(
            serde_json::to_string(&landed).expect("a report serializes"),
            r#"{"schema":"rk.status/5","landed":true,"tech":"rust","forge":"github","workflow":"worktree","style":"trunk","nix":true,"rk_version":"0.1.0","binary_version":"0.2.0","alignment":"binary-newer","drift":{"rendered":0,"seeded":1},"missing":[],"stale_pins":[{"tool":"release-plz","landed":"0.3.160","available":"0.3.170"}],"sentinels":1,"invariant_failures":[{"code":"attestations-disabled","destination":"dist-workspace.toml","reason":"github-attestations is not effectively true","remediation":"set github-attestations = true in [dist]"}]}"#
        );
        let absent = Report {
            landed: false,
            tech: None,
            forge: None,
            workflow: None,
            style: None,
            nix: None,
            rk_version: None,
            binary_version: None,
            alignment: None,
            drift: None,
            missing: None,
            stale_pins: None,
            sentinels: None,
            invariant_failures: None,
            violations: None,
            ..landed
        };
        assert_eq!(
            serde_json::to_string(&absent).expect("a report serializes"),
            r#"{"schema":"rk.status/5","landed":false}"#,
            "an absent landing reports one field a caller can branch on"
        );
    }
}
