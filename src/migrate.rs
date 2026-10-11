//! The archive-first migration from another release tool.
//!
//! A migration moves every predecessor file into a workspace under this
//! tool's state root, lands a clean greenfield instance, and carries what
//! the project still needs from the archive one group at a time. The
//! workspace is the one place a run keeps state across runs: the archived
//! files, the snapshots no Git holds, and `registry.yaml`, which only the
//! `rk migrate` verbs write and whose per-item history is the migration's
//! log. An open migration is visible to every clone through the
//! `migration` key of the committed configuration.
//!
//! [`inventory`] is the one detector of predecessor files, [`registry`]
//! owns the workspace's record, and [`git`] is the scrubbed git these
//! verbs read the repository through.

pub mod git;
pub mod inventory;
pub mod registry;

use std::path::{Path, PathBuf};

use camino::{Utf8Path, Utf8PathBuf};

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// The machine shape every `rk migrate` report carries.
pub const SCHEMA: &str = "rk.migrate/1";

/// The directory below the state root that holds every workspace.
pub const MIGRATIONS_DIR: &str = "migrations";

/// The directory inside a workspace that holds the archived files, at
/// their target-relative paths.
pub const SOURCES_DIR: &str = "sources";

/// The directory inside a workspace that holds the Git snapshots: the
/// bundle of the long-lived branches and the tag list.
pub const REFS_DIR: &str = "refs";

/// The registry file inside a workspace.
pub const REGISTRY_FILE: &str = "registry.yaml";

/// The workspace of migration `id` at a target, below the state root:
/// `<state root>/migrations/<repo key>/<id>/`.
///
/// The repo key is the one the snapshots use, so one target's saved
/// copies and its migrations sit under one name.
#[must_use]
pub fn workspace(canonical_target: &Path, id: &str) -> Option<PathBuf> {
    Some(runs(canonical_target)?.join(id))
}

/// The directory that holds every migration workspace of one target:
/// `<state root>/migrations/<repo key>/`.
#[must_use]
pub fn runs(canonical_target: &Path) -> Option<PathBuf> {
    Some(
        crate::applog::state_root()?
            .join(MIGRATIONS_DIR)
            .join(crate::snapshot::repo_key(canonical_target)),
    )
}

/// Claim a fresh workspace below `runs` for a migration opened at
/// `stamp`, and return its identifier with its path.
///
/// The claim is the snapshots' own exclusive create, so a second archive
/// in the same second takes `<stamp>-2`, and the identifier carries the
/// counter.
///
/// # Errors
///
/// Any I/O error creating the workspace.
pub fn claim(runs: &Path, stamp: &str) -> std::io::Result<(String, PathBuf)> {
    let path = crate::snapshot::claim(runs, stamp)?;
    let id = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_owned)
        .ok_or_else(|| std::io::Error::other("a claimed workspace has no name"))?;
    Ok((id, path))
}

/// The identifier a claim below `runs` would take now, read without
/// creating anything: what a preview reports.
#[must_use]
pub fn free_id(runs: &Path, stamp: &str) -> String {
    let mut id = stamp.to_owned();
    let mut counter = 1_u32;
    while runs.join(&id).exists() {
        counter += 1;
        id = format!("{stamp}-{counter}");
    }
    id
}

/// Whether `word` is a migration identifier.
///
/// An identifier is the UTC second the archive opened, `%Y%m%dT%H%M%SZ`,
/// and `-<n>` from 2 up where an earlier archive took that second.
/// Nothing else can name a workspace, so an identifier never reaches
/// outside the target's migrations directory.
#[must_use]
pub fn is_id(word: &str) -> bool {
    // Bytes, not characters: a hand-edited value may hold a multi-byte
    // character anywhere, and it refuses rather than splitting it.
    let bytes = word.as_bytes();
    let (stamp, counter) = bytes.split_at(bytes.len().min(16));
    let digits = |part: &[u8]| part.iter().all(u8::is_ascii_digit);
    let stamp_ok = stamp.len() == 16
        && digits(&stamp[..8])
        && stamp[8] == b'T'
        && digits(&stamp[9..15])
        && stamp[15] == b'Z';
    let counter_ok = counter.is_empty()
        || counter.strip_prefix(b"-").is_some_and(|number| {
            number.first().is_some_and(|first| *first != b'0')
                && digits(number)
                && std::str::from_utf8(number)
                    .ok()
                    .and_then(|text| text.parse::<u32>().ok())
                    .is_some_and(|n| n >= 2)
        });
    stamp_ok && counter_ok
}

/// The migration the committed configuration names as open, if any.
///
/// # Errors
///
/// Propagates an unreadable or invalid configuration.
pub fn open_id(target: &Utf8Path) -> Result<Option<String>, RkError> {
    Ok(crate::config::load(target.as_std_path())?.and_then(|config| config.migration))
}

/// The workspace of the open migration, where one is open and a state
/// root resolves.
///
/// # Errors
///
/// Propagates an unreadable configuration, and a target that does not
/// canonicalize.
pub fn open_workspace(target: &Utf8Path) -> Result<Option<Utf8PathBuf>, RkError> {
    let Some(id) = open_id(target)? else {
        return Ok(None);
    };
    let canonical = std::fs::canonicalize(target.as_std_path())?;
    Ok(workspace(&canonical, &id).and_then(|path| Utf8PathBuf::from_path_buf(path).ok()))
}

/// A migration identifier: the UTC second it opened, `%Y%m%dT%H%M%SZ`.
#[must_use]
pub fn stamp(now: std::time::SystemTime) -> String {
    at(now).replace(['-', ':'], "")
}

/// A history time: UTC to the second, `%Y-%m-%dT%H:%M:%SZ`.
#[must_use]
pub fn at(now: std::time::SystemTime) -> String {
    humantime::format_rfc3339_seconds(now).to_string()
}

/// The refusal for a target with no open migration.
#[must_use]
pub fn none_open() -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            "no migration is open here; rk migrate archive opens one (migration:the-configuration-names-the-open-migration)",
        )
        .expected("a migration key in .release-kit/config.toml")
        .action("rk migrate inventory --target . lists what an archive would move")
        .target_state("unchanged"),
    )
}

/// The refusal for an open migration whose workspace this host does not
/// hold: the workspace is local state, and another clone rebuilds it.
#[must_use]
pub fn workspace_missing(id: &str, path: &Utf8Path) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!(
                "the migration {id} is open, and this host holds no workspace for it at {path} (migration:a-rebuild-recreates-the-files-alone)"
            ),
        )
        .expected("the workspace the archive wrote, on this host")
        .action("rk migrate archive --rebuild --apply --target . recreates the archived files from Git")
        .target_state("unchanged"),
    )
}

#[cfg(test)]
mod tests {
    use super::{at, claim, free_id, is_id, stamp};

    #[test]
    fn an_identifier_is_the_utc_second_it_opened() {
        let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_791_554_525);
        assert_eq!(at(when), "2026-10-09T14:02:05Z");
        assert_eq!(stamp(when), "20261009T140205Z");
    }

    /// A second archive in the same second claims the next free name, and
    /// both identifiers are ones the configuration accepts.
    #[test]
    fn a_second_archive_in_one_second_takes_the_next_free_identifier() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let runs = dir.path().join("migrations").join("widget-000000000000");
        let stamp = "20261009T140205Z";
        assert_eq!(free_id(&runs, stamp), stamp);
        let (first, first_path) = claim(&runs, stamp).expect("claims");
        assert_eq!(free_id(&runs, stamp), "20261009T140205Z-2");
        let (second, second_path) = claim(&runs, stamp).expect("claims again");
        assert_eq!(first, stamp);
        assert_eq!(second, "20261009T140205Z-2");
        assert_eq!(second_path, runs.join(&second));
        assert_ne!(first_path, second_path);
        assert!(is_id(&first) && is_id(&second));
    }

    #[test]
    fn an_identifier_is_a_stamp_and_an_optional_counter() {
        for good in [
            "20261009T140205Z",
            "20261009T140205Z-2",
            "20261009T140205Z-12",
        ] {
            assert!(is_id(good), "{good}");
        }
        for bad in [
            "",
            "20261009T140205",
            "20261009-140205Z",
            "20261009T140205Z-",
            "20261009T140205Z-1",
            "20261009T140205Z-02",
            "20261009T140205Z-2a",
            "../20261009T140205Z",
            "20261009T140205Z/..",
            // A multi-byte character across byte 16, and one after the
            // stamp, refuse rather than split.
            "20261009T140205\u{e9}",
            "20261009T140205\u{e9}-2",
            "20261009T140205Z-\u{e9}",
            "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}",
        ] {
            assert!(!is_id(bad), "{bad}");
        }
    }
}
