//! Shared argument shape for the read-only payload commands.

use clap::Args;

/// Read one payload entry, or list the valid names.
#[derive(Debug, Args)]
pub struct ReadArgs {
    /// The entry to print; omit it and pass --list to see the names.
    pub name: Option<String>,

    /// List the valid names instead of printing an entry.
    #[arg(long)]
    pub list: bool,
}
