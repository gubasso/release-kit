//! Arguments for `rk payload`.

use clap::Args;

/// Report the payload this binary carries: the version, every root, and
/// the digests that identify the artifact set.
#[derive(Debug, Args)]
pub struct PayloadArgs {
    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
