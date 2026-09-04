//! The fenced two-file transaction behind `rk devshell sync`.
//!
//! The tag in `flake.nix` and the `release-kit` node in `flake.lock`
//! move together or not at all. Before the first write both files are
//! copied under the state root and a marker names the target and this
//! process; a build inside the transaction is the fence, so a pin that
//! does not build against the consumer's own nixpkgs never reaches the
//! tree. Any failure restores both files through the `Drop` guard, which
//! covers every `?`, every early return, and a panic. The crate forbids
//! `unsafe`, so no signal handler exists: a terminal interrupt during
//! the build leaves the marker, and the next run recovers from it.

use std::fs;
use std::path::{Path, PathBuf};

use camino::Utf8Path;

use super::{backup_dir, marker_path};
use crate::error::RkError;
use crate::maintenance::{GIT_HOOK_VARS, last_line};
use crate::probes::nix_bin;

/// The two files the transaction guards, in restore order.
const FILES: [&str; 2] = ["flake.nix", "flake.lock"];

/// A marker older than this whose owner procfs cannot judge is recovered
/// anyway: a build does not take a day.
const PENDING_GRACE: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// One failed step: which, and the child's last stderr line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepFailure {
    /// `flake-update`, `current-system`, or `build`.
    pub step: &'static str,
    /// The child's last non-empty stderr line, or why it did not run.
    pub detail: String,
}

/// An open transaction: the backups exist and the marker names this
/// process. Dropping it without `commit` restores both files.
#[derive(Debug)]
pub struct Txn {
    target: PathBuf,
    backup: PathBuf,
    marker: PathBuf,
    committed: bool,
}

/// Open a transaction over the target's two files.
///
/// # Errors
///
/// Returns [`RkError::Io`] where the state root is unknown, the backup
/// cannot be written, or `flake.nix` cannot be copied.
pub fn open(target: &Utf8Path, key: &str) -> Result<Txn, RkError> {
    let (Some(backup), Some(marker)) = (backup_dir(key), marker_path(key)) else {
        return Err(RkError::Io(std::io::Error::other(
            "neither XDG_STATE_HOME nor HOME is set, so the transaction has no backup root",
        )));
    };
    open_at(target, backup, marker)
}

/// [`open`] with the state paths named, which is what the unit tests
/// use in place of the environment.
fn open_at(target: &Utf8Path, backup: PathBuf, marker: PathBuf) -> Result<Txn, RkError> {
    fs::create_dir_all(&backup)?;
    for name in FILES {
        let source = target.join(name);
        let copy = backup.join(name);
        if source.exists() {
            fs::copy(&source, &copy)?;
        } else if copy.exists() {
            fs::remove_file(&copy)?;
        }
    }
    // The marker records which files existed, so a restore never reads a
    // missing backup as "the file was absent" and deletes a real file.
    let record = serde_json::json!({
        "target": target.as_str(),
        "pid": std::process::id(),
        "present": {
            FILES[0]: target.join(FILES[0]).exists(),
            FILES[1]: target.join(FILES[1]).exists(),
        },
    });
    crate::atomic::write(&marker, record.to_string().as_bytes())?;
    Ok(Txn {
        target: target.as_std_path().to_path_buf(),
        backup,
        marker,
        committed: false,
    })
}

impl Txn {
    /// Keep the new contents: clear the backup and the marker.
    pub fn commit(mut self) {
        self.committed = true;
        clear(&self.backup, &self.marker);
    }

    /// Put both files back and clear the marker; the names restored.
    ///
    /// # Errors
    ///
    /// Where a file cannot be put back, the marker and the backups stay
    /// for the next run's recovery, and the error names the file.
    pub fn abort(mut self) -> Result<Vec<String>, RestoreFailure> {
        self.committed = true;
        let restored = restore(&self.target, &self.backup, &self.marker)?;
        clear(&self.backup, &self.marker);
        Ok(restored)
    }
}

impl Drop for Txn {
    fn drop(&mut self) {
        if !self.committed {
            // A restore that fails keeps its material: the marker stays
            // for the next run, which is the one recovery left.
            if restore(&self.target, &self.backup, &self.marker).is_ok() {
                clear(&self.backup, &self.marker);
            }
        }
    }
}

/// A restore that could not put a file back; the backups stay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreFailure {
    /// The file that is not back.
    pub file: &'static str,
    /// Why, one line.
    pub detail: String,
}

impl std::fmt::Display for RestoreFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} could not be restored: {}", self.file, self.detail)
    }
}

/// Restore both files from the backup. A backed-up file is copied back;
/// a file the marker records as absent is removed; a file the marker
/// records as present with no backup to read is corruption, and the
/// restore stops there with the material kept. The names touched.
fn restore(target: &Path, backup: &Path, marker: &Path) -> Result<Vec<String>, RestoreFailure> {
    let record: serde_json::Value = fs::read(marker)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(serde_json::Value::Null);
    let mut restored = Vec::new();
    for name in FILES {
        let copy = backup.join(name);
        let destination = target.join(name);
        let was_present = record["present"][name].as_bool();
        let outcome = if copy.exists() {
            fs::read(&copy).and_then(|bytes| crate::atomic::write(&destination, &bytes))
        } else {
            match was_present {
                Some(false) if destination.exists() => fs::remove_file(&destination),
                Some(true) => Err(std::io::Error::other(
                    "the backup is missing although the file existed before the run",
                )),
                // No marker knowledge and no backup: leave the file alone.
                _ => continue,
            }
        };
        match outcome {
            Ok(()) => restored.push(name.to_owned()),
            Err(source) => {
                return Err(RestoreFailure {
                    file: name,
                    detail: source.to_string(),
                });
            }
        }
    }
    Ok(restored)
}

/// Remove the backup directory and the marker, then the per-checkout
/// directory they shared once it is empty, best effort.
fn clear(backup: &Path, marker: &Path) {
    let _ = fs::remove_file(marker);
    let _ = fs::remove_dir_all(backup);
    if let Some(parent) = marker.parent() {
        let _ = fs::remove_dir(parent);
    }
}

/// What a recovery attempt found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    /// Both files are back and the marker is cleared; the names restored.
    Restored(Vec<String>),
    /// A file could not be put back; the marker and backups stay.
    Failed(RestoreFailure),
}

/// Recover a transaction an earlier run left open, where its owner is
/// provably gone.
///
/// `Ok(None)` where no marker exists or the owner may still be running.
/// The caller holds the checkout's lock, so two runs never recover the
/// same marker at once.
///
/// # Errors
///
/// Returns [`RkError::Io`] where the marker exists and does not read.
pub fn recover_pending(target: &Utf8Path, key: &str) -> Result<Option<Recovery>, RkError> {
    let (Some(backup), Some(marker)) = (backup_dir(key), marker_path(key)) else {
        return Ok(None);
    };
    recover_at(target, &backup, &marker)
}

/// [`recover_pending`] with the state paths named.
fn recover_at(
    target: &Utf8Path,
    backup: &Path,
    marker: &Path,
) -> Result<Option<Recovery>, RkError> {
    if !marker.exists() {
        return Ok(None);
    }
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(marker)?).unwrap_or(serde_json::Value::Null);
    let pid = record["pid"].as_u64();
    if !owner_gone(pid, marker) {
        return Ok(None);
    }
    match restore(target.as_std_path(), backup, marker) {
        Ok(restored) => {
            clear(backup, marker);
            Ok(Some(Recovery::Restored(restored)))
        }
        Err(failure) => Ok(Some(Recovery::Failed(failure))),
    }
}

/// Whether a recorded owner is provably gone. Only a readable procfs
/// answer decides; where it cannot, a marker past the grace period is
/// treated as abandoned.
pub(crate) fn owner_gone(pid: Option<u64>, marker: &Path) -> bool {
    owner_gone_after(pid, marker, PENDING_GRACE)
}

/// [`owner_gone`] with the grace period named, for the lock's shorter one.
pub(crate) fn owner_gone_after(
    pid: Option<u64>,
    marker: &Path,
    grace: std::time::Duration,
) -> bool {
    if let Some(pid) = pid {
        if Path::new("/proc/self").is_dir() {
            match Path::new(&format!("/proc/{pid}")).try_exists() {
                Ok(true) => return false,
                Ok(false) => return true,
                Err(_) => {}
            }
        }
    }
    fs::metadata(marker)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > grace)
}

/// `nix flake update release-kit`, refreshing the one node.
///
/// # Errors
///
/// The failing step with nix's last stderr line.
pub fn flake_update(target: &Utf8Path) -> Result<(), StepFailure> {
    nix(target, "flake-update", &["flake", "update", "release-kit"]).map(|_| ())
}

/// The concrete system attribute this host builds for, from nix itself:
/// guessing it from the compile target would be wrong on the systems
/// that matter.
///
/// # Errors
///
/// The failing step with nix's last stderr line.
pub fn current_system(target: &Utf8Path) -> Result<String, StepFailure> {
    let output = nix(
        target,
        "current-system",
        &[
            "eval",
            "--raw",
            "--impure",
            "--expr",
            "builtins.currentSystem",
        ],
    )?;
    let system = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if system.is_empty() {
        return Err(StepFailure {
            step: "current-system",
            detail: "nix eval answered no system".to_owned(),
        });
    }
    Ok(system)
}

/// `nix build --no-link` of the default devshell: the fence. `--no-link`
/// keeps a directory entry from dropping a `result` symlink.
///
/// # Errors
///
/// The failing step with nix's last stderr line.
pub fn build_devshell(target: &Utf8Path, system: &str) -> Result<(), StepFailure> {
    let attribute = format!(".#devShells.{system}.default");
    nix(target, "build", &["build", "--no-link", &attribute]).map(|_| ())
}

/// One nix launch against the target, through the shared resolver, with
/// the git hook variables scrubbed like every other child this crate
/// spawns against a named target.
fn nix(
    target: &Utf8Path,
    step: &'static str,
    args: &[&str],
) -> Result<std::process::Output, StepFailure> {
    let mut command = std::process::Command::new(nix_bin());
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let output = command
        .args(args)
        .current_dir(target.as_std_path())
        .output()
        .map_err(|source| StepFailure {
            step,
            detail: format!("nix did not run: {source}"),
        })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(StepFailure {
            step,
            detail: last_line(&output.stderr),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use camino::Utf8PathBuf;

    use super::{Recovery, open_at, owner_gone, recover_at};

    fn scratch() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8");
        (dir, path)
    }

    /// The `Drop` guard restores; a commit keeps.
    #[test]
    fn a_dropped_transaction_restores_and_a_committed_one_keeps() {
        let (_state, state) = scratch();
        let (_target, target) = scratch();
        let backup = state.join("backup").into_std_path_buf();
        let marker = state.join("pending.json").into_std_path_buf();
        let lock = target.join("flake.lock");
        std::fs::write(target.join("flake.nix"), "old\n").expect("writes");
        let txn = open_at(&target, backup.clone(), marker.clone()).expect("opens");
        assert!(marker.exists(), "an open transaction leaves its marker");
        std::fs::write(target.join("flake.nix"), "new\n").expect("writes");
        std::fs::write(&lock, "{}\n").expect("writes");
        drop(txn);
        assert_eq!(
            std::fs::read_to_string(target.join("flake.nix")).expect("reads"),
            "old\n"
        );
        assert!(
            !lock.exists(),
            "a lock that did not exist before is removed"
        );
        assert!(!marker.exists());
        let txn = open_at(&target, backup.clone(), marker.clone()).expect("opens");
        std::fs::write(target.join("flake.nix"), "new\n").expect("writes");
        txn.commit();
        assert_eq!(
            std::fs::read_to_string(target.join("flake.nix")).expect("reads"),
            "new\n"
        );
        assert!(!backup.exists(), "a committed transaction leaves no backup");
        assert!(!marker.exists());
    }

    #[test]
    fn a_marker_with_a_dead_owner_is_recovered() {
        let (_state, state) = scratch();
        let (_target, target) = scratch();
        let backup = state.join("backup").into_std_path_buf();
        let marker = state.join("pending.json").into_std_path_buf();
        std::fs::write(target.join("flake.nix"), "old\n").expect("writes");
        let txn = open_at(&target, backup.clone(), marker.clone()).expect("opens");
        std::fs::write(target.join("flake.nix"), "half\n").expect("writes");
        // Forget the guard: stand in for a killed process.
        std::mem::forget(txn);
        std::fs::write(&marker, r#"{"target":"t","pid":4294967295}"#).expect("writes");
        let restored = recover_at(&target, &backup, &marker).expect("recovers");
        assert_eq!(
            restored,
            Some(Recovery::Restored(vec!["flake.nix".to_owned()]))
        );
        assert!(!marker.exists());
        assert_eq!(
            recover_at(&target, &backup, &marker).expect("recovers"),
            None
        );
        assert_eq!(
            std::fs::read_to_string(target.join("flake.nix")).expect("reads"),
            "old\n"
        );
    }

    /// A marker that records a file as present with no backup to read is
    /// corruption — never a file to delete — so a second recovery racing
    /// the first cannot remove what the first just put back.
    #[test]
    fn a_missing_backup_for_a_present_file_deletes_nothing() {
        let (_state, state) = scratch();
        let (_target, target) = scratch();
        let backup = state.join("backup").into_std_path_buf();
        let marker = state.join("pending.json").into_std_path_buf();
        std::fs::write(target.join("flake.nix"), "kept\n").expect("writes");
        std::fs::create_dir_all(&backup).expect("creates");
        std::fs::write(
            &marker,
            r#"{"pid":4294967295,"present":{"flake.nix":true,"flake.lock":false}}"#,
        )
        .expect("writes");
        let outcome = recover_at(&target, &backup, &marker).expect("judges");
        assert!(
            matches!(outcome, Some(Recovery::Failed(ref failure)) if failure.file == "flake.nix"),
            "{outcome:?}"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("flake.nix")).expect("reads"),
            "kept\n"
        );
        assert!(marker.exists(), "the marker stays for a later recovery");
    }

    /// A restore that cannot write keeps its backups and its marker, and
    /// says which file is not back.
    #[test]
    fn a_failed_restore_keeps_its_material() {
        let (_state, state) = scratch();
        let (_target, target) = scratch();
        let backup = state.join("backup").into_std_path_buf();
        let marker = state.join("pending.json").into_std_path_buf();
        std::fs::write(target.join("flake.nix"), "old\n").expect("writes");
        let txn = open_at(&target, backup.clone(), marker.clone()).expect("opens");
        // A directory where the file must go back: the rename fails.
        std::fs::remove_file(target.join("flake.nix")).expect("removes");
        std::fs::create_dir(target.join("flake.nix")).expect("blocks");
        let failure = txn.abort().expect_err("the restore fails");
        assert_eq!(failure.file, "flake.nix");
        assert!(marker.exists(), "the marker stays");
        assert!(backup.join("flake.nix").exists(), "the backup stays");
    }

    #[test]
    fn a_live_owner_is_left_alone() {
        let (_dir, dir) = scratch();
        let marker = dir.join("pending.json");
        std::fs::write(&marker, "{}").expect("writes");
        if std::path::Path::new("/proc/self").is_dir() {
            assert!(!owner_gone(
                Some(u64::from(std::process::id())),
                marker.as_std_path()
            ));
            assert!(owner_gone(Some(4_294_967_295), marker.as_std_path()));
        }
        assert!(
            !owner_gone(None, marker.as_std_path()),
            "a fresh marker with no pid waits"
        );
    }
}
