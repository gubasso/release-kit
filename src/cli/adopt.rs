//! Arguments for `rk adopt`.

use camino::Utf8PathBuf;
use clap::Args;

use super::profile_flags::ProfileFlags;

/// Write the landing record for a repository that already runs the
/// convention, landed before the record existed.
///
/// Strict: every rendered file must match what this binary would
/// render, and no target file is ever changed.
#[derive(Debug, Clone, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one clap flag, and an opt-in capability's flag is a boolean by design; a state machine would hide the command line this struct describes"
)]
pub struct AdoptArgs {
    /// The repository to adopt.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The project profile and the Git workflow the candidate is rendered
    /// under. The checkout mode chooses which candidate adoption verifies
    /// against and never blesses the disk; its default is main-worktree,
    /// the compatibility-safe reading of a pre-record target. An automatic
    /// release requires the style from the config or the flag: the style
    /// changes the bytes, so neither value is a safe guess.
    #[command(flatten)]
    pub profile: ProfileFlags,

    /// The target runs the Nix capability: the candidate includes its
    /// files, and the record carries the request. A target whose flake
    /// pair is its own is verified without the pair, exactly as a landing
    /// would have withheld it.
    #[arg(long, alias = "nix")]
    pub nix_packaging: bool,

    /// The target carries the landed reporting policy.
    #[arg(long)]
    pub reporting_policy: bool,

    /// The target runs the Scorecard capability: the candidate includes its
    /// workflow, and the record carries the request.
    #[arg(long)]
    pub scorecard: bool,

    /// The target runs code scanning under this provider: codeql or
    /// semgrep. The candidate includes its workflow and the record carries
    /// the request.
    #[arg(long, value_name = "PROVIDER")]
    pub code_scanning: Option<String>,

    /// Write the config and record; without it verification runs and nothing is
    /// touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
