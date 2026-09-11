//! `rk upgrade`: a landed target takes a newer payload.
//!
//! Three digests decide each file: the baseline the record keeps — the
//! payload as it stood at landing — the bytes on disk now, and this
//! binary's candidate, rendered under the recorded parameters. A
//! `rendered` file nobody touched is rewritten; one the target edited is
//! a conflict, and every conflict is collected before the whole upgrade
//! refuses in one run. There is no merge: the two outcomes are a clean
//! write and a refusal, because a wrong guess in a release workflow is
//! discovered at the next release.

use serde::Serialize;

use crate::cli::upgrade::UpgradeArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::manifest::{self, Alignment, FileRecord, Manifest, Style, Workflow};
use crate::landing::{self, Entry, Kind};
use crate::output::Output;
use crate::{embedded, registry};

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
    /// What plausibly follows.
    next: Vec<String>,
}

/// One decided destination, carried from the decision pass to the write
/// pass and the record rewrite.
struct Decision<'a> {
    entry: Option<&'a Entry>,
    action: &'static str,
    record: FileRecord,
}

/// Upgrade the landed target to this binary's payload.
///
/// # Errors
///
/// Returns a refusal for a missing record, an unknown record schema, a
/// record from a newer binary, a `rendered` destination that is not a
/// regular file, and — on apply — any collected conflict; and
/// [`RkError::Io`] on filesystem failure.
pub fn run(args: &UpgradeArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let recorded = load_upgradable(&args.target)?;
    let existing = crate::config::load(args.target.as_std_path())?;
    let params = resolve_params(args, &recorded, existing.as_ref())?;
    let config = crate::config::Plan::new(
        args.target.as_std_path(),
        &params,
        existing.as_ref(),
        Some(&recorded),
    )?;
    for key in &config.changes {
        out.result_line(format!("configuration changes {key}"));
    }
    out.result_line(format!("{} {}", config.action, crate::config::CONFIG_PATH));
    let style = params
        .style()
        .ok_or_else(|| RkError::Usage("landing style is unresolved".into()))?;
    let mut entries = landing::projection(&params)?;
    let withheld =
        landing::withhold_nix(&args.target, params.nix(), Some(&recorded), &mut entries)?;
    refuse_non_regular(&args.target, &entries)?;

    let (decisions, conflicts) = decide_all(args, &recorded, &entries)?;
    // A file this payload stops shipping is a file the target owns from
    // that moment: left in place, named, and dropped from the record.
    let dropped: Vec<String> = recorded
        .files
        .iter()
        .filter(|file| {
            !entries
                .iter()
                .any(|entry| entry.destination == file.destination)
        })
        .map(|file| file.destination.clone())
        .collect();

    if args.apply && !conflicts.is_empty() {
        return Err(refuse_conflicts(&conflicts));
    }

    let mut sentinels: Vec<String> = Vec::new();
    for decision in &decisions {
        if args.apply && matches!(decision.action, "updated" | "added") {
            if let Some(entry) = decision.entry {
                landing::write_destination(&args.target, entry)?;
                collect_sentinels(entry, &mut sentinels);
            }
        }
        out.result_line(describe(decision));
    }
    for path in &dropped {
        out.result_line(format!(
            "dropped {path} (no longer shipped; now target-owned)"
        ));
    }
    for entry in &withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }

    if args.apply {
        config.apply(args.target.as_std_path())?;
        rewrite_record(&args.target, &recorded, &params, &decisions)?;
        out.result_line(format!("rewrote {}", manifest::MANIFEST_PATH));
        for sentinel in &sentinels {
            out.result_line(format!("fill this sentinel: {sentinel}"));
        }
    }

    let next = next_lines(args, conflicts.is_empty());
    out.next(&next);
    out.emit(&Report {
        schema: "rk.upgrade/5",
        config,
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        tech: params.tech().into(),
        forge: params.forge().into(),
        from_version: recorded.rk_version.clone(),
        to_version: env!("CARGO_PKG_VERSION"),
        workflow: params.workflow().as_str(),
        style: style.as_str(),
        nix: params.nix(),
        withheld: (!withheld.is_empty()).then_some(withheld),
        files: decisions
            .iter()
            .map(|decision| FileEntry {
                path: decision.record.destination.clone(),
                kind: decision.record.kind.as_str(),
                action: decision.action,
            })
            .chain(dropped.iter().map(|path| FileEntry {
                path: path.clone(),
                kind: "dropped",
                action: "dropped",
            }))
            .collect(),
        next,
    })
}

fn describe(decision: &Decision<'_>) -> String {
    match decision.action {
        "drift" => format!(
            "drift {} (seeded, target-owned)",
            decision.record.destination
        ),
        "kept" => format!("kept {} (target-owned)", decision.record.destination),
        "conflict" => format!(
            "conflict {} (edited, release-kit-owned)",
            decision.record.destination
        ),
        action => format!("{action} {}", decision.record.destination),
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

/// The record after a successful apply, rewritten whole: new version, new
/// digests, new pins and resolved parameters; the first landing's instant
/// and origin are preserved.
fn rewrite_record(
    target: &camino::Utf8Path,
    recorded: &Manifest,
    params: &landing::Params,
    decisions: &[Decision],
) -> Result<(), RkError> {
    manifest::write(
        target,
        &Manifest {
            schema_version: manifest::SCHEMA_VERSION,
            rk_version: env!("CARGO_PKG_VERSION").to_owned(),
            payload_sha256: crate::commands::payload::report().payload_sha256,
            origin: recorded.origin.clone(),
            tech: params.tech().into(),
            forge: params.forge().into(),
            landed_at: recorded.landed_at.clone(),
            parameters: manifest::Parameters {
                repo: params.repo().into(),
                workflow: params.workflow(),
                style: params.style(),
                nix: params.nix(),
                trunk: params.trunk().to_owned(),
                line_prefix: params.line_prefix().to_owned(),
            },
            files: decisions
                .iter()
                .map(|decision| clone_record(&decision.record))
                .collect(),
            pins: registry::pins_for(params.tech())
                .into_iter()
                .map(|pin| (pin.name, pin.version))
                .collect(),
        },
    )
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

/// Decide one candidate destination from the three digests.
/// Every entry decided in one pass, with the collected conflicts. An
/// ill-formed hook file is a conflict in preview and apply alike: its
/// first block may match while a duplicate still executes, so the
/// per-entry comparison cannot see it, and the refusal names each
/// conflict once.
fn decide_all<'a>(
    args: &UpgradeArgs,
    recorded: &'a Manifest,
    entries: &'a [Entry],
) -> Result<(Vec<Decision<'a>>, Vec<String>), RkError> {
    let mut conflicts: Vec<String> = Vec::new();
    let mut decisions: Vec<Decision<'a>> = Vec::new();
    if landing::hooks_file_defect(&args.target)?.is_some() {
        conflicts.push(landing::HOOKS_DESTINATION.to_owned());
    }
    for entry in entries {
        let disk = landing::read_recorded(&args.target, &entry.destination)?;
        let mut decision = decide(
            entry,
            recorded.file(&entry.destination),
            disk.as_deref(),
            &mut conflicts,
        );
        if entry.destination == landing::HOOKS_DESTINATION
            && conflicts.iter().any(|c| c == landing::HOOKS_DESTINATION)
        {
            decision.action = "conflict";
        }
        decisions.push(decision);
    }
    let mut seen = std::collections::HashSet::new();
    conflicts.retain(|conflict| seen.insert(conflict.clone()));
    Ok((decisions, conflicts))
}

fn decide<'a>(
    entry: &'a Entry,
    recorded: Option<&FileRecord>,
    disk: Option<&[u8]>,
    conflicts: &mut Vec<String>,
) -> Decision<'a> {
    let candidate_record = |sha256: Digest| FileRecord {
        destination: entry.destination.clone(),
        kind: entry.kind,
        sha256,
        baseline_sha256: match entry.kind {
            Kind::State => None,
            Kind::Rendered | Kind::Seeded => Some(Digest::of(&entry.baseline)),
        },
    };
    let Some(recorded) = recorded else {
        return decide_added(entry, disk, conflicts);
    };

    // A seeded file this payload reclassifies as rendered claims
    // ownership of a file the target may have tuned; only untouched bytes
    // — matching the recorded baseline — permit the claim.
    if recorded.kind == Kind::Seeded && entry.kind == Kind::Rendered {
        let untouched =
            disk.is_some_and(|bytes| Some(Digest::of(bytes)) == recorded.baseline_sha256);
        if !untouched {
            conflicts.push(entry.destination.clone());
            return Decision {
                entry: Some(entry),
                action: "conflict",
                record: candidate_record(Digest::of(&entry.rendered)),
            };
        }
        return Decision {
            entry: Some(entry),
            action: "updated",
            record: candidate_record(Digest::of(&entry.rendered)),
        };
    }

    match entry.kind {
        Kind::Rendered => match disk {
            Some(bytes) if Digest::of(bytes) == recorded.sha256 => Decision {
                entry: Some(entry),
                action: if bytes == entry.rendered {
                    "unchanged"
                } else {
                    "updated"
                },
                record: candidate_record(Digest::of(&entry.rendered)),
            },
            Some(bytes) if bytes == entry.rendered => Decision {
                entry: Some(entry),
                action: "unchanged",
                record: candidate_record(Digest::of(&entry.rendered)),
            },
            // Edited or deleted: either way the target changed a file
            // release-kit owns.
            _ => {
                conflicts.push(entry.destination.clone());
                Decision {
                    entry: Some(entry),
                    action: "conflict",
                    record: candidate_record(Digest::of(&entry.rendered)),
                }
            }
        },
        Kind::Seeded => {
            // Never written; the record keeps the target's current bytes
            // and the baseline it tunes away from. For a file this payload
            // reclassifies from rendered to seeded — safe and silent — that
            // baseline is the rendered bytes release-kit last wrote, not
            // the pre-substitution payload, so an untouched file is not
            // reported as drift.
            let baseline = if recorded.kind == Kind::Rendered {
                Some(recorded.sha256.clone())
            } else {
                recorded.baseline_sha256.clone()
            };
            let (action, sha256) = disk.map_or_else(
                || ("drift", recorded.sha256.clone()),
                |bytes| {
                    let digest = Digest::of(bytes);
                    if Some(&digest) == baseline.as_ref() {
                        ("unchanged", digest)
                    } else {
                        ("drift", digest)
                    }
                },
            );
            Decision {
                entry: None,
                action,
                record: FileRecord {
                    destination: entry.destination.clone(),
                    kind: entry.kind,
                    sha256,
                    baseline_sha256: baseline,
                },
            }
        }
        Kind::State => Decision {
            entry: None,
            action: "state",
            record: FileRecord {
                destination: entry.destination.clone(),
                kind: entry.kind,
                sha256: recorded.sha256.clone(),
                baseline_sha256: None,
            },
        },
    }
}

/// A destination the record does not name, added by this payload: it
/// lands exactly as `rk init` lands it — a differing `rendered`
/// destination is a conflict, a differing `seeded` or `state` one is the
/// target's and is kept.
fn decide_added<'a>(
    entry: &'a Entry,
    disk: Option<&[u8]>,
    conflicts: &mut Vec<String>,
) -> Decision<'a> {
    let (action, sha256) = match disk {
        None => ("added", Digest::of(&entry.rendered)),
        Some(bytes) if bytes == entry.rendered => ("unchanged", Digest::of(bytes)),
        Some(bytes) if entry.kind != Kind::Rendered => ("kept", Digest::of(bytes)),
        Some(_) => {
            conflicts.push(entry.destination.clone());
            ("conflict", Digest::of(&entry.rendered))
        }
    };
    Decision {
        entry: Some(entry),
        action,
        record: FileRecord {
            destination: entry.destination.clone(),
            kind: entry.kind,
            sha256,
            baseline_sha256: match entry.kind {
                Kind::State => None,
                Kind::Rendered | Kind::Seeded => Some(Digest::of(&entry.baseline)),
            },
        },
    }
}

/// A `rendered` destination that exists and is not a regular file refuses
/// before anything is read.
fn refuse_non_regular(target: &camino::Utf8Path, entries: &[Entry]) -> Result<(), RkError> {
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
fn collect_sentinels(entry: &Entry, found: &mut Vec<String>) {
    let text = String::from_utf8_lossy(&entry.rendered);
    for (idx, line) in text.lines().enumerate() {
        if line.contains(embedded::SENTINEL) {
            found.push(format!(
                "{}:{}: {}",
                entry.destination,
                idx + 1,
                line.trim()
            ));
        }
    }
}

/// [`FileRecord`] carries digests, which are cheap to clone by field.
fn clone_record(record: &FileRecord) -> FileRecord {
    FileRecord {
        destination: record.destination.clone(),
        kind: record.kind,
        sha256: record.sha256.clone(),
        baseline_sha256: record.baseline_sha256.clone(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{FileEntry, Report};

    /// The complete `rk.upgrade/5` shape, held by snapshot.
    #[test]
    fn the_upgrade_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.upgrade/5",
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
            next: vec!["rk upgrade --target /tmp/t --apply writes".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.upgrade/5","mode":"preview","target":"/tmp/t","tech":"rust","forge":"github","from_version":"0.1.0","to_version":"0.2.0","workflow":"branches","style":"trunk","nix":false,"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"drift"}],"next":["rk upgrade --target /tmp/t --apply writes"]}"#
        );
    }
}
