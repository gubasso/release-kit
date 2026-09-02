//! Arguments for `rk status`.

use camino::Utf8PathBuf;
use clap::Args;

/// Report what landed in a target and whether it stayed truthful.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// The repository to report on.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Judge the identical report: exit 1 on a violation — drift on a
    /// rendered file, an invalid or missing landing, an unresolved
    /// judgment sentinel, or an invariant a landed file's effective
    /// configuration violates. Seeded drift and pin staleness stay
    /// informational.
    #[arg(long)]
    pub check: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
