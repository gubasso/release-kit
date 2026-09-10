//! Arguments for `rk adopt`.

use camino::Utf8PathBuf;
use clap::Args;

/// Write the landing record for a repository that already runs the
/// convention, landed before the record existed.
///
/// Strict: every rendered file must match what this payload would
/// render, and no target file is ever changed.
#[derive(Debug, Clone, Args)]
pub struct AdoptArgs {
    /// The repository to adopt.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The technology whose payload the target runs. Defaults to
    /// detection from the version file.
    #[arg(long)]
    pub tech: Option<String>,

    /// The forge whose payload the target runs: github or gitlab.
    /// Defaults to detection from the target's git remote.
    #[arg(long)]
    pub forge: Option<String>,

    /// The project path on the forge, the parameter the candidate is
    /// rendered under. Defaults to detection from the target's git
    /// remote.
    #[arg(long)]
    pub repo: Option<String>,

    /// The working-copy mode the candidate is rendered under: worktree or
    /// branches. It chooses which candidate adoption verifies against and
    /// never blesses the disk; the default is branches, the
    /// compatibility-safe reading of a pre-record target.
    #[arg(long)]
    pub workflow: Option<String>,

    /// The release style the candidate is rendered under: trunk or lines.
    /// Required from the config or this flag: the style changes the bytes. An
    /// adoption verifies bytes against exactly one rendered candidate, so
    /// neither value is a safe guess.
    #[arg(long)]
    pub style: Option<String>,

    /// The target runs the Nix capability: the candidate includes its
    /// files, and the record carries the parameter. A target whose flake
    /// pair is its own is verified without the pair and the workflow,
    /// exactly as a landing would have withheld them.
    #[arg(long)]
    pub nix: bool,

    /// Write the config and record; without it verification runs and nothing is
    /// touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
