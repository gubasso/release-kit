//! `rk migrate`: carry a target off another release tool through an
//! archive and a greenfield landing.
//!
//! `inventory` lists what the archive moves. `archive` moves every
//! predecessor file into a workspace under the state root, snapshots what
//! no Git holds, writes the registry, and names the open migration in the
//! committed configuration; `--rebuild` recreates the archived files from
//! Git on a clone whose workspace is gone. `mark` records the map and the
//! loop, `status` reads the registry back, and `close` ends the migration
//! once nothing is unfinished and the setup proof is current. Every verb
//! that writes the target previews by default.
//!
//! SATISFIES migration:an-archive-moves-only-committed-files
//! SATISFIES migration:the-configuration-names-the-open-migration

use std::path::{Path, PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::cli::migrate::{MigrateAction, MigrateArgs};
use crate::commands::setup::{self as setup_cmd, ForgeSnapshot};
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::manifest;
use crate::migrate::inventory::{self, Entry, Placement};
use crate::migrate::registry::{
    self, HistoryEntry, Integrity, Mark, Marked, Registry, Snapshot, Source, Status,
};
use crate::migrate::{self, git};
use crate::output::Output;
use crate::snapshot::Snapshot as Saved;

/// Route one `rk migrate` invocation.
///
/// # Errors
///
/// Each verb's own refusals; every failure is typed.
pub fn run(args: &MigrateArgs) -> Result<(), RkError> {
    match &args.action {
        MigrateAction::Inventory { target, json } => inventory(target, Output::new(*json)),
        MigrateAction::Archive {
            target,
            apply,
            rebuild,
            json,
        } => {
            if *rebuild {
                rebuild_sources(target, *apply, Output::new(*json))
            } else {
                archive(target, *apply, Output::new(*json))
            }
        }
        MigrateAction::Mark {
            name,
            status,
            group,
            tags,
            destinations,
            reason,
            target,
            json,
        } => mark(
            target,
            &Mark {
                name,
                status: Some(Status::parse(status)?),
                group: group.as_deref(),
                tags,
                destinations,
                reason: reason.as_deref(),
            },
            Output::new(*json),
        ),
        MigrateAction::Status { target, json } => status(target, Output::new(*json)),
        MigrateAction::Close {
            target,
            apply,
            json,
        } => close(target, *apply, Output::new(*json)),
    }
}

/// The target's canonical path, or the refusal for one that is not a
/// directory.
fn canonical(target: &Utf8Path) -> Result<PathBuf, RkError> {
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("{target} is not a directory"),
            )
            .expected("an existing repository"),
        ));
    }
    Ok(std::fs::canonicalize(target.as_std_path())?)
}

/// The workspace path of migration `id`, or the refusal where no state
/// root resolves.
fn workspace_of(canonical: &Path, id: &str) -> Result<Utf8PathBuf, RkError> {
    Ok(runs_of(canonical)?.join(id))
}

/// The directory of every workspace at this target, or the refusal where
/// no state root resolves.
fn runs_of(canonical: &Path) -> Result<Utf8PathBuf, RkError> {
    migrate::runs(canonical)
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .ok_or_else(|| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    "no state root resolves, so the migration has no workspace, and nothing was written",
                )
                .expected("a state root under XDG_STATE_HOME or HOME")
                .target_state("unchanged"),
            )
        })
}

/// The open migration's id and workspace, and its registry.
fn open(target: &Utf8Path) -> Result<(String, Utf8PathBuf, Registry), RkError> {
    let canonical = canonical(target)?;
    let id = migrate::open_id(target)?.ok_or_else(migrate::none_open)?;
    let workspace = workspace_of(&canonical, &id)?;
    if !workspace.join(migrate::REGISTRY_FILE).is_file() {
        return Err(migrate::workspace_missing(&id, &workspace));
    }
    let registry = registry::load(&workspace)?;
    Ok((id, workspace, registry))
}

/// The inventory report.
#[derive(Debug, Serialize)]
struct InventoryReport {
    schema: &'static str,
    target: String,
    files: Vec<Entry>,
    next: Vec<String>,
}

fn inventory(target: &Utf8PathBuf, out: Output) -> Result<(), RkError> {
    canonical(target)?;
    let files = inventory::scan(target)?;
    for entry in &files {
        match entry.placement {
            Placement::Archived => out.result_line(format!(
                "archive {} ({}, {})",
                entry.path,
                entry.tool,
                entry.kind.as_str()
            )),
            Placement::InPlace => out.result_line(format!("in place {}", entry.path)),
        }
    }
    if !files
        .iter()
        .any(|entry| entry.placement == Placement::Archived)
    {
        out.result_line("no predecessor file is listed here");
    }
    let next = vec![format!(
        "rk migrate archive --target {target} previews the move"
    )];
    out.next(&next);
    out.emit(&InventoryReport {
        schema: migrate::SCHEMA,
        target: target.to_string(),
        files,
        next,
    })
}

/// One destructive step's forge snapshot, in a report.
#[derive(Debug, Serialize)]
struct ForgeRow {
    step: String,
    /// `taken` or `unknown`.
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// The archive report.
#[derive(Debug, Serialize)]
struct ArchiveReport {
    schema: &'static str,
    /// `preview` for an archive or a rebuild that writes nothing, else
    /// `apply`, `resume`, or `rebuild`.
    mode: &'static str,
    target: String,
    id: String,
    workspace: String,
    base_commit: String,
    archived: Vec<String>,
    in_place: Vec<String>,
    snapshots: Vec<String>,
    forge: Vec<ForgeRow>,
    next: Vec<String>,
}

/// The kind of a forge snapshot in the registry.
const FORGE_STATE: &str = "forge-state";

/// A refusal that leaves the target unchanged.
fn refused(reason: Reason, message: String, expected: &str, action: Option<String>) -> RkError {
    let mut diagnostic = Diagnostic::new(reason, message)
        .expected(expected)
        .target_state("unchanged");
    if let Some(action) = action {
        diagnostic = diagnostic.action(action);
    }
    RkError::refusal(diagnostic)
}

#[allow(
    clippy::too_many_lines,
    reason = "the archive is one ordered sequence of checks and writes, and splitting it would separate a write from the check that licensed it"
)]
fn archive(target: &Utf8PathBuf, apply: bool, out: Output) -> Result<(), RkError> {
    let canonical = canonical(target)?;
    if let Some(id) = migrate::open_id(target)? {
        if apply {
            return resume(target, &canonical, &id, out);
        }
        return Err(refused(
            Reason::PrerequisiteUnmet,
            format!(
                "the migration {id} is already open; read it with rk migrate status (migration:the-configuration-names-the-open-migration)"
            ),
            "one open migration per target",
            Some(format!(
                "rk migrate archive --apply --target {target} takes the forge snapshots it still misses"
            )),
        ));
    }
    if manifest::load(target)?.is_some() {
        return Err(refused(
            Reason::PrerequisiteUnmet,
            "this target carries a landing record, so it is upgraded or adopted rather than migrated (migration:an-archive-moves-only-committed-files)".to_owned(),
            "a target another release tool drives, with no landing record",
            Some(format!("rk status --target {target} names its classification")),
        ));
    }
    let trunk = crate::config::trunk_of(target.as_std_path())?;
    let trunk_ref = format!("refs/heads/{trunk}^{{commit}}");
    let head = git::run(
        target,
        &["rev-parse", "--verify", "--quiet", trunk_ref.as_str()],
    )?;
    if !head.status.success() {
        return Err(refused(
            Reason::PrerequisiteUnmet,
            format!(
                "{target} has no local branch {trunk} with a commit, and the archive reads every listed file from the trunk (migration:an-archive-moves-only-committed-files)"
            ),
            "the trunk git.trunk names, as a local branch with at least one commit",
            Some(format!(
                "set git.trunk in {} to the trunk's branch name",
                crate::config::CONFIG_PATH
            )),
        ));
    }
    let base_commit = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    let entries = inventory::scan(target)?;
    let archived: Vec<&Entry> = entries
        .iter()
        .filter(|entry| entry.placement == Placement::Archived)
        .collect();
    if archived.is_empty() {
        return Err(refused(
            Reason::PrerequisiteUnmet,
            format!(
                "the inventory lists no predecessor file at {target}, so there is nothing to archive"
            ),
            "a predecessor release tool's files",
            Some(format!(
                "rk status --target {target} names its classification"
            )),
        ));
    }
    // The trunk must hold every listed file's exact bytes, unfiltered, and
    // no listed file the target lacks: a rebuild reads the files back from
    // the trunk, so an ignored, uncommitted, or unintegrated file, one a
    // filter converts, and an unintegrated deletion all refuse here.
    let known: Vec<&str> = inventory::PREDECESSORS
        .iter()
        .map(|(path, _, _)| *path)
        .chain(inventory::ROOT_FILES)
        .collect();
    let blobs = git::regular_blobs(target, &base_commit, &known)?;
    let paths: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
    let raw = git::raw_blob_ids(target, &paths)?;
    let mut differing: Vec<&str> = paths
        .iter()
        .zip(&raw)
        .filter(|(path, oid)| blobs.get(**path) != Some(*oid))
        .map(|(path, _)| *path)
        .collect();
    let on_trunk: Vec<&str> = blobs.keys().map(String::as_str).collect();
    differing.extend(
        inventory::listed(&on_trunk)
            .into_iter()
            .filter(|path| !paths.contains(path)),
    );
    if !differing.is_empty() {
        return Err(refused(
            Reason::StateDrift,
            format!(
                "these files differ from the trunk's copy on {trunk}: {}; integrate their edits onto the trunk, then run the archive again (migration:an-archive-moves-only-committed-files)",
                differing.join(", ")
            ),
            "every listed file's bytes equal to its copy on the trunk, so Git holds the bytes the archive moves",
            None,
        ));
    }
    crate::config::check_writable(target.as_std_path())?;
    let now = std::time::SystemTime::now();
    let stamp = migrate::stamp(now);
    let runs = runs_of(&canonical)?;
    // The apply claims its workspace exclusively, and a preview reads the
    // name that claim would take, writing nothing.
    let (id, workspace) = if apply {
        let (id, _) = migrate::claim(runs.as_std_path(), &stamp)?;
        let workspace = runs.join(&id);
        (id, workspace)
    } else {
        let id = migrate::free_id(runs.as_std_path(), &stamp);
        let workspace = runs.join(&id);
        (id, workspace)
    };
    let archived_paths: Vec<String> = archived.iter().map(|entry| entry.path.clone()).collect();
    let in_place: Vec<String> = entries
        .iter()
        .filter(|entry| entry.placement == Placement::InPlace)
        .map(|entry| entry.path.clone())
        .collect();
    if !apply {
        for path in &archived_paths {
            out.result_line(format!("would archive {path}"));
        }
        for path in &in_place {
            out.result_line(format!("stays in place {path}"));
        }
        out.result_line(format!("would open the migration {id} at {workspace}"));
        let next = vec![format!(
            "rk migrate archive --apply --target {target} moves the files and opens the migration"
        )];
        out.next(&next);
        return out.emit(&ArchiveReport {
            schema: migrate::SCHEMA,
            mode: "preview",
            target: target.to_string(),
            id,
            workspace: workspace.to_string(),
            base_commit,
            archived: archived_paths,
            in_place,
            snapshots: Vec::new(),
            forge: Vec::new(),
            next,
        });
    }

    // Copy first, and read each copy back against the checksum the
    // inventory took, before any file leaves the target.
    let saved = Saved::at(workspace.clone().into_std_path_buf());
    for entry in &archived {
        let bytes = std::fs::read(target.join(&entry.path))?;
        let path = saved.save(&Path::new(migrate::SOURCES_DIR).join(&entry.path), &bytes)?;
        if Digest::of(&std::fs::read(&path)?).to_string() != entry.sha256 {
            return Err(refused(
                Reason::StateDrift,
                format!(
                    "the archived copy of {} does not match the bytes the inventory read; the target is unchanged",
                    entry.path
                ),
                "the same bytes on both sides",
                None,
            ));
        }
    }
    let at = migrate::at(now);
    let mut snapshots = git_snapshots(target, &workspace, &saved, &at)?;
    let forge = take_forge(target, &workspace, &saved, &at, &[], &mut snapshots);
    let mut sources: Vec<Source> = entries
        .iter()
        .map(|entry| {
            source_of(
                &entry.path,
                &entry.sha256,
                entry.tool,
                entry.kind,
                entry.placement,
                &at,
            )
        })
        .collect();
    registry::assign_ids(&mut sources);
    let registry = Registry {
        schema_version: registry::SCHEMA_VERSION,
        id: id.clone(),
        base_commit: base_commit.clone(),
        sources,
        groups: Vec::new(),
        snapshots,
    };
    registry::save(&workspace, &registry)?;
    for path in &archived_paths {
        std::fs::remove_file(target.join(path))?;
        prune_empty_parents(target, path);
    }
    crate::config::set_migration(target.as_std_path(), Some(&id))?;
    for path in &archived_paths {
        out.result_line(format!("archived {path}"));
    }
    for row in &forge {
        out.result_line(format!(
            "forge snapshot {}: {}{}",
            row.step,
            row.state,
            row.detail
                .as_deref()
                .map_or_else(String::new, |detail| format!(" ({detail})"))
        ));
    }
    out.result_line(format!(
        "opened the migration {id} at {workspace}; .release-kit/config.toml names it"
    ));
    let next = vec![
        format!("rk init --target {target} previews the greenfield landing"),
        format!("rk migrate status --target {target} reads the registry"),
    ];
    out.next(&next);
    out.emit(&ArchiveReport {
        schema: migrate::SCHEMA,
        mode: "apply",
        target: target.to_string(),
        id,
        workspace: workspace.to_string(),
        base_commit,
        archived: archived_paths,
        in_place,
        snapshots: registry
            .snapshots
            .iter()
            .map(|snapshot| snapshot.id.clone())
            .collect(),
        forge,
        next,
    })
}

/// One registry source, pending since `at`.
fn source_of(
    path: &str,
    sha256: &str,
    tool: &str,
    kind: inventory::Kind,
    placement: Placement,
    at: &str,
) -> Source {
    Source {
        id: registry::source_id(path),
        path: path.to_owned(),
        sha256: sha256.to_owned(),
        tool: tool.to_owned(),
        kind,
        placement,
        tags: Vec::new(),
        group: None,
        status: Status::Pending,
        history: vec![HistoryEntry {
            at: at.to_owned(),
            status: Status::Pending,
            reason: None,
        }],
    }
}

/// The Git snapshots no clone of the trunk holds: a bundle of every
/// long-lived branch, and the tag list.
fn git_snapshots(
    target: &Utf8Path,
    workspace: &Utf8Path,
    saved: &Saved,
    at: &str,
) -> Result<Vec<Snapshot>, RkError> {
    let mut snapshots = Vec::new();
    let branches: Vec<String> = git::lines(
        target,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )?
    .into_iter()
    .filter(|branch| {
        crate::assess::LONG_LIVED_BRANCHES.contains(&branch.as_str())
            || branch.starts_with("release/")
    })
    .collect();
    if !branches.is_empty() {
        let relative = format!("{}/branches.bundle", migrate::REFS_DIR);
        let path = workspace.join(&relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut args = vec!["bundle", "create", path.as_str()];
        args.extend(branches.iter().map(String::as_str));
        git::stdout(target, &args)?;
        snapshots.push(Snapshot {
            id: "long-lived-branches".to_owned(),
            kind: "git-bundle".to_owned(),
            file: relative,
            sha256: Digest::of(&std::fs::read(&path)?).to_string(),
            taken_at: at.to_owned(),
            second_copy: false,
        });
    }
    let tags = git::lines(target, &["tag", "--list"])?;
    let text = tags.iter().fold(String::new(), |mut text, tag| {
        text.push_str(tag);
        text.push('\n');
        text
    });
    let relative = format!("{}/tags.txt", migrate::REFS_DIR);
    saved.save(Path::new(&relative), text.as_bytes())?;
    snapshots.push(Snapshot {
        id: "tag-list".to_owned(),
        kind: "tag-list".to_owned(),
        file: relative,
        sha256: Digest::of(text.as_bytes()).to_string(),
        taken_at: at.to_owned(),
        second_copy: false,
    });
    Ok(snapshots)
}

/// Take the forge snapshots still missing, appending each one taken to
/// `snapshots`; a target whose setup does not resolve reads unknown.
fn take_forge(
    target: &Utf8PathBuf,
    workspace: &Utf8Path,
    saved: &Saved,
    at: &str,
    held: &[String],
    snapshots: &mut Vec<Snapshot>,
) -> Vec<ForgeRow> {
    let taken = match setup_cmd::migration_snapshots(target, saved, held) {
        Ok(taken) => taken,
        Err(error) => {
            return vec![ForgeRow {
                step: "setup".to_owned(),
                state: "unknown",
                file: None,
                detail: Some(error.to_string()),
            }];
        }
    };
    let mut rows = Vec::new();
    for (step, result) in taken {
        match result {
            ForgeSnapshot::Taken(path) => {
                let relative = Utf8PathBuf::from_path_buf(path.clone())
                    .ok()
                    .and_then(|path| path.strip_prefix(workspace).ok().map(Utf8Path::to_path_buf))
                    .map_or_else(|| path.display().to_string(), |path| path.to_string());
                let sha256 = std::fs::read(&path)
                    .map(|bytes| Digest::of(&bytes).to_string())
                    .unwrap_or_default();
                snapshots.push(Snapshot {
                    id: setup_cmd::forge_snapshot_id(step),
                    kind: FORGE_STATE.to_owned(),
                    file: relative.clone(),
                    sha256,
                    taken_at: at.to_owned(),
                    second_copy: false,
                });
                rows.push(ForgeRow {
                    step: step.to_owned(),
                    state: "taken",
                    file: Some(relative),
                    detail: None,
                });
            }
            ForgeSnapshot::Unknown(detail) => rows.push(ForgeRow {
                step: step.to_owned(),
                state: "unknown",
                file: None,
                detail: Some(detail),
            }),
        }
    }
    rows
}

/// Take the forge snapshots an open migration still misses.
fn resume(target: &Utf8PathBuf, canonical: &Path, id: &str, out: Output) -> Result<(), RkError> {
    let workspace = workspace_of(canonical, id)?;
    if !workspace.join(migrate::REGISTRY_FILE).is_file() {
        return Err(migrate::workspace_missing(id, &workspace));
    }
    let mut registry = registry::load(&workspace)?;
    let now = std::time::SystemTime::now();
    // A forge snapshot whose file is gone or altered is no snapshot: it
    // leaves the registry, an altered file is set aside rather than
    // destroyed, and this run takes the snapshot again.
    let stamp = migrate::stamp(now);
    let mut aside = Vec::new();
    let mut kept = Vec::new();
    for snapshot in std::mem::take(&mut registry.snapshots) {
        let state = if snapshot.kind == FORGE_STATE {
            registry::integrity(&workspace, &snapshot)?
        } else {
            Integrity::Intact
        };
        match state {
            Integrity::Intact => kept.push(snapshot),
            Integrity::Missing => {}
            Integrity::Altered => {
                let moved = workspace.join(format!("{}.altered-{stamp}", snapshot.file));
                std::fs::rename(workspace.join(&snapshot.file), &moved)?;
                aside.push(moved);
            }
        }
    }
    registry.snapshots = kept;
    let held: Vec<String> = registry
        .snapshots
        .iter()
        .map(|snapshot| snapshot.id.clone())
        .collect();
    let saved = Saved::at(workspace.clone().into_std_path_buf());
    let at = migrate::at(now);
    let forge = take_forge(
        target,
        &workspace,
        &saved,
        &at,
        &held,
        &mut registry.snapshots,
    );
    registry::save(&workspace, &registry)?;
    for moved in &aside {
        out.result_line(format!(
            "set an altered snapshot aside at {moved}; its checksum no longer matched"
        ));
    }
    for row in &forge {
        out.result_line(format!("forge snapshot {}: {}", row.step, row.state));
    }
    if forge.is_empty() {
        out.result_line(format!(
            "the migration {id} holds every forge snapshot its destructive steps need"
        ));
    }
    let next = vec![format!(
        "rk migrate status --target {target} reads the registry"
    )];
    out.next(&next);
    out.emit(&ArchiveReport {
        schema: migrate::SCHEMA,
        mode: "resume",
        target: target.to_string(),
        id: id.to_owned(),
        workspace: workspace.to_string(),
        base_commit: registry.base_commit.clone(),
        archived: Vec::new(),
        in_place: Vec::new(),
        snapshots: registry
            .snapshots
            .iter()
            .map(|snapshot| snapshot.id.clone())
            .collect(),
        forge,
        next,
    })
}

/// Remove the directories the archive emptied, up to and never including
/// the target root.
fn prune_empty_parents(target: &Utf8Path, path: &str) {
    let mut current = Utf8Path::new(path).parent();
    while let Some(dir) = current.filter(|dir| !dir.as_str().is_empty()) {
        let full = target.join(dir);
        let empty = std::fs::read_dir(&full).is_ok_and(|mut entries| entries.next().is_none());
        if !empty || std::fs::remove_dir(&full).is_err() {
            break;
        }
        current = dir.parent();
    }
}

/// The commit the archive read its sources from: the first parent of the
/// oldest commit on `HEAD`'s first-parent history that names the
/// migration in the configuration. A merge is searched by its diff
/// against its first parent, so a key a side branch carried in is found
/// at the merge, whose first parent holds the trunk's bytes, never at the
/// side commit, whose parent may predate the trunk's later edits.
fn base_from_history(target: &Utf8Path, id: &str) -> Result<String, RkError> {
    let needle = format!("-Smigration = \"{id}\"");
    let commits = git::lines(
        target,
        &[
            "log",
            "--format=%H",
            "--reverse",
            "--first-parent",
            "--diff-merges=first-parent",
            needle.as_str(),
            "--",
            crate::config::CONFIG_PATH,
        ],
    )?;
    let first = commits.first().ok_or_else(|| {
        refused(
            Reason::PrerequisiteUnmet,
            format!(
                "no commit names the migration {id} in {}, so the base commit cannot be found (migration:a-rebuild-recreates-the-files-alone)",
                crate::config::CONFIG_PATH
            ),
            "the archive's configuration change committed",
            None,
        )
    })?;
    let parent = format!("{first}^1");
    Ok(git::lines(target, &["rev-parse", parent.as_str()])?
        .into_iter()
        .next()
        .unwrap_or_default())
}

/// SATISFIES migration:a-rebuild-recreates-the-files-alone
#[allow(
    clippy::too_many_lines,
    reason = "one pass reads the base, checks each file against the registry, and writes or previews it"
)]
fn rebuild_sources(target: &Utf8PathBuf, apply: bool, out: Output) -> Result<(), RkError> {
    let canonical = canonical(target)?;
    let id = migrate::open_id(target)?.ok_or_else(migrate::none_open)?;
    let workspace = workspace_of(&canonical, &id)?;
    let existing = if workspace.join(migrate::REGISTRY_FILE).is_file() {
        Some(registry::load(&workspace)?)
    } else {
        None
    };
    let base_commit = match &existing {
        Some(registry) => registry.base_commit.clone(),
        None => base_from_history(target, &id)?,
    };
    // Regular files alone, as the inventory lists them: a symlink or a
    // submodule at the base is no file the archive took.
    let blobs = git::regular_blobs(target, &base_commit, &[])?;
    let paths: Vec<&str> = blobs.keys().map(String::as_str).collect();
    let saved = Saved::at(workspace.clone().into_std_path_buf());
    let at = migrate::at(std::time::SystemTime::now());
    let mut sources = Vec::new();
    let mut rebuilt = Vec::new();
    let mut in_place = Vec::new();
    for path in inventory::listed(&paths) {
        let Some((tool, kind, placement)) = inventory::classify(path) else {
            continue;
        };
        // With a registry, the archive's own list is the subject, never a
        // file the base holds that the archive did not take.
        if existing
            .as_ref()
            .is_some_and(|registry| !registry.sources.iter().any(|source| source.path == path))
        {
            continue;
        }
        let object = format!("{base_commit}:{path}");
        let bytes = git::stdout(target, &["show", object.as_str()])?;
        let sha256 = Digest::of(&bytes).to_string();
        if let Some(registry) = &existing
            && let Some(source) = registry.sources.iter().find(|source| source.path == path)
            && source.sha256 != sha256
        {
            return Err(refused(
                Reason::StateDrift,
                format!(
                    "{path} at {base_commit} does not match the registry's checksum, and nothing was rebuilt from it"
                ),
                "the bytes the archive took",
                None,
            ));
        }
        match placement {
            Placement::Archived => {
                if apply {
                    saved.save(&Path::new(migrate::SOURCES_DIR).join(path), &bytes)?;
                }
                rebuilt.push(path.to_owned());
            }
            Placement::InPlace => in_place.push(path.to_owned()),
        }
        sources.push(source_of(path, &sha256, tool, kind, placement, &at));
    }
    if apply && existing.is_none() {
        registry::assign_ids(&mut sources);
        registry::save(
            &workspace,
            &Registry {
                schema_version: registry::SCHEMA_VERSION,
                id: id.clone(),
                base_commit: base_commit.clone(),
                sources,
                groups: Vec::new(),
                snapshots: Vec::new(),
            },
        )?;
    }
    let verb = if apply { "rebuilt" } else { "would rebuild" };
    for path in &rebuilt {
        out.result_line(format!("{verb} {path}"));
    }
    out.result_line(format!(
        "{verb} the files of the migration {id} at {workspace} from {base_commit}; a snapshot has no second copy, so none is rebuilt"
    ));
    let next = vec![if apply {
        format!("rk migrate status --target {target} reads the registry")
    } else {
        format!("rk migrate archive --rebuild --apply --target {target} writes them")
    }];
    out.next(&next);
    out.emit(&ArchiveReport {
        schema: migrate::SCHEMA,
        mode: if apply { "rebuild" } else { "preview" },
        target: target.to_string(),
        id,
        workspace: workspace.to_string(),
        base_commit,
        archived: rebuilt,
        in_place,
        snapshots: existing
            .map(|registry| {
                registry
                    .snapshots
                    .into_iter()
                    .map(|snapshot| snapshot.id)
                    .collect()
            })
            .unwrap_or_default(),
        forge: Vec::new(),
        next,
    })
}

/// The mark report.
#[derive(Debug, Serialize)]
struct MarkReport {
    schema: &'static str,
    id: String,
    status: Status,
    marked: Marked,
    next: Vec<String>,
}

/// SATISFIES migration:a-mark-appends-and-never-rewrites
fn mark(target: &Utf8PathBuf, request: &Mark<'_>, out: Output) -> Result<(), RkError> {
    let (id, workspace, mut registry) = open(target)?;
    let at = migrate::at(std::time::SystemTime::now());
    let marked = registry.mark(request, &at)?;
    registry::save(&workspace, &registry)?;
    let status = request.status.unwrap_or(Status::Pending);
    out.result_line(format!(
        "marked {} {} {}: {}",
        marked.subject,
        marked.id,
        status.as_str(),
        marked.sources.join(", ")
    ));
    let next = vec![format!(
        "rk migrate status --target {target} reads the registry"
    )];
    out.next(&next);
    out.emit(&MarkReport {
        schema: migrate::SCHEMA,
        id,
        status,
        marked,
        next,
    })
}

/// One source in the status report.
#[derive(Debug, Serialize)]
struct SourceRow {
    id: String,
    path: String,
    tool: String,
    kind: inventory::Kind,
    placement: Placement,
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    /// `same`, `changed`, or `missing`: the bytes now against the
    /// checksum the archive took.
    checksum: &'static str,
}

/// The status report.
#[derive(Debug, Serialize)]
struct StatusReport {
    schema: &'static str,
    id: String,
    workspace: String,
    base_commit: String,
    sources: Vec<SourceRow>,
    groups: Vec<registry::Group>,
    snapshots: Vec<Snapshot>,
    /// The destructive steps whose forge snapshot the registry lacks.
    forge_missing: Vec<String>,
    /// What still blocks the close.
    blocking: Vec<String>,
    next: Vec<String>,
}

/// Where the bytes of one source stand against its checksum.
fn checksum(target: &Utf8Path, workspace: &Utf8Path, source: &Source) -> &'static str {
    let path = match source.placement {
        Placement::Archived => workspace.join(migrate::SOURCES_DIR).join(&source.path),
        Placement::InPlace => target.join(&source.path),
    };
    match std::fs::read(&path) {
        Ok(bytes) if Digest::of(&bytes).to_string() == source.sha256 => "same",
        Ok(_) => "changed",
        Err(_) => "missing",
    }
}

/// The destructive steps whose snapshot the registry lacks, and every
/// reason the close refuses.
fn blocking(
    target: &Utf8PathBuf,
    workspace: &Utf8Path,
    registry: &Registry,
) -> Result<(Vec<String>, Vec<String>), RkError> {
    let mut held: Vec<&str> = Vec::new();
    for snapshot in &registry.snapshots {
        if registry::integrity(workspace, snapshot)? == Integrity::Intact {
            held.push(snapshot.id.as_str());
        }
    }
    let forge_missing: Vec<String> = setup_cmd::destructive_steps(target)
        .unwrap_or_default()
        .into_iter()
        .filter(|step| !held.contains(&setup_cmd::forge_snapshot_id(step).as_str()))
        .map(str::to_owned)
        .collect();
    let mut reasons = registry.unfinished(target);
    match setup_cmd::proof_word(target) {
        Ok("compatible") => {}
        Ok(word) => reasons.push(format!(
            "the setup proof is {word}; rk setup checkpoint on a host that holds the bot's key records the setup as satisfied"
        )),
        Err(error) => reasons.push(format!("the setup contract does not resolve: {error}")),
    }
    Ok((forge_missing, reasons))
}

fn status(target: &Utf8PathBuf, out: Output) -> Result<(), RkError> {
    let (id, workspace, registry) = open(target)?;
    let sources: Vec<SourceRow> = registry
        .sources
        .iter()
        .map(|source| SourceRow {
            id: source.id.clone(),
            path: source.path.clone(),
            tool: source.tool.clone(),
            kind: source.kind,
            placement: source.placement,
            status: source.status,
            group: source.group.clone(),
            tags: source.tags.clone(),
            checksum: checksum(target, &workspace, source),
        })
        .collect();
    let (forge_missing, reasons) = blocking(target, &workspace, &registry)?;
    out.result_line(format!(
        "migration {id} at {workspace}, base {}",
        registry.base_commit
    ));
    for row in &sources {
        out.result_line(format!(
            "{} {} ({}{}){}",
            row.status.as_str(),
            row.path,
            row.group
                .as_deref()
                .map_or_else(|| "no group".to_owned(), |group| format!("group {group}")),
            if row.tags.is_empty() {
                String::new()
            } else {
                format!("; tags {}", row.tags.join(", "))
            },
            if row.checksum == "same" {
                String::new()
            } else {
                format!(" — checksum {}", row.checksum)
            }
        ));
    }
    for step in &forge_missing {
        out.result_line(format!(
            "no forge snapshot of {step}: the step stays refused"
        ));
    }
    for reason in &reasons {
        out.result_line(format!("blocks the close: {reason}"));
    }
    let next = if reasons.is_empty() {
        vec![format!(
            "rk migrate close --target {target} previews the close"
        )]
    } else {
        vec![format!(
            "rk migrate mark <source-or-group> <status> --target {target} records the next step"
        )]
    };
    out.next(&next);
    out.emit(&StatusReport {
        schema: migrate::SCHEMA,
        id,
        workspace: workspace.to_string(),
        base_commit: registry.base_commit.clone(),
        sources,
        groups: registry.groups.clone(),
        snapshots: registry.snapshots,
        forge_missing,
        blocking: reasons,
        next,
    })
}

/// The close report.
#[derive(Debug, Serialize)]
struct CloseReport {
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    id: String,
    workspace: String,
    closed: bool,
    next: Vec<String>,
}

/// SATISFIES migration:close-refuses-an-unfinished-migration
fn close(target: &Utf8PathBuf, apply: bool, out: Output) -> Result<(), RkError> {
    let (id, workspace, registry) = open(target)?;
    let (_, reasons) = blocking(target, &workspace, &registry)?;
    if !reasons.is_empty() {
        return Err(refused(
            Reason::PrerequisiteUnmet,
            format!(
                "the migration {id} cannot close: {} (migration:close-refuses-an-unfinished-migration)",
                reasons.join("; ")
            ),
            "every source verified or dropped, each verified source's group naming destinations the target holds, and a compatible setup proof",
            Some(format!(
                "rk migrate status --target {target} lists what is left"
            )),
        ));
    }
    if apply {
        crate::config::set_migration(target.as_std_path(), None)?;
        out.result_line(format!(
            "closed the migration {id}; .release-kit/config.toml no longer names it"
        ));
    } else {
        out.result_line(format!("the migration {id} can close"));
    }
    let next = if apply {
        vec![
            format!("keep or remove the workspace at {workspace}; its removal is the operator's"),
            "the first release through the rk-release skill is the proof".to_owned(),
        ]
    } else {
        vec![format!(
            "rk migrate close --apply --target {target} removes the key"
        )]
    };
    out.next(&next);
    out.emit(&CloseReport {
        schema: migrate::SCHEMA,
        mode: if apply { "apply" } else { "preview" },
        id,
        workspace: workspace.to_string(),
        closed: apply,
        next,
    })
}
