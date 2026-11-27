//! The version gate: every command's class, and the check a gated command
//! makes against its target's landing record before its handler runs.
//!
//! [`CLASSES`] is the one table of every action and its class. [`classify`]
//! resolves a parsed command to its class, its effect, and its target, and
//! [`require_aligned`] judges the record against this binary. The gate
//! never writes, never moves a pin, and never runs `init` or `upgrade` by
//! itself.
//!
//! SATISFIES version-gate:every-command-has-a-gate-class

use camino::{Utf8Path, Utf8PathBuf};

use super::manifest::{self, Alignment};
use crate::cli::Commands;
use crate::cli::branches::BranchesAction;
use crate::cli::issue::IssueAction;
use crate::cli::lines::LinesAction;
use crate::cli::migrate::MigrateAction;
use crate::cli::release::ReleaseAction;
use crate::cli::setup::SetupAction;
use crate::cli::stage::StageAction;
use crate::cli::worktree::WorktreeAction;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// The class of one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Needs no target, or must never fail: never reads the record.
    Exempt,
    /// Runs on a target with no record by design. On a target with a
    /// record it follows its own effect.
    PreLanding,
    /// `upgrade` alone, which keeps its own checks.
    Upgrade,
    /// Reads a landed target and writes nothing.
    ReadOnly,
    /// Writes a landed target or its forge.
    Writing,
}

impl Class {
    /// The table form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exempt => "exempt",
            Self::PreLanding => "pre-landing",
            Self::Upgrade => "upgrade",
            Self::ReadOnly => "read-only",
            Self::Writing => "writing",
        }
    }
}

/// Every action of every command family and its class.
///
/// A key is the space-joined path of subcommand names. A writing action's
/// preview is read-only, and a pre-landing action follows its own effect
/// on a target with a record.
pub const CLASSES: &[(&str, Class)] = &[
    ("method", Class::Exempt),
    ("binding", Class::Exempt),
    ("snippet", Class::Exempt),
    ("guide", Class::Exempt),
    ("forge", Class::Exempt),
    ("versions", Class::Exempt),
    ("init", Class::PreLanding),
    ("profile", Class::PreLanding),
    ("status", Class::Exempt),
    ("upgrade", Class::Upgrade),
    ("adopt", Class::PreLanding),
    ("assess", Class::PreLanding),
    ("stage", Class::PreLanding),
    ("stage clean", Class::Exempt),
    ("stage compare", Class::PreLanding),
    ("setup", Class::PreLanding),
    ("setup check", Class::PreLanding),
    ("setup checkpoint", Class::PreLanding),
    ("setup status", Class::PreLanding),
    ("setup step", Class::PreLanding),
    ("setup script", Class::Exempt),
    ("branches prune", Class::Writing),
    ("lines list", Class::ReadOnly),
    ("lines open", Class::Writing),
    ("lines mint", Class::Writing),
    ("lines rc", Class::ReadOnly),
    ("lines retire", Class::Writing),
    ("release status", Class::ReadOnly),
    ("release merge", Class::Writing),
    ("release request", Class::Writing),
    ("release build", Class::Writing),
    ("release eligible", Class::ReadOnly),
    ("release publish", Class::Writing),
    ("integrate", Class::Writing),
    ("message", Class::ReadOnly),
    ("migrate inventory", Class::PreLanding),
    ("migrate archive", Class::PreLanding),
    ("migrate mark", Class::PreLanding),
    ("migrate status", Class::PreLanding),
    ("migrate close", Class::PreLanding),
    ("worktree list", Class::ReadOnly),
    ("worktree add", Class::Writing),
    ("worktree prune", Class::Writing),
    ("issue start", Class::Writing),
    ("runs", Class::Exempt),
    ("skill", Class::Exempt),
    ("doctor", Class::Exempt),
    ("usage", Class::Exempt),
    ("license", Class::Exempt),
    ("completions", Class::Exempt),
];

/// What a gated action does to its target, which decides its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Reads, or previews a write.
    ReadOnly,
    /// Writes the target or its forge.
    Writing,
}

/// One gated action: its class, its effect, and the target it judges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gated {
    /// The class the table gives it.
    pub class: Class,
    /// What this invocation does.
    pub effect: Effect,
    /// The repository whose record the gate reads.
    pub target: Utf8PathBuf,
}

const fn effect(apply: bool) -> Effect {
    if apply {
        Effect::Writing
    } else {
        Effect::ReadOnly
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "every arm of classify answers an Option, and this builds the Some arm"
)]
fn gated(class: Class, effect: Effect, target: &Utf8Path) -> Option<Gated> {
    Some(Gated {
        class,
        effect,
        target: target.to_owned(),
    })
}

/// The class, effect, and target of one parsed command, or `None` for an
/// action the gate never judges: an exempt action, `upgrade`, and an
/// action with no target.
#[must_use]
pub fn classify(command: &Commands) -> Option<Gated> {
    use Class::{PreLanding, ReadOnly, Writing};
    use Effect::{ReadOnly as Reads, Writing as Writes};
    match command {
        Commands::Method(_)
        | Commands::Binding(_)
        | Commands::Snippet(_)
        | Commands::Guide(_)
        | Commands::Forge(_)
        | Commands::Versions(_)
        | Commands::Status(_)
        | Commands::Upgrade(_)
        | Commands::Runs(_)
        | Commands::Skill(_)
        | Commands::Doctor(_)
        | Commands::Usage
        | Commands::License
        | Commands::Completions(_) => None,
        Commands::Init(args) => gated(PreLanding, effect(args.apply), &args.target),
        Commands::Adopt(args) => gated(PreLanding, effect(args.apply), &args.target),
        Commands::Profile(args) => gated(PreLanding, Reads, &args.target),
        Commands::Assess(args) => gated(PreLanding, Reads, &args.target),
        Commands::Stage(args) => match &args.action {
            None => gated(PreLanding, Reads, &args.target),
            Some(StageAction::Clean { .. }) => None,
            Some(StageAction::Compare { target, .. }) => gated(PreLanding, Reads, target),
        },
        Commands::Setup(args) => match &args.action {
            None if args.list => None,
            None => args
                .target
                .as_deref()
                .and_then(|target| gated(PreLanding, effect(args.apply), target)),
            // The import writes outside the target and reads only its
            // configuration.
            Some(
                SetupAction::Check { target, .. }
                | SetupAction::Status { target, .. }
                | SetupAction::ImportKey { target, .. },
            ) => gated(PreLanding, Reads, target),
            Some(SetupAction::Checkpoint { target, .. }) => gated(PreLanding, Writes, target),
            Some(SetupAction::Step { target, apply, .. }) => {
                gated(PreLanding, effect(*apply), target)
            }
            Some(SetupAction::Script { .. }) => None,
        },
        Commands::Migrate(args) => match &args.action {
            MigrateAction::Inventory { target, .. } | MigrateAction::Status { target, .. } => {
                gated(PreLanding, Reads, target)
            }
            MigrateAction::Archive { target, apply, .. }
            | MigrateAction::Close { target, apply, .. } => {
                gated(PreLanding, effect(*apply), target)
            }
            MigrateAction::Mark { target, .. } => gated(PreLanding, Writes, target),
        },
        Commands::Branches(args) => match &args.action {
            BranchesAction::Prune { target, apply, .. } => gated(Writing, effect(*apply), target),
        },
        Commands::Lines(args) => match &args.action {
            LinesAction::List { target, .. } | LinesAction::Rc { target, .. } => {
                gated(ReadOnly, Reads, target)
            }
            LinesAction::Open { target, apply, .. }
            | LinesAction::Mint { target, apply, .. }
            | LinesAction::Retire { target, apply, .. } => gated(Writing, effect(*apply), target),
        },
        Commands::Release(args) => match &args.action {
            ReleaseAction::Status { target, .. } | ReleaseAction::Eligible { target, .. } => {
                gated(ReadOnly, Reads, target)
            }
            ReleaseAction::Merge { target, apply, .. } => gated(Writing, effect(*apply), target),
            ReleaseAction::Request { target, .. } | ReleaseAction::Publish { target, .. } => {
                gated(Writing, Writes, target)
            }
            ReleaseAction::Build { target, list, .. } => gated(Writing, effect(!*list), target),
        },
        Commands::Integrate(args) => gated(Writing, effect(args.apply), &args.target),
        Commands::Message(args) => gated(ReadOnly, Reads, &args.target),
        Commands::Worktree(args) => match &args.action {
            WorktreeAction::List { target, .. } => gated(ReadOnly, Reads, target),
            WorktreeAction::Add { target, apply, .. }
            | WorktreeAction::Prune { target, apply, .. } => gated(Writing, effect(*apply), target),
        },
        Commands::Issue(args) => match &args.action {
            IssueAction::Start { target, apply, .. } => gated(Writing, effect(*apply), target),
        },
    }
}

/// Judge `command`'s target record against this binary before its handler
/// runs, by the rows of `version-gate:a-gated-command-names-its-fix`.
///
/// SATISFIES version-gate:a-gated-command-names-its-fix
///
/// # Errors
///
/// A refusal with the reason `StateDrift` and exit 73 naming the fix, and
/// the record's own taxonomy for a record that does not load.
pub fn require_aligned(command: &Commands) -> Result<(), RkError> {
    let Some(gated) = classify(command) else {
        return Ok(());
    };
    // A target that is no directory is the handler's to report, under its
    // own reason: the gate judges a record, and there is no place for one.
    if !gated.target.is_dir() {
        return Ok(());
    }
    // An integration makes the branch record the trunk's, so it agrees
    // with the record at the tip of the branch. A branch with no record
    // there falls through to the target's.
    if let Commands::Integrate(args) = command
        && let Some(recorded) = branch_record_version(&gated.target, &args.branch)
    {
        return judge_branch(&gated, &args.branch, &recorded);
    }
    // A record that does not load is the handler's to report, under the
    // record's own taxonomy and after any lock the handler takes first.
    let Ok(loaded) = manifest::load(&gated.target) else {
        return Ok(());
    };
    let Some(record) = loaded else {
        if gated.class == Class::PreLanding {
            return Ok(());
        }
        return Err(no_record(&gated.target));
    };
    let binary = env!("CARGO_PKG_VERSION");
    let recorded = record.rk_version.as_str();
    match manifest::alignment(recorded, binary) {
        Alignment::Aligned => Ok(()),
        Alignment::BinaryNewer => {
            if gated.effect == Effect::ReadOnly {
                crate::output::Output::human().warn(binary_newer_warning(recorded));
                return Ok(());
            }
            Err(binary_newer(&gated.target, recorded))
        }
        Alignment::TargetNewer => Err(record_newer(recorded)),
    }
}

/// The `rk_version` of the record at the tip of `branch` in `target`,
/// where the branch resolves and carries a record. It reads that one
/// field, so a record at a newer schema still names its release.
///
/// SATISFIES version-gate:integrate-judges-the-branch-record
fn branch_record_version(target: &Utf8Path, branch: &str) -> Option<String> {
    let out = std::process::Command::new(crate::probes::git_bin())
        .arg("-C")
        .arg(target)
        .args([
            "show",
            &format!("refs/heads/{branch}:{}", manifest::MANIFEST_PATH),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    value["rk_version"].as_str().map(str::to_owned)
}

/// The worktree that has `branch` checked out, where one does.
fn seat_of(target: &Utf8Path, branch: &str) -> Option<Utf8PathBuf> {
    let out = std::process::Command::new(crate::probes::git_bin())
        .arg("-C")
        .arg(target)
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()
        .ok()?;
    crate::worktree::parse_worktrees(&out.stdout)
        .ok()?
        .into_iter()
        .find(|seat| seat.branch.as_deref() == Some(branch))
        .map(|seat| seat.path)
}

/// Judge an integration against the record of its branch, by the rows of
/// `version-gate:a-gated-command-names-its-fix`.
fn judge_branch(gated: &Gated, branch: &str, recorded: &str) -> Result<(), RkError> {
    let binary = env!("CARGO_PKG_VERSION");
    let seat = seat_of(&gated.target, branch)
        .map_or_else(|| "<worktree>".to_owned(), |seat| seat.to_string());
    let target = &gated.target;
    match manifest::alignment(recorded, binary) {
        Alignment::Aligned => Ok(()),
        Alignment::BinaryNewer => {
            let warning = format!(
                "branch {branch} records rk {recorded} and this rk is {binary}; run `rk upgrade --target {seat} --apply` on the branch and commit it"
            );
            if gated.effect == Effect::ReadOnly {
                crate::output::Output::human().warn(warning);
                return Ok(());
            }
            Err(RkError::refusal(
                Diagnostic::new(Reason::StateDrift, warning)
                    .expected("a binary at the rk_version of the branch's record")
                    .action(format!(
                        "run `rk upgrade --target {seat} --apply` in the branch's worktree, commit the landing, then run this command again. Where the branch pins rk {recorded}, {OWN_RK}"
                    ))
                    .target_state("unchanged"),
            ))
        }
        Alignment::TargetNewer => Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "branch {branch} records rk {recorded} and this rk is {binary}, which is older"
                ),
            )
            .expected("a binary at the rk_version of the branch's record")
            .action(format!(
                "run the rk of the branch's environment: `direnv exec {seat} rk integrate {branch} --target {target}`. From inside the worktree, `rk integrate {branch} --target .` works on every release, including one before 0.10.1, which judges the trunk record"
            ))
            .target_state("unchanged"),
        )),
    }
}

/// The run hint for the project's own rk.
const OWN_RK: &str =
    "run the rk of this project's environment, for example through `direnv exec . rk ...`";

fn no_record(target: &Utf8Path) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!("{target} has no release-kit landing, and this command reads one"),
        )
        .expected("a landing record at .release-kit/manifest.json")
        .action(format!(
            "run `rk init --target {target}` to land it. Read the setup skill with `rk skill show rk-setup`, and each file it names with `rk skill show rk-setup --file <path>`"
        ))
        .target_state("unchanged"),
    )
}

/// The skill that runs an upgrade and provisions the binary.
pub const SKILL: &str = "rk-setup";

/// The route after a pin move, through the setup skill of the new rk.
///
/// The gate and `rk upgrade` carry this one wording, and the managed
/// block of `AGENTS.md` carries its short form. It is shared with sdd
/// word for word, so a reader who moves both pins reads one procedure.
///
/// SATISFIES version-gate:a-gated-command-names-its-fix
#[must_use]
pub fn after_move() -> String {
    format!(
        "After the pin move, load the devshell and run the upgrade through the {SKILL} skill of the rk in that devshell, and do every migration it names. Before `rk upgrade`, read it with `rk skill show {SKILL}`. After it, the project's .claude/skills/{SKILL} is that skill. Never use a host copy or an older skill."
    )
}

fn binary_newer(target: &Utf8Path, recorded: &str) -> RkError {
    let binary = env!("CARGO_PKG_VERSION");
    let action = format!(
        "run `rk upgrade --target {target} --apply` to move the landing, then run this command again. Where the project pins rk {recorded}, {OWN_RK}. {}",
        after_move()
    );
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!("the landing is at rk {recorded} and this rk is {binary}"),
        )
        .expected("a binary at the landing's rk_version")
        .action(action)
        .target_state("unchanged"),
    )
}

fn binary_newer_warning(recorded: &str) -> String {
    let binary = env!("CARGO_PKG_VERSION");
    format!(
        "the landing is at rk {recorded} and this rk is {binary}; run `rk upgrade` to move the landing"
    )
}

fn record_newer(recorded: &str) -> RkError {
    let binary = env!("CARGO_PKG_VERSION");
    let action = format!(
        "{OWN_RK}. Where that environment lacks it, install rk {recorded} through the project's tool manager, by the `provision` task of the {SKILL} skill: `rk skill show {SKILL} --file references/provision.md`"
    );
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!("the landing is at rk {recorded} and this rk is {binary}, which is older"),
        )
        .expected("a binary at the landing's rk_version")
        .action(action)
        .target_state("unchanged"),
    )
}

#[cfg(test)]
mod tests {
    use super::CLASSES;
    use clap::CommandFactory as _;

    /// Every command path of the clap tree, as the space-joined names a
    /// [`CLASSES`] key uses, leaving out a command whose subcommand is
    /// required and the generated help.
    fn paths(command: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for sub in command.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            let path = if prefix.is_empty() {
                sub.get_name().to_owned()
            } else {
                format!("{prefix} {}", sub.get_name())
            };
            if !sub.is_subcommand_required_set() || sub.get_subcommands().next().is_none() {
                out.push(path.clone());
            }
            paths(sub, &path, out);
        }
    }

    /// VERIFIES version-gate:every-command-has-a-gate-class
    ///
    /// A new action without a class fails here, and so does a class whose
    /// action is gone.
    #[test]
    fn every_action_has_a_gate_class() {
        let mut all = Vec::new();
        paths(&crate::cli::Cli::command(), "", &mut all);
        let classed = |path: &str| {
            let words: Vec<&str> = path.split(' ').collect();
            (1..=words.len()).any(|end| {
                let prefix = words[..end].join(" ");
                CLASSES.iter().any(|(key, _)| *key == prefix)
            })
        };
        for path in &all {
            assert!(classed(path), "{path} has no gate class");
        }
        for (key, _) in CLASSES {
            assert!(
                all.iter().any(|path| path == key)
                    || all.iter().any(|path| path.starts_with(&format!("{key} "))),
                "{key} names no command"
            );
        }
    }
}
