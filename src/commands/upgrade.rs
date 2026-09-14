//! `rk upgrade`: a landed target takes this binary's projection.
//!
//! A front over the direct writer: the receipt is required, the target's
//! evidence is gathered once, this binary's projection is computed from
//! its embedded sources, and every destination is decided by its recorded
//! kind alone. A recorded generated file is replaced from the candidate
//! whatever its bytes are, because the operator and the agent authorized
//! the migration and Git holds the recovery; a recorded seeded or state
//! file is preserved with its current digest entering the receipt; a
//! recorded marked region is replaced alone; a recorded destination the
//! projection no longer produces is released to the target. A whole-file
//! destination present on disk and absent from the receipt refuses
//! before any write, every collision collected in one pass. Preview by
//! default; `--apply` lands under one target lock with the receipt last.
//!
//! SATISFIES landing:ownership-is-elementary
//! SATISFIES landing:a-dropped-file-stays
//! SATISFIES landing:a-target-is-never-downgraded

use serde::Serialize;

use crate::cli::upgrade::UpgradeArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::embedded;
use crate::error::RkError;
use crate::landing::apply::{self, Action, Collision, Prepared};
use crate::landing::manifest::{self, Alignment, Manifest, Style, Workflow};
use crate::landing::{self, lock};
use crate::output::Output;
use crate::release::EmbeddedReleaseSource;

/// One destination and what the upgrade decided for it.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// The kind this projection declares for it, or the recorded kind of
    /// a released destination.
    kind: &'static str,
    /// `created`, `replaced`, `preserved`, `drift`, `released`, or
    /// `collision` in a preview.
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
    /// The version the receipt came from.
    from_version: String,
    /// This binary's version.
    to_version: &'static str,
    /// The working-copy mode the rewritten receipt carries.
    workflow: &'static str,
    style: &'static str,
    /// Whether the rewritten receipt carries the Nix capability.
    nix: bool,
    /// The Nix destinations this target could not take, each with why;
    /// absent where nothing was withheld.
    #[serde(skip_serializing_if = "Option::is_none")]
    withheld: Option<Vec<landing::Withheld>>,
    /// The destinations the landing refuses as they stand; absent where
    /// there is none. A preview lists them and exits 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    collisions: Option<Vec<Collision>>,
    config: crate::config::Plan,
    /// Every destination, with its action.
    files: Vec<FileEntry>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Upgrade the landed target to this binary's projection.
///
/// # Errors
///
/// Returns a refusal for a missing receipt, an unknown receipt schema, a
/// receipt from a newer binary, and, on apply, any collected collision;
/// and [`RkError::Io`] on filesystem failure.
pub fn run(args: &UpgradeArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let recorded = load_upgradable(&args.target)?;
    let existing = crate::config::load(args.target.as_std_path())?;
    let params = resolve_params(args, &recorded, existing.as_ref())?;
    let style = params
        .style()
        .ok_or_else(|| RkError::Usage("landing style is unresolved".into()))?;

    let (prepared, landed) = if args.apply {
        let lock = lock::acquire(&args.target)?;
        let prepared = apply::prepare(&args.target, Some(&recorded), &params, existing.as_ref())?;
        let landed = apply::land(
            &args.target,
            Some(&recorded),
            &prepared,
            apply::Origin::Upgrade,
            &lock,
        )?;
        (prepared, Some(landed))
    } else {
        (
            apply::prepare(&args.target, Some(&recorded), &params, existing.as_ref())?,
            None,
        )
    };

    for key in &prepared.config.changes {
        out.result_line(format!("configuration changes {key}"));
    }
    out.result_line(format!(
        "{} {}",
        prepared.config.action,
        crate::config::CONFIG_PATH
    ));
    let mut sentinels: Vec<String> = Vec::new();
    for decision in &prepared.decisions {
        if landed.is_some() && matches!(decision.action, Action::Created | Action::Replaced) {
            let bytes =
                landing::read_recorded(&args.target, &decision.destination)?.unwrap_or_default();
            collect_sentinels(&decision.destination, &bytes, &mut sentinels);
        }
        out.result_line(crate::commands::init::describe(decision));
    }
    for collision in &prepared.collisions {
        out.result_line(format!(
            "collision {}: {}",
            collision.path, collision.reason
        ));
    }
    for entry in &prepared.projection.omissions {
        out.result_line(format!("withheld {}: {}", entry.destination, entry.reason));
    }
    if landed.is_some() {
        out.result_line(format!("rewrote {}", manifest::MANIFEST_PATH));
        for sentinel in &sentinels {
            out.result_line(format!("fill this sentinel: {sentinel}"));
        }
    }

    let next = next_lines(args, prepared.collisions.is_empty());
    out.next(&next);
    out.emit(&Report {
        schema: "rk.upgrade/7",
        config: prepared.config.clone(),
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        tech: params.tech().into(),
        forge: params.forge().into(),
        from_version: recorded.rk_version,
        to_version: env!("CARGO_PKG_VERSION"),
        workflow: params.workflow().as_str(),
        style: style.as_str(),
        nix: params.nix(),
        withheld: withheld_of(&prepared),
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
        next,
    })
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
            "commit the upgraded files, the receipt included".to_owned(),
            format!("rk status --target {} reports the result", args.target),
        ]
    } else if clean {
        vec![
            format!(
                "rk upgrade{identity_flags}{workflow_flag}{style_flag}{nix_flag} --target {} --apply writes",
                args.target
            ),
            format!(
                "rk stage --target {} stages the complete candidate for a byte comparison",
                args.target
            ),
        ]
    } else {
        vec![
            format!(
                "resolve each collision above through the rk-setup skill; rk upgrade{identity_flags}{workflow_flag}{style_flag}{nix_flag} --target {} --apply refuses until then",
                args.target
            ),
            format!(
                "rk stage --target {} stages the complete candidate for a byte comparison",
                args.target
            ),
        ]
    }
}

/// The receipt an upgrade may act on: present, at a known schema, and not
/// from a newer binary than this one.
fn load_upgradable(target: &camino::Utf8Path) -> Result<Manifest, RkError> {
    let Some(recorded) = manifest::load(target)? else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "no {} at {target}: an upgrade needs the receipt of the landing it moves, and nothing was written",
                    manifest::MANIFEST_PATH
                ),
            )
            .expected("a recorded landing")
            .action(format!(
                "rk stage --target {target} stages this binary's candidate for a byte comparison; the rk-setup skill carries the best-effort migration, ending in rk adopt for a target brought to the candidate or rk init for a fresh one"
            ))
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

    /// The complete `rk.upgrade/7` shape, held by snapshot.
    #[test]
    fn the_upgrade_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.upgrade/7",
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
            collisions: None,
            files: vec![
                FileEntry {
                    path: "release-plz.toml".into(),
                    kind: "seeded",
                    action: "drift",
                },
                FileEntry {
                    path: "legacy.yml".into(),
                    kind: "rendered",
                    action: "released",
                },
            ],
            next: vec!["rk upgrade --target /tmp/t --apply writes".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.upgrade/7","mode":"preview","target":"/tmp/t","tech":"rust","forge":"github","from_version":"0.1.0","to_version":"0.2.0","workflow":"branches","style":"trunk","nix":false,"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"drift"},{"path":"legacy.yml","kind":"rendered","action":"released"}],"next":["rk upgrade --target /tmp/t --apply writes"]}"#
        );
    }
}
