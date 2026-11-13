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

    /// Keep the one release request of the pushed base on
    /// `rk-release/<base>`: propose the next version, write it and the
    /// changelog, force-push as the bot, open or edit the request, and arm
    /// it where the record's style arms. Runs in CI on a push to the
    /// trunk or a release line; every fact comes from the record.
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
