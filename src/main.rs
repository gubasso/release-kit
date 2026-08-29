//! Machine-facing binary entry point.
//!
//! `main` is the process boundary and nothing else: it prints one error
//! line and classifies it once through the exit-code matrix. Dispatch lives
//! in `run`; behavior lives in `commands`.

use std::process::ExitCode;

use clap::Parser;
use release_kit::cli::{Cli, Commands};
use release_kit::commands;
use release_kit::error::RkError;

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
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
        Commands::License => commands::license::run(),
        Commands::Completions(args) => commands::completions::run(args),
    }
}
