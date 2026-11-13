//! Arguments for `rk release`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// Report where the target's release stands, and run the release
/// engine's actions.
#[derive(Debug, Args)]
pub struct ReleaseArgs {
    /// What to report.
    #[command(subcommand)]
    pub action: ReleaseAction,
}

/// The release verbs.
#[derive(Debug, Subcommand)]
pub enum ReleaseAction {
    /// Report the release style, the release request, the last release,
    /// each release line, and every symptom a stuck release shows. The
    /// record fields read offline; a forge field reads unknown where this
    /// runtime cannot read the forge.
    Status {
        /// The repository to report on.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Wait until the publish run and the artifact run on this commit
        /// conclude, selected by the commit and never by recency, then
        /// report the release they made.
        #[arg(long, value_name = "SHA")]
        wait: Option<String>,

        /// How long `--wait` waits before it gives up, in seconds.
        #[arg(long, default_value_t = 3600, requires = "wait")]
        timeout: u64,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Merge the trunk's release request on the operator's order. Without
    /// `--apply`, read the one open release request against the trunk, the
    /// trunk tip, its head, and the required check for that head, and write
    /// nothing. With `--apply`, dispatch the landed merge job with exactly
    /// those values, which runs as the release bot, proves each one again,
    /// and merges with the head pinned, then wait for that one run. A
    /// release line keeps the merge by a person.
    Merge {
        /// The landed target whose trunk release request merges.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Dispatch the merge job and wait for it; without it, preview.
        #[arg(long)]
        apply: bool,

        /// How long `--apply` waits for the merge run, in seconds.
        #[arg(long, default_value_t = 1800, requires = "apply")]
        timeout: u64,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Keep the one release request of the pushed base on
    /// `rk-release/<base>`: propose the next version, write it and the
    /// changelog, force-push as the bot, and open or edit the request,
    /// which it never arms: a release waits for the operator's order. Runs
    /// in CI on a push to the trunk or a release line; every fact comes
    /// from the record.
    Request {
        /// The checkout the release job runs in.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Decide whether this push releases, and build it: report
    /// `release`, `resume`, or `skip`, and write the artifacts'
    /// checksums, the notes, and the report into `.rk-release/`. A skip
    /// exits 0.
    Build {
        /// The checkout the release job runs in.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Print the files the package holds at `HEAD`, one path per line,
        /// as `cargo package --list` does, and write nothing: no build
        /// output, no report, and no forge read.
        #[arg(long)]
        list: bool,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Answer whether the change since the last release is worth a
    /// release: a file the package ships changed, and a commit that touched
    /// the packed files is a feat, fix, or perf commit or marks a breaking
    /// change. Writes nothing and reads no forge; the last release is the
    /// release tag git holds. A rust target's request job runs it before
    /// release-plz.
    Eligible {
        /// The checkout to judge.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },

    /// Publish what `rk release build` reported, after the provenance:
    /// upload every asset, then create the forge release, which creates
    /// the tag last. A rerun keeps what already exists and adds what is missing.
    Publish {
        /// The checkout the release job runs in.
        #[arg(long, default_value = ".")]
        target: Utf8PathBuf,

        /// Emit one JSON object on stdout instead of the human report.
        #[arg(long)]
        json: bool,
    },
}
