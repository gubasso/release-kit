//! `rk self-depend status | add | clean | sync`: the consumer half of
//! release-kit's own distribution.
//!
//! The producer half already ships: the crate, the flake at every tag,
//! and the release archives. This handler serves a consumer that pins
//! one of those through the tool manager it runs. `status` is the offline
//! reporter and fetches nothing; `add` serves the fragments for one
//! manager and venue pair and seeds the manager file where a target has
//! none, never editing a file the target owns; `clean` removes what a
//! predecessor mechanism left and names the rest, so the wiring is a
//! replacement and never an addition; `sync` moves the pin to the latest
//! release through the wired manager, inside a fenced transaction for
//! the flake pair, both files or neither. Every report goes through the
//! output boundary with a versioned schema.

use serde::Serialize;

use camino::Utf8Path;

use crate::cli::self_depend::{
    AddArgs, Caller, CleanArgs, SelfDependAction, SelfDependArgs, StatusArgs, SyncArgs,
};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::output::Output;
use crate::probes::{self, ProbeStatus};
use crate::self_depend::discover::{self, Discovery};
use crate::self_depend::fragments::{self, Fragment};
use crate::self_depend::guard::{self, Acquired};
use crate::self_depend::leftovers::{self, Action, Leftover};
use crate::self_depend::manager::{self, Entry, Manager, PinRead};
use crate::self_depend::matrix::{self, Mode, Pair, Support};
use crate::self_depend::txn::{self, AbortFailure, Recovery, StepFailure};
use crate::self_depend::venue::Venue;
use crate::self_depend::{self, Observed, Presence, pin};

/// The `rk.self-depend-status/2` document.
#[derive(Debug, Serialize)]
struct StatusReport<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// The target, canonical.
    target: &'a str,
    /// The rollup: `ready`, `superseded`, `no-manager`, `not-wired`,
    /// `unpinned`, `ambiguous-pin`, or `pending-recovery`. It describes
    /// and never judges: every state exits 0.
    state: &'static str,
    /// The one manager whose file names release-kit, where exactly one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    wired: Option<Manager>,
    /// One entry per manager in the closed order, absent ones included;
    /// one entry under `--manager`.
    managers: &'a [Entry],
    /// Whether `.envrc` exists. Direnv is not a manager: it loads a
    /// shell and pins nothing, so it sits outside the list.
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

/// The `rk.self-depend-add/2` document.
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
    /// The manager the fragments are for.
    manager: Manager,
    /// The venue the manager fetches rk from.
    venue: Venue,
    /// How the pair lands here: `fragment` into the present file, `seed`
    /// of the absent file, or `manual` with its reason.
    support: Mode,
    /// Why the pair is manual, from the closed set, where it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    /// The manager file, relative to the target: the present one, or
    /// the one a seed writes.
    file: &'a str,
    /// Whether the manager file existed before the run.
    file_present: Presence,
    /// Whether `.envrc` existed before the run.
    envrc: Presence,
    /// The seed files this run wrote, relative to the target; empty in
    /// preview.
    written: &'a [String],
    /// Why an owned file was refused, where one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<&'a str>,
    /// The fragments, in application order, the `.envrc` line last.
    fragments: &'a [Fragment],
    /// What plausibly follows.
    next: &'a [String],
}

/// The `rk.self-depend-clean/1` document.
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

/// The `rk.self-depend-sync/2` document.
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
    /// The manager whose pin the run reads and moves, where one was
    /// resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    manager: Option<Manager>,
    /// The closed outcome vocabulary: `bumped`, `current`, `ahead`,
    /// `would-bump`, `pending-recovery`, `recovery-failed`, `skipped-ci`,
    /// `skipped-disabled`, `skipped-stamped`, `skipped-locked`,
    /// `lock-unavailable`, `not-wired`, `unpinned`, `no-manager`,
    /// `ambiguous-pin`, `refused-dirty`, `unreachable`, `unparsable`,
    /// `update-failed`, `build-failed`, `restore-failed`, or
    /// `cleanup-failed`.
    outcome: &'static str,
    /// The pinned version before the run, as the manager records it,
    /// where the pin was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<&'a str>,
    /// The version the run moved to or would move to, in the same form,
    /// where one was resolved.
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
#[allow(
    clippy::struct_field_names,
    reason = "the fields are the keys of a serialized machine shape, so they answer to the schema rather than to the struct name"
)]
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
    manager: Option<Manager>,
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

/// Dispatch the self-depend action.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory,
/// [`RkError::Io`] where a present file does not read, and
/// [`RkError::Usage`] for an action this build does not carry yet.
pub fn run(args: &SelfDependArgs) -> Result<(), RkError> {
    match &args.action {
        SelfDependAction::Status(args) => status(args),
        SelfDependAction::Add(args) => add(args),
        SelfDependAction::Clean(args) => clean(args),
        SelfDependAction::Sync(args) => sync(args),
    }
}

/// Report the wiring, offline: it describes every state and exits 0 on
/// each, because the verb has no `--check` mode and a report is not a
/// verdict.
fn status(args: &StatusArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let observed = self_depend::observe(&args.target)?;
    let host = Host {
        nix: probe_word(&probes::nix()),
        direnv: probe_word(&probes::direnv()),
    };
    let state = observed.state();
    let managers: Vec<Entry> = observed
        .managers
        .iter()
        .filter(|entry| args.manager.is_none_or(|wanted| entry.manager == wanted))
        .cloned()
        .collect();
    out.result_line(format!("state {state}"));
    if let Some(wired) = observed.wired {
        out.result_line(format!("wired through {}", wired.as_str()));
    }
    for entry in &managers {
        out.result_line(manager_line(entry));
    }
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
        schema: "rk.self-depend-status/2",
        target: observed.target.as_str(),
        state,
        wired: observed.wired,
        managers: &managers,
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

/// Serve the fragments for one pair; seed the manager file a target
/// lacks under `--apply`, and the `.envrc` beside it for the flake pair.
#[allow(
    clippy::too_many_lines,
    reason = "one pass from the observation to the report, so every refusal is judged before any write"
)]
fn add(args: &AddArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let observed = self_depend::observe(&args.target)?;
    let (tag, tag_source) = resolve_tag(args.tag.as_deref())?;
    let pair = matrix::choose(&observed, args.manager, args.venue)?;
    let entry = observed.entry(pair.manager);
    let file = entry
        .and_then(|entry| entry.file.clone())
        .unwrap_or_else(|| pair.manager.default_file().to_owned());
    let file_present = entry.map_or(Presence::Absent, |entry| entry.present);
    let (support, reason) = match matrix::support(pair.manager, pair.venue) {
        Support::Fragment if file_present.is_present() => (Mode::Fragment, None),
        Support::Fragment => (Mode::Seed, None),
        Support::Manual(reason) => (Mode::Manual, Some(reason)),
    };
    // One target runs one bump mechanism: a second manager naming
    // release-kit is two pins, refused before any write.
    if let Some(wired) = observed.wired.filter(|wired| *wired != pair.manager) {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::DestructiveRefusal,
                format!(
                    "the target already pins release-kit through {}; a second manager is a second bump mechanism",
                    wired.as_str()
                ),
            )
            .expected(format!(
                "one manager naming release-kit; rk self-depend add --manager {} serves that one",
                wired.as_str()
            ))
            .target_state("nothing was written"),
        ));
    }
    let fragments = fragments::fragments(pair, &tag, &observed);
    let mode = if args.apply { "apply" } else { "preview" };
    let flake_pair = pair.manager == Manager::Flake && pair.venue == Venue::Flake;
    let mut written = Vec::new();
    let mut owned = Vec::new();
    if args.apply && support != Mode::Manual {
        let mut seeds = vec![(file.clone(), file_present, fragments::seed(pair, &tag))];
        if flake_pair {
            seeds.push((
                ".envrc".to_owned(),
                observed.envrc,
                Some(fragments::seed_envrc()),
            ));
        }
        for (name, present, seed) in seeds {
            let Some(seed) = seed else {
                continue;
            };
            if present.is_present() {
                owned.push(name);
            } else {
                crate::atomic::write(observed.target.join(&name).as_std_path(), seed.as_bytes())?;
                written.push(name);
            }
        }
    }
    let refusal = (!owned.is_empty()).then(|| {
        format!(
            "the target already carries {}; rk self-depend add never edits a file the target owns",
            owned.join(" and ")
        )
    });
    if args.apply {
        for name in &written {
            out.result_line(format!("wrote {name}"));
        }
    } else {
        out.result_line("DRY RUN: rk self-depend add prints the fragments; --apply seeds only the files the target lacks");
    }
    out.result_line(format!("tag {tag} (from the {tag_source})"));
    out.result_line(match (support, reason) {
        (Mode::Manual, Some(reason)) => format!(
            "pair {} via {}: manual ({reason}); nothing is written",
            pair.manager.as_str(),
            pair.venue.as_str()
        ),
        (mode, _) => format!(
            "pair {} via {}: {}",
            pair.manager.as_str(),
            pair.venue.as_str(),
            match mode {
                Mode::Fragment => "fragments into the present file",
                Mode::Seed => "seeded on --apply",
                Mode::Manual => "manual",
            }
        ),
    });
    if support != Mode::Manual {
        out.result_line(match file_present {
            Presence::Present => {
                format!("{file} present: the target owns it, so its fragments are applied by hand")
            }
            Presence::Absent => format!("{file} absent: --apply seeds it"),
        });
    }
    out.result_line(match (observed.envrc, flake_pair) {
        (Presence::Present, _) => {
            ".envrc present: the target owns it, so the sync line is applied by hand".to_owned()
        }
        (Presence::Absent, true) => ".envrc absent: --apply seeds it".to_owned(),
        (Presence::Absent, false) => {
            ".envrc absent: the sync line is applied by hand once the shell loads rk".to_owned()
        }
    });
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
    let next = add_next(&observed, pair, &file, args.apply, &written);
    out.next(&next);
    out.emit(&AddReport {
        schema: "rk.self-depend-add/2",
        mode,
        target: observed.target.as_str(),
        tag: &tag,
        tag_source,
        manager: pair.manager,
        venue: pair.venue,
        support,
        reason,
        file: &file,
        file_present,
        envrc: observed.envrc,
        written: &written,
        refusal: refusal.as_deref(),
        fragments: &fragments,
        next: &next,
    })?;
    if args.apply && support == Mode::Manual {
        return Err(RkError::Usage(format!(
            "{} via {} is manual ({}); nothing can be written, and the report names the edit",
            pair.manager.as_str(),
            pair.venue.as_str(),
            reason.unwrap_or_default()
        )));
    }
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
            .expected("a target with no file for the pair, or the fragments applied by hand")
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
    let mut observed = self_depend::observe(&args.target)?;
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
                    "rk self-depend sync: the lock cannot be taken: {source}"
                ));
                return Ok(());
            }
        }
    }
    if args.apply {
        // A marker that cannot be read is a reported outcome, never an
        // error: the unattended caller must still exit 0.
        let recovery = match txn::recover_pending(&observed.target, key) {
            Ok(recovery) => recovery,
            Err(source) => {
                run.outcome = "recovery-failed";
                run.detail = Some(format!(
                    "the transaction marker under the state root cannot be read: {source}; remove or repair it by hand"
                ));
                return Ok(());
            }
        };
        match recovery {
            Some(Recovery::Restored(restored)) => {
                run.recovered = Some(restored);
                *observed = self_depend::observe(&args.target)?;
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
                *observed = self_depend::observe(&args.target)?;
            }
            None => {}
        }
    }
    decide(args, observed, key, run)
}

/// The manager whose pin a sync reads: the flag, else the one manager
/// naming release-kit. Where none or several do, the outcome is set and
/// `None` returned.
fn sync_manager(args: &SyncArgs, observed: &Observed, run: &mut SyncRun) -> Option<Manager> {
    if let Some(manager) = args.manager {
        return Some(manager);
    }
    if let Some(wired) = observed.wired {
        return Some(wired);
    }
    if observed
        .managers
        .iter()
        .all(|entry| !entry.present.is_present())
    {
        run.outcome = "no-manager";
        return None;
    }
    let named: Vec<&str> = observed
        .managers
        .iter()
        .filter(|entry| entry.read.names())
        .map(|entry| entry.manager.as_str())
        .collect();
    if named.len() > 1 {
        run.outcome = "ambiguous-pin";
        run.detail = Some(format!(
            "{} manager files name release-kit: {}",
            named.len(),
            named.join(" and ")
        ));
    } else {
        run.outcome = "not-wired";
    }
    None
}

/// The files a sync judges for uncommitted edits: the flake pair for
/// the flake manager, the one manager file otherwise.
fn sync_files(manager: Manager, file: &str) -> Vec<&str> {
    if manager == Manager::Flake {
        vec!["flake.nix", "flake.lock"]
    } else {
        vec![file]
    }
}

/// The decision sequence: the wiring, the dirty check, the target tag,
/// the comparison, and — under `--apply` on a behind pin — the move.
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
    let Some(manager) = sync_manager(args, observed, run) else {
        return Ok(());
    };
    run.manager = Some(manager);
    let Some(entry) = observed
        .entry(manager)
        .filter(|entry| entry.present.is_present())
    else {
        run.outcome = "no-manager";
        run.detail = Some(format!("the target carries no {} file", manager.as_str()));
        return Ok(());
    };
    let file = entry.file.clone().unwrap_or_default();
    let (line, from) = match &entry.read {
        PinRead::Many { count } => {
            run.outcome = "ambiguous-pin";
            run.detail = Some(format!("{count} lines name release-kit in {file}"));
            return Ok(());
        }
        PinRead::Absent => {
            run.outcome = "not-wired";
            return Ok(());
        }
        PinRead::Unpinned { line } => {
            run.outcome = "unpinned";
            run.detail = Some(format!(
                "{file} line {line} names release-kit with no version"
            ));
            return Ok(());
        }
        PinRead::One { line, version } => (*line, version.clone()),
    };
    run.from = Some(from.clone());
    if guard::files_dirty(&observed.target, &sync_files(manager, &file)) {
        run.outcome = "refused-dirty";
        run.detail = Some(format!(
            "{} carries uncommitted edits; commit or stash them first",
            sync_files(manager, &file).join(" or ")
        ));
        return Ok(());
    }
    let to = match args.tag.as_deref() {
        Some(raw) => self_depend::normalize_tag(raw).ok_or_else(|| {
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
    let to = manager.recorded(&to);
    run.to = Some(to.clone());
    match discover::version_order(&from, &to) {
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
    if manager == Manager::Flake {
        let pin::Scan::One(pin) = &observed.scan else {
            run.outcome = "not-wired";
            return Ok(());
        };
        return apply_bump(observed, key, pin, &to, run);
    }
    move_one_fact(observed, entry, &file, line, &from, &to, run);
    Ok(())
}

/// A one-fact manager moves its one fact: no lock to refresh and no
/// build to fence, so the atomic write is the whole transaction.
fn move_one_fact(
    observed: &Observed,
    entry: &Entry,
    file: &str,
    line: usize,
    from: &str,
    to: &str,
    run: &mut SyncRun,
) {
    let text = entry.text.as_deref().unwrap_or_default();
    let rewritten = manager::rewrite_line(text, line, from, to);
    let step = match crate::atomic::write(
        observed.target.join(file).as_std_path(),
        rewritten.as_bytes(),
    ) {
        Ok(()) => {
            run.outcome = "bumped";
            Step {
                step: "rewrite-pin",
                status: "ok",
                detail: None,
            }
        }
        Err(source) => {
            run.outcome = "update-failed";
            run.detail = Some(format!("rewrite-pin failed: {source}"));
            run.restored = Some(Vec::new());
            Step {
                step: "rewrite-pin",
                status: "failed",
                detail: Some(source.to_string()),
            }
        }
    };
    run.steps = Some(vec![step]);
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
            | b"no-manager"
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
        schema: "rk.self-depend-sync/2",
        mode: if args.apply { "apply" } else { "preview" },
        caller: match args.caller {
            Caller::Envrc => "envrc",
            Caller::Operator => "operator",
        },
        target: observed.target.as_str(),
        manager: run.manager,
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
    let files = run
        .manager
        .map(|manager| {
            let file = observed
                .entry(manager)
                .and_then(|entry| entry.file.clone())
                .unwrap_or_else(|| manager.default_file().to_owned());
            sync_files(manager, &file).join(" ")
        })
        .unwrap_or_default();
    let mut next = match run.outcome {
        "bumped" => vec![
            format!("git -C {target} diff -- {files} shows the change to review and commit"),
            "the next shell reload takes the new rk; nothing here commits".to_owned(),
        ],
        "would-bump" => vec![format!(
            "rk self-depend sync --caller operator --apply --target {target} moves the pin, locks it, and proves the build"
        )],
        "current" => vec![format!(
            "rk self-depend status --target {target} reports the wiring"
        )],
        "ahead" => vec![
            "a pin past the latest release is a deliberate state; nothing moves it back".to_owned(),
        ],
        "pending-recovery" => vec![format!(
            "rk self-depend sync --caller operator --apply --target {target} restores both files first"
        )],
        "no-manager" | "not-wired" | "unpinned" => vec![format!(
            "rk self-depend add --target {target} prints the fragments; --apply seeds the files a target lacks"
        )],
        "ambiguous-pin" => vec![format!(
            "leave exactly one line naming release-kit, in one manager file under {target}, then rerun"
        )],
        "refused-dirty" => vec![format!(
            "git -C {target} status -- {files} names the edits; commit or stash them, then rerun"
        )],
        "skipped-disabled" => vec![format!(
            "unset {} to let the sync run again",
            guard::SWITCH_VAR
        )],
        "skipped-stamped" => vec![format!(
            "rk self-depend sync --caller operator --apply --target {target} runs the attempt now, whatever the stamp says"
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
                "rk self-depend sync --caller operator --apply --target {target} retries after the fix"
            ),
        ],
        _ => Vec::new(),
    };
    if !observed.leftovers.is_empty() {
        next.push(format!(
            "rk self-depend clean --target {target}: the target still carries a predecessor bump mechanism"
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
                .expected("exactly one committed pin line in one manager file")
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
    let observed = self_depend::observe(&args.target)?;
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
            "DRY RUN: rk self-depend clean removes and rewrites these on --apply, and names the rest",
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
        schema: "rk.self-depend-clean/1",
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
            "rk self-depend clean --target {target} --apply removes the files and rewrites .envrc"
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
        "rk self-depend status --target {target} reports ready once the leftovers list is empty"
    ));
    if matches!(observed.scan, pin::Scan::None) {
        next.push(format!(
            "rk self-depend add --target {target} wires the native mechanism once the predecessor is gone"
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
    self_depend::normalize_tag(raw)
        .map(|tag| (tag, "argument"))
        .ok_or_else(|| {
            RkError::Usage(format!(
                "--tag {raw} is not a release tag; pass v0.2.16, 0.2.16, or the release URL"
            ))
        })
}

/// What plausibly follows an add.
fn add_next(
    observed: &Observed,
    pair: Pair,
    file: &str,
    apply: bool,
    written: &[String],
) -> Vec<String> {
    let target = &observed.target;
    let flake_pair = pair.manager == Manager::Flake;
    let mut next = Vec::new();
    if !observed.leftovers.is_empty() {
        next.push(format!(
            "rk self-depend clean --target {target} first: the target carries a predecessor bump mechanism, and one project runs one"
        ));
    }
    if let Support::Manual(reason) = matrix::support(pair.manager, pair.venue) {
        next.push(format!(
            "{} via {} is manual ({reason}): apply the edit by hand, or pick a pair the matrix renders",
            pair.manager.as_str(),
            pair.venue.as_str()
        ));
        return next;
    }
    if !apply {
        next.push(format!(
            "rk self-depend add --target {target} --apply seeds the files the target lacks; an owned file takes its fragments by hand, in the order above"
        ));
        if flake_pair {
            next.push(
                "run rk init --nix before the apply where the landed packaging capability is also wanted: a seeded flake.nix withholds it later".to_owned(),
            );
        }
    }
    if !written.is_empty() {
        next.push(format!(
            "commit {} first: the sync refuses uncommitted edits to the file it moves",
            written.join(" and ")
        ));
    }
    if flake_pair {
        next.push(format!(
            "rk self-depend sync --caller operator --apply --target {target} writes the lock and proves the build; commit flake.lock, then direnv allow"
        ));
    } else {
        next.push(format!(
            "rk self-depend sync --caller operator --apply --target {target} moves the pin in {file}; the manager's own install takes it from there"
        ));
    }
    next
}

/// The human line for one manager's entry.
fn manager_line(entry: &Entry) -> String {
    use std::fmt::Write as _;
    let mut line = format!("manager {} {}", entry.manager.as_str(), word(entry.present));
    let Some(file) = &entry.file else {
        return line;
    };
    let _ = write!(line, " ({file})");
    match (entry.pin, entry.pin_lines) {
        ("absent", _) => line.push_str(", not named"),
        ("unpinned", _) => line.push_str(", named with no version"),
        ("ambiguous", Some(count)) => {
            let _ = write!(line, ", ambiguous: {count} lines name it");
        }
        _ => {
            let _ = write!(
                line,
                ", pinned {}",
                entry.version.as_deref().unwrap_or_default()
            );
            if let Some(freshness) = entry.freshness {
                let _ = write!(
                    line,
                    " ({} this binary)",
                    match freshness {
                        crate::self_depend::manager::Freshness::Behind => "behind",
                        crate::self_depend::manager::Freshness::Current => "same as",
                        crate::self_depend::manager::Freshness::Ahead => "ahead of",
                    }
                );
            }
        }
    }
    if let Some(lock) = entry.lock {
        let _ = write!(line, ", lock {}", word(lock));
    }
    if let Some(rev) = &entry.locked_rev {
        let _ = write!(line, ", locked at {rev}");
    }
    line
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
            "rk self-depend sync --caller operator --target {target} recovers the interrupted run"
        )],
        "no-manager" | "not-wired" | "unpinned" => vec![format!(
            "rk self-depend add --target {target} prints the fragments; --apply seeds the files a target lacks"
        )],
        "ambiguous-pin" => vec![format!(
            "leave exactly one line naming release-kit, in one manager file under {target}, then rerun"
        )],
        "superseded" => vec![format!(
            "rk self-depend clean --target {target} previews the removal of the predecessor mechanism; --apply removes it"
        )],
        _ => vec![format!(
            "rk self-depend sync --caller operator --target {target} reports whether the pin is current"
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AddReport, CleanReport, Host, Manual, Mode, StatusReport, Step, SyncReport, Venue,
    };

    /// The complete `rk.self-depend-sync/1` shape, held by snapshot.
    #[test]
    fn the_self_depend_sync_schema_snapshot_holds() {
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
            schema: "rk.self-depend-sync/2",
            mode: "apply",
            caller: "operator",
            target: "/srv/widget",
            manager: Some(Manager::Flake),
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
            r#"{"schema":"rk.self-depend-sync/2","mode":"apply","caller":"operator","target":"/srv/widget","manager":"flake","outcome":"build-failed","from":"v0.2.15","to":"v0.2.16","detail":"build failed: error: builder failed","steps":[{"step":"rewrite-pin","status":"ok"},{"step":"build","status":"failed","detail":"error: builder failed"}],"restored":["flake.nix","flake.lock"],"recovered":["flake.nix"],"stamp":"2026-09-04","next":["both files are as they were"]}"#
        );
        let bare = SyncReport {
            schema: "rk.self-depend-sync/2",
            mode: "preview",
            caller: "envrc",
            target: "/srv/widget",
            manager: None,
            outcome: "no-manager",
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
            r#"{"schema":"rk.self-depend-sync/2","mode":"preview","caller":"envrc","target":"/srv/widget","outcome":"no-manager","next":[]}"#,
            "an unknown value is omitted, never null"
        );
    }

    /// The complete `rk.self-depend-clean/1` shape, held by snapshot.
    #[test]
    fn the_self_depend_clean_schema_snapshot_holds() {
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
        let next = vec!["rk self-depend status".to_owned()];
        let report = CleanReport {
            schema: "rk.self-depend-clean/1",
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
            r#"{"schema":"rk.self-depend-clean/1","mode":"apply","target":"/srv/widget","leftovers":[{"id":"bump-script","file":"scripts/rk-bump.sh","action":"remove-file","reason":"the file exists only for the predecessor bump mechanism"}],"removed":["scripts/rk-bump.sh"],"rewritten":[".envrc"],"manual":[{"id":"just-recipe","file":"justfile","line":42,"text":"rk-bump:","reason":"a recipe body carries structure a line scan cannot judge"}],"next":["rk self-depend status"]}"#
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
    use crate::self_depend::Presence;
    use crate::self_depend::fragments::{Anchor, Fragment};
    use crate::self_depend::leftovers::{Action, Leftover};
    use crate::self_depend::manager::{Entry, Freshness, Manager, PinRead};

    /// The complete `rk.self-depend-add/1` shape, held by snapshot, the
    /// fragment carrying every field the agent contract names.
    #[test]
    fn the_self_depend_add_schema_snapshot_holds() {
        let fragments = vec![Fragment {
            id: "flake-input",
            file: "flake.nix".to_owned(),
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
            schema: "rk.self-depend-add/2",
            mode: "apply",
            target: "/srv/widget",
            tag: "v0.2.16",
            tag_source: "binary",
            manager: Manager::Flake,
            venue: Venue::Flake,
            support: Mode::Fragment,
            reason: None,
            file: "flake.nix",
            file_present: Presence::Present,
            envrc: Presence::Absent,
            written: &written,
            refusal: Some("the target already carries flake.nix"),
            fragments: &fragments,
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.self-depend-add/2","mode":"apply","target":"/srv/widget","tag":"v0.2.16","tag_source":"binary","manager":"flake","venue":"flake","support":"fragment","file":"flake.nix","file_present":"present","envrc":"absent","written":[".envrc"],"refusal":"the target already carries flake.nix","fragments":[{"id":"flake-input","file":"flake.nix","role":"the pinned release-kit input","placement":"insert-into-attrset","anchor":{"kind":"attrset","path":"inputs","needle":"inputs = {"},"text":"release-kit = {};","present":false}],"next":["direnv allow"]}"#
        );
        let manual = AddReport {
            schema: "rk.self-depend-add/2",
            mode: "preview",
            target: "/srv/widget",
            tag: "v0.2.16",
            tag_source: "binary",
            manager: Manager::Asdf,
            venue: Venue::Crates,
            support: Mode::Manual,
            reason: Some("asdf-plugin-unknown"),
            file: ".tool-versions",
            file_present: Presence::Absent,
            envrc: Presence::Absent,
            written: &[],
            refusal: None,
            fragments: &[],
            next: &[],
        };
        assert_eq!(
            serde_json::to_string(&manual).expect("a report serializes"),
            r#"{"schema":"rk.self-depend-add/2","mode":"preview","target":"/srv/widget","tag":"v0.2.16","tag_source":"binary","manager":"asdf","venue":"crates","support":"manual","reason":"asdf-plugin-unknown","file":".tool-versions","file_present":"absent","envrc":"absent","written":[],"fragments":[],"next":[]}"#,
            "a manual pair carries its reason and no refusal"
        );
        let bare = Fragment {
            id: "envrc-sync",
            file: ".envrc".to_owned(),
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

    /// The complete `rk.self-depend-status/2` shape, held by snapshot,
    /// per `distribution:machine-output-declares-its-schema`.
    #[test]
    fn the_status_schema_is_versioned_and_snapshot_tested() {
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
        let managers = vec![
            Entry {
                manager: Manager::Flake,
                present: Presence::Present,
                file: Some("flake.nix".to_owned()),
                pin: "pinned",
                version: Some("v0.2.16".to_owned()),
                pin_lines: Some(1),
                freshness: Some(Freshness::Behind),
                lock: Some(Presence::Present),
                locked_ref: Some("refs/tags/v0.2.16".to_owned()),
                locked_rev: Some("9f3c".to_owned()),
                read: PinRead::One {
                    line: 4,
                    version: "v0.2.16".to_owned(),
                },
                text: None,
            },
            Entry::absent(Manager::Mise),
        ];
        let next = vec!["rk self-depend sync --caller operator --target /srv/widget reports whether the pin is current".to_owned()];
        let report = StatusReport {
            schema: "rk.self-depend-status/2",
            target: "/srv/widget",
            state: "ready",
            wired: Some(Manager::Flake),
            managers: &managers,
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
            r#"{"schema":"rk.self-depend-status/2","target":"/srv/widget","state":"ready","wired":"flake","managers":[{"manager":"flake","present":"present","file":"flake.nix","pin":"pinned","version":"v0.2.16","pin_lines":1,"freshness":"behind","lock":"present","locked_ref":"refs/tags/v0.2.16","locked_rev":"9f3c"},{"manager":"mise","present":"absent","pin":"absent"}],"envrc":"present","envrc_sync":true,"stamp":"2026-09-04","pending":false,"host":{"nix":"ok","direnv":"failed"},"leftovers":[{"id":"just-recipe","file":"justfile","line":42,"text":"rk-bump:","action":"manual","reason":"a recipe body carries structure a line scan cannot judge"},{"id":"bump-script","file":"scripts/rk-bump.sh","action":"remove-file","reason":"the file exists only for the predecessor bump mechanism"}],"next":["rk self-depend sync --caller operator --target /srv/widget reports whether the pin is current"]}"#
        );
        let bare = StatusReport {
            schema: "rk.self-depend-status/2",
            target: "/srv/widget",
            state: "no-manager",
            wired: None,
            managers: &[],
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
            r#"{"schema":"rk.self-depend-status/2","target":"/srv/widget","state":"no-manager","managers":[],"envrc":"absent","envrc_sync":false,"pending":false,"host":{"nix":"failed","direnv":"failed"},"leftovers":[],"next":[]}"#,
            "an unknown value must be omitted, not serialized as null"
        );
    }
}
