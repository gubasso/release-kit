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
    /// A recorded marked region, or a recorded rendered whole file whose
    /// bytes the record vouches for and the candidate changes, is
    /// rewritten from the candidate.
    Replaced,
    /// A whole-file destination already holds the candidate's bytes:
    /// nothing is written, and the receipt records it. A recorded rendered
    /// file the landing leaves byte for byte is matched, not replaced, and
    /// an unrecorded one is attributed by its content, because a run
    /// stopped after creating it must be rerunnable.
    Matched,
    /// A recorded seeded or state file stays as it is, its current digest
    /// entering the receipt.
    Preserved,
    /// A recorded seeded file stays and its bytes differ from the receipt:
    /// the target tuned it, which is what a seeded file is for.
    Drift,
    /// A recorded whole rendered file this binary no longer produces,
    /// whose bytes the record still vouches for: removed, and out of the
    /// new receipt.
    Retired,
    /// A recorded destination this binary no longer produces that the
    /// target owns: a seeded or state file, a marked region, a rendered
    /// file someone edited, or one the target's own state withholds. Left
    /// on disk, target-owned from this landing, out of the new receipt.
    Dropped,
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
            Self::Retired => "retired",
            Self::Dropped => "dropped",
        }
    }
}

/// What an adoption finds at one candidate's destination.
///
/// It is judged on the bytes alone. `rk adopt` decides from it, and
/// `rk stage compare` takes the word of a target the adoption lands from
/// it, so the comparison is the adoption's own decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adoption {
    /// The destination holds the candidate's bytes.
    Matches,
    /// A seeded file the target tuned: it stays, target-owned.
    Tuned,
    /// A state file: it stays as the target left it.
    State,
    /// A rendered destination whose bytes differ from the candidate.
    Differs,
    /// The destination is absent, or carries no release-kit block where
    /// `exists`.
    Missing {
        /// Whether a file stands at the destination.
        exists: bool,
    },
    /// Something other than a regular file stands at the destination.
    Irregular,
}

impl Adoption {
    /// The comparison word, in the landing's vocabulary and with the
    /// meaning the method gives it: a kept seeded or state file is
    /// `preserved`, and whatever the adoption refuses is a `collision`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Matches => "matched",
            Self::Tuned | Self::State => "preserved",
            Self::Differs | Self::Missing { .. } | Self::Irregular => "collision",
        }
    }

    /// Whether the adoption accepts `--replace` for a destination of
    /// `kind` judged so: a rendered destination that differs or is
    /// missing. Every other collision refuses whatever is named.
    #[must_use]
    pub fn replaceable(self, kind: Kind) -> bool {
        matches!(self, Self::Differs)
            || (matches!(self, Self::Missing { .. }) && kind == Kind::Rendered)
    }
}

/// Judge one candidate's destination as an adoption does.
///
/// # Errors
///
/// [`RkError::Io`] for a read that fails for a reason other than
/// absence.
pub fn judge_adoption(target: &Held, candidate: &Candidate) -> Result<Adoption, RkError> {
    // The landing's own walk: a linked or non-directory parent, a link at
    // the destination, dangling or not, and any non-regular entry are
    // irregular, because the write would refuse to follow them.
    match locate(&target.root, &candidate.destination)? {
        Located::Collision(_) => return Ok(Adoption::Irregular),
        Located::Absent => return Ok(Adoption::Missing { exists: false }),
        Located::Present { .. } => {}
    }
    let Some(current) = super::read_recorded(target.base(), &candidate.destination)? else {
        return Ok(Adoption::Missing { exists: true });
    };
    let expected: &[u8] = match candidate.placement {
        Placement::Whole => &candidate.bytes,
        Placement::Region { .. } => candidate.region.as_deref().unwrap_or(&candidate.bytes),
    };
    Ok(match candidate.kind {
        Kind::Rendered | Kind::Seeded if current == expected => Adoption::Matches,
        Kind::Rendered => Adoption::Differs,
        Kind::Seeded => Adoption::Tuned,
        Kind::State => Adoption::State,
    })
}

/// A rendered file the landing preserves under an accepted-drift entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    /// Why the project deviates, from its entry.
    pub reason: String,
    /// Whether the candidate for the destination changed since the
    /// previous receipt, which tells the project its deviation may be
    /// droppable.
    pub candidate_moved: bool,
    /// The digest of the accepted bytes, which the landing checks again
    /// through the held directory before its first write.
    pub sha256: String,
}

/// One decided destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The destination, relative to the target.
    pub destination: String,
    /// The kind the candidate declares, or the recorded kind for a
    /// destination this binary no longer produces.
    pub kind: Kind,
    /// What happens to it.
    pub action: Action,
    /// The accepted drift a preserved rendered file stays under.
    pub accepted: Option<Accepted>,
}

/// One destination the landing refuses before any write.
///
/// It is a whole-file destination present on disk with no receipt entry
/// attributing it, a recorded rendered file or region whose bytes the
/// record no longer vouches for, or a document whose markers offer the
/// block no place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Collision {
    /// The destination.
    pub path: String,
    /// Why it refuses.
    pub reason: String,
    /// Whether it is a conflict `--replace` may name: a regular file whose
    /// bytes a landing could save and overwrite. A defective marker or a
    /// linked parent is no conflict, and only a fix to the target clears
    /// it.
    pub replaceable: bool,
}

/// One conflict the operator named, replaced after its bytes were saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Replaced {
    /// The destination.
    pub path: String,
    /// Where its bytes were saved; absent in a preview, which saves
    /// nothing.
    pub saved_to: Option<String>,
}

/// The reason a recorded rendered file reads as a conflict.
const EDITED: &str = "was edited after the landing, and the record no longer vouches for its bytes";

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
/// projection order with the dropped and retired destinations after them, every
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
    /// The conflicts the operator named, which the landing saves and then
    /// replaces.
    pub replacements: Vec<String>,
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
    super::accepted_drift_faults(&projection, recorded, existing_config)?;
    let accepted_drift = existing_config
        .map(|config| config.landing.accepted_drift.clone())
        .unwrap_or_default();
    let (decisions, collisions) = decide(target, recorded, &projection, &accepted_drift)?;
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
        replacements: Vec::new(),
    })
}

/// Turn every conflict `replace` names into a replacement, and refuse a
/// name that is no conflict.
///
/// A conflict is an unattributed collision or a recorded rendered file
/// whose bytes the record no longer vouches for. Every conflict the
/// operator did not name stays a collision, and the landing still
/// refuses it before the first write; a name that matches no conflict is
/// a usage error, because a replace the operator believes happened and
/// did not is worse than a refusal.
///
/// # Errors
///
/// [`RkError::Usage`] naming the first path that is no conflict.
pub fn name_replacements(prepared: &mut Prepared, replace: &[String]) -> Result<(), RkError> {
    for path in replace {
        if prepared.replacements.contains(path) {
            continue;
        }
        let Some(index) = prepared
            .collisions
            .iter()
            .position(|collision| collision.path == *path && collision.replaceable)
        else {
            return Err(RkError::Usage(format!(
                "--replace {path} names no conflict this landing has; a conflict is an unattributed file or a recorded rendered file someone edited"
            )));
        };
        prepared.collisions.remove(index);
        let kind = prepared
            .projection
            .candidates
            .iter()
            .find(|candidate| candidate.destination == *path)
            .map_or(Kind::Rendered, |candidate| candidate.kind);
        prepared.decisions.push(Decision {
            destination: path.clone(),
            kind,
            action: Action::Replaced,
            accepted: None,
        });
        prepared.replacements.push(path.clone());
    }
    let order = |destination: &str| {
        prepared
            .projection
            .candidates
            .iter()
            .position(|candidate| candidate.destination == destination)
            .unwrap_or(usize::MAX)
    };
    let mut decisions = std::mem::take(&mut prepared.decisions);
    decisions.sort_by_key(|decision| order(&decision.destination));
    prepared.decisions = decisions;
    Ok(())
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
#[allow(
    clippy::too_many_lines,
    reason = "one pass decides every destination by its placement, its record, and its bytes"
)]
pub fn decide(
    target: &Held,
    recorded: Option<&Manifest>,
    projection: &Projection,
    accepted_drift: &std::collections::BTreeMap<String, config::AcceptedDrift>,
) -> Result<(Vec<Decision>, Vec<Collision>), RkError> {
    let root = &target.root;
    let mut decisions = Vec::new();
    let mut collisions: Vec<Collision> = projection
        .collisions
        .iter()
        .map(|collision| Collision {
            path: collision.destination.clone(),
            reason: collision.reason.clone(),
            replaceable: false,
        })
        .collect();
    for candidate in &projection.candidates {
        let record = recorded.and_then(|record| record.file(&candidate.destination));
        let located = match locate(root, &candidate.destination)? {
            Located::Collision(reason) => {
                collisions.push(Collision {
                    path: candidate.destination.clone(),
                    reason,
                    replaceable: false,
                });
                continue;
            }
            other => other,
        };
        let present = matches!(located, Located::Present { .. });
        let current = || located.read();
        let mut accepted = None;
        let action = match (candidate.placement, present, record) {
            (_, false, _) => Action::Created,
            // A recorded region someone edited is a conflict like an
            // edited rendered file: the record no longer vouches for it,
            // and it does not already hold the candidate's region, which
            // is what a landing stopped part way leaves behind.
            (Placement::Region { begin, end }, true, Some(record))
                if record.kind == Kind::Rendered
                    && super::extract_block(&String::from_utf8_lossy(&current()?), begin, end)
                        .is_some_and(|block| {
                            Digest::of(block.as_bytes()) != record.sha256
                                && candidate.region.as_deref() != Some(block.as_bytes())
                        }) =>
            {
                collisions.push(Collision {
                    path: candidate.destination.clone(),
                    reason: format!("its marked region {EDITED}"),
                    replaceable: true,
                });
                continue;
            }
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
                        replaceable: true,
                    });
                    continue;
                }
            }
            (Placement::Whole, true, Some(record)) => match (candidate.kind, record.kind) {
                // The record vouches for the bytes on disk, so they are
                // release-kit's own; bytes it no longer vouches for are an
                // edit, which only a named replace overwrites.
                (Kind::Rendered, Kind::Rendered) => {
                    let bytes = current()?;
                    // Bytes equal to the candidate need no write, whether
                    // the record vouches for them or a landing stopped part
                    // way wrote them: the file is matched, so the report
                    // tells an untouched file from a rewritten one.
                    let entry = accepted_drift
                        .get(&candidate.destination)
                        .filter(|entry| Digest::of(&bytes).to_string() == entry.sha256);
                    if bytes == candidate.bytes {
                        Action::Matched
                    } else if Digest::of(&bytes) == record.sha256 {
                        Action::Replaced
                    } else if let Some(entry) = entry {
                        // An accepted deviation stays on disk: the receipt
                        // keeps the candidate's digest, so it still says
                        // what the tool would write.
                        accepted = Some(Accepted {
                            reason: entry.reason.clone(),
                            candidate_moved: Digest::of(&candidate.bytes) != record.sha256,
                            sha256: entry.sha256.clone(),
                        });
                        Action::Preserved
                    } else {
                        collisions.push(Collision {
                            path: candidate.destination.clone(),
                            reason: EDITED.to_owned(),
                            replaceable: true,
                        });
                        continue;
                    }
                }
                (Kind::Rendered, Kind::Seeded | Kind::State) => {
                    collisions.push(Collision {
                        path: candidate.destination.clone(),
                        reason: format!(
                            "is recorded as {}, and this release renders it, so its bytes are the target's",
                            record.kind.as_str()
                        ),
                        replaceable: true,
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
            accepted,
        });
    }
    if let Some(record) = recorded {
        decisions.extend(dropped(target, record, projection)?);
    }
    collisions.sort_by(|a, b| a.path.cmp(&b.path));
    collisions.dedup_by(|a, b| a.path == b.path);
    Ok((decisions, collisions))
}

/// Decide every recorded destination `projection` no longer produces.
///
/// A whole rendered file whose bytes the record still vouches for is
/// `retired`, and a landing removes it: release-kit wrote those bytes and
/// nobody changed them, and a workflow left behind keeps running beside
/// its successor. One already absent is retired too, because nothing of
/// it remains. Every other such destination is `dropped` and stays on
/// disk as the target's own: a seeded or state file, whose bytes are the
/// target's by kind, a marked region inside the target's document, a
/// rendered file someone edited, an entry that is no regular file, and a
/// destination the target's own state withholds. Each leaves the receipt.
///
/// `rk stage`, `rk stage compare`, the upgrade preview, and the apply all
/// decide through this one function.
///
/// SATISFIES landing:a-dropped-file-stays
///
/// # Errors
///
/// A read of the held target that fails for a reason other than absence.
pub fn dropped(
    target: &Held,
    recorded: &Manifest,
    projection: &Projection,
) -> Result<Vec<Decision>, RkError> {
    let mut decisions = Vec::new();
    for file in &recorded.files {
        let produced = projection
            .candidates
            .iter()
            .any(|candidate| candidate.destination == file.destination);
        if produced {
            continue;
        }
        let withheld = projection
            .omissions
            .iter()
            .any(|omission| omission.destination == file.destination);
        let retirable = !withheld
            && file.kind == Kind::Rendered
            && file.placement == manifest::Placement::Whole;
        let action = if retirable && unedited(&target.root, file)? {
            Action::Retired
        } else {
            Action::Dropped
        };
        decisions.push(Decision {
            destination: file.destination.clone(),
            kind: file.kind,
            action,
            accepted: None,
        });
    }
    Ok(decisions)
}

/// Whether a recorded whole file is absent, or a regular file holding the
/// bytes the record vouches for.
fn unedited(root: &File, file: &FileRecord) -> std::io::Result<bool> {
    Ok(match locate(root, &file.destination)? {
        Located::Absent => true,
        Located::Collision(_) => false,
        present @ Located::Present { .. } => Digest::of(&present.read()?) == file.sha256,
    })
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

/// The refusal a project the selected release driver cannot release
/// answers, before any write: a python project with a dynamic version or
/// no `uv.lock` would land a release that fails at its first bump.
///
/// SATISFIES project-profile:release-intent-has-three-modes
#[must_use]
pub fn driver_refusal(reason: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!("the project cannot take the selected release driver, and nothing was written: {reason}"),
        )
        .expected("a uv project with a static [project] version and a committed uv.lock")
        .action("uv lock, or a static version in [project], then land again")
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
/// No flag forces past a collision. Only a named `--replace`, which saves
/// a copy first, passes a conflict, and a defective marker or a linked
/// parent only a fix to the target clears.
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
            "rk stage --target {target}, then rk stage compare <stage> names each conflict, and the rk-setup skill asks the operator about each one; --replace <path> saves a copy of a named conflict and replaces it, then re-run"
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

/// One stored secret whose reading moved when a landing replaced a GitHub
/// workflow: the replaced file read it and the rendered file does not, or
/// the reverse.
///
/// SATISFIES landing:ownership-is-elementary
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct SecretMove {
    /// The workflow, relative to the target.
    pub path: String,
    /// The secret's name.
    pub name: String,
    /// `dropped` where only the replaced file read it, `new` where only
    /// the rendered file reads it.
    pub direction: String,
    /// The setup step that stores a new secret, where one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
}

impl SecretMove {
    /// The human line.
    #[must_use]
    pub fn line(&self) -> String {
        if self.direction == "dropped" {
            format!(
                "secret {} read by the replaced {} is not read by the rendered file",
                self.name, self.path
            )
        } else {
            self.step.as_deref().map_or_else(
                || {
                    format!(
                        "secret {} is new; no setup step stores it, so the operator does",
                        self.name
                    )
                },
                |step| format!("secret {} is new; {step} stores it", self.name),
            )
        }
    }
}

/// The stored secrets a workflow text reads: every `secrets.NAME` inside a
/// `${{ }}` expression, with or without spaces. `GITHUB_TOKEN` is the
/// forge's own and no stored secret.
#[must_use]
pub fn secret_names(text: &str) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find("${{") {
        let Some(end) = rest[start..].find("}}") else {
            break;
        };
        let expression = &rest[start + 3..start + end];
        let mut inner = expression;
        while let Some(at) = inner.find("secrets.") {
            let name: String = inner[at + "secrets.".len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && name != "GITHUB_TOKEN" {
                names.insert(name);
            }
            inner = &inner[at + "secrets.".len()..];
        }
        rest = &rest[start + end + 2..];
    }
    names
}

/// The secrets that move when the workflow at `path` goes from `old` to
/// `new`: each name `old` reads and `new` does not, then each name `new`
/// reads and `old` did not, with the step that stores it.
#[must_use]
pub fn secrets_moved(path: &str, old: &str, new: &str) -> Vec<SecretMove> {
    let (before, after) = (secret_names(old), secret_names(new));
    let dropped = before.difference(&after).map(|name| SecretMove {
        path: path.to_owned(),
        name: name.clone(),
        direction: "dropped".to_owned(),
        step: None,
    });
    let added = after.difference(&before).map(|name| SecretMove {
        path: path.to_owned(),
        name: name.clone(),
        direction: "new".to_owned(),
        step: crate::setup::steps::secret_step(name).map(str::to_owned),
    });
    dropped.chain(added).collect()
}

/// The secrets every workflow this landing replaces would move: each
/// destination under `.github/workflows/` the landing replaces, read
/// before the write and set beside its candidate.
///
/// # Errors
///
/// A read of the held target that fails for a reason other than absence.
pub fn replaced_secrets(target: &Held, prepared: &Prepared) -> Result<Vec<SecretMove>, RkError> {
    let mut moved = Vec::new();
    for decision in &prepared.decisions {
        if decision.action != Action::Replaced
            || !decision.destination.starts_with(".github/workflows/")
        {
            continue;
        }
        let Some(candidate) = prepared
            .projection
            .candidates
            .iter()
            .find(|candidate| candidate.destination == decision.destination)
        else {
            continue;
        };
        let Some(current) = read_relative(&target.root, &decision.destination)? else {
            continue;
        };
        moved.extend(secrets_moved(
            &decision.destination,
            &String::from_utf8_lossy(&current),
            &String::from_utf8_lossy(&candidate.bytes),
        ));
    }
    Ok(moved)
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
    /// Every conflict replaced, with where its bytes were saved.
    pub replaced: Vec<Replaced>,
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
    if let Some(reason) = prepared.projection.driver_refusal.as_deref() {
        return Err(driver_refusal(reason));
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
    // Every file the landing retires must still hold the bytes the record
    // vouches for, or it became the target's after the decision.
    let retiring = retiring(root, recorded, prepared)?;
    // Every named conflict is saved before the first write, so a failed
    // save leaves the target exactly as it stood.
    let replaced = save_replacements(target, root, prepared)?;
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
        let accepted = prepared
            .decision(&candidate.destination)
            .is_some_and(|decision| decision.accepted.is_some());
        let sha256 = match unwritten.get(&candidate.destination) {
            // The receipt of an accepted deviation keeps the candidate's
            // digest, not the bytes on disk, so it goes on meaning what the
            // tool would write and the next landing preserves it again.
            Some(_) if accepted => candidate_digest(candidate),
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
    // The retired files go after every write and before the receipt, so
    // a stop between leaves the old receipt naming each one, and the
    // rerun finds it absent and retires it again.
    for destination in &retiring {
        writer.remove(destination)?;
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
        replaced,
    })
}

/// Save the bytes of every named conflict that stands on disk.
fn save_replacements(
    target: &Held,
    root: &File,
    prepared: &Prepared,
) -> Result<Vec<Replaced>, RkError> {
    if prepared.replacements.is_empty() {
        return Ok(Vec::new());
    }
    // Every run claims its own state-root snapshot, an open migration's
    // included: nothing reads a saved copy back, and a fresh claim never
    // collides with an earlier run's copy of the same path.
    let canonical = std::fs::canonicalize(target.display().as_std_path())?;
    let snapshot = crate::snapshot::Snapshot::begin(&canonical)?;
    let mut replaced = Vec::new();
    for path in &prepared.replacements {
        let saved_to = match read_relative(root, path)? {
            Some(bytes) => Some(snapshot.save_replaced(path, &bytes)?.display().to_string()),
            None => None,
        };
        replaced.push(Replaced {
            path: path.clone(),
            saved_to,
        });
    }
    Ok(replaced)
}

/// What a preview reports for the named conflicts: each path, with no
/// saved copy yet.
#[must_use]
pub fn previewed_replacements(prepared: &Prepared) -> Vec<Replaced> {
    prepared
        .replacements
        .iter()
        .map(|path| Replaced {
            path: path.clone(),
            saved_to: None,
        })
        .collect()
}

/// What the landing does with one destination under `origin`: an
/// adoption preserves everything it verified; a landing follows its
/// decision, and a candidate without one was a collision the refusal
/// already named.
fn action_of(prepared: &Prepared, origin: Origin, destination: &str) -> Option<Action> {
    match origin {
        Origin::Adopt if prepared.replacements.iter().any(|path| path == destination) => {
            Some(Action::Replaced)
        }
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
            (_, Some(Action::Replaced | Action::Created)) => continue,
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
        // An accepted deviation is preserved only while the bytes on disk
        // are the ones its entry accepted.
        let accepted = prepared
            .decision(&candidate.destination)
            .and_then(|decision| decision.accepted.as_ref());
        if let Some(accepted) = accepted
            && Digest::of(&current).to_string() != accepted.sha256
        {
            return Err(moved(
                &candidate.destination,
                "no longer holds the bytes its accepted drift entry names",
            ));
        }
        digests.insert(candidate.destination.clone(), Digest::of(&current));
    }
    Ok(digests)
}

/// Every destination the landing retires, each read again through the
/// held directory: it must still be absent or hold the bytes the record
/// vouches for.
///
/// # Errors
///
/// A `state-drift` refusal naming the destination someone edited since the
/// decision, with nothing written; and any read failure.
fn retiring(
    root: &File,
    recorded: Option<&Manifest>,
    prepared: &Prepared,
) -> Result<Vec<String>, RkError> {
    let mut out = Vec::new();
    for decision in &prepared.decisions {
        if decision.action != Action::Retired {
            continue;
        }
        let Some(file) = recorded.and_then(|record| record.file(&decision.destination)) else {
            continue;
        };
        if !unedited(root, file)? {
            return Err(moved(
                &decision.destination,
                "no longer holds the bytes the record vouches for",
            ));
        }
        out.push(decision.destination.clone());
    }
    Ok(out)
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
            security_scope: params.security_scope().to_owned(),
            required_check: params.required_check().to_owned(),
            dev_environment: params.dev_environment().cloned(),
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

    /// Remove the retired file at the relative `destination` through the
    /// held directory chain, following no link; an absent file is already
    /// retired.
    fn remove(&mut self, destination: &str) -> Result<(), RkError> {
        let path = Path::new(destination);
        let (parent, name) = split(path)?;
        let dir = match held::hold_dir_existing(self.root, parent) {
            Ok(dir) => dir,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(self.failure(destination, &[], &error)),
        };
        match std::fs::remove_file(held::proc_path(&dir).join(name)) {
            Ok(()) => {
                self.completed.push(destination.to_owned());
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(self.failure(destination, &[], &error)),
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
    /// last at schema 7, and a rerun matches the unchanged rendered whole
    /// files, replaces the marked regions, and preserves the seeded ones
    /// from the receipt alone.
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
            let region = again.projection.candidates.iter().any(|candidate| {
                candidate.destination == decision.destination
                    && matches!(
                        candidate.placement,
                        crate::projection::Placement::Region { .. }
                    )
            });
            let expected = match decision.kind {
                crate::landing::Kind::Rendered if region => Action::Replaced,
                crate::landing::Kind::Rendered => Action::Matched,
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

    /// A recorded destination the projection stops producing leaves the
    /// receipt: removed where its bytes are the ones the record vouches
    /// for, and kept, named dropped, where someone edited them.
    #[test]
    fn a_dropped_destination_is_retired_unless_the_target_edited_it() {
        for (bytes, action, stays) in [
            (&b"old\n"[..], Action::Retired, false),
            (&b"edited\n"[..], Action::Dropped, true),
        ] {
            let (_dir, target) = target();
            landed(&target, None, Origin::Init);
            let mut record = manifest::load(&target).expect("loads").expect("exists");
            std::fs::write(target.join("legacy.yml"), bytes).expect("writes");
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
                &std::collections::BTreeMap::new(),
            )
            .expect("decides");
            let decided = decisions
                .iter()
                .find(|decision| decision.destination == "legacy.yml")
                .expect("the dropped destination is decided");
            assert_eq!(decided.action, action);
            let outcome = landed(&target, Some(&record), Origin::Upgrade);
            assert_eq!(target.join("legacy.yml").is_file(), stays);
            assert!(outcome.receipt.file("legacy.yml").is_none());
        }
    }
}
