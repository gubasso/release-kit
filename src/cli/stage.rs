//! Arguments for `rk stage`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Write the candidate this binary would land in a target into a
/// disposable stage, beside the knowledge that explains it.
///
/// Reads the target and writes nothing inside it. The stage holds every
/// proposed destination under `artifacts/`, the changelog, guidance,
/// method, bindings, runbooks, forge notes, and setup skill under
/// `reference/`, and one explanatory `stage.json`. Production landing
/// never reads a stage; `rk stage clean <path>` removes one.
#[derive(Debug, Args)]
pub struct StageArgs {
    /// The one companion: remove a stage.
    #[command(subcommand)]
    pub action: Option<StageAction>,

    /// The technology whose candidate is staged; one of the bindings.
    #[arg(long)]
    pub tech: Option<String>,

    /// The repository the candidate is computed for; read, never written.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The forge whose files are staged: github or gitlab. Defaults to
    /// detection from the target's git remote.
    #[arg(long)]
    pub forge: Option<String>,

    /// The project path on the forge, substituted into the rendered files.
    /// Defaults to detection from the target's git remote.
    #[arg(long)]
    pub repo: Option<String>,

    /// The working-copy mode the candidate renders under: worktree or
    /// branches.
    #[arg(long)]
    pub workflow: Option<String>,

    /// The release style the candidate renders under: trunk or lines.
    #[arg(long)]
    pub style: Option<String>,

    /// Stage the Nix capability too.
    #[arg(long)]
    pub nix: bool,

    /// The exact directory to stage into; it must be absent or empty.
    /// Without it, a target and version directory below `RK_STAGE_ROOT`,
    /// else below the private state root.
    #[arg(long)]
    pub output: Option<Utf8PathBuf>,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// The stage subcommands.
#[derive(Debug, Subcommand)]
pub enum StageAction {
    /// Remove exactly one stage directory, which must name itself in its
    /// own stage.json.
    Clean {
        /// The stage directory, as `rk stage` printed it. No glob, no
        /// parent, no link.
        path: Utf8PathBuf,
        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
