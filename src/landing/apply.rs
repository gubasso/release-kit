//! The direct landing writer: from a computed [`Projection`] and the
//! receipt as it stands to the files on disk and the receipt written
//! last.
//!
//! Every landing renders afresh from this binary and the target at
//! invocation. The writer decides each destination by its recorded kind
//! alone, validates every destination before the first write, holds one
//! target lock through the receipt write, opens the target directory once
//! and writes every file relative to that held directory so a component
//! swapped for a link after validation redirects nothing, and replaces
//! each destination through a same-directory temporary file and a
//! rename. The set is not transactional: a failure names every completed
//! path, leaves the previous receipt, and the rerun lands the rest.
//!
//! SATISFIES landing:ownership-is-elementary
//! SATISFIES landing:a-partial-landing-is-visible-and-rerunnable
//! SATISFIES landing:a-landing-leaves-a-record

use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;

use camino::Utf8Path;
use serde::Serialize;

use super::manifest::{self, FileRecord, Manifest, Parameters};
use super::{Kind, Params, lock};
use crate::config;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::held;
use crate::projection::{Candidate, Placement, Projection, ProjectionInput, TargetEvidence};

/// The environment variable the interruption proof sets to the relative
/// destination whose rename is to fail on purpose.
///
/// A rename cannot be made to fail from outside without a read failing
/// first, and the proof is about what the tree holds after a landing that
/// stopped part way.
pub const INTERRUPT_VAR: &str = "RK_APPLY_INTERRUPT_AT";

/// The environment variable naming a directory the proof pauses through
/// once validation is over and the target is held: `validated` appears
/// there, and the landing waits for `proceed`.
pub const PAUSE_VAR: &str = "RK_APPLY_PAUSE_DIR";

/// What the landing does with one destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// The destination is absent and the candidate is written.
    Created,
    /// A recorded generated whole file or marked region is rewritten from
    /// the candidate, whatever its bytes were.
    Replaced,
    /// A recorded seeded or state file stays as it is, its current digest
    /// entering the receipt.
    Preserved,
    /// A recorded seeded file stays and its bytes differ from the receipt:
    /// the target tuned it, which is what a seeded file is for.
    Drift,
    /// A recorded destination this binary no longer produces: left on
    /// disk, target-owned from this landing, out of the new receipt.
    Released,
}

impl Action {
    /// The report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Replaced => "replaced",
            Self::Preserved => "preserved",
            Self::Drift => "drift",
            Self::Released => "released",
        }
    }
}

/// One decided destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The destination, relative to the target.
    pub destination: String,
    /// The kind the candidate declares, or the recorded kind for a
    /// released destination.
    pub kind: Kind,
    /// What happens to it.
    pub action: Action,
}

/// One destination the landing refuses before any write: a whole-file
/// destination present on disk with no receipt entry attributing it, or a
/// document whose markers offer the block no place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Collision {
    /// The destination.
    pub path: String,
    /// Why it refuses.
    pub reason: String,
}

/// The landing decided and ready: the projection, every decision in
/// projection order with the released destinations after them, every
/// collision, and the configuration the landing writes first.
#[derive(Debug)]
pub struct Prepared {
    /// The resolved parameters.
    pub params: Params,
    /// The candidate tree.
    pub projection: Projection,
    /// Every decision.
    pub decisions: Vec<Decision>,
    /// Every collision, in destination order.
    pub collisions: Vec<Collision>,
    /// The configuration the landing writes before the files.
    pub config: config::Plan,
}

impl Prepared {
    /// The decision for one destination.
    #[must_use]
    pub fn decision(&self, destination: &str) -> Option<&Decision> {
        self.decisions
            .iter()
            .find(|decision| decision.destination == destination)
    }
}

/// Gather the target's evidence once, compute the projection, and decide
/// every destination, writing nothing.
///
/// Under an apply this runs inside the target lock, so the evidence the
/// landing writes from is the evidence it gathered.
///
/// # Errors
///
/// The evidence read's failures, the projection's own defects, and an
/// invalid committed configuration.
pub fn prepare(
    target: &Utf8Path,
    recorded: Option<&Manifest>,
    params: &Params,
    existing_config: Option<&config::Config>,
) -> Result<Prepared, RkError> {
    let evidence = TargetEvidence::gather(target, recorded)?;
    let projection = Projection::compute(&ProjectionInput {
        params: params.clone(),
        evidence,
    })?;
    let (decisions, collisions) = decide(target, recorded, &projection)?;
    let config = config::Plan::new(target.as_std_path(), params, existing_config, recorded)?;
    Ok(Prepared {
        params: params.clone(),
        projection,
        decisions,
        collisions,
        config,
    })
}

/// Decide every destination from the receipt and the disk, collecting
/// every collision rather than stopping at the first.
///
/// # Errors
///
/// A read failure other than absence.
pub fn decide(
    target: &Utf8Path,
    recorded: Option<&Manifest>,
    projection: &Projection,
) -> Result<(Vec<Decision>, Vec<Collision>), RkError> {
    let mut decisions = Vec::new();
    let mut collisions: Vec<Collision> = projection
        .collisions
        .iter()
        .map(|collision| Collision {
            path: collision.destination.clone(),
            reason: collision.reason.clone(),
        })
        .collect();
    for candidate in &projection.candidates {
        let record = recorded.and_then(|record| record.file(&candidate.destination));
        let path = target.join(&candidate.destination);
        let present = match std::fs::symlink_metadata(path.as_std_path()) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(metadata) = &present
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            collisions.push(Collision {
                path: candidate.destination.clone(),
                reason: "exists and is not a regular file".to_owned(),
            });
            continue;
        }
        let action = match (candidate.placement, present.is_some(), record) {
            (_, false, _) => Action::Created,
            // A marked region lands into the target's document whether
            // the receipt names it or not: the bytes outside the markers
            // stay the target's, so nothing is taken from it.
            (Placement::Region { .. }, true, _) => Action::Replaced,
            (Placement::Whole, true, None) => {
                collisions.push(Collision {
                    path: candidate.destination.clone(),
                    reason: "exists, and no receipt attributes it to release-kit".to_owned(),
                });
                continue;
            }
            (Placement::Whole, true, Some(record)) => match (candidate.kind, record.kind) {
                (Kind::Rendered, Kind::Rendered) => Action::Replaced,
                (Kind::Rendered, Kind::Seeded | Kind::State) => {
                    collisions.push(Collision {
                        path: candidate.destination.clone(),
                        reason: format!(
                            "is recorded as {}, and this release renders it, so its bytes are the target's",
                            record.kind.as_str()
                        ),
                    });
                    continue;
                }
                (Kind::Seeded, _) => {
                    let bytes = std::fs::read(path.as_std_path())?;
                    if Digest::of(&bytes) == record.sha256 {
                        Action::Preserved
                    } else {
                        Action::Drift
                    }
                }
                (Kind::State, _) => Action::Preserved,
            },
        };
        decisions.push(Decision {
            destination: candidate.destination.clone(),
            kind: candidate.kind,
            action,
        });
    }
    if let Some(record) = recorded {
        for file in &record.files {
            let produced = projection
                .candidates
                .iter()
                .any(|candidate| candidate.destination == file.destination);
            if !produced {
                decisions.push(Decision {
                    destination: file.destination.clone(),
                    kind: file.kind,
                    action: Action::Released,
                });
            }
        }
    }
    collisions.sort_by(|a, b| a.path.cmp(&b.path));
    collisions.dedup_by(|a, b| a.path == b.path);
    Ok((decisions, collisions))
}

/// The one refusal for every collision, before any write.
///
/// The verbs offer no force flag: an unattributed file becomes landable
/// through the agent's migration alone, which brings the target to the
/// projection or records it through `rk adopt`.
#[must_use]
pub fn refusal(target: &Utf8Path, collisions: &[Collision]) -> RkError {
    let listed: Vec<String> = collisions
        .iter()
        .map(|collision| format!("{} ({})", collision.path, collision.reason))
        .collect();
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "these destinations cannot be landed as they stand, and nothing was written: {}",
                listed.join("; ")
            ),
        )
        .expected(
            "every whole-file destination absent or named by the receipt, and every marked document offering its block one place",
        )
        .action(format!(
            "rk stage --target {target} stages this binary's candidate for a byte comparison; the rk-setup skill carries the migration that brings each file to the candidate or records it, then re-run"
        ))
        .target_state("unchanged"),
    )
}

/// How a landing came to write its receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A first landing: files and receipt.
    Init,
    /// A landing over a receipt: files and receipt, the first landing's
    /// instant and origin preserved.
    Upgrade,
    /// A record of a target already at the projection: config and
    /// receipt only.
    Adopt,
}

/// What a landing completed.
#[derive(Debug)]
pub struct Landed {
    /// Every path written, in order, the config and the receipt included.
    pub completed: Vec<String>,
    /// Whether the configuration was written, or already held its bytes.
    pub config_written: bool,
    /// The receipt as written.
    pub receipt: Manifest,
}

/// Land `prepared` into `target`.
///
/// Refuse every collision first, open the target once under the lock the
/// caller holds, write the configuration where its bytes changed, then
/// each candidate by its decision, then the receipt.
///
/// The caller gathers under the same lock where it wants the evidence
/// and the writes to agree; [`prepare`] itself takes none, so a preview
/// holds nothing.
///
/// # Errors
///
/// The collision refusal at exit 73 with nothing written; the lock's
/// refusals; and [`RkError::Io`] for a write that fails, naming every
/// path completed before it, with the previous receipt left in place.
pub fn land(
    target: &Utf8Path,
    recorded: Option<&Manifest>,
    prepared: &Prepared,
    origin: Origin,
    _lock: &lock::TargetLock,
) -> Result<Landed, RkError> {
    if !prepared.collisions.is_empty() {
        return Err(refusal(target, &prepared.collisions));
    }
    let root = held::open_dir(target.as_std_path())?;
    // The proof's pause: validation is over and the target is held, so a
    // link swapped in from here on meets the held directory, not a path.
    held::pause(PAUSE_VAR, "validated", "proceed");
    let mut writer = Writer {
        root: &root,
        completed: Vec::new(),
        stop: std::env::var_os(INTERRUPT_VAR).map(|value| value.to_string_lossy().into_owned()),
    };
    // The configuration first, where the resolved answers changed.
    let config_current = read_relative(&root, config::CONFIG_PATH)?;
    let config_written = config_current.as_deref() != Some(prepared.config.content.as_bytes());
    if config_written {
        writer.write(config::CONFIG_PATH, prepared.config.content.as_bytes())?;
    }
    let mut files = Vec::new();
    for candidate in &prepared.projection.candidates {
        // An adoption verified every destination beforehand and writes
        // none: the receipt digests what stands. A landing writes by its
        // decision, and a candidate without one was a collision the
        // refusal above already named.
        let action = match origin {
            Origin::Adopt => Action::Preserved,
            Origin::Init | Origin::Upgrade => match prepared.decision(&candidate.destination) {
                Some(decision) => decision.action,
                None => continue,
            },
        };
        let sha256 = match action {
            Action::Preserved | Action::Drift => {
                // What the destination holds now is what the receipt
                // digests: the whole file, or the marked region alone.
                let current = read_relative(&root, &candidate.destination)?.unwrap_or_default();
                match candidate.placement {
                    Placement::Whole => Digest::of(&current),
                    Placement::Region { begin, end } => {
                        let text = String::from_utf8_lossy(&current);
                        super::extract_block(&text, begin, end)
                            .map_or_else(|| Digest::of(b""), |block| Digest::of(block.as_bytes()))
                    }
                }
            }
            Action::Created | Action::Replaced => {
                writer.write(&candidate.destination, &candidate.bytes)?;
                candidate_digest(candidate)
            }
            Action::Released => continue,
        };
        files.push(FileRecord {
            destination: candidate.destination.clone(),
            kind: candidate.kind,
            sha256,
            placement: match candidate.placement {
                Placement::Whole => manifest::Placement::Whole,
                Placement::Region { .. } => manifest::Placement::Region,
            },
        });
    }
    let receipt = receipt(&prepared.params, recorded, origin, files);
    writer.write(manifest::MANIFEST_PATH, &manifest::render(&receipt)?)?;
    Ok(Landed {
        completed: writer.completed,
        config_written,
        receipt,
    })
}

/// The digest the receipt carries for a written candidate: the whole
/// file, or the marked region alone.
fn candidate_digest(candidate: &Candidate) -> Digest {
    candidate
        .region
        .as_deref()
        .map_or_else(|| Digest::of(&candidate.bytes), Digest::of)
}

/// The receipt for this landing.
fn receipt(
    params: &Params,
    recorded: Option<&Manifest>,
    origin: Origin,
    files: Vec<FileRecord>,
) -> Manifest {
    Manifest {
        schema_version: manifest::SCHEMA_VERSION,
        rk_version: env!("CARGO_PKG_VERSION").to_owned(),
        origin: recorded.map_or_else(
            || match origin {
                Origin::Adopt => "adopt".to_owned(),
                Origin::Init | Origin::Upgrade => "init".to_owned(),
            },
            |record| record.origin.clone(),
        ),
        tech: params.tech().to_owned(),
        forge: params.forge().to_owned(),
        landed_at: recorded.map_or_else(manifest::now, |record| record.landed_at.clone()),
        parameters: Parameters {
            repo: params.repo().to_owned(),
            workflow: params.workflow(),
            style: params.style(),
            nix: params.nix(),
            trunk: params.trunk().to_owned(),
            line_prefix: params.line_prefix().to_owned(),
            security_contact: params.security_contact().to_owned(),
            security_response: params.security_response().to_owned(),
        },
        files,
        pins: crate::registry::pins_for(params.tech())
            .into_iter()
            .map(|pin| (pin.name, pin.version))
            .collect(),
    }
}

/// The bytes at a relative path below the held root, read through the
/// held directory chain, or `None` where nothing stands there.
fn read_relative(root: &File, relative: &str) -> std::io::Result<Option<Vec<u8>>> {
    let path = Path::new(relative);
    let (parent, name) = split(path)?;
    let dir = match held::hold_dir_existing(root, parent) {
        Ok(dir) => dir,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    held::read_file(&dir, name)
}

/// A relative path split into its parent and its file name.
fn split(path: &Path) -> std::io::Result<(&Path, &OsStr)> {
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other(format!("{} has no file name", path.display())))?;
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    Ok((parent, name))
}

/// The writes of one landing, each through the held root, with the
/// completed paths kept for the failure report.
struct Writer<'a> {
    root: &'a File,
    completed: Vec<String>,
    stop: Option<String>,
}

impl Writer<'_> {
    /// Write `bytes` at the relative `destination` through the held
    /// directory chain and a same-directory temporary file and rename.
    fn write(&mut self, destination: &str, bytes: &[u8]) -> Result<(), RkError> {
        let path = Path::new(destination);
        let (parent, name) = split(path)?;
        let outcome = held::hold_dir(self.root, parent).and_then(|dir| {
            if self.stop.as_deref() == Some(destination) {
                return Err(std::io::Error::other(
                    "the rename was stopped here for the proof",
                ));
            }
            held::write_file(&dir, name, bytes)
        });
        match outcome {
            Ok(()) => {
                self.completed.push(destination.to_owned());
                Ok(())
            }
            Err(error) => Err(self.failure(destination, bytes, &error)),
        }
    }

    /// The failure of a write that stopped the landing: the destination is
    /// observed again, because a remote filesystem may have completed a
    /// rename it reported as failed, and every completed path is named.
    /// The previous receipt stands; Git holds the diff; a rerun lands the
    /// rest.
    fn failure(&self, destination: &str, bytes: &[u8], error: &std::io::Error) -> RkError {
        let observed = match read_relative(self.root, destination) {
            Ok(None) => "is absent".to_owned(),
            Ok(Some(current)) if current == bytes => "holds the candidate bytes whole".to_owned(),
            Ok(Some(_)) => "holds its previous bytes whole".to_owned(),
            Err(again) => format!("could not be observed again: {again}"),
        };
        let completed = if self.completed.is_empty() {
            "none".to_owned()
        } else {
            self.completed.join(", ")
        };
        RkError::Io(std::io::Error::new(
            error.kind(),
            format!(
                "the landing stopped at {destination}: {error}; observed again, {destination} {observed}; the previous receipt stands; these landed before it: {completed}; re-run to land the rest"
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Origin, Prepared, decide, land, prepare};
    use crate::landing::manifest::{self, Manifest};
    use crate::landing::{Params, Style, lock};
    use crate::projection::{Projection, ProjectionInput, TargetEvidence};

    fn target() -> (tempfile::TempDir, camino::Utf8PathBuf) {
        let dir = tempfile::tempdir().expect("a scratch target exists");
        let path = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8");
        (dir, path)
    }

    fn params() -> Params {
        Params::for_test("acme/widget", Some(Style::Trunk))
    }

    fn prepared(target: &camino::Utf8Path, recorded: Option<&Manifest>) -> Prepared {
        prepare(target, recorded, &params(), None).expect("prepares")
    }

    fn landed(
        target: &camino::Utf8Path,
        recorded: Option<&Manifest>,
        origin: Origin,
    ) -> super::Landed {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let lock = lock::acquire_in(locks.path(), target).expect("the target is taken");
        land(target, recorded, &prepared(target, recorded), origin, &lock).expect("lands")
    }

    /// A fresh target: every candidate is created, the receipt is written
    /// last at schema 7, and a rerun replaces the rendered files and
    /// preserves the seeded ones from the receipt alone.
    #[test]
    fn a_fresh_landing_creates_and_a_rerun_decides_by_the_receipt() {
        let (_dir, target) = target();
        let first = prepared(&target, None);
        assert!(first.collisions.is_empty(), "{:?}", first.collisions);
        assert!(
            first
                .decisions
                .iter()
                .all(|decision| decision.action == Action::Created)
        );
        let outcome = landed(&target, None, Origin::Init);
        assert_eq!(
            outcome.completed.last().map(String::as_str),
            Some(manifest::MANIFEST_PATH)
        );
        assert_eq!(outcome.receipt.schema_version, 7);
        let record = manifest::load(&target).expect("loads").expect("exists");
        let again = prepared(&target, Some(&record));
        for decision in &again.decisions {
            let expected = match decision.kind {
                crate::landing::Kind::Rendered => Action::Replaced,
                crate::landing::Kind::Seeded | crate::landing::Kind::State => Action::Preserved,
            };
            assert_eq!(decision.action, expected, "{}", decision.destination);
        }
    }

    /// A whole file the receipt does not name, a non-regular entry, and a
    /// document with a doubled marker are all collected in one pass, and
    /// the refusal writes nothing.
    #[test]
    fn every_collision_is_collected_and_the_refusal_writes_nothing() {
        let (_dir, target) = target();
        std::fs::write(target.join("SECURITY.md"), "ours\n").expect("writes");
        std::fs::create_dir_all(target.join("release-plz.toml")).expect("creates");
        std::fs::write(
            target.join("AGENTS.md"),
            format!(
                "{b}\n{e}\n{b}\n{e}\n",
                b = crate::landing::BLOCK_BEGIN,
                e = crate::landing::BLOCK_END
            ),
        )
        .expect("writes");
        let prepared = prepared(&target, None);
        let paths: Vec<&str> = prepared
            .collisions
            .iter()
            .map(|collision| collision.path.as_str())
            .collect();
        assert_eq!(paths, ["AGENTS.md", "SECURITY.md", "release-plz.toml"]);
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let lock = lock::acquire_in(locks.path(), &target).expect("the target is taken");
        let refused =
            land(&target, None, &prepared, Origin::Init, &lock).expect_err("the landing refuses");
        assert_eq!(refused.exit_code(), 73);
        assert!(!target.join(".release-kit").exists());
        assert!(!target.join("dist-workspace.toml").exists());
    }

    /// A recorded destination the projection stops producing is released:
    /// on disk, named, and out of the receipt.
    #[test]
    fn a_released_destination_stays_and_leaves_the_receipt() {
        let (_dir, target) = target();
        landed(&target, None, Origin::Init);
        let mut record = manifest::load(&target).expect("loads").expect("exists");
        std::fs::write(target.join("legacy.yml"), "old\n").expect("writes");
        record.files.push(manifest::FileRecord {
            destination: "legacy.yml".into(),
            kind: crate::landing::Kind::Rendered,
            sha256: crate::digest::Digest::of(b"old\n"),
            placement: manifest::Placement::Whole,
        });
        let (decisions, _) = decide(
            &target,
            Some(&record),
            &Projection::compute(&ProjectionInput {
                params: params(),
                evidence: TargetEvidence::gather(&target, Some(&record)).expect("gathers"),
            })
            .expect("projects"),
        )
        .expect("decides");
        let released = decisions
            .iter()
            .find(|decision| decision.destination == "legacy.yml")
            .expect("the released destination is decided");
        assert_eq!(released.action, Action::Released);
        let outcome = landed(&target, Some(&record), Origin::Upgrade);
        assert!(target.join("legacy.yml").is_file());
        assert!(outcome.receipt.file("legacy.yml").is_none());
    }
}
