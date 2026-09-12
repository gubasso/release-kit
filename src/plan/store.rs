//! The plan store: one directory per plan under the state root, holding
//! the plan, the request that computed it, and every byte it names.
//!
//! A stored plan is the thing that was reviewed, and an apply executes
//! exactly its operations with exactly its bytes. Plans carry bytes from
//! the target, some of which are not public, so the store is owner-only,
//! the JSON view carries digests and bounded text rather than blobs, and
//! a plan is ephemeral: never committed and never posted to a forge.
//! Retention is bounded the way the run journal is: the newest
//! [`PLANS_KEPT`] plans stay, pruned after every persist.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::applog;
use crate::atomic;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;

use super::{Plan, PlanRequest, Planned};

/// How many plans the store keeps; a persist past the cap prunes the
/// oldest first.
pub const PLANS_KEPT: usize = 20;

/// The plan document inside a plan's directory.
pub const PLAN_FILE: &str = "plan.json";
/// The request that computed it, for the revalidation.
pub const REQUEST_FILE: &str = "request.json";
/// The blobs, one file per digest.
pub const BLOBS_DIR: &str = "blobs";

/// A plan read back from the store.
#[derive(Debug)]
pub struct Stored {
    /// The plan as persisted.
    pub plan: Plan,
    /// The request that computed it.
    pub request: PlanRequest,
    /// Every blob the plan names, by digest.
    pub blobs: BTreeMap<Digest, Vec<u8>>,
    /// The plan's directory.
    pub dir: PathBuf,
}

/// The store root: `<state root>/plans`.
#[must_use]
pub fn plans_root() -> Option<PathBuf> {
    applog::state_root().map(|root| root.join("plans"))
}

/// Persist one computed plan with its request and its blobs, then prune
/// the store to its cap. The directory is created owner-only.
///
/// # Errors
///
/// Returns a refusal with [`Reason::JournalUnavailable`] where no state
/// root resolves, and [`RkError::Io`] for a write that fails.
pub fn persist(planned: &Planned, request: &PlanRequest) -> Result<PathBuf, RkError> {
    let root = plans_root().ok_or_else(no_root)?;
    fs::create_dir_all(&root)?;
    restrict_dir(&root);
    let dir = root.join(&planned.plan.identity.plan_id);
    let staging = root.join(format!(".{}.staging", planned.plan.identity.plan_id));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir(&staging)?;
    restrict_dir(&staging);
    let blobs = staging.join(BLOBS_DIR);
    fs::create_dir(&blobs)?;
    restrict_dir(&blobs);
    for (digest, bytes) in &planned.blobs {
        let path = blobs.join(digest.to_string());
        atomic::write(&path, bytes)?;
        restrict_file(&path);
    }
    let request_json = serde_json::to_vec_pretty(request).map_err(anyhow::Error::from)?;
    atomic::write(&staging.join(REQUEST_FILE), &request_json)?;
    restrict_file(&staging.join(REQUEST_FILE));
    let plan_json = serde_json::to_vec_pretty(&planned.plan).map_err(anyhow::Error::from)?;
    atomic::write(&staging.join(PLAN_FILE), &plan_json)?;
    restrict_file(&staging.join(PLAN_FILE));
    // The same id names the same fingerprint at the same instant, so a
    // directory already there holds this very plan: a second writer
    // keeps it and drops its own staging rather than racing the first.
    if let Err(error) = fs::rename(&staging, &dir) {
        let _ = fs::remove_dir_all(&staging);
        if !dir.join(PLAN_FILE).is_file() {
            return Err(RkError::Io(error));
        }
    }
    let _ = prune_to(PLANS_KEPT);
    Ok(dir)
}

/// Read one plan back by id.
///
/// # Errors
///
/// Returns [`RkError::Missing`] naming the id and the retention rule
/// where no plan directory holds it, a refusal with
/// [`Reason::UnsupportedSchema`] where the stored document is not one
/// this engine reads, and [`RkError::Io`] for a read that fails.
pub fn load(id: &str) -> Result<Stored, RkError> {
    // An id is one directory name under the store, never a path.
    if id.contains(['/', '\\']) || id == ".." || id == "." || id.is_empty() {
        return Err(unknown(id));
    }
    let root = plans_root().ok_or_else(no_root)?;
    let dir = root.join(id);
    let plan_bytes = match fs::read(dir.join(PLAN_FILE)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(unknown(id)),
        Err(error) => return Err(RkError::Io(error)),
    };
    let plan: Plan = serde_json::from_slice(&plan_bytes).map_err(|error| {
        RkError::refusal(
            Diagnostic::new(
                Reason::UnsupportedSchema,
                format!("plan {id} does not read as {}: {error}", super::PLAN_SCHEMA),
            )
            .expected("a plan this engine stored")
            .action("rk reconcile plan computes a fresh one"),
        )
    })?;
    if plan.schema != super::PLAN_SCHEMA {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::UnsupportedSchema,
                format!(
                    "plan {id} declares {} and this engine reads {}",
                    plan.schema,
                    super::PLAN_SCHEMA
                ),
            )
            .expected("a plan computed by this engine")
            .action("rk reconcile plan computes a fresh one"),
        ));
    }
    let request: PlanRequest =
        serde_json::from_slice(&fs::read(dir.join(REQUEST_FILE))?).map_err(anyhow::Error::from)?;
    let mut blobs = BTreeMap::new();
    let blobs_dir = dir.join(BLOBS_DIR);
    if blobs_dir.is_dir() {
        for entry in fs::read_dir(&blobs_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(digest) = Digest::parse(&name) {
                blobs.insert(digest, fs::read(entry.path())?);
            }
        }
    }
    Ok(Stored {
        plan,
        request,
        blobs,
        dir,
    })
}

/// Every stored plan, oldest first, as `(created_at, id)`.
#[must_use]
pub fn list() -> Vec<(String, String)> {
    let Some(root) = plans_root() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut plans: Vec<(String, String)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            if id.starts_with('.') {
                return None;
            }
            let bytes = fs::read(entry.path().join(PLAN_FILE)).ok()?;
            let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
            let created = value["identity"]["created_at"].as_str()?.to_owned();
            Some((created, id))
        })
        .collect();
    plans.sort();
    plans
}

/// Remove the oldest plans past `keep`; the count removed.
#[must_use]
pub fn prune_to(keep: usize) -> usize {
    let Some(root) = plans_root() else { return 0 };
    let plans = list();
    let excess = plans.len().saturating_sub(keep);
    let mut removed = 0;
    for (_, id) in plans.into_iter().take(excess) {
        if fs::remove_dir_all(root.join(id)).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn no_root() -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::JournalUnavailable,
            "neither XDG_STATE_HOME nor HOME is set; the plan store has no root",
        )
        .expected("a state root for the plan store"),
    )
}

fn unknown(id: &str) -> RkError {
    RkError::missing(
        Diagnostic::new(
            Reason::Usage,
            format!(
                "no stored plan {id}: the store keeps the newest {PLANS_KEPT} plans and prunes the rest after every persist"
            ),
        )
        .expected("the id rk reconcile plan printed, still within the retention window")
        .action("rk reconcile plan computes and stores a fresh plan"),
    )
}

/// 0700 on a store directory.
fn restrict_dir(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// 0600 on a stored file: data, not an executable.
fn restrict_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}
