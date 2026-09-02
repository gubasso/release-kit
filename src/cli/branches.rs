//! Arguments for `rk branches`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Report and prune local branches the forge already merged.
#[derive(Debug, Args)]
pub struct BranchesArgs {
    /// What to do with the local branches.
    #[command(subcommand)]
    pub action: BranchesAction,
}

/// The local-branch verbs.
#[derive(Debug, Subcommand)]
pub enum BranchesAction {
    /// Report the local branches whose remote branch is gone; delete only
    /// the forge-confirmed ones, and only under --apply.
    Prune {
        /// The repository to read.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Override the detected project path (owner/name) for the forge
        /// confirmation.
        #[arg(long)]
        repo: Option<String>,

        /// Override the detected forge: github or gitlab.
        #[arg(long)]
        forge: Option<String>,

        /// Confirm each candidate against the forge's merged requests,
        /// deleting nothing.
        #[arg(long)]
        verify: bool,

        /// Confirm each candidate, then delete the confirmed branches
        /// with git branch -D.
        #[arg(long)]
        apply: bool,

        /// Print nothing when there is nothing to report, for the
        /// post-merge hook.
        #[arg(long, conflicts_with = "json")]
        quiet: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
