//! `rk stage compare`: what a landing from a stage would do, destination
//! by destination, with the evidence that decides who owns each one.
//!
//! The comparison reads the stage's receipt, resolves its answers the way
//! `--stage` does, and runs the production decision over the target as it
//! stands, so the word a row carries is the word the landing prints. Each
//! row states how the target's bytes stand against the candidate, the
//! destination's kind, and, where the bytes differ, the provenance: the
//! record half is whether the receipt vouches for the bytes on disk, and
//! the history half is whether Git ever touched the path. A destination
//! no receipt names also says whether its history shows an edit after
//! the commit that brought it. The same verb is the proof after a
//! landing: every rendered destination then reads `equal`.
//!
//! SATISFIES staging:the-comparison-is-the-landing-decision

use std::path::Path;
use std::process::Command;

use camino::Utf8Path;
use serde::Serialize;

use super::{ARTIFACTS_DIR, read};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing;
use crate::landing::apply::{self, Held};
use crate::landing::manifest;

/// The report's shape version.
pub const SCHEMA: &str = "rk.stage-compare/1";

/// One destination, compared.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The destination, relative to the target.
    pub path: String,
    /// `equal`, `absent`, `differs`, or `retired`.
    pub state: &'static str,
    /// `rendered`, `region`, `seeded`, or `state`.
    pub kind: &'static str,
    /// For a destination that differs or is retired: `attributed` where
    /// the record vouches for the bytes and Git touched the path,
    /// `reduced` where one of the two holds, `unknown` where neither does.
    pub provenance: Option<&'static str>,
    /// Whether the receipt vouches for the bytes the target holds; null
    /// where the receipt names no such destination or nothing stands.
    pub vouched: Option<bool>,
    /// For a destination no receipt names: whether its history shows an
    /// edit after the commit that brought it; null where Git cannot say.
    pub edited_after_added: Option<bool>,
    /// What the production landing prints for it: `created`, `replaced`,
    /// `matched`, `preserved`, `drift`, `released`, or `collision`.
    pub word: &'static str,
    /// Whether the landing verb accepts `--replace` for it: true only for
    /// a `collision` the production decision itself marks replaceable, so
    /// any other collision is a refusal no flag passes.
    pub replaceable: bool,
}

/// Whether the history half could be read.
#[derive(Debug, Clone, Serialize)]
pub struct History {
    /// Whether Git answers for the target.
    pub available: bool,
    /// Why not, where it does not.
    pub reason: Option<String>,
}

/// The machine form of `rk stage compare`.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// The shape version of this document.
    pub schema: &'static str,
    /// The stage compared from.
    pub stage_root: String,
    /// The target compared.
    pub target: String,
    /// Whether the history half could be read.
    pub history: History,
    /// One row per destination the stage carries or the record names.
    pub rows: Vec<Row>,
}

/// Compare the stage at `root` with `target`.
///
/// # Errors
///
/// The stage reader's refusals for another binary's or another target's
/// stage, the parameter resolution's refusals, and read failures.
#[allow(
    clippy::too_many_lines,
    reason = "one pass resolves the stage's answers, runs the production decision, and judges every row"
)]
pub fn compare(root: &Utf8Path, target: &Utf8Path) -> Result<Report, RkError> {
    let canonical = std::fs::canonicalize(target.as_std_path())?;
    let receipt = read::load(root, &canonical)?;
    let held = Held::open(target)?;
    let recorded = manifest::load(held.base())?;
    let config = crate::config::load(held.base().as_std_path())?;
    // Git cannot enter the held base, so the standing reads the path, as
    // the history below does.
    let purpose = super::purpose(target, config.as_ref(), recorded.is_some())?;
    let params = landing::Params::resolve(
        held.base(),
        &read::inputs(&receipt.parameters),
        config.as_ref(),
        recorded.as_ref(),
        purpose,
    )?;
    read::confirm(&params, &receipt, root)?;
    let prepared = apply::prepare(&held, recorded.as_ref(), &params, config.as_ref())?;
    let history = match crate::assess::git_work_tree(target) {
        Ok(true) => History {
            available: true,
            reason: None,
        },
        Ok(false) => History {
            available: false,
            reason: Some(
                "the target is not a non-bare Git work tree, so the history half reads as missing"
                    .to_owned(),
            ),
        },
        Err(error) => History {
            available: false,
            reason: Some(format!(
                "git did not answer, so the history half reads as missing: {error}"
            )),
        },
    };
    // The word is the decision of the verb that lands this stage: an
    // adoption judges each destination itself and writes nothing else.
    let adopting = purpose == landing::Purpose::Adopt;
    // The upgrade refuses a record from a newer binary before it reaches
    // any replace check, so nothing it would refuse reads replaceable.
    let refused = recorded.as_ref().is_some_and(|record| {
        manifest::alignment(&record.rk_version, env!("CARGO_PKG_VERSION"))
            == manifest::Alignment::TargetNewer
    });
    let word = |destination: &str| -> Result<(&'static str, bool), RkError> {
        if adopting
            && let Some(candidate) = prepared
                .projection
                .candidates
                .iter()
                .find(|candidate| candidate.destination == destination)
        {
            let judged = apply::judge_adoption(&held, candidate)?;
            return Ok((judged.word(), judged.replaceable(candidate.kind)));
        }
        let replaceable = !refused
            && prepared
                .collisions
                .iter()
                .any(|collision| collision.path == destination && collision.replaceable);
        Ok((
            prepared
                .decision(destination)
                .map_or("collision", |decision| decision.action.as_str()),
            replaceable,
        ))
    };
    let mut rows = Vec::new();
    for candidate in &receipt.candidates {
        let region = candidate.placement == "region";
        let artifact = std::fs::read(
            root.join(ARTIFACTS_DIR)
                .join(&candidate.destination)
                .as_std_path(),
        )?;
        let expected = if region {
            region_of(&candidate.destination, &artifact).unwrap_or(artifact)
        } else {
            artifact
        };
        let current = landing::read_recorded(held.base(), &candidate.destination)?;
        let state = match &current {
            None => "absent",
            Some(bytes) if *bytes == expected => "equal",
            Some(_) => "differs",
        };
        let record = recorded
            .as_ref()
            .and_then(|record| record.file(&candidate.destination));
        rows.push(row(
            &candidate.destination,
            state,
            if region {
                "region"
            } else {
                candidate.kind.as_str()
            },
            record,
            current.as_deref(),
            &history,
            target,
            word(&candidate.destination)?,
        ));
    }
    for destination in &receipt.retired {
        let record = recorded
            .as_ref()
            .and_then(|record| record.file(destination));
        let kind = record.map_or("rendered", |record| match record.placement {
            manifest::Placement::Region => "region",
            manifest::Placement::Whole => record.kind.as_str(),
        });
        // The landing leaves a retired path alone, so an entry no read
        // answers, such as a directory where the file stood, still has
        // its row: nothing vouches for it, its provenance is unknown, and
        // every other destination is reported.
        let Ok(current) = landing::read_recorded(held.base(), destination) else {
            rows.push(Row {
                path: destination.clone(),
                state: "retired",
                kind,
                provenance: Some("unknown"),
                vouched: None,
                edited_after_added: None,
                word: "released",
                replaceable: false,
            });
            continue;
        };
        rows.push(row(
            destination,
            "retired",
            kind,
            record,
            current.as_deref(),
            &history,
            target,
            ("released", false),
        ));
    }
    Ok(Report {
        schema: SCHEMA,
        stage_root: receipt.stage_root,
        target: receipt.target,
        history,
        rows,
    })
}

/// One row, its provenance computed from the record and the history.
#[allow(
    clippy::too_many_arguments,
    reason = "a row joins the stage, the record, the disk, and the history, each one argument"
)]
fn row(
    path: &str,
    state: &'static str,
    kind: &'static str,
    record: Option<&manifest::FileRecord>,
    current: Option<&[u8]>,
    history: &History,
    target: &Utf8Path,
    (word, replaceable): (&'static str, bool),
) -> Row {
    let vouched = record
        .zip(current)
        .map(|(record, bytes)| Digest::of(bytes) == record.sha256);
    let commits = history
        .available
        .then(|| commits_touching(target, path))
        .flatten();
    let provenance = matches!(state, "differs" | "retired").then(|| {
        let record_half = vouched == Some(true);
        let history_half = commits.as_ref().is_some_and(|commits| commits > &0);
        match (record_half, history_half) {
            (true, true) => "attributed",
            (true, false) | (false, true) => "reduced",
            (false, false) => "unknown",
        }
    });
    let edited_after_added = if record.is_none() && current.is_some() {
        commits.and_then(|commits| match commits {
            0 => None,
            1 => edited_in_the_tree(target, path),
            _ => Some(true),
        })
    } else {
        None
    };
    Row {
        path: path.to_owned(),
        state,
        kind,
        provenance,
        vouched,
        edited_after_added,
        word,
        replaceable,
    }
}

/// The marked region of a spliced document, by the markers its name
/// implies.
fn region_of(destination: &str, document: &[u8]) -> Option<Vec<u8>> {
    let (begin, end) = landing::block_markers(destination)?;
    landing::extract_block(&String::from_utf8_lossy(document), begin, end)
        .map(|block| block.as_bytes().to_vec())
}

/// How many commits touched `path`, or `None` where Git cannot say.
fn commits_touching(target: &Utf8Path, path: &str) -> Option<usize> {
    let output = git(target)
        .args(["log", "--format=%H", "--", path])
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    })
}

/// Whether the work tree holds an edit to `path` past its committed
/// bytes, or `None` where Git cannot say.
fn edited_in_the_tree(target: &Utf8Path, path: &str) -> Option<bool> {
    let status = git(target)
        .args(["diff", "--quiet", "HEAD", "--", path])
        .status()
        .ok()?;
    match status.code() {
        Some(0) => Some(false),
        Some(1) => Some(true),
        _ => None,
    }
}

/// A git command against the target alone: the variables a running hook
/// exports are scrubbed, so an inherited `GIT_DIR` cannot redirect it.
fn git(target: &Utf8Path) -> Command {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .env("LC_ALL", "C")
        .arg("-C")
        .arg(Path::new(target.as_str()));
    command
}

/// Whether every rendered destination of a report reads `equal`: the
/// proof after a landing. A retired row is no candidate: the landing
/// released it to the target, which owns its bytes from then on.
#[must_use]
pub fn rendered_equal(report: &Report) -> bool {
    report
        .rows
        .iter()
        .filter(|row| row.state != "retired" && matches!(row.kind, "rendered" | "region"))
        .all(|row| row.state == "equal")
}

#[cfg(test)]
mod tests {
    use super::{History, Report, Row};

    /// The complete `rk.stage-compare/1` shape, held by snapshot.
    #[test]
    fn the_compare_report_schema_snapshot_holds() {
        let report = Report {
            schema: super::SCHEMA,
            stage_root: "/state/stages/abc/0.9.0".into(),
            target: "/srv/widget".into(),
            history: History {
                available: true,
                reason: None,
            },
            rows: vec![
                Row {
                    path: ".github/workflows/release-plz.yml".into(),
                    state: "differs",
                    kind: "rendered",
                    provenance: Some("reduced"),
                    vouched: Some(false),
                    edited_after_added: None,
                    word: "collision",
                    replaceable: true,
                },
                Row {
                    path: "AGENTS.md".into(),
                    state: "equal",
                    kind: "region",
                    provenance: None,
                    vouched: Some(true),
                    edited_after_added: None,
                    word: "replaced",
                    replaceable: false,
                },
            ],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.stage-compare/1","stage_root":"/state/stages/abc/0.9.0","target":"/srv/widget","history":{"available":true,"reason":null},"rows":[{"path":".github/workflows/release-plz.yml","state":"differs","kind":"rendered","provenance":"reduced","vouched":false,"edited_after_added":null,"word":"collision","replaceable":true},{"path":"AGENTS.md","state":"equal","kind":"region","provenance":null,"vouched":true,"edited_after_added":null,"word":"replaced","replaceable":false}]}"#
        );
        assert!(!super::rendered_equal(&report));
    }

    /// A retired destination stays the target's after the landing, so
    /// it never counts against the proof.
    #[test]
    fn a_retired_row_does_not_count_against_the_proof() {
        let row = |path: &str, state: &'static str| Row {
            path: path.into(),
            state,
            kind: "rendered",
            provenance: None,
            vouched: None,
            edited_after_added: None,
            word: "released",
            replaceable: false,
        };
        let report = Report {
            schema: super::SCHEMA,
            stage_root: "/state/stages/abc/0.9.0".into(),
            target: "/srv/widget".into(),
            history: History {
                available: true,
                reason: None,
            },
            rows: vec![
                row(".github/workflows/release-plz.yml", "equal"),
                row(".github/workflows/legacy.yml", "retired"),
            ],
        };
        assert!(super::rendered_equal(&report));
    }
}
