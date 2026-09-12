//! `rk upgrade`: a landed target takes a newer payload.
//!
//! A front over the engine: the plan is computed with the upgrade
//! intent, and on `--apply` the engine executes exactly its operations
//! through one staged transaction with the record last. Three digests
//! decide each file: the baseline the record keeps — the payload as it
//! stood at landing — the bytes on disk now, and this binary's candidate,
//! rendered under the recorded parameters. A `rendered` file nobody
//! touched is rewritten; one the target edited is a conflict, and every
//! conflict is collected before the whole upgrade refuses in one run.
//! There is no merge: the two outcomes are a clean write and a refusal,
//! because a wrong guess in a release workflow is discovered at the next
//! release.

use serde::Serialize;

use crate::cli::upgrade::UpgradeArgs;
use crate::commands::reconcile::{self, FrontApplied, Trace};
use crate::diagnostic::{Diagnostic, Reason};
use crate::embedded;
use crate::error::RkError;
use crate::landing::manifest::{self, Alignment, Manifest, Style, Workflow};
use crate::landing::{self, Kind};
use crate::output::Output;
use crate::plan::gather::Flags;
use crate::plan::{Disposition, Intent, PlanRequest, Planned};
use crate::release::EmbeddedReleaseSource;

/// One destination and what the upgrade decided for it.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// The kind this payload declares for it.
    kind: &'static str,
    /// `updated`, `unchanged`, `added`, `drift`, `kept`, `dropped`,
    /// `state`, or `conflict`.
    action: &'static str,
}

/// The machine form of an upgrade report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target directory.
    target: String,
    /// The recorded technology.
    tech: String,
    /// The recorded forge.
    forge: String,
    /// The version the record came from.
    from_version: String,
    /// This binary's version.
    to_version: &'static str,
    /// The working-copy mode the rewritten record carries — the recorded
    /// mode, or the `--workflow` override this run applies.
    workflow: &'static str,
    style: &'static str,
    /// Whether the rewritten record carries the Nix capability.
    nix: bool,
    /// The Nix destinations this target could not take, each with why;
    /// absent where nothing was withheld.
    #[serde(skip_serializing_if = "Option::is_none")]
    withheld: Option<Vec<landing::Withheld>>,
    config: crate::config::Plan,
    /// Every destination, with its action.
    files: Vec<FileEntry>,
    /// The plan the apply executed; absent in a preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<Trace>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// One decided destination, read off the plan's outcomes.
struct Decision {
    path: String,
    kind: Kind,
    action: &'static str,
}

/// Upgrade the landed target to this binary's payload.
///
/// # Errors
///
/// Returns a refusal for a missing record, an unknown record schema, a
/// record from a newer binary, a `rendered` destination that is not a
/// regular file, and — on apply — any collected conflict; and
/// [`RkError::Io`] on filesystem failure.
#[allow(
    clippy::too_many_lines,
    reason = "one upgrade run is one linear sequence from the record to the report, and cutting it would separate a refusal from the order it is reported in"
)]
pub fn run(args: &UpgradeArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let recorded = load_upgradable(&args.target)?;
    let existing = crate::config::load(args.target.as_std_path())?;
    let params = resolve_params(args, &recorded, existing.as_ref())?;
    let style = params
        .style()
        .ok_or_else(|| RkError::Usage("landing style is unresolved".into()))?;
    let request = PlanRequest {
        target: args.target.clone(),
        intent: Intent::Upgrade,
        selector: "embedded".into(),
        fetch: false,
        observe_forge: false,
        flags: Flags {
            tech: Some(params.tech().to_owned()),
            forge: Some(params.forge().to_owned()),
            repo: Some(params.repo().to_owned()),
            workflow: Some(params.workflow().as_str().to_owned()),
            style: Some(style.as_str().to_owned()),
            nix: Some(params.nix()),
        },
        decisions: std::collections::BTreeMap::new(),
    };
    refuse_non_regular(
        &args.target,
        &landing::projection(&EmbeddedReleaseSource, &params)?,
    )?;
    let planned = reconcile::compute(&request, &manifest::now())?;
    let config = planned
        .config
        .clone()
        .ok_or_else(|| RkError::Usage("landing parameters are unresolved".into()))?;
    for key in &config.changes {
        out.result_line(format!("configuration changes {key}"));
    }
    out.result_line(format!("{} {}", config.action, crate::config::CONFIG_PATH));

    let hooks_defect = planned
        .plan
        .preconditions
        .iter()
        .any(|p| p.id == "hooks-file-spliceable" && !p.evaluation.holds());
    let (decisions, conflicts) = decide_all(&planned, hooks_defect);
    // A file this payload stops shipping is a file the target owns from
    // that moment: left in place, named, and dropped from the record.
    let dropped: Vec<String> = planned
        .plan
        .observed_state
        .installation
        .destinations
        .iter()
        .filter(|destination| destination.recorded_kind.is_some())
        .filter(|destination| {
            !planned
                .outcomes
                .iter()
                .any(|outcome| outcome.path == destination.path)
        })
        .map(|destination| destination.path.clone())
        .collect();

    if args.apply && !conflicts.is_empty() {
        return Err(refuse_conflicts(&conflicts));
    }

    let applied = if args.apply {
        Some(reconcile::apply_in_process(&planned, &request, "upgrade")?)
    } else {
        None
    };
    let mut sentinels: Vec<String> = Vec::new();
    for decision in &decisions {
        if args.apply && matches!(decision.action, "updated" | "added") {
            if let Some(bytes) = FrontApplied::written(&planned, &decision.path) {
                collect_sentinels(&decision.path, bytes, &mut sentinels);
            }
        }
        out.result_line(describe(decision));
    }
    for path in &dropped {
        out.result_line(format!(
            "dropped {path} (no longer shipped; now target-owned)"
        ));
    }
    for entry in &planned.withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }

    if let Some(applied) = &applied {
        out.result_line(format!("rewrote {}", manifest::MANIFEST_PATH));
        out.result_line(applied.line());
        for sentinel in &sentinels {
            out.result_line(format!("fill this sentinel: {sentinel}"));
        }
    }

    let next = next_lines(args, conflicts.is_empty());
    out.next(&next);
    out.emit(&Report {
        schema: "rk.upgrade/6",
        config,
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        tech: params.tech().into(),
        forge: params.forge().into(),
        from_version: recorded.rk_version,
        to_version: env!("CARGO_PKG_VERSION"),
        workflow: params.workflow().as_str(),
        style: style.as_str(),
        nix: params.nix(),
        withheld: (!planned.withheld.is_empty()).then(|| planned.withheld.clone()),
        files: decisions
            .iter()
            .map(|decision| FileEntry {
                path: decision.path.clone(),
                kind: decision.kind.as_str(),
                action: decision.action,
            })
            .chain(dropped.iter().map(|path| FileEntry {
                path: path.clone(),
                kind: "dropped",
                action: "dropped",
            }))
            .collect(),
        plan: applied.as_ref().map(FrontApplied::trace),
        next,
    })?;
    applied
        .and_then(|applied| applied.applied.failure())
        .map_or(Ok(()), Err)
}

fn describe(decision: &Decision) -> String {
    match decision.action {
        "drift" => format!("drift {} (seeded, target-owned)", decision.path),
        "kept" => format!("kept {} (target-owned)", decision.path),
        "conflict" => format!("conflict {} (edited, release-kit-owned)", decision.path),
        action => format!("{action} {}", decision.path),
    }
}

fn resolve_params(
    args: &UpgradeArgs,
    recorded: &Manifest,
    existing: Option<&crate::config::Config>,
) -> Result<landing::Params, RkError> {
    let nix = match args.nix.as_deref() {
        None => None,
        Some("on") => Some(true),
        Some("off") => Some(false),
        Some(other) => {
            return Err(RkError::Usage(format!(
                "unknown --nix value '{other}'; the values are: on, off"
            )));
        }
    };
    landing::Params::resolve(
        &EmbeddedReleaseSource,
        &args.target,
        &landing::Inputs {
            tech: args.tech.as_deref(),
            forge: args.forge.as_deref(),
            repo: args.repo.as_deref(),
            workflow: args.workflow.as_deref().map(Workflow::parse).transpose()?,
            style: args.style.as_deref().map(Style::parse).transpose()?,
            nix,
        },
        existing,
        Some(recorded),
        landing::Purpose::Upgrade,
    )
}

/// The collect-then-refuse conflict answer: the whole list in one run, so
/// an operator resolves everything and re-runs once.
fn refuse_conflicts(conflicts: &[String]) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "these files release-kit owns were edited, and nothing was written: {}",
                conflicts.join(", ")
            ),
        )
        .expected("every rendered file as the record left it")
        .action("resolve each, or re-land it, then run 'rk upgrade' again")
        .target_state("unchanged"),
    )
}

/// The `Next:` lines for each outcome. A behavior-defining flag the
/// preview was run with rides into the follow-up command, so following
/// it applies the decision that was previewed, never a different one.
fn next_lines(args: &UpgradeArgs, clean: bool) -> Vec<String> {
    let identity_flags: String = [
        ("tech", args.tech.as_deref()),
        ("forge", args.forge.as_deref()),
        ("repo", args.repo.as_deref()),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!(" --{key} {value}")))
    .collect();
    let workflow_flag = args
        .workflow
        .as_deref()
        .map_or_else(String::new, |mode| format!(" --workflow {mode}"));
    let style_flag = args
        .style
        .as_deref()
        .map_or_else(String::new, |style| format!(" --style {style}"));
    let nix_flag = args
        .nix
        .as_deref()
        .map_or_else(String::new, |value| format!(" --nix {value}"));
    if args.apply {
        vec![
            "commit the upgraded files, the record included".to_owned(),
            format!("rk status --target {} reports the result", args.target),
        ]
    } else if clean {
        vec![format!(
            "rk upgrade{identity_flags}{workflow_flag}{style_flag}{nix_flag} --target {} --apply writes",
            args.target
        )]
    } else {
        vec![format!(
            "resolve each conflict above; rk upgrade{identity_flags}{workflow_flag}{style_flag}{nix_flag} --target {} --apply refuses until then",
            args.target
        )]
    }
}

/// The record an upgrade may act on: present, at a known schema, and not
/// from a newer binary than this one.
fn load_upgradable(target: &camino::Utf8Path) -> Result<Manifest, RkError> {
    let Some(recorded) = manifest::load(target)? else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "no {} at {target}: there is no baseline to upgrade against",
                    manifest::MANIFEST_PATH
                ),
            )
            .expected("a recorded landing")
            .action(
                "rk init lands a first landing; rk adopt records one made before the record existed",
            )
            .target_state("unchanged"),
        ));
    };
    if manifest::alignment(&recorded.rk_version, env!("CARGO_PKG_VERSION"))
        == Alignment::TargetNewer
    {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "this landing came from rk {}, newer than this binary's {}; downgrading a target is not an upgrade",
                    recorded.rk_version,
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .expected("a binary at or above the recorded rk_version")
            .action(format!("install release-kit {} or newer", recorded.rk_version))
            .target_state("unchanged"),
        ));
    }
    Ok(recorded)
}

/// Every destination decided off the plan's outcomes, with the collected
/// conflicts. An ill-formed hook file is a conflict in preview and apply
/// alike: its first block may match while a duplicate still executes,
/// so the per-entry comparison cannot see it, and the refusal names each
/// conflict once.
fn decide_all(planned: &Planned, hooks_defect: bool) -> (Vec<Decision>, Vec<String>) {
    let mut conflicts: Vec<String> = Vec::new();
    if hooks_defect {
        conflicts.push(landing::HOOKS_DESTINATION.to_owned());
    }
    let mut decisions = Vec::new();
    for outcome in &planned.outcomes {
        let decided = match outcome.disposition {
            Disposition::Write if outcome.recorded => "updated",
            Disposition::Write => "added",
            Disposition::Unchanged => "unchanged",
            Disposition::Kept => "kept",
            Disposition::Drift => "drift",
            Disposition::State => "state",
            Disposition::Conflict | Disposition::Missing => {
                conflicts.push(outcome.path.clone());
                "conflict"
            }
        };
        let action = if outcome.path == landing::HOOKS_DESTINATION && hooks_defect {
            "conflict"
        } else {
            decided
        };
        decisions.push(Decision {
            path: outcome.path.clone(),
            kind: outcome.kind,
            action,
        });
    }
    let mut seen = std::collections::HashSet::new();
    conflicts.retain(|conflict| seen.insert(conflict.clone()));
    (decisions, conflicts)
}

/// A `rendered` destination that exists and is not a regular file refuses
/// before anything is read.
fn refuse_non_regular(
    target: &camino::Utf8Path,
    entries: &[landing::Entry],
) -> Result<(), RkError> {
    for entry in entries {
        if entry.kind != Kind::Rendered {
            continue;
        }
        let path = target.join(&entry.destination);
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if !meta.is_file() {
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::StateDrift,
                        format!("{path} exists and is not a regular file; nothing was written"),
                    )
                    .expected("every rendered destination a regular file")
                    .target_state("unchanged"),
                ));
            }
        }
    }
    Ok(())
}

/// The judgment sentinels a newly written file carries.
fn collect_sentinels(destination: &str, bytes: &[u8], found: &mut Vec<String>) {
    let text = String::from_utf8_lossy(bytes);
    for (idx, line) in text.lines().enumerate() {
        if line.contains(embedded::SENTINEL) {
            found.push(format!("{destination}:{}: {}", idx + 1, line.trim()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FileEntry, Report};

    /// The complete `rk.upgrade/6` shape, held by snapshot.
    #[test]
    fn the_upgrade_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.upgrade/6",
            config: crate::config::Plan {
                action: "added",
                changes: vec![],
                content: "schema_version = 1\n".into(),
            },
            mode: "preview",
            target: "/tmp/t".into(),
            tech: "rust".into(),
            forge: "github".into(),
            from_version: "0.1.0".into(),
            to_version: "0.2.0",
            workflow: "branches",
            style: "trunk",
            nix: false,
            withheld: None,
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                kind: "seeded",
                action: "drift",
            }],
            plan: None,
            next: vec!["rk upgrade --target /tmp/t --apply writes".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.upgrade/6","mode":"preview","target":"/tmp/t","tech":"rust","forge":"github","from_version":"0.1.0","to_version":"0.2.0","workflow":"branches","style":"trunk","nix":false,"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"drift"}],"next":["rk upgrade --target /tmp/t --apply writes"]}"#
        );
    }
}
