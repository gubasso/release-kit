//! One target held by one landing at a time.
//!
//! A landing reads the target, decides against what it read, and writes.
//! Two of them interleaved can each pass their own validation and then
//! write over each other, leaving one landing's files beside another's
//! receipt: a target describing a landing that never happened.
//!
//! So a landing takes the target before its final evidence gathering and
//! holds it through the receipt write. The lock is one file under the
//! state root, named for the canonical target path, and what holds the
//! target is the advisory lock the operating system puts on the open
//! file, not the file's existence. The kernel owns that lock: it releases
//! when the holder exits, however it exits, so a run killed outright
//! frees the target rather than stranding it. The file itself is left in
//! place, because removing one another run has already opened would leave
//! two runs holding locks on two different inodes under one name.
//!
//! It lives outside the target because a target's cleanliness is judged
//! byte by byte, and a lock file inside it would be drift.
//!
//! Where the lock cannot be taken, the landing refuses. A guard that
//! silently does nothing is worse than none, because the call site still
//! reads as guarded. It is advisory, and it bounds this binary's own
//! runs rather than every writer.
//!
//! SATISFIES landing:a-partial-landing-is-visible-and-rerunnable

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
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
/// unwinding. The only constructors either take the lock or refuse, so
/// there is no such thing as a lock that holds nothing.
#[derive(Debug)]
pub struct TargetLock {
    /// Held open, because closing it is what releases the lock. Dropped
    /// with this value.
    _file: File,
    path: PathBuf,
    /// The identity of the directory the lock key was derived from, so
    /// the directory later held can be checked to be the same one.
    identity: crate::held::Identity,
}

impl TargetLock {
    /// The identity of the directory this lock was taken for.
    #[must_use]
    pub const fn identity(&self) -> crate::held::Identity {
        self.identity
    }

    /// The lock file's path, while it is held.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Take `target` for this run, or refuse.
///
/// Exclusive ownership or a refusal, never a silent pass. Where no state
/// root resolves there is nowhere to put the lock, and a landing that
/// proceeds anyway is the unguarded landing this module exists to stop,
/// so it refuses before it writes.
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
        .expected("one landing against a target at a time, held by a lock under the state root")
        .action("set XDG_STATE_HOME, or HOME, and run it again")
        .target_state("unchanged"),
    )
}

/// The acquisition against one locks directory, which is what the tests
/// drive so no test has to move the state root out from under itself.
///
/// The target must exist and resolve: the lock is keyed by the canonical
/// path and carries the directory's identity, so a target that cannot be
/// resolved has no lock to take and refuses, rather than being keyed by
/// the text it was named with while another run keys the same directory
/// by its canonical path. The canonical path is digested rather than
/// flattened, so a path carrying a separator cannot name another target's
/// lock.
///
/// # Errors
///
/// As [`acquire`], and [`RkError::Missing`] naming a target that does not
/// resolve to a directory.
pub fn acquire_in(dir: &Path, target: &Utf8Path) -> Result<TargetLock, RkError> {
    let resolved = std::fs::canonicalize(target)
        .and_then(|path| std::fs::metadata(&path).map(|metadata| (path, metadata)))
        .map_err(|error| {
            RkError::missing(
                Diagnostic::new(
                    Reason::TargetNotFound,
                    format!("target {target} does not resolve to a directory to lock: {error}"),
                )
                .expected("an existing directory to land into")
                .target_state("unchanged"),
            )
        })?;
    let (resolved, metadata) = resolved;
    if !metadata.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory, and nothing was written"),
            )
            .expected("an existing directory to land into")
            .target_state("unchanged"),
        ));
    }
    let identity = crate::held::Identity::of(&metadata);
    let canonical = resolved.display().to_string();
    std::fs::create_dir_all(dir)?;
    restrict_lock_dir(dir)?;
    let path = dir.join(format!("{}.lock", Digest::of(canonical.as_bytes())));
    // Opened rather than created exclusively: the file outlives every
    // run that took it, so its existence says a target was locked once,
    // never that it is locked now. Only the lock below says that.
    let file = open_lock_file(&path)?;
    match file.try_lock() {
        Ok(()) => {
            // The file carries no holder description: what holds the
            // target is the kernel's lock, and an empty regular file is
            // all the entry needs to be. Truncated only after the open
            // verified a regular file, so a crafted entry loses nothing.
            let _ = file.set_len(0);
            Ok(TargetLock {
                _file: file,
                path,
                identity,
            })
        }
        Err(TryLockError::WouldBlock) => Err(busy(&canonical)),
        Err(TryLockError::Error(error)) => Err(RkError::Io(error)),
    }
}

/// Make the lock namespace private and refuse a link or another file type.
fn restrict_lock_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    let metadata = std::fs::symlink_metadata(dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_lock_entry(dir, "lock namespace is not a directory"));
    }
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir)?;
    directory.set_permissions(std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// Open the persistent lock entry without following its final component,
/// owner-only, and verify through the opened descriptor that a regular
/// file is what was opened before anything truncates it.
fn open_lock_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(invalid_lock_entry(path, "lock entry is not a regular file"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid_lock_entry(path, "lock entry is not a regular file"));
    }
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

fn invalid_lock_entry(path: &Path, detail: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{detail}: {}", path.display()),
    )
}

/// The refusal for a target another run holds.
fn busy(target: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::TargetBusy,
            format!("another run holds {target}, and nothing was written"),
        )
        .expected("one landing against a target at a time")
        .action("wait for that run to finish, then run it again")
        .target_state("unchanged"),
    )
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::{acquire_in, rootless};
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
        let refused = second.expect_err("the second refuses");
        assert_eq!(refused.reason(), Reason::TargetBusy);
        assert_eq!(
            refused.diagnostic().target_state.as_deref(),
            Some("unchanged")
        );
        drop(first);
        acquire_in(locks.path(), &path).expect("the target is free again");
    }

    /// Two targets are two locks, so one landing does not block another.
    #[test]
    fn two_targets_are_two_locks() {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let a = tempfile::tempdir().expect("a scratch target exists");
        let b = tempfile::tempdir().expect("a second scratch target exists");
        let _first = acquire_in(locks.path(), &utf8(&a)).expect("the first target is taken");
        acquire_in(locks.path(), &utf8(&b)).expect("the second target is free");
    }

    /// A lock file a dead run left behind holds nothing, so the next
    /// landing takes the target rather than refusing until somebody
    /// removes the file by hand, and whatever bytes the corpse left are
    /// gone once the entry is verified and taken.
    #[test]
    fn a_lock_file_without_a_live_holder_is_taken_over() {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let target = tempfile::tempdir().expect("a scratch target exists");
        let path = utf8(&target);
        let held = acquire_in(locks.path(), &path)
            .expect("the first run takes the target")
            .path()
            .to_path_buf();
        drop(acquire_in(locks.path(), &path));
        std::fs::write(&held, "4242\n/some/target\n").expect("the corpse's line writes");
        assert!(held.exists(), "a killed run leaves its lock file");

        let taken = acquire_in(locks.path(), &path).expect("the next run takes the target");
        assert_eq!(taken.path(), held, "it is the same lock file");
        assert_eq!(
            std::fs::read_to_string(&held).expect("the lock file reads"),
            "",
            "the entry carries no holder description"
        );
    }

    /// A link, a directory, or another non-regular lock entry is refused
    /// without changing what it names, and the namespace and the file
    /// keep their owner-only permissions.
    ///
    /// SATISFIES landing:a-partial-landing-is-visible-and-rerunnable
    #[test]
    fn a_non_regular_lock_entry_is_refused_and_the_lock_stays_owner_only() {
        use std::os::unix::fs::symlink;

        // A link at the entry: the victim it names is neither truncated
        // nor written.
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let target = tempfile::tempdir().expect("a scratch target exists");
        let path = utf8(&target);
        let victim = locks.path().join("victim");
        std::fs::write(&victim, "untouched\n").expect("the victim exists");
        symlink(&victim, lock_path(&locks, &path)).expect("the crafted lock link exists");
        acquire_in(locks.path(), &path).expect_err("a lock link is refused");
        assert_eq!(
            std::fs::read_to_string(&victim).expect("the victim reads"),
            "untouched\n"
        );

        // A directory at the entry stays a directory.
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let entry = lock_path(&locks, &path);
        std::fs::create_dir(&entry).expect("a non-regular entry exists");
        acquire_in(locks.path(), &path).expect_err("a non-regular lock is refused");
        assert!(entry.is_dir(), "the refused entry stays unchanged");

        // A linked namespace redirects no lock.
        let parent = tempfile::tempdir().expect("a scratch parent exists");
        let destination = tempfile::tempdir().expect("a scratch destination exists");
        let linked = parent.path().join("locks");
        symlink(destination.path(), &linked).expect("the crafted namespace link exists");
        acquire_in(&linked, &path).expect_err("a linked namespace is refused");
        assert_eq!(
            std::fs::read_dir(destination.path())
                .expect("the destination reads")
                .count(),
            0
        );

        // The namespace and the entry belong to the user running rk.
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let taken = acquire_in(locks.path(), &path).expect("the target is taken");
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

    /// A host where no state root resolves refuses rather than landing
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
}
