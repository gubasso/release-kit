//! Install the embedded skills into the agent skill directories.
//!
//! Two references decide what a destination holds: the payload this binary
//! carries, and the record of what a previous apply wrote there. Bytes
//! matching either are the tool's own and may be replaced; anything else is
//! the user's and refuses.
//!
//! Preview by default, list every conflict at once, and restore on failure —
//! the same conventions `rk init` follows, and for a sharper reason: an apply
//! crosses two roots, so a failure partway leaves one agent reading this
//! version of a skill and another agent reading the last one.
//!
//! The installer reports what it did as typed [`Action`]s and renders
//! nothing; the handler in `commands::skill` owns both renderings.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::atomic;
use crate::error::RkError;
use crate::skills::record::Record;
use crate::skills::{Digest, Skill};

/// One thing an install or uninstall did, or — in a preview — would do.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Action {
    /// The payload's bytes land at this destination.
    Write {
        /// The `SKILL.md` path written.
        destination: Utf8PathBuf,
    },
    /// The destination already holds the payload's bytes.
    Unchanged {
        /// The `SKILL.md` path left alone.
        destination: Utf8PathBuf,
    },
    /// A recorded leftover the payload no longer names is removed.
    Sweep {
        /// The leftover taken back.
        destination: Utf8PathBuf,
    },
    /// A leftover could not be removed; it stays for the operator.
    SweepFailed {
        /// The leftover still in place.
        destination: Utf8PathBuf,
        /// Why the removal failed.
        error: String,
    },
    /// An installed destination is removed.
    Remove {
        /// The `SKILL.md` path removed.
        destination: Utf8PathBuf,
    },
    /// A destination holding bytes neither the payload nor the record
    /// accounts for is the user's now, and an uninstall leaves it.
    KeptEdited {
        /// The edited `SKILL.md` path left in place.
        destination: Utf8PathBuf,
    },
    /// A directory survives a removal because something else lives in it.
    KeptDirectory {
        /// The directory kept.
        directory: Utf8PathBuf,
    },
    /// The record could not be written; a later install may ask for
    /// `--force` it should not need.
    RecordUnwritten {
        /// The record path that did not write.
        record: Utf8PathBuf,
    },
}

/// One planned write: where, and which bytes.
struct Planned {
    /// The path this run writes.
    destination: Utf8PathBuf,
    /// The payload bytes that belong there.
    bytes: &'static [u8],
}

/// Where one run writes: the agent roots, the shared root, and the record.
///
/// The shared root is not an agent root and no `--agent` selects it. Every
/// skill names its artifacts by one absolute path, so one copy serves both
/// agent families, and an install writes it whichever family it was asked
/// for.
#[derive(Debug, Clone)]
pub struct Layout {
    /// The agent skill roots this run was asked to touch.
    pub roots: Vec<Utf8PathBuf>,
    /// Every agent skill root, whichever this run selected.
    pub every_root: Vec<Utf8PathBuf>,
    /// The root holding what the skills share.
    pub shared: Utf8PathBuf,
    /// The user-scope digest record.
    pub record: Utf8PathBuf,
}

/// Every skill destination under `roots`, root by root and skill by skill.
fn plan_roots(roots: &[Utf8PathBuf], skills: &[Skill]) -> Vec<Planned> {
    let mut planned = Vec::new();
    for root in roots {
        for skill in skills {
            planned.push(Planned {
                destination: root.join(&skill.name).join("SKILL.md"),
                bytes: skill.text.as_bytes(),
            });
        }
    }
    planned
}

/// Every shared destination under `shared`.
fn plan_shared(shared: &Utf8Path) -> Vec<Planned> {
    crate::skills::shared()
        .into_iter()
        .map(|artifact| Planned {
            destination: shared.join(&artifact.path),
            bytes: artifact.bytes,
        })
        .collect()
}

/// Refuse a shared root reached through a symlink this installer would
/// follow.
///
/// The chain from the state directory down to the shared root is this tool's
/// own — nothing here ever creates a symlink in it, so one found there
/// redirects every shared write and removal somewhere else, and
/// [`check_destination`] cannot see it: the final component it checks is not
/// itself a link. The directories above the state directory are the user's
/// layout and stay unjudged.
fn check_shared_root(shared: &Utf8Path, record: &Utf8Path) -> Result<(), RkError> {
    let Some(state_dir) = record.parent() else {
        return Ok(());
    };
    let mut current = Some(shared);
    while let Some(dir) = current {
        if !dir.starts_with(state_dir) {
            break;
        }
        if dir.is_symlink() {
            return Err(RkError::Refused(format!(
                "the shared root is reached through a symlink, and nothing was written: {dir}"
            )));
        }
        current = dir.parent();
    }
    Ok(())
}

/// Refuse a destination this installer must not write through or replace.
///
/// A symlink is never followed: the payload would land wherever it points,
/// which is outside the home this command was asked to touch.
fn check_destination(destination: &Utf8Path) -> Result<(), RkError> {
    if destination.is_symlink() {
        return Err(RkError::Refused(format!(
            "destination is a symlink, and nothing was written: {destination}"
        )));
    }
    if destination.exists() && !destination.is_file() {
        return Err(RkError::Refused(format!(
            "destination is not a regular file, and nothing was written: {destination}"
        )));
    }
    Ok(())
}

/// Destinations holding bytes neither the payload nor the record accounts for.
///
/// A destination the record vouches for carries a copy this tool wrote and a
/// later release has since changed. That is an upgrade, not a conflict, and
/// naming it one would make every skill-touching release refuse on files
/// nobody edited.
fn conflicts(planned: &[Planned], record: &Record) -> Result<Vec<String>, RkError> {
    let mut conflicts = Vec::new();
    for entry in planned {
        if !entry.destination.is_file() {
            continue;
        }
        // An unreadable destination raises instead of passing as clean: a
        // comparison that cannot run must never license an overwrite.
        let found = fs::read(&entry.destination)?;
        if found == entry.bytes || record.wrote(&entry.destination, &Digest::of(&found)) {
            continue;
        }
        conflicts.push(entry.destination.to_string());
    }
    Ok(conflicts)
}

/// Recorded destinations under `roots` that this run no longer covers.
///
/// A skill the canon renamed or dropped leaves its file behind otherwise, and
/// a stale name is not inert: an agent keys its picker on the name, so the
/// leftover keeps showing up beside the skill that replaced it. Only bytes the
/// record still vouches for are swept — a leftover the user has since edited
/// is theirs, and a symlink is never followed.
fn leftovers(roots: &[Utf8PathBuf], record: &Record, keep: &[Utf8PathBuf]) -> Vec<Utf8PathBuf> {
    let kept: BTreeSet<&Utf8Path> = keep.iter().map(Utf8PathBuf::as_path).collect();
    record
        .written
        .iter()
        .filter(|(destination, digest)| {
            !kept.contains(destination.as_path())
                && roots.iter().any(|root| destination.starts_with(root))
                && !destination.is_symlink()
                && destination.is_file()
                && fs::read(destination).is_ok_and(|found| Digest::of(&found) == **digest)
        })
        .map(|(destination, _)| destination.clone())
        .collect()
}

/// Write `bytes` at `path` through the temp-plus-rename writer, creating
/// the directories it needs.
fn write_file(path: &Utf8Path, bytes: &[u8]) -> std::io::Result<()> {
    atomic::write(path.as_std_path(), bytes)
}

/// Remove one installed destination, and its directory when nothing else is
/// left there.
///
/// Returns the directory kept because something else lives in it, so
/// whatever a user put beside a skill survives its removal.
fn remove_installed(destination: &Utf8Path) -> Result<Option<Utf8PathBuf>, RkError> {
    fs::remove_file(destination)?;
    let Some(directory) = destination.parent() else {
        return Ok(None);
    };
    if fs::read_dir(directory)?.next().is_none() {
        fs::remove_dir(directory)?;
        return Ok(None);
    }
    Ok(Some(directory.to_owned()))
}

/// Restore every backed-up destination, returning those that would not go back.
///
/// A destination already holding what it held counts as restored, whatever a
/// write to it would do. Nothing else distinguishes the two ways an apply
/// reaches here — the write loop stopped before this destination, or a
/// read-only root refused every write including this one — and reporting the
/// second as unrestored sends the operator to verify files no write reached.
fn rollback(backups: &BTreeMap<Utf8PathBuf, Option<Vec<u8>>>) -> Vec<Utf8PathBuf> {
    let mut unrestored = Vec::new();
    for (destination, previous) in backups {
        let restored = previous.as_ref().map_or_else(
            || !destination.exists() || fs::remove_file(destination).is_ok(),
            |bytes| {
                fs::read(destination).is_ok_and(|found| &found == bytes)
                    || write_file(destination, bytes).is_ok()
            },
        );
        if !restored {
            unrestored.push(destination.clone());
        }
    }
    unrestored
}

/// The refusal a failed apply carries, naming the cause and what it restored.
fn abort(unrestored: &[Utf8PathBuf], cause: &str) -> RkError {
    if unrestored.is_empty() {
        return RkError::Refused(format!(
            "the install was aborted and the destinations were restored: {cause}"
        ));
    }
    let paths: Vec<&str> = unrestored.iter().map(|p| p.as_str()).collect();
    RkError::Refused(format!(
        "the install was aborted and restoration is incomplete; verify these by hand: {}: {cause}",
        paths.join(", ")
    ))
}

/// Install every embedded skill under each root, previewing by default.
///
/// `record_path` names the user-scope digest record: read to tell a stale copy
/// this tool wrote from a file the user edited, and rewritten after a
/// successful apply. Failing to write it is not a failure of the install — the
/// files landed — so it costs only the benefit of the doubt next time.
///
/// # Errors
///
/// Returns [`RkError::Refused`] when the shared root is reached through a
/// symlink, when a destination cannot be touched, when one holds bytes neither
/// reference accounts for and `force` is unset, or when a write fails partway,
/// in which case the destinations are restored first.
/// Returns [`RkError::Io`] when a destination exists but cannot be read.
pub fn install(layout: &Layout, apply: bool, force: bool) -> Result<Vec<Action>, RkError> {
    check_shared_root(&layout.shared, &layout.record)?;
    let record_path = layout.record.as_path();
    let skills = crate::skills::all()?;
    let mut planned = plan_roots(&layout.roots, &skills);
    planned.extend(plan_shared(&layout.shared));
    for entry in &planned {
        check_destination(&entry.destination)?;
    }

    let mut record = Record::load(record_path);
    let covered: Vec<Utf8PathBuf> = planned
        .iter()
        .map(|entry| entry.destination.clone())
        .collect();
    let mut scanned = layout.roots.clone();
    scanned.push(layout.shared.clone());
    let stale = leftovers(&scanned, &record, &covered);

    if !apply {
        let mut actions: Vec<Action> = covered
            .into_iter()
            .map(|destination| Action::Write { destination })
            .collect();
        actions.extend(
            stale
                .into_iter()
                .map(|destination| Action::Sweep { destination }),
        );
        return Ok(actions);
    }

    if !force {
        let conflicts = conflicts(&planned, &record)?;
        if !conflicts.is_empty() {
            return Err(RkError::Refused(format!(
                "these destinations hold bytes this tool did not write, and nothing was written: {}; re-run with --force to overwrite",
                conflicts.join(", ")
            )));
        }
    }

    // Back up every destination before the first write, so a failure on the
    // second root cannot leave the first one upgraded.
    let mut backups: BTreeMap<Utf8PathBuf, Option<Vec<u8>>> = BTreeMap::new();
    for entry in &planned {
        let previous = if entry.destination.is_file() {
            Some(fs::read(&entry.destination).map_err(|source| {
                RkError::Refused(format!(
                    "cannot back up {}, and nothing was written: {source}",
                    entry.destination
                ))
            })?)
        } else {
            None
        };
        backups.insert(entry.destination.clone(), previous);
    }

    let mut actions = Vec::new();
    for entry in &planned {
        let held = backups.get(&entry.destination).and_then(Option::as_ref);
        if held.is_some_and(|previous| previous == entry.bytes) {
            actions.push(Action::Unchanged {
                destination: entry.destination.clone(),
            });
            continue;
        }
        if let Err(source) = write_file(&entry.destination, entry.bytes) {
            return Err(abort(
                &rollback(&backups),
                &format!("writing {} failed: {source}", entry.destination),
            ));
        }
        actions.push(Action::Write {
            destination: entry.destination.clone(),
        });
    }

    // Sweep after the writes, never before: a refusal must leave the home
    // exactly as it found it, and a leftover is harmless until the install
    // superseding it has actually landed.
    for destination in &stale {
        match remove_installed(destination) {
            Ok(kept) => {
                actions.push(Action::Sweep {
                    destination: destination.clone(),
                });
                actions.extend(kept.map(|directory| Action::KeptDirectory { directory }));
                record.written.remove(destination);
            }
            Err(source) => actions.push(Action::SweepFailed {
                destination: destination.clone(),
                error: source.to_string(),
            }),
        }
    }

    for entry in &planned {
        record
            .written
            .insert(entry.destination.clone(), Digest::of(entry.bytes));
    }
    if write_file(record_path, record.to_text().as_bytes()).is_err() {
        actions.push(Action::RecordUnwritten {
            record: record_path.to_owned(),
        });
    }
    Ok(actions)
}

/// Remove every installed skill under each root, previewing by default.
///
/// Only bytes this tool can vouch for go: a destination holding the payload's
/// bytes or bytes the record says it wrote, plus the recorded leftovers, and a
/// directory only once nothing else lives in it. A destination the user has
/// edited is theirs now and stays, reported rather than removed. An absent
/// destination is a no-op, so a re-run succeeds. Removed destinations leave
/// the record too; what is gone cannot be vouched for.
///
/// # Errors
///
/// Returns [`RkError::Refused`] when the shared root is reached through a
/// symlink, or when a destination is a symlink or not a regular file, and
/// [`RkError::Io`] when a removal fails.
pub fn uninstall(layout: &Layout, apply: bool) -> Result<Vec<Action>, RkError> {
    check_shared_root(&layout.shared, &layout.record)?;
    let record_path = layout.record.as_path();
    let skills = crate::skills::all()?;
    let record_found = Record::load(record_path);
    let mut removable: Vec<Utf8PathBuf> = Vec::new();
    let mut edited: Vec<Utf8PathBuf> = Vec::new();
    let classify = |entry: &Planned,
                    removable: &mut Vec<Utf8PathBuf>,
                    edited: &mut Vec<Utf8PathBuf>|
     -> Result<(), RkError> {
        check_destination(&entry.destination)?;
        if !entry.destination.is_file() {
            return Ok(());
        }
        // The same two references an install trusts decide what goes: the
        // payload's bytes, or bytes the record vouches this tool wrote.
        // Anything else is the user's edit, and removing it would destroy
        // work an install refuses to even overwrite.
        let found = fs::read(&entry.destination)?;
        if found == entry.bytes || record_found.wrote(&entry.destination, &Digest::of(&found)) {
            removable.push(entry.destination.clone());
        } else {
            edited.push(entry.destination.clone());
        }
        Ok(())
    };

    let selected = plan_roots(&layout.roots, &skills);
    for entry in &selected {
        classify(entry, &mut removable, &mut edited)?;
    }

    // The shared artifacts serve every agent root, so they go only once no
    // root still holds a skill that reads them. Taking them during an
    // `--agent codex` uninstall would leave the Claude skills naming a file
    // that is no longer there.
    let going: BTreeSet<&Utf8Path> = removable.iter().map(Utf8PathBuf::as_path).collect();
    let retained = plan_roots(&layout.every_root, &skills)
        .iter()
        .any(|entry| !going.contains(entry.destination.as_path()) && entry.destination.is_file());
    let mut scanned = layout.roots.clone();
    if !retained {
        for entry in plan_shared(&layout.shared) {
            classify(&entry, &mut removable, &mut edited)?;
        }
        scanned.push(layout.shared.clone());
    }

    let mut record = record_found;
    // A skill the payload has since dropped is still ours to take back, and an
    // uninstall leaving it behind is the leftover an agent keeps offering. The
    // record is what names it; the payload no longer can.
    let stale = leftovers(&scanned, &record, &removable);

    if !apply {
        let mut actions: Vec<Action> = removable
            .into_iter()
            .map(|destination| Action::Remove { destination })
            .collect();
        actions.extend(
            stale
                .into_iter()
                .map(|destination| Action::Sweep { destination }),
        );
        actions.extend(
            edited
                .into_iter()
                .map(|destination| Action::KeptEdited { destination }),
        );
        return Ok(actions);
    }

    removable.extend(stale);
    let mut actions = Vec::new();
    for destination in &removable {
        let kept = remove_installed(destination)?;
        actions.push(Action::Remove {
            destination: destination.clone(),
        });
        actions.extend(kept.map(|directory| Action::KeptDirectory { directory }));
        record.written.remove(destination);
    }
    actions.extend(
        edited
            .into_iter()
            .map(|destination| Action::KeptEdited { destination }),
    );

    let recorded = if record.written.is_empty() {
        fs::remove_file(record_path).or_else(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(source)
            }
        })
    } else {
        write_file(record_path, record.to_text().as_bytes())
    };
    if recorded.is_err() {
        actions.push(Action::RecordUnwritten {
            record: record_path.to_owned(),
        });
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use camino::Utf8PathBuf;

    use super::{Action, Layout, install, leftovers, uninstall};
    use crate::skills::record::{RECORD_PATH, Record};
    use crate::skills::{Digest, all};

    /// A scratch home, plus the roots and record path it implies.
    struct Home {
        dir: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("a scratch home exists"),
            }
        }

        fn path(&self) -> Utf8PathBuf {
            Utf8PathBuf::from_path_buf(self.dir.path().to_path_buf())
                .expect("the temp path is UTF-8")
        }

        fn roots(&self) -> Vec<Utf8PathBuf> {
            let home = self.path();
            vec![home.join(".claude/skills"), home.join(".agents/skills")]
        }

        fn record(&self) -> Utf8PathBuf {
            self.path().join(RECORD_PATH)
        }

        fn destination(&self, root: &str, skill: &str) -> Utf8PathBuf {
            self.path().join(root).join(skill).join("SKILL.md")
        }

        fn shared(&self) -> Utf8PathBuf {
            self.path().join(".local/state/release-kit/skills/shared")
        }

        /// The layout for every agent root.
        fn layout(&self) -> Layout {
            self.layout_for(self.roots())
        }

        /// The layout for a subset of the agent roots.
        fn layout_for(&self, roots: Vec<Utf8PathBuf>) -> Layout {
            Layout {
                roots,
                every_root: self.roots(),
                shared: self.shared(),
                record: self.record(),
            }
        }
    }

    /// How many shared artifacts every install writes, whatever the agent.
    fn shared_count() -> usize {
        crate::skills::shared().len()
    }

    fn first_skill() -> String {
        all().expect("the skills read").swap_remove(0).name
    }

    #[test]
    fn a_preview_lists_every_destination_and_writes_nothing() {
        let home = Home::new();
        let actions = install(&home.layout(), false, false).unwrap();
        let count = all().unwrap().len();
        assert_eq!(actions.len(), count * 2 + shared_count(), "{actions:?}");
        assert!(
            actions
                .iter()
                .all(|action| matches!(action, Action::Write { .. })),
            "{actions:?}"
        );
        assert!(!home.path().join(".claude").exists());
        assert!(!home.record().exists());
    }

    #[test]
    fn an_apply_is_idempotent_and_records_what_it_wrote() {
        let home = Home::new();
        let first = install(&home.layout(), true, false).unwrap();
        assert!(
            first
                .iter()
                .all(|action| matches!(action, Action::Write { .. })),
            "{first:?}"
        );
        let second = install(&home.layout(), true, false).unwrap();
        assert!(
            second
                .iter()
                .all(|action| matches!(action, Action::Unchanged { .. })),
            "{second:?}"
        );
        let record = Record::load(&home.record());
        assert_eq!(
            record.written.len(),
            all().unwrap().len() * 2 + shared_count()
        );
    }

    /// The defect the record exists for: bytes a previous release wrote are
    /// not the user's, and refusing on them makes every skill-touching release
    /// break the install recipe.
    #[test]
    fn a_copy_a_previous_release_wrote_is_replaced_without_force() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();

        // Stand in for an older release: rewrite each destination and record
        // its digest, exactly as that release's apply would have left it.
        let mut stale = Record::default();
        for destination in Record::load(&home.record()).written.into_keys() {
            std::fs::write(&destination, "older canon bytes\n").unwrap();
            stale
                .written
                .insert(destination, Digest::of(b"older canon bytes\n"));
        }
        std::fs::write(home.record(), stale.to_text()).unwrap();

        install(&home.layout(), true, false).unwrap();
        let text =
            std::fs::read_to_string(home.destination(".claude/skills", &first_skill())).unwrap();
        assert!(text.contains(&format!("name: {}", first_skill())));
    }

    /// A record vouching for one destination says nothing about another.
    #[test]
    fn an_edit_refuses_and_names_every_conflict() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let edited: Vec<Utf8PathBuf> = all()
            .unwrap()
            .iter()
            .map(|skill| home.destination(".claude/skills", &skill.name))
            .collect();
        for destination in &edited {
            std::fs::write(destination, "the user wrote this").unwrap();
        }

        let message = install(&home.layout(), true, false)
            .unwrap_err()
            .to_string();
        for destination in &edited {
            assert!(message.contains(destination.as_str()), "{message}");
        }
        for destination in &edited {
            assert_eq!(
                std::fs::read_to_string(destination).unwrap(),
                "the user wrote this",
                "a refused install must not overwrite"
            );
        }
        install(&home.layout(), true, true).unwrap();
        assert!(
            std::fs::read_to_string(&edited[0])
                .unwrap()
                .starts_with("---")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_destination_refuses_before_anything_is_written() {
        let home = Home::new();
        let skill = first_skill();
        let destination = home.destination(".claude/skills", &skill);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let elsewhere = home.path().join("elsewhere");
        std::fs::write(&elsewhere, "the user's file\n").unwrap();
        std::os::unix::fs::symlink(&elsewhere, &destination).unwrap();

        let message = install(&home.layout(), true, true).unwrap_err().to_string();
        assert!(message.contains("symlink"), "{message}");
        assert_eq!(
            std::fs::read_to_string(&elsewhere).unwrap(),
            "the user's file\n"
        );
        assert!(!home.path().join(".agents").exists());
    }

    /// A failure on the second root must not leave the first one upgraded.
    #[test]
    fn a_failed_write_restores_every_destination() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let first = home.destination(".claude/skills", &first_skill());
        std::fs::write(&first, "older canon bytes\n").unwrap();
        let mut record = Record::load(&home.record());
        record
            .written
            .insert(first.clone(), Digest::of(b"older canon bytes\n"));
        std::fs::write(home.record(), record.to_text()).unwrap();

        // A regular file where the second root's skill directory belongs: the
        // directory cannot be created, so that root's write fails.
        let blocked = home.path().join(".agents/skills").join(first_skill());
        std::fs::remove_file(blocked.join("SKILL.md")).unwrap();
        std::fs::remove_dir(&blocked).unwrap();
        std::fs::write(&blocked, "in the way\n").unwrap();

        let message = install(&home.layout(), true, false)
            .unwrap_err()
            .to_string();
        assert!(message.contains("aborted"), "{message}");
        assert_eq!(
            std::fs::read_to_string(&first).unwrap(),
            "older canon bytes\n",
            "the first root must be restored"
        );
    }

    #[test]
    fn an_install_sweeps_a_destination_the_payload_dropped() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let dropped = home.destination(".claude/skills", "rk-retired");
        std::fs::create_dir_all(dropped.parent().unwrap()).unwrap();
        std::fs::write(&dropped, "a skill a later release dropped\n").unwrap();
        let mut record = Record::load(&home.record());
        record.written.insert(
            dropped.clone(),
            Digest::of(b"a skill a later release dropped\n"),
        );
        std::fs::write(home.record(), record.to_text()).unwrap();

        let actions = install(&home.layout(), true, false).unwrap();
        assert!(
            actions.contains(&Action::Sweep {
                destination: dropped.clone()
            }),
            "{actions:?}"
        );
        assert!(!dropped.exists());
        assert!(!dropped.parent().unwrap().exists());
        assert!(!Record::load(&home.record()).written.contains_key(&dropped));
    }

    /// A leftover the user has since edited is theirs, not ours to remove.
    #[test]
    fn a_sweep_leaves_an_edited_leftover_alone() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let dropped = home.destination(".claude/skills", "rk-retired");
        std::fs::create_dir_all(dropped.parent().unwrap()).unwrap();
        std::fs::write(&dropped, "the user rewrote this\n").unwrap();
        let mut record = Record::load(&home.record());
        record
            .written
            .insert(dropped.clone(), Digest::of(b"what we wrote\n"));
        std::fs::write(home.record(), record.to_text()).unwrap();

        assert!(
            !leftovers(&home.roots(), &record, &[]).contains(&dropped),
            "a leftover whose bytes differ from the record is the user's"
        );
        install(&home.layout(), true, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&dropped).unwrap(),
            "the user rewrote this\n"
        );
    }

    /// A destination the user edited after installing is theirs: an
    /// uninstall reports it and leaves it, exactly as an install refuses
    /// to overwrite it.
    #[test]
    fn an_uninstall_keeps_an_edited_destination() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let edited = home.destination(".claude/skills", &first_skill());
        std::fs::write(&edited, "the user rewrote this\n").unwrap();

        let preview = uninstall(&home.layout(), false).unwrap();
        assert!(
            preview.contains(&Action::KeptEdited {
                destination: edited.clone()
            }),
            "{preview:?}"
        );
        assert!(
            !preview.contains(&Action::Remove {
                destination: edited.clone()
            }),
            "{preview:?}"
        );

        let actions = uninstall(&home.layout(), true).unwrap();
        assert!(
            actions.contains(&Action::KeptEdited {
                destination: edited.clone()
            }),
            "{actions:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&edited).unwrap(),
            "the user rewrote this\n",
            "an uninstall must never delete a user's edit"
        );
        // Everything the tool can vouch for is still removed.
        assert!(!home.destination(".agents/skills", &first_skill()).exists());
    }

    #[test]
    fn an_uninstall_removes_what_it_wrote_and_keeps_the_rest() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let skill = first_skill();
        let beside = home
            .destination(".claude/skills", &skill)
            .parent()
            .unwrap()
            .join("notes.md");
        std::fs::write(&beside, "the user's notes\n").unwrap();

        let actions = uninstall(&home.layout(), true).unwrap();
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::KeptDirectory { .. })),
            "{actions:?}"
        );
        assert!(!home.destination(".claude/skills", &skill).exists());
        assert!(beside.is_file(), "a file beside a skill must survive");
        assert!(!home.record().exists(), "an empty record is removed");
        // A re-run over an emptied home is a no-op, not a failure.
        uninstall(&home.layout(), true).unwrap();
    }

    #[test]
    fn one_root_installs_and_uninstalls_without_touching_the_other() {
        let home = Home::new();
        let claude = home.layout_for(vec![home.path().join(".claude/skills")]);
        install(&claude, true, false).unwrap();
        assert!(home.destination(".claude/skills", &first_skill()).is_file());
        assert!(!home.path().join(".agents").exists());

        uninstall(&claude, true).unwrap();
        assert!(!home.destination(".claude/skills", &first_skill()).exists());
    }

    /// Every agent reads the same shared artifacts, so an install writes them
    /// whichever family it was asked for.
    #[test]
    fn either_agent_alone_still_lands_the_shared_artifacts() {
        for root in [".claude/skills", ".agents/skills"] {
            let home = Home::new();
            let one = home.layout_for(vec![home.path().join(root)]);
            install(&one, true, false).unwrap();
            assert!(
                home.shared().join("plan-gate.md").is_file(),
                "{root}: the shared gate did not land"
            );
        }
    }

    /// The defect this guards: taking the shared artifacts while another root
    /// still holds skills leaves those skills naming a file that is gone.
    #[test]
    fn the_shared_artifacts_stay_while_another_root_still_holds_skills() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let gate = home.shared().join("plan-gate.md");
        assert!(gate.is_file());

        let codex = home.layout_for(vec![home.path().join(".agents/skills")]);
        uninstall(&codex, true).unwrap();
        assert!(!home.destination(".agents/skills", &first_skill()).exists());
        assert!(
            gate.is_file(),
            "the Claude skills still read the gate, so it must stay"
        );

        let claude = home.layout_for(vec![home.path().join(".claude/skills")]);
        let actions = uninstall(&claude, true).unwrap();
        assert!(
            !gate.exists(),
            "the last uninstall takes the gate: {actions:?}"
        );
    }

    /// A preview of that last uninstall names the gate before it removes it.
    #[test]
    fn the_last_uninstall_previews_the_shared_artifacts() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let actions = uninstall(&home.layout(), false).unwrap();
        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::Remove { destination } if destination.file_name() == Some("plan-gate.md")
            )),
            "{actions:?}"
        );
        assert!(
            home.shared().join("plan-gate.md").is_file(),
            "a preview writes nothing"
        );
    }

    /// A shared artifact the user edited is theirs, exactly as a skill is.
    #[test]
    fn an_edited_shared_artifact_refuses_an_install_and_survives_an_uninstall() {
        let home = Home::new();
        install(&home.layout(), true, false).unwrap();
        let gate = home.shared().join("plan-gate.md");
        std::fs::write(&gate, b"mine now").unwrap();

        let message = install(&home.layout(), true, false)
            .expect_err("an edited gate refuses")
            .to_string();
        assert!(message.contains("plan-gate.md"), "{message}");

        let actions = uninstall(&home.layout(), true).unwrap();
        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::KeptEdited { destination } if destination == &gate
            )),
            "{actions:?}"
        );
        assert_eq!(std::fs::read(&gate).unwrap(), b"mine now");
    }

    /// A symlinked shared destination refuses before anything is written, the
    /// same as a symlinked skill.
    #[test]
    fn a_symlinked_shared_destination_refuses_before_writing() {
        let home = Home::new();
        let gate = home.shared().join("plan-gate.md");
        std::fs::create_dir_all(home.shared()).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", &gate).unwrap();

        let message = install(&home.layout(), true, false)
            .expect_err("a symlink refuses")
            .to_string();
        assert!(message.contains("symlink"), "{message}");
        assert!(
            !home.destination(".claude/skills", &first_skill()).exists(),
            "the refusal must come before the first write"
        );
    }

    /// A shared root reached through a symlinked directory refuses both verbs:
    /// the write would land, and the removal would delete, wherever the link
    /// points, and no per-destination check can see it.
    #[test]
    fn a_symlinked_shared_root_refuses_install_and_uninstall() {
        for symlinked in ["skills", "skills/shared"] {
            let home = Home::new();
            let elsewhere = home.path().join("elsewhere");
            std::fs::create_dir_all(&elsewhere).unwrap();
            let state_dir = home.record().parent().unwrap().to_path_buf();
            let linked = state_dir.join(symlinked);
            std::fs::create_dir_all(linked.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();

            let message = install(&home.layout(), true, false)
                .expect_err("a symlinked shared root refuses an install")
                .to_string();
            assert!(message.contains("symlink"), "{message}");
            assert!(
                !elsewhere.join("plan-gate.md").exists(),
                "an install must never write through a symlinked shared root"
            );
            assert!(!home.path().join(".claude").exists());

            std::fs::write(elsewhere.join("plan-gate.md"), "theirs\n").unwrap();
            let message = uninstall(&home.layout(), true)
                .expect_err("a symlinked shared root refuses an uninstall")
                .to_string();
            assert!(message.contains("symlink"), "{message}");
            assert!(
                elsewhere.join("plan-gate.md").exists(),
                "an uninstall must never remove through a symlinked shared root"
            );
        }
    }
}
