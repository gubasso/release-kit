//! Arguments for `rk message`.

use camino::Utf8PathBuf;
use clap::{Args, ValueEnum};

/// Judge a commit message, title, or body against the content guards.
#[derive(Debug, Args)]
pub struct MessageArgs {
    /// The file holding the text, `-` or absent for stdin; pre-commit's
    /// commit-msg stage passes the message file here.
    pub file: Option<Utf8PathBuf>,

    /// Exit 1 when any finding stands, instead of only reporting.
    #[arg(long)]
    pub check: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,

    /// What the text is: a whole commit message, a title alone, or a
    /// request body.
    #[arg(long, value_enum, default_value_t = MessageKind::Commit)]
    pub kind: MessageKind,

    /// The repository whose ignore rules judge a referenced path.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// The request title a body belongs to, for the bot exemption.
    #[arg(long)]
    pub title: Option<String>,
}

/// The three text shapes the verb judges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MessageKind {
    /// A whole commit message; its first line is the title.
    Commit,
    /// A title alone.
    Title,
    /// A request body; `--title` supplies its title.
    Body,
}

impl MessageKind {
    /// The kind's report name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Title => "title",
            Self::Body => "body",
        }
    }
}
