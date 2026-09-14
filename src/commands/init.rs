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
use crate::landing::manifest::{self, Style, Workflow};
use crate::landing::{self, lock};
use crate::output::Output;

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
    /// The technology whose files land.
    tech: String,
    /// The forge whose subtree lands.
    forge: String,
    /// The target directory.
    target: String,
    /// The resolved project path, where detection or `--repo` named one.
    #[serde(skip_serializing_if = "Option::is_none")]
    repo: Option<String>,
    /// The working-copy mode the landing records and renders under.
    workflow: &'static str,
    style: &'static str,
    /// Whether the landing carries the Nix capability.
    nix: bool,
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
            tech: args.tech.as_deref(),
            forge: args.forge.as_deref(),
            repo: args.repo.as_deref(),
            workflow: args.workflow.as_deref().map(Workflow::parse).transpose()?,
            style: args.style.as_deref().map(Style::parse).transpose()?,
            nix: args.nix.then_some(true),
        },
        config.as_ref(),
        None,
        if args.apply {
            landing::Purpose::Init
        } else {
            landing::Purpose::Preview
        },
    )?;
    let style = params
        .style()
        .ok_or_else(|| RkError::Usage("landing style is unresolved".into()))?;
    if let Some(lock) = lock {
        refuse_a_recorded_target(&held)?;
        let prepared = apply::prepare(&held, None, &params, config.as_ref())?;
        let landed = apply::land(&held, None, &prepared, apply::Origin::Init, &lock)?;
        drop(lock);
        report_apply(out, args, &held, &params, style, &prepared, &landed)
    } else {
        let prepared = apply::prepare(&held, None, &params, config.as_ref())?;
        let repo = (params.repo() != landing::REPO_PLACEHOLDER).then(|| params.repo().to_owned());
        if repo.is_none() {
            out.frame(
                "note: no repository detected; an apply derives the owner from --repo <path>",
            );
        }
        preview(out, args, &params, repo, style, &prepared)
    }
}

/// List every destination with what a production landing would do, and
/// write nothing.
fn preview(
    out: Output,
    args: &InitArgs,
    params: &landing::Params,
    repo: Option<String>,
    style: Style,
    prepared: &Prepared,
) -> Result<(), RkError> {
    let repo_argument = repo.as_deref().unwrap_or("<owner/name>");
    let nix_flag = if params.nix() { " --nix" } else { "" };
    let mut next = vec![format!(
        "rk init --tech {} --forge {} --repo {repo_argument} --workflow {} --style {}{nix_flag} --target {} --apply",
        params.tech(),
        params.forge(),
        params.workflow().as_str(),
        style.as_str(),
        args.target
    )];
    if !prepared.collisions.is_empty() {
        next.insert(
            0,
            "resolve each collision above through the rk-setup skill; the apply refuses until then"
                .to_owned(),
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
    }
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/7",
        config: prepared.config.clone(),
        mode: "preview",
        tech: params.tech().to_owned(),
        forge: params.forge().to_owned(),
        target: args.target.to_string(),
        repo,
        workflow: params.workflow().as_str(),
        style: style.as_str(),
        nix: params.nix(),
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
fn report_apply(
    out: Output,
    args: &InitArgs,
    held: &apply::Held,
    params: &landing::Params,
    style: Style,
    prepared: &Prepared,
    landed: &apply::Landed,
) -> Result<(), RkError> {
    let mut file_entries = Vec::new();
    let mut sentinels = Vec::new();
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
    let next = vec![
        if sentinels.is_empty() {
            "commit the landed files, the receipt included".to_owned()
        } else {
            "fill each sentinel above, then commit the landed files, the receipt included"
                .to_owned()
        },
        format!("rk status --target {} reports this landing", args.target),
        "rk method setup orders what follows".to_owned(),
    ];
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/7",
        config: prepared.config.clone(),
        mode: "apply",
        tech: params.tech().to_owned(),
        forge: params.forge().to_owned(),
        target: args.target.to_string(),
        repo: Some(params.repo().to_owned()),
        workflow: params.workflow().as_str(),
        style: style.as_str(),
        nix: params.nix(),
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

    /// The complete `rk.init/7` shape, held by snapshot in both modes: a
    /// field rename or removal fails here and becomes a schema-version
    /// bump instead of a silent parser break at some agent.
    #[test]
    fn the_init_report_schema_snapshot_holds() {
        let apply = Report {
            schema: "rk.init/7",
            config: crate::config::Plan {
                action: "added",
                changes: vec![],
                content: "schema_version = 1\n".into(),
            },
            mode: "apply",
            tech: "rust".into(),
            forge: "github".into(),
            target: "/tmp/t".into(),
            repo: Some("acme/widget".into()),
            workflow: "worktree",
            style: "trunk",
            nix: true,
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
            r##"{"schema":"rk.init/7","mode":"apply","tech":"rust","forge":"github","target":"/tmp/t","repo":"acme/widget","workflow":"worktree","style":"trunk","nix":true,"withheld":[{"path":"flake.nix","reason":"the target already carries flake.nix"}],"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"created"}],"sentinels":[{"path":"/tmp/t/release-plz.toml","line":3,"text":"# TODO(release-kit): keep false for a binary-only crate"}],"next":["commit the landed files, the receipt included"]}"##
        );
        let preview = Report {
            sentinels: None,
            repo: None,
            mode: "preview",
            nix: false,
            withheld: None,
            collisions: Some(vec![super::Collision {
                path: "SECURITY.md".into(),
                reason: "exists, and no receipt attributes it to release-kit".into(),
            }]),
            ..apply
        };
        assert_eq!(
            serde_json::to_string(&preview).expect("a report serializes"),
            r#"{"schema":"rk.init/7","mode":"preview","tech":"rust","forge":"github","target":"/tmp/t","workflow":"worktree","style":"trunk","nix":false,"collisions":[{"path":"SECURITY.md","reason":"exists, and no receipt attributes it to release-kit"}],"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"created"}],"next":["commit the landed files, the receipt included"]}"#,
            "a preview omits the sentinels, the unresolved repo, and an empty withheld list rather than serializing null"
        );
    }
}
