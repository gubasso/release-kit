//! Saved copies: what a destructive step keeps before it changes
//! anything.
//!
//! A landing that replaces a conflict the operator named saves the bytes
//! it overwrites, and a setup step that removes or replaces forge state
//! saves the state it found. Both go to one directory per run under the
//! state root, `snapshots/<repo>/<timestamp>/`, or `<timestamp>-<n>/` for
//! a second run in the same second, with `replaced/<path>` for files,
//! `forge/<step>.json` for forge state, and `refs/` for branch bundles.
//! A saved copy is a backup, never run state: no later run reads it to
//! decide anything, and only the operator removes it. It never lives
//! inside a stage, so cleaning a stage never deletes one.
//!
//! SATISFIES landing:ownership-is-elementary

use std::fs;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use crate::applog;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;

/// The directory below the state root that holds every snapshot.
pub const SNAPSHOTS_DIR: &str = "snapshots";

/// The directory below one snapshot that holds replaced files.
pub const REPLACED_DIR: &str = "replaced";

/// The directory below one snapshot that holds forge state.
pub const FORGE_DIR: &str = "forge";

/// The directory below one snapshot that holds branch bundles.
pub const REFS_DIR: &str = "refs";

/// The key naming one target below the snapshot base.
///
/// It is the target's directory name and the first twelve hex digits of
/// the digest of its canonical path, so a person can read it and two
/// targets with one name stay apart.
#[must_use]
pub fn repo_key(canonical_target: &Path) -> String {
    let digest = Digest::of(canonical_target.display().to_string().as_bytes()).to_string();
    let leaf = canonical_target.file_name().map_or_else(
        || "root".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!("{leaf}-{}", &digest[..12])
}

/// The current instant as `YYYYMMDDTHHMMSSZ`, a name every filesystem
/// takes and that sorts by time.
#[must_use]
pub fn timestamp() -> String {
    applog::now_utc()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// Claim a directory of this run's own under `runs`, named by `stamp`.
///
/// The directory is created exclusively and owner-only, so two runs in one
/// second never share one. Where `stamp` is taken, the next free name
/// follows it, `<stamp>-2`, `<stamp>-3`, so the names still sort by time.
///
/// # Errors
///
/// Any I/O error creating `runs` or the run directory, other than the name
/// being taken.
pub fn claim(runs: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(runs)?;
    let mut counter = 1_u32;
    loop {
        let name = if counter == 1 {
            stamp.to_owned()
        } else {
            format!("{stamp}-{counter}")
        };
        let candidate = runs.join(name);
        match fs::DirBuilder::new().mode(0o700).create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => counter += 1,
            Err(error) => return Err(error),
        }
    }
}

/// One run's snapshot directory, claimed when the run first needs it.
#[derive(Debug, Clone)]
pub struct Snapshot {
    root: PathBuf,
}

impl Snapshot {
    /// Claim the snapshot directory for one run against `canonical_target`.
    ///
    /// A caller begins a snapshot only at its first destructive step, so a
    /// run that saves nothing claims nothing.
    ///
    /// # Errors
    ///
    /// A `prerequisite-unmet` refusal where no state root resolves, because
    /// a destructive step that cannot save first does not run, and
    /// [`RkError::Io`] where the directory cannot be created.
    pub fn begin(canonical_target: &Path) -> Result<Self, RkError> {
        let Some(state) = applog::state_root() else {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    "no state root resolves, so nothing can be saved before it is changed, and nothing was written",
                )
                .expected("a state root under XDG_STATE_HOME or HOME")
                .action("set XDG_STATE_HOME or HOME, and run it again")
                .target_state("unchanged"),
            ));
        };
        let runs = state.join(SNAPSHOTS_DIR).join(repo_key(canonical_target));
        Ok(Self {
            root: claim(&runs, &timestamp())?,
        })
    }

    /// A snapshot rooted at `root`, for a workspace that names its own.
    #[must_use]
    pub const fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// The directory this snapshot writes under.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Save `bytes` at `relative` below the snapshot, owner-only, and
    /// return where. A copy already standing there with the same bytes is
    /// kept; one with other bytes refuses, because overwriting a backup
    /// loses the bytes it exists to keep.
    ///
    /// # Errors
    ///
    /// A `state-drift` refusal for a saved copy with other bytes, and
    /// [`RkError::Io`] for a write that fails.
    pub fn save(&self, relative: &Path, bytes: &[u8]) -> Result<PathBuf, RkError> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        match fs::read(&path) {
            Ok(existing) if existing == bytes => return Ok(path),
            Ok(_) => {
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::StateDrift,
                        format!(
                            "a saved copy with other bytes already stands at {}, and nothing was written",
                            path.display()
                        ),
                    )
                    .expected("no saved copy at that path, or one holding the same bytes")
                    .action("move the saved copy aside, then run it again")
                    .target_state("unchanged"),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        crate::atomic::write(&path, bytes)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        Ok(path)
    }

    /// Save a replaced file's bytes under `replaced/<destination>`.
    ///
    /// # Errors
    ///
    /// As [`Self::save`].
    pub fn save_replaced(&self, destination: &str, bytes: &[u8]) -> Result<PathBuf, RkError> {
        self.save(&Path::new(REPLACED_DIR).join(destination), bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    use super::{Snapshot, claim, repo_key, timestamp};

    #[test]
    fn the_repo_key_reads_as_a_name_and_keeps_targets_apart() {
        let one = repo_key(Path::new("/srv/a/widget"));
        let two = repo_key(Path::new("/srv/b/widget"));
        assert!(one.starts_with("widget-"), "{one}");
        assert_eq!(one.len(), "widget-".len() + 12);
        assert_ne!(one, two);
    }

    #[test]
    fn the_timestamp_is_compact_and_utc() {
        let stamp = timestamp();
        assert_eq!(stamp.len(), "20261009T153000Z".len(), "{stamp}");
        assert!(stamp.ends_with('Z'));
        assert_eq!(stamp.as_bytes()[8], b'T');
    }

    /// Two runs in one second each claim a directory of their own, so each
    /// keeps its copy where a shared directory would refuse the second.
    #[test]
    fn two_runs_in_one_second_keep_both_copies() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let runs = dir.path().join("snapshots").join("widget-000000000000");
        let stamp = "20261009T213000Z";
        let first = Snapshot::at(claim(&runs, stamp).expect("claims"));
        let second = Snapshot::at(claim(&runs, stamp).expect("claims again"));
        assert_ne!(first.root(), second.root());
        assert!(second.root().ends_with("20261009T213000Z-2"));
        let kept = first.save_replaced("AGENTS.md", b"first\n").expect("saves");
        let again = second
            .save_replaced("AGENTS.md", b"second\n")
            .expect("saves");
        assert_eq!(std::fs::read(kept).expect("reads"), b"first\n");
        assert_eq!(std::fs::read(again).expect("reads"), b"second\n");
        let mode = std::fs::metadata(second.root())
            .expect("stats")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    /// A saved copy is kept, an identical second save is a no-op, and a
    /// different one refuses rather than overwriting the backup.
    #[test]
    fn a_saved_copy_is_never_overwritten_with_other_bytes() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let snapshot = Snapshot::at(dir.path().join("snap"));
        let saved = snapshot
            .save_replaced(".github/workflows/release.yml", b"ours\n")
            .expect("saves");
        assert_eq!(std::fs::read(&saved).expect("reads"), b"ours\n");
        assert!(saved.ends_with("replaced/.github/workflows/release.yml"));
        snapshot
            .save_replaced(".github/workflows/release.yml", b"ours\n")
            .expect("the same bytes save again");
        let refused = snapshot
            .save_replaced(".github/workflows/release.yml", b"theirs\n")
            .expect_err("other bytes refuse");
        assert_eq!(refused.exit_code(), 73);
        assert_eq!(std::fs::read(&saved).expect("reads"), b"ours\n");
    }
}
