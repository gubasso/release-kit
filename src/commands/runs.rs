//! `rk runs`: inspect and prune the run journals.
//!
//! The journal is audit evidence: what ran, against what, and what came
//! back. These verbs read and bound it; nothing here resumes anything.

use serde::Serialize;

use crate::cli::runs::{RunsAction, RunsArgs};
use crate::error::RkError;
use crate::output::Output;
use crate::setup::journal::{self, RUNS_KEPT};

/// One run's listing row, read from its `meta.json`.
#[derive(Debug, Serialize)]
struct RunRow {
    /// The run id, equal to its directory name.
    id: String,
    /// The subcommand that ran, where the record is readable.
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    /// The forge acted on.
    #[serde(skip_serializing_if = "Option::is_none")]
    forge: Option<String>,
    /// The process exit code, absent for a run still open or killed.
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i64>,
    /// The failure reason, where one was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// The machine form of `rk runs list`.
#[derive(Debug, Serialize)]
struct ListReport {
    /// The shape version of this document.
    schema: &'static str,
    /// Every kept run, oldest first.
    runs: Vec<RunRow>,
}

/// Dispatch the runs surface.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown run id and I/O failures
/// from the state root.
pub fn run(args: &RunsArgs) -> Result<(), RkError> {
    match &args.action {
        RunsAction::List { json } => list(Output::new(*json)),
        RunsAction::Show { id, json } => show(Output::new(*json), id),
        RunsAction::Prune { keep } => {
            prune(keep.unwrap_or(RUNS_KEPT));
            Ok(())
        }
    }
}

fn read_row(id: &str) -> RunRow {
    let meta = journal::runs_root()
        .map(|root| root.join(id).join("meta.json"))
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let field = |name: &str| {
        meta.as_ref()
            .and_then(|value| value[name].as_str().map(str::to_owned))
    };
    RunRow {
        id: id.to_owned(),
        command: field("command"),
        forge: field("forge"),
        exit_code: meta.as_ref().and_then(|value| value["exit_code"].as_i64()),
        reason: field("reason"),
    }
}

fn list(out: Output) -> Result<(), RkError> {
    let rows: Vec<RunRow> = journal::list_run_ids()
        .iter()
        .map(|id| read_row(id))
        .collect();
    for row in &rows {
        use std::fmt::Write as _;
        let mut line = row.id.clone();
        if let Some(command) = &row.command {
            let _ = write!(line, "  {command}");
        }
        match (row.exit_code, &row.reason) {
            (Some(0), _) => line.push_str("  ok"),
            (Some(code), Some(reason)) => {
                let _ = write!(line, "  exit {code} ({reason})");
            }
            (Some(code), None) => {
                let _ = write!(line, "  exit {code}");
            }
            (None, _) => line.push_str("  unfinished"),
        }
        out.result_line(line);
    }
    if rows.is_empty() {
        out.result_line("no runs are kept");
    }
    out.emit(&ListReport {
        schema: "rk.runs/1",
        runs: rows,
    })?;
    Ok(())
}

fn show(out: Output, id: &str) -> Result<(), RkError> {
    // An id is one directory name under the runs root, never a path: a
    // separator or a parent component would let a stray argument read a
    // meta.json from anywhere on the filesystem.
    if id.contains(['/', '\\']) || id == ".." || id == "." || id.is_empty() {
        return Err(RkError::NotFound {
            kind: "run",
            name: id.to_owned(),
        });
    }
    let root = journal::runs_root()
        .ok_or_else(|| RkError::Other(anyhow::anyhow!("neither XDG_STATE_HOME nor HOME is set")))?;
    let dir = root.join(id);
    let meta_path = dir.join("meta.json");
    let bytes = std::fs::read(&meta_path).map_err(|_| RkError::NotFound {
        kind: "run",
        name: id.to_owned(),
    })?;
    if out.is_json() {
        let meta: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(anyhow::Error::from)?;
        out.emit(&meta)?;
        return Ok(());
    }
    out.result_raw(&String::from_utf8_lossy(&bytes));
    out.result_line("");
    out.result_line(format!(
        "events:     {}",
        dir.join("events.jsonl").display()
    ));
    out.result_line(format!(
        "transcript: {}",
        dir.join("transcript.txt").display()
    ));
    if dir.join("scripts").is_dir() {
        out.result_line(format!("scripts:    {}", dir.join("scripts").display()));
    }
    Ok(())
}

fn prune(keep: usize) {
    let removed = journal::prune_to(keep);
    let out = Output::human();
    out.result_line(format!("pruned {removed} runs; keeping the newest {keep}"));
}
