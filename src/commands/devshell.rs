//! `rk devshell status | add | clean | sync`: the consumer half of the
//! release-kit flake.
//!
//! The producer half already ships: the flake at every tag, and the
//! landed package expression. This handler serves a consumer that pins
//! that flake as a devshell input. `status` is the offline reporter and
//! fetches nothing; `add` serves the fragments and seeds the pair where a
//! target has neither file, never editing a file the target owns;
//! `clean` removes what a predecessor mechanism left and names the rest,
//! so the wiring is a replacement and never an addition; `sync` moves
//! the pin to the latest release inside a fenced transaction, both files
//! or neither. Every report goes through the output boundary with a
//! versioned schema.

use serde::Serialize;

use camino::Utf8Path;

use crate::cli::devshell::{
    AddArgs, Caller, CleanArgs, DevshellAction, DevshellArgs, StatusArgs, SyncArgs,
};
use crate::devshell::discover::{self, Discovery};
use crate::devshell::fragments::{self, Fragment};
use crate::devshell::guard::{self, Acquired};
use crate::devshell::leftovers::{self, Action, Leftover};
use crate::devshell::txn::{self, AbortFailure, Recovery, StepFailure};
use crate::devshell::{self, Observed, Presence, pin};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::output::Output;
use crate::probes::{self, ProbeStatus};

/// The `rk.devshell-status/1` document.
#[derive(Debug, Serialize)]
struct StatusReport<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// The target, canonical.
    target: &'a str,
    /// The rollup: `ready`, `superseded`, `no-flake`, `not-wired`,
    /// `unpinned`, `ambiguous-pin`, or `pending-recovery`.
    state: &'static str,
    /// Whether `flake.nix` exists.
    flake: Presence,
    /// Whether `flake.lock` exists.
    lock: Presence,
    /// `pinned`, `unpinned`, `absent`, or `ambiguous`.
    input: &'static str,
    /// The pinned tag, where exactly one pin was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pin_tag: Option<&'a str>,
    /// How many lines name the input, where any does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pin_lines: Option<usize>,
    /// The locked ref of the input, where the lock names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    locked_ref: Option<&'a str>,
    /// The locked commit of the input, where the lock names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    locked_rev: Option<&'a str>,
    /// Whether `.envrc` exists.
    envrc: Presence,
    /// Whether `.envrc` carries the sync line.
    envrc_sync: bool,
    /// The day of the last sync attempt, where stamped.
    #[serde(skip_serializing_if = "Option::is_none")]
    stamp: Option<&'a str>,
    /// Whether an interrupted transaction awaits recovery.
    pending: bool,
    /// The two host probes the sync depends on.
    host: Host,
    /// What a predecessor bump mechanism left, whatever the state.
    leftovers: &'a [Leftover],
    /// What plausibly follows.
    next: &'a [String],
}

/// The `rk.devshell-add/1` document.
#[derive(Debug, Serialize)]
struct AddReport<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target, canonical.
    target: &'a str,
    /// The tag the fragments pin.
    tag: &'a str,
    /// `binary` or `argument`: where the tag came from.
    tag_source: &'static str,
    /// Whether `flake.nix` existed before the run.
    flake: Presence,
    /// Whether `.envrc` existed before the run.
    envrc: Presence,
    /// The seed files this run wrote, relative to the target; empty in
    /// preview.
    written: &'a [String],
    /// Why an owned file was refused, where one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<&'a str>,
    /// The four fragments, in application order.
    fragments: &'a [Fragment],
    /// What plausibly follows.
    next: &'a [String],
}

/// The `rk.devshell-clean/1` document.
#[derive(Debug, Serialize)]
struct CleanReport<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target, canonical.
    target: &'a str,
    /// Every leftover the scan found before the run, in catalog order,
    /// plus one `also` row per `--also` path.
    leftovers: &'a [Leftover],
    /// The files this run removed, relative to the target; empty in preview.
    removed: &'a [String],
    /// The files this run rewrote; empty in preview.
    rewritten: &'a [String],
    /// What this run left in place by design, for a hand edit; empty in
    /// preview.
    manual: &'a [Manual],
    /// What plausibly follows.
    next: &'a [String],
}

/// One leftover the cleanup names and does not touch.
#[derive(Debug, Clone, Serialize)]
struct Manual {
    /// The catalog entry.
    id: &'static str,
    /// The file, relative to the target.
    file: String,
    /// The one-based line, where the entry is a line.
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    /// The matched line, trimmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// Why a line scan must not touch it.
    reason: &'static str,
}

/// The `rk.devshell-sync/1` document.
#[derive(Debug, Serialize)]
struct SyncReport<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// `envrc` or `operator`.
    caller: &'static str,
    /// The target, canonical.
    target: &'a str,
    /// The closed outcome vocabulary: `bumped`, `current`, `ahead`,
    /// `would-bump`, `pending-recovery`, `recovery-failed`, `skipped-ci`,
    /// `skipped-disabled`, `skipped-stamped`, `skipped-locked`,
    /// `lock-unavailable`, `not-wired`, `unpinned`, `no-flake`,
    /// `ambiguous-pin`, `refused-dirty`, `unreachable`, `unparsable`,
    /// `update-failed`, `build-failed`, `restore-failed`, or
    /// `cleanup-failed`.
    outcome: &'static str,
    /// The pinned tag before the run, where the pin was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<&'a str>,
    /// The tag the run moved to or would move to, where one was resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<&'a str>,
    /// One line of detail for an outcome that has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a str>,
    /// The transaction's steps, once it opened.
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<&'a [Step]>,
    /// The files a failure rolled back.
    #[serde(skip_serializing_if = "Option::is_none")]
    restored: Option<&'a [String]>,
    /// The files an earlier interrupted run left half-moved, restored
    /// before this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    recovered: Option<&'a [String]>,
    /// The day this checkout's last attempt was stamped.
    #[serde(skip_serializing_if = "Option::is_none")]
    stamp: Option<&'a str>,
    /// What plausibly follows.
    next: &'a [String],
}

/// One transaction step.
#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_field_names)]
struct Step {
    /// `rewrite-pin`, `flake-update`, `current-system`, or `build`.
    step: &'static str,
    /// `ok` or `failed`.
    status: &'static str,
    /// The child's last stderr line, for a failed step.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// Everything one sync run decided, before rendering.
#[derive(Debug, Default)]
struct SyncRun {
    outcome: &'static str,
    from: Option<String>,
    to: Option<String>,
    detail: Option<String>,
    steps: Option<Vec<Step>>,
    restored: Option<Vec<String>>,
    recovered: Option<Vec<String>>,
    stamp: Option<String>,
}

/// The two Soft probes the sync spawns, as `ok` or `failed`.
#[derive(Debug, Serialize)]
struct Host {
    /// Whether `nix` answers.
    nix: &'static str,
    /// Whether `direnv` answers.
    direnv: &'static str,
}

/// Dispatch the devshell action.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory,
/// [`RkError::Io`] where a present file does not read, and
/// [`RkError::Usage`] for an action this build does not carry yet.
pub fn run(args: &DevshellArgs) -> Result<(), RkError> {
    match &args.action {
        DevshellAction::Status(args) => status(args),
        DevshellAction::Add(args) => add(args),
        DevshellAction::Clean(args) => clean(args),
        DevshellAction::Sync(args) => sync(args),
    }
}

/// Report the wiring, offline.
fn status(args: &StatusArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let observed = devshell::observe(&args.target)?;
    let host = Host {
        nix: probe_word(&probes::nix()),
        direnv: probe_word(&probes::direnv()),
    };
    let state = observed.state();
    out.result_line(format!("state {state}"));
    out.result_line(format!(
        "flake {}, lock {}",
        word(observed.flake),
        word(observed.lock)
    ));
    out.result_line(input_line(&observed));
    out.result_line(format!(
        ".envrc {}, sync line {}",
        word(observed.envrc),
        if observed.envrc_sync { "yes" } else { "no" }
    ));
    if let Some(stamp) = &observed.stamp {
        out.result_line(format!("last sync attempt {stamp}"));
    }
    if observed.pending {
        out.result_line("an interrupted sync awaits recovery");
    }
    out.result_line(format!("host nix {}, direnv {}", host.nix, host.direnv));
    for leftover in &observed.leftovers {
        out.result_line(leftover_line(leftover));
    }
    let next = status_next(&observed);
    out.next(&next);
    out.emit(&StatusReport {
        schema: "rk.devshell-status/1",
        target: observed.target.as_str(),
        state,
        flake: observed.flake,
        lock: observed.lock,
        input: input_word(&observed.scan),
        pin_tag: observed.pin_tag(),
        pin_lines: pin_lines(&observed.scan),
        locked_ref: observed.locked_ref.as_deref(),
        locked_rev: observed.locked_rev.as_deref(),
        envrc: observed.envrc,
        envrc_sync: observed.envrc_sync,
        stamp: observed.stamp.as_deref(),
        pending: observed.pending,
        host,
        leftovers: &observed.leftovers,
        next: &next,
    })
}

/// The one human line for a leftover.
fn leftover_line(leftover: &Leftover) -> String {
    use std::fmt::Write as _;
    let action = match leftover.action {
        Action::RemoveFile => "remove-file",
        Action::ReplaceLine => "replace-line",
        Action::Manual => "manual",
    };
    let mut line = format!("leftover {action} {}", leftover.file);
    if let Some(number) = leftover.line {
        let _ = write!(line, ":{number}");
    }
    if let Some(text) = &leftover.text {
        let _ = write!(line, " {text}");
    }
    let _ = write!(line, " ({}: {})", leftover.id, leftover.reason);
    line
}

/// Serve the fragments; seed the files a target lacks under `--apply`.
fn add(args: &AddArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let observed = devshell::observe(&args.target)?;
    let (tag, tag_source) = resolve_tag(args.tag.as_deref())?;
    let fragments = fragments::fragments(&tag, &observed);
    let mode = if args.apply { "apply" } else { "preview" };
    let mut written = Vec::new();
    let mut owned = Vec::new();
    if args.apply {
        for (name, present, seed) in [
            ("flake.nix", observed.flake, fragments::seed_flake(&tag)),
            (".envrc", observed.envrc, fragments::seed_envrc()),
        ] {
            if present.is_present() {
                owned.push(name);
            } else {
                crate::atomic::write(observed.target.join(name).as_std_path(), seed.as_bytes())?;
                written.push(name.to_owned());
            }
        }
    }
    let refusal = (!owned.is_empty()).then(|| {
        format!(
            "the target already carries {}; rk devshell add never edits a file the target owns",
            owned.join(" and ")
        )
    });
    if args.apply {
        for name in &written {
            out.result_line(format!("wrote {name}"));
        }
    } else {
        out.result_line("DRY RUN: rk devshell add prints the fragments; --apply seeds only the files the target lacks");
    }
    out.result_line(format!("tag {tag} (from the {tag_source})"));
    for (name, present) in [("flake.nix", observed.flake), (".envrc", observed.envrc)] {
        out.result_line(match present {
            Presence::Present => {
                format!("{name} present: the target owns it, so its fragments are applied by hand")
            }
            Presence::Absent => format!("{name} absent: --apply seeds it"),
        });
    }
    for fragment in &fragments {
        out.result_line(format!(
            "--- {} into {} ({} at {}){}",
            fragment.id,
            fragment.file,
            fragment.placement,
            fragment.anchor.path,
            match fragment.present {
                Some(true) => ": already present",
                Some(false) => ": missing",
                None => ": not judged",
            }
        ));
        out.result_line(&fragment.text);
    }
    let next = add_next(&observed, args.apply, &written);
    out.next(&next);
    out.emit(&AddReport {
        schema: "rk.devshell-add/1",
        mode,
        target: observed.target.as_str(),
        tag: &tag,
        tag_source,
        flake: observed.flake,
        envrc: observed.envrc,
        written: &written,
        refusal: refusal.as_deref(),
        fragments: &fragments,
        next: &next,
    })?;
    let Some(message) = refusal else {
        return Ok(());
    };
    let state = if written.is_empty() {
        "nothing was written".to_owned()
    } else {
        format!(
            "wrote {}; the owned file is byte-identical",
            written.join(", ")
        )
    };
    Err(RkError::refusal(
        Diagnostic::new(Reason::DestructiveRefusal, message)
            .expected("a target with no flake.nix and no .envrc, or the fragments applied by hand")
            .target_state(state),
    ))
}

/// Move the pin forward, both files or neither, behind the gates.
///
/// The gate order is the design: the off switches first, so CI and a
/// switched-off shell fetch nothing and spawn nothing; then the recovery
/// of an interrupted run; then the daily stamp, which under the `.envrc`
/// caller ends a stamped day before the lock, the fetch, and nix; then
/// the lock; then the observation and the dirty check; then the decision.
fn sync(args: &SyncArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let mut observed = devshell::observe(&args.target)?;
    let key = observed.key();
    let mut run = SyncRun {
        stamp: observed.stamp.clone(),
        ..SyncRun::default()
    };
    let mut held = None;
    if guard::switched_off() {
        run.outcome = "skipped-disabled";
        run.detail = Some(format!("{}=0 is set", guard::SWITCH_VAR));
    } else if guard::in_ci() {
        run.outcome = "skipped-ci";
        run.detail = Some("a CI variable is set; the sync never runs on a runner".to_owned());
    } else {
        gate_and_decide(args, &mut observed, &key, &mut run, &mut held, out)?;
    }
    render_sync(out, args, &observed, &run)?;
    drop(held);
    exit_for(args.caller, &run)
}

/// The stamp, the lock, the recovery, the dirty check, and the
/// decision, in that order; `held` keeps the lock alive until the report
/// is rendered. The recovery runs under the lock, so two entries never
/// restore the same marker at once.
fn gate_and_decide(
    args: &SyncArgs,
    observed: &mut Observed,
    key: &str,
    run: &mut SyncRun,
    held: &mut Option<guard::Lock>,
    out: Output,
) -> Result<(), RkError> {
    let envrc = args.caller == Caller::Envrc;
    let today = guard::today();
    // A pending marker outranks the stamp: an interrupted run recovers on
    // the next entry, not on the next day.
    if args.apply && envrc && !observed.pending && observed.stamp.as_deref() == Some(today.as_str())
    {
        run.outcome = "skipped-stamped";
        run.detail = Some(format!("today's attempt already happened ({today})"));
        return Ok(());
    }
    if args.apply && envrc {
        // A stamp that cannot be written is not the run's failure: the
        // lock under the same root answers for the broken state root.
        if guard::write_stamp(key).is_ok() {
            run.stamp = Some(today);
        }
    }
    if args.apply {
        match guard::acquire(key) {
            Acquired::Held(lock) => *held = Some(lock),
            Acquired::Contended => {
                run.outcome = "skipped-locked";
                run.detail = Some("another run holds this checkout".to_owned());
                return Ok(());
            }
            Acquired::Unavailable(source) => {
                run.outcome = "lock-unavailable";
                run.detail = Some(format!("the lock cannot be taken: {source}"));
                out.warn(format!(
                    "rk devshell sync: the lock cannot be taken: {source}"
                ));
                return Ok(());
            }
        }
    }
    if args.apply {
        match txn::recover_pending(&observed.target, key)? {
            Some(Recovery::Restored(restored)) => {
                run.recovered = Some(restored);
                *observed = devshell::observe(&args.target)?;
            }
            Some(Recovery::Failed(failure)) => {
                run.outcome = "recovery-failed";
                run.detail = Some(format!(
                    "{failure}; the backups stay under the state root for the next attempt"
                ));
                return Ok(());
            }
            Some(Recovery::Unfinished(failure)) => {
                run.outcome = "cleanup-failed";
                run.detail = Some(format!("both files are back, but {failure}"));
                return Ok(());
            }
            Some(Recovery::Finished) => {
                *observed = devshell::observe(&args.target)?;
            }
            None => {}
        }
    }
    if observed.pending {
        return decide(args, observed, key, run);
    }
    if observed.flake.is_present() && guard::two_files_dirty(&observed.target) {
        run.outcome = "refused-dirty";
        run.from = observed.pin_tag().map(str::to_owned);
        run.detail = Some(
            "flake.nix or flake.lock carries uncommitted edits; commit or stash them first"
                .to_owned(),
        );
        return Ok(());
    }
    decide(args, observed, key, run)
}

/// The decision sequence: the wiring, the target tag, the comparison,
/// and — under `--apply` on a behind pin — the transaction.
fn decide(
    args: &SyncArgs,
    observed: &Observed,
    key: &str,
    run: &mut SyncRun,
) -> Result<(), RkError> {
    if observed.pending {
        run.outcome = "pending-recovery";
        run.detail =
            Some("an interrupted run left its marker; --apply recovers it first".to_owned());
        return Ok(());
    }
    if !observed.flake.is_present() {
        run.outcome = "no-flake";
        return Ok(());
    }
    let pin = match &observed.scan {
        pin::Scan::Many(count) => {
            run.outcome = "ambiguous-pin";
            run.detail = Some(format!(
                "{count} lines name the release-kit input in flake.nix"
            ));
            return Ok(());
        }
        pin::Scan::None => {
            run.outcome = "not-wired";
            return Ok(());
        }
        pin::Scan::Unpinned(line) => {
            run.outcome = "unpinned";
            run.detail = Some(format!("flake.nix line {line} names the input with no tag"));
            return Ok(());
        }
        pin::Scan::One(pin) => pin,
    };
    run.from = Some(pin.tag.clone());
    let to = match args.tag.as_deref() {
        Some(raw) => devshell::normalize_tag(raw).ok_or_else(|| {
            RkError::Usage(format!(
                "--tag {raw} is not a release tag; pass v0.2.16, 0.2.16, or the release URL"
            ))
        })?,
        None => match discover::latest_tag() {
            Discovery::Tag(tag) => tag,
            Discovery::Unreachable(detail) => {
                run.outcome = "unreachable";
                run.detail = Some(detail);
                return Ok(());
            }
            Discovery::Unparsable(answer) => {
                run.outcome = "unparsable";
                run.detail = Some(format!("the release page answered no tag: {answer}"));
                return Ok(());
            }
        },
    };
    run.to = Some(to.clone());
    match discover::version_order(&pin.tag, &to) {
        std::cmp::Ordering::Equal => {
            run.outcome = "current";
            return Ok(());
        }
        // A discovered tag moves the pin forward only; an explicit --tag is
        // the operator's deliberate choice and pins in either direction.
        std::cmp::Ordering::Greater if args.tag.is_none() => {
            run.outcome = "ahead";
            run.detail = Some(
                "the pin is ahead of the latest release and is never moved backward".to_owned(),
            );
            return Ok(());
        }
        std::cmp::Ordering::Greater | std::cmp::Ordering::Less => {}
    }
    if !args.apply {
        run.outcome = "would-bump";
        return Ok(());
    }
    apply_bump(observed, key, pin, &to, run)
}

/// Rewrite, refresh, and build inside one transaction; a failure
/// restores both files, and a restore that cannot is its own outcome.
fn apply_bump(
    observed: &Observed,
    key: &str,
    pin: &pin::Pin,
    to: &str,
    run: &mut SyncRun,
) -> Result<(), RkError> {
    let flake_text = observed.flake_text.as_deref().unwrap_or_default();
    let rewritten = pin::rewrite(flake_text, pin, to);
    let transaction = txn::open(&observed.target, key)?;
    let mut steps = Vec::new();
    let failure = transact(&observed.target, &rewritten, &mut steps);
    match failure {
        None => {
            run.outcome = "bumped";
            if let Err(failure) = transaction.commit() {
                run.outcome = "cleanup-failed";
                run.detail = Some(format!("the pin moved, but {failure}"));
            }
        }
        Some(failed) => {
            run.outcome = match failed.step {
                "rewrite-pin" | "flake-update" => "update-failed",
                _ => "build-failed",
            };
            match transaction.abort() {
                Ok(restored) => {
                    run.restored = Some(restored);
                    run.detail = Some(format!("{} failed: {}", failed.step, failed.detail));
                }
                Err(AbortFailure::Restore(failure)) => {
                    run.outcome = "restore-failed";
                    run.detail = Some(format!(
                        "{} failed: {}; then {failure}; the backups stay under the state root for the next run",
                        failed.step, failed.detail
                    ));
                }
                Err(AbortFailure::Finish(failure)) => {
                    run.outcome = "cleanup-failed";
                    run.detail = Some(format!(
                        "{} failed: {}; both files are back, but {failure}",
                        failed.step, failed.detail
                    ));
                }
            }
        }
    }
    run.steps = Some(steps);
    Ok(())
}

/// One step's work, boxed so the three steps sit in one ordered table.
type Attempt<'a> = Box<dyn Fn() -> Result<(), StepFailure> + 'a>;

/// The ordered steps inside an open transaction; the first failure ends
/// the sequence and is returned.
fn transact(target: &Utf8Path, rewritten: &str, steps: &mut Vec<Step>) -> Option<StepFailure> {
    let attempts: [(&'static str, Attempt<'_>); 3] = [
        (
            "rewrite-pin",
            Box::new(|| {
                crate::atomic::write(target.join("flake.nix").as_std_path(), rewritten.as_bytes())
                    .map_err(|source| StepFailure {
                        step: "rewrite-pin",
                        detail: source.to_string(),
                    })
            }),
        ),
        ("flake-update", Box::new(|| txn::flake_update(target))),
        (
            "build",
            Box::new(|| {
                let system = txn::current_system(target)?;
                txn::build_devshell(target, &system)
            }),
        ),
    ];
    for (name, attempt) in attempts {
        match attempt() {
            Ok(()) => steps.push(Step {
                step: name,
                status: "ok",
                detail: None,
            }),
            Err(failed) => {
                steps.push(Step {
                    step: failed.step,
                    status: "failed",
                    detail: Some(failed.detail.clone()),
                });
                return Some(failed);
            }
        }
    }
    None
}

/// Whether an outcome is "nothing to do", which the `.envrc` caller
/// reports in silence.
const fn is_quiet(outcome: &str) -> bool {
    matches!(
        outcome.as_bytes(),
        b"current"
            | b"ahead"
            | b"no-flake"
            | b"not-wired"
            | b"unpinned"
            | b"skipped-ci"
            | b"skipped-disabled"
            | b"skipped-stamped"
            | b"skipped-locked"
    )
}

/// Render the human lines and emit the document.
fn render_sync(
    out: Output,
    args: &SyncArgs,
    observed: &Observed,
    run: &SyncRun,
) -> Result<(), RkError> {
    use std::fmt::Write as _;
    let quiet = args.caller == Caller::Envrc && is_quiet(run.outcome);
    if let Some(recovered) = &run.recovered {
        out.result_line(format!(
            "recovered an interrupted sync: restored {}",
            recovered.join(", ")
        ));
    }
    if !quiet {
        out.result_line(sync_line(run));
        if let Some(steps) = &run.steps {
            for step in steps {
                let mut line = format!("  {} {}", step.status, step.step);
                if let Some(detail) = &step.detail {
                    let _ = write!(line, ": {detail}");
                }
                out.result_line(line);
            }
        }
        if let Some(restored) = &run.restored {
            out.result_line(format!("restored {}", restored.join(", ")));
        }
    }
    let next = if quiet {
        Vec::new()
    } else {
        sync_next(observed, run)
    };
    out.next(&next);
    out.emit(&SyncReport {
        schema: "rk.devshell-sync/1",
        mode: if args.apply { "apply" } else { "preview" },
        caller: match args.caller {
            Caller::Envrc => "envrc",
            Caller::Operator => "operator",
        },
        target: observed.target.as_str(),
        outcome: run.outcome,
        from: run.from.as_deref(),
        to: run.to.as_deref(),
        detail: run.detail.as_deref(),
        steps: run.steps.as_deref(),
        restored: run.restored.as_deref(),
        recovered: run.recovered.as_deref(),
        stamp: run.stamp.as_deref(),
        next: &next,
    })
}

/// The one human line for an outcome.
fn sync_line(run: &SyncRun) -> String {
    use std::fmt::Write as _;
    let movement = match (&run.from, &run.to) {
        (Some(from), Some(to)) if from != to => format!(" {from} -> {to}"),
        (Some(from), _) => format!(" {from}"),
        _ => String::new(),
    };
    let mut line = format!("{}{movement}", run.outcome);
    if let Some(detail) = &run.detail {
        let _ = write!(line, ": {detail}");
    }
    line
}

/// What plausibly follows a sync.
fn sync_next(observed: &Observed, run: &SyncRun) -> Vec<String> {
    let target = &observed.target;
    let mut next = match run.outcome {
        "bumped" => vec![
            format!(
                "git -C {target} diff -- flake.nix flake.lock shows the two-file change to review and commit"
            ),
            "the next direnv reload takes the new rk; nothing here commits".to_owned(),
        ],
        "would-bump" => vec![format!(
            "rk devshell sync --caller operator --apply --target {target} moves the pin, locks it, and proves the build"
        )],
        "current" => vec![format!(
            "rk devshell status --target {target} reports the wiring"
        )],
        "ahead" => vec![
            "a pin past the latest release is a deliberate state; nothing moves it back".to_owned(),
        ],
        "pending-recovery" => vec![format!(
            "rk devshell sync --caller operator --apply --target {target} restores both files first"
        )],
        "no-flake" | "not-wired" | "unpinned" => vec![format!(
            "rk devshell add --target {target} prints the fragments; --apply seeds the files a target lacks"
        )],
        "ambiguous-pin" => vec![format!(
            "leave exactly one release-kit input line in {target}/flake.nix, then rerun"
        )],
        "refused-dirty" => vec![format!(
            "git -C {target} status -- flake.nix flake.lock names the edits; commit or stash them, then rerun"
        )],
        "skipped-disabled" => vec![format!(
            "unset {} to let the sync run again",
            guard::SWITCH_VAR
        )],
        "skipped-stamped" => vec![format!(
            "rk devshell sync --caller operator --apply --target {target} runs the attempt now, whatever the stamp says"
        )],
        "skipped-locked" => vec!["let the other run finish; nothing here is owed".to_owned()],
        "lock-unavailable" => vec!["make the state root writable: rk doctor reports it".to_owned()],
        "unreachable" | "unparsable" => vec![
            "retry when the release page answers; --tag <TAG> makes no request at all".to_owned(),
        ],
        "cleanup-failed" => vec![
            "remove the named marker by hand before the next entry; an active marker beside its backups would overwrite later edits".to_owned(),
        ],
        "recovery-failed" | "restore-failed" => vec![
            "a file is not back: free the path the detail names, then rerun; the backups wait under the state root".to_owned(),
        ],
        "update-failed" | "build-failed" => vec![
            "both files are as they were; the failing step's last line is above".to_owned(),
            format!(
                "rk devshell sync --caller operator --apply --target {target} retries after the fix"
            ),
        ],
        _ => Vec::new(),
    };
    if !observed.leftovers.is_empty() {
        next.push(format!(
            "rk devshell clean --target {target}: the target still carries a predecessor bump mechanism"
        ));
    }
    next
}

/// The exit for a finished run: the `.envrc` caller exits 0 on every
/// reported outcome, and the operator caller takes the matrix.
fn exit_for(caller: Caller, run: &SyncRun) -> Result<(), RkError> {
    if caller == Caller::Envrc {
        return Ok(());
    }
    let detail = run.detail.clone().unwrap_or_default();
    match run.outcome {
        "ambiguous-pin" | "refused-dirty" => Err(RkError::refusal(
            Diagnostic::new(Reason::StateDrift, detail)
                .expected("exactly one committed pin line in flake.nix")
                .target_state("nothing was written"),
        )),
        "unreachable" | "unparsable" => {
            let mut diagnostic = Diagnostic::new(Reason::ForgeTemporary, detail)
                .action("rerun when the release page answers, or pass --tag")
                .target_state("nothing was written");
            diagnostic.retry = Some(true);
            Err(RkError::subprocess(diagnostic))
        }
        "update-failed" | "build-failed" => {
            let step = run
                .steps
                .as_ref()
                .and_then(|steps| steps.iter().find(|s| s.status == "failed"))
                .map_or("transaction", |s| s.step);
            Err(RkError::subprocess(
                Diagnostic::new(Reason::SubprocessFailed, detail)
                    .step(step)
                    .target_state(format!(
                        "restored {}",
                        run.restored.as_deref().unwrap_or_default().join(" and ")
                    )),
            ))
        }
        "lock-unavailable" | "restore-failed" | "recovery-failed" | "cleanup-failed" => {
            Err(RkError::Io(std::io::Error::other(detail)))
        }
        _ => Ok(()),
    }
}

/// Remove what the catalog can judge, name the rest.
fn clean(args: &CleanArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let observed = devshell::observe(&args.target)?;
    let target = &observed.target;
    let mut leftovers = observed.leftovers.clone();
    for path in &args.also {
        leftovers.push(also_leftover(target, path)?);
    }
    let mode = if args.apply { "apply" } else { "preview" };
    let mut removed = Vec::new();
    let mut rewritten = Vec::new();
    let mut manual = Vec::new();
    if args.apply {
        for leftover in &leftovers {
            match leftover.action {
                Action::RemoveFile => {
                    std::fs::remove_file(target.join(&leftover.file))?;
                    removed.push(leftover.file.clone());
                }
                Action::ReplaceLine => {}
                Action::Manual => manual.push(Manual {
                    id: leftover.id,
                    file: leftover.file.clone(),
                    line: leftover.line,
                    text: leftover.text.clone(),
                    reason: leftover.reason,
                }),
            }
        }
        if leftovers.iter().any(|l| l.action == Action::ReplaceLine) {
            let envrc = target.join(".envrc");
            let text = std::fs::read_to_string(&envrc)?;
            if let Some(swapped) = leftovers::swap_envrc(&text, &fragments::envrc_line()) {
                crate::atomic::write(envrc.as_std_path(), swapped.as_bytes())?;
                rewritten.push(".envrc".to_owned());
            }
        }
    }
    if args.apply {
        for file in &removed {
            out.result_line(format!("removed {file}"));
        }
        for file in &rewritten {
            out.result_line(format!(
                "rewrote {file}: the sync line replaces the invocation"
            ));
        }
        for entry in &manual {
            out.result_line(format!(
                "manual {}{} {} ({}: {})",
                entry.file,
                entry.line.map(|n| format!(":{n}")).unwrap_or_default(),
                entry.text.as_deref().unwrap_or_default(),
                entry.id,
                entry.reason
            ));
        }
        if removed.is_empty() && rewritten.is_empty() && manual.is_empty() {
            out.result_line("nothing to remove: the target carries no predecessor mechanism");
        }
    } else {
        out.result_line(
            "DRY RUN: rk devshell clean removes and rewrites these on --apply, and names the rest",
        );
        for leftover in &leftovers {
            out.result_line(leftover_line(leftover));
        }
        if leftovers.is_empty() {
            out.result_line("nothing to remove: the target carries no predecessor mechanism");
        }
    }
    let next = clean_next(&observed, args.apply, &leftovers, &manual);
    out.next(&next);
    out.emit(&CleanReport {
        schema: "rk.devshell-clean/1",
        mode,
        target: target.as_str(),
        leftovers: &leftovers,
        removed: &removed,
        rewritten: &rewritten,
        manual: &manual,
        next: &next,
    })
}

/// One `--also` path as a leftover, or the refusal: it must be a regular
/// file inside the target, judged before any write.
fn also_leftover(target: &Utf8Path, path: &Utf8Path) -> Result<Leftover, RkError> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        target.join(path)
    };
    let refuse = |why: &str| {
        RkError::refusal(
            Diagnostic::new(
                Reason::DestructiveRefusal,
                format!("--also {path} is {why}; nothing was removed"),
            )
            .expected("a regular file inside the target, named for removal"),
        )
    };
    let Ok(meta) = std::fs::symlink_metadata(&absolute) else {
        return Err(refuse("not a file that exists"));
    };
    if meta.file_type().is_symlink() {
        return Err(refuse("a symlink, which a file removal never follows"));
    }
    if meta.is_dir() {
        return Err(refuse("a directory, and the cleanup removes files alone"));
    }
    let canonical = absolute.canonicalize_utf8()?;
    let Ok(relative) = canonical.strip_prefix(target) else {
        return Err(refuse("outside the target"));
    };
    Ok(Leftover {
        id: "also",
        file: relative.to_string(),
        line: None,
        text: None,
        action: Action::RemoveFile,
        reason: "named by the operator as a predecessor file the catalog does not know",
    })
}

/// What plausibly follows a clean.
fn clean_next(
    observed: &Observed,
    apply: bool,
    leftovers: &[Leftover],
    manual: &[Manual],
) -> Vec<String> {
    let target = &observed.target;
    let mut next = Vec::new();
    if !apply && !leftovers.is_empty() {
        next.push(format!(
            "rk devshell clean --target {target} --apply removes the files and rewrites .envrc"
        ));
    }
    let by_hand: Vec<String> = if apply {
        manual
            .iter()
            .map(|entry| entry.file.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    } else {
        leftovers
            .iter()
            .filter(|l| l.action == Action::Manual)
            .map(|l| l.file.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    if !by_hand.is_empty() {
        next.push(format!(
            "edit by hand what a line scan must not touch: {}",
            by_hand.join(", ")
        ));
    }
    next.push(format!(
        "rk devshell status --target {target} reports ready once the leftovers list is empty"
    ));
    if matches!(observed.scan, pin::Scan::None) {
        next.push(format!(
            "rk devshell add --target {target} wires the native mechanism once the predecessor is gone"
        ));
    }
    next
}

/// The tag an `add` pins: the argument, normalized, or this binary's own
/// version, so the fragments stay offline and deterministic.
fn resolve_tag(argument: Option<&str>) -> Result<(String, &'static str), RkError> {
    let Some(raw) = argument else {
        return Ok((format!("v{}", env!("CARGO_PKG_VERSION")), "binary"));
    };
    devshell::normalize_tag(raw)
        .map(|tag| (tag, "argument"))
        .ok_or_else(|| {
            RkError::Usage(format!(
                "--tag {raw} is not a release tag; pass v0.2.16, 0.2.16, or the release URL"
            ))
        })
}

/// What plausibly follows an add.
fn add_next(observed: &Observed, apply: bool, written: &[String]) -> Vec<String> {
    let target = &observed.target;
    let mut next = Vec::new();
    if !observed.leftovers.is_empty() {
        next.push(format!(
            "rk devshell clean --target {target} first: the target carries a predecessor bump mechanism, and one project runs one"
        ));
    }
    if !apply {
        next.push(format!(
            "rk devshell add --target {target} --apply seeds the files the target lacks; an owned file takes its fragments by hand, in the order above"
        ));
        next.push(
            "run rk init --nix before the apply where the landed packaging capability is also wanted: a seeded flake.nix withholds it later".to_owned(),
        );
    }
    if !written.is_empty() {
        next.push(format!(
            "commit {} first — nix reads only tracked files, and the sync refuses uncommitted edits to the pair",
            written.join(" and ")
        ));
    }
    next.push(format!(
        "rk devshell sync --caller operator --apply --target {target} writes the lock and proves the build; commit flake.lock, then direnv allow"
    ));
    next
}

/// The human line for the input.
fn input_line(observed: &Observed) -> String {
    use std::fmt::Write as _;
    match &observed.scan {
        pin::Scan::None => "input absent".to_owned(),
        pin::Scan::Unpinned(line) => format!("input unpinned at line {line}"),
        pin::Scan::Many(count) => format!("input ambiguous: {count} lines name it"),
        pin::Scan::One(pin) => {
            let mut line = format!("input pinned {}", pin.tag);
            if let Some(rev) = &observed.locked_rev {
                let _ = write!(line, ", locked at {rev}");
            }
            line
        }
    }
}

/// The closed `input` vocabulary.
const fn input_word(scan: &pin::Scan) -> &'static str {
    match scan {
        pin::Scan::None => "absent",
        pin::Scan::Unpinned(_) => "unpinned",
        pin::Scan::One(_) => "pinned",
        pin::Scan::Many(_) => "ambiguous",
    }
}

/// How many lines name the input, where any does.
const fn pin_lines(scan: &pin::Scan) -> Option<usize> {
    match scan {
        pin::Scan::None => None,
        pin::Scan::Unpinned(_) | pin::Scan::One(_) => Some(1),
        pin::Scan::Many(count) => Some(*count),
    }
}

/// The human word for a presence.
const fn word(presence: Presence) -> &'static str {
    match presence {
        Presence::Present => "present",
        Presence::Absent => "absent",
    }
}

/// The host word for a probe.
const fn probe_word(probe: &probes::ProbeResult) -> &'static str {
    match probe.status {
        ProbeStatus::Ok => "ok",
        ProbeStatus::Failed => "failed",
    }
}

/// What plausibly follows a status.
fn status_next(observed: &Observed) -> Vec<String> {
    let target = &observed.target;
    match observed.state() {
        "pending-recovery" => vec![format!(
            "rk devshell sync --caller operator --target {target} recovers the interrupted run"
        )],
        "no-flake" | "not-wired" | "unpinned" => vec![format!(
            "rk devshell add --target {target} prints the fragments; --apply seeds the files a target lacks"
        )],
        "ambiguous-pin" => vec![format!(
            "leave exactly one release-kit input line in {target}/flake.nix, then rerun"
        )],
        "superseded" => vec![format!(
            "rk devshell clean --target {target} previews the removal of the predecessor mechanism; --apply removes it"
        )],
        _ => vec![format!(
            "rk devshell sync --caller operator --target {target} reports whether the pin is current"
        )],
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{AddReport, CleanReport, Host, Manual, StatusReport, Step, SyncReport};

    /// The complete `rk.devshell-sync/1` shape, held by snapshot.
    #[test]
    fn the_devshell_sync_schema_snapshot_holds() {
        let steps = vec![
            Step {
                step: "rewrite-pin",
                status: "ok",
                detail: None,
            },
            Step {
                step: "build",
                status: "failed",
                detail: Some("error: builder failed".to_owned()),
            },
        ];
        let restored = vec!["flake.nix".to_owned(), "flake.lock".to_owned()];
        let recovered = vec!["flake.nix".to_owned()];
        let next = vec!["both files are as they were".to_owned()];
        let report = SyncReport {
            schema: "rk.devshell-sync/1",
            mode: "apply",
            caller: "operator",
            target: "/srv/widget",
            outcome: "build-failed",
            from: Some("v0.2.15"),
            to: Some("v0.2.16"),
            detail: Some("build failed: error: builder failed"),
            steps: Some(&steps),
            restored: Some(&restored),
            recovered: Some(&recovered),
            stamp: Some("2026-09-04"),
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.devshell-sync/1","mode":"apply","caller":"operator","target":"/srv/widget","outcome":"build-failed","from":"v0.2.15","to":"v0.2.16","detail":"build failed: error: builder failed","steps":[{"step":"rewrite-pin","status":"ok"},{"step":"build","status":"failed","detail":"error: builder failed"}],"restored":["flake.nix","flake.lock"],"recovered":["flake.nix"],"stamp":"2026-09-04","next":["both files are as they were"]}"#
        );
        let bare = SyncReport {
            schema: "rk.devshell-sync/1",
            mode: "preview",
            caller: "envrc",
            target: "/srv/widget",
            outcome: "no-flake",
            from: None,
            to: None,
            detail: None,
            steps: None,
            restored: None,
            recovered: None,
            stamp: None,
            next: &[],
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("a report serializes"),
            r#"{"schema":"rk.devshell-sync/1","mode":"preview","caller":"envrc","target":"/srv/widget","outcome":"no-flake","next":[]}"#,
            "an unknown value is omitted, never null"
        );
    }

    /// The complete `rk.devshell-clean/1` shape, held by snapshot.
    #[test]
    fn the_devshell_clean_schema_snapshot_holds() {
        let leftovers = vec![Leftover {
            id: "bump-script",
            file: "scripts/rk-bump.sh".to_owned(),
            line: None,
            text: None,
            action: Action::RemoveFile,
            reason: "the file exists only for the predecessor bump mechanism",
        }];
        let removed = vec!["scripts/rk-bump.sh".to_owned()];
        let rewritten = vec![".envrc".to_owned()];
        let manual = vec![Manual {
            id: "just-recipe",
            file: "justfile".to_owned(),
            line: Some(42),
            text: Some("rk-bump:".to_owned()),
            reason: "a recipe body carries structure a line scan cannot judge",
        }];
        let next = vec!["rk devshell status".to_owned()];
        let report = CleanReport {
            schema: "rk.devshell-clean/1",
            mode: "apply",
            target: "/srv/widget",
            leftovers: &leftovers,
            removed: &removed,
            rewritten: &rewritten,
            manual: &manual,
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.devshell-clean/1","mode":"apply","target":"/srv/widget","leftovers":[{"id":"bump-script","file":"scripts/rk-bump.sh","action":"remove-file","reason":"the file exists only for the predecessor bump mechanism"}],"removed":["scripts/rk-bump.sh"],"rewritten":[".envrc"],"manual":[{"id":"just-recipe","file":"justfile","line":42,"text":"rk-bump:","reason":"a recipe body carries structure a line scan cannot judge"}],"next":["rk devshell status"]}"#
        );
        let bare = Manual {
            id: "also",
            file: "old.sh".to_owned(),
            line: None,
            text: None,
            reason: "named by the operator",
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("an entry serializes"),
            r#"{"id":"also","file":"old.sh","reason":"named by the operator"}"#,
            "an absent line and text are omitted, never null"
        );
    }
    use crate::devshell::Presence;
    use crate::devshell::fragments::{Anchor, Fragment};
    use crate::devshell::leftovers::{Action, Leftover};

    /// The complete `rk.devshell-add/1` shape, held by snapshot, the
    /// fragment carrying every field the agent contract names.
    #[test]
    fn the_devshell_add_schema_snapshot_holds() {
        let fragments = vec![Fragment {
            id: "flake-input",
            file: "flake.nix",
            role: "the pinned release-kit input",
            placement: "insert-into-attrset",
            anchor: Anchor {
                kind: "attrset",
                path: "inputs",
                needle: Some("inputs = {"),
            },
            text: "release-kit = {};".to_owned(),
            present: Some(false),
        }];
        let written = vec![".envrc".to_owned()];
        let next = vec!["direnv allow".to_owned()];
        let report = AddReport {
            schema: "rk.devshell-add/1",
            mode: "apply",
            target: "/srv/widget",
            tag: "v0.2.16",
            tag_source: "binary",
            flake: Presence::Present,
            envrc: Presence::Absent,
            written: &written,
            refusal: Some("the target already carries flake.nix"),
            fragments: &fragments,
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.devshell-add/1","mode":"apply","target":"/srv/widget","tag":"v0.2.16","tag_source":"binary","flake":"present","envrc":"absent","written":[".envrc"],"refusal":"the target already carries flake.nix","fragments":[{"id":"flake-input","file":"flake.nix","role":"the pinned release-kit input","placement":"insert-into-attrset","anchor":{"kind":"attrset","path":"inputs","needle":"inputs = {"},"text":"release-kit = {};","present":false}],"next":["direnv allow"]}"#
        );
        let bare = Fragment {
            id: "envrc-sync",
            file: ".envrc",
            role: "the daily sync on directory entry",
            placement: "append-line",
            anchor: Anchor {
                kind: "file",
                path: ".envrc",
                needle: None,
            },
            text: "line".to_owned(),
            present: None,
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("a fragment serializes"),
            r#"{"id":"envrc-sync","file":".envrc","role":"the daily sync on directory entry","placement":"append-line","anchor":{"kind":"file","path":".envrc"},"text":"line"}"#,
            "an unjudged presence and a missing needle are omitted, never null"
        );
    }

    /// The complete `rk.devshell-status/1` shape, held by snapshot.
    #[test]
    fn the_devshell_status_schema_snapshot_holds() {
        let leftovers = vec![
            Leftover {
                id: "just-recipe",
                file: "justfile".to_owned(),
                line: Some(42),
                text: Some("rk-bump:".to_owned()),
                action: Action::Manual,
                reason: "a recipe body carries structure a line scan cannot judge",
            },
            Leftover {
                id: "bump-script",
                file: "scripts/rk-bump.sh".to_owned(),
                line: None,
                text: None,
                action: Action::RemoveFile,
                reason: "the file exists only for the predecessor bump mechanism",
            },
        ];
        let next = vec!["rk devshell sync --caller operator --target /srv/widget reports whether the pin is current".to_owned()];
        let report = StatusReport {
            schema: "rk.devshell-status/1",
            target: "/srv/widget",
            state: "ready",
            flake: Presence::Present,
            lock: Presence::Present,
            input: "pinned",
            pin_tag: Some("v0.2.16"),
            pin_lines: Some(1),
            locked_ref: Some("refs/tags/v0.2.16"),
            locked_rev: Some("9f3c"),
            envrc: Presence::Present,
            envrc_sync: true,
            stamp: Some("2026-09-04"),
            pending: false,
            host: Host {
                nix: "ok",
                direnv: "failed",
            },
            leftovers: &leftovers,
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.devshell-status/1","target":"/srv/widget","state":"ready","flake":"present","lock":"present","input":"pinned","pin_tag":"v0.2.16","pin_lines":1,"locked_ref":"refs/tags/v0.2.16","locked_rev":"9f3c","envrc":"present","envrc_sync":true,"stamp":"2026-09-04","pending":false,"host":{"nix":"ok","direnv":"failed"},"leftovers":[{"id":"just-recipe","file":"justfile","line":42,"text":"rk-bump:","action":"manual","reason":"a recipe body carries structure a line scan cannot judge"},{"id":"bump-script","file":"scripts/rk-bump.sh","action":"remove-file","reason":"the file exists only for the predecessor bump mechanism"}],"next":["rk devshell sync --caller operator --target /srv/widget reports whether the pin is current"]}"#
        );
        let bare = StatusReport {
            schema: "rk.devshell-status/1",
            target: "/srv/widget",
            state: "no-flake",
            flake: Presence::Absent,
            lock: Presence::Absent,
            input: "absent",
            pin_tag: None,
            pin_lines: None,
            locked_ref: None,
            locked_rev: None,
            envrc: Presence::Absent,
            envrc_sync: false,
            stamp: None,
            pending: false,
            host: Host {
                nix: "failed",
                direnv: "failed",
            },
            leftovers: &[],
            next: &[],
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("a report serializes"),
            r#"{"schema":"rk.devshell-status/1","target":"/srv/widget","state":"no-flake","flake":"absent","lock":"absent","input":"absent","envrc":"absent","envrc_sync":false,"pending":false,"host":{"nix":"failed","direnv":"failed"},"leftovers":[],"next":[]}"#,
            "an unknown value must be omitted, not serialized as null"
        );
    }
}
