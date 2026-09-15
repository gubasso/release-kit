//! `rk init`: land a technology's deterministic files into a target.
//!
//! A front over the direct writer: the target's evidence is gathered
//! once, this binary's projection is computed from its embedded sources,
//! every destination is decided by elementary ownership, and on `--apply`
//! the writer lands the files and the receipt last under one target lock.
//! Dry-run by default: without `--apply` the destinations and what a
//! fresh production invocation would do with them are listed and nothing
//! is touched; `rk stage` is the full-byte comparison surface. A
//! whole-file destination present on disk with no receipt naming it
//! refuses before any write, every collision collected in one pass, and
//! no force flag exists.
//!
//! SATISFIES landing:a-landing-leaves-a-record
//! SATISFIES landing:a-missing-receipt-is-a-classification

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::init::InitArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::embedded;
use crate::error::RkError;
use crate::held;
use crate::landing::apply::{self, Action, Collision, Prepared};
use crate::landing::manifest::{self, Provider};
use crate::landing::{self, lock};
use crate::output::Output;
use crate::profile::{CapabilityRequests, GitWorkflow, ProfileSnapshot};
use crate::stage::CapabilityNote;

/// One destination and what happened to it.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// The declared ownership kind.
    kind: &'static str,
    /// `created`, `replaced`, `matched`, `preserved`, `drift`, `released`,
    /// or `collision` in a preview.
    action: &'static str,
}

/// One sentinel line left for the operator.
#[derive(Debug, Serialize)]
struct SentinelEntry {
    /// The landed file holding the sentinel.
    path: String,
    /// The 1-indexed line.
    line: usize,
    /// The line's text, trimmed.
    text: String,
}

/// The machine form of a landing report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target directory.
    target: String,
    /// What the project is.
    profile: ProfileSnapshot,
    /// How topic branches reach the trunk.
    git: GitWorkflow,
    /// Which optional products the target requested.
    capabilities: CapabilityRequests,
    /// The resolved project path, where detection or `--repo` named one.
    #[serde(skip_serializing_if = "Option::is_none")]
    repo: Option<String>,
    /// Every capability, in catalog order, with its status.
    selection: Vec<CapabilityNote>,
    /// Why the selected release automation cannot land in this release,
    /// absent where it can or none is selected. A preview reports it and
    /// exits 0; the apply refuses on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    release_unavailable: Option<String>,
    /// Why the provider's licence condition refuses this target, absent
    /// where no condition applies or the licence satisfies it. A preview
    /// reports it and exits 0; the apply refuses on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    licence_refusal: Option<String>,
    /// The Nix destinations this target could not take, each with why;
    /// absent where nothing was withheld.
    #[serde(skip_serializing_if = "Option::is_none")]
    withheld: Option<Vec<landing::Withheld>>,
    /// The destinations a production landing refuses as they stand;
    /// absent where there is none. A preview lists them and exits 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    collisions: Option<Vec<Collision>>,
    config: crate::config::Plan,
    /// Every destination, with its kind and action.
    files: Vec<FileEntry>,
    /// The sentinels an apply left to fill; absent in a preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    sentinels: Option<Vec<SentinelEntry>>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// The withheld list a report carries.
fn withheld_of(prepared: &Prepared) -> Option<Vec<landing::Withheld>> {
    let withheld: Vec<landing::Withheld> = prepared
        .projection
        .omissions
        .iter()
        .map(|omission| landing::Withheld {
            path: omission.destination.clone(),
            reason: omission.reason.clone(),
        })
        .collect();
    (!withheld.is_empty()).then_some(withheld)
}

/// Land the files for `--tech` into `--target`.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for an unknown technology or pair,
/// [`RkError::Refusal`] when the target is missing, already carries a
/// receipt, or a destination collides, [`RkError::Missing`] when an apply
/// resolves no repository, and [`RkError::Io`] on filesystem failure.
pub fn run(args: &InitArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    if !args.target.is_dir() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!(
                    "target {} is not a directory; nothing was written",
                    args.target
                ),
            )
            .expected("an existing directory to land into")
            .target_state("unchanged"),
        ));
    }
    // An apply takes the target before it reads anything of it, the
    // receipt and the configuration included, so the world the decisions
    // describe is the world the writer writes. A preview holds nothing.
    let lock = args
        .apply
        .then(|| lock::acquire(&args.target))
        .transpose()?;
    // One directory descriptor, held from here through the receipt write:
    // every read and every write goes through it, so a root exchanged
    // under the pathname later receives nothing.
    // The proof's pause: the lock is held and the directory is not yet.
    held::pause(apply::PAUSE_VAR, "locked", "proceed-locked");
    let held = lock.as_ref().map_or_else(
        || apply::Held::open(&args.target),
        |lock| apply::Held::open_locked(&args.target, lock),
    )?;
    // The proof's pause: the target is held, and nothing has been read.
    held::pause(apply::PAUSE_VAR, "held", "proceed-held");
    let config = crate::config::load(held.base().as_std_path())?;
    let params = landing::Params::resolve(
        held.base(),
        &landing::Inputs {
            nix: args.nix_packaging.then_some(true),
            reporting_policy: args.reporting_policy.then_some(true),
            scorecard: args.scorecard.then_some(true),
            code_scanning: args
                .code_scanning
                .as_deref()
                .map(Provider::parse)
                .transpose()?,
            ..args.profile.inputs()?
        },
        config.as_ref(),
        None,
        if args.apply {
            landing::Purpose::Init
        } else {
            landing::Purpose::Preview
        },
    )?;
    if let Some(lock) = lock {
        refuse_a_recorded_target(&held)?;
        let prepared = apply::prepare(&held, None, &params, config.as_ref())?;
        let landed = apply::land(&held, None, &prepared, apply::Origin::Init, &lock)?;
        drop(lock);
        report_apply(out, args, &held, &params, &prepared, &landed)
    } else {
        let prepared = apply::prepare(&held, None, &params, config.as_ref())?;
        let repo = (params.repo() != landing::REPO_PLACEHOLDER && !params.repo().is_empty())
            .then(|| params.repo().to_owned());
        if params.repo() == landing::REPO_PLACEHOLDER {
            out.frame(
                "note: no repository detected; an apply derives the owner from --repo <path>",
            );
        }
        preview(out, args, &params, repo, &prepared)
    }
}

/// The capability notes a report carries, in catalog order.
fn selection_of(prepared: &Prepared) -> Vec<CapabilityNote> {
    prepared
        .projection
        .capabilities
        .iter()
        .map(|selection| CapabilityNote::of(selection, &prepared.projection))
        .collect()
}

/// The human lines naming every capability's answer.
pub(crate) fn describe_selection(out: Output, prepared: &Prepared) {
    for note in selection_of(prepared) {
        let mut line = format!("capability {}: {}", note.id, note.status);
        if let Some(reason) = &note.reason {
            line.push_str(" (");
            line.push_str(reason);
            line.push(')');
        }
        out.result_line(line);
        if let Some(action) = &note.action {
            out.result_line(format!("  action: {action}"));
        }
    }
}

/// List every destination with what a production landing would do, and
/// write nothing.
#[allow(
    clippy::too_many_lines,
    reason = "one pass prints the profile, the selection, every decision, and the follow-up, and splitting it would separate a line from the value behind it"
)]
fn preview(
    out: Output,
    args: &InitArgs,
    params: &landing::Params,
    repo: Option<String>,
    prepared: &Prepared,
) -> Result<(), RkError> {
    let flags = params.canonical_flags();
    let flags = if params.forge().is_some() && repo.is_none() {
        format!("{flags} --repo <owner/name>")
    } else {
        flags
    };
    let capabilities = params.capability_flags();
    let mut next = vec![format!(
        "rk init{flags}{capabilities} --target {} --apply",
        args.target
    )];
    if let Some(reason) = prepared.projection.release_unavailable() {
        next.insert(
            0,
            format!("the apply refuses until the release automation resolves: {reason}"),
        );
    }
    if !prepared.collisions.is_empty() {
        next.insert(
            0,
            "resolve each collision above through the rk-setup skill; the apply refuses until then"
                .to_owned(),
        );
    }
    if let Some(reason) = prepared.projection.licence_refusal.as_deref() {
        next.insert(
            0,
            format!("the apply refuses until the licence condition is answered: {reason}"),
        );
    }
    next.push(format!(
        "rk stage --target {} stages the complete candidate for a byte comparison",
        args.target
    ));
    out.result_line(format!(
        "DRY RUN: rk init writes these files into {}; re-run with --apply",
        args.target
    ));
    out.result_line(format!(
        "profile: {}",
        crate::commands::profile::describe(
            params.profile(),
            params.git(),
            params.capabilities(),
            params.repo()
        )
    ));
    describe_selection(out, prepared);
    for decision in &prepared.decisions {
        out.result_line(format!(
            "{} {}",
            decision.action.as_str(),
            decision.destination
        ));
    }
    for collision in &prepared.collisions {
        out.result_line(format!(
            "collision {}: {}",
            collision.path, collision.reason
        ));
    }
    out.result_line(format!(
        "{} {}\n{}",
        prepared.config.action,
        crate::config::CONFIG_PATH,
        prepared.config.content
    ));
    for entry in &prepared.projection.omissions {
        out.result_line(format!("withheld {}: {}", entry.destination, entry.reason));
        if let Some(action) = &entry.action {
            out.result_line(format!("  action: {action}"));
        }
    }
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/10",
        config: prepared.config.clone(),
        mode: "preview",
        target: args.target.to_string(),
        profile: params.profile().clone(),
        git: params.git().clone(),
        capabilities: params.capabilities().clone(),
        repo,
        selection: selection_of(prepared),
        release_unavailable: prepared.projection.release_unavailable().map(str::to_owned),
        licence_refusal: prepared.projection.licence_refusal.clone(),
        withheld: withheld_of(prepared),
        collisions: (!prepared.collisions.is_empty()).then(|| prepared.collisions.clone()),
        files: prepared
            .decisions
            .iter()
            .map(|decision| FileEntry {
                path: decision.destination.clone(),
                kind: decision.kind.as_str(),
                action: decision.action.as_str(),
            })
            .chain(prepared.collisions.iter().map(|collision| FileEntry {
                path: collision.path.clone(),
                kind: landing::kind_of(&collision.path).map_or("unknown", landing::Kind::as_str),
                action: "collision",
            }))
            .collect(),
        sentinels: None,
        next,
    })
}

/// Report a landing the writer completed, with the judgment sentinels the
/// operator still owes.
#[allow(
    clippy::too_many_arguments,
    reason = "the report reads the landed files through the held target and names them by the path the operator gave, which are two arguments for one target"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one pass reports the profile, the selection, every decision, and the sentinels the operator still owes"
)]
fn report_apply(
    out: Output,
    args: &InitArgs,
    held: &apply::Held,
    params: &landing::Params,
    prepared: &Prepared,
    landed: &apply::Landed,
) -> Result<(), RkError> {
    let mut file_entries = Vec::new();
    let mut sentinels = Vec::new();
    out.result_line(format!(
        "profile: {}",
        crate::commands::profile::describe(
            params.profile(),
            params.git(),
            params.capabilities(),
            params.repo()
        )
    ));
    describe_selection(out, prepared);
    for decision in &prepared.decisions {
        out.result_line(describe(decision));
        if decision.action != Action::Released {
            let bytes =
                landing::read_recorded(held.base(), &decision.destination)?.unwrap_or_default();
            collect_sentinels(
                held.display(),
                &decision.destination,
                &bytes,
                &mut sentinels,
            );
        }
        file_entries.push(FileEntry {
            path: decision.destination.clone(),
            kind: decision.kind.as_str(),
            action: decision.action.as_str(),
        });
    }
    for entry in &prepared.projection.omissions {
        out.result_line(format!("withheld {}: {}", entry.destination, entry.reason));
        if let Some(action) = &entry.action {
            out.result_line(format!("  action: {action}"));
        }
    }
    out.result_line(format!(
        "{} {}",
        prepared.config.action,
        crate::config::CONFIG_PATH
    ));
    for (key, empty_line) in [
        ("setup.required_check", "required_check = \"\""),
        ("setup.bot.app_id", "app_id = \"\""),
    ] {
        if let Some((index, _)) = prepared
            .config
            .content
            .lines()
            .enumerate()
            .find(|(_, line)| line.starts_with(empty_line))
        {
            sentinels.push(SentinelEntry {
                path: crate::config::CONFIG_PATH.into(),
                line: index + 1,
                text: format!("set {key} before forge setup"),
            });
        }
    }
    out.result_line(format!("wrote {}", manifest::MANIFEST_PATH));
    debug_assert_eq!(
        landed.completed.last().map(String::as_str),
        Some(manifest::MANIFEST_PATH)
    );

    if sentinels.is_empty() {
        out.result_line("no sentinels to fill");
    } else {
        out.result_line("fill these sentinels before the workflow runs:");
        for sentinel in &sentinels {
            out.result_line(format!(
                "{}:{}: {}",
                sentinel.path, sentinel.line, sentinel.text
            ));
        }
    }
    let mut next = vec![
        if sentinels.is_empty() {
            "commit the landed files, the receipt included".to_owned()
        } else {
            "fill each sentinel above, then commit the landed files, the receipt included"
                .to_owned()
        },
        format!("rk status --target {} reports this landing", args.target),
    ];
    // Only an automatic release routes to the bot-operate chapter; an
    // external or none release has no bot to operate.
    if params.release_mode() == crate::profile::ReleaseMode::Automatic {
        next.push("rk method setup orders what follows".to_owned());
    } else if params.forge().is_some() {
        next.push("rk setup --target . previews the applicable forge steps".to_owned());
    }
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/10",
        config: prepared.config.clone(),
        mode: "apply",
        target: args.target.to_string(),
        profile: params.profile().clone(),
        git: params.git().clone(),
        capabilities: params.capabilities().clone(),
        repo: (!params.repo().is_empty()).then(|| params.repo().to_owned()),
        selection: selection_of(prepared),
        release_unavailable: None,
        licence_refusal: prepared.projection.licence_refusal.clone(),
        withheld: withheld_of(prepared),
        collisions: None,
        files: file_entries,
        sentinels: Some(sentinels),
        next,
    })
}

/// The human line for one decision.
pub(crate) fn describe(decision: &apply::Decision) -> String {
    match decision.action {
        Action::Preserved | Action::Drift => format!(
            "{} {} ({}, target-owned)",
            decision.action.as_str(),
            decision.destination,
            decision.kind.as_str()
        ),
        Action::Released => format!(
            "released {} (no longer produced; target-owned from this landing)",
            decision.destination
        ),
        Action::Matched => format!(
            "matched {} (already holds the candidate's bytes)",
            decision.destination
        ),
        Action::Created | Action::Replaced => {
            format!("{} {}", decision.action.as_str(), decision.destination)
        }
    }
}

/// A re-landing over an existing receipt is `rk upgrade`'s job, not a
/// second `rk init`.
fn refuse_a_recorded_target(held: &apply::Held) -> Result<(), RkError> {
    if landing::manifest::load(held.base())?.is_none() {
        return Ok(());
    }
    let target = held.display();
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "{target} already carries {}, and nothing was written",
                manifest::MANIFEST_PATH
            ),
        )
        .expected("a target without a landing receipt")
        .action(format!(
            "rk upgrade --target {target} takes it to this binary's projection"
        ))
        .target_state("unchanged"),
    ))
}

/// Collect every judgment-sentinel line one landed file carries, so
/// nothing stays half-configured silently.
fn collect_sentinels(
    target: &Utf8Path,
    destination: &str,
    bytes: &[u8],
    found: &mut Vec<SentinelEntry>,
) {
    let text = String::from_utf8_lossy(bytes);
    for (idx, line) in text.lines().enumerate() {
        if line.contains(embedded::SENTINEL) {
            found.push(SentinelEntry {
                path: target.join(destination).to_string(),
                line: idx + 1,
                text: line.trim().to_owned(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FileEntry, Report, SentinelEntry};
    use crate::landing::CheckoutMode;
    use crate::landing::Integration;
    use crate::profile::{
        CapabilityRequests, GitWorkflow, ProfileSnapshot, ReleaseIntent, ReleaseMode,
    };
    use crate::stage::CapabilityNote;

    /// The complete `rk.init/10` shape, held by snapshot in both modes: a
    /// field rename or removal fails here and becomes a schema-version
    /// bump instead of a silent parser break at some agent.
    #[test]
    fn the_init_report_schema_snapshot_holds() {
        let apply = Report {
            schema: "rk.init/10",
            config: crate::config::Plan {
                action: "added",
                changes: vec![],
                content: "schema_version = 2\n".into(),
            },
            mode: "apply",
            target: "/tmp/t".into(),
            profile: ProfileSnapshot {
                technologies: vec!["rust".into()],
                forge: Some("github".into()),
                release: ReleaseIntent {
                    mode: ReleaseMode::Automatic,
                    driver: Some("rust".into()),
                    style: Some(crate::landing::Style::Trunk),
                    line_prefix: Some("release/".into()),
                },
            },
            git: GitWorkflow {
                trunk: "master".into(),
                checkout_mode: CheckoutMode::LinkedWorktree,
                integration: Integration::Local,
            },
            capabilities: CapabilityRequests {
                nix_packaging: true,
                reporting_policy: true,
                scorecard: true,
                code_scanning: Some(crate::landing::Provider::CodeQl),
            },
            repo: Some("acme/widget".into()),
            selection: vec![CapabilityNote {
                id: "git.guards".into(),
                status: "selected".into(),
                reason: None,
                action: None,
                destinations: vec!["AGENTS.md".into()],
            }],
            release_unavailable: None,
            licence_refusal: None,
            withheld: Some(vec![crate::landing::Withheld {
                path: "flake.nix".into(),
                reason: "the target already carries flake.nix".into(),
            }]),
            collisions: None,
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                kind: "seeded",
                action: "created",
            }],
            sentinels: Some(vec![SentinelEntry {
                path: "/tmp/t/release-plz.toml".into(),
                line: 3,
                text: "# TODO(release-kit): keep false for a binary-only crate".into(),
            }]),
            next: vec!["commit the landed files, the receipt included".into()],
        };
        assert_eq!(
            serde_json::to_string(&apply).expect("a report serializes"),
            r##"{"schema":"rk.init/10","mode":"apply","target":"/tmp/t","profile":{"technologies":["rust"],"forge":"github","release":{"mode":"automatic","driver":"rust","style":"trunk","line_prefix":"release/"}},"git":{"trunk":"master","checkout_mode":"linked-worktree","integration":"local"},"capabilities":{"nix_packaging":true,"reporting_policy":true,"scorecard":true,"code_scanning":"codeql"},"repo":"acme/widget","selection":[{"id":"git.guards","status":"selected","destinations":["AGENTS.md"]}],"withheld":[{"path":"flake.nix","reason":"the target already carries flake.nix"}],"config":{"action":"added","changes":[],"content":"schema_version = 2\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"created"}],"sentinels":[{"path":"/tmp/t/release-plz.toml","line":3,"text":"# TODO(release-kit): keep false for a binary-only crate"}],"next":["commit the landed files, the receipt included"]}"##
        );
        let preview = Report {
            sentinels: None,
            repo: None,
            mode: "preview",
            capabilities: CapabilityRequests {
                nix_packaging: false,
                reporting_policy: false,
                scorecard: false,
                code_scanning: None,
            },
            selection: vec![],
            release_unavailable: Some(
                "the release automation at (python, gitlab) has no landable files".to_owned(),
            ),
            licence_refusal: Some("the target's Cargo.toml declares no license field".to_owned()),
            withheld: None,
            collisions: Some(vec![super::Collision {
                path: "SECURITY.md".into(),
                reason: "exists, and no receipt attributes it to release-kit".into(),
            }]),
            ..apply
        };
        assert_eq!(
            serde_json::to_string(&preview).expect("a report serializes"),
            r#"{"schema":"rk.init/10","mode":"preview","target":"/tmp/t","profile":{"technologies":["rust"],"forge":"github","release":{"mode":"automatic","driver":"rust","style":"trunk","line_prefix":"release/"}},"git":{"trunk":"master","checkout_mode":"linked-worktree","integration":"local"},"capabilities":{"nix_packaging":false,"reporting_policy":false,"scorecard":false},"selection":[],"release_unavailable":"the release automation at (python, gitlab) has no landable files","licence_refusal":"the target's Cargo.toml declares no license field","collisions":[{"path":"SECURITY.md","reason":"exists, and no receipt attributes it to release-kit"}],"config":{"action":"added","changes":[],"content":"schema_version = 2\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"created"}],"next":["commit the landed files, the receipt included"]}"#,
            "a preview omits the sentinels, the unresolved repo, and an empty withheld list rather than serializing null"
        );
    }
}
