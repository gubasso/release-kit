//! Arguments for `rk upgrade`.

use camino::Utf8PathBuf;
use clap::Args;

use super::profile_flags::ProfileFlags;

/// Take a landed target to this binary's projection.
///
/// Flags resolve over configuration, and recorded answers stand where
/// both are silent. Preview by default; `--apply` replaces every recorded
/// generated file, preserves every seeded and state file, and rewrites
/// the receipt last.
#[derive(Debug, Args)]
pub struct UpgradeArgs {
    /// The landed repository to upgrade.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The project profile and the Git workflow, each flag overriding the
    /// configured answer; the apply writes the answer back into the
    /// config. Omitted, the config answers first and the record is the
    /// fallback.
    #[command(flatten)]
    pub profile: ProfileFlags,

    /// Change the recorded Nix request: `on` adds the capability's files
    /// and records it; `off` drops them from the record while the files
    /// stay on disk as the target's own, like any file this binary stops
    /// shipping. Omitted, configuration precedes the recorded choice.
    #[arg(long, alias = "nix", value_name = "on|off")]
    pub nix_packaging: Option<String>,

    /// Change the recorded reporting policy request: `on` lands
    /// `SECURITY.md` and records it; `off` drops it from the record while
    /// the file stays on disk as the target's own.
    #[arg(long, value_name = "on|off")]
    pub reporting_policy: Option<String>,

    /// Change the recorded Scorecard request: `on` adds the workflow and
    /// records it; `off` drops it from the record while the file stays on
    /// disk as the target's own, like any file this binary stops shipping.
    /// Omitted, configuration precedes the recorded choice.
    #[arg(long, value_name = "on|off")]
    pub scorecard: Option<String>,

    /// Change the recorded code scanning provider: `codeql` or `semgrep`
    /// lands that provider's workflow and records it; `off` drops it from
    /// the record while the file stays on disk as the target's own. Omitted,
    /// configuration precedes the recorded choice.
    #[arg(long, value_name = "PROVIDER")]
    pub code_scanning: Option<String>,

    /// Write the upgrade; without it every file's action is listed and
    /// nothing is touched. `rk stage` is the full-byte comparison surface.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
