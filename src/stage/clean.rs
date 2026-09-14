//! `rk stage clean`: remove one stage that names itself, and nothing else.
//!
//! The argument is resolved without following a final symlink, judged
//! against the protected set, and then the parent directory and the
//! stage directory are opened and held. Every removal goes through those
//! descriptors, addressed as `/proc/self/fd/<fd>/<name>`, which resolves
//! against the directory the descriptor holds and not against the path
//! that was validated: a path swapped for a link after validation
//! redirects no deletion. The crate forbids `unsafe`, so the descriptor
//! walk uses the kernel's own link to an open directory instead of
//! `openat` and `unlinkat` through FFI.

use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use super::{RECEIPT_NAME, STAGE_SCHEMA};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// The proof's seam: a directory where the run writes `validated` after
/// it has opened the stage, then waits for `proceed` before it removes
/// anything, so a test can swap the path in between.
pub const PAUSE_VAR: &str = "RK_STAGE_CLEAN_PAUSE_AFTER_VALIDATE";

/// A stage validated and held open for removal.
#[derive(Debug)]
pub struct Validated {
    parent: File,
    stage: File,
    stage_ino: u64,
    name: std::ffi::OsString,
    resolved: PathBuf,
    target: String,
}

impl Validated {
    /// The canonical path the stage stands at.
    #[must_use]
    pub fn resolved(&self) -> &Path {
        &self.resolved
    }

    /// The target the stage receipt names.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }
}

/// The kernel's link to an open directory.
fn proc_path(dir: &File) -> PathBuf {
    use std::os::fd::AsRawFd as _;
    PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()))
}

/// Open a directory without following a final symlink.
fn open_dir(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

fn refuse(message: String, expected: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(Reason::DestructiveRefusal, message)
            .expected(expected)
            .action("pass the path of one stage directory, as rk stage printed it")
            .target_state("unchanged"),
    )
}

/// Resolve `argument` without following a final symlink, refuse every
/// protected or ambiguous path, and hold the stage and its parent open.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a path that does not exist, an
/// `unsupported-schema` refusal for a receipt outside `rk.stage/1`, and a
/// `destructive-refusal` for the filesystem root, a home directory, a
/// repository root, the receipt's target or any ancestor of it, a
/// symlink, a directory without a receipt, and a receipt whose
/// `stage_root` differs from the resolved argument.
pub fn validate(argument: &Path) -> Result<Validated, RkError> {
    let (parent, name, resolved) = resolve(argument)?;
    let metadata = judge_entry(argument, &resolved)?;
    let target = judge_receipt(&resolved)?;
    let parent_dir = open_dir(&parent)?;
    let stage = open_dir(&proc_path(&parent_dir).join(&name))?;
    let stage_ino = stage.metadata()?.ino();
    if stage_ino != metadata.ino() {
        return Err(refuse(
            format!(
                "{} changed while it was being validated",
                resolved.display()
            ),
            "a stage directory that stays put",
        ));
    }
    Ok(Validated {
        parent: parent_dir,
        stage,
        stage_ino,
        name,
        resolved,
        target,
    })
}

/// The canonical parent, the final name, and their join: the argument
/// resolved without following its final component, with the filesystem
/// root and the home directory refused by name.
fn resolve(argument: &Path) -> Result<(PathBuf, std::ffi::OsString, PathBuf), RkError> {
    let name = argument
        .file_name()
        .filter(|name| *name != "." && *name != "..")
        .ok_or_else(|| {
            refuse(
                format!("{} names no directory to remove", argument.display()),
                "the path of one stage directory",
            )
        })?
        .to_owned();
    let parent = argument
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| Path::new("."), |parent| parent);
    let parent = fs::canonicalize(parent).map_err(|error| missing(argument, &error))?;
    let resolved = parent.join(&name);
    if resolved.parent().is_none() {
        return Err(refuse(
            "the filesystem root is never a stage".to_owned(),
            "the path of one stage directory",
        ));
    }
    if let Some(home) = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .and_then(|home| fs::canonicalize(home).ok())
        && home == resolved
    {
        return Err(refuse(
            format!(
                "{} is the home directory, never a stage",
                resolved.display()
            ),
            "the path of one stage directory",
        ));
    }
    Ok((parent, name, resolved))
}

/// The entry at the resolved path: an existing directory that is neither
/// a link nor a repository root.
fn judge_entry(argument: &Path, resolved: &Path) -> Result<fs::Metadata, RkError> {
    let metadata = match fs::symlink_metadata(resolved) {
        Ok(metadata) => metadata,
        Err(error) => return Err(missing(argument, &error)),
    };
    if metadata.file_type().is_symlink() {
        return Err(refuse(
            format!(
                "{} is a symlink, and a link is never removed as a stage",
                resolved.display()
            ),
            "the stage directory itself, not a link to it",
        ));
    }
    if !metadata.is_dir() {
        return Err(refuse(
            format!("{} is not a directory", resolved.display()),
            "the path of one stage directory",
        ));
    }
    if fs::symlink_metadata(resolved.join(".git")).is_ok() {
        return Err(refuse(
            format!("{} is a repository root, never a stage", resolved.display()),
            "a stage directory, which carries no .git",
        ));
    }
    Ok(metadata)
}

/// The receipt at the resolved path, judged: present, parsing, at this
/// binary's schema, naming the directory it sits in, and naming a target
/// that is not the directory or below it. Returns the target it names.
fn judge_receipt(resolved: &Path) -> Result<String, RkError> {
    let receipt_path = resolved.join(RECEIPT_NAME);
    let receipt: serde_json::Value = match fs::read(&receipt_path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            refuse(
                format!(
                    "{} does not parse as a stage receipt: {error}",
                    receipt_path.display()
                ),
                "a directory rk stage wrote, whose stage.json reads",
            )
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(refuse(
                format!(
                    "{} carries no {RECEIPT_NAME}, so it is not a stage",
                    resolved.display()
                ),
                "a directory rk stage wrote",
            ));
        }
        Err(error) => return Err(error.into()),
    };
    let schema = receipt.get("schema").and_then(serde_json::Value::as_str);
    if schema != Some(STAGE_SCHEMA) {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::UnsupportedSchema,
                format!(
                    "{} declares schema {}, and this binary removes only {STAGE_SCHEMA}",
                    receipt_path.display(),
                    schema.map_or_else(|| "none".to_owned(), |schema| format!("{schema:?}"))
                ),
            )
            .expected(format!("a receipt declaring {STAGE_SCHEMA}"))
            .target_state("unchanged"),
        ));
    }
    let declared = receipt
        .get("stage_root")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if Path::new(declared) != resolved {
        return Err(refuse(
            format!(
                "{} names {declared:?} as its stage root, not {}",
                receipt_path.display(),
                resolved.display()
            ),
            "a receipt whose stage_root is the directory it sits in",
        ));
    }
    let target = receipt
        .get("target")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if !target.is_empty() && Path::new(&target).starts_with(resolved) {
        return Err(refuse(
            format!(
                "{} is the target {target} or an ancestor of it, never a stage",
                resolved.display()
            ),
            "a stage directory outside the target it describes",
        ));
    }
    Ok(target)
}

/// The absence of the argument, as the no-input failure.
fn missing(argument: &Path, error: &std::io::Error) -> RkError {
    if error.kind() == std::io::ErrorKind::NotFound {
        RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("{} does not exist", argument.display()),
            )
            .expected("the path of one stage directory, as rk stage printed it")
            .target_state("unchanged"),
        )
    } else {
        RkError::Io(std::io::Error::new(error.kind(), error.to_string()))
    }
}

/// Remove the validated stage through its held descriptors: every entry
/// below it, then the directory entry itself, which is removed only while
/// the parent still names the directory that was validated.
///
/// # Errors
///
/// Any removal failure, and an I/O failure naming the path where the
/// parent's entry was replaced while the stage was being emptied; in that
/// case the validated directory's contents are gone and the entry at the
/// path was left alone.
pub fn remove(validated: &Validated) -> Result<(), RkError> {
    pause_for_proof();
    remove_contents(&validated.stage)?;
    let entry = proc_path(&validated.parent).join(&validated.name);
    let current = fs::symlink_metadata(&entry)?;
    if current.file_type().is_symlink() || !current.is_dir() || current.ino() != validated.stage_ino
    {
        return Err(RkError::Io(std::io::Error::other(format!(
            "{} was replaced while the stage was being removed; the validated directory's contents were removed and the entry now at that path was left alone",
            validated.resolved.display()
        ))));
    }
    fs::remove_dir(&entry)?;
    Ok(())
}

/// Remove every entry below the open directory `dir`, addressing each by
/// the descriptor and never by the validated path.
fn remove_contents(dir: &File) -> std::io::Result<()> {
    let base = proc_path(dir);
    for entry in fs::read_dir(&base)? {
        let entry = entry?;
        let path = base.join(entry.file_name());
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            let sub = open_dir(&path)?;
            remove_contents(&sub)?;
            drop(sub);
            fs::remove_dir(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// The proof's pause: announce that validation is done, then wait for the
/// go-ahead. Bounded, so a forgotten variable cannot hang a run forever.
fn pause_for_proof() {
    let Some(dir) = std::env::var_os(PAUSE_VAR).filter(|value| !value.is_empty()) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let _ = fs::write(dir.join("validated"), b"");
    let proceed = dir.join("proceed");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !proceed.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::{remove, validate};
    use crate::diagnostic::Reason;
    use crate::error::RkError;

    fn reason_of(error: &RkError) -> Reason {
        error.reason()
    }

    /// A directory without a receipt, a receipt at another schema, and a
    /// receipt naming another root all refuse, and none is touched.
    #[test]
    fn a_directory_that_does_not_name_itself_refuses() {
        let scratch = tempfile::tempdir().expect("a scratch dir exists");
        let bare = scratch.path().join("bare");
        std::fs::create_dir(&bare).expect("creates");
        assert_eq!(
            reason_of(&validate(&bare).expect_err("refuses")),
            Reason::DestructiveRefusal
        );
        let other = scratch.path().join("other");
        std::fs::create_dir(&other).expect("creates");
        std::fs::write(other.join("stage.json"), r#"{"schema":"rk.other/1"}"#).expect("writes");
        assert_eq!(
            reason_of(&validate(&other).expect_err("refuses")),
            Reason::UnsupportedSchema
        );
        let moved = scratch.path().join("moved");
        std::fs::create_dir(&moved).expect("creates");
        std::fs::write(
            moved.join("stage.json"),
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":"/nowhere"}}"#,
                other.display()
            ),
        )
        .expect("writes");
        assert_eq!(
            reason_of(&validate(&moved).expect_err("refuses")),
            Reason::DestructiveRefusal
        );
        assert!(bare.is_dir() && other.is_dir() && moved.is_dir());
    }

    /// A valid stage is removed through its descriptors, and its siblings
    /// stand.
    #[test]
    fn a_valid_stage_is_removed_and_its_siblings_stand() {
        let scratch = tempfile::tempdir().expect("a scratch dir exists");
        let canonical = std::fs::canonicalize(scratch.path()).expect("canonical");
        let stage = canonical.join("stage");
        std::fs::create_dir_all(stage.join("artifacts/deep")).expect("creates");
        std::fs::write(stage.join("artifacts/deep/file"), b"x").expect("writes");
        std::fs::write(
            stage.join("stage.json"),
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":"/nowhere"}}"#,
                stage.display()
            ),
        )
        .expect("writes");
        let sibling = canonical.join("sibling");
        std::fs::write(&sibling, b"keep").expect("writes");
        let validated = validate(&stage).expect("validates");
        remove(&validated).expect("removes");
        assert!(!stage.exists());
        assert_eq!(std::fs::read(&sibling).expect("reads"), b"keep");
    }
}
