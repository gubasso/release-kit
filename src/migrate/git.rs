//! The git a migration reads the repository through.
//!
//! Every call answers for the `-C` target alone: the variables a running
//! hook exports are scrubbed, so an inherited `GIT_DIR` cannot redirect a
//! call at another repository, and the locale is pinned to `C`, so the
//! diagnostics stay untranslated.

use std::process::{Command, Output};

use camino::Utf8Path;

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// One git call against `target`, its output whatever the status.
///
/// # Errors
///
/// A spawn failure.
pub fn run(target: &Utf8Path, args: &[&str]) -> Result<Output, RkError> {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .env("LC_ALL", "C")
        .env_remove("LANGUAGE")
        .arg("-C")
        .arg(target)
        .args(args)
        .output()
        .map_err(|error| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("git could not be spawned: {error}"),
                )
                .expected("git on PATH, or RK_GIT_BIN naming it"),
            )
        })
}

/// The stdout of one git call that must succeed.
///
/// # Errors
///
/// A spawn failure, or a call that exits non-zero, naming its stderr.
pub fn stdout(target: &Utf8Path, args: &[&str]) -> Result<Vec<u8>, RkError> {
    let out = run(target, args)?;
    if !out.status.success() {
        return Err(RkError::subprocess(
            Diagnostic::new(
                Reason::SubprocessFailed,
                format!(
                    "git {} failed at {target}: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            )
            .expected("a git repository with at least one commit"),
        ));
    }
    Ok(out.stdout)
}

/// The non-empty lines of one git call that must succeed.
///
/// # Errors
///
/// As [`stdout`].
pub fn lines(target: &Utf8Path, args: &[&str]) -> Result<Vec<String>, RkError> {
    Ok(String::from_utf8_lossy(&stdout(target, args)?)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// The regular files `rev` holds among `paths`, each with its blob id:
/// mode 100644 or 100755, never a symlink or a submodule.
///
/// # Errors
///
/// As [`stdout`], where `rev` names no tree.
pub fn regular_blobs(
    target: &Utf8Path,
    rev: &str,
    paths: &[&str],
) -> Result<std::collections::BTreeMap<String, String>, RkError> {
    let mut args = vec!["ls-tree", "-r", "-z", rev, "--"];
    args.extend_from_slice(paths);
    Ok(stdout(target, &args)?
        .split(|byte| *byte == 0)
        .filter_map(|record| {
            let (meta, path) = std::str::from_utf8(record).ok()?.split_once('\t')?;
            let mut fields = meta.split(' ');
            let mode = fields.next()?;
            let oid = fields.nth(1)?;
            matches!(mode, "100644" | "100755").then(|| (path.to_owned(), oid.to_owned()))
        })
        .collect())
}

/// The blob id of the bytes at each of `paths`, read raw, with no
/// filter applied, in the order given.
///
/// # Errors
///
/// As [`stdout`], where a path cannot be read.
pub fn raw_blob_ids(target: &Utf8Path, paths: &[&str]) -> Result<Vec<String>, RkError> {
    let mut args = vec!["hash-object", "--no-filters", "--"];
    args.extend_from_slice(paths);
    lines(target, &args)
}
