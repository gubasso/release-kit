//! Machine-facing binary entry point.
//!
//! `main` is the process boundary and nothing else: it renders one error
//! through the output boundary, classifies it once through the exit-code
//! matrix, and leaves one application-log record per invocation. Dispatch
//! lives in `run`; behavior lives in `commands`.

use std::process::ExitCode;

use clap::Parser;
use release_kit::cli::{Cli, Commands};
use release_kit::error::RkError;
use release_kit::{applog, commands, output};

fn main() -> ExitCode {
    // `try_parse` instead of `parse`, so an argument error goes through the
    // same contract as every other failure: exit 64 from the matrix, a
    // diagnostic on stderr, and an application-log record — clap's own
    // process exit would bypass all three.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return parse_failure(&err),
    };
    let started = std::time::Instant::now();
    // A run whose work succeeded but whose stdout died mid-result still
    // failed to deliver, so the retained boundary failure becomes the
    // run's one typed error — before the log record, so the log agrees
    // with the exit. A handler's own error takes precedence.
    let result = match (run(&cli), output::take_stdout_failure()) {
        (Ok(()), Some(source)) => Err(RkError::Io(source)),
        (outcome, _) => outcome,
    };
    let status = result
        .as_ref()
        .map_or_else(|e| e.reason().as_str(), |()| "ok");
    applog::record(name(&cli.command), status, started.elapsed().as_millis());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            output::render_error(&e, wants_json(&cli.command));
            ExitCode::from(e.exit_code())
        }
    }
}

fn run(cli: &Cli) -> Result<(), RkError> {
    match &cli.command {
        Commands::Method(args) => commands::read::method(args),
        Commands::Binding(args) => commands::read::binding(args),
        Commands::Snippet(args) => commands::read::snippet(args),
        Commands::Guide(args) => commands::guide::run(args),
        Commands::Forge(args) => commands::forge::run(args),
        Commands::Versions => commands::versions::run(),
        Commands::Payload(args) => commands::payload::run(args),
        Commands::Init(args) => commands::init::run(args),
        Commands::Setup(args) => commands::setup::run(args),
        Commands::Runs(args) => commands::runs::run(args),
        Commands::Skill(args) => commands::skill::run(args),
        Commands::Doctor(args) => commands::doctor::run(args),
        Commands::Usage => commands::usage::run(),
        Commands::License => commands::license::run(),
        Commands::Completions(args) => commands::completions::run(args),
    }
}

/// Render an argument-parsing failure through the output contract.
///
/// Help and version are successes clap merely reports through its error
/// type; a real usage error keeps clap's helpful human rendering, maps to
/// exit 64 like every other usage failure, and — when the raw arguments
/// asked for `--json` — reports as one diagnostic line instead.
fn parse_failure(err: &clap::Error) -> ExitCode {
    if err.exit_code() == 0 {
        let _ = err.print();
        applog::record("parse", "ok", 0);
        return ExitCode::SUCCESS;
    }
    // `args_os`, not `args`: the argument that failed to parse may itself
    // be invalid Unicode, and this handler must not panic on the very
    // input it exists to report.
    if std::env::args_os().any(|arg| arg == std::ffi::OsStr::new("--json")) {
        let message = err
            .to_string()
            .lines()
            .next()
            .unwrap_or("invalid arguments")
            .trim_start_matches("error: ")
            .to_owned();
        output::render_error(&RkError::Usage(message), true);
    } else {
        let _ = err.print();
    }
    applog::record("parse", "usage", 0);
    ExitCode::from(64)
}

/// The subcommand's log name.
const fn name(command: &Commands) -> &'static str {
    match command {
        Commands::Method(_) => "method",
        Commands::Binding(_) => "binding",
        Commands::Snippet(_) => "snippet",
        Commands::Guide(_) => "guide",
        Commands::Forge(_) => "forge",
        Commands::Versions => "versions",
        Commands::Payload(_) => "payload",
        Commands::Init(_) => "init",
        Commands::Setup(_) => "setup",
        Commands::Runs(_) => "runs",
        Commands::Skill(_) => "skill",
        Commands::Doctor(_) => "doctor",
        Commands::Usage => "usage",
        Commands::License => "license",
        Commands::Completions(_) => "completions",
    }
}

/// Whether the invocation asked for machine output, which decides how an
/// error renders on stderr.
const fn wants_json(command: &Commands) -> bool {
    use release_kit::cli::runs::RunsAction;
    use release_kit::cli::setup::SetupAction;
    use release_kit::cli::skill::SkillAction;
    match command {
        Commands::Payload(args) => args.json,
        Commands::Init(args) => args.json,
        Commands::Doctor(args) => args.json,
        Commands::Setup(args) => match &args.action {
            Some(SetupAction::Check { json, .. } | SetupAction::Step { json, .. }) => *json,
            Some(SetupAction::Script { .. }) => false,
            None => args.json,
        },
        Commands::Runs(args) => match &args.action {
            RunsAction::List { json } | RunsAction::Show { json, .. } => *json,
            RunsAction::Prune { .. } => false,
        },
        Commands::Skill(args) => match &args.action {
            SkillAction::Install { json, .. } | SkillAction::Uninstall { json, .. } => *json,
            SkillAction::List | SkillAction::Show { .. } => false,
        },
        _ => false,
    }
}
