//! Command-line surface: clap derive types only, one module per
//! subcommand's arguments. No behavior lives here; each variant routes to
//! its handler in `commands`.

pub mod adopt;
pub mod assess;
pub mod branches;
pub mod completions;
pub mod devshell;
pub mod doctor;
pub mod guide;
pub mod init;
pub mod lines;
pub mod message;
pub mod payload;
pub mod read;
pub mod runs;
pub mod setup;
pub mod skill;
pub mod status;
pub mod upgrade;
pub mod versions;
pub mod worktree;

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
    /// Print a runbook with what detection knows filled in.
    Guide(guide::GuideArgs),
    /// Read the per-forge documents.
    Forge(read::ReadArgs),
    /// Print the pinned-tool registry, with --check as its online freshness report.
    Versions(versions::VersionsArgs),
    /// Report the payload this binary carries, with its digests.
    Payload(payload::PayloadArgs),
    /// Land a technology's files into a target repository.
    Init(init::InitArgs),
    /// Report what landed in a target and whether it drifted.
    Status(status::StatusArgs),
    /// Take a landed target to this binary's payload.
    Upgrade(upgrade::UpgradeArgs),
    /// Record a target landed before the record existed.
    Adopt(adopt::AdoptArgs),
    /// Classify a target before anything lands: greenfield, brownfield, or needs-decision.
    Assess(assess::AssessArgs),
    /// Execute the repository-side setup against the detected forge.
    Setup(setup::SetupArgs),
    /// Report and prune local branches the forge already merged.
    Branches(branches::BranchesArgs),
    /// Open, inventory, and retire the release lines.
    Lines(lines::LinesArgs),
    /// Judge a commit message, title, or body against the content guards.
    Message(message::MessageArgs),
    /// Inspect, create, and prune the linked worktrees beside a checkout.
    Worktree(worktree::WorktreeArgs),
    /// Inspect and prune the run journals.
    Runs(runs::RunsArgs),
    /// Manage the agent skills at user scope.
    Skill(skill::SkillArgs),
    /// Wire release-kit as a consumer's devshell dependency and keep its pin fresh.
    Devshell(devshell::DevshellArgs),
    /// Run every environment probe and report by class.
    Doctor(doctor::DoctorArgs),
    /// Print the whole command surface in one call.
    Usage,
    /// Print the license terms the binary carries.
    License,
    /// Generate shell completions.
    Completions(completions::CompletionsArgs),
}
