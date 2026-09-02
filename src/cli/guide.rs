//! Arguments for `rk guide`.

use clap::Args;

/// Print a runbook with what detection knows filled in, or list them.
#[derive(Debug, Args)]
pub struct GuideArgs {
    /// The runbook to print; omit it and pass --list to see the names.
    pub name: Option<String>,

    /// List the runbooks instead of printing one.
    #[arg(long)]
    pub list: bool,

    /// Select the technology's lines where the runbook branches; defaults
    /// to detection from the version file.
    #[arg(long)]
    pub tech: Option<String>,

    /// Select the forge's lines where the runbook branches; defaults to
    /// detection from the git remote.
    #[arg(long)]
    pub forge: Option<String>,

    /// The project path substituted for <repo>; defaults to detection from
    /// the git remote.
    #[arg(long)]
    pub repo: Option<String>,

    /// Select the workflow mode's lines where the runbook branches:
    /// worktree or branches. Defaults to the mode the landing record
    /// states; without a record, every variant prints with its label.
    #[arg(long)]
    pub workflow: Option<String>,
}
