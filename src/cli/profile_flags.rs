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

    /// How the commits of a branch reach the trunk: squash (the branch
    /// arrives as one commit) or rebase (the branch is rebased on the
    /// trunk tip and the trunk fast-forwards to it, every commit keeping
    /// its own message). Recorded as a Git workflow parameter. Defaults
    /// to squash.
    #[arg(long, value_name = "FORM")]
    pub merge_form: Option<String>,

    /// The check the release gate believes: the context the trunk ruleset
    /// requires under forge integration, and the context the rendered
    /// release gate judges under local integration. GitHub only.
    #[arg(long, value_name = "NAME")]
    pub required_check: Option<String>,

    /// The workflow whose completion wakes the release gate under local
    /// integration, named by its `name:` key rather than its filename.
    /// GitHub only.
    #[arg(long, value_name = "NAME")]
    pub required_workflow: Option<String>,
}

impl ProfileFlags {
    /// Whether no flag answered anything.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.technology.is_empty()
            && self.forge.is_none()
            && self.repo.is_none()
            && self.release_mode.is_none()
            && self.release_driver.is_none()
            && self.release_style.is_none()
            && self.trunk.is_none()
            && self.checkout_mode.is_none()
            && self.integration.is_none()
            && self.merge_form.is_none()
            && self.required_check.is_none()
            && self.required_workflow.is_none()
    }

    /// The parsed answers, as the resolution takes them.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::RkError::Usage`] for a value outside its
    /// vocabulary.
    pub fn inputs(&self) -> Result<crate::profile::Inputs<'_>, crate::error::RkError> {
        use crate::landing::{CheckoutMode, Integration, MergeForm, Style};
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
            merge_form: self
                .merge_form
                .as_deref()
                .map(MergeForm::parse)
                .transpose()?,
            required_check: self.required_check.as_deref(),
            required_workflow: self.required_workflow.as_deref(),
            ..crate::profile::Inputs::default()
        })
    }
}

/// The preset flag's name, as an operator types it.
pub const WORKFLOW_ONLY: &str = "--workflow-only";

/// Expand `--workflow-only` over the answers an invocation states.
///
/// The preset sets four answers: no release, local integration, linked
/// worktrees, and the workflow guards. It records nothing of its own, so
/// the expansion is the four answers and nothing else. A flag that states
/// one of them the same way agrees with it; a flag that states another
/// answer, or an answer only a release takes, refuses before any write,
/// naming the flag and the preset. A flag the preset does not set, such
/// as `--merge-form`, passes through.
///
/// # Errors
///
/// A [`crate::diagnostic::Reason::PresetContradiction`] refusal naming
/// every contradicting flag.
///
/// SATISFIES project-profile:the-workflow-only-preset-expands-to-recorded-answers
pub fn workflow_only(
    mut inputs: crate::profile::Inputs<'_>,
) -> Result<crate::profile::Inputs<'_>, crate::error::RkError> {
    use crate::landing::{CheckoutMode, Integration};
    use crate::profile::ReleaseMode;
    let mut contradicting: Vec<String> = Vec::new();
    if let Some(mode) = inputs
        .release_mode
        .filter(|mode| *mode != ReleaseMode::None)
    {
        contradicting.push(format!(
            "--release-mode {} (the preset sets profile.release.mode = \"none\")",
            mode.as_str()
        ));
    }
    if let Some(driver) = inputs.release_driver {
        contradicting.push(format!(
            "--release-driver {driver} (an automatic release alone takes a driver)"
        ));
    }
    if let Some(style) = inputs.style {
        contradicting.push(format!(
            "--release-style {} (an automatic release alone takes a style)",
            style.as_str()
        ));
    }
    if let Some(integration) = inputs
        .integration
        .filter(|integration| *integration != Integration::Local)
    {
        contradicting.push(format!(
            "--integration {} (the preset sets git.integration = \"local\")",
            integration.as_str()
        ));
    }
    if let Some(mode) = inputs
        .checkout_mode
        .filter(|mode| *mode != CheckoutMode::LinkedWorktree)
    {
        contradicting.push(format!(
            "--checkout-mode {} (the preset sets git.checkout_mode = \"linked-worktree\")",
            mode.as_str()
        ));
    }
    if inputs.workflow_guards == Some(false) {
        contradicting.push(
            "--workflow-guards off (the preset sets capabilities.workflow_guards = true)"
                .to_owned(),
        );
    }
    if !contradicting.is_empty() {
        return Err(crate::error::RkError::refusal(
            crate::diagnostic::Diagnostic::new(
                crate::diagnostic::Reason::PresetContradiction,
                format!(
                    "{} contradict{} {WORKFLOW_ONLY}; nothing was written",
                    contradicting.join(", "),
                    if contradicting.len() == 1 { "s" } else { "" }
                ),
            )
            .expected(format!(
                "{WORKFLOW_ONLY} alone, or beside a flag it does not set, such as --merge-form"
            ))
            .action(format!(
                "drop {WORKFLOW_ONLY} and state each answer by its own flag, or drop the contradicting flag"
            ))
            .target_state("unchanged"),
        ));
    }
    inputs.release_mode = Some(ReleaseMode::None);
    inputs.integration = Some(Integration::Local);
    inputs.checkout_mode = Some(CheckoutMode::LinkedWorktree);
    inputs.workflow_guards = Some(true);
    Ok(inputs)
}
