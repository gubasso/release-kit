//! Gathering the target evidence a projection consumes.
//!
//! The one place on the projection path that reads a target. It runs
//! before [`Projection::compute`](super::Projection::compute) and returns
//! values, so the projection itself stays pure and a source scan can hold
//! `src/projection.rs` to that boundary without excluding this file's
//! reads by name.

use std::collections::BTreeMap;

use camino::Utf8Path;

use super::{BLOCK_DESTINATIONS, CrateShape, TargetEvidence};
use crate::landing::manifest::Manifest;

impl TargetEvidence {
    /// The evidence at `target`, read once, with `recorded` answering
    /// whether the receipt already names the flake pair.
    ///
    /// # Errors
    ///
    /// Any read failure other than a file being absent.
    pub fn gather(target: &Utf8Path, recorded: Option<&Manifest>) -> std::io::Result<Self> {
        gather(target, recorded)
    }
}

/// The evidence at `target`: every block destination's existing document,
/// the crate shape, the flake pair's presence, and whether `recorded`
/// names `flake.nix`.
///
/// # Errors
///
/// Any read failure other than a file being absent.
pub fn gather(target: &Utf8Path, recorded: Option<&Manifest>) -> std::io::Result<TargetEvidence> {
    let mut documents = BTreeMap::new();
    for destination in BLOCK_DESTINATIONS {
        if let Some(bytes) = read_optional(&target.join(destination))? {
            documents.insert(destination.to_owned(), bytes);
        }
    }
    let (flake_nix_present, flake_lock_present) = flake_presence(target)?;
    Ok(TargetEvidence {
        documents,
        crate_shape: crate_shape(target),
        flake_nix_present,
        flake_lock_present,
        flake_recorded: flake_recorded(recorded),
    })
}

/// Whether the receipt names `flake.nix`: a pair release-kit landed is its
/// own and is never withheld.
#[must_use]
pub fn flake_recorded(recorded: Option<&Manifest>) -> bool {
    recorded.is_some_and(|record| record.file("flake.nix").is_some())
}

/// The crate facts at `target` the Nix seed relies on. An unreadable
/// `Cargo.toml` is `None`, which the judgment names.
#[must_use]
pub fn crate_shape(target: &Utf8Path) -> CrateShape {
    CrateShape {
        cargo_toml: std::fs::read_to_string(target.join("Cargo.toml")).ok(),
        cargo_lock: target.join("Cargo.lock").is_file(),
        main_rs: target.join("src/main.rs").is_file(),
    }
}

/// Whether `flake.nix` and `flake.lock` are present at `target`, a link
/// or any other entry counted as present: a seed lock beside anything
/// bearing that name describes the wrong input graph.
///
/// # Errors
///
/// Any read failure other than the entry being absent.
pub fn flake_presence(target: &Utf8Path) -> std::io::Result<(bool, bool)> {
    let present = |name: &str| match std::fs::symlink_metadata(target.join(name).as_std_path()) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    };
    Ok((present("flake.nix")?, present("flake.lock")?))
}

/// The bytes at `path`, or `None` where no file exists.
fn read_optional(path: &Utf8Path) -> std::io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
