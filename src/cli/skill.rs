//! Arguments for `rk skill`.

use clap::{Args, Subcommand, ValueEnum};

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
        /// Which agent's skill directory to install into.
        #[arg(long, value_enum, default_value_t = Agent::All)]
        agent: Agent,
        /// Where the skills land.
        #[arg(long, value_enum, default_value_t = Scope::User)]
        scope: Scope,
        /// Write the files; without it the destinations are listed.
        #[arg(long)]
        apply: bool,
        /// Overwrite a destination whose bytes differ from the payload.
        #[arg(long, requires = "apply")]
        force: bool,
        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
    /// Remove the installed skills, previewing by default.
    Uninstall {
        /// Which agent's skill directory to remove from.
        #[arg(long, value_enum, default_value_t = Agent::All)]
        agent: Agent,
        /// Where the skills live.
        #[arg(long, value_enum, default_value_t = Scope::User)]
        scope: Scope,
        /// Remove the files; without it the removals are listed.
        #[arg(long)]
        apply: bool,
        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}

/// Which skill directory family a run touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Agent {
    /// `.claude/skills`, which Claude Code reads.
    Claude,
    /// `.agents/skills`, which Codex, Gemini CLI, and Copilot read.
    Codex,
    /// Both directories.
    All,
}

/// Where an install lands.
///
/// One value, and a flag rather than a silent default, because the scope is
/// the decision an operator most needs stated: an agent resolves a skill by
/// name across scopes, so the skills have exactly one owner and no project
/// scope is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Scope {
    /// The home-directory skill roots, shared across every repository.
    User,
}
