//! The candidate stage: this binary's projection for one target,
//! materialized on disk beside the knowledge that explains it.
//!
//! A stage is evidence, never an installation transaction. `rk stage`
//! gathers the target's evidence, computes the one pure [`Projection`],
//! and writes the complete proposed bytes of every candidate under
//! `artifacts/`, the installed binary's changelog, guidance, method,
//! bindings, runbooks, forge notes, setup skill, and the shared resources
//! that skill routes to under `reference/`, and one explanatory receipt,
//! `stage.json`. It writes nothing inside the target. Production landing
//! reads no byte of it, and only `rk stage clean` removes it.
//!
//! The stage is built whole under a fresh sibling of its resolved path and
//! renamed into place after the receipt, so a stage that is visible is
//! complete. The default root below the private state directory and the
//! receipt are created owner-only.

pub mod clean;
pub(crate) mod held;

use std::borrow::Cow;
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::applog;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::embedded;
use crate::error::RkError;
use crate::landing::manifest::{Manifest, Style, Workflow};
use crate::landing::{Kind, Params};
use crate::projection::{Placement, Projection};
use crate::skills;

/// The shape version of the stage receipt and of the `rk stage` report.
pub const STAGE_SCHEMA: &str = "rk.stage/1";

/// The receipt's name at the stage root.
pub const RECEIPT_NAME: &str = "stage.json";

/// The variable naming an alternative base for the default stage path.
pub const OUTPUT_ROOT_VAR: &str = "RK_STAGE_ROOT";

/// The directory below the state root that holds the default stages.
pub const STAGES_DIR: &str = "stages";

/// The directory below the stage root holding the candidate tree.
pub const ARTIFACTS_DIR: &str = "artifacts";

/// The directory below the stage root holding the installed knowledge.
pub const REFERENCE_DIR: &str = "reference";

/// The skill whose installed text and routed resources the reference
/// tree carries.
pub const SETUP_SKILL: &str = "rk-setup";

/// The interruption proof's seam: the stage-relative path after which a
/// materialization stops on purpose, as if the write after it had failed.
pub const INTERRUPT_VAR: &str = "RK_STAGE_INTERRUPT_AT";

/// Every reference root a stage writes, in the order the receipt lists
/// them.
pub const REFERENCE_ROOTS: [&str; 8] = [
    "CHANGELOG.md",
    "guidance",
    "method",
    "bindings",
    "runbooks",
    "forges",
    "skills/rk-setup",
    "skill-shared",
];

/// The explanatory receipt a stage carries, and the document `rk stage
/// --json` reports. Metadata only: no later command reads it as input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// The shape version of this document.
    pub schema: String,
    /// The binary that staged.
    pub rk_version: String,
    /// The canonical absolute path of the target that was read.
    pub target: String,
    /// The canonical absolute path of the stage itself.
    pub stage_root: String,
    /// The resolved landing parameters the projection ran under.
    pub parameters: Parameters,
    /// The `schema_version` the target's landing record declares, where
    /// the record is present and readable as JSON.
    pub receipt_schema_version: Option<u64>,
    /// One entry per candidate destination under `artifacts/`.
    pub candidates: Vec<CandidateEntry>,
    /// The destinations the target's own state withholds.
    pub omissions: Vec<Note>,
    /// The block destinations whose document offers the block no place.
    pub collisions: Vec<Note>,
    /// The destinations the landing record names that this projection no
    /// longer produces: target-owned from the next landing on.
    pub retired: Vec<String>,
    /// The recorded `seeded` destinations present on disk, which a
    /// production landing preserves.
    pub seeded_present: Vec<String>,
    /// The recorded `state` destinations present on disk, which a
    /// production landing preserves.
    pub state_present: Vec<String>,
    /// The reference roots written under `reference/`.
    pub reference: Vec<String>,
}

/// The resolved landing parameters, stated whole.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameters {
    /// The binding selected.
    pub tech: String,
    /// The forge selected.
    pub forge: String,
    /// The project path on the forge.
    pub repo: String,
    /// The working-copy mode.
    pub workflow: Workflow,
    /// The release style, where one resolved.
    pub style: Option<Style>,
    /// Whether the landing carries the Nix capability.
    pub nix: bool,
    /// The one permanent branch.
    pub trunk: String,
    /// The release-line prefix.
    pub line_prefix: String,
    /// The security contact, empty for the forge's own wording.
    pub security_contact: String,
    /// The acknowledgment window.
    pub security_response: String,
}

impl From<&Params> for Parameters {
    fn from(params: &Params) -> Self {
        Self {
            tech: params.tech().to_owned(),
            forge: params.forge().to_owned(),
            repo: params.repo().to_owned(),
            workflow: params.workflow(),
            style: params.style(),
            nix: params.nix(),
            trunk: params.trunk().to_owned(),
            line_prefix: params.line_prefix().to_owned(),
            security_contact: params.security_contact().to_owned(),
            security_response: params.security_response().to_owned(),
        }
    }
}

/// One candidate under `artifacts/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEntry {
    /// The destination, relative to the target root and to `artifacts/`.
    pub destination: String,
    /// Who owns the bytes after landing.
    pub kind: Kind,
    /// `whole` for a whole file, `region` for a marked region whose
    /// artifact is the complete spliced document.
    pub placement: String,
    /// The digest of the complete artifact bytes.
    pub sha256: Digest,
    /// The digest of the rendered region alone, for a region destination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_sha256: Option<Digest>,
    /// The embedded source paths the candidate was rendered from.
    pub sources: Vec<String>,
}

/// One destination named with a reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// The destination.
    pub destination: String,
    /// Why it is listed here.
    pub reason: String,
}

/// Where the resolved output path came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSource {
    /// `--output` named it.
    Flag,
    /// `RK_STAGE_ROOT` supplied the base.
    Environment,
    /// The private state root supplied the base.
    StateRoot,
}

impl OutputSource {
    /// The report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "--output",
            Self::Environment => "RK_STAGE_ROOT",
            Self::StateRoot => "state root",
        }
    }
}

/// The filesystem-safe key naming one target below a stage base.
///
/// The digest of the canonical target path, the same derivation the
/// target lock uses, so a path carrying a separator cannot name another
/// target's stage.
#[must_use]
pub fn target_key(canonical_target: &Path) -> String {
    Digest::of(canonical_target.display().to_string().as_bytes()).to_string()
}

/// Resolve the output directory: `--output` first, then a target and
/// version directory below `RK_STAGE_ROOT`, then the same below the
/// private state root.
///
/// # Errors
///
/// Returns a `prerequisite-unmet` refusal where neither a flag, the
/// variable, nor a state root names a base.
pub fn resolve_output(
    flag: Option<&Utf8Path>,
    canonical_target: &Path,
) -> Result<(PathBuf, OutputSource), RkError> {
    if let Some(flag) = flag {
        let path = if flag.is_absolute() {
            flag.as_std_path().to_path_buf()
        } else {
            std::env::current_dir()?.join(flag.as_std_path())
        };
        return Ok((path, OutputSource::Flag));
    }
    let leaf = Path::new(&target_key(canonical_target)).join(env!("CARGO_PKG_VERSION"));
    if let Some(base) = std::env::var_os(OUTPUT_ROOT_VAR).filter(|value| !value.is_empty()) {
        let base = PathBuf::from(base);
        let base = if base.is_absolute() {
            base
        } else {
            std::env::current_dir()?.join(base)
        };
        return Ok((base.join(leaf), OutputSource::Environment));
    }
    let Some(root) = applog::state_root() else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                "no state root resolves, so the stage has nowhere to go, and nothing was written",
            )
            .expected("--output <dir>, RK_STAGE_ROOT, or a state root under XDG_STATE_HOME or HOME")
            .action("pass --output <dir>, or set XDG_STATE_HOME or HOME, and run it again")
            .target_state("unchanged"),
        ));
    };
    Ok((root.join(STAGES_DIR).join(leaf), OutputSource::StateRoot))
}

/// An output path checked and made ready: its parent exists and is
/// canonical, the resolved stage root is known, and nothing nonempty
/// stands there.
#[derive(Debug)]
pub struct Prepared {
    parent: PathBuf,
    name: std::ffi::OsString,
    resolved: PathBuf,
    owner_only: bool,
}

impl Prepared {
    /// The canonical absolute path the stage will stand at.
    #[must_use]
    pub fn resolved(&self) -> &Path {
        &self.resolved
    }
}

/// The path `output` will stand at once created, computed without
/// creating any component: the deepest existing ancestor canonicalized,
/// the remaining components appended as named.
///
/// # Errors
///
/// A refusal for a remaining component that is `..`, which no stage path
/// may carry, and [`RkError::Io`] where the existing ancestor cannot be
/// canonicalized.
fn eventual(output: &Path) -> Result<PathBuf, RkError> {
    let mut existing = output;
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let Some(name) = existing.file_name() else {
            break;
        };
        rest.push(name);
        existing = existing.parent().unwrap_or_else(|| Path::new("/"));
    }
    let mut path = fs::canonicalize(if existing.as_os_str().is_empty() {
        Path::new(".")
    } else {
        existing
    })?;
    for name in rest.into_iter().rev() {
        if name == ".." {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::Usage,
                    format!(
                        "{} climbs through a directory that does not exist yet, and nothing was written",
                        output.display()
                    ),
                )
                .expected("an output path whose absent components are plain names")
                .target_state("unchanged"),
            ));
        }
        if name != "." {
            path.push(name);
        }
    }
    Ok(path)
}

/// Resolve where the stage will stand, refuse a stage inside the target,
/// refuse an existing nonempty output, create the parent, and name the
/// canonical stage root.
///
/// Nothing is created before the stage root is known and judged against
/// the target: a stage below the target would be a write inside the
/// repository this verb promises to leave alone, whichever of the flag,
/// the variable, or the state root put it there. Below the state root
/// every directory this creates is owner-only, and a base that turns out
/// to be a link or another file type refuses, because a private stage
/// under a directory somebody else controls is not private.
///
/// # Errors
///
/// Returns a `destructive-refusal` for a stage root at or below the
/// target, a `state-drift` refusal for an existing nonempty output, a
/// refusal for an output whose final component is no name, and
/// [`RkError::Io`] for a parent that cannot be created or read.
pub fn prepare(
    output: &Path,
    source: OutputSource,
    canonical_target: &Path,
) -> Result<Prepared, RkError> {
    let name = output
        .file_name()
        .filter(|name| *name != "." && *name != "..")
        .ok_or_else(|| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::Usage,
                    format!("{} names no directory to stage into", output.display()),
                )
                .expected("an output path ending in a directory name")
                .target_state("unchanged"),
            )
        })?
        .to_owned();
    let eventual = eventual(output)?;
    if eventual.starts_with(canonical_target) {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::DestructiveRefusal,
                format!(
                    "the stage would stand at {}, inside the target {}, and nothing was written",
                    eventual.display(),
                    canonical_target.display()
                ),
            )
            .expected("a stage root outside the target repository")
            .action(match source {
                OutputSource::Flag => "pass an --output outside the target".to_owned(),
                OutputSource::Environment => {
                    format!("point {OUTPUT_ROOT_VAR} outside the target, or pass --output")
                }
                OutputSource::StateRoot => {
                    "move the state root outside the target, or pass --output".to_owned()
                }
            })
            .target_state("unchanged"),
        ));
    }
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("/"), Path::to_path_buf);
    let owner_only = source == OutputSource::StateRoot;
    if owner_only {
        create_owner_only(&parent)?;
    } else {
        fs::create_dir_all(&parent)?;
    }
    let parent = fs::canonicalize(&parent)?;
    let resolved = parent.join(&name);
    match fs::symlink_metadata(&resolved) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(metadata) if metadata.is_dir() && fs::read_dir(&resolved)?.next().is_none() => {}
        Ok(_) => {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!(
                        "{} already exists and is not empty, and nothing was written",
                        resolved.display()
                    ),
                )
                .expected("an absent or empty output directory")
                .action(format!(
                    "rk stage clean {} removes a stage that stands there; otherwise pass another --output",
                    resolved.display()
                ))
                .target_state("unchanged"),
            ));
        }
    }
    Ok(Prepared {
        parent,
        name,
        resolved,
        owner_only,
    })
}

/// Create the default base for a stage owner-only, component by
/// component, and refuse a component that is a link or not a directory.
fn create_owner_only(dir: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    // The state root itself belongs to every run-shaped artifact; the
    // stage base below it and every component under that are private.
    let Some(state_root) = applog::state_root() else {
        return Ok(());
    };
    let base = state_root.join(STAGES_DIR);
    let Ok(rest) = dir.strip_prefix(&base) else {
        return Ok(());
    };
    let mut current = base;
    restrict(&current)?;
    for component in rest {
        current.push(component);
        restrict(&current)?;
    }
    Ok(())
}

/// Make one existing base component private, refusing a link or another
/// file type.
fn restrict(dir: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("stage base is not a directory: {}", dir.display()),
        ));
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// A stage composed and ready to write: every file with its
/// stage-relative path, and the receipt.
#[derive(Debug)]
pub struct Composed {
    /// Every file under the stage root except the receipt, sorted by path.
    pub files: Vec<(String, Cow<'static, [u8]>)>,
    /// The receipt, written last.
    pub receipt: Receipt,
}

/// Compose the stage for `projection`, rooted at `stage_root`, from the
/// projection, the resolved parameters, the target's record, and this
/// binary's embedded knowledge.
#[must_use]
pub fn compose(
    projection: &Projection,
    params: &Params,
    canonical_target: &Path,
    stage_root: &Path,
    record: Option<&Manifest>,
    receipt_schema_version: Option<u64>,
) -> Composed {
    let mut files: Vec<(String, Cow<'static, [u8]>)> = Vec::new();
    let mut candidates = Vec::new();
    for candidate in &projection.candidates {
        files.push((
            format!("{ARTIFACTS_DIR}/{}", candidate.destination),
            Cow::Owned(candidate.bytes.clone()),
        ));
        candidates.push(CandidateEntry {
            destination: candidate.destination.clone(),
            kind: candidate.kind,
            placement: match candidate.placement {
                Placement::Whole => "whole",
                Placement::Region { .. } => "region",
            }
            .to_owned(),
            sha256: Digest::of(&candidate.bytes),
            region_sha256: candidate.region.as_deref().map(Digest::of),
            sources: candidate.sources.clone(),
        });
    }
    for (path, bytes) in reference_files() {
        files.push((format!("{REFERENCE_DIR}/{path}"), Cow::Borrowed(bytes)));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let produced = |destination: &str| {
        projection
            .candidates
            .iter()
            .any(|candidate| candidate.destination == destination)
            || projection
                .omissions
                .iter()
                .any(|omission| omission.destination == destination)
    };
    let mut retired = Vec::new();
    let mut seeded_present = Vec::new();
    let mut state_present = Vec::new();
    if let Some(record) = record {
        for file in &record.files {
            if !produced(&file.destination) {
                retired.push(file.destination.clone());
            }
            let present = fs::symlink_metadata(canonical_target.join(&file.destination)).is_ok();
            match file.kind {
                Kind::Seeded if present => seeded_present.push(file.destination.clone()),
                Kind::State if present => state_present.push(file.destination.clone()),
                Kind::Rendered | Kind::Seeded | Kind::State => {}
            }
        }
    }
    let receipt = Receipt {
        schema: STAGE_SCHEMA.to_owned(),
        rk_version: env!("CARGO_PKG_VERSION").to_owned(),
        target: canonical_target.display().to_string(),
        stage_root: stage_root.display().to_string(),
        parameters: Parameters::from(params),
        receipt_schema_version,
        candidates,
        omissions: projection
            .omissions
            .iter()
            .map(|omission| Note {
                destination: omission.destination.clone(),
                reason: omission.reason.clone(),
            })
            .collect(),
        collisions: projection
            .collisions
            .iter()
            .map(|collision| Note {
                destination: collision.destination.clone(),
                reason: collision.reason.clone(),
            })
            .collect(),
        retired,
        seeded_present,
        state_present,
        reference: REFERENCE_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
    };
    Composed { files, receipt }
}

/// Every file the reference tree carries, as `(path, bytes)` below
/// `reference/`.
///
/// From the embedded sources and from nowhere else: the changelog, every
/// guidance file, the method, the bindings, the runbooks, the forge
/// documents, the setup skill as installed, and the shared resources that
/// skill names.
#[must_use]
pub fn reference_files() -> Vec<(String, &'static [u8])> {
    let mut out: Vec<(String, &'static [u8])> =
        vec![("CHANGELOG.md".to_owned(), embedded::CHANGELOG.as_bytes())];
    for (root, dir) in [
        ("guidance", &embedded::GUIDANCE),
        ("method", &embedded::METHOD),
        ("bindings", &embedded::BINDINGS),
        ("runbooks", &embedded::RUNBOOKS),
        ("forges", &embedded::FORGES),
    ] {
        for (path, bytes) in embedded::walk(dir) {
            out.push((format!("{root}/{path}"), bytes));
        }
    }
    let prefix = format!("{SETUP_SKILL}/");
    let mut skill_text = String::new();
    for (path, bytes) in embedded::walk(&embedded::SKILLS) {
        if path.starts_with(&prefix) {
            if path == format!("{prefix}SKILL.md") {
                skill_text = String::from_utf8_lossy(bytes).into_owned();
            }
            out.push((format!("skills/{path}"), bytes));
        }
    }
    for artifact in skills::shared() {
        if skill_text.contains(&artifact.path) {
            out.push((format!("skill-shared/{}", artifact.path), artifact.bytes));
        }
    }
    out
}

/// How many sibling names a write tries before it refuses.
const TEMP_ATTEMPTS: u32 = 8;

/// The proof's seam for the failure cleanup: a directory where a stopped
/// write announces `stopped` before it quarantines its sibling and waits
/// for `proceed`.
pub const PAUSE_BEFORE_CLEANUP_VAR: &str = "RK_STAGE_PAUSE_BEFORE_CLEANUP";

/// The prefix of a quarantined sibling's name.
const QUARANTINE_PREFIX: &str = ".rk-stage-quarantine-";

/// The sibling name for one attempt: the first names this process alone,
/// and every retry adds a nonce, so an entry somebody else left under the
/// first name is stepped around rather than reused.
fn temp_name(name: &std::ffi::OsStr, attempt: u32) -> std::ffi::OsString {
    let mut out = std::ffi::OsString::from(format!(".rk-stage-{}", std::process::id()));
    if attempt > 0 {
        out.push(format!("-{:08x}", held::nonce() & 0xffff_ffff));
    }
    out.push(".");
    out.push(name);
    out
}

/// The sibling this write created and holds: its name under the held
/// parent, the open directory, and the identity the directory had the
/// moment it was opened, which every later act on it is judged against.
struct Temp {
    name: std::ffi::OsString,
    dir: File,
    identity: held::Identity,
}

/// Create the fresh sibling this write owns, exclusively, and hold it
/// open: a name that already exists is never entered or removed, and the
/// next name is tried instead, a bounded number of times.
fn create_temp(prepared: &Prepared, parent: &File) -> std::io::Result<Temp> {
    let mut builder = fs::DirBuilder::new();
    if prepared.owner_only {
        builder.mode(0o700);
    }
    let base = held::proc_path(parent);
    for attempt in 0..TEMP_ATTEMPTS {
        let name = temp_name(&prepared.name, attempt);
        match builder.create(base.join(&name)) {
            Ok(()) => {
                let dir = held::open_dir(&base.join(&name))?;
                let identity = held::Identity::of(&dir.metadata()?);
                return Ok(Temp {
                    name,
                    dir,
                    identity,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!(
            "every sibling name for the stage below {} is taken, and nothing was written or removed",
            prepared.parent.display()
        ),
    ))
}

/// Write the composed stage whole, then rename it into place.
///
/// Every file goes into a fresh sibling of the resolved root that this
/// write created exclusively and holds open, written through the held
/// descriptor rather than by name; the receipt goes last and owner-only;
/// then, once the entry under the sibling's name still carries the
/// created identity, one rename lands it. A failure anywhere quarantines
/// the entry under an unpredictable name in the same parent, judges it
/// against the created identity, and removes it only on a match: an
/// entry this write did not create is never removed.
///
/// # Errors
///
/// Any I/O failure, including the injected stop of the interruption
/// proof, which reports as an I/O failure naming the path it stopped at.
pub fn write(prepared: &Prepared, composed: &Composed) -> Result<(), RkError> {
    let stop = std::env::var_os(INTERRUPT_VAR).map(PathBuf::from);
    write_stopping_at(prepared, composed, stop.as_deref())
}

/// [`write`], stopped on purpose after the file at `stop`, as if the
/// write after it had failed: the interruption proof's seam.
///
/// # Errors
///
/// As [`write`], plus the injected stop.
pub fn write_stopping_at(
    prepared: &Prepared,
    composed: &Composed,
    stop: Option<&Path>,
) -> Result<(), RkError> {
    let parent = held::open_dir(&prepared.parent)?;
    let temp = create_temp(prepared, &parent)?;
    if let Err(error) = write_into(&temp, composed, stop) {
        return Err(RkError::Io(cleanup(&parent, &temp, error)));
    }
    land(&parent, &temp, prepared).map_err(RkError::Io)
}

/// Rename the finished sibling into place, from its verified identity:
/// the entry under the sibling's name is judged against the created
/// identity immediately before the rename, and a mismatch refuses
/// without touching anything.
fn land(parent: &File, temp: &Temp, prepared: &Prepared) -> std::io::Result<()> {
    let base = held::proc_path(parent);
    let current = fs::symlink_metadata(base.join(&temp.name))?;
    if current.file_type().is_symlink() || held::Identity::of(&current) != temp.identity {
        return Err(std::io::Error::other(format!(
            "the sibling under {} was replaced before the stage could land; nothing was renamed or removed",
            prepared.parent.join(&temp.name).display()
        )));
    }
    match fs::rename(base.join(&temp.name), &prepared.resolved) {
        Ok(()) => Ok(()),
        Err(error) => Err(cleanup(parent, temp, error)),
    }
}

/// The failure cleanup: quarantine whatever stands under the sibling's
/// name, judge it against the created identity, and remove it only on a
/// match. Returns `error` annotated with what was left where.
fn cleanup(parent: &File, temp: &Temp, error: std::io::Error) -> std::io::Error {
    held::pause(PAUSE_BEFORE_CLEANUP_VAR, "stopped", "proceed");
    let base = held::proc_path(parent);
    let (quarantined, current) = match held::quarantine(parent, &temp.name, QUARANTINE_PREFIX) {
        Ok(moved) => moved,
        Err(quarantine) => {
            return std::io::Error::new(
                error.kind(),
                format!(
                    "{error}; the sibling under {} could not be quarantined and was left in place: {quarantine}",
                    temp.name.display()
                ),
            );
        }
    };
    if current.file_type().is_symlink() || held::Identity::of(&current) != temp.identity {
        return std::io::Error::new(
            error.kind(),
            format!(
                "{error}; the entry under the sibling's name {} was not the directory this run created, so it was moved to {} and left in place",
                temp.name.display(),
                quarantined.display()
            ),
        );
    }
    match fs::remove_dir_all(base.join(&quarantined)) {
        Ok(()) => error,
        Err(removal) => std::io::Error::new(
            error.kind(),
            format!(
                "{error}; the stopped sibling was moved to {} and could not be removed: {removal}",
                quarantined.display()
            ),
        ),
    }
}

/// The body of [`write`]: every file into the held sibling, then the
/// receipt, each addressed through the descriptor.
fn write_into(temp: &Temp, composed: &Composed, stop: Option<&Path>) -> std::io::Result<()> {
    let base = held::proc_path(&temp.dir);
    for (path, bytes) in &composed.files {
        let destination = base.join(path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&destination, bytes)?;
        if stop.is_some_and(|stop| Path::new(path) == stop) {
            return Err(std::io::Error::other(format!(
                "the stage was stopped after {path} for the proof"
            )));
        }
    }
    let text = serde_json::to_string_pretty(&composed.receipt).map_err(std::io::Error::other)?;
    let mut receipt = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(base.join(RECEIPT_NAME))?;
    receipt.write_all(text.as_bytes())?;
    receipt.write_all(b"\n")?;
    receipt.sync_all()?;
    Ok(())
}

/// The `schema_version` the target's landing record declares, read
/// leniently: `None` where no record exists or it does not parse as a
/// JSON object carrying an integer there. Explanatory, never a gate.
#[must_use]
pub fn recorded_schema_version(target: &Utf8Path) -> Option<u64> {
    let bytes = fs::read(target.join(crate::landing::manifest::MANIFEST_PATH)).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("schema_version")?.as_u64()
}

#[cfg(test)]
mod tests {
    use super::{
        CandidateEntry, Note, Parameters, REFERENCE_ROOTS, Receipt, STAGE_SCHEMA, reference_files,
        target_key,
    };
    use crate::digest::Digest;
    use crate::landing::Kind;
    use crate::landing::manifest::{Style, Workflow};

    /// The complete `rk.stage/1` receipt shape, held by snapshot: a field
    /// rename or removal fails here and becomes a schema-version bump.
    #[test]
    fn the_stage_receipt_schema_snapshot_holds() {
        let receipt = Receipt {
            schema: STAGE_SCHEMA.to_owned(),
            rk_version: "0.0.0".into(),
            target: "/tmp/t".into(),
            stage_root: "/tmp/s".into(),
            parameters: Parameters {
                tech: "rust".into(),
                forge: "github".into(),
                repo: "acme/widget".into(),
                workflow: Workflow::Worktree,
                style: Some(Style::Trunk),
                nix: false,
                trunk: "master".into(),
                line_prefix: "release/".into(),
                security_contact: String::new(),
                security_response: "best-effort".into(),
            },
            receipt_schema_version: Some(6),
            candidates: vec![
                CandidateEntry {
                    destination: "AGENTS.md".into(),
                    kind: Kind::Rendered,
                    placement: "region".into(),
                    sha256: Digest::of(b"a"),
                    region_sha256: Some(Digest::of(b"r")),
                    sources: vec!["blocks/routing.md.in".into()],
                },
                CandidateEntry {
                    destination: "release-plz.toml".into(),
                    kind: Kind::Seeded,
                    placement: "whole".into(),
                    sha256: Digest::of(b"b"),
                    region_sha256: None,
                    sources: vec!["snippets/rust/github/release-plz.toml".into()],
                },
            ],
            omissions: vec![Note {
                destination: "flake.nix".into(),
                reason: "the target already carries flake.nix".into(),
            }],
            collisions: vec![],
            retired: vec!["old.yml".into()],
            seeded_present: vec!["release-plz.toml".into()],
            state_present: vec![],
            reference: REFERENCE_ROOTS
                .iter()
                .map(|root| (*root).to_owned())
                .collect(),
        };
        assert_eq!(
            serde_json::to_string(&receipt).expect("a receipt serializes"),
            format!(
                r#"{{"schema":"rk.stage/1","rk_version":"0.0.0","target":"/tmp/t","stage_root":"/tmp/s","parameters":{{"tech":"rust","forge":"github","repo":"acme/widget","workflow":"worktree","style":"trunk","nix":false,"trunk":"master","line_prefix":"release/","security_contact":"","security_response":"best-effort"}},"receipt_schema_version":6,"candidates":[{{"destination":"AGENTS.md","kind":"rendered","placement":"region","sha256":"{}","region_sha256":"{}","sources":["blocks/routing.md.in"]}},{{"destination":"release-plz.toml","kind":"seeded","placement":"whole","sha256":"{}","sources":["snippets/rust/github/release-plz.toml"]}}],"omissions":[{{"destination":"flake.nix","reason":"the target already carries flake.nix"}}],"collisions":[],"retired":["old.yml"],"seeded_present":["release-plz.toml"],"state_present":[],"reference":["CHANGELOG.md","guidance","method","bindings","runbooks","forges","skills/rk-setup","skill-shared"]}}"#,
                Digest::of(b"a"),
                Digest::of(b"r"),
                Digest::of(b"b")
            )
        );
        let back: Receipt =
            serde_json::from_str(&serde_json::to_string(&receipt).expect("serializes"))
                .expect("a receipt reads back");
        assert_eq!(back.stage_root, "/tmp/s");
    }

    /// Every declared reference root is served by at least one file, and
    /// no file reaches outside the declared roots.
    #[test]
    fn every_reference_root_serves_a_file_and_nothing_else_is_served() {
        let files = reference_files();
        for root in REFERENCE_ROOTS {
            assert!(
                files
                    .iter()
                    .any(|(path, _)| path == root || path.starts_with(&format!("{root}/"))),
                "{root}: the reference tree carries no file for it"
            );
        }
        for (path, _) in &files {
            assert!(
                REFERENCE_ROOTS
                    .iter()
                    .any(|root| path == root || path.starts_with(&format!("{root}/"))),
                "{path}: outside every declared reference root"
            );
            assert!(
                !path
                    .split('/')
                    .any(|part| part == "_docs" || part == "tests" || part == "src"),
                "{path}: an instance-owned or source path in the reference tree"
            );
        }
    }

    /// A sibling somebody else left under the exact first candidate name
    /// is neither entered nor removed: the write steps to the next name,
    /// lands, and every byte of the stranger survives.
    #[test]
    fn a_pre_existing_temp_sibling_is_never_touched() {
        let scratch = tempfile::tempdir().expect("a scratch dir exists");
        let parent = std::fs::canonicalize(scratch.path()).expect("canonical");
        let output = parent.join("stage");
        let target = parent.join("target");
        std::fs::create_dir(&target).expect("creates");
        let prepared =
            super::prepare(&output, super::OutputSource::Flag, &target).expect("prepares");
        let stranger = parent.join(super::temp_name(std::ffi::OsStr::new("stage"), 0));
        std::fs::create_dir_all(stranger.join("deep")).expect("creates");
        std::fs::write(stranger.join("deep/canary"), b"not yours").expect("writes");
        std::fs::write(stranger.join("canary"), b"still not yours").expect("writes");
        let composed = super::Composed {
            files: vec![(
                "artifacts/a.txt".to_owned(),
                std::borrow::Cow::Borrowed(b"a"),
            )],
            receipt: sample_receipt(),
        };
        super::write(&prepared, &composed).expect("the write lands beside the stranger");
        assert_eq!(
            std::fs::read(output.join("artifacts/a.txt")).expect("reads"),
            b"a"
        );
        assert_eq!(
            std::fs::read(stranger.join("deep/canary")).expect("the stranger reads"),
            b"not yours"
        );
        assert_eq!(
            std::fs::read(stranger.join("canary")).expect("the stranger reads"),
            b"still not yours"
        );
        // The interrupted variant removes only what it created.
        let output_two = parent.join("stage-two");
        let prepared =
            super::prepare(&output_two, super::OutputSource::Flag, &target).expect("prepares");
        let stranger_two = parent.join(super::temp_name(std::ffi::OsStr::new("stage-two"), 0));
        std::fs::create_dir(&stranger_two).expect("creates");
        std::fs::write(stranger_two.join("canary"), b"kept").expect("writes");
        let stopped = super::write_stopping_at(
            &prepared,
            &composed,
            Some(std::path::Path::new("artifacts/a.txt")),
        );
        assert!(stopped.is_err());
        assert!(!output_two.exists());
        assert_eq!(
            std::fs::read(stranger_two.join("canary")).expect("the stranger reads"),
            b"kept"
        );
        let leftovers: Vec<String> = std::fs::read_dir(&parent)
            .expect("reads")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.starts_with(".rk-stage-"))
            .collect();
        assert_eq!(
            leftovers.len(),
            2,
            "only the two strangers remain: {leftovers:?}"
        );
    }

    /// A stage root at or below the target refuses before any component
    /// is created, whichever source named it.
    #[test]
    fn a_stage_root_inside_the_target_refuses_before_anything_is_created() {
        let scratch = tempfile::tempdir().expect("a scratch dir exists");
        let target = std::fs::canonicalize(scratch.path())
            .expect("canonical")
            .join("t");
        std::fs::create_dir(&target).expect("creates");
        for (output, source) in [
            (target.join("stage"), super::OutputSource::Flag),
            (
                target.join("deep/er/stage"),
                super::OutputSource::Environment,
            ),
            (
                target.join("state/release-kit/stages/k/v"),
                super::OutputSource::StateRoot,
            ),
            (target.clone(), super::OutputSource::Flag),
        ] {
            let error = super::prepare(&output, source, &target).expect_err("refuses");
            assert_eq!(
                error.reason(),
                crate::diagnostic::Reason::DestructiveRefusal
            );
            assert_eq!(error.exit_code(), 73);
        }
        assert_eq!(
            std::fs::read_dir(&target).expect("reads").count(),
            0,
            "a refusal created a component inside the target"
        );
    }

    fn sample_receipt() -> Receipt {
        Receipt {
            schema: STAGE_SCHEMA.to_owned(),
            rk_version: "0.0.0".into(),
            target: "/tmp/t".into(),
            stage_root: "/tmp/s".into(),
            parameters: Parameters {
                tech: "rust".into(),
                forge: "github".into(),
                repo: "acme/widget".into(),
                workflow: Workflow::Worktree,
                style: Some(Style::Trunk),
                nix: false,
                trunk: "master".into(),
                line_prefix: "release/".into(),
                security_contact: String::new(),
                security_response: "best-effort".into(),
            },
            receipt_schema_version: None,
            candidates: vec![],
            omissions: vec![],
            collisions: vec![],
            retired: vec![],
            seeded_present: vec![],
            state_present: vec![],
            reference: vec![],
        }
    }

    /// The key is the lock's derivation: one digest per canonical path,
    /// and a separator in the path cannot escape the base.
    #[test]
    fn the_target_key_is_one_flat_digest() {
        let key = target_key(std::path::Path::new("/a/b"));
        assert_eq!(key.len(), 64);
        assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(key, target_key(std::path::Path::new("/a/c")));
    }
}
