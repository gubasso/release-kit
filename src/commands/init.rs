//! `rk init`: land a technology's deterministic files into a target.
//!
//! A front over the engine: the plan is computed with the setup intent,
//! and on `--apply` the engine executes exactly its operations through
//! one staged transaction with the record last. Dry-run by default:
//! without `--apply` the destinations are listed and nothing is touched.
//! The payload is rendered before anything is compared, so the comparison
//! is against what would be written, not against the raw payload. Apply
//! is all-or-nothing against conflicts on `rendered` files; a differing
//! `seeded` or `state` file is the target's own and is reported and kept.
//! A refused landing writes nothing, the record included.

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::init::InitArgs;
use crate::commands::reconcile::{self, FrontApplied, Trace};
use crate::diagnostic::{Diagnostic, Reason};
use crate::embedded;
use crate::error::RkError;
use crate::landing::manifest::{self, Style, Workflow};
use crate::landing::{self, Kind};
use crate::output::Output;
use crate::plan::gather::Flags;
use crate::plan::{Disposition, Intent, PlanRequest, Planned};
use crate::release::EmbeddedReleaseSource;

/// One destination and what happened to it.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// The declared ownership kind.
    kind: &'static str,
    /// `land` in a preview; `write`, `unchanged`, or `kept` in an apply.
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
    config: crate::config::Plan,
    /// Every destination, with its kind and action.
    files: Vec<FileEntry>,
    /// The sentinels an apply left to fill; absent in a preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    sentinels: Option<Vec<SentinelEntry>>,
    /// The plan the apply executed; absent in a preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<Trace>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Land the files for `--tech` into `--target`.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for an unknown technology or pair,
/// [`RkError::Refusal`] when the target is missing, already carries a
/// record, or a `rendered` destination conflicts, [`RkError::Missing`]
/// when an apply resolves no repository, and [`RkError::Io`] on
/// filesystem failure.
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
    let config = crate::config::load(args.target.as_std_path())?;
    let source = EmbeddedReleaseSource;
    let params = landing::Params::resolve(
        &source,
        &args.target,
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
    let mut effective = args.clone();
    effective.tech = Some(params.tech().into());
    effective.nix = params.nix();
    let style = params
        .style()
        .ok_or_else(|| RkError::Usage("landing style is unresolved".into()))?;
    // The resolved answers ride into the plan as flags, so the planner
    // resolves the same parameters and asks no decision the front already
    // answered by its defaults.
    let request = PlanRequest {
        target: args.target.clone(),
        intent: Intent::Setup,
        selector: "embedded".into(),
        fetch: false,
        observe_forge: false,
        flags: Flags {
            tech: Some(params.tech().to_owned()),
            forge: Some(params.forge().to_owned()),
            repo: args
                .repo
                .clone()
                .or_else(|| (params.repo() != "OWNER").then(|| params.repo().to_owned())),
            workflow: Some(params.workflow().as_str().to_owned()),
            style: Some(style.as_str().to_owned()),
            nix: Some(params.nix()),
        },
        decisions: std::collections::BTreeMap::new(),
    };
    let planned = reconcile::compute(&request, &manifest::now())?;
    let config_plan = planned
        .config
        .clone()
        .ok_or_else(|| RkError::Usage("landing parameters are unresolved".into()))?;
    if args.apply {
        apply(
            out,
            &effective,
            params.forge(),
            params.repo(),
            params.workflow(),
            style,
            &planned,
            &request,
            config_plan,
        )
    } else {
        let repo = (params.repo() != "OWNER").then(|| params.repo().to_owned());
        if repo.is_none() {
            out.frame(
                "note: no repository detected; an apply derives the owner from --repo <path>",
            );
        }
        preview(
            out,
            &effective,
            params.forge(),
            repo,
            params.workflow(),
            style,
            &planned,
            config_plan,
        )
    }
}

/// List every destination and write nothing.
#[allow(
    clippy::too_many_arguments,
    reason = "the landing parameters are one flat set the caller resolves once, and a struct around them would add a type nothing else reads"
)]
fn preview(
    out: Output,
    args: &InitArgs,
    forge: &str,
    repo: Option<String>,
    workflow: Workflow,
    style: Style,
    planned: &Planned,
    config: crate::config::Plan,
) -> Result<(), RkError> {
    let repo_argument = repo.as_deref().unwrap_or("<owner/name>");
    let nix_flag = if args.nix { " --nix" } else { "" };
    let next = vec![format!(
        "rk init --tech {} --forge {forge} --repo {repo_argument} --workflow {} --style {}{nix_flag} --target {} --apply",
        args.tech.as_deref().unwrap_or_default(),
        workflow.as_str(),
        style.as_str(),
        args.target
    )];
    out.result_line(format!(
        "DRY RUN: rk init writes these files into {}; re-run with --apply",
        args.target
    ));
    for outcome in &planned.outcomes {
        out.result_line(&outcome.path);
    }
    out.result_line(format!(
        "{} {}\n{}",
        config.action,
        crate::config::CONFIG_PATH,
        config.content
    ));
    for entry in &planned.withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/6",
        config,
        mode: "preview",
        tech: args.tech.clone().unwrap_or_default(),
        forge: forge.to_owned(),
        target: args.target.to_string(),
        repo,
        workflow: workflow.as_str(),
        style: style.as_str(),
        nix: args.nix,
        withheld: (!planned.withheld.is_empty()).then(|| planned.withheld.clone()),
        files: planned
            .outcomes
            .iter()
            .map(|outcome| FileEntry {
                path: outcome.path.clone(),
                kind: outcome.kind.as_str(),
                action: "land",
            })
            .collect(),
        sentinels: None,
        plan: None,
        next,
    })
}

/// Land the files through the engine — all-or-nothing against `rendered`
/// conflicts — with the record last, and report the judgment sentinels
/// the operator still owes.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the landing parameters are one flat set the caller resolves once, and the landing is all-or-nothing, so its ordered steps stay in one place"
)]
fn apply(
    out: Output,
    args: &InitArgs,
    forge: &str,
    repo: &str,
    workflow: Workflow,
    style: Style,
    planned: &Planned,
    request: &PlanRequest,
    config: crate::config::Plan,
) -> Result<(), RkError> {
    refuse_a_recorded_target(args)?;
    landing::hooks_splice_refusal(&EmbeddedReleaseSource, &args.target)?;
    refuse_conflicts(planned)?;
    let applied = reconcile::apply_in_process(planned, request, "init")?;
    let mut file_entries = Vec::new();
    let mut sentinels = Vec::new();
    for outcome in &planned.outcomes {
        let action = match outcome.disposition {
            Disposition::Write => "write",
            Disposition::Kept | Disposition::Drift | Disposition::State => "kept",
            Disposition::Unchanged | Disposition::Conflict | Disposition::Missing => "unchanged",
        };
        out.result_line(format!(
            "{} {}",
            match action {
                "write" => "wrote",
                "kept" => "kept (target-owned)",
                _ => "unchanged",
            },
            outcome.path
        ));
        // What the destination now holds: the written bytes, or the
        // target's own where a seeded or state file was kept.
        let landed: Vec<u8> = match FrontApplied::written(planned, &outcome.path) {
            Some(bytes) => bytes.to_vec(),
            None => landing::read_recorded(&args.target, &outcome.path)?.unwrap_or_default(),
        };
        collect_sentinels(&args.target, &outcome.path, &landed, &mut sentinels);
        file_entries.push(FileEntry {
            path: outcome.path.clone(),
            kind: outcome.kind.as_str(),
            action,
        });
    }
    for entry in &planned.withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }

    out.result_line(format!("{} {}", config.action, crate::config::CONFIG_PATH));
    for (key, empty_line) in [
        ("setup.required_check", "required_check = \"\""),
        ("setup.bot.app_id", "app_id = \"\""),
    ] {
        if let Some((index, _)) = config
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
    out.result_line(applied.line());

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
            "commit the landed files, the record included".to_owned()
        } else {
            "fill each sentinel above, then commit the landed files, the record included".to_owned()
        },
        format!("rk status --target {} reports this landing", args.target),
        "rk method setup orders what follows".to_owned(),
    ];
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/6",
        config,
        mode: "apply",
        tech: args.tech.clone().unwrap_or_default(),
        forge: forge.to_owned(),
        target: args.target.to_string(),
        repo: Some(repo.to_owned()),
        workflow: workflow.as_str(),
        style: style.as_str(),
        nix: args.nix,
        withheld: (!planned.withheld.is_empty()).then(|| planned.withheld.clone()),
        files: file_entries,
        sentinels: Some(sentinels),
        plan: Some(applied.trace()),
        next,
    })?;
    applied.applied.failure().map_or(Ok(()), Err)
}

/// A re-landing over an existing record is `rk upgrade`'s job, not a
/// second `rk init`.
fn refuse_a_recorded_target(args: &InitArgs) -> Result<(), RkError> {
    if landing::manifest::load(&args.target)?.is_none() {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "{} already carries {}, and nothing was written",
                args.target,
                manifest::MANIFEST_PATH
            ),
        )
        .expected("a target without a landing record")
        .action(format!(
            "rk upgrade --target {} takes it to this binary's payload",
            args.target
        ))
        .target_state("unchanged"),
    ))
}

/// Every `rendered` conflict the plan found, refused in one run before
/// anything writes.
fn refuse_conflicts(planned: &Planned) -> Result<(), RkError> {
    let conflicts: Vec<&str> = planned
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.kind == Kind::Rendered
                && matches!(
                    outcome.disposition,
                    Disposition::Conflict | Disposition::Missing
                )
        })
        .map(|outcome| outcome.path.as_str())
        .collect();
    if conflicts.is_empty() {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "these files exist with different content, and nothing was written: {}",
                conflicts.join(", ")
            ),
        )
        .expected("every rendered destination absent, or holding this landing's bytes")
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
    use crate::digest::Digest;

    /// The complete `rk.init/6` shape, held by snapshot in both modes: a
    /// field rename or removal fails here and becomes a schema-version
    /// bump instead of a silent parser break at some agent.
    #[test]
    fn the_init_report_schema_snapshot_holds() {
        let apply = Report {
            schema: "rk.init/6",
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
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                kind: "seeded",
                action: "write",
            }],
            sentinels: Some(vec![SentinelEntry {
                path: "/tmp/t/release-plz.toml".into(),
                line: 3,
                text: "# TODO(release-kit): keep false for a binary-only crate".into(),
            }]),
            plan: Some(crate::commands::reconcile::Trace {
                plan_id: "0123456789abcdef".into(),
                input_fingerprint: Digest::of(b"a"),
                stored: true,
                run_id: Some("run".into()),
            }),
            next: vec!["commit the landed files, the record included".into()],
        };
        assert_eq!(
            serde_json::to_string(&apply).expect("a report serializes"),
            format!(
                r##"{{"schema":"rk.init/6","mode":"apply","tech":"rust","forge":"github","target":"/tmp/t","repo":"acme/widget","workflow":"worktree","style":"trunk","nix":true,"withheld":[{{"path":"flake.nix","reason":"the target already carries flake.nix"}}],"config":{{"action":"added","changes":[],"content":"schema_version = 1\n"}},"files":[{{"path":"release-plz.toml","kind":"seeded","action":"write"}}],"sentinels":[{{"path":"/tmp/t/release-plz.toml","line":3,"text":"# TODO(release-kit): keep false for a binary-only crate"}}],"plan":{{"plan_id":"0123456789abcdef","input_fingerprint":"{}","stored":true,"run_id":"run"}},"next":["commit the landed files, the record included"]}}"##,
                Digest::of(b"a")
            )
        );
        let preview = Report {
            sentinels: None,
            repo: None,
            mode: "preview",
            nix: false,
            withheld: None,
            plan: None,
            ..apply
        };
        assert_eq!(
            serde_json::to_string(&preview).expect("a report serializes"),
            r#"{"schema":"rk.init/6","mode":"preview","tech":"rust","forge":"github","target":"/tmp/t","workflow":"worktree","style":"trunk","nix":false,"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"write"}],"next":["commit the landed files, the record included"]}"#,
            "a preview omits the sentinels, the unresolved repo, the plan, and an empty withheld list rather than serializing null"
        );
    }
}
