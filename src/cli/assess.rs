//! Arguments for `rk assess`.

use camino::Utf8PathBuf;
use clap::Args;

/// Classify a target before anything lands: greenfield, brownfield, or
/// needs-decision, from evidence read off the disk and the local git
/// state.
///
/// Reporting only: it writes nothing, touches no network, and every
/// verdict exits 0 — the classification is a result, not a judgment.
#[derive(Debug, Args)]
pub struct AssessArgs {
    /// The repository to classify.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
