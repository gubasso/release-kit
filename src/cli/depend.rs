//! Arguments for `rk depend`.

use camino::Utf8PathBuf;
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

/// Add another project as a dependency of a target, from how the source distributes itself.
#[derive(Debug, Args)]
pub struct DependArgs {
    /// What to do with the dependency.
    #[command(subcommand)]
    pub action: DependAction,
}

/// The depend operations.
#[derive(Debug, Subcommand)]
pub enum DependAction {
    /// Read the source and the target, offline, and report every way the dependency can land.
    Assess(AssessArgs),
    /// Serve the fragment or the native command for one way; seed a manager file only where the target has none.
    Add(AddArgs),
}

/// Arguments for `rk depend assess`.
#[derive(Debug, Args)]
pub struct AssessArgs {
    /// The dependency's checkout: a local directory, never a URL.
    #[arg(long)]
    pub source: Utf8PathBuf,

    /// The project that takes the dependency.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `rk depend add`.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// The dependency's checkout: a local directory, never a URL.
    #[arg(long)]
    pub source: Utf8PathBuf,

    /// The project that takes the dependency.
    #[arg(long, default_value = ".")]
    pub target: Utf8PathBuf,

    /// dev: a tool on PATH in the development environment; prod: a library the code imports.
    #[arg(long, value_enum)]
    pub kind: Kind,

    /// The target's tool manager; required when the target carries several, or none.
    #[arg(long, value_enum)]
    pub manager: Option<Manager>,

    /// How the source is fetched; the first viable channel for the manager by default.
    #[arg(long, value_enum)]
    pub channel: Option<Channel>,

    /// The version to pin: 1.2.3, v1.2.3, or the release URL; the source tree's declared version by default.
    #[arg(long)]
    pub pin: Option<String>,

    /// Write the seed file; without it the fragment or command is printed and nothing is written.
    #[arg(long)]
    pub apply: bool,

    /// Emit one JSON object on stdout instead of the human report.
    #[arg(long)]
    pub json: bool,
}

/// What kind of dependency the target takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum Kind {
    /// A tool on PATH in the development environment.
    Dev,
    /// A library the code imports, through the technology's own manifest.
    Prod,
}

impl Kind {
    /// The wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Prod => "prod",
        }
    }
}

/// The target's tool manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum Manager {
    /// A Nix flake: an input pinned at a tag and its package in the devshell.
    Flake,
    /// mise: one `[tools]` entry in its configuration file.
    Mise,
    /// asdf: one line in `.tool-versions`.
    Asdf,
    /// devbox: one entry in the `packages` array of `devbox.json`.
    Devbox,
}

impl Manager {
    /// The wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flake => "flake",
            Self::Mise => "mise",
            Self::Asdf => "asdf",
            Self::Devbox => "devbox",
        }
    }

    /// Every manager, in the order the reports list them.
    pub const ALL: [Self; 4] = [Self::Flake, Self::Mise, Self::Asdf, Self::Devbox];
}

/// Where the source is fetched from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum Channel {
    /// The crate on crates.io.
    Crates,
    /// The flake the source repository serves, at a tag.
    Flake,
    /// The project on the Python package index.
    Pypi,
    /// The package on the npm registry.
    Npm,
    /// The prebuilt archives attached to a GitHub release.
    GithubRelease,
}

impl Channel {
    /// The wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crates => "crates",
            Self::Flake => "flake",
            Self::Pypi => "pypi",
            Self::Npm => "npm",
            Self::GithubRelease => "github-release",
        }
    }

    /// Every channel, in the order the reports list them.
    pub const ALL: [Self; 5] = [
        Self::Crates,
        Self::Flake,
        Self::Pypi,
        Self::Npm,
        Self::GithubRelease,
    ];
}
