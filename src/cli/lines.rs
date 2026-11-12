//! Arguments for `rk lines`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Open, inventory, mint candidates on, and retire the release lines.
#[derive(Debug, Args)]
pub struct LinesArgs {
    /// What to do with the lines.
    #[command(subcommand)]
    pub action: LinesAction,
}

/// The line verbs. A line is `release/<major>.<minor>`: cut from an
/// explicit base, validated through the candidates it mints, released by
/// hand, and retired only behind its tags.
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

    /// Mint the line's next release candidate: write its version into the
    /// manifest, commit, and push the line; the release half tags it.
    /// Preview by default.
    Mint {
        /// The line's `<major>.<minor>`.
        line: String,

        /// The line's tip the caller saw; the mint refuses when the remote
        /// tip, the checkout's HEAD, or the working tree moved from it.
        /// Required with --apply.
        #[arg(long)]
        expect: Option<String>,

        /// Promote the newest candidate instead: write its stable version,
        /// so the line's release request proposes it. Refuses while the
        /// line holds a commit that candidate did not test.
        #[arg(long)]
        promote: bool,

        /// The repository to act on; any of its worktrees names it.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Commit and push the candidate's version; without it the intent
        /// is reported and nothing is touched.
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
