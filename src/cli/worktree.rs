//! Arguments for `rk worktree`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Inspect, create, and prune the linked worktrees beside a checkout.
#[derive(Debug, Args)]
pub struct WorktreeArgs {
    /// What to do with the worktrees.
    #[command(subcommand)]
    pub action: WorktreeAction,
}

/// The worktree verbs, mode-free by design: they behave identically under
/// the worktree and branches workflows.
#[derive(Debug, Subcommand)]
pub enum WorktreeAction {
    /// Report every worktree of the repository, offline.
    List {
        /// The repository to read; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Create or adopt one branch's worktree at the sibling path
    /// `../<project>-<flattened branch>`; preview by default.
    Add {
        /// The branch to seat: an existing local branch is adopted, a
        /// lone matching remote tip becomes a tracking branch, and
        /// anything else is created from --base or the refreshed trunk.
        branch: String,

        /// The repository to act on; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// The commit-ish a new branch starts from; required for a
        /// release/* line, which is cut from a tag and never the tip.
        #[arg(long)]
        base: Option<String>,

        /// Create the worktree; without it the intent is reported and
        /// nothing is touched.
        #[arg(long)]
        apply: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
