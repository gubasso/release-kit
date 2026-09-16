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
    /// A whole-file destination the receipt does not name already holds
    /// the candidate's bytes: nothing is written, and the receipt records
    /// it, because replacing identical bytes changes nothing and a run
    /// stopped after creating it must be rerunnable.
    Matched,
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
            Self::Matched => "matched",
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

/// One target directory held open for a whole landing verb.
///
/// Opened once, right after the lock under an apply and before any
/// evidence is gathered, and carried through every decision read and
/// every write, so a target root exchanged under the pathname after
/// validation receives nothing: the descriptor names the directory that
/// was validated, whatever its path has since become.
#[derive(Debug)]
pub struct Held {
    root: File,
    base: camino::Utf8PathBuf,
    display: camino::Utf8PathBuf,
}

impl Held {
    /// Hold `target`, following no link at its final component.
    ///
    /// # Errors
    ///
    /// The open failure, and a kernel link that is not UTF-8.
    pub fn open(target: &Utf8Path) -> Result<Self, RkError> {
        let root = held::open_dir(target.as_std_path())?;
        let base = camino::Utf8PathBuf::from_path_buf(held::proc_path(&root))
            .map_err(|path| anyhow::anyhow!("the kernel's link {} is not UTF-8", path.display()))?;
        Ok(Self {
            root,
            base,
            display: target.to_owned(),
        })
    }

    /// Hold `target` under `lock`: the directory opened must be the one
    /// the lock key was derived from, or the target was exchanged between
    /// the two steps and the landing refuses before it reads anything.
    ///
    /// # Errors
    ///
    /// As [`Self::open`], and a `state-drift` refusal naming the exchange.
    pub fn open_locked(target: &Utf8Path, lock: &lock::TargetLock) -> Result<Self, RkError> {
        let held = Self::open(target)?;
        let opened = held::Identity::of(&held.root.metadata()?);
        if lock.identity() != opened {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!(
                        "the directory at {target} was exchanged after the lock was taken, and nothing was written"
                    ),
                )
                .expected("one directory at the target path from the lock through the receipt write")
                .action("re-run once the target is at rest")
                .target_state("unchanged"),
            ));
        }
        Ok(held)
    }

    /// The path every read of this target goes through: the kernel's
    /// link to the held directory.
    #[must_use]
    pub fn base(&self) -> &Utf8Path {
        &self.base
    }

    /// The path the operator named, for reports.
    #[must_use]
    pub fn display(&self) -> &Utf8Path {
        &self.display
    }
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
    target: &Held,
    recorded: Option<&Manifest>,
    params: &Params,
    existing_config: Option<&config::Config>,
) -> Result<Prepared, RkError> {
    let evidence = TargetEvidence::gather(target.base(), recorded)?;
    let projection = Projection::compute(&ProjectionInput {
        params: params.clone(),
        evidence,
    })?;
    let (decisions, collisions) = decide(target, recorded, &projection)?;
    let config = config::Plan::new(
        target.base().as_std_path(),
        params,
        existing_config,
        recorded,
    )?;
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
/// Every existing parent component of a candidate is walked relative to
/// the held target directory with no link followed, so a linked or
/// non-directory component is a collision here, before any write, and
/// not a failure after the configuration landed.
///
/// # Errors
///
/// A read failure other than absence.
pub fn decide(
    target: &Held,
    recorded: Option<&Manifest>,
    projection: &Projection,
) -> Result<(Vec<Decision>, Vec<Collision>), RkError> {
    let root = &target.root;
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
        let located = match locate(root, &candidate.destination)? {
            Located::Collision(reason) => {
                collisions.push(Collision {
                    path: candidate.destination.clone(),
                    reason,
                });
                continue;
            }
            other => other,
        };
        let present = matches!(located, Located::Present { .. });
        let current = || located.read();
        let action = match (candidate.placement, present, record) {
            (_, false, _) => Action::Created,
            // A marked region lands into the target's document whether
            // the receipt names it or not: the bytes outside the markers
            // stay the target's, so nothing is taken from it.
            (Placement::Region { .. }, true, _) => Action::Replaced,
            // An unrecorded whole file holding the candidate's bytes is
            // attributed by its content: a run stopped after creating it
            // leaves exactly this, and replacing identical bytes changes
            // nothing. Differing bytes are the target's, and refuse.
            (Placement::Whole, true, None) => {
                if current()? == candidate.bytes {
                    Action::Matched
                } else {
                    collisions.push(Collision {
                        path: candidate.destination.clone(),
                        reason:
                            "exists with bytes differing from the candidate, and no receipt attributes it to release-kit"
                                .to_owned(),
                    });
                    continue;
                }
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
                    if Digest::of(&current()?) == record.sha256 {
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

/// What stands at a destination inside the held target.
enum Located {
    /// The parent chain or the final component cannot be landed through:
    /// a linked or non-directory parent, or a non-regular entry.
    Collision(String),
    /// Nothing stands there.
    Absent,
    /// A regular file stands there, inside its held parent.
    Present { dir: File, name: std::ffi::OsString },
}

impl Located {
    /// The bytes present, empty where nothing stands.
    fn read(&self) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Present { dir, name } => {
                held::read_file(dir, name).map(Option::unwrap_or_default)
            }
            Self::Absent | Self::Collision(_) => Ok(Vec::new()),
        }
    }
}

/// Locate `destination` inside the held `root`: the parent chain is held
/// first, following no link, and the final component is then examined
/// inside the held parent.
fn locate(root: &File, destination: &str) -> std::io::Result<Located> {
    let (parent, name) = split(Path::new(destination))?;
    let dir = match held::hold_dir_existing(root, parent) {
        Ok(dir) => dir,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Located::Absent),
        Err(error) => {
            return Ok(Located::Collision(format!(
                "a parent component cannot be held: {error}"
            )));
        }
    };
    match std::fs::symlink_metadata(held::proc_path(&dir).join(name)) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Ok(
            Located::Collision("exists and is not a regular file".to_owned()),
        ),
        Ok(_) => Ok(Located::Present {
            dir,
            name: name.to_owned(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Located::Absent),
        Err(error) => Err(error),
    }
}

/// The refusal a licence condition answers, before any write.
///
/// A landing that guessed past it would write a workflow whose provider's
/// terms the target's own licence does not permit, which is a licence
/// violation this convention does not commit on a target's behalf.
#[must_use]
pub fn licence_refusal(reason: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!("the code scanning provider's licence condition is not satisfied, and nothing was written: {reason}"),
        )
        .expected("a provider whose terms the target's declared licence permits")
        .action("pass --code-scanning semgrep, which carries no licence condition, or --code-scanning off")
        .target_state("unchanged"),
    )
}

/// The refusal a selected release automation this release cannot land
/// answers, before any write: silently omitting the declared release would
/// write a record that claims an automation nothing landed.
///
/// SATISFIES project-profile:an-operation-refuses-only-what-it-requires
#[must_use]
pub fn release_unavailable(reason: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!("the selected release automation is not available in this release, and nothing was written: {reason}"),
        )
        .expected("a release driver and forge this release ships automation for, or a release mode that selects none")
        .action("pass --release-driver and --forge at an available tuple, or --release-mode external or none")
        .target_state("unchanged"),
    )
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
            "every whole-file destination absent, named by the receipt, or already holding the candidate's bytes, every parent component a directory reached through no link, and every marked document offering its block one place",
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
    target: &Held,
    recorded: Option<&Manifest>,
    prepared: &Prepared,
    origin: Origin,
    _lock: &lock::TargetLock,
) -> Result<Landed, RkError> {
    if let Some(reason) = prepared.projection.licence_refusal.as_deref() {
        return Err(licence_refusal(reason));
    }
    if let Some(reason) = prepared.projection.release_unavailable() {
        return Err(release_unavailable(reason));
    }
    if !prepared.collisions.is_empty() {
        return Err(refusal(target.display(), &prepared.collisions));
    }
    let root = &target.root;
    // The proof's pause: validation is over and the target is held, so a
    // link or a directory swapped in under the pathname from here on
    // meets the held descriptor, not the path.
    held::pause(PAUSE_VAR, "validated", "proceed");
    // Every destination the landing does not write is read again through
    // the held directory before the first write: a preserved or matched
    // file must still stand, and an adopted value must still equal the
    // candidate, or the landing refuses with the old receipt intact.
    let unwritten = reverify(root, prepared, origin)?;
    let mut writer = Writer {
        root,
        completed: Vec::new(),
        stop: std::env::var_os(INTERRUPT_VAR).map(|value| value.to_string_lossy().into_owned()),
    };
    // The configuration first, where the resolved answers changed.
    let config_current = read_relative(root, config::CONFIG_PATH)?;
    let config_written = config_current.as_deref() != Some(prepared.config.content.as_bytes());
    if config_written {
        writer.write(config::CONFIG_PATH, prepared.config.content.as_bytes())?;
    }
    let mut files = Vec::new();
    for candidate in &prepared.projection.candidates {
        let sha256 = match unwritten.get(&candidate.destination) {
            Some(digest) => digest.clone(),
            None => match action_of(prepared, origin, &candidate.destination) {
                Some(Action::Created | Action::Replaced) => {
                    writer.write(&candidate.destination, &candidate.bytes)?;
                    candidate_digest(candidate)
                }
                _ => continue,
            },
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
    let receipt = receipt(
        &prepared.params,
        &prepared.projection,
        recorded,
        origin,
        files,
    );
    writer.write(manifest::MANIFEST_PATH, &manifest::render(&receipt)?)?;
    Ok(Landed {
        completed: writer.completed,
        config_written,
        receipt,
    })
}

/// What the landing does with one destination under `origin`: an
/// adoption preserves everything it verified; a landing follows its
/// decision, and a candidate without one was a collision the refusal
/// already named.
fn action_of(prepared: &Prepared, origin: Origin, destination: &str) -> Option<Action> {
    match origin {
        Origin::Adopt => Some(Action::Preserved),
        Origin::Init | Origin::Upgrade => prepared.decision(destination).map(|d| d.action),
    }
}

/// The recorded form of what a destination holds now, read through the
/// held directory: the whole file, or the marked region alone; `None`
/// where the file, or the region, is absent.
fn current_form(root: &File, candidate: &Candidate) -> Result<Option<Vec<u8>>, RkError> {
    let Some(current) = read_relative(root, &candidate.destination)? else {
        return Ok(None);
    };
    Ok(match candidate.placement {
        Placement::Whole => Some(current),
        Placement::Region { begin, end } => {
            let text = String::from_utf8_lossy(&current);
            super::extract_block(&text, begin, end).map(|block| block.as_bytes().to_vec())
        }
    })
}

/// Read every destination the landing leaves unwritten again, through
/// the held directory, and digest it for the receipt: a preserved,
/// drifted, or matched file must still be present, and a matched or
/// adopted rendered value must still equal the candidate.
///
/// # Errors
///
/// A `state-drift` refusal naming the destination that moved since the
/// decision, with nothing written; and any read failure.
fn reverify(
    root: &File,
    prepared: &Prepared,
    origin: Origin,
) -> Result<std::collections::BTreeMap<String, Digest>, RkError> {
    let mut digests = std::collections::BTreeMap::new();
    for candidate in &prepared.projection.candidates {
        let action = action_of(prepared, origin, &candidate.destination);
        let must_match = match (origin, action) {
            (Origin::Adopt, _) => candidate.kind == Kind::Rendered,
            (_, Some(Action::Matched)) => true,
            (_, Some(Action::Preserved | Action::Drift)) => false,
            _ => continue,
        };
        let Some(current) = current_form(root, candidate)? else {
            return Err(moved(&candidate.destination, "is no longer present"));
        };
        let expected: &[u8] = candidate.region.as_deref().unwrap_or(&candidate.bytes);
        if must_match && current != expected {
            return Err(moved(
                &candidate.destination,
                "no longer holds the candidate's bytes",
            ));
        }
        digests.insert(candidate.destination.clone(), Digest::of(&current));
    }
    Ok(digests)
}

/// The refusal for a destination that changed between the decision and
/// the first write.
fn moved(destination: &str, what: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "{destination} {what} since it was validated, and nothing was written; the previous receipt stands"
            ),
        )
        .expected("every destination the landing leaves as it stands to stand still until the receipt is written")
        .action("re-run once the target is at rest")
        .target_state("unchanged"),
    )
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
    projection: &Projection,
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
        landed_at: recorded.map_or_else(manifest::now, |record| record.landed_at.clone()),
        profile: params.profile().clone(),
        git: params.git().clone(),
        capabilities: params.capabilities().clone(),
        parameters: Parameters {
            repo: params.repo().to_owned(),
            security_contact: params.security_contact().to_owned(),
            security_response: params.security_response().to_owned(),
            required_check: params.required_check().to_owned(),
            required_workflow: params.required_workflow().to_owned(),
        },
        files,
        pins: crate::registry::pins_for(&projection.capabilities)
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
    use super::{Action, Held, Origin, Prepared, decide, land, prepare};
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
        prepare(
            &Held::open(target).expect("opens"),
            recorded,
            &params(),
            None,
        )
        .expect("prepares")
    }

    fn landed(
        target: &camino::Utf8Path,
        recorded: Option<&Manifest>,
        origin: Origin,
    ) -> super::Landed {
        let locks = tempfile::tempdir().expect("a scratch locks directory exists");
        let lock = lock::acquire_in(locks.path(), target).expect("the target is taken");
        let held = Held::open(target).expect("opens");
        land(&held, recorded, &prepared(target, recorded), origin, &lock).expect("lands")
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
        assert_eq!(outcome.receipt.schema_version, manifest::SCHEMA_VERSION);
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
        let held = Held::open(&target).expect("opens");
        let refused =
            land(&held, None, &prepared, Origin::Init, &lock).expect_err("the landing refuses");
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
            &Held::open(&target).expect("opens"),
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
