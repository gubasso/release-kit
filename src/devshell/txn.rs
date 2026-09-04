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
    let record = serde_json::json!({
        "target": target.as_str(),
        "pid": std::process::id(),
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
    #[must_use]
    pub fn abort(mut self) -> Vec<String> {
        self.committed = true;
        let restored = restore(&self.target, &self.backup);
        clear(&self.backup, &self.marker);
        restored
    }
}

impl Drop for Txn {
    fn drop(&mut self) {
        if !self.committed {
            let _ = restore(&self.target, &self.backup);
            clear(&self.backup, &self.marker);
        }
    }
}

/// Restore both files from the backup: a backed-up file is copied back,
/// and a file with no backup — a lock that did not exist before — is
/// removed. The names touched, in order.
fn restore(target: &Path, backup: &Path) -> Vec<String> {
    let mut restored = Vec::new();
    for name in FILES {
        let copy = backup.join(name);
        let destination = target.join(name);
        let outcome = if copy.exists() {
            fs::read(&copy).and_then(|bytes| crate::atomic::write(&destination, &bytes))
        } else if destination.exists() {
            fs::remove_file(&destination)
        } else {
            continue;
        };
        if outcome.is_ok() {
            restored.push(name.to_owned());
        }
    }
    restored
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

/// Recover a transaction an earlier run left open, where its owner is
/// provably gone: restore both files and clear the marker. `Ok(None)`
/// where no marker exists or the owner may still be running.
///
/// # Errors
///
/// Returns [`RkError::Io`] where the marker exists and does not read.
pub fn recover_pending(target: &Utf8Path, key: &str) -> Result<Option<Vec<String>>, RkError> {
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
) -> Result<Option<Vec<String>>, RkError> {
    if !marker.exists() {
        return Ok(None);
    }
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(marker)?).unwrap_or(serde_json::Value::Null);
    let pid = record["pid"].as_u64();
    if !owner_gone(pid, marker) {
        return Ok(None);
    }
    let restored = restore(target.as_std_path(), backup);
    clear(backup, marker);
    Ok(Some(restored))
}

/// Whether a recorded owner is provably gone. Only a readable procfs
/// answer decides; where it cannot, a marker past the grace period is
/// treated as abandoned.
pub(crate) fn owner_gone(pid: Option<u64>, marker: &Path) -> bool {
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
        .is_some_and(|age| age > PENDING_GRACE)
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

    use super::{open_at, owner_gone, recover_at};

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
        assert_eq!(restored, Some(vec!["flake.nix".to_owned()]));
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
