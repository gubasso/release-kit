//! Arguments for `rk integrate`.

use camino::Utf8PathBuf;
use clap::Args;

/// Move one implementation onto the trunk, through the recorded authority.
#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one clap flag: the two authority overrides are mutually exclusive by declaration, and the preview and the machine form are the flags every verb here carries"
)]
pub struct IntegrateArgs {
    /// The branch to integrate.
    pub branch: String,

    /// The target repository.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The trunk commit's message. A local integration writes it as the
    /// squash commit, so it must be a scoped Conventional Commit the
    /// landed guards admit. Required for a local integration.
    #[arg(long, short = 'm', value_name = "TEXT")]
    pub message: Option<String>,

    /// Integrate locally for this one execution, whatever the record
    /// says. It writes no record and changes no landed byte.
    #[arg(long, conflicts_with = "forge")]
    pub local: bool,

    /// Integrate at the forge for this one execution, whatever the
    /// record says.
    #[arg(long, conflicts_with = "local")]
    pub forge: bool,

    /// Perform the integration. Without it the command reports what it
    /// would do and writes nothing.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
