//! The flags every verb that resolves a target configuration takes: the
//! project profile and the Git workflow, as `rk init`, `rk adopt`,
//! `rk stage`, and `rk profile` read them.
//!
//! The canonical names are the long forms below. Three older spellings
//! stay as hidden aliases, so an existing command line keeps working:
//! `--tech` for one `--technology`, `--style` for `--release-style`, and
//! `--workflow` for `--checkout-mode`, whose older values `worktree` and
//! `branches` the parser still reads.

use clap::Args;

/// The profile and Git workflow answers an invocation states.
#[derive(Debug, Clone, Default, Args)]
pub struct ProfileFlags {
    /// A technology present in the project; repeat it for several. A
    /// supplied list replaces the configured one whole. Defaults to
    /// detection from the version files.
    #[arg(long = "technology", alias = "tech", value_name = "NAME")]
    pub technology: Vec<String>,

    /// The forge hosting the project: github or gitlab. Defaults to
    /// detection from the target's git remote; a project with no forge
    /// resolves to none.
    #[arg(long)]
    pub forge: Option<String>,

    /// The project path on the forge, substituted into the rendered files
    /// and recorded as the landing parameter. Defaults to detection from
    /// the target's git remote.
    #[arg(long)]
    pub repo: Option<String>,

    /// The release intent: automatic, where release-kit drives the
    /// release; external, where the project releases through a process
    /// release-kit does not drive; or none. Defaults to the observation's
    /// proposal: automatic with the one release-bearing technology, none
    /// where there is none.
    #[arg(long, value_name = "MODE")]
    pub release_mode: Option<String>,

    /// The technology that states the version and takes the bot, for an
    /// automatic release. Required where more than one release-bearing
    /// technology is present.
    #[arg(long, value_name = "NAME")]
    pub release_driver: Option<String>,

    /// The release style of an automatic release: trunk (the bot's
    /// request carries auto-merge from creation) or lines (every request
    /// waits for a human's merge).
    #[arg(long, alias = "style", value_name = "STYLE")]
    pub release_style: Option<String>,

    /// The one permanent branch. Defaults to the configuration, the
    /// record, and then master.
    #[arg(long, value_name = "BRANCH")]
    pub trunk: Option<String>,

    /// Where a topic branch opens: linked-worktree (every code-changing
    /// branch in its own linked worktree, the main checkout commits
    /// nothing) or main-worktree (the original working tree switches to
    /// it). Recorded as a Git workflow parameter and rendered into the
    /// landed blocks.
    #[arg(long, alias = "workflow", value_name = "MODE")]
    pub checkout_mode: Option<String>,

    /// Which authority moves an implementation onto the trunk: local
    /// (the checkout squashes and records it, through `rk integrate`) or
    /// forge (a pull request or merge request does, behind its required
    /// check). Recorded as a Git workflow parameter and rendered into the
    /// landed blocks. Defaults to local.
    #[arg(long, value_name = "MODE")]
    pub integration: Option<String>,
}

impl ProfileFlags {
    /// The parsed answers, as the resolution takes them.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::RkError::Usage`] for a value outside its
    /// vocabulary.
    pub fn inputs(&self) -> Result<crate::profile::Inputs<'_>, crate::error::RkError> {
        use crate::landing::{CheckoutMode, Integration, Style};
        use crate::profile::ReleaseMode;
        Ok(crate::profile::Inputs {
            technologies: &self.technology,
            forge: self.forge.as_deref(),
            repo: self.repo.as_deref(),
            release_mode: self
                .release_mode
                .as_deref()
                .map(ReleaseMode::parse)
                .transpose()?,
            release_driver: self.release_driver.as_deref(),
            style: self
                .release_style
                .as_deref()
                .map(Style::parse)
                .transpose()?,
            trunk: self.trunk.as_deref(),
            checkout_mode: self
                .checkout_mode
                .as_deref()
                .map(CheckoutMode::parse)
                .transpose()?,
            integration: self
                .integration
                .as_deref()
                .map(Integration::parse)
                .transpose()?,
            ..crate::profile::Inputs::default()
        })
    }
}
