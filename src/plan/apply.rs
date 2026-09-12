//! The apply: a stored plan executed exactly, or refused because its
//! inputs moved.
//!
//! An apply that recomputed intent from the tree could act on something
//! the operator never saw. So it reads the plan that was reviewed, asks
//! one question first, is the world still the one the plan described,
//! and refuses naming what moved when it is not. Readiness is enforced
//! with no override: a gap is honest and is not permission. Every local
//! operation is staged through the transaction in [`crate::atomic`] and
//! renamed in order, with the record last, and the run lands in the
//! journal `rk runs` reads.

use std::collections::BTreeMap;

use camino::Utf8Path;
use serde::Serialize;

use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::{self, manifest};
use crate::self_depend::manager::{self, Manager, PinRead};
use crate::setup::journal::Journal;

use super::{Classification, Operation, Plan, Postcondition, Readiness, fingerprint};

/// The version of the apply report's shape.
pub const APPLY_SCHEMA: &str = "rk.reconcile-apply/1";

/// One operation and what happened to it.
#[derive(Debug, Clone, Serialize)]
pub struct OperationResult {
    /// The operation's kind word.
    pub op: &'static str,
    /// The path it touched, where it touched one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `ok`.
    pub status: &'static str,
}

/// One postcondition and what it found.
#[derive(Debug, Clone, Serialize)]
pub struct PostconditionResult {
    /// The check, as the plan names it.
    pub check: String,
    /// `ok`, `failed`, or `reported` for the standing status check,
    /// whose answer names what is short without failing the apply.
    pub status: &'static str,
    /// What was found, where the check failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The machine form of an apply.
#[derive(Debug, Serialize)]
pub struct Applied {
    /// The shape version of this document.
    pub schema: &'static str,
    /// The plan that was applied.
    pub plan_id: String,
    /// The fingerprint the apply revalidated against.
    pub input_fingerprint: Digest,
    /// The target written.
    pub target: String,
    /// The plan's classification.
    pub classification: Classification,
    /// Every operation, in the order it landed.
    pub operations: Vec<OperationResult>,
    /// Every postcondition, with its outcome.
    pub postconditions: Vec<PostconditionResult>,
    /// The journal entry, where the journal took one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// What plausibly follows.
    pub next: Vec<String>,
}

impl Applied {
    /// The postconditions that failed.
    #[must_use]
    pub fn failed(&self) -> Vec<&PostconditionResult> {
        self.postconditions
            .iter()
            .filter(|result| result.status == "failed")
            .collect()
    }

    /// The failure a caller returns after rendering the report: the
    /// writes landed, and a check then found the target short of what
    /// the plan promised.
    #[must_use]
    pub fn failure(&self) -> Option<RkError> {
        let failed = self.failed();
        if failed.is_empty() {
            return None;
        }
        Some(RkError::check_failed(
            Diagnostic::new(
                Reason::PostconditionFailed,
                format!(
                    "the writes landed and {} postcondition{} failed: {}",
                    failed.len(),
                    if failed.len() == 1 { "" } else { "s" },
                    failed
                        .iter()
                        .map(|result| {
                            format!(
                                "{} ({})",
                                result.check,
                                result.detail.as_deref().unwrap_or("no detail")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
            .expected("every postcondition the plan carries satisfied after the writes")
            .action(format!(
                "rk status --target {} names what is short",
                self.target
            ))
            .target_state("written"),
        ))
    }
}

/// Refuse anything but a ready plan.
///
/// # Errors
///
/// A refusal with [`Reason::PlanNotReady`] naming the unresolved decision
/// ids for a plan that needs a decision, or the failed required
/// preconditions for a blocked one.
pub fn gate(plan: &Plan) -> Result<(), RkError> {
    match plan.readiness {
        Readiness::Ready => Ok(()),
        Readiness::NeedsDecision => {
            let ids: Vec<String> = plan
                .preconditions
                .iter()
                .filter(|p| !p.evaluation.holds())
                .filter_map(|p| p.decision.clone())
                .collect();
            Err(RkError::refusal(
                Diagnostic::new(
                    Reason::PlanNotReady,
                    format!(
                        "plan {} waits on a decision, and nothing was written: {}",
                        plan.identity.plan_id,
                        ids.join(", ")
                    ),
                )
                .expected("every decision the plan names selected")
                .action("rk reconcile plan --decide <id>=<answer> selects an answer and stores a fresh plan")
                .target_state("unchanged"),
            ))
        }
        Readiness::Blocked => {
            let failed: Vec<String> = plan
                .preconditions
                .iter()
                .filter(|p| p.requirement == super::Requirement::Required && !p.evaluation.holds())
                .map(|p| {
                    let reason = match &p.evaluation {
                        super::Evaluation::Satisfied => String::new(),
                        super::Evaluation::NotObserved { reason }
                        | super::Evaluation::Unsatisfied { reason } => reason.clone(),
                    };
                    format!("{} ({reason})", p.id)
                })
                .collect();
            Err(RkError::refusal(
                Diagnostic::new(
                    Reason::PlanNotReady,
                    format!(
                        "plan {} is blocked, and nothing was written: {}",
                        plan.identity.plan_id,
                        failed.join(", ")
                    ),
                )
                .expected("every required precondition satisfied")
                .action("resolve each, then rk reconcile plan again")
                .target_state("unchanged"),
            ))
        }
    }
}

/// Refuse a stored plan whose semantic inputs no longer match a fresh
/// computation over the same request.
///
/// # Errors
///
/// A refusal with [`Reason::StateDrift`] naming every field or
/// destination whose canonical line changed, collected in one pass.
pub fn revalidate(stored: &Plan, fresh: &Plan) -> Result<(), RkError> {
    // The stored document is recomputed too, so a plan edited in the
    // store after approval reads as moved rather than as approved.
    let stored_now = fingerprint::compute(stored);
    if stored.input_fingerprint == fresh.input_fingerprint && stored_now == stored.input_fingerprint
    {
        return Ok(());
    }
    let before = fingerprint::canonical(stored);
    let after = fingerprint::canonical(fresh);
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let mut moved: Vec<String> = Vec::new();
    if stored_now != stored.input_fingerprint {
        moved.push("the stored plan".to_owned());
    }
    for line in before_lines
        .iter()
        .filter(|line| !after_lines.contains(line))
        .chain(
            after_lines
                .iter()
                .filter(|line| !before_lines.contains(line)),
        )
    {
        let label = label(line);
        if !moved.contains(&label) {
            moved.push(label);
        }
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "plan {} no longer matches its inputs, and nothing was written: {}",
                stored.identity.plan_id,
                moved.join(", ")
            ),
        )
        .expected("the target, the bundle, and the decisions as the plan observed them")
        .action("rk reconcile plan computes a fresh plan over what is there now")
        .target_state("unchanged"),
    ))
}

/// The human label for one canonical line.
fn label(line: &str) -> String {
    let mut parts = line.split('\t');
    match parts.next().unwrap_or_default() {
        "candidate" => "the candidate bundle".to_owned(),
        "record" => "the record".to_owned(),
        "configuration" => "the configuration".to_owned(),
        "operation" => {
            let kind = parts.next().unwrap_or_default();
            let subject = parts.next().unwrap_or_default();
            format!("operation {kind} {subject}")
        }
        "precondition" => format!("precondition {}", parts.next().unwrap_or_default()),
        "decision" => format!("decision {}", parts.next().unwrap_or_default()),
        other => other.to_owned(),
    }
}

/// Refuse where a destination no longer holds the digest an operation's
/// `before` names, every mismatch collected in one pass.
///
/// # Errors
///
/// A refusal with [`Reason::StateDrift`] naming each destination, and
/// [`RkError::Io`] for a read that fails.
pub fn verify_before_digests(target: &Utf8Path, plan: &Plan) -> Result<(), RkError> {
    let mut moved: Vec<String> = Vec::new();
    let pin = crate::self_depend::observe(target).ok();
    for operation in &plan.operations {
        let (subject, held, wanted): (String, Option<String>, Option<String>) = match operation {
            Operation::WriteFile { path, before, .. }
            | Operation::SpliceBlock { path, before, .. } => (
                path.clone(),
                landing::read_recorded(target, path)?.map(|bytes| Digest::of(&bytes).to_string()),
                before.as_ref().map(ToString::to_string),
            ),
            Operation::RemoveOwnedFile { path, before } => (
                path.clone(),
                read_optional(target, path)?.map(|bytes| Digest::of(&bytes).to_string()),
                Some(before.to_string()),
            ),
            Operation::WriteRecord { before, .. } => (
                manifest::MANIFEST_PATH.to_owned(),
                read_optional(target, manifest::MANIFEST_PATH)?
                    .map(|bytes| Digest::of(&bytes).to_string()),
                before.as_ref().map(ToString::to_string),
            ),
            Operation::UpdatePin {
                manager, before, ..
            } => (
                format!("the {manager} pin"),
                pin.as_ref().and_then(|observed| {
                    parse_manager(manager)
                        .and_then(|m| observed.entry(m))
                        .and_then(|entry| entry.version.clone())
                }),
                Some(before.clone()),
            ),
        };
        if held != wanted {
            moved.push(subject);
        }
    }
    if moved.is_empty() {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "these destinations changed since the plan observed them, and nothing was written: {}",
                moved.join(", ")
            ),
        )
        .expected("every destination holding what the plan's before digest names")
        .action("rk reconcile plan computes a fresh plan over what is there now")
        .target_state("unchanged"),
    ))
}

/// Execute one gated, revalidated plan: stage every operation, commit
/// the renames in order, run the postconditions, and journal the run.
///
/// # Errors
///
/// The gate's and the revalidation's refusals, a refusal for a
/// destination that moved, and [`RkError::Io`] for a staged write or a
/// rename that fails, whose message names every destination that landed
/// before it. A failed postcondition is not an error here: the report
/// carries it, and [`Applied::failure`] is the error the caller returns
/// after rendering.
pub fn run(
    target: &Utf8Path,
    stored: &Plan,
    blobs: &BTreeMap<Digest, Vec<u8>>,
    fresh: &Plan,
    mut journal: Option<Journal>,
) -> Result<Applied, RkError> {
    let outcome = execute(target, stored, blobs, fresh, journal.as_mut());
    if let Some(journal) = journal.as_mut() {
        match &outcome {
            Ok(applied) => {
                let failed = applied.failed();
                if failed.is_empty() {
                    journal.finish(0, None);
                } else {
                    journal.finish(1, Some(Reason::PostconditionFailed.as_str()));
                }
            }
            Err(error) => {
                journal.finish(i32::from(error.exit_code()), Some(error.reason().as_str()));
            }
        }
    }
    let run_id = journal.as_ref().map(|journal| journal.run_id().to_owned());
    outcome.map(|mut applied| {
        applied.run_id = run_id;
        applied
    })
}

fn execute(
    target: &Utf8Path,
    stored: &Plan,
    blobs: &BTreeMap<Digest, Vec<u8>>,
    fresh: &Plan,
    mut journal: Option<&mut Journal>,
) -> Result<Applied, RkError> {
    gate(stored)?;
    revalidate(stored, fresh)?;
    verify_before_digests(target, stored)?;
    if let Some(journal) = journal.as_deref_mut() {
        journal.event_line(&format!(
            r#"{{"event":"plan","plan_id":"{}","input_fingerprint":"{}","operations":{}}}"#,
            stored.identity.plan_id,
            stored.input_fingerprint,
            stored.operations.len()
        ));
    }
    let (txn, removals) = stage(target, stored, blobs)?;
    // The interruption proof's seam: a rename stopped on purpose.
    let stop = std::env::var_os("RK_APPLY_INTERRUPT_AT").map(std::path::PathBuf::from);
    let landed = txn
        .commit_stopping_at(stop.as_deref())
        .map_err(|interrupted| {
            if let Some(journal) = journal.as_deref_mut() {
                for path in &interrupted.landed {
                    journal.event_line(&format!(
                        r#"{{"event":"operation","path":"{}","status":"ok"}}"#,
                        path.display()
                    ));
                }
                journal.event_line(&format!(
                    r#"{{"event":"operation","path":"{}","status":"failed","detail":"{}"}}"#,
                    interrupted.failed.display(),
                    interrupted.error
                ));
            }
            interrupted_error(&interrupted)
        })?;
    for path in &removals {
        std::fs::remove_file(target.join(path))?;
    }
    debug_assert_eq!(landed.len(), txn_len(&stored.operations));
    let operations: Vec<OperationResult> = stored
        .operations
        .iter()
        .map(|operation| OperationResult {
            op: operation.kind(),
            path: match operation {
                Operation::WriteRecord { .. } => Some(manifest::MANIFEST_PATH.to_owned()),
                Operation::UpdatePin { manager, .. } => Some(format!("the {manager} pin")),
                other => other.path().map(str::to_owned),
            },
            status: "ok",
        })
        .collect();
    let postconditions = postconditions(target, stored);
    if let Some(journal) = journal {
        for result in &operations {
            journal.event_line(&format!(
                r#"{{"event":"operation","op":"{}","path":"{}","status":"{}"}}"#,
                result.op,
                result.path.as_deref().unwrap_or_default(),
                result.status
            ));
        }
        for result in &postconditions {
            journal.event_line(&format!(
                r#"{{"event":"postcondition","check":"{}","status":"{}"}}"#,
                result.check, result.status
            ));
        }
    }
    Ok(Applied {
        schema: APPLY_SCHEMA,
        plan_id: stored.identity.plan_id.clone(),
        input_fingerprint: stored.input_fingerprint.clone(),
        target: target.to_string(),
        classification: stored.classification,
        operations,
        postconditions,
        run_id: None,
        next: vec![
            "commit the written files, the record included".to_owned(),
            format!("rk status --target {target} reports the result"),
        ],
    })
}

/// Every write staged before any rename, so an unreadable destination
/// or a missing blob surfaces while the target is still untouched. The
/// removals come back beside the transaction, because a removal is not
/// a rename and runs after the commit.
fn stage(
    target: &Utf8Path,
    stored: &Plan,
    blobs: &BTreeMap<Digest, Vec<u8>>,
) -> Result<(crate::atomic::Transaction, Vec<String>), RkError> {
    let mut txn = crate::atomic::Transaction::new();
    let mut removals: Vec<String> = Vec::new();
    let pin = stored
        .operations
        .iter()
        .any(|operation| matches!(operation, Operation::UpdatePin { .. }))
        .then(|| crate::self_depend::observe(target))
        .transpose()?;
    for operation in &stored.operations {
        match operation {
            Operation::WriteFile { path, after, .. } => {
                txn.stage(target.join(path).as_std_path(), blob(blobs, after)?)?;
            }
            Operation::SpliceBlock { path, after, .. } => {
                let existing = read_optional(target, path)?
                    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
                let block = String::from_utf8_lossy(blob(blobs, after)?).into_owned();
                let spliced = if path == landing::HOOKS_DESTINATION {
                    landing::splice_hooks_block(existing.as_deref(), &block)
                        .map_err(std::io::Error::other)?
                } else {
                    landing::splice_agents_block(existing.as_deref(), &block)
                };
                txn.stage(target.join(path).as_std_path(), spliced.as_bytes())?;
            }
            Operation::RemoveOwnedFile { path, .. } => removals.push(path.clone()),
            Operation::WriteRecord { after, .. } => {
                txn.stage(
                    target.join(manifest::MANIFEST_PATH).as_std_path(),
                    blob(blobs, after)?,
                )?;
            }
            Operation::UpdatePin {
                manager,
                before,
                after,
            } => {
                let (file, text) = pin_text(pin.as_ref(), manager, before)?;
                let PinRead::One { line, .. } =
                    manager::read_pin(parse_manager(manager).unwrap_or(Manager::Mise), &text)
                else {
                    return Err(pin_moved(manager));
                };
                let rewritten = manager::rewrite_line(&text, line, before, after);
                txn.stage(target.join(&file).as_std_path(), rewritten.as_bytes())?;
            }
        }
    }
    Ok((txn, removals))
}

/// The failure of a commit that stopped part way, naming every
/// destination that landed before it, under the I/O kind that stopped it.
fn interrupted_error(interrupted: &crate::atomic::Interrupted) -> RkError {
    RkError::Io(std::io::Error::new(
        interrupted.error.kind(),
        format!(
            "the apply stopped at {}: {}; each destination holds its previous bytes or its new ones, and these landed before it: {}",
            interrupted.failed.display(),
            interrupted.error,
            if interrupted.landed.is_empty() {
                "none".to_owned()
            } else {
                interrupted
                    .landed
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ),
    ))
}

/// How many staged renames the operations produce.
fn txn_len(operations: &[Operation]) -> usize {
    operations
        .iter()
        .filter(|operation| !matches!(operation, Operation::RemoveOwnedFile { .. }))
        .count()
}

/// Run every postcondition the plan carries and report each outcome.
#[must_use]
pub fn postconditions(target: &Utf8Path, plan: &Plan) -> Vec<PostconditionResult> {
    let pin = plan
        .postconditions
        .iter()
        .any(|check| matches!(check, Postcondition::PinReads { .. }))
        .then(|| crate::self_depend::observe(target).ok())
        .flatten();
    plan.postconditions
        .iter()
        .map(|check| {
            let (name, outcome): (String, Result<(), String>) = match check {
                Postcondition::RecordReadsBack { sha256 } => (
                    "record-reads-back".to_owned(),
                    holds(read_optional(target, manifest::MANIFEST_PATH), sha256),
                ),
                Postcondition::DestinationHolds { path, sha256 } => (
                    format!("destination-holds:{path}"),
                    holds(
                        landing::read_recorded(target, path).map_err(RkError::Io),
                        sha256,
                    ),
                ),
                Postcondition::StatusCheckClean => {
                    ("status-check-clean".to_owned(), status_check(target))
                }
                Postcondition::PinReads { manager, version } => (
                    format!("pin-reads:{manager}"),
                    match pin
                        .as_ref()
                        .and_then(|observed| parse_manager(manager).and_then(|m| observed.entry(m)))
                        .and_then(|entry| entry.version.clone())
                    {
                        Some(found) if &found == version => Ok(()),
                        Some(found) => Err(format!("the {manager} pin reads {found}")),
                        None => Err(format!("no {manager} pin reads")),
                    },
                ),
            };
            // The status check judges the whole target, sentinels the
            // operator still owes included, so its answer is reported and
            // never fails an apply whose own writes landed as promised.
            let advisory = matches!(check, Postcondition::StatusCheckClean);
            match outcome {
                Ok(()) => PostconditionResult {
                    check: name,
                    status: "ok",
                    detail: None,
                },
                Err(detail) => PostconditionResult {
                    check: name,
                    status: if advisory { "reported" } else { "failed" },
                    detail: Some(detail),
                },
            }
        })
        .collect()
}

/// Whether the bytes read digest to what the plan promised.
fn holds(read: Result<Option<Vec<u8>>, RkError>, wanted: &Digest) -> Result<(), String> {
    match read {
        Ok(Some(bytes)) if Digest::of(&bytes) == *wanted => Ok(()),
        Ok(Some(bytes)) => Err(format!("holds {}", Digest::of(&bytes))),
        Ok(None) => Err("absent".to_owned()),
        Err(error) => Err(error.to_string()),
    }
}

/// `rk status --check` as the standing postcondition: this binary run
/// against the target, judged by its exit code.
fn status_check(target: &Utf8Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| format!("no engine path: {error}"))?;
    let mut command = std::process::Command::new(exe);
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .args(["status", "--check", "--target"])
        .arg(target)
        .output()
        .map_err(|error| format!("rk status --check could not run: {error}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(stderr
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("rk status --check exited nonzero")
        .trim()
        .to_owned())
}

fn blob<'a>(blobs: &'a BTreeMap<Digest, Vec<u8>>, digest: &Digest) -> Result<&'a [u8], RkError> {
    blobs.get(digest).map(Vec::as_slice).ok_or_else(|| {
        RkError::Other(anyhow::anyhow!(
            "the plan names digest {digest} and its store holds no such blob"
        ))
    })
}

fn read_optional(target: &Utf8Path, rel: &str) -> Result<Option<Vec<u8>>, RkError> {
    match std::fs::read(target.join(rel)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RkError::Io(error)),
    }
}

fn parse_manager(name: &str) -> Option<Manager> {
    Manager::ALL.into_iter().find(|m| m.as_str() == name)
}

/// The wired manager's file and text, for the one-fact rewrite the
/// apply stages; the flake pair is never moved here.
fn pin_text(
    observed: Option<&crate::self_depend::Observed>,
    manager: &str,
    before: &str,
) -> Result<(String, String), RkError> {
    let parsed = parse_manager(manager).ok_or_else(|| pin_moved(manager))?;
    if parsed == Manager::Flake {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                "the flake pin moves through the self-depend sync verb under --apply, with nix and the network; an offline apply cannot stage it",
            )
            .expected("a one-fact manager pin, or the sync verb for the flake pair")
            .target_state("unchanged"),
        ));
    }
    let entry = observed
        .and_then(|observed| observed.entry(parsed))
        .filter(|entry| entry.version.as_deref() == Some(before))
        .ok_or_else(|| pin_moved(manager))?;
    match (&entry.file, &entry.text) {
        (Some(file), Some(text)) => Ok((file.clone(), text.clone())),
        _ => Err(pin_moved(manager)),
    }
}

fn pin_moved(manager: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "the {manager} pin no longer reads as the plan observed it, and nothing was written"
            ),
        )
        .expected("the manager file as the plan observed it")
        .action("rk reconcile plan computes a fresh plan over what is there now")
        .target_state("unchanged"),
    )
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::{PostconditionResult, postconditions};
    use crate::digest::Digest;
    use crate::plan::{Plan, Postcondition};

    /// A plan whose postconditions run against a scratch target.
    fn plan_with(checks: &[Postcondition]) -> Plan {
        let json = serde_json::json!({
            "schema": crate::plan::PLAN_SCHEMA,
            "identity": {"plan_id": "0123456789abcdef", "created_at": "2026-01-01T00:00:00Z", "engine_version": "0.0.0"},
            "classification": "setup",
            "findings": [],
            "desired_state": {"intent": "reconcile", "selector": "embedded", "release": {"version": "0.0.0", "venue": "embedded", "payload_sha256": Digest::of(b"a").to_string(), "payload_schema": 1}},
            "observed_state": {
                "repository": {"target": "/tmp/t", "git": false, "tags": 0, "long_lived_branches": [], "release_markers": [], "collisions": [], "verdict": "greenfield", "evidence_refs": []},
                "installation": {"record": {"state": "absent"}, "configuration": {"present": false, "pending": []}, "destinations": [], "evidence_refs": []},
                "host": {"engine_version": "0.0.0", "evidence_refs": []},
                "forge": {"state": "not-observed", "reason": "not requested"}
            },
            "release": {"candidate": {"version": "0.0.0", "payload_sha256": Digest::of(b"a").to_string(), "payload_schema": 1, "artifacts": 0, "evidence_refs": []}, "verification": {"method": "embedded"}, "baseline": {"state": "not-needed"}, "compatibility": {"engine_schema": 1, "bundle_schema": 1, "readable": true, "intermediate": [], "evidence_refs": []}, "guidance": {"coverage": {"state": "not-needed"}, "steps": [], "excluded": 0, "evidence_refs": []}},
            "operations": [],
            "preconditions": [],
            "decisions": [],
            "postconditions": checks,
            "evidence": [],
            "readiness": "ready",
            "input_fingerprint": Digest::of(b"a").to_string()
        });
        serde_json::from_value(json).expect("a plan deserializes")
    }

    /// Every postcondition runs and reports; a destination holding other
    /// bytes than promised is reported failed with what it holds, and
    /// the check beside it still reports ok.
    #[test]
    fn postconditions_run_and_a_failure_is_reported() {
        let dir = tempfile::tempdir().expect("a scratch target exists");
        let target = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8");
        std::fs::write(target.join("SECURITY.md"), b"held").expect("writes");
        let plan = plan_with(&[
            Postcondition::DestinationHolds {
                path: "SECURITY.md".into(),
                sha256: Digest::of(b"held"),
            },
            Postcondition::DestinationHolds {
                path: "SECURITY.md".into(),
                sha256: Digest::of(b"promised"),
            },
            Postcondition::RecordReadsBack {
                sha256: Digest::of(b"record"),
            },
        ]);
        let results: Vec<PostconditionResult> = postconditions(&target, &plan);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, "ok");
        assert_eq!(results[1].status, "failed");
        assert_eq!(
            results[1].detail.as_deref(),
            Some(format!("holds {}", Digest::of(b"held")).as_str())
        );
        assert_eq!(results[2].status, "failed");
        assert_eq!(results[2].detail.as_deref(), Some("absent"));
        let applied = super::Applied {
            schema: super::APPLY_SCHEMA,
            plan_id: "0123456789abcdef".into(),
            input_fingerprint: Digest::of(b"a"),
            target: target.to_string(),
            classification: crate::plan::Classification::Setup,
            operations: vec![],
            postconditions: results,
            run_id: None,
            next: vec![],
        };
        let failure = applied
            .failure()
            .expect("a failed postcondition is a failure");
        assert_eq!(failure.exit_code(), 1);
        assert_eq!(
            failure.reason(),
            crate::diagnostic::Reason::PostconditionFailed
        );
    }

    /// The apply report's shape, held by snapshot.
    #[test]
    fn the_apply_report_schema_snapshot_holds() {
        let applied = super::Applied {
            schema: super::APPLY_SCHEMA,
            plan_id: "0123456789abcdef".into(),
            input_fingerprint: Digest::of(b"a"),
            target: "/tmp/t".into(),
            classification: crate::plan::Classification::Upgrade,
            operations: vec![super::OperationResult {
                op: "write-record",
                path: Some(".release-kit/manifest.json".into()),
                status: "ok",
            }],
            postconditions: vec![PostconditionResult {
                check: "record-reads-back".into(),
                status: "failed",
                detail: Some("absent".into()),
            }],
            run_id: Some("run".into()),
            next: vec!["commit the written files, the record included".into()],
        };
        assert_eq!(
            serde_json::to_string(&applied).expect("serializes"),
            format!(
                r#"{{"schema":"rk.reconcile-apply/1","plan_id":"0123456789abcdef","input_fingerprint":"{}","target":"/tmp/t","classification":"upgrade","operations":[{{"op":"write-record","path":".release-kit/manifest.json","status":"ok"}}],"postconditions":[{{"check":"record-reads-back","status":"failed","detail":"absent"}}],"run_id":"run","next":["commit the written files, the record included"]}}"#,
                Digest::of(b"a")
            )
        );
    }
}
