//! Arguments for `rk migrate`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Carry a target off another release tool through an archive and a
/// greenfield landing.
#[derive(Debug, Args)]
pub struct MigrateArgs {
    /// What to do with the migration.
    #[command(subcommand)]
    pub action: MigrateAction,
}

/// The migration verbs. The archive opens a migration, the marks record
/// the map and the loop, and the close ends it; the workspace under the
/// state root keeps the archive and the registry between runs.
#[derive(Debug, Subcommand)]
pub enum MigrateAction {
    /// List every predecessor file with its tool and its kind, and the
    /// root files that stay in place; read-only.
    Inventory {
        /// The repository to read.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Move every predecessor file into a new workspace, snapshot what no
    /// Git holds, and open the migration; preview by default. On an open
    /// migration it takes the forge snapshots still missing.
    Archive {
        /// The repository to act on.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Move the files and write the workspace; without it the archive
        /// is reported and nothing is touched.
        #[arg(long)]
        apply: bool,

        /// Recreate the archived files of the open migration from Git at
        /// its base commit, on a clone whose workspace is gone; preview by
        /// default, written with `--apply`. It writes the workspace alone,
        /// never the target.
        #[arg(long)]
        rebuild: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Record a status for a source or a group, appending a dated history
    /// entry; `--group` files a source, `--destination` names where a
    /// group's kept content lives, and a drop names its reason.
    Mark {
        /// A source, by id or path, or a group, by id.
        name: String,

        /// pending, mapped, written, verified, or dropped.
        status: String,

        /// The group a source joins, created where it is new.
        #[arg(long)]
        group: Option<String>,

        /// A topic label for a source; repeatable.
        #[arg(long = "tag")]
        tags: Vec<String>,

        /// A destination a group's kept content lives at; repeatable.
        #[arg(long = "destination")]
        destinations: Vec<String>,

        /// Why a source or a group is dropped.
        #[arg(long)]
        reason: Option<String>,

        /// The repository whose migration to mark.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Report the open migration: each source's status and checksum, the
    /// groups, the snapshots, and what still blocks the close; read-only.
    Status {
        /// The repository to read.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// End the migration once every source is verified or dropped and the
    /// setup proof is current, removing the configuration key; preview by
    /// default. The workspace stays for the operator.
    Close {
        /// The repository to act on.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Remove the key; without it the close is judged and nothing is
        /// touched.
        #[arg(long)]
        apply: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
