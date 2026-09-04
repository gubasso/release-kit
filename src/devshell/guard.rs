//! The gates around the devshell transaction, so the `.envrc` line is
//! safe on every directory entry.
//!
//! Each gate answers one question before anything fetches or spawns:
//! whether the run is switched off, whether today's attempt already
//! happened, whether another shell holds this checkout, and whether the
//! two files carry uncommitted edits. The lock and the stamp live under
//! the state root, keyed per checkout — not in the target, where an
//! untracked file is noise, and not in `.git/`, which belongs to git.

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use camino::Utf8Path;

use super::{lock_path, stamp_path};
use crate::maintenance::GIT_HOOK_VARS;
use crate::probes::git_bin;

/// A lock whose owner procfs cannot judge is taken over after this long.
const LOCK_GRACE: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// The variables a CI runner exports; any of them switches the sync off.
pub const CI_VARS: [&str; 6] = [
    "CI",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "BUILDKITE",
    "CIRCLECI",
    "TF_BUILD",
];

/// The operator's own off switch, read from the environment `.envrc.local`
/// exports.
pub const SWITCH_VAR: &str = "RK_DEVSHELL_SYNC";

/// Whether the operator switched the sync off with `RK_DEVSHELL_SYNC=0`.
#[must_use]
pub fn switched_off() -> bool {
    std::env::var(SWITCH_VAR).is_ok_and(|value| value.trim() == "0")
}

/// Whether a CI variable is set to anything but an explicit no.
#[must_use]
pub fn in_ci() -> bool {
    CI_VARS.iter().any(|var| {
        std::env::var(var).is_ok_and(|value| {
            let value = value.trim().to_ascii_lowercase();
            !(value.is_empty() || value == "0" || value == "false")
        })
    })
}

/// Today, as the first ten characters of the UTC clock.
#[must_use]
pub fn today() -> String {
    crate::applog::now_utc()[..10].to_owned()
}

/// Stamp today's attempt for one checkout, before the attempt.
///
/// # Errors
///
/// Returns the I/O failure of the write.
pub fn write_stamp(key: &str) -> std::io::Result<()> {
    let Some(path) = stamp_path(key) else {
        return Err(std::io::Error::other("no state root for the stamp"));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::atomic::write(&path, format!("{}\n", today()).as_bytes())
}

/// The held single-writer lock; removed on drop.
#[derive(Debug)]
pub struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// What acquisition found.
#[derive(Debug)]
pub enum Acquired {
    /// This run holds the checkout.
    Held(Lock),
    /// Another live run holds it: normal, and skipped in silence.
    Contended,
    /// The lock cannot be taken at all: the mechanism itself is broken.
    Unavailable(std::io::Error),
}

/// Take the checkout's lock, atomically through `O_EXCL`. A lock whose
/// owner is provably gone — or, where procfs cannot judge, older than
/// the grace period — is removed and the take retried once.
#[must_use]
pub fn acquire(key: &str) -> Acquired {
    let Some(path) = lock_path(key) else {
        return Acquired::Unavailable(std::io::Error::other(
            "neither XDG_STATE_HOME nor HOME is set, so the lock has no root",
        ));
    };
    if let Some(parent) = path.parent() {
        if let Err(source) = fs::create_dir_all(parent) {
            return Acquired::Unavailable(source);
        }
    }
    match take(&path) {
        Ok(lock) => Acquired::Held(lock),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let pid = fs::read_to_string(&path).ok().and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("pid=")?.trim().parse::<u64>().ok())
            });
            if !super::txn::owner_gone_after(pid, &path, LOCK_GRACE) {
                return Acquired::Contended;
            }
            let _ = fs::remove_file(&path);
            match take(&path) {
                Ok(lock) => Acquired::Held(lock),
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                    Acquired::Contended
                }
                Err(source) => Acquired::Unavailable(source),
            }
        }
        Err(source) => Acquired::Unavailable(source),
    }
}

/// One exclusive create, holding the owner's process id and start time.
fn take(path: &std::path::Path) -> std::io::Result<Lock> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(format!("pid={}\n", std::process::id()).as_bytes())?;
    file.write_all(format!("started={}\n", crate::applog::now_utc()).as_bytes())?;
    Ok(Lock(path.to_path_buf()))
}

/// Whether `flake.nix` or `flake.lock` carries uncommitted edits.
///
/// Judged by the target repository with the hook variables scrubbed, so
/// a run from inside a git hook judges the target and not the hook's own
/// repository. Fails closed: a git that did not run counts as dirty.
#[must_use]
pub fn two_files_dirty(target: &Utf8Path) -> bool {
    let mut command = std::process::Command::new(git_bin());
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .arg("-C")
        .arg(target.as_std_path())
        .args(["status", "--porcelain", "--", "flake.nix", "flake.lock"])
        .output()
        .map_or(true, |probed| {
            !probed.status.success() || !probed.stdout.is_empty()
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Acquired, take};

    #[test]
    fn the_lock_is_exclusive_and_released_on_drop() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let path = dir.path().join("k.lock");
        let held = take(&path).expect("the first take holds");
        assert!(path.exists());
        let second = take(&path).expect_err("the second take refuses");
        assert_eq!(second.kind(), std::io::ErrorKind::AlreadyExists);
        drop(held);
        assert!(!path.exists(), "the lock is removed on drop");
        let again = take(&path).expect("the lock is free again");
        assert!(
            std::fs::read_to_string(&path)
                .expect("reads")
                .starts_with("pid="),
            "the lock names its owner"
        );
        drop(again);
    }

    #[test]
    fn the_acquired_vocabulary_is_three_states() {
        let unavailable = Acquired::Unavailable(std::io::Error::other("x"));
        assert!(matches!(unavailable, Acquired::Unavailable(_)));
        assert!(matches!(Acquired::Contended, Acquired::Contended));
    }
}
