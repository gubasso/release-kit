//! Arguments for `rk stage`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

use super::profile_flags::ProfileFlags;

/// Write the candidate this binary would land in a target into a
/// disposable stage, beside the knowledge that explains it.
///
/// Reads the target and writes nothing inside it. The stage holds every
/// proposed destination under `artifacts/`, the changelog, guidance,
/// method, bindings, runbooks, forge notes, and setup skill under
/// `reference/`, and one explanatory `stage.json`. Production landing
/// never reads a stage; `rk stage clean <path>` removes one.
#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one clap flag, and an opt-in capability's flag is a boolean by design; a state machine would hide the command line this struct describes"
)]
pub struct StageArgs {
    /// The one companion: remove a stage.
    #[command(subcommand)]
    pub action: Option<StageAction>,

    /// The project profile and the Git workflow the candidate renders
    /// under.
    #[command(flatten)]
    pub profile: ProfileFlags,

    /// The repository the candidate is computed for; read, never written.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Stage the Nix capability too.
    #[arg(long, alias = "nix")]
    pub nix_packaging: bool,

    /// Stage the reporting policy too.
    #[arg(long)]
    pub reporting_policy: bool,

    /// Stage the Scorecard capability too.
    #[arg(long)]
    pub scorecard: bool,

    /// Stage the code scanning capability under this provider: codeql or
    /// semgrep.
    #[arg(long, value_name = "PROVIDER")]
    pub code_scanning: Option<String>,

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
