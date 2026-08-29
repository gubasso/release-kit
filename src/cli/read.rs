//! Shared argument shape for the read-only payload commands.

use clap::Args;

/// Read one payload entry, or list the valid names.
///
/// The group makes the either-or explicit to clap, so `rk method` with
/// neither is refused at parse time and `rk usage` can render the
/// alternative in its example.
#[derive(Debug, Args)]
#[group(id = "selection", required = true, multiple = true)]
pub struct ReadArgs {
    /// The entry to print; omit it and pass --list to see the names.
    pub name: Option<String>,

    /// List the valid names instead of printing an entry.
    #[arg(long)]
    pub list: bool,
}
