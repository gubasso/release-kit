//! Command-line surface: clap derive types only, one module per
//! subcommand's arguments. No behavior lives here; each variant routes to
//! its handler in `commands`.

pub mod completions;
pub mod doctor;
pub mod init;
pub mod payload;
pub mod read;
pub mod skill;

use clap::{Parser, Subcommand};

/// The release-kit CLI: reads the canon and lands the deterministic files.
#[derive(Debug, Parser)]
#[command(name = "rk", version, about, propagate_version = true)]
pub struct Cli {
    /// Which subcommand to run.
    #[command(subcommand)]
    pub command: Commands,
}

/// Every subcommand the binary offers.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Read the technology-agnostic method chapters.
    Method(read::ReadArgs),
    /// Read the per-technology bindings.
    Binding(read::ReadArgs),
    /// Read the deterministic files a binding lands.
    Snippet(read::ReadArgs),
    /// Print the pinned-tool registry.
    Versions,
    /// Report the payload this binary carries, with its digests.
    Payload(payload::PayloadArgs),
    /// Land a technology's files into a target repository.
    Init(init::InitArgs),
    /// Manage the agent skills at user scope.
    Skill(skill::SkillArgs),
    /// Run every environment probe and report by class.
    Doctor(doctor::DoctorArgs),
    /// Print the whole command surface in one call.
    Usage,
    /// Print the license terms the binary carries.
    License,
    /// Generate shell completions.
    Completions(completions::CompletionsArgs),
}
