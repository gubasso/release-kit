//! `rk completions`: shell completion generation.

use clap::CommandFactory;

use crate::cli::Cli;
use crate::cli::completions::CompletionsArgs;
use crate::error::RkError;

/// Generate completions for the requested shell on stdout.
///
/// # Errors
///
/// Never fails; the signature matches the dispatch table.
pub fn run(args: &CompletionsArgs) -> Result<(), RkError> {
    clap_complete::generate(
        args.shell,
        &mut Cli::command(),
        "rk",
        &mut std::io::stdout(),
    );
    Ok(())
}
