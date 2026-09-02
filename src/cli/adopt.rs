//! Arguments for `rk adopt`.

use camino::Utf8PathBuf;
use clap::Args;

/// Write the landing record for a repository that already runs the
/// convention, landed before the record existed.
///
/// Strict: every rendered file must match what this payload would
/// render, and no target file is ever changed.
#[derive(Debug, Args)]
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

    /// The Conventional Commit scopes this project accepts,
    /// comma-separated, the parameter the candidate is rendered under and
    /// the record carries. There is no record to read it from yet, so the
    /// adoption refuses without it.
    #[arg(long)]
    pub scopes: Option<String>,

    /// The working-copy mode the candidate is rendered under: worktree or
    /// branches. It chooses which candidate adoption verifies against and
    /// never blesses the disk; the default is branches, the
    /// compatibility-safe reading of a pre-record target.
    #[arg(long, default_value = "branches")]
    pub workflow: String,

    /// Write the record; without it the verification runs and nothing is
    /// touched.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
