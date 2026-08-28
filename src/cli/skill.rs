//! Arguments for `rk skill`.

use clap::{Args, Subcommand};

/// Manage the agent skills at user scope.
#[derive(Debug, Args)]
pub struct SkillArgs {
    /// What to do with the skills.
    #[command(subcommand)]
    pub action: SkillAction,
}

/// The skill operations.
#[derive(Debug, Subcommand)]
pub enum SkillAction {
    /// List the skills the binary carries.
    List,
    /// Print one skill.
    Show {
        /// The skill's name.
        name: String,
    },
    /// Install the skills at user scope, previewing by default.
    Install {
        /// Write the files; without it the destinations are listed.
        #[arg(long)]
        apply: bool,
        /// Overwrite a destination whose bytes differ from the payload.
        #[arg(long)]
        force: bool,
    },
    /// Remove the installed skills, previewing by default.
    Uninstall {
        /// Remove the files; without it the removals are listed.
        #[arg(long)]
        apply: bool,
    },
}
