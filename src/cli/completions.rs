//! Arguments for `rk completions`.

use clap::Args;
use clap_complete::Shell;

/// Generate shell completions on stdout.
#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// The shell to generate for.
    pub shell: Shell,
}
