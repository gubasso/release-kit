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
    let cli = Cli::parse();
    let started = std::time::Instant::now();
    let result = run(&cli);
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
        Commands::Versions => commands::versions::run(),
        Commands::Payload(args) => commands::payload::run(args),
        Commands::Init(args) => commands::init::run(args),
        Commands::Skill(args) => commands::skill::run(args),
        Commands::Doctor(args) => commands::doctor::run(args),
        Commands::Usage => commands::usage::run(),
        Commands::License => commands::license::run(),
        Commands::Completions(args) => commands::completions::run(args),
    }
}

/// The subcommand's log name.
const fn name(command: &Commands) -> &'static str {
    match command {
        Commands::Method(_) => "method",
        Commands::Binding(_) => "binding",
        Commands::Snippet(_) => "snippet",
        Commands::Versions => "versions",
        Commands::Payload(_) => "payload",
        Commands::Init(_) => "init",
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
    use release_kit::cli::skill::SkillAction;
    match command {
        Commands::Payload(args) => args.json,
        Commands::Init(args) => args.json,
        Commands::Doctor(args) => args.json,
        Commands::Skill(args) => match &args.action {
            SkillAction::Install { json, .. } | SkillAction::Uninstall { json, .. } => *json,
            SkillAction::List | SkillAction::Show { .. } => false,
        },
        _ => false,
    }
}
