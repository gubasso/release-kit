//! `rk setup`: execute the repository-side setup against the detected forge.
//!
//! Preview is the default and is an offline rendering — it materializes
//! nothing and invokes no external command. Apply runs each step as the
//! observe-compare-apply-verify lifecycle: observe the current state,
//! report and skip when satisfied, otherwise materialize the embedded
//! script into the run's private journal directory, verify its digest,
//! spawn it as `sh <path>`, and read the state back. `check` calls the same
//! observe functions with the mutating half unreachable from its code path.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use zeroize::Zeroizing;

use crate::cli::setup::{SetupAction, SetupArgs};
use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::embedded;
use crate::error::RkError;
use crate::events::{ChildStream, Event, EventKind};
use crate::output::Output;
use crate::setup::app_jwt::{self, AppApi};
use crate::setup::context::{Ctx, SECRET_VARS};
use crate::setup::journal::Journal;
use crate::setup::observe::{self, StepState};
use crate::setup::process::{self, Exec, Outcome};
use crate::setup::secrets;
use crate::setup::steps::{Mutates, STEPS, StepSpec, spec};

/// Dispatch the setup surface.
///
/// # Errors
///
/// Every failure classified through the matrix, each carrying its `reason`.
pub fn run(args: &SetupArgs) -> Result<(), RkError> {
    match &args.action {
        Some(SetupAction::Script { name, forge }) => script(name, forge.as_deref()),
        Some(SetupAction::Check {
            target,
            repo,
            forge,
            required_check,
            json,
        }) => {
            let ctx = Ctx::resolve(
                target,
                repo.as_deref(),
                forge.as_deref(),
                required_check.as_deref(),
            )?;
            reject_check_flag_on_gitlab(&ctx)?;
            check(Output::new(*json), ctx)
        }
        Some(SetupAction::Step {
            name,
            target,
            repo,
            forge,
            required_check,
            apply,
            json,
        }) => {
            let selected = spec(name).ok_or_else(|| {
                RkError::Usage(format!("unknown step '{name}'; rk setup --list names them"))
            })?;
            let ctx = Ctx::resolve(
                target,
                repo.as_deref(),
                forge.as_deref(),
                required_check.as_deref(),
            )?;
            reject_check_flag_on_gitlab(&ctx)?;
            if *apply {
                require_check_for(&ctx, &[selected])?;
                execute(Output::new(*json), ctx, &[selected], "setup step")
            } else {
                preview(Output::new(*json), &ctx, &[selected])
            }
        }
        None if args.list => list(args.forge.as_deref()),
        None => {
            let target = args.target.clone().ok_or_else(|| {
                RkError::Usage("name a --target, or pass --list to see the steps".into())
            })?;
            let ctx = Ctx::resolve(
                &target,
                args.repo.as_deref(),
                args.forge.as_deref(),
                args.required_check.as_deref(),
            )?;
            reject_check_flag_on_gitlab(&ctx)?;
            let all: Vec<&StepSpec> = STEPS.iter().collect();
            if args.apply {
                require_check_for(&ctx, &all)?;
                execute(Output::new(args.json), ctx, &all, "setup")
            } else {
                preview(Output::new(args.json), &ctx, &all)
            }
        }
    }
}

/// On GitLab `--required-check` is a usage error, per the forge document:
/// the forge requires the whole pipeline and names no individual check, and
/// a flag silently discarded would read as configured while nothing uses it.
fn reject_check_flag_on_gitlab(ctx: &Ctx) -> Result<(), RkError> {
    if ctx.forge == Forge::Gitlab && ctx.required_check.is_some() {
        return Err(RkError::Usage(
            "--required-check is refused on gitlab: the forge requires the whole pipeline and names no individual check".into(),
        ));
    }
    Ok(())
}

/// On GitHub the trunk protection needs the check name before any step
/// runs: a wrong or missing one does not fail, it hangs the merge button,
/// so a full apply refuses up front rather than writing eight steps and
/// stopping.
fn require_check_for(ctx: &Ctx, steps: &[&StepSpec]) -> Result<(), RkError> {
    let needs = ctx.forge == Forge::Github
        && ctx.required_check.is_none()
        && steps.iter().any(|step| step.name == "protect-trunk");
    if needs {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                "protect-trunk refuses without --required-check, and nothing was written",
            )
            .expected("the name of the CI check the release merge must pass")
            .action(format!(
                "pass --required-check <name>; gh api repos/{}/commits/HEAD/check-runs lists the project's check names",
                ctx.repo
            ))
            .step("protect-trunk"),
        ));
    }
    Ok(())
}

/// `rk setup --list`: the ordered steps, what each proves, and which needs
/// input on which forge — visible before a first apply rather than
/// discovered by one.
fn list(forge: Option<&str>) -> Result<(), RkError> {
    let forge = forge
        .map(|name| {
            Forge::parse(name).ok_or_else(|| {
                RkError::Usage(format!(
                    "unknown forge '{name}'; the forges are: github, gitlab"
                ))
            })
        })
        .transpose()?;
    let out = Output::human();
    for (idx, step) in STEPS.iter().enumerate() {
        let mut line = format!(
            "{:2}. {} [{}] proves: {}",
            idx + 1,
            step.name,
            step.chapter,
            step.proves
        );
        if step.name == "protect-trunk" && forge != Some(Forge::Gitlab) {
            line.push_str(" (needs --required-check on github)");
        }
        if step.destructive {
            line.push_str(" (destructive)");
        }
        if step.optional {
            line.push_str(" (optional; a full apply skips it)");
        }
        out.result_line(line);
    }
    out.next(&[
        "rk setup --target . previews every step".to_owned(),
        "rk setup script <name> prints one embedded script".to_owned(),
    ]);
    Ok(())
}

/// `rk setup script <name>`: the audit escape hatch, printed byte-identical
/// to the embedded file.
fn script(name: &str, forge: Option<&str>) -> Result<(), RkError> {
    if name == "package-check" {
        return Err(RkError::Usage(
            "package-check reads its command from the technology binding and has no script".into(),
        ));
    }
    if name == "branch-reminder" {
        return Err(RkError::Usage(
            "branch-reminder writes an embedded hook body and has no script; rk setup step branch-reminder previews the write".into(),
        ));
    }
    let forge = match forge {
        Some(value) => Forge::parse(value).ok_or_else(|| {
            RkError::Usage(format!(
                "unknown forge '{value}'; the forges are: github, gitlab"
            ))
        })?,
        None => Forge::Github,
    };
    let path = format!("{}/{name}", forge.as_str());
    let file = embedded::SETUP.get_file(&path).ok_or(RkError::NotFound {
        kind: "setup step",
        name: name.to_owned(),
    })?;
    Output::human().result_raw(&String::from_utf8_lossy(file.contents()));
    Ok(())
}

/// The shared run state: the boundary, the resolved context, the journal,
/// and the event stream.
struct Engine {
    out: Output,
    ctx: Ctx,
    journal: Option<Journal>,
    secrets: Vec<Zeroizing<Vec<u8>>>,
    /// The run's one read of the named key file; see [`key_file_for`].
    key: Option<secrets::KeyFile>,
    /// The run's App JWT, minted at most once; see [`app_jwt_for`].
    app_jwt: Option<String>,
    seq: u64,
    command: &'static str,
    run_id: String,
}

impl Engine {
    /// Open the run: journal first, before any remote mutation. An apply
    /// that cannot create its journal refuses; observability-only modes
    /// warn and continue, because refusing them over observability is
    /// self-defeating.
    fn open(
        out: Output,
        ctx: Ctx,
        command: &'static str,
        journal_required: bool,
    ) -> Result<Self, RkError> {
        // A stale key export is refused wherever a run opens, so every
        // mode catches it and not the one step that would have used it.
        secrets::refuse_legacy_key()?;
        let journal =
            match Journal::create(command, ctx.target.as_str(), ctx.forge.as_str(), &ctx.repo) {
                Ok(journal) => Some(journal),
                Err(source) if journal_required => {
                    return Err(RkError::refusal(
                        Diagnostic::new(
                            Reason::JournalUnavailable,
                            format!("the run journal cannot be created: {source}"),
                        )
                        .expected("a writable state root for the journal")
                        .target_state("nothing was run and nothing changed"),
                    ));
                }
                Err(source) => {
                    out.warn(format!("no run journal for this run: {source}"));
                    None
                }
            };
        let run_id = journal
            .as_ref()
            .map_or_else(|| "unjournaled".to_owned(), |j| j.run_id().to_owned());
        let mut engine = Self {
            out,
            ctx,
            journal,
            secrets: Ctx::secret_values(),
            key: None,
            app_jwt: None,
            seq: 0,
            command,
            run_id,
        };
        let opening = Event::opening(
            engine.next_seq(),
            crate::applog::now_utc(),
            engine.run_id.clone(),
            engine.command,
        );
        engine.emit(&opening);
        if engine.ctx.self_hosted_gitlab() {
            engine.out.warn(
                "this remote is a self-hosted GitLab: registry trusted publishing covers GitLab.com only, so the OIDC invariant cannot be satisfied here",
            );
        }
        Ok(engine)
    }

    const fn next_seq(&mut self) -> u64 {
        let seq = self.seq;
        self.seq += 1;
        seq
    }

    fn event(&mut self, kind: EventKind, step: Option<&str>) -> Event {
        let mut event = Event::opening(
            self.next_seq(),
            crate::applog::now_utc(),
            self.run_id.clone(),
            self.command,
        );
        event.kind = kind;
        event.step = step.map(str::to_owned);
        event
    }

    fn emit(&mut self, event: &Event) {
        self.out.event(event);
        if let Some(journal) = &mut self.journal {
            if let Ok(line) = serde_json::to_string(event) {
                journal.event_line(&line);
            }
        }
    }

    /// Run one external command: echo it, stream and redact its output,
    /// journal everything, surface its exit.
    fn exec(&mut self, exec: &Exec, passthrough: bool) -> Result<Outcome, RkError> {
        let echo = exec.echo();
        self.out.frame(&echo);
        if let Some(journal) = &mut self.journal {
            journal.transcript(echo.as_bytes());
            journal.transcript(b"\n");
        }
        let secrets = std::mem::take(&mut self.secrets);
        let step_name: Option<String> = None;
        let mut chunks: Vec<(ChildStream, Vec<u8>)> = Vec::new();
        let spawned = process::run(exec, |stream, chunk| {
            chunks.push((stream, process::redact(chunk, &secrets)));
        });
        self.secrets = secrets;
        for (stream, chunk) in chunks {
            if passthrough {
                self.out.child_passthrough(stream, &chunk);
            }
            let event = self.event(EventKind::ChildOutput, step_name.as_deref());
            let event = event.child_output(stream, &chunk);
            self.emit(&event);
            if let Some(journal) = &mut self.journal {
                journal.transcript(&chunk);
            }
        }
        spawned.map_err(|source| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("{} did not spawn: {source}", exec.program.to_string_lossy()),
                )
                .expected("a POSIX sh and the forge CLI on PATH")
                .run(self.run_path()),
            )
        })
    }

    fn run_path(&self) -> String {
        self.journal.as_ref().map_or_else(
            || "no journal was written".to_owned(),
            |j| j.dir.display().to_string(),
        )
    }

    fn finish(&mut self, exit_code: i32, reason: Option<&str>) {
        let mut event = self.event(EventKind::RunFinished, None);
        event.exit_code = Some(exit_code);
        event.status = Some(if exit_code == 0 {
            "ok".into()
        } else {
            "failed".into()
        });
        self.emit(&event);
        if let Some(journal) = &mut self.journal {
            journal.finish(exit_code, reason);
        }
    }
}

/// Attach the failure to its run journal and close the run.
fn fail(engine: &mut Engine, error: RkError) -> RkError {
    let error = match error {
        RkError::Refusal(mut diagnostic) => {
            diagnostic.run.get_or_insert_with(|| engine.run_path());
            RkError::Refusal(diagnostic)
        }
        RkError::Subprocess(mut diagnostic) => {
            diagnostic.run.get_or_insert_with(|| engine.run_path());
            RkError::Subprocess(diagnostic)
        }
        RkError::CheckFailed(mut diagnostic) => {
            diagnostic.run.get_or_insert_with(|| engine.run_path());
            RkError::CheckFailed(diagnostic)
        }
        other => other,
    };
    engine.finish(i32::from(error.exit_code()), Some(error.reason().as_str()));
    error
}

/// Preview: walk the ordered steps and print, for each, the step name, what
/// it proves, and the resolved invocation — without materializing anything
/// and without invoking any external command. What preview shows is what
/// apply would run, because both read the same step table and environment
/// construction.
fn preview(out: Output, ctx: &Ctx, steps: &[&StepSpec]) -> Result<(), RkError> {
    let mut engine = Engine::open(out, clone_ctx(ctx), "setup preview", false)?;
    out.result_line(format!(
        "DRY RUN: rk setup would run these steps against {} on {}; re-run with --apply",
        engine.ctx.repo,
        engine.ctx.forge.as_str()
    ));
    for (idx, step) in steps.iter().enumerate() {
        out.result_line(format!(
            "step {}/{} {} — proves {}",
            idx + 1,
            steps.len(),
            step.name,
            step.proves
        ));
        // Preview is the rehearsal of apply, so a credential apply could
        // not use is a preview failure: the operator learns it here rather
        // than one flag later, and before an invocation is claimed.
        if step.name == "bot-secrets" && engine.ctx.forge == Forge::Github {
            secrets::resolve_key_file(&engine.ctx.target)?;
        }
        out.result_line(format!("  {}", render_invocation(&engine.ctx, step)));
        if step.name == "protect-trunk"
            && engine.ctx.forge == Forge::Github
            && engine.ctx.required_check.is_none()
        {
            out.result_line("  needs: --required-check <name> before apply");
        }
        if step.optional && steps.len() > 1 {
            out.result_line(format!(
                "  optional: a full apply skips it; rk setup step {} --apply runs it",
                step.name
            ));
        }
        let mut event = engine.event(EventKind::StepFinished, Some(step.name));
        event.status = Some("previewed".into());
        engine.emit(&event);
    }
    let next = next_for_apply(&engine.ctx, steps);
    out.next(&[
        next,
        "rk setup check --target . proves what is already true".to_owned(),
    ]);
    engine.finish(0, None);
    Ok(())
}

/// The one line preview prints per step: the exact spawn shape, with every
/// non-secret variable resolved.
fn render_invocation(ctx: &Ctx, step: &StepSpec) -> String {
    match step.name {
        "branch-reminder" => {
            "would write: the post-merge reminder hook at $(git rev-parse --git-path hooks)/post-merge".to_owned()
        }
        "package-check" => match ctx.tech {
            Some("rust") => "would run: cargo publish --dry-run --allow-dirty".to_owned(),
            Some("python") => "would run: python3 -m build".to_owned(),
            Some("bash") => "nothing to run: no registry for this technology".to_owned(),
            _ => "needs: a version file naming the technology".to_owned(),
        },
        name => {
            let check = ctx
                .required_check
                .as_ref()
                .filter(|_| ctx.forge == Forge::Github && name == "protect-trunk")
                .map(|value| format!(" RK_REQUIRED_CHECK={value}"))
                .unwrap_or_default();
            format!(
                "would run: sh <embedded setup/{}/{name}> with RK_REPO={} RK_TRUNK_BRANCH=master{check}",
                ctx.forge.as_str(),
                ctx.repo
            )
        }
    }
}

fn next_for_apply(ctx: &Ctx, steps: &[&StepSpec]) -> String {
    let check = ctx
        .required_check
        .as_ref()
        .map(|value| format!(" --required-check {value}"))
        .unwrap_or_default();
    if steps.len() == 1 {
        format!(
            "rk setup step {} --target {} --apply{check}",
            steps[0].name, ctx.target
        )
    } else {
        format!("rk setup --target {} --apply{check}", ctx.target)
    }
}

/// A `Ctx` copy for engine ownership; the context is plain data.
fn clone_ctx(ctx: &Ctx) -> Ctx {
    Ctx {
        target: ctx.target.clone(),
        repo: ctx.repo.clone(),
        forge: ctx.forge,
        host: ctx.host.clone(),
        required_check: ctx.required_check.clone(),
        cli: ctx.cli.clone(),
        tech: ctx.tech,
    }
}

/// Apply: run the selected steps in order, each through the full lifecycle.
fn execute(
    out: Output,
    ctx: Ctx,
    steps: &[&StepSpec],
    command: &'static str,
) -> Result<(), RkError> {
    guard_sh()?;
    let mut engine = Engine::open(out, ctx, command, true)?;
    let mut done: Vec<(String, String)> = Vec::new();
    for (idx, step) in steps.iter().enumerate() {
        // An optional step applies only by name: a full run states the skip
        // rather than acting on a condition the operator never asserted.
        if step.optional && steps.len() > 1 {
            engine.out.frame(format!(
                "step {}/{} {} — skipped (optional; rk setup step {} --apply runs it)",
                idx + 1,
                steps.len(),
                step.name,
                step.name
            ));
            let mut finished = engine.event(EventKind::StepFinished, Some(step.name));
            finished.status = Some("skipped".into());
            engine.emit(&finished);
            done.push((step.name.to_owned(), "skipped".to_owned()));
            continue;
        }
        engine.out.frame(format!(
            "step {}/{} {} — {}",
            idx + 1,
            steps.len(),
            step.name,
            step.proves
        ));
        let mut started = engine.event(EventKind::StepStarted, Some(step.name));
        started.status = Some("running".into());
        engine.emit(&started);
        let clock = Instant::now();
        let status = match apply_step(&mut engine, step) {
            Ok(status) => status,
            Err(error) => {
                let error = attach_progress(error, &done, step, steps);
                let mut finished = engine.event(EventKind::StepFinished, Some(step.name));
                finished.status = Some("failed".into());
                finished.reason = Some(error.reason());
                finished.duration_ms = Some(elapsed_ms(clock));
                engine.emit(&finished);
                return Err(fail(&mut engine, error));
            }
        };
        engine
            .out
            .frame(format!("ok {}: {}", step.name, status.line()));
        let mut finished = engine.event(EventKind::StepFinished, Some(step.name));
        finished.status = Some(status.wire().into());
        finished.exit_code = Some(0);
        finished.duration_ms = Some(elapsed_ms(clock));
        engine.emit(&finished);
        done.push((step.name.to_owned(), status.wire().to_owned()));
    }
    engine.out.result_line(format!(
        "setup: {} completed against {}",
        step_count(done.len()),
        engine.ctx.repo
    ));
    for (name, status) in &done {
        engine.out.result_line(format!("  {status} {name}"));
    }
    engine.out.next(&[
        format!("rk setup check --target {}", engine.ctx.target),
        "rk guide setup orders what no command performs".to_owned(),
    ]);
    engine.finish(0, None);
    Ok(())
}

/// A step count rendered with the noun that agrees with it, so no summary
/// line can regrow a dangling plural.
fn step_count(count: usize) -> String {
    format!("{count} {}", if count == 1 { "step" } else { "steps" })
}

fn elapsed_ms(clock: Instant) -> u64 {
    u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// What one applied step reported.
enum Done {
    /// The desired state already held; nothing ran.
    Satisfied(String),
    /// The script ran and the postcondition was read back.
    Changed(String, Option<String>),
    /// A read-only step ran and passed.
    Passed(String),
}

impl Done {
    const fn wire(&self) -> &'static str {
        match self {
            Self::Satisfied(_) => "satisfied",
            Self::Changed(..) => "applied",
            Self::Passed(_) => "passed",
        }
    }

    fn line(&self) -> String {
        match self {
            Self::Satisfied(detail) | Self::Passed(detail) => detail.clone(),
            Self::Changed(detail, limitation) => limitation.as_ref().map_or_else(
                || detail.clone(),
                |limit| format!("{detail} (limitation: {limit})"),
            ),
        }
    }
}

/// One step, full lifecycle.
#[allow(clippy::too_many_lines)]
fn apply_step(engine: &mut Engine, step: &StepSpec) -> Result<Done, RkError> {
    // Prerequisites are observed, not remembered: the forge is the
    // authority on whether an earlier step's state holds.
    for prereq in step.prereqs {
        let state = observe_with(engine, prereq)?;
        if !state.satisfied() {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    format!(
                        "{} requires {prereq} first: {}",
                        step.name,
                        state_detail(&state)
                    ),
                )
                .expected(format!("{prereq} satisfied before {}", step.name))
                .action(format!(
                    "rk setup step {prereq} --target {} --apply",
                    engine.ctx.target
                ))
                .step(step.name),
            ));
        }
    }
    match step.name {
        "package-check" => {
            if engine.ctx.tech.is_none() {
                return Err(RkError::Usage(
                    "no version file names a technology; rk binding --list names the bindings"
                        .into(),
                ));
            }
            let state = observe_with(engine, "package-check")?;
            match state {
                StepState::Satisfied { detail, .. } => Ok(Done::Passed(detail)),
                StepState::Unsatisfied { detail } | StepState::Inapplicable { detail } => {
                    Err(RkError::subprocess(
                        Diagnostic::new(
                            Reason::SubprocessFailed,
                            format!("package-check failed: {detail}"),
                        )
                        .expected(step.proves.to_owned())
                        .step(step.name),
                    ))
                }
                StepState::Unknown { detail } => Err(RkError::subprocess(
                    Diagnostic::new(
                        Reason::SubprocessFailed,
                        format!("package-check could not run: {detail}"),
                    )
                    .step(step.name),
                )),
            }
        }
        "branch-reminder" => {
            use crate::setup::branch_reminder::{HookState, hook_body, hook_path, observe_hook};
            match observe_hook(&engine.ctx.target) {
                HookState::Installed => Ok(Done::Satisfied(
                    "the post-merge reminder hook is installed".into(),
                )),
                HookState::Foreign => Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::StateDrift,
                        "a foreign post-merge hook exists; the reminder is never written over it",
                    )
                    .expected("no post-merge hook, or one carrying the release-kit marker")
                    .action(
                        "merge by hand: guard each call behind its own capability probe inside the existing hook — `rk branches prune --help >/dev/null 2>&1` before `rk branches prune --quiet || :`, and the same pair for `rk worktree prune`",
                    )
                    .target_state("unchanged")
                    .step(step.name),
                )),
                HookState::Unreadable(detail) => Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::StateDrift,
                        format!("the post-merge hook cannot be read: {detail}"),
                    )
                    .target_state("unchanged")
                    .step(step.name),
                )),
                HookState::Absent | HookState::Drifted => {
                    let path = hook_path(&engine.ctx.target).map_err(|detail| {
                        RkError::refusal(
                            Diagnostic::new(
                                Reason::PrerequisiteUnmet,
                                format!("the hooks directory cannot be resolved: {detail}"),
                            )
                            .expected("a git repository whose hooks directory git can name")
                            .step(step.name),
                        )
                    })?;
                    crate::atomic::write(&path, hook_body())?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt as _;
                        std::fs::set_permissions(
                            &path,
                            std::fs::Permissions::from_mode(0o755),
                        )?;
                    }
                    Ok(Done::Changed(
                        "wrote the post-merge reminder hook".into(),
                        None,
                    ))
                }
            }
        }
        "single-trunk" => {
            let guard = {
                let ctx = clone_ctx(&engine.ctx);
                let mut runner = |exec: &Exec| engine.exec(exec, false);
                observe::single_trunk_guard(&ctx, &mut runner)?
            };
            // A destructive step fails closed: an ancestry the guard cannot
            // establish is treated exactly like one it refuted.
            match &guard {
                StepState::Satisfied { .. } => {}
                StepState::Unsatisfied { detail }
                | StepState::Inapplicable { detail }
                | StepState::Unknown { detail } => {
                    return Err(RkError::refusal(
                        Diagnostic::new(
                            Reason::DestructiveRefusal,
                            format!("single-trunk refuses: {detail}"),
                        )
                        .expected(
                            "proof that every candidate branch is absent, or an ancestor of the trunk",
                        )
                        .step(step.name),
                    ));
                }
            }
            run_forge_step(engine, step)
        }
        "bot-secrets" => {
            // Validate before observing: a wrong path or a wrong mode is
            // the operator's answer either way, and the refusal costs no
            // forge call. Only GitHub reads a key file; GitLab's credential
            // is a token, and its step must not fail over a variable it
            // never consumes.
            let key = match engine.ctx.forge {
                // The run's one read: an install-bot observation earlier in
                // this run already holds the bytes, and this step stores
                // those very bytes rather than reopening the path.
                Forge::Github => key_file_for(engine)?.map(|key| key.bytes.clone()),
                Forge::Gitlab => None,
            };
            let provided = match engine.ctx.forge {
                // Both halves of an App identity, or neither: a run holding
                // only one of them would store half a credential.
                Forge::Github => secrets::value_of("RK_BOT_APP_ID").is_some() && key.is_some(),
                Forge::Gitlab => secrets::value_of("RK_BOT_TOKEN").is_some(),
            };
            let state = observe_with(engine, step.name)?;
            if !provided {
                if state.satisfied() {
                    return Ok(Done::Satisfied(state_detail(&state)));
                }
                let wanted = match engine.ctx.forge {
                    Forge::Github => {
                        "export RK_BOT_APP_ID and RK_BOT_PRIVATE_KEY_FILE, the second naming the .pem; rk forge github carries the walkthrough"
                    }
                    Forge::Gitlab => {
                        "rk setup step install-bot --apply stores the token, or export RK_BOT_TOKEN to rotate one"
                    }
                };
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::PrerequisiteUnmet,
                        "bot-secrets has no credentials to store",
                    )
                    .expected("the bot credentials in the environment, the key as a path")
                    .action(wanted.to_owned())
                    .step(step.name),
                ));
            }
            if let Some(journal) = &mut engine.journal {
                for name in SECRET_VARS {
                    if secrets::value_of(name).is_some() {
                        journal.record_secret(name, true, "environment");
                    }
                }
                if key.is_some() {
                    journal.record_secret(secrets::PRIVATE_KEY_FILE, true, "file");
                }
            }
            // The bytes rk validated are the bytes the child receives and
            // the bytes the redactor holds: one read, one value, so nothing
            // can be substituted between the check and the forge.
            let stdin = key;
            run_forge_step_with(engine, step, stdin, Vec::new())
        }
        "protections-check" => {
            let (outcome, _) = run_script(engine, step)?;
            if !outcome.success() {
                return Err(classify_failure(engine, step, &outcome));
            }
            // The script is the operator-auditable mirror; the observation
            // is the authoritative shape check, so the step passes only
            // when both agree.
            match observe_with(engine, step.name)? {
                StepState::Satisfied { detail, limitation } => {
                    Ok(Done::Passed(limitation.map_or_else(
                        || detail.clone(),
                        |limit| format!("{detail} (limitation: {limit})"),
                    )))
                }
                StepState::Unsatisfied { detail } | StepState::Inapplicable { detail } => {
                    Err(RkError::refusal(
                        Diagnostic::new(
                            Reason::StateDrift,
                            format!("protections-check passed its script and the observation disagrees: {detail}"),
                        )
                        .expected(step.proves.to_owned())
                        .step(step.name),
                    ))
                }
                // An unreadable readback is a retryable outage, not drift,
                // exactly as the postcondition lifecycle classifies it.
                StepState::Unknown { detail } => Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::ForgeTemporary,
                        format!(
                            "protections-check passed its script and the readback could not confirm it: {detail}"
                        ),
                    )
                    .expected(step.proves.to_owned())
                    .action("check authentication and connectivity, then rerun")
                    .step(step.name),
                )),
            }
        }
        // GitHub's grant is the one write the forge offers a command, and
        // it takes a user credential; everything else here — the pre- and
        // post-observation, and the installation id the script is handed —
        // happens as the App itself. GitLab's install-bot needs none of
        // this and takes the generic lifecycle below.
        "install-bot" if engine.ctx.forge == Forge::Github => {
            match observe_with(engine, step.name)? {
                StepState::Satisfied { detail, .. } => {
                    return Ok(Done::Satisfied(detail));
                }
                StepState::Unsatisfied { .. } | StepState::Inapplicable { .. } => {}
                StepState::Unknown { detail } => {
                    return Err(RkError::refusal(
                        Diagnostic::new(
                            Reason::ForgeTemporary,
                            format!("{} cannot observe the current state: {detail}", step.name),
                        )
                        .expected("a readable forge answer before anything mutates")
                        .action("check the App credentials and connectivity, then rerun")
                        .step(step.name),
                    ));
                }
            }
            let installation = github_installation_id(engine, step)?;
            run_forge_step_with(
                engine,
                step,
                None,
                vec![("RK_BOT_INSTALLATION".into(), installation.into())],
            )
        }
        _ => {
            if step.mutates == Mutates::Forge {
                // The lifecycle applies only on a state it has read: an
                // observation that cannot decide fails closed here exactly
                // as it does after the write, so no mutation ever rides on
                // an unreadable forge answer.
                match observe_with(engine, step.name)? {
                    StepState::Satisfied { detail, .. } => {
                        return Ok(Done::Satisfied(detail));
                    }
                    StepState::Unsatisfied { .. } | StepState::Inapplicable { .. } => {}
                    StepState::Unknown { detail } => {
                        return Err(RkError::refusal(
                            Diagnostic::new(
                                Reason::ForgeTemporary,
                                format!("{} cannot observe the current state: {detail}", step.name),
                            )
                            .expected("a readable forge answer before anything mutates")
                            .action("check authentication and connectivity, then rerun")
                            .step(step.name),
                        ));
                    }
                }
            }
            run_forge_step(engine, step)
        }
    }
}

/// The id of the App's installation on this repository's owner, read as
/// the App itself. The grant needs it, and no user credential can read
/// it: the observation that just ran answered 404, so the repository
/// endpoint that names the id directly has nothing to say yet. The
/// account-level installation is a direct read for either account kind —
/// the user endpoint answers for a person, the organization endpoint for
/// an organization — so nothing here lists or paginates.
fn github_installation_id(engine: &mut Engine, step: &StepSpec) -> Result<String, RkError> {
    let refuse = |message: String, action: &str| {
        RkError::refusal(
            Diagnostic::new(Reason::PrerequisiteUnmet, message)
                .expected("the App installed on the repository's owner")
                .action(action.to_owned())
                .step(step.name),
        )
    };
    let jwt = match app_jwt_for(engine)? {
        Ok(jwt) => jwt,
        Err(detail) => {
            return Err(refuse(
                format!("install-bot has no App token: {detail}"),
                app_jwt::REMEDIATION,
            ));
        }
    };
    let owner = engine
        .ctx
        .repo
        .split('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    let ctx = clone_ctx(&engine.ctx);
    for path in [
        format!("users/{owner}/installation"),
        format!("orgs/{owner}/installation"),
    ] {
        match app_jwt::api_get(&ctx, &jwt, &path) {
            AppApi::Ok(body) => {
                return body["id"].as_i64().map(|id| id.to_string()).ok_or_else(|| {
                    refuse(
                        format!("the forge answered {path} without an installation id"),
                        "check RK_BOT_APP_ID and the key file name the same App",
                    )
                });
            }
            AppApi::Missing => {}
            AppApi::Refused(detail) => {
                return Err(refuse(
                    detail,
                    "check RK_BOT_APP_ID and the key file name the same App",
                ));
            }
            AppApi::Failed(detail) => {
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::ForgeTemporary,
                        format!("install-bot cannot read the App's installation: {detail}"),
                    )
                    .action("check connectivity, then rerun")
                    .step(step.name),
                ));
            }
        }
    }
    Err(refuse(
        format!("the App has no installation on {owner}"),
        "install the App on the account first; the setup guide's step 5 walks it",
    ))
}

/// Materialize, spawn, classify, and verify one forge-mutating step.
fn run_forge_step(engine: &mut Engine, step: &StepSpec) -> Result<Done, RkError> {
    run_forge_step_with(engine, step, None, Vec::new())
}

/// The same, with bytes written to the step's standard input and values
/// `rk` derived added to its environment.
fn run_forge_step_with(
    engine: &mut Engine,
    step: &StepSpec,
    stdin: Option<Zeroizing<Vec<u8>>>,
    extra_env: Vec<(OsString, OsString)>,
) -> Result<Done, RkError> {
    let (outcome, _) = run_script_with(engine, step, stdin, extra_env)?;
    if !outcome.success() {
        return Err(classify_failure(engine, step, &outcome));
    }
    let state = observe_with(engine, step.name)?;
    match state {
        StepState::Satisfied { detail, limitation } => Ok(Done::Changed(detail, limitation)),
        StepState::Unsatisfied { detail } | StepState::Inapplicable { detail } => {
            Err(RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!(
                        "{} ran and its postcondition does not hold: {detail}",
                        step.name
                    ),
                )
                .expected(step.proves.to_owned())
                .step(step.name),
            ))
        }
        // The lifecycle ends with a proven postcondition; a readback that
        // cannot run leaves the step unproven, and an unproven apply is a
        // failure a retry can cure, never a success.
        StepState::Unknown { detail } => Err(RkError::refusal(
            Diagnostic::new(
                Reason::ForgeTemporary,
                format!(
                    "{} ran and the readback could not confirm it: {detail}",
                    step.name
                ),
            )
            .expected(step.proves.to_owned())
            .action(format!(
                "rk setup step {} --target {} --apply re-asserts and re-proves it",
                step.name, engine.ctx.target
            ))
            .step(step.name),
        )),
    }
}

/// Observe one step through the engine's executor.
///
/// `install-bot` on GitHub is the one observation that authenticates as
/// the App itself, so it routes through [`app_jwt_for`] here — where the
/// engine can mint once and register the redaction needles — rather than
/// through the credential-free name dispatch in [`observe::observe`].
fn observe_with(engine: &mut Engine, step: &str) -> Result<StepState, RkError> {
    if step == "install-bot" && engine.ctx.forge == Forge::Github {
        let jwt = match app_jwt_for(engine)? {
            Ok(jwt) => jwt,
            Err(detail) => return Ok(StepState::Unknown { detail }),
        };
        return Ok(observe::github_install_bot(&engine.ctx, &jwt));
    }
    let ctx = clone_ctx(&engine.ctx);
    let mut runner = |exec: &Exec| engine.exec(exec, false);
    observe::observe(&ctx, step, &mut runner)
}

/// The run's validated key file, read exactly once per run: the first
/// consumer resolves it and every later one reuses the same bytes, so the
/// file that authenticated the App is the file `bot-secrets` stores, and
/// no replacement between steps can split the two. The bytes become a
/// redaction needle the moment they are read.
fn key_file_for(engine: &mut Engine) -> Result<Option<&secrets::KeyFile>, RkError> {
    if engine.key.is_none() {
        engine.key = secrets::resolve_key_file(&engine.ctx.target)?;
        if let Some(key) = &engine.key {
            engine.secrets.push(key.bytes.clone());
        }
    }
    Ok(engine.key.as_ref())
}

/// The run's App JWT, minted at most once: the key comes from the run's
/// one read, and the minted token and its signature segment become
/// redaction needles before anything else spawns. Every install-bot
/// observation and the grant's installation-id discovery reuse the one
/// token, whose nine-minute life covers a run's contiguous step easily.
///
/// The inner value is `Err` with a one-line detail where no token can
/// exist — absent exports, or a signer that failed — which an observation
/// reports as `unknown` and an apply turns into a refusal.
fn app_jwt_for(engine: &mut Engine) -> Result<Result<String, String>, RkError> {
    if let Some(jwt) = &engine.app_jwt {
        return Ok(Ok(jwt.clone()));
    }
    let app_id = app_jwt::app_id()?;
    let key_bytes = key_file_for(engine)?.map(|key| key.bytes.clone());
    let (Some(app_id), Some(key_bytes)) = (app_id, key_bytes) else {
        return Ok(Err(format!(
            "the installation is readable only to the App itself; {}",
            app_jwt::REMEDIATION
        )));
    };
    let credentials = app_jwt::AppCredentials { app_id, key_bytes };
    let ctx = clone_ctx(&engine.ctx);
    Ok(match app_jwt::mint(&ctx, &credentials) {
        Ok(jwt) => {
            engine
                .secrets
                .push(Zeroizing::new(jwt.clone().into_bytes()));
            if let Some(signature) = jwt.rsplit('.').next() {
                engine
                    .secrets
                    .push(Zeroizing::new(signature.as_bytes().to_vec()));
            }
            engine.app_jwt = Some(jwt.clone());
            Ok(jwt)
        }
        Err(detail) => Err(detail),
    })
}

fn state_detail(state: &StepState) -> String {
    match state {
        StepState::Satisfied { detail, .. }
        | StepState::Unsatisfied { detail }
        | StepState::Inapplicable { detail }
        | StepState::Unknown { detail } => detail.clone(),
    }
}

/// Materialize the step's script into the run's private directory, prove
/// the written bytes by digest, and spawn it through the interpreter.
fn run_script(engine: &mut Engine, step: &StepSpec) -> Result<(Outcome, PathBuf), RkError> {
    run_script_with(engine, step, None, Vec::new())
}

/// The same, with bytes written to the script's standard input.
///
/// A credential travels this way and no other: `rk` reads it, validates it,
/// and hands the child the bytes it validated, so nothing between the check
/// and the forge can substitute a different file.
fn run_script_with(
    engine: &mut Engine,
    step: &StepSpec,
    stdin: Option<Zeroizing<Vec<u8>>>,
    extra_env: Vec<(OsString, OsString)>,
) -> Result<(Outcome, PathBuf), RkError> {
    let rel = format!("{}/{}", engine.ctx.forge.as_str(), step.name);
    let bytes = embedded::SETUP
        .get_file(&rel)
        .map(include_dir::File::contents)
        .ok_or_else(|| RkError::Other(anyhow::anyhow!("no embedded script at setup/{rel}")))?;
    let journal = engine
        .journal
        .as_mut()
        .ok_or_else(|| RkError::Other(anyhow::anyhow!("an apply always has a journal")))?;
    let dir = journal.scripts_dir().join(engine.ctx.forge.as_str());
    fs::create_dir_all(&dir)?;
    restrict(&dir, 0o700);
    let path = dir.join(step.name);
    fs::write(&path, bytes)?;
    restrict(&path, 0o600);
    let written = fs::read(&path)?;
    let digest = Digest::of(&written);
    if digest != Digest::of(bytes) {
        return Err(RkError::Other(anyhow::anyhow!(
            "the materialized script at {} differs from the embedded bytes",
            path.display()
        )));
    }
    journal.record_script(format!("scripts/{rel}"), digest.to_string());
    let mut env = engine.ctx.child_env(step.name);
    env.extend(extra_env);
    let exec = Exec {
        program: crate::probes::sh_bin(),
        args: vec![path.clone().into_os_string()],
        env,
        cwd: engine.ctx.target.as_std_path().to_path_buf(),
        stdin,
    };
    let outcome = engine.exec(&exec, true)?;
    Ok((outcome, path))
}

/// Honest classification: `gh` documents exit 4 as authentication required;
/// beyond that only an HTTP status in the response says more, and a step
/// that fails for a reason nothing establishes stays `subprocess-failed`
/// with its own stderr surfaced verbatim.
fn classify_failure(engine: &Engine, step: &StepSpec, outcome: &Outcome) -> RkError {
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    // A signalled child usually writes nothing before it dies, and
    // "no output" would blame the forge for a kill that came from
    // outside it. The adapter already resolved 128+N; say which.
    let last = if outcome.exit_code >= 128 {
        format!("killed by signal {}", outcome.exit_code - 128)
    } else {
        stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("no output")
            .to_owned()
    };
    let reason = if (engine.ctx.forge == Forge::Github && outcome.exit_code == 4)
        || stderr.contains("HTTP 401")
    {
        Reason::ForgeAuthentication
    } else if stderr.contains("HTTP 403") {
        Reason::ForgePermission
    } else if stderr.contains("HTTP 429") || stderr.contains("rate limit") {
        Reason::ForgeRateLimit
    } else {
        Reason::SubprocessFailed
    };
    let diagnostic = Diagnostic::new(reason, format!("the forge refused '{}': {last}", step.name))
        .expected(step.proves.to_owned())
        .action(format!(
            "rk setup step {} --target {} --apply",
            step.name, engine.ctx.target
        ))
        .step(step.name);
    let diagnostic = match reason {
        Reason::ForgePermission => diagnostic.expected(format!(
            "repository administration write on {} for the authenticated account",
            engine.ctx.repo
        )),
        _ => diagnostic,
    };
    match reason {
        Reason::SubprocessFailed => RkError::subprocess(diagnostic),
        _ => RkError::refusal(diagnostic),
    }
}

/// Fold the run's progress into the failure, so the diagnostic answers
/// what state the target is in.
fn attach_progress(
    error: RkError,
    done: &[(String, String)],
    failed: &StepSpec,
    steps: &[&StepSpec],
) -> RkError {
    let remaining = steps.len().saturating_sub(done.len() + 1);
    let state = format!(
        "{} completed; {} failed; {remaining} not attempted",
        step_count(done.len()),
        failed.name
    );
    match error {
        RkError::Refusal(mut diagnostic) => {
            diagnostic.target_state.get_or_insert(state);
            RkError::Refusal(diagnostic)
        }
        RkError::Subprocess(mut diagnostic) => {
            diagnostic.target_state.get_or_insert(state);
            RkError::Subprocess(diagnostic)
        }
        other => other,
    }
}

/// `rk setup check`: observe and verify every step, report per step, and
/// judge at the end. The mutating half is unreachable from this path: it
/// calls only the observe functions.
fn check(out: Output, ctx: Ctx) -> Result<(), RkError> {
    let mut engine = Engine::open(out, ctx, "setup check", false)?;
    let mut unsatisfied = 0usize;
    let mut unverifiable = 0usize;
    for step in &STEPS {
        let clock = Instant::now();
        let state = observe_with(&mut engine, step.name)?;
        let (label, wire) = match &state {
            StepState::Satisfied { .. } => ("ok", "satisfied"),
            // An optional step whose condition does not hold is stated, not
            // judged: nothing is wrong and nothing was skipped silently.
            StepState::Inapplicable { .. } => ("skipped", "skipped"),
            StepState::Unsatisfied { .. } => {
                unsatisfied += 1;
                ("unsatisfied", "unsatisfied")
            }
            // A step the check cannot verify has not passed: an unreadable
            // forge answer must never read as a clean setup.
            StepState::Unknown { .. } => {
                unverifiable += 1;
                ("unknown", "unknown")
            }
        };
        let mut line = format!("{label} {} — {}", step.name, state_detail(&state));
        if let StepState::Satisfied {
            limitation: Some(limit),
            ..
        } = &state
        {
            use std::fmt::Write as _;
            let _ = write!(line, " (limitation: {limit})");
        }
        engine.out.result_line(line);
        let mut finished = engine.event(EventKind::StepFinished, Some(step.name));
        finished.status = Some(wire.into());
        finished.duration_ms = Some(elapsed_ms(clock));
        engine.emit(&finished);
    }
    if unsatisfied > 0 || unverifiable > 0 {
        let error = RkError::check_failed(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "{} {} not satisfied and {unverifiable} could not be verified",
                    step_count(unsatisfied),
                    if unsatisfied == 1 { "is" } else { "are" }
                ),
            )
            .expected("every step's proof column to hold and to be readable")
            .action(format!(
                "rk setup --target {} --apply re-asserts them",
                engine.ctx.target
            )),
        );
        return Err(fail(&mut engine, error));
    }
    engine
        .out
        .next(&["rk guide release orders the first release".to_owned()]);
    engine.finish(0, None);
    Ok(())
}

/// Restrict a materialized path's mode: data, not an executable — nothing
/// ever executes a script directly, so no mode is load-bearing.
fn restrict(path: &std::path::Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

/// A POSIX shell must spawn before anything else does; every step runs
/// through it.
fn guard_sh() -> Result<(), RkError> {
    let ok = std::process::Command::new(crate::probes::sh_bin())
        .args(["-c", "exit 0"])
        .status()
        .is_ok_and(|status| status.success());
    if ok {
        Ok(())
    } else {
        Err(RkError::refusal(
            Diagnostic::new(Reason::PrerequisiteUnmet, "no POSIX sh runs on this host")
                .expected("a working sh on PATH; every step spawns through it")
                .action("install a POSIX shell, then rerun")
                .target_state("nothing was run and nothing changed"),
        ))
    }
}

#[cfg(test)]
mod tests {
    /// Every summary line reports its count through one helper, so none of
    /// them can regrow a dangling plural.
    #[test]
    fn a_step_count_carries_a_noun_that_agrees_with_it() {
        assert_eq!(super::step_count(0), "0 steps");
        assert_eq!(super::step_count(1), "1 step");
        assert_eq!(super::step_count(2), "2 steps");
    }
}
