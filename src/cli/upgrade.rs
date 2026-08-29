//! Arguments for `rk upgrade`.

use camino::Utf8PathBuf;
use clap::Args;

/// Take a landed target to this binary's payload. The technology, the
/// forge, and the landing parameters all come from the record, so an
/// upgrade cannot silently switch either.
#[derive(Debug, Args)]
pub struct UpgradeArgs {
    /// The landed repository to upgrade.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Write the upgrade; without it every file's action is listed and
    /// nothing is touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
