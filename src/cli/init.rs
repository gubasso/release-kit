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

    /// Write the files; without it the destinations are listed and nothing
    /// is touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
