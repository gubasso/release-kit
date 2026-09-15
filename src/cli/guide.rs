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

    /// Select the release driver's lines where the runbook branches;
    /// defaults to the configured or recorded driver, then to detection
    /// from the version files.
    #[arg(long, alias = "tech", value_name = "NAME")]
    pub technology: Option<String>,

    /// Select the forge's lines where the runbook branches; defaults to
    /// detection from the git remote.
    #[arg(long)]
    pub forge: Option<String>,

    /// The project path substituted for <repo>; defaults to detection from
    /// the git remote.
    #[arg(long)]
    pub repo: Option<String>,

    /// Select the checkout mode's lines where the runbook branches:
    /// linked-worktree or main-worktree. Defaults to the mode the
    /// configuration or the landing record states; without either, every
    /// variant prints with its label.
    #[arg(long, alias = "workflow", value_name = "MODE")]
    pub checkout_mode: Option<String>,

    /// Select the release style's lines where the runbook branches: trunk
    /// or lines. Defaults to the style the configuration or the landing
    /// record states; without either, every variant prints with its label.
    #[arg(long, alias = "style", value_name = "STYLE")]
    pub release_style: Option<String>,

    /// Select the integration mode's lines where the runbook branches:
    /// local or forge. Defaults to the mode the configuration or the
    /// landing record states; without either, every variant prints with
    /// its label.
    #[arg(long, value_name = "MODE")]
    pub integration: Option<String>,
}
