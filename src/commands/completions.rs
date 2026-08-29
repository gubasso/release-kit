//! `rk completions`: shell completion generation.

use clap::CommandFactory;

use crate::cli::Cli;
use crate::cli::completions::CompletionsArgs;
use crate::error::RkError;
use crate::output::Output;

/// Generate completions for the requested shell on stdout.
///
/// The script is generated into a buffer and emitted through the output
/// boundary, so the one command whose result clap produces still follows
/// the same stream and failure policy as every other.
///
/// # Errors
///
/// Never fails; the signature matches the dispatch table.
pub fn run(args: &CompletionsArgs) -> Result<(), RkError> {
    let mut script = Vec::new();
    clap_complete::generate(args.shell, &mut Cli::command(), "rk", &mut script);
    Output::human().result_bytes(&script);
    Ok(())
}
