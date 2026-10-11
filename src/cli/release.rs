//! Arguments for `rk release`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Report where the target's release stands.
#[derive(Debug, Args)]
pub struct ReleaseArgs {
    /// What to report.
    #[command(subcommand)]
    pub action: ReleaseAction,
}

/// The release verbs.
#[derive(Debug, Subcommand)]
pub enum ReleaseAction {
    /// Report the release style, the release request, the last release,
    /// each release line, and every symptom a stuck release shows. The
    /// record fields read offline; a forge field reads unknown where this
    /// runtime cannot read the forge.
    Status {
        /// The repository to report on.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Wait until the publish run and the artifact run on this commit
        /// conclude, selected by the commit and never by recency, then
        /// report the release they made.
        #[arg(long, value_name = "SHA")]
        wait: Option<String>,

        /// How long `--wait` waits before it gives up, in seconds.
        #[arg(long, default_value_t = 3600, requires = "wait")]
        timeout: u64,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
