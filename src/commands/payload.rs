//! `rk payload`: what this binary carries, provably.
//!
//! The version alone does not identify a payload — two locally built
//! binaries can share a Cargo version while embedding different bytes —
//! so the report carries a digest per artifact and one aggregate over the
//! ordered list, computed at runtime over the embedded bytes. A landing
//! record, a bug report, or a comparison between two installs can then
//! name the payload it actually saw.

use serde::Serialize;

use crate::cli::payload::PayloadArgs;
use crate::digest::Digest;
use crate::embedded;
use crate::error::RkError;

/// The version of this report's shape, not of the payload it describes; a
/// consumer is told when the shape changes without being told when the
/// content does.
const PAYLOAD_SCHEMA: u32 = 1;

/// One embedded file and the digest of its bytes.
#[derive(Debug, Serialize)]
pub struct Artifact {
    /// The artifact's path, carrying its payload root as the first segment.
    pub path: String,
    /// SHA-256 of the embedded bytes.
    pub sha256: Digest,
}

/// The machine form of the payload report.
#[derive(Debug, Serialize)]
pub struct Report {
    /// The one version, from `CARGO_PKG_VERSION` and nowhere else.
    pub release_kit_version: &'static str,
    /// The version of this document's shape.
    pub payload_schema: u32,
    /// One digest over the ordered artifact list, identifying the payload
    /// as a whole.
    pub payload_sha256: Digest,
    /// Every embedded artifact, in root order and sorted within each root.
    pub artifacts: Vec<Artifact>,
}

/// Build the report over the embedded payload.
#[must_use]
pub fn report() -> Report {
    let artifacts: Vec<Artifact> = embedded::artifacts()
        .into_iter()
        .map(|(path, bytes)| Artifact {
            path,
            sha256: Digest::of(bytes),
        })
        .collect();
    Report {
        release_kit_version: env!("CARGO_PKG_VERSION"),
        payload_schema: PAYLOAD_SCHEMA,
        payload_sha256: aggregate(&artifacts),
        artifacts,
    }
}

/// The aggregate digest: SHA-256 over one `<path>\n<sha256>\n` record per
/// artifact, in list order. Any change to any artifact, any rename, and
/// any reordering of the roots changes it.
fn aggregate(artifacts: &[Artifact]) -> Digest {
    let mut lines = String::new();
    for artifact in artifacts {
        lines.push_str(&artifact.path);
        lines.push('\n');
        lines.push_str(&artifact.sha256.to_string());
        lines.push('\n');
    }
    Digest::of(lines.as_bytes())
}

/// Print the payload report.
///
/// # Errors
///
/// Returns [`RkError::Other`] when the report cannot serialize, which is a
/// defect in this binary rather than anything a caller can correct.
pub fn run(args: &PayloadArgs) -> Result<(), RkError> {
    let report = report();
    if args.json {
        let text = serde_json::to_string_pretty(&report).map_err(anyhow::Error::from)?;
        println!("{text}");
        return Ok(());
    }
    println!("release-kit {}", report.release_kit_version);
    println!("payload sha256 {}", report.payload_sha256);
    for root in embedded::PAYLOAD_ROOTS {
        let count = report
            .artifacts
            .iter()
            .filter(|a| a.path == root || a.path.starts_with(&format!("{root}/")))
            .count();
        let noun = if count == 1 { "file" } else { "files" };
        println!("{root}: {count} {noun}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{aggregate, report};

    #[test]
    fn the_report_names_the_cargo_version_and_every_artifact() {
        let report = report();
        assert_eq!(report.release_kit_version, env!("CARGO_PKG_VERSION"));
        assert!(!report.artifacts.is_empty());
        assert_eq!(report.payload_sha256, aggregate(&report.artifacts));
    }

    /// The aggregate must see renames and reorders, not only content.
    #[test]
    fn the_aggregate_covers_paths_and_order() {
        let mut artifacts = report().artifacts;
        let original = aggregate(&artifacts);
        artifacts.reverse();
        assert_ne!(original, aggregate(&artifacts));
    }
}
