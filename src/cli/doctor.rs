//! Arguments for `rk doctor`.

use clap::Args;

/// Run every environment probe and report by class.
///
/// Reporting only: the exit code stays 0 whatever the probes find, and no
/// read-only verb gates on them — a doctor you cannot run while broken is
/// its own bug.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}
