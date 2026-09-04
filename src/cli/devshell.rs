//! Arguments for `rk devshell`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand, ValueEnum};

/// Wire release-kit as a consumer's devshell dependency and keep its pin fresh.
#[derive(Debug, Args)]
pub struct DevshellArgs {
    /// What to do with the devshell wiring.
    #[command(subcommand)]
    pub action: DevshellAction,
}

/// The devshell operations.
#[derive(Debug, Subcommand)]
pub enum DevshellAction {
    /// Report what a target carries, offline: the pin, the lock, the .envrc line, and any leftover.
    Status(StatusArgs),
    /// Serve the flake fragments and the .envrc line; seed both files where the target has none.
    Add(AddArgs),
    /// Remove what a predecessor bump mechanism left, and name what a line scan must not touch.
    Clean(CleanArgs),
    /// Move the pin to the latest release, lock it, and prove it builds; both files or neither.
    Sync(SyncArgs),
}

/// Arguments for `rk devshell status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// The project to read.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk devshell add`.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// The project to wire.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The release tag to pin: v0.2.16, 0.2.16, or the release URL; this binary's version by default.
    #[arg(long)]
    pub tag: Option<String>,

    /// Write the seed files; without it the fragments are printed and nothing is written.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk devshell clean`.
#[derive(Debug, Args)]
pub struct CleanArgs {
    /// The project to clean.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// One extra file to remove, for a predecessor the catalog does not know; repeatable.
    #[arg(long, value_name = "PATH")]
    pub also: Vec<Utf8PathBuf>,

    /// Remove the files and rewrite .envrc; without it every leftover is listed.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk devshell sync`.
#[derive(Debug, Args)]
pub struct SyncArgs {
    /// The project to sync.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The release tag to move to, making no network request; the latest release by default.
    #[arg(long)]
    pub tag: Option<String>,

    /// Who is calling: envrc stays silent and exits 0 on every outcome, operator reports and fails loudly.
    #[arg(long, value_enum, default_value_t = Caller::Envrc)]
    pub caller: Caller,

    /// Rewrite the pin, refresh the lock, and build; without it the bump is reported and nothing runs.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Who invoked the sync. One flag decides four behaviors as a bundle: the
/// daily stamp, silence on nothing to do, the exit code of a reported
/// failure, and silence under lock contention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Caller {
    /// The `.envrc` line on directory entry: gated by the stamp, silent, exit 0.
    Envrc,
    /// A person or an agent at a prompt: every outcome reported, the exit-code matrix.
    Operator,
}
