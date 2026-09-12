//! One target held by one apply at a time.
//!
//! An apply reads the target, decides against what it read, stages, and
//! renames. Two of them interleaved can each pass their own validation
//! and then commit over each other, leaving one plan's files beside
//! another's record: a target describing a landing that never happened.
//! The transaction cannot see that, because it renames files it staged
//! before the other run existed.
//!
//! So an apply takes the target first and holds it until its
//! postconditions have run. The lock is one file under the state root,
//! named for the canonical target path, created with `create_new` so the
//! creation is the acquisition, and removed when the guard drops. It
//! lives outside the target because a target's cleanliness is judged
//! byte by byte, and a lock file inside it would be drift.
//!
//! Where the lock cannot be taken, the apply refuses. A guard that
//! silently does nothing is worse than none, because the call site still
//! reads as guarded.
//!
//! It is advisory, and it bounds this engine's own runs rather than
//! every writer: a hand edit during an apply is what the before-digests
//! and the postconditions are for.

use std::io::Write;
use std::path::{Path, PathBuf};

use camino::Utf8Path;

use crate::applog;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;

/// The directory holding the locks, under the state root.
pub const LOCKS_DIR: &str = "locks";

/// One target held for the life of this value.
///
/// The file is removed on drop, on every path out: a refusal, an error,
/// or a clean apply. A process killed outright leaves the file behind,
/// and the refusal it causes names the file so the operator can remove
/// it.
/// A held lock always names its file: the only two constructors either
/// create one or refuse, so there is no such thing as a lock that holds
/// nothing.
#[derive(Debug)]
pub struct TargetLock {
    path: PathBuf,
}

impl TargetLock {
    /// The lock file's path, while it is held.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TargetLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Take `target` for this run, or refuse.
///
/// Exclusive ownership or a refusal, never a silent pass. Where no state
/// root resolves there is nowhere to put the lock, and an apply that
/// proceeds anyway is the unguarded apply this module exists to stop, so
/// it refuses before it stages. A second location is no answer either: a
/// run that locked elsewhere would not exclude a run that locked here,
/// and two processes disagreeing about where the lock lives hold no lock
/// at all.
///
/// # Errors
///
/// Returns a `target-busy` refusal where another run holds the target, a
/// `prerequisite-unmet` refusal where no state root resolves, and
/// [`RkError::Io`] where the lock cannot be written.
pub fn acquire(target: &Utf8Path) -> Result<TargetLock, RkError> {
    let Some(root) = applog::state_root() else {
        return Err(rootless());
    };
    acquire_in(&root.join(LOCKS_DIR), target)
}

/// The refusal for a host where the lock has nowhere to live.
fn rootless() -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            "no state root resolves, so this run cannot take the target, and nothing was written",
        )
        .expected("one apply against a target at a time, held by a lock under the state root")
        .action("set XDG_STATE_HOME, or HOME, and run it again")
        .target_state("unchanged"),
    )
}

/// The acquisition against one locks directory, which is what the tests
/// drive so no test has to move the state root out from under itself.
///
/// The target path is digested rather than flattened, so a path carrying
/// a separator cannot name another target's lock.
///
/// # Errors
///
/// As [`acquire`].
pub fn acquire_in(dir: &Path, target: &Utf8Path) -> Result<TargetLock, RkError> {
    let canonical = std::fs::canonicalize(target)
        .map_or_else(|_| target.to_string(), |path| path.display().to_string());
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.lock", Digest::of(canonical.as_bytes())));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            // Best effort: the body is for the operator reading a
            // refusal, and a lock that cannot be described still holds.
            let _ = writeln!(file, "{}", std::process::id());
            let _ = writeln!(file, "{canonical}");
            Ok(TargetLock { path })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(busy(&canonical, &path))
        }
        Err(error) => Err(RkError::Io(error)),
    }
}

/// The refusal for a target another run holds.
fn busy(target: &str, path: &Path) -> RkError {
    let holder = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.lines().next().map(str::to_owned))
        .map_or_else(
            || "another run".to_owned(),
            |pid| format!("the run at process {pid}"),
        );
    RkError::refusal(
        Diagnostic::new(
            Reason::TargetBusy,
            format!("{holder} holds {target}, and nothing was written"),
        )
        .expected("one apply against a target at a time")
        .action(format!(
            "wait for that run to finish; where it is gone, remove {}",
            path.display()
        ))
        .target_state("unchanged"),
    )
}

#[cfg(test)]
mod tests {
    use super::{acquire_in, busy, rootless};
    use crate::diagnostic::Reason;

    fn utf8(dir: &tempfile::TempDir) -> camino::Utf8PathBuf {
        camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a utf-8 path")
    }

    /// The second acquisition refuses while the first is held, and the
    /// drop frees the target for the next one.
    #[test]
    fn one_run_holds_a_target_at_a_time() {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let target = tempfile::tempdir().expect("a scratch target exists");
        let path = utf8(&target);
        let first = acquire_in(locks.path(), &path).expect("the first run takes the target");
        let second = acquire_in(locks.path(), &path);
        assert_eq!(
            second.expect_err("the second refuses").reason(),
            Reason::TargetBusy
        );
        drop(first);
        acquire_in(locks.path(), &path).expect("the target is free again");
    }

    /// Two targets are two locks, so one apply does not block another.
    #[test]
    fn two_targets_are_two_locks() {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let a = tempfile::tempdir().expect("a scratch target exists");
        let b = tempfile::tempdir().expect("a second scratch target exists");
        let _first = acquire_in(locks.path(), &utf8(&a)).expect("the first target is taken");
        acquire_in(locks.path(), &utf8(&b)).expect("the second target is free");
    }

    /// A host where no state root resolves refuses rather than applying
    /// unguarded, and names what the operator can set.
    #[test]
    fn a_host_with_no_state_root_refuses() {
        let error = rootless();
        assert_eq!(error.reason(), Reason::PrerequisiteUnmet);
        let diagnostic = error.diagnostic();
        assert!(
            diagnostic
                .action
                .unwrap_or_default()
                .contains("XDG_STATE_HOME"),
            "{:?}",
            diagnostic.message
        );
        assert_eq!(diagnostic.target_state.as_deref(), Some("unchanged"));
    }

    /// The refusal names the holder and the lock file, so an operator
    /// whose run died can clear it.
    #[test]
    fn the_refusal_names_the_lock_file() {
        let dir = tempfile::tempdir().expect("a scratch directory exists");
        let path = dir.path().join("held.lock");
        std::fs::write(&path, "4242\n/some/target\n").expect("the lock file is written");
        let diagnostic = busy("/some/target", &path).diagnostic();
        assert!(diagnostic.message.contains("4242"), "{diagnostic:?}");
        let action = diagnostic.action.unwrap_or_default();
        assert!(action.contains("held.lock"), "{action}");
    }
}
