//! Arguments for `rk reconcile`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand, ValueEnum};

/// Compute the plan that converges a target toward one release.
#[derive(Debug, Args)]
pub struct ReconcileArgs {
    /// What to do with a plan.
    #[command(subcommand)]
    pub action: ReconcileAction,
}

/// The reconcile operations.
#[derive(Debug, Subcommand)]
pub enum ReconcileAction {
    /// Observe the target, resolve the release, compute the plan, and print it; nothing is written.
    Plan(PlanArgs),
}

/// What a plan may read beyond the target and the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Observe {
    /// Read the trunk's tip at the origin remote, and stamp it.
    Forge,
}

/// Arguments for `rk reconcile plan`.
#[derive(Debug, Args)]
pub struct PlanArgs {
    /// The repository to plan for.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The release to converge toward: `embedded` for this binary's own
    /// bundle, offline; `latest` or an exact version through the crates
    /// venue, which reaches the network and says so.
    #[arg(long, default_value = "embedded", value_name = "SELECTOR")]
    pub to: String,

    /// Read the recorded release's bundle through the crates venue where
    /// the cache does not hold it, so the baseline is bytes; without it a
    /// missing baseline is reported as not observed.
    #[arg(long)]
    pub fetch: bool,

    /// Opt into a read beyond the target: `forge` reads the trunk's tip
    /// at the origin remote.
    #[arg(long, value_enum, value_name = "WHAT")]
    pub observe: Vec<Observe>,

    /// Select an answer to a decision the plan names, as `<id>=<answer>`;
    /// repeatable. A selected decision is a fingerprint input.
    #[arg(long, value_name = "ID=ANSWER")]
    pub decide: Vec<String>,

    /// Override the configured payload binding.
    #[arg(long)]
    pub tech: Option<String>,

    /// Override the configured forge.
    #[arg(long)]
    pub forge: Option<String>,

    /// Override the configured project path.
    #[arg(long)]
    pub repo: Option<String>,

    /// Answer the working-copy mode: worktree or branches.
    #[arg(long)]
    pub workflow: Option<String>,

    /// Answer the release style: trunk or lines.
    #[arg(long)]
    pub style: Option<String>,

    /// Answer the Nix opt-in: `on` or `off`.
    #[arg(long)]
    pub nix: Option<String>,

    /// Emit the plan as one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
