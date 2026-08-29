//! Arguments for `rk setup`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Execute the repository-side setup against the detected forge.
///
/// Without a subcommand: preview every step, or run them in order under
/// `--apply`; `--list` prints the ordered steps and what each proves.
#[derive(Debug, Args)]
pub struct SetupArgs {
    /// The read-only companions: check, one step, one script.
    #[command(subcommand)]
    pub action: Option<SetupAction>,

    /// The repository to set up.
    #[arg(long)]
    pub target: Option<Utf8PathBuf>,

    /// Override the detected project path (owner/name).
    #[arg(long)]
    pub repo: Option<String>,

    /// Override the detected forge: github or gitlab.
    #[arg(long)]
    pub forge: Option<String>,

    /// The check the gate must pass; required on github, refused on gitlab.
    #[arg(long)]
    pub required_check: Option<String>,

    /// Run the steps; without it every step is previewed and nothing runs.
    #[arg(long)]
    pub apply: bool,

    /// List the ordered steps and what each proves, and run nothing.
    #[arg(long)]
    pub list: bool,

    /// Emit NDJSON events on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// The setup subcommands.
#[derive(Debug, Subcommand)]
pub enum SetupAction {
    /// Prove the desired state against the forge, mutating nothing.
    Check {
        /// The repository to check.
        #[arg(long)]
        target: Utf8PathBuf,
        /// Override the detected project path (owner/name).
        #[arg(long)]
        repo: Option<String>,
        /// Override the detected forge: github or gitlab.
        #[arg(long)]
        forge: Option<String>,
        /// The check the gate must pass, verified where given.
        #[arg(long)]
        required_check: Option<String>,
        /// Emit NDJSON events on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
    /// Run one step by name, for recovery and rerun.
    Step {
        /// The step, from `rk setup --list`.
        name: String,
        /// The repository to set up.
        #[arg(long)]
        target: Utf8PathBuf,
        /// Override the detected project path (owner/name).
        #[arg(long)]
        repo: Option<String>,
        /// Override the detected forge: github or gitlab.
        #[arg(long)]
        forge: Option<String>,
        /// The check the gate must pass; required on github, refused on gitlab.
        #[arg(long)]
        required_check: Option<String>,
        /// Run the step; without it the step is previewed and nothing runs.
        #[arg(long)]
        apply: bool,
        /// Emit NDJSON events on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
    /// Print one embedded setup script, for audit.
    Script {
        /// The step whose script to print.
        name: String,
        /// Which forge's tree to read; defaults to github.
        #[arg(long)]
        forge: Option<String>,
    },
}
