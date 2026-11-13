//! Arguments for `rk skill`.

use clap::{Args, Subcommand, ValueEnum};

/// Read the agent skills this binary carries, and remove the copies an
/// earlier release installed under the home.
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
    /// Print one skill's `SKILL.md`, or one file of its package.
    Show {
        /// The skill's name.
        name: String,
        /// One file of the package, relative to the skill's folder, such
        /// as `references/pre-flight-gate.md`.
        #[arg(long)]
        file: Option<String>,
    },
    /// Retired: every landing carries the skills. Names the landing, and
    /// refuses under `--apply`.
    Install {
        /// Kept so that an older call reads the retirement, not a usage
        /// error.
        #[arg(long, value_enum, default_value_t = Agent::All)]
        agent: Agent,
        /// Refuse, because no rk verb writes a skill under the home.
        #[arg(long)]
        apply: bool,
        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
    /// Remove the skill copies an earlier release installed under the
    /// home, previewing by default.
    Uninstall {
        /// Which agent's skill directory to remove from.
        #[arg(long, value_enum, default_value_t = Agent::All)]
        agent: Agent,
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
