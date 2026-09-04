//! `rk init`: land a technology's deterministic files into a target.
//!
//! Dry-run by default: without `--apply` the destinations are listed and
//! nothing is touched. The payload is rendered before anything is
//! compared — the repository owner substitutes into `rendered` files from
//! the detection-resolved `--repo` parameter — so the comparison is
//! against what would be written, not against the raw payload. Apply is
//! all-or-nothing against conflicts on `rendered` files; a differing
//! `seeded` or `state` file is the target's own and is reported and kept.
//! Every write goes through the temp-plus-rename writer, and the landing
//! record is written last: a refused landing writes nothing, the record
//! included.

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::init::InitArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, FileRecord, Manifest, Parameters, Style, Workflow};
use crate::landing::{self, Entry, Kind};
use crate::output::Output;
use crate::{digest::Digest, embedded, registry};

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
    /// Every destination, with its kind and action.
    files: Vec<FileEntry>,
    /// The sentinels an apply left to fill; absent in a preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    sentinels: Option<Vec<SentinelEntry>>,
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
    let resolved = landing::resolve(&args.target, args.forge.as_deref(), args.repo.as_deref())?;
    let forge = resolved.forge;
    let workflow = Workflow::parse(&args.workflow)?;
    let style = Style::parse(&args.style)?;
    if args.apply {
        let repo = resolved.repo.ok_or_else(landing::repo_unresolved)?;
        let scopes = landing::parse_scopes(args.scopes.as_deref().ok_or_else(|| {
            RkError::Usage(
                "an apply renders the scope-bearing files; pass --scopes <list>, the Conventional Commit scopes this project accepts".into(),
            )
        })?)?;
        let mut entries = landing::projection(
            &args.tech,
            &forge,
            &repo,
            &scopes,
            workflow,
            Some(style),
            args.nix,
        )?;
        let withheld = landing::withhold_nix(&args.target, args.nix, None, &mut entries)?;
        apply(
            out, args, &forge, &repo, &scopes, workflow, style, &entries, withheld,
        )
    } else {
        // A preview lists destinations and compares nothing, so an
        // unresolved repository only means the owner substitution is
        // shown unrendered; the placeholder substitutes to itself, and an
        // absent scope list leaves the scope tokens standing.
        if resolved.repo.is_none() {
            out.frame(
                "note: no repository detected; an apply derives the owner from --repo <path>",
            );
        }
        let repo = resolved.repo;
        let scopes = args
            .scopes
            .as_deref()
            .map(landing::parse_scopes)
            .transpose()?
            .unwrap_or_default();
        let mut entries = landing::projection(
            &args.tech,
            &forge,
            repo.as_deref().unwrap_or("OWNER"),
            &scopes,
            workflow,
            Some(style),
            args.nix,
        )?;
        // The preview withholds exactly as the apply would, so what is
        // listed is what lands.
        let withheld = landing::withhold_nix(&args.target, args.nix, None, &mut entries)?;
        preview(out, args, &forge, repo, workflow, style, &entries, withheld)
    }
}

/// List every destination and write nothing.
#[allow(clippy::too_many_arguments)]
fn preview(
    out: Output,
    args: &InitArgs,
    forge: &str,
    repo: Option<String>,
    workflow: Workflow,
    style: Style,
    entries: &[Entry],
    withheld: Vec<landing::Withheld>,
) -> Result<(), RkError> {
    let repo_argument = repo.as_deref().unwrap_or("<owner/name>");
    let scopes_argument = args.scopes.as_deref().unwrap_or("<scope,scope>");
    let nix_flag = if args.nix { " --nix" } else { "" };
    let next = vec![format!(
        "rk init --tech {} --forge {forge} --repo {repo_argument} --scopes {scopes_argument} --workflow {} --style {}{nix_flag} --target {} --apply",
        args.tech,
        workflow.as_str(),
        style.as_str(),
        args.target
    )];
    out.result_line(format!(
        "DRY RUN: rk init writes these files into {}; re-run with --apply",
        args.target
    ));
    for entry in entries {
        out.result_line(&entry.destination);
    }
    for entry in &withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/4",
        mode: "preview",
        tech: args.tech.clone(),
        forge: forge.to_owned(),
        target: args.target.to_string(),
        repo,
        workflow: workflow.as_str(),
        style: style.as_str(),
        nix: args.nix,
        withheld: (!withheld.is_empty()).then_some(withheld),
        files: entries
            .iter()
            .map(|entry| FileEntry {
                path: entry.destination.clone(),
                kind: entry.kind.as_str(),
                action: "land",
            })
            .collect(),
        sentinels: None,
        next,
    })
}

/// Land the files — all-or-nothing against `rendered` conflicts — write
/// the record last, and report the judgment sentinels the operator still
/// owes.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn apply(
    out: Output,
    args: &InitArgs,
    forge: &str,
    repo: &str,
    scopes: &[String],
    workflow: Workflow,
    style: Style,
    entries: &[Entry],
    withheld: Vec<landing::Withheld>,
) -> Result<(), RkError> {
    refuse_a_recorded_target(args)?;
    landing::hooks_splice_refusal(&args.target)?;
    let planned = plan(&args.target, entries)?;
    let mut file_entries = Vec::new();
    let mut records = Vec::new();
    let mut sentinels = Vec::new();
    for Planned {
        entry,
        action,
        found,
    } in planned
    {
        if action == "write" {
            landing::write_destination(&args.target, entry)?;
        }
        out.result_line(format!(
            "{} {}",
            match action {
                "write" => "wrote",
                "kept" => "kept (target-owned)",
                _ => "unchanged",
            },
            entry.destination
        ));
        // What the destination now holds: the rendered bytes, or the
        // target's own where a seeded or state file was kept.
        let landed = match (action, found) {
            ("kept", Some(bytes)) => bytes,
            _ => entry.rendered.clone(),
        };
        collect_sentinels(&args.target, &entry.destination, &landed, &mut sentinels);
        records.push(FileRecord {
            destination: entry.destination.clone(),
            kind: entry.kind,
            sha256: Digest::of(&landed),
            baseline_sha256: match entry.kind {
                Kind::State => None,
                Kind::Rendered | Kind::Seeded => Some(Digest::of(&entry.baseline)),
            },
        });
        file_entries.push(FileEntry {
            path: entry.destination.clone(),
            kind: entry.kind.as_str(),
            action,
        });
    }
    for entry in &withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }

    // The record, last, after every file has landed.
    manifest::write(
        &args.target,
        &Manifest {
            schema_version: manifest::SCHEMA_VERSION,
            rk_version: env!("CARGO_PKG_VERSION").to_owned(),
            payload_sha256: crate::commands::payload::report().payload_sha256,
            origin: "init".to_owned(),
            tech: args.tech.clone(),
            forge: forge.to_owned(),
            landed_at: manifest::now(),
            parameters: Parameters {
                repo: repo.to_owned(),
                scopes: scopes.to_vec(),
                workflow,
                style: Some(style),
                nix: args.nix,
            },
            files: records,
            pins: registry::pins_for(&args.tech)
                .into_iter()
                .map(|pin| (pin.name, pin.version))
                .collect(),
        },
    )?;
    out.result_line(format!("wrote {}", manifest::MANIFEST_PATH));

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
        schema: "rk.init/4",
        mode: "apply",
        tech: args.tech.clone(),
        forge: forge.to_owned(),
        target: args.target.to_string(),
        repo: Some(repo.to_owned()),
        workflow: workflow.as_str(),
        style: style.as_str(),
        nix: args.nix,
        withheld: (!withheld.is_empty()).then_some(withheld),
        files: file_entries,
        sentinels: Some(sentinels),
        next,
    })
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

/// One planned destination: what was found there, and what an apply does
/// about it.
struct Planned<'a> {
    /// The projected artifact.
    entry: &'a Entry,
    /// `write`, `unchanged`, or `kept`.
    action: &'static str,
    /// The bytes the destination already held, where it held any.
    found: Option<Vec<u8>>,
}

/// The read pass before anything writes: every destination is read and
/// classified, so an unreadable path — a directory where a file should
/// land, a permission failure — surfaces here and the target is never
/// left half-written, and every `rendered` conflict is collected before
/// the one refusal.
fn plan<'a>(target: &Utf8Path, entries: &'a [Entry]) -> Result<Vec<Planned<'a>>, RkError> {
    let mut conflicts: Vec<&str> = Vec::new();
    let mut planned = Vec::new();
    for entry in entries {
        let found = landing::read_destination(target, entry)?;
        let action = match (&found, entry.kind) {
            (None, _) => "write",
            (Some(bytes), _) if *bytes == entry.rendered => "unchanged",
            (Some(_), Kind::Rendered) => {
                conflicts.push(entry.destination.as_str());
                "conflict"
            }
            (Some(_), Kind::Seeded | Kind::State) => "kept",
        };
        planned.push(Planned {
            entry,
            action,
            found,
        });
    }
    if conflicts.is_empty() {
        return Ok(planned);
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
    #![allow(clippy::expect_used)]

    use super::{FileEntry, Report, SentinelEntry};

    /// The complete `rk.init/3` shape, held by snapshot in both modes: a
    /// field rename or removal fails here and becomes a schema-version
    /// bump instead of a silent parser break at some agent.
    #[test]
    fn the_init_report_schema_snapshot_holds() {
        let apply = Report {
            schema: "rk.init/4",
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
            next: vec!["commit the landed files, the record included".into()],
        };
        assert_eq!(
            serde_json::to_string(&apply).expect("a report serializes"),
            r##"{"schema":"rk.init/4","mode":"apply","tech":"rust","forge":"github","target":"/tmp/t","repo":"acme/widget","workflow":"worktree","style":"trunk","nix":true,"withheld":[{"path":"flake.nix","reason":"the target already carries flake.nix"}],"files":[{"path":"release-plz.toml","kind":"seeded","action":"write"}],"sentinels":[{"path":"/tmp/t/release-plz.toml","line":3,"text":"# TODO(release-kit): keep false for a binary-only crate"}],"next":["commit the landed files, the record included"]}"##
        );
        let preview = Report {
            sentinels: None,
            repo: None,
            mode: "preview",
            nix: false,
            withheld: None,
            ..apply
        };
        assert_eq!(
            serde_json::to_string(&preview).expect("a report serializes"),
            r#"{"schema":"rk.init/4","mode":"preview","tech":"rust","forge":"github","target":"/tmp/t","workflow":"worktree","style":"trunk","nix":false,"files":[{"path":"release-plz.toml","kind":"seeded","action":"write"}],"next":["commit the landed files, the record included"]}"#,
            "a preview omits the sentinels, the unresolved repo, and an empty withheld list rather than serializing null"
        );
    }
}
