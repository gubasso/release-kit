//! Arguments for `rk lines`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Open, inventory, and retire the release lines.
#[derive(Debug, Args)]
pub struct LinesArgs {
    /// What to do with the lines.
    #[command(subcommand)]
    pub action: LinesAction,
}

/// The line verbs. A line is `release/<major>.<minor>`: cut from an
/// explicit base, released by hand, and retired only behind its tags.
#[derive(Debug, Subcommand)]
pub enum LinesAction {
    /// Report every release line — its tags, its tag coverage, and its
    /// seat — offline.
    List {
        /// The repository to read; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Open `release/<line>` at an explicit base, seated per the recorded
    /// workflow mode; preview by default.
    Open {
        /// The line's `<major>.<minor>`.
        line: String,

        /// The commit-ish the line is cut from — the tag it patches. A
        /// line is a snapshot of a chosen commit, so there is no default.
        #[arg(long)]
        base: Option<String>,

        /// The repository to act on; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Create the line; without it the intent is reported and nothing
        /// is touched.
        #[arg(long)]
        apply: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Report a line's newest candidate tag and the next number a finding
    /// would mint; read-only, because a tag is never hand-authored.
    Rc {
        /// The line's `<major>.<minor>`.
        line: String,

        /// The repository to read; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Retire a line that left production: the seat before the branch,
    /// only behind its tags, and only under --apply; the remote deletion
    /// stays the operator's.
    Retire {
        /// The line's `<major>.<minor>`.
        line: String,

        /// The repository to act on; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Remove the seat and delete the local branch; without it the
        /// intent is reported and nothing is touched.
        #[arg(long)]
        apply: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
