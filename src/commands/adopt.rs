//! `rk adopt`: a pre-record target becomes a recorded one.
//!
//! A front over the engine: the plan is computed with the adopt intent,
//! which verifies every destination and writes none, and on `--apply`
//! the engine writes the configuration and the record, last, through
//! one staged transaction. The candidate payload is rendered first,
//! exactly as `rk init` would produce it; every `rendered` destination
//! must match it byte for byte, and one mismatch refuses the whole
//! adoption listing every mismatch in one run. Blessing whatever is on
//! disk would launder arbitrary drift into release-kit ownership, so
//! nothing here ever takes the disk as the baseline — and no target file
//! is ever changed: not a byte, not a mode, not a sentinel.

use serde::Serialize;

use crate::cli::adopt::AdoptArgs;
use crate::commands::reconcile::{self, FrontApplied, Trace};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, Style, Workflow};
use crate::landing::{self, Kind};
use crate::output::Output;
use crate::plan::gather::Flags;
use crate::plan::{Disposition, Intent, PlanRequest, Planned};
use crate::release::EmbeddedReleaseSource;

/// One verified destination.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// The declared ownership kind.
    kind: &'static str,
    /// `matches`, `differs` for a seeded file, or `state`.
    action: &'static str,
}

/// The machine form of an adoption report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target directory.
    target: String,
    /// The technology whose payload was verified.
    tech: String,
    /// The forge whose payload was verified.
    forge: String,
    /// The parameter the candidate was rendered under.
    repo: String,
    /// The working-copy mode the candidate was rendered under and the
    /// record carries.
    workflow: &'static str,
    style: &'static str,
    /// Whether the record carries the Nix capability.
    nix: bool,
    /// The Nix destinations excluded from the candidate, each with why;
    /// absent where nothing was withheld.
    #[serde(skip_serializing_if = "Option::is_none")]
    withheld: Option<Vec<landing::Withheld>>,
    config: crate::config::Plan,
    /// Every destination, with its verification result.
    files: Vec<FileEntry>,
    /// The plan the apply executed; absent in a preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<Trace>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Verify the target against the rendered candidate and, on `--apply`,
/// write the config and record inside `.release-kit/`.
///
/// # Errors
///
/// Returns a refusal for a target already carrying a record, for any
/// `rendered` mismatch or missing expected file — listing every one in
/// one run — and [`RkError::Missing`] where detection resolves no
/// technology, forge, or repository and no flag covers the gap.
#[allow(
    clippy::too_many_lines,
    reason = "one adopt run is one linear sequence of checks against one target, and cutting it would separate a refusal from the order it is reported in"
)]
pub fn run(args: &AdoptArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    if !args.target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {} is not a directory", args.target),
            )
            .expected("an existing repository to adopt"),
        ));
    }
    if landing::manifest::load(&args.target)?.is_some() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "{} already carries {}; it needs no adoption",
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
        landing::Purpose::Adopt,
    )?;
    let tech = params.tech().to_owned();
    let repo = params.repo().to_owned();
    let workflow = params.workflow();
    let style = params
        .style()
        .ok_or_else(|| RkError::Usage("landing style is unresolved".into()))?;
    let request = PlanRequest {
        target: args.target.clone(),
        intent: Intent::Adopt,
        selector: "embedded".into(),
        fetch: false,
        observe_forge: false,
        flags: Flags {
            tech: Some(tech.clone()),
            forge: Some(params.forge().to_owned()),
            repo: Some(repo.clone()),
            workflow: Some(workflow.as_str().to_owned()),
            style: Some(style.as_str().to_owned()),
            nix: Some(params.nix()),
        },
        decisions: std::collections::BTreeMap::new(),
    };
    let planned = reconcile::compute(&request, &manifest::now())?;
    let config = planned
        .config
        .clone()
        .ok_or_else(|| RkError::Usage("landing parameters are unresolved".into()))?;
    let files = verify(args, workflow, &planned)?;

    for file in &files {
        out.result_line(match file.action {
            "differs" => format!("differs {} (seeded, target-owned)", file.path),
            action => format!("{action} {}", file.path),
        });
    }
    for entry in &planned.withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }

    let applied = if args.apply {
        let applied = reconcile::apply_in_process(&planned, &request, "adopt")?;
        out.result_line(format!("wrote {}", manifest::MANIFEST_PATH));
        out.result_line(applied.line());
        Some(applied)
    } else {
        None
    };

    let next = if args.apply {
        vec![
            "commit the config and the record".to_owned(),
            format!("rk status --target {} reports this landing", args.target),
        ]
    } else {
        vec![format!(
            "rk adopt --tech {tech} --forge {} --repo {repo} --workflow {} --style {}{} --target {} --apply writes the config and the record inside .release-kit/",
            params.forge().to_owned(),
            workflow.as_str(),
            style.as_str(),
            if params.nix() { " --nix" } else { "" },
            args.target
        )]
    };
    out.result_line(format!(
        "{} {}\n{}",
        config.action,
        crate::config::CONFIG_PATH,
        config.content
    ));
    out.next(&next);
    out.emit(&Report {
        schema: "rk.adopt/6",
        config,
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        tech,
        forge: params.forge().to_owned(),
        repo,
        workflow: workflow.as_str(),
        style: style.as_str(),
        nix: params.nix(),
        withheld: (!planned.withheld.is_empty()).then(|| planned.withheld.clone()),
        files,
        plan: applied.as_ref().map(FrontApplied::trace),
        next,
    })?;
    applied
        .and_then(|applied| applied.applied.failure())
        .map_or(Ok(()), Err)
}

/// The verification pass, read off the plan: every destination checked
/// against the rendered candidate, every failure collected before the one
/// refusal, so an operator resolves everything and re-runs once.
fn verify(
    args: &AdoptArgs,
    workflow: Workflow,
    planned: &Planned,
) -> Result<Vec<FileEntry>, RkError> {
    let mut mismatches: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut files = Vec::new();
    // An ill-formed hook file lists beside the mismatches rather than
    // refusing alone, so one run still names everything unadoptable.
    let defects: Vec<String> = planned
        .plan
        .preconditions
        .iter()
        .filter(|p| p.id == "hooks-file-spliceable")
        .filter_map(|p| match &p.evaluation {
            crate::plan::Evaluation::Unsatisfied { reason } => Some(reason.clone()),
            _ => None,
        })
        .collect();
    for outcome in &planned.outcomes {
        if outcome.disposition == Disposition::Write {
            // A block-placed artifact reads as absent from a file that
            // exists; the operator's remedy differs, so the label must.
            let label = if args.target.join(&outcome.path).exists() {
                format!("{} (carries no release-kit block)", outcome.path)
            } else {
                format!("{} (expected and missing)", outcome.path)
            };
            missing.push(label);
            continue;
        }
        let action = match (outcome.kind, outcome.disposition) {
            (Kind::Rendered | Kind::Seeded, Disposition::Unchanged) => "matches",
            (Kind::Rendered, _) => {
                mismatches.push(outcome.path.clone());
                "differs"
            }
            (Kind::Seeded, _) => "differs",
            (Kind::State, _) => "state",
        };
        files.push(FileEntry {
            path: outcome.path.clone(),
            kind: outcome.kind.as_str(),
            action,
        });
    }
    if mismatches.is_empty() && missing.is_empty() && defects.is_empty() {
        return Ok(files);
    }
    let listed: Vec<String> = mismatches
        .iter()
        .map(|path| format!("{path} (differs from the rendered candidate)"))
        .chain(missing.iter().cloned())
        .chain(defects.iter().cloned())
        .collect();
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "this target is not adoptable as-is, and no record was written: {}",
                listed.join(", ")
            ),
        )
        .expected(format!(
            "every rendered destination matching the {} candidate, byte for byte",
            workflow.as_str()
        ))
        .action(format!(
            "align first: rk adopt without --apply lists every differing destination; bring each to the selected candidate's bytes — rk snippet and rk payload print them — then re-run, or select the other candidate with --workflow or --style{}",
            // A policy the target wrote its own contact into is the one
            // mismatch a committed answer resolves rather than an edit:
            // naming the keys turns a dead end into the next step.
            if mismatches.iter().any(|path| path == "SECURITY.md") {
                ". SECURITY.md states two facts a target owns: set security.contact and security.response in .release-kit/config.toml to the wording this policy already carries, and the candidate matches"
            } else {
                ""
            }
        ))
        .target_state("unchanged"),
    ))
}

#[cfg(test)]
mod tests {
    use super::{FileEntry, Report};

    /// The complete `rk.adopt/6` shape, held by snapshot.
    #[test]
    fn the_adopt_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.adopt/6",
            config: crate::config::Plan {
                action: "added",
                changes: vec![],
                content: "schema_version = 1\n".into(),
            },
            mode: "apply",
            target: "/tmp/t".into(),
            tech: "rust".into(),
            forge: "github".into(),
            repo: "acme/widget".into(),
            workflow: "branches",
            style: "trunk",
            nix: false,
            withheld: None,
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                kind: "seeded",
                action: "differs",
            }],
            plan: None,
            next: vec!["commit the config and the record".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.adopt/6","mode":"apply","target":"/tmp/t","tech":"rust","forge":"github","repo":"acme/widget","workflow":"branches","style":"trunk","nix":false,"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"differs"}],"next":["commit the config and the record"]}"#
        );
    }
}
