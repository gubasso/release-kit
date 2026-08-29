//! Arguments for `rk versions`.

use clap::Args;

/// Print the pinned-tool registry, or check it against the world.
#[derive(Debug, Args)]
pub struct VersionsArgs {
    /// Explicitly go online: fetch each pin's check URL and report per
    /// pin — current, update-available, source-unreachable, or
    /// source-unparsable. Mutates nothing; a pin update is a reviewed
    /// change to versions.toml.
    #[arg(long)]
    pub check: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long, requires = "check")]
    pub json: bool,
}
