//! Arguments for `rk init`.

use camino::Utf8PathBuf;
use clap::Args;

/// Land a technology's deterministic files into a target repository.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// The technology whose files land; one of the bindings.
    #[arg(long)]
    pub tech: String,

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

    /// The Conventional Commit scopes this project accepts,
    /// comma-separated, rendered into the title checks and the commit
    /// hook and recorded as a landing parameter. An apply without it
    /// refuses: the scope vocabulary is a decision, not a default.
    #[arg(long)]
    pub scopes: Option<String>,

    /// The working-copy mode this project chooses: worktree (every
    /// code-changing branch in a linked worktree, the main checkout
    /// commits nothing) or branches (branches worked in the main
    /// checkout, worktrees optional beside them). Recorded as a landing
    /// parameter and rendered into the landed blocks.
    #[arg(long, default_value = "worktree")]
    pub workflow: String,

    /// Write the files; without it the destinations are listed and nothing
    /// is touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
