//! Arguments for `rk payload`.

use clap::Args;

/// Report the payload this binary carries: the version, every root, and
/// the digests that identify the artifact set.
#[derive(Debug, Args)]
pub struct PayloadArgs {
    /// Another release's manifest, read through the crates venue: an exact
    /// version or `latest`, fetched once, verified against the registry
    /// index checksum, and cached by that checksum.
    #[arg(long, value_name = "VERSION")]
    pub release: Option<String>,
    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
