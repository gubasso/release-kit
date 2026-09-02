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
    /// `../<project>@<flattened branch>`; preview by default.
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

    /// Report the worktrees a squash merge retired — stale records and
    /// gone upstreams, never healthy seats — and remove only the
    /// forge-confirmed ones, and only under --apply.
    Prune {
        /// The repository to read; any of its worktrees names it.
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
        /// removing nothing.
        #[arg(long)]
        verify: bool,

        /// Confirm each candidate, then remove the worktree before its
        /// branch, and clear the stale records.
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
