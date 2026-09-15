//! Arguments for `rk profile`.

use camino::Utf8PathBuf;
use clap::Args;

use super::profile_flags::ProfileFlags;

/// Report what a target resolves to, and write nothing.
///
/// Every profile, Git workflow, and capability value with its source, the
/// unknown categories, the capabilities the catalog selects, and the
/// complete destinations the projection lands.
#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one clap flag, and an opt-in capability's flag is a boolean by design; a state machine would hide the command line this struct describes"
)]
pub struct ProfileArgs {
    /// The repository to resolve for.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The project profile and the Git workflow, as `rk init` takes them.
    #[command(flatten)]
    pub profile: ProfileFlags,

    /// Resolve with the Nix capability requested.
    #[arg(long, alias = "nix")]
    pub nix_packaging: bool,

    /// Resolve with the reporting policy requested.
    #[arg(long)]
    pub reporting_policy: bool,

    /// Resolve with the Scorecard capability requested.
    #[arg(long)]
    pub scorecard: bool,

    /// Resolve with code scanning requested under this provider.
    #[arg(long, value_name = "PROVIDER")]
    pub code_scanning: Option<String>,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
