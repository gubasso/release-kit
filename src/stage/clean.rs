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

/// The proof's second seam: the name of one child directory whose open
/// waits, after its check, for `proceed-checked` under the pause
/// directory, having written `checked` there.
pub const PAUSE_BEFORE_OPEN_VAR: &str = "RK_STAGE_CLEAN_PAUSE_BEFORE_OPEN";

/// A stage validated and held open for removal.
#[derive(Debug)]
pub struct Validated {
    parent: File,
    stage: File,
    stage_dev: u64,
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
    let opened = stage.metadata()?;
    let (stage_dev, stage_ino) = (opened.dev(), opened.ino());
    if Identity::of(&opened) != Identity::of(&metadata) {
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
        stage_dev,
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

/// The schema alone, read first so a receipt at another schema is named
/// as such rather than as one missing this schema's fields.
#[derive(Debug, serde::Deserialize)]
struct SchemaOnly {
    schema: String,
}

/// The two paths a cleanup reads from a receipt once its schema is
/// known, each required: a receipt missing one is not a stage receipt
/// this verb removes.
#[derive(Debug, serde::Deserialize)]
struct CleanReceipt {
    stage_root: String,
    target: String,
}

/// The receipt at the resolved path, judged: present, parsing as a typed
/// receipt, at this binary's schema, naming the directory it sits in, and
/// naming a canonical absolute target that is not the directory or below
/// it. Returns the target it names.
fn judge_receipt(resolved: &Path) -> Result<String, RkError> {
    let receipt_path = resolved.join(RECEIPT_NAME);
    let unparsed = |error: serde_json::Error| {
        refuse(
            format!(
                "{} does not parse as a stage receipt: {error}",
                receipt_path.display()
            ),
            "a directory rk stage wrote, whose stage.json carries schema, stage_root, and target",
        )
    };
    let bytes = match fs::read(&receipt_path) {
        Ok(bytes) => bytes,
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
    let declared: SchemaOnly = serde_json::from_slice(&bytes).map_err(unparsed)?;
    if declared.schema != STAGE_SCHEMA {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::UnsupportedSchema,
                format!(
                    "{} declares schema {:?}, and this binary removes only {STAGE_SCHEMA}",
                    receipt_path.display(),
                    declared.schema
                ),
            )
            .expected(format!("a receipt declaring {STAGE_SCHEMA}"))
            .target_state("unchanged"),
        ));
    }
    let receipt: CleanReceipt = serde_json::from_slice(&bytes).map_err(unparsed)?;
    if Path::new(&receipt.stage_root) != resolved {
        return Err(refuse(
            format!(
                "{} names {:?} as its stage root, not {}",
                receipt_path.display(),
                receipt.stage_root,
                resolved.display()
            ),
            "a receipt whose stage_root is the directory it sits in",
        ));
    }
    let target = Path::new(&receipt.target);
    if !is_canonical_shape(target) {
        return Err(refuse(
            format!(
                "{} names {:?} as its target, which is not a canonical absolute path",
                receipt_path.display(),
                receipt.target
            ),
            "a receipt whose target is the canonical absolute path rk stage recorded",
        ));
    }
    if let Ok(canonical) = fs::canonicalize(target)
        && canonical != target
    {
        return Err(refuse(
            format!(
                "{} names {:?} as its target, whose canonical path is {}",
                receipt_path.display(),
                receipt.target,
                canonical.display()
            ),
            "a receipt whose target is the canonical absolute path rk stage recorded",
        ));
    }
    if target.starts_with(resolved) {
        return Err(refuse(
            format!(
                "{} is the target {} or an ancestor of it, never a stage",
                resolved.display(),
                receipt.target
            ),
            "a stage directory outside the target it describes",
        ));
    }
    Ok(receipt.target)
}

/// Whether a recorded path has the shape a canonical path has: absolute,
/// nonempty, and made of plain names alone.
fn is_canonical_shape(path: &Path) -> bool {
    use std::path::Component;
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return false;
    }
    let mut any = false;
    for component in components {
        if !matches!(component, Component::Normal(_)) {
            return false;
        }
        any = true;
    }
    any
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

/// One entry's identity: the device and inode the check saw, which the
/// opened descriptor must agree with before anything below it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
}

impl Identity {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
}

/// The failure for an entry that changed under the removal: the walk
/// stops, nothing beyond the held descriptors was touched, and the run
/// exits as an I/O failure naming where.
fn swapped(path: &Path, detail: &str) -> std::io::Error {
    swapped_leaving(
        path,
        detail,
        "the removal stopped there, and nothing outside the validated stage was touched",
    )
}

/// [`swapped`], stating what the run left behind in its own words.
fn swapped_leaving(path: &Path, detail: &str, aftermath: &str) -> std::io::Error {
    std::io::Error::other(format!(
        "{} {detail} while the stage was being removed; {aftermath}",
        path.display()
    ))
}

/// Remove the validated stage through its held descriptors: every entry
/// below it, then the directory entry itself, which is removed only while
/// the parent still names the directory that was validated.
///
/// # Errors
///
/// Any removal failure, and an I/O failure naming the entry where the
/// tree changed under the removal: a child whose opened identity differs
/// from the one checked, a name that vanished after validation, or a
/// parent entry replaced while the stage was being emptied. In the last
/// case the validated directory's contents are gone and the entry at the
/// path was left alone.
pub fn remove(validated: &Validated) -> Result<(), RkError> {
    pause_for_proof("validated");
    let shown = validated.resolved.as_path();
    remove_contents(&validated.stage, shown)?;
    let entry = proc_path(&validated.parent).join(&validated.name);
    let current = match fs::symlink_metadata(&entry) {
        Ok(current) => current,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RkError::Io(swapped_leaving(
                shown,
                "vanished from its parent",
                "the validated directory's contents were removed through the held descriptor, and nothing else was touched",
            )));
        }
        Err(error) => return Err(error.into()),
    };
    if current.file_type().is_symlink()
        || !current.is_dir()
        || Identity::of(&current)
            != (Identity {
                dev: validated.stage_dev,
                ino: validated.stage_ino,
            })
    {
        return Err(RkError::Io(swapped_leaving(
            shown,
            "was replaced",
            "the validated directory's contents were removed and the entry now at that path was left alone",
        )));
    }
    fs::remove_dir(&entry)?;
    Ok(())
}

/// Remove every entry below the open directory `dir`, addressing each by
/// the descriptor and never by the validated path. A directory child is
/// opened and its descriptor's identity compared with the checked entry
/// before the walk descends, and compared again before its name goes; a
/// file's entry is checked immediately before its unlink. `shown` is the
/// path the failure names for the operator.
fn remove_contents(dir: &File, shown: &Path) -> std::io::Result<()> {
    let base = proc_path(dir);
    let vanished = |error: std::io::Error, name: &std::ffi::OsStr| {
        if error.kind() == std::io::ErrorKind::NotFound {
            swapped(&shown.join(name), "vanished")
        } else {
            error
        }
    };
    for entry in fs::read_dir(&base)? {
        let entry = entry?;
        let name = entry.file_name();
        let path = base.join(&name);
        let checked = fs::symlink_metadata(&path).map_err(|error| vanished(error, &name))?;
        if checked.is_dir() {
            pause_before_open(&name);
            let sub = open_dir(&path).map_err(|error| vanished(error, &name))?;
            let opened = Identity::of(&sub.metadata()?);
            if opened != Identity::of(&checked) {
                return Err(swapped(
                    &shown.join(&name),
                    "was exchanged for another directory",
                ));
            }
            remove_contents(&sub, &shown.join(&name))?;
            drop(sub);
            let again = fs::symlink_metadata(&path).map_err(|error| vanished(error, &name))?;
            if again.file_type().is_symlink() || Identity::of(&again) != opened {
                return Err(swapped(
                    &shown.join(&name),
                    "was replaced after it was emptied",
                ));
            }
            fs::remove_dir(&path).map_err(|error| vanished(error, &name))?;
        } else {
            let again = fs::symlink_metadata(&path).map_err(|error| vanished(error, &name))?;
            if again.is_dir() || Identity::of(&again) != Identity::of(&checked) {
                return Err(swapped(
                    &shown.join(&name),
                    "was exchanged for another entry",
                ));
            }
            fs::remove_file(&path).map_err(|error| vanished(error, &name))?;
        }
    }
    Ok(())
}

/// The proof's pause: announce `tag` under the pause directory, then
/// wait for the matching go-ahead. Bounded, so a forgotten variable
/// cannot hang a run forever. The first pause, after validation, waits
/// for `proceed`; every later one waits for `proceed-<tag>`.
fn pause_for_proof(tag: &str) {
    let Some(dir) = std::env::var_os(PAUSE_VAR).filter(|value| !value.is_empty()) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let _ = fs::write(dir.join(tag), b"");
    let proceed = if tag == "validated" {
        dir.join("proceed")
    } else {
        dir.join(format!("proceed-{tag}"))
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !proceed.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// The second seam: pause between the check of a child directory named
/// by [`PAUSE_BEFORE_OPEN_VAR`] and its open, so a test can exchange it
/// in the one window the identity comparison exists for.
fn pause_before_open(name: &std::ffi::OsStr) {
    if std::env::var_os(PAUSE_BEFORE_OPEN_VAR).is_some_and(|wanted| wanted == name) {
        pause_for_proof("checked");
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

    /// A receipt lacking its target, naming a relative target, or naming a
    /// non-canonical target refuses before anything opens, even where a
    /// repository sits below the candidate path.
    #[test]
    fn a_malformed_receipt_refuses_before_anything_is_removed() {
        let scratch = tempfile::tempdir().expect("a scratch dir exists");
        let canonical = std::fs::canonicalize(scratch.path()).expect("canonical");
        let candidate = canonical.join("candidate");
        std::fs::create_dir_all(candidate.join("repo/.git")).expect("creates");
        std::fs::write(candidate.join("repo/precious"), b"keep").expect("writes");
        let receipt = candidate.join("stage.json");
        for body in [
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}"}}"#,
                candidate.display()
            ),
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":null}}"#,
                candidate.display()
            ),
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":""}}"#,
                candidate.display()
            ),
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":"repo"}}"#,
                candidate.display()
            ),
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":"{}/../candidate/repo"}}"#,
                candidate.display(),
                candidate.display()
            ),
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":"{}/repo/"}}"#,
                candidate.display(),
                candidate.display()
            ),
        ] {
            std::fs::write(&receipt, &body).expect("writes");
            let error = validate(&candidate).expect_err("refuses");
            assert_eq!(reason_of(&error), Reason::DestructiveRefusal, "{body}");
            assert_eq!(
                std::fs::read(candidate.join("repo/precious")).expect("reads"),
                b"keep",
                "{body}"
            );
        }
        // A receipt whose target is a link to the real repository: its
        // canonical path differs from what it names.
        let link = canonical.join("link");
        std::os::unix::fs::symlink(candidate.join("repo"), &link).expect("links");
        std::fs::write(
            &receipt,
            format!(
                r#"{{"schema":"rk.stage/1","stage_root":"{}","target":"{}"}}"#,
                candidate.display(),
                link.display()
            ),
        )
        .expect("writes");
        assert_eq!(
            reason_of(&validate(&candidate).expect_err("refuses")),
            Reason::DestructiveRefusal
        );
        assert!(candidate.join("repo/.git").is_dir());
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
