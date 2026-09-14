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
//! named for the canonical target path, and what holds the target is the
//! advisory lock the operating system puts on the open file, not the
//! file's existence. The kernel owns that lock: it releases when the
//! holder exits, however it exits, so a run killed outright frees the
//! target rather than stranding it. The file itself is left in place,
//! because removing one another run has already opened would leave two
//! runs holding locks on two different inodes under one name.
//!
//! It lives outside the target because a target's cleanliness is judged
//! byte by byte, and a lock file inside it would be drift.
//!
//! Where the lock cannot be taken, the apply refuses. A guard that
//! silently does nothing is worse than none, because the call site still
//! reads as guarded.
//!
//! It is advisory, and it bounds this engine's own runs rather than
//! every writer: a hand edit during an apply is what the before-digests
//! and the postconditions are for.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Write};
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
/// The held file stays open for as long as this value lives, and the
/// operating system releases its lock when the file closes: on a drop,
/// on a refusal, on an error, and on a process that dies without
/// unwinding. A held lock always names its file, and the only
/// constructors either take the lock or refuse, so there is no such
/// thing as a lock that holds nothing.
#[derive(Debug)]
pub struct TargetLock {
    /// Held open, because closing it is what releases the lock. Dropped
    /// with this value.
    _file: File,
    path: PathBuf,
}

impl TargetLock {
    /// The lock file's path, while it is held.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
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
/// Returns a `target-busy` refusal where another live run holds the
/// target, a `prerequisite-unmet` refusal where no state root resolves,
/// and [`RkError::Io`] where the lock file cannot be opened.
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
    restrict_lock_dir(dir)?;
    let path = dir.join(format!("{}.lock", Digest::of(canonical.as_bytes())));
    // Opened rather than created exclusively: the file outlives every
    // run that took it, so its existence says a target was locked once,
    // never that it is locked now. Only the lock below says that.
    let mut file = open_lock_file(&path)?;
    match file.try_lock() {
        Ok(()) => {
            // Best effort: the body is for the operator reading a
            // refusal, and a lock that cannot be described still holds.
            // Truncated first, because the previous holder's line is
            // still there and a short write would leave its tail.
            let _ = file.set_len(0);
            let _ = writeln!(file, "{}", std::process::id());
            let _ = writeln!(file, "{canonical}");
            let _ = file.flush();
            Ok(TargetLock { _file: file, path })
        }
        Err(TryLockError::WouldBlock) => Err(busy(&canonical, &path)),
        Err(TryLockError::Error(error)) => Err(RkError::Io(error)),
    }
}

/// Make the lock namespace private and refuse a link or another file type.
fn restrict_lock_dir(dir: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_lock_entry(dir, "lock namespace is not a directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(dir)?;
        directory.set_permissions(std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Open the persistent lock entry without following its final component.
fn open_lock_file(path: &Path) -> io::Result<File> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(invalid_lock_entry(path, "lock entry is not a regular file"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    configure_lock_open(&mut options);
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid_lock_entry(path, "lock entry is not a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

#[cfg(unix)]
fn configure_lock_open(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt as _;

    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
}

#[cfg(windows)]
fn configure_lock_open(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt as _;

    // Win32 FILE_FLAG_OPEN_REPARSE_POINT makes CreateFileW open the link
    // itself, so the regular-file check below refuses it rather than opening
    // its destination.
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(not(any(unix, windows)))]
fn configure_lock_open(_options: &mut OpenOptions) {}

fn invalid_lock_entry(path: &Path, detail: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{detail}: {}", path.display()),
    )
}

/// The refusal for a target another run holds.
fn busy(target: &str, path: &Path) -> RkError {
    let holder = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.lines().next().map(str::to_owned))
        .filter(|line| !line.is_empty())
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
        .action("wait for that run to finish, then run it again")
        .target_state("unchanged"),
    )
}

#[cfg(test)]
mod tests {
    use super::{acquire_in, busy, rootless};
    use crate::diagnostic::Reason;
    use crate::digest::Digest;

    fn utf8(dir: &tempfile::TempDir) -> camino::Utf8PathBuf {
        camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a utf-8 path")
    }

    fn lock_path(locks: &tempfile::TempDir, target: &camino::Utf8Path) -> std::path::PathBuf {
        let canonical = std::fs::canonicalize(target).expect("the target canonicalizes");
        locks.path().join(format!(
            "{}.lock",
            Digest::of(canonical.display().to_string().as_bytes())
        ))
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

    /// A lock file a dead run left behind holds nothing, so the next
    /// apply takes the target rather than refusing until somebody
    /// removes the file by hand.
    ///
    /// The file is what a killed process leaves: the operating system
    /// released its lock when the process died, and the bytes stayed.
    #[test]
    fn a_lock_file_without_a_live_holder_is_taken_over() {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let target = tempfile::tempdir().expect("a scratch target exists");
        let path = utf8(&target);
        let held = acquire_in(locks.path(), &path)
            .expect("the first run takes the target")
            .path()
            .to_path_buf();

        // The holder gone the way a kill leaves it: the file and its
        // line survive, the lock does not.
        drop(acquire_in(locks.path(), &path));
        std::fs::write(&held, "4242\n/some/target\n").expect("the corpse's line writes");
        assert!(held.exists(), "a killed run leaves its lock file");

        let taken = acquire_in(locks.path(), &path).expect("the next run takes the target");
        assert_eq!(taken.path(), held, "it is the same lock file");
        let body = std::fs::read_to_string(&held).expect("the lock file reads");
        assert!(
            body.starts_with(&format!("{}\n", std::process::id())),
            "the taking run names itself, and no tail of the corpse survives: {body:?}"
        );
    }

    /// A lock pathname cannot redirect the holder description writes to
    /// another file.
    #[cfg(unix)]
    #[test]
    fn a_symlink_lock_file_is_refused_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let target = tempfile::tempdir().expect("a scratch target exists");
        let path = utf8(&target);
        let victim = locks.path().join("victim");
        std::fs::write(&victim, "untouched\n").expect("the victim exists");
        symlink(&victim, lock_path(&locks, &path)).expect("the crafted lock link exists");

        acquire_in(locks.path(), &path).expect_err("a lock link is refused");
        assert_eq!(
            std::fs::read_to_string(victim).expect("the victim reads"),
            "untouched\n",
            "acquisition must not truncate or write through the link"
        );
    }

    /// A persistent lock entry is a regular file, never another kind of
    /// filesystem object.
    #[test]
    fn a_non_regular_lock_entry_is_refused() {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let target = tempfile::tempdir().expect("a scratch target exists");
        let path = utf8(&target);
        let entry = lock_path(&locks, &path);
        std::fs::create_dir(&entry).expect("a non-regular entry exists");

        acquire_in(locks.path(), &path).expect_err("a non-regular lock is refused");
        assert!(entry.is_dir(), "the refused entry stays unchanged");
    }

    /// The lock namespace and its operator-readable entries belong only
    /// to the user running rk.
    #[cfg(unix)]
    #[test]
    fn the_lock_namespace_and_file_are_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;

        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let target = tempfile::tempdir().expect("a scratch target exists");
        let taken = acquire_in(locks.path(), &utf8(&target)).expect("the target is taken");

        let dir_mode = std::fs::metadata(locks.path())
            .expect("the namespace has metadata")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = std::fs::metadata(taken.path())
            .expect("the lock has metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }

    /// The namespace itself cannot redirect every target lock into
    /// another directory.
    #[cfg(unix)]
    #[test]
    fn a_symlink_lock_namespace_is_refused() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().expect("a scratch parent exists");
        let destination = tempfile::tempdir().expect("a scratch destination exists");
        let locks = parent.path().join("locks");
        symlink(destination.path(), &locks).expect("the crafted namespace link exists");
        let target = tempfile::tempdir().expect("a scratch target exists");

        acquire_in(&locks, &utf8(&target)).expect_err("a linked namespace is refused");
        assert_eq!(
            std::fs::read_dir(destination.path())
                .expect("the destination reads")
                .count(),
            0,
            "no lock is written through the namespace link"
        );
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

    /// The refusal names the holder, so an operator meeting it knows
    /// which run to wait for.
    #[test]
    fn the_refusal_names_the_holder() {
        let dir = tempfile::tempdir().expect("a scratch directory exists");
        let path = dir.path().join("held.lock");
        std::fs::write(&path, "4242\n/some/target\n").expect("the lock file is written");
        let diagnostic = busy("/some/target", &path).diagnostic();
        assert!(diagnostic.message.contains("4242"), "{diagnostic:?}");
        assert!(
            diagnostic.message.contains("/some/target"),
            "{diagnostic:?}"
        );
    }
}
