//! Arguments for `rk reconcile`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand, ValueEnum};

/// Compute, show, and apply the plan that converges a target toward one release.
#[derive(Debug, Args)]
pub struct ReconcileArgs {
    /// What to do with a plan.
    #[command(subcommand)]
    pub action: ReconcileAction,
}

/// The reconcile operations.
#[derive(Debug, Subcommand)]
#[allow(
    clippy::large_enum_variant,
    reason = "the plan arguments carry every landing flag and the other actions carry an id; one enum per verb is the clap shape every subcommand here follows"
)]
pub enum ReconcileAction {
    /// Observe the target, resolve the release, compute the plan, store it, and print it; nothing is written into the target.
    Plan(PlanArgs),
    /// Render a stored plan, human or --json.
    Show(ShowArgs),
    /// Execute a stored plan: recompute its fingerprint, refuse on any difference, write, and journal.
    Apply(ApplyArgs),
    /// List the stored plans, oldest first.
    List(ListArgs),
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

/// Arguments for `rk reconcile show`.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// The plan id `rk reconcile plan` printed.
    pub plan_id: String,

    /// Emit the stored plan as one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk reconcile apply`.
#[derive(Debug, Args)]
pub struct ApplyArgs {
    /// The plan id `rk reconcile plan` printed.
    pub plan_id: String,

    /// Emit the apply report as one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk reconcile list`.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Emit the listing as one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
