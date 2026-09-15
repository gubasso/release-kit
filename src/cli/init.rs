//! Arguments for `rk init`.

use camino::Utf8PathBuf;
use clap::Args;

/// Land a technology's deterministic files into a target repository.
#[derive(Debug, Clone, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one clap flag, and an opt-in capability's flag is a boolean by design; a state machine would hide the command line this struct describes"
)]
pub struct InitArgs {
    /// The technology whose files land; one of the bindings.
    #[arg(long)]
    pub tech: Option<String>,

    /// The repository the files land into.
    #[arg(long)]
    pub target: Utf8PathBuf,

    /// The forge whose files land: github or gitlab. Defaults to detection
    /// from the target's git remote; an unrecognized host refuses.
    #[arg(long)]
    pub forge: Option<String>,

    /// The project path on the forge, substituted into the rendered files
    /// and recorded as the landing parameter. Defaults to detection from
    /// the target's git remote; an apply with neither refuses.
    #[arg(long)]
    pub repo: Option<String>,

    /// The working-copy mode this project chooses: worktree (every
    /// code-changing branch in a linked worktree, the main checkout
    /// commits nothing) or branches (branches worked in the main
    /// checkout, worktrees optional beside them). Recorded as a landing
    /// parameter and rendered into the landed blocks.
    #[arg(long)]
    pub workflow: Option<String>,

    /// The release style this project chooses: trunk (the bot's release
    /// request carries auto-merge from creation, so a green trunk ships
    /// itself) or lines (every request waits for a human's merge).
    /// Recorded as a landing parameter and rendered into the landed
    /// release workflow.
    #[arg(long)]
    pub style: Option<String>,

    /// Opt the landing into the Nix capability: a seeded package
    /// expression, a seed flake pair where the target has none, and the
    /// workflow that proves the build. Recorded as a landing parameter;
    /// off by default, because a packaging surface is a decision, not a
    /// default.
    #[arg(long)]
    pub nix: bool,

    /// Opt the landing into the Scorecard capability: a workflow that
    /// computes an `OpenSSF` Scorecard result and publishes it to the public
    /// Scorecard API. GitHub only. Recorded as a landing parameter; off by
    /// default, because publishing a score is a decision, not a default.
    #[arg(long)]
    pub scorecard: bool,

    /// Write the files; without it the destinations are listed and nothing
    /// is touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
