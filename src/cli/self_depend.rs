//! Arguments for `rk self-depend`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand, ValueEnum};

use crate::self_depend::manager::Manager;
use crate::self_depend::venue::Venue;

/// Wire release-kit as a consumer's dependency and keep its pin fresh.
#[derive(Debug, Args)]
pub struct SelfDependArgs {
    /// What to do with the pin wiring.
    #[command(subcommand)]
    pub action: SelfDependAction,
}

/// The self-depend operations.
#[derive(Debug, Subcommand)]
pub enum SelfDependAction {
    /// Report what a target carries, offline: each manager's pin, the .envrc line, and any leftover.
    Status(StatusArgs),
    /// Serve the fragments for one manager and venue pair and the .envrc line; seed the manager file where the target has none.
    Add(AddArgs),
    /// Remove what a predecessor bump mechanism left, and name what a line scan must not touch.
    Clean(CleanArgs),
    /// Move the pin to the latest release through the wired manager; the flake pair locks and builds, both files or neither.
    Sync(SyncArgs),
}

/// Arguments for `rk self-depend status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// The project to read.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Report one manager's entry alone; every manager by default, absent ones included.
    #[arg(long, value_enum)]
    pub manager: Option<Manager>,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk self-depend add`.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// The project to wire.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The release tag to pin: v0.2.16, 0.2.16, or the release URL; this binary's version by default.
    #[arg(long)]
    pub tag: Option<String>,

    /// The target's tool manager; required when the target carries several, the flake when it carries none.
    #[arg(long, value_enum)]
    pub manager: Option<Manager>,

    /// Where rk is fetched from; the first venue the manager renders a fragment for by default.
    #[arg(long, value_enum)]
    pub venue: Option<Venue>,

    /// Write the seed files; without it the fragments are printed and nothing is written.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk self-depend clean`.
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

/// Arguments for `rk self-depend sync`.
#[derive(Debug, Args)]
pub struct SyncArgs {
    /// The project to sync.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The release tag to pin, in either direction, making no network request; the latest release by default, forward only.
    #[arg(long)]
    pub tag: Option<String>,

    /// The manager whose pin moves; the one manager naming release-kit by default.
    #[arg(long, value_enum)]
    pub manager: Option<Manager>,

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
