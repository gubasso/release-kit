//! Arguments for `rk init`.

use camino::Utf8PathBuf;
use clap::Args;

use super::profile_flags::ProfileFlags;

/// Land the files the target configuration selects into a repository.
#[derive(Debug, Clone, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each field is one clap flag, and an opt-in capability's flag is a boolean by design; a state machine would hide the command line this struct describes"
)]
pub struct InitArgs {
    /// The project profile and the Git workflow.
    #[command(flatten)]
    pub profile: ProfileFlags,

    /// The repository the files land into.
    #[arg(long)]
    pub target: Utf8PathBuf,

    /// Opt the landing into the Nix capability: a seeded package
    /// expression and a seed flake pair where the target has none.
    /// Recorded as a capability request; off by default, because a
    /// packaging surface is a decision, not a default.
    #[arg(long, alias = "nix")]
    pub nix_packaging: bool,

    /// Request the landed vulnerability reporting policy, `SECURITY.md`.
    /// An automatic release requests it by default; a release-less
    /// profile asks for it with this flag.
    #[arg(long)]
    pub reporting_policy: bool,

    /// Opt the landing into the Scorecard capability: a workflow that
    /// computes an `OpenSSF` Scorecard result and publishes it to the public
    /// Scorecard API. GitHub only. Recorded as a capability request; off by
    /// default, because publishing a score is a decision, not a default.
    #[arg(long)]
    pub scorecard: bool,

    /// Opt the landing into code scanning, naming the provider: codeql,
    /// GitHub's own analyzer, whose terms cover an open-source codebase so
    /// the landing reads the driver's declared licence and refuses the pair
    /// where it is not OSI-approved; or semgrep, which carries no licence
    /// condition and runs on either forge. Recorded as a capability
    /// request; off by default, because a scanning surface is a decision.
    #[arg(long, value_name = "PROVIDER")]
    pub code_scanning: Option<String>,

    /// Write the files; without it the destinations are listed and nothing
    /// is touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
