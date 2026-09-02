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

    /// The Conventional Commit scopes this project accepts,
    /// comma-separated. A record that already carries them needs no flag;
    /// a record from before the parameter existed refuses until one names
    /// them, and the answer is recorded.
    #[arg(long)]
    pub scopes: Option<String>,

    /// Change the recorded working-copy mode: worktree or branches. The
    /// one overridden parameter — everything else comes from the record —
    /// and the apply's diff is the visible mode change. Omitted, the
    /// recorded mode is kept.
    #[arg(long)]
    pub workflow: Option<String>,

    /// Write the upgrade; without it every file's action is listed and
    /// nothing is touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
