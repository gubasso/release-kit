//! `rk init`: land a technology's deterministic files into a target.
//!
//! Dry-run by default: without `--apply` the destinations are listed and
//! nothing is touched. Apply is all-or-nothing against conflicts: every
//! destination whose bytes differ from the payload is reported and the
//! whole landing is refused. Each write goes through the temp-plus-rename
//! writer, so a file lands whole or not at all; an I/O failure mid-loop
//! can still leave earlier files landed, and the refusal path is what
//! guarantees a conflicting target is never touched.

use std::fs;

use camino::Utf8Path;
use serde::Serialize;

use crate::atomic;
use crate::cli::init::InitArgs;
use crate::commands::walk;
use crate::diagnostic::{Diagnostic, Reason};
use crate::embedded;
use crate::error::RkError;
use crate::output::Output;

/// One destination and what happened to it.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// `land` in a preview; `write` or `unchanged` in an apply.
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
    /// The target directory.
    target: String,
    /// Every destination, with its action.
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
/// Returns [`RkError::Usage`] for an unknown technology,
/// [`RkError::Refusal`] when the target is missing or a destination
/// conflicts, and [`RkError::Io`] on filesystem failure.
pub fn run(args: &InitArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let tech_dir = embedded::SNIPPETS.get_dir(&args.tech).ok_or_else(|| {
        let known: Vec<String> = embedded::SNIPPETS
            .dirs()
            .map(|d| d.path().to_string_lossy().into_owned())
            .collect();
        RkError::Usage(format!(
            "unknown tech '{}'; the bindings are: {}",
            args.tech,
            known.join(", ")
        ))
    })?;

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

    // Payload paths carry the `<tech>/` prefix; destinations do not.
    let files: Vec<(String, &[u8])> = walk(tech_dir)
        .into_iter()
        .map(|(path, contents)| {
            let rel = path
                .strip_prefix(&format!("{}/", args.tech))
                .map_or(path.as_str(), |r| r)
                .to_owned();
            (rel, contents)
        })
        .collect();

    if args.apply {
        apply(out, args, &files)
    } else {
        preview(out, args, &files)
    }
}

/// List every destination and write nothing.
fn preview(out: Output, args: &InitArgs, files: &[(String, &[u8])]) -> Result<(), RkError> {
    let next = vec![format!(
        "rk init --tech {} --target {} --apply",
        args.tech, args.target
    )];
    out.result_line(format!(
        "DRY RUN: rk init writes these files into {}; re-run with --apply",
        args.target
    ));
    for (rel, _) in files {
        out.result_line(rel);
    }
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/1",
        mode: "preview",
        tech: args.tech.clone(),
        target: args.target.to_string(),
        files: files
            .iter()
            .map(|(rel, _)| FileEntry {
                path: rel.clone(),
                action: "land",
            })
            .collect(),
        sentinels: None,
        next,
    })
}

/// Land the files, all-or-nothing against conflicts, and report the
/// sentinels the operator still owes.
fn apply(out: Output, args: &InitArgs, files: &[(String, &[u8])]) -> Result<(), RkError> {
    // Every destination is read before anything writes, so an unreadable
    // path — a directory where a file should land, a permission failure —
    // surfaces here and the target is never left half-written.
    let mut conflicts: Vec<&str> = Vec::new();
    for (rel, contents) in files {
        let dest = args.target.join(rel);
        match fs::read(&dest) {
            Ok(found) if found == *contents => {}
            Ok(_) => conflicts.push(rel.as_str()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    if !conflicts.is_empty() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "these files exist with different content, and nothing was written: {}",
                    conflicts.join(", ")
                ),
            )
            .expected("every destination absent, or holding this payload's bytes")
            .target_state("unchanged"),
        ));
    }

    let mut entries = Vec::new();
    for (rel, contents) in files {
        let dest = args.target.join(rel);
        let action = if dest.is_file() {
            "unchanged"
        } else {
            atomic::write(dest.as_std_path(), contents)?;
            "write"
        };
        out.result_line(match action {
            "write" => format!("wrote {rel}"),
            _ => format!("unchanged {rel}"),
        });
        entries.push(FileEntry {
            path: rel.clone(),
            action,
        });
    }

    let sentinels = collect_sentinels(&args.target, files);
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
            "commit the landed files".to_owned()
        } else {
            "fill each sentinel above, then commit the landed files".to_owned()
        },
        "rk method setup orders what follows".to_owned(),
    ];
    out.next(&next);
    out.emit(&Report {
        schema: "rk.init/1",
        mode: "apply",
        tech: args.tech.clone(),
        target: args.target.to_string(),
        files: entries,
        sentinels: Some(sentinels),
        next,
    })
}

/// Every sentinel line left in the landed files, so nothing stays
/// half-configured silently.
fn collect_sentinels(target: &Utf8Path, files: &[(String, &[u8])]) -> Vec<SentinelEntry> {
    let mut found = Vec::new();
    for (rel, contents) in files {
        let text = String::from_utf8_lossy(contents);
        for (idx, line) in text.lines().enumerate() {
            if line.contains(embedded::SENTINEL) {
                found.push(SentinelEntry {
                    path: target.join(rel).to_string(),
                    line: idx + 1,
                    text: line.trim().to_owned(),
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{FileEntry, Report, SentinelEntry};

    /// The complete `rk.init/1` shape, held by snapshot in both modes: a
    /// field rename or removal fails here and becomes a schema-version
    /// bump instead of a silent parser break at some agent.
    #[test]
    fn the_init_report_schema_snapshot_holds() {
        let apply = Report {
            schema: "rk.init/1",
            mode: "apply",
            tech: "rust".into(),
            target: "/tmp/t".into(),
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                action: "write",
            }],
            sentinels: Some(vec![SentinelEntry {
                path: "/tmp/t/release-plz.toml".into(),
                line: 3,
                text: "# TODO(release-kit): set the repository owner".into(),
            }]),
            next: vec!["commit the landed files".into()],
        };
        assert_eq!(
            serde_json::to_string(&apply).expect("a report serializes"),
            r##"{"schema":"rk.init/1","mode":"apply","tech":"rust","target":"/tmp/t","files":[{"path":"release-plz.toml","action":"write"}],"sentinels":[{"path":"/tmp/t/release-plz.toml","line":3,"text":"# TODO(release-kit): set the repository owner"}],"next":["commit the landed files"]}"##
        );
        let preview = Report {
            sentinels: None,
            mode: "preview",
            ..apply
        };
        assert_eq!(
            serde_json::to_string(&preview).expect("a report serializes"),
            r#"{"schema":"rk.init/1","mode":"preview","tech":"rust","target":"/tmp/t","files":[{"path":"release-plz.toml","action":"write"}],"next":["commit the landed files"]}"#,
            "a preview omits the sentinels field rather than serializing null"
        );
    }
}
