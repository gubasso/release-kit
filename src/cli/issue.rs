//! Arguments for `rk issue`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Start work from a forge issue, with the forge naming the branch.
#[derive(Debug, Args)]
pub struct IssueArgs {
    /// What to do with the issue.
    #[command(subcommand)]
    pub action: IssueAction,
}

/// The issue verbs. Unlike the worktree verbs, these read the recorded
/// workflow mode: starting from an issue seats a worktree under one mode
/// and checks out in place under the other.
#[derive(Debug, Subcommand)]
pub enum IssueAction {
    /// Mint the issue's branch at the forge and seat it; preview by
    /// default.
    Start {
        /// The issue: a number, `#<number>`, or the forge's issue URL.
        issue: String,

        /// The repository to act on; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Override the detected forge: github or gitlab.
        #[arg(long)]
        forge: Option<String>,

        /// Override the detected project path (owner/name).
        #[arg(long)]
        repo: Option<String>,

        /// Override the recorded workflow mode: worktree or branches.
        #[arg(long)]
        workflow: Option<String>,

        /// The remote branch the new branch starts from; the forge's
        /// default branch where absent. GitHub takes a remote branch
        /// name alone; GitLab also takes a commit SHA.
        #[arg(long)]
        base: Option<String>,

        /// Mint and seat; without it the intent is reported and nothing
        /// is touched, locally or on the forge.
        #[arg(long)]
        apply: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
