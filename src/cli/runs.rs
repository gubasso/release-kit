//! Arguments for `rk runs`.

use clap::{Args, Subcommand};

/// Inspect and prune the run journals under the state root.
#[derive(Debug, Args)]
pub struct RunsArgs {
    /// What to do with the journals.
    #[command(subcommand)]
    pub action: RunsAction,
}

/// The journal verbs.
#[derive(Debug, Subcommand)]
pub enum RunsAction {
    /// List the kept runs, oldest first.
    List {
        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
    /// Print one run's record and where its files live.
    Show {
        /// The run id, from `rk runs list`.
        id: String,
        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
    /// Remove old runs past the retention bound.
    Prune {
        /// How many newest runs to keep; defaults to the retention bound.
        #[arg(long)]
        keep: Option<usize>,
    },
}
