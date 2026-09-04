//! `rk devshell`: release-kit as a consumer project's development
//! dependency, kept fresh.
//!
//! A consumer pins release-kit as a flake input at a release tag and
//! takes `rk` from its devshell. Two files carry the fact: the tag in
//! `flake.nix` is the version, and the `release-kit` node in
//! `flake.lock` is the content. This module owns the offline observation
//! of that wiring and the per-checkout state key; `pin` owns the line
//! grammar, `fragments` the authored texts `add` serves, `leftovers` the
//! predecessor catalog `clean` removes, `discover` the one network call,
//! and `txn` the fenced two-file transaction.

pub mod discover;
pub mod fragments;
pub mod leftovers;
pub mod pin;
pub mod txn;

use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;

/// Whether a file exists at its expected path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Presence {
    /// The path holds a file, a symlink included.
    Present,
    /// Nothing is at the path.
    Absent,
}

impl Presence {
    /// Judge a path by `symlink_metadata`, so a dangling symlink still
    /// counts as present: the verb would refuse to write over it.
    #[must_use]
    pub fn of(path: &Utf8Path) -> Self {
        if std::fs::symlink_metadata(path).is_ok() {
            Self::Present
        } else {
            Self::Absent
        }
    }

    /// Whether the file is there.
    #[must_use]
    pub const fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }
}

/// Everything the offline pass reads from a target and this host's state
/// root. It spawns nothing and fetches nothing.
#[derive(Debug, Clone)]
pub struct Observed {
    /// The target, canonical.
    pub target: Utf8PathBuf,
    /// Whether `flake.nix` exists.
    pub flake: Presence,
    /// Whether `flake.lock` exists.
    pub lock: Presence,
    /// What the pin matcher found in `flake.nix`.
    pub scan: pin::Scan,
    /// The `flake.nix` text, where the file read.
    pub flake_text: Option<String>,
    /// The locked commit of the `release-kit` node, where the lock names one.
    pub locked_rev: Option<String>,
    /// The locked ref of the `release-kit` node, where the lock names one.
    pub locked_ref: Option<String>,
    /// Whether `.envrc` exists.
    pub envrc: Presence,
    /// Whether `.envrc` carries the sync line.
    pub envrc_sync: bool,
    /// Whether a transaction marker for this checkout survives.
    pub pending: bool,
    /// The day of the last sync attempt for this checkout, where stamped.
    pub stamp: Option<String>,
    /// What a predecessor bump mechanism left in the target.
    pub leftovers: Vec<leftovers::Leftover>,
}

impl Observed {
    /// The per-checkout state key.
    #[must_use]
    pub fn key(&self) -> String {
        state_key(&self.target)
    }

    /// The pinned tag, where the scan found exactly one pin.
    #[must_use]
    pub fn pin_tag(&self) -> Option<&str> {
        match &self.scan {
            pin::Scan::One(pin) => Some(pin.tag.as_str()),
            _ => None,
        }
    }

    /// The rollup state, first match wins.
    #[must_use]
    pub fn state(&self) -> &'static str {
        if self.pending {
            return "pending-recovery";
        }
        if !self.flake.is_present() {
            return "no-flake";
        }
        match self.scan {
            pin::Scan::Many(_) => return "ambiguous-pin",
            pin::Scan::None => return "not-wired",
            pin::Scan::Unpinned(_) => return "unpinned",
            pin::Scan::One(_) => {}
        }
        if self.leftovers.is_empty() {
            "ready"
        } else {
            "superseded"
        }
    }
}

/// Read a target's devshell wiring, offline.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory and
/// [`RkError::Io`] where a present file does not read.
pub fn observe(target: &Utf8Path) -> Result<Observed, RkError> {
    let target = canonical_target(target)?;
    let flake_path = target.join("flake.nix");
    let flake = Presence::of(&flake_path);
    let flake_text = if flake.is_present() {
        Some(std::fs::read_to_string(&flake_path)?)
    } else {
        None
    };
    let scan = flake_text.as_deref().map_or(pin::Scan::None, pin::scan);
    let lock_path = target.join("flake.lock");
    let lock = Presence::of(&lock_path);
    let (locked_rev, locked_ref_name) = if lock.is_present() {
        locked_node(&std::fs::read(&lock_path)?)
    } else {
        (None, None)
    };
    let envrc_path = target.join(".envrc");
    let envrc = Presence::of(&envrc_path);
    let envrc_sync = envrc.is_present() && has_sync_line(&std::fs::read_to_string(&envrc_path)?);
    let key = state_key(&target);
    let pending = marker_path(&key).is_some_and(|marker| marker.exists());
    let stamp = read_stamp(&key);
    let leftovers = leftovers::scan(&target)?;
    Ok(Observed {
        target,
        flake,
        lock,
        scan,
        flake_text,
        locked_rev,
        locked_ref: locked_ref_name,
        envrc,
        envrc_sync,
        pending,
        stamp,
        leftovers,
    })
}

/// Whether an `.envrc` text carries the sync line: a line whose
/// trimmed start is the verb, whatever flags follow.
#[must_use]
pub fn has_sync_line(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim_start().starts_with("rk devshell sync"))
}

/// The `release-kit` node's locked commit and ref, from a `flake.lock`.
fn locked_node(bytes: &[u8]) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return (None, None);
    };
    let locked = &value["nodes"]["release-kit"]["locked"];
    let read = |field: &str| locked[field].as_str().map(str::to_owned);
    (read("rev"), read("ref"))
}

/// The target as a canonical directory, or the missing-target refusal.
fn canonical_target(target: &Utf8Path) -> Result<Utf8PathBuf, RkError> {
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory"),
            )
            .expected("an existing project directory to read"),
        ));
    }
    Ok(target.canonicalize_utf8()?)
}

/// The per-checkout key every state file is named by:
/// `<basename>-<digest16>` over the canonical path, so two clones never
/// share a lock, a stamp, or a backup.
#[must_use]
pub fn state_key(target: &Utf8Path) -> String {
    let base = target
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or("root");
    let digest = Digest::of(target.as_str().as_bytes()).to_string();
    format!("{base}-{}", &digest[..16])
}

/// The directory every devshell state file lives under:
/// `<state root>/devshell`.
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    crate::applog::state_root().map(|root| root.join("devshell"))
}

/// The single-writer lock for one checkout.
#[must_use]
pub fn lock_path(key: &str) -> Option<PathBuf> {
    state_dir().map(|dir| dir.join(format!("{key}.lock")))
}

/// The daily stamp for one checkout.
#[must_use]
pub fn stamp_path(key: &str) -> Option<PathBuf> {
    state_dir().map(|dir| dir.join(format!("{key}.stamp")))
}

/// The directory a transaction backs the two files up into.
#[must_use]
pub fn backup_dir(key: &str) -> Option<PathBuf> {
    state_dir().map(|dir| dir.join(key).join("backup"))
}

/// The marker an open transaction leaves until it commits or restores.
#[must_use]
pub fn marker_path(key: &str) -> Option<PathBuf> {
    state_dir().map(|dir| dir.join(key).join("pending.json"))
}

/// The day the last sync attempt was stamped, where one was.
#[must_use]
pub fn read_stamp(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(stamp_path(key)?).ok()?;
    let day = text.trim();
    (day.len() == 10).then(|| day.to_owned())
}

/// Fold the three tag shapes — `v0.2.16`, `0.2.16`, and the release URL
/// — to one tag with exactly one leading `v`.
#[must_use]
pub fn normalize_tag(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let tail = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let bare = tail.strip_prefix('v').unwrap_or(tail);
    let shaped = bare.chars().next().is_some_and(|c| c.is_ascii_digit())
        && bare
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    shaped.then(|| format!("v{bare}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use camino::Utf8Path;

    use super::{has_sync_line, locked_node, normalize_tag, state_key};

    #[test]
    fn the_tag_normalizer_folds_three_shapes_to_one() {
        for raw in [
            "v0.2.16",
            "0.2.16",
            "https://github.com/owner/release-kit/releases/tag/v0.2.16",
            "https://github.com/owner/release-kit/releases/tag/v0.2.16/",
            " v0.2.16\n",
        ] {
            assert_eq!(normalize_tag(raw).as_deref(), Some("v0.2.16"), "{raw:?}");
        }
        assert_eq!(normalize_tag("v0.3.0-rc.1").as_deref(), Some("v0.3.0-rc.1"));
        assert_eq!(normalize_tag(""), None);
        assert_eq!(normalize_tag("latest"), None);
        assert_eq!(normalize_tag("vv0.2.16"), None, "a doubled v is not a tag");
        assert_eq!(
            normalize_tag("https://github.com/owner/release-kit/releases/latest"),
            None
        );
    }

    #[test]
    fn the_state_key_is_stable_per_checkout() {
        let a = state_key(Utf8Path::new("/srv/one/widget"));
        let b = state_key(Utf8Path::new("/srv/two/widget"));
        assert_eq!(a, state_key(Utf8Path::new("/srv/one/widget")));
        assert_ne!(a, b, "two clones of one project key apart");
        assert!(a.starts_with("widget-"), "{a}");
        assert_eq!(a.len(), "widget-".len() + 16);
        assert!(state_key(Utf8Path::new("/")).starts_with("root-"));
    }

    #[test]
    fn the_sync_line_is_found_by_its_verb() {
        assert!(has_sync_line(
            "use flake\nrk devshell sync --apply || true\n"
        ));
        assert!(has_sync_line("  rk devshell sync\n"));
        assert!(!has_sync_line("# rk devshell sync\nuse flake\n"));
        assert!(!has_sync_line(""));
    }

    #[test]
    fn the_locked_node_reads_the_release_kit_input() {
        let lock = br#"{"nodes":{"release-kit":{"locked":{"rev":"9f3c","ref":"refs/tags/v0.2.16"}},"root":{}}}"#;
        assert_eq!(
            locked_node(lock),
            (
                Some("9f3c".to_owned()),
                Some("refs/tags/v0.2.16".to_owned())
            )
        );
        assert_eq!(locked_node(b"not json"), (None, None));
        assert_eq!(locked_node(br#"{"nodes":{}}"#), (None, None));
    }
}
