//! Arguments for `rk upgrade`.

use camino::Utf8PathBuf;
use clap::Args;

/// Take a landed target to this binary's payload, resolving flags over
/// configuration and keeping recorded answers where both are silent.
#[derive(Debug, Args)]
pub struct UpgradeArgs {
    /// The landed repository to upgrade.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Override the configured payload binding.
    #[arg(long)]
    pub tech: Option<String>,

    /// Override the configured forge.
    #[arg(long)]
    pub forge: Option<String>,

    /// Override the configured project path.
    #[arg(long)]
    pub repo: Option<String>,

    /// Change the recorded working-copy mode: worktree or branches. The
    /// apply writes the answer back into the config. Omitted, the config
    /// answers first and the recorded mode is the fallback.
    #[arg(long)]
    pub workflow: Option<String>,

    /// Change the recorded release style: trunk or lines. A record that
    /// already carries one needs no flag; a pre-style record needs
    /// landing.style in the config or this flag to answer it.
    #[arg(long)]
    pub style: Option<String>,

    /// Change the recorded Nix opt-in: `on` adds the capability's files
    /// and records it; `off` drops them from the record while the files
    /// stay on disk as the target's own, like any file this payload stops
    /// shipping. Omitted, configuration precedes the recorded choice.
    #[arg(long)]
    pub nix: Option<String>,

    /// Write the upgrade; without it every file's action is listed and
    /// nothing is touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
