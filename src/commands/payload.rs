//! `rk payload`: what this binary carries, provably, and what another
//! release carries, through the seam.
//!
//! The version alone does not identify a payload — two locally built
//! binaries can share a Cargo version while embedding different bytes —
//! so the report carries a digest per artifact and one aggregate over the
//! ordered list, computed at runtime over the bytes. A landing record, a
//! bug report, or a comparison between two installs can then name the
//! payload it actually saw. `--release <version>` answers the same
//! document for a release this binary does not carry, which is the first
//! proof that the engine reads a bundle it was not compiled with.

use crate::cli::payload::PayloadArgs;
use crate::error::RkError;
use crate::output::Output;
use crate::release::{CrateReleaseSource, EmbeddedReleaseSource, ReleaseManifest, ReleaseSource};

/// The machine form of the payload report, `rk.payload/1`: the release
/// manifest itself.
pub type Report = ReleaseManifest;

/// The report over the embedded payload.
#[must_use]
pub fn report() -> Report {
    EmbeddedReleaseSource::manifest_ref().clone()
}

/// Print the payload report.
///
/// # Errors
///
/// Returns [`RkError::Other`] when the report cannot serialize, which is a
/// defect in this binary rather than anything a caller can correct, and
/// the crate source's refusals under `--release`.
pub fn run(args: &PayloadArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let (report, source_line) = match args.release.as_deref() {
        None => (report(), None),
        Some(selector) => {
            let source = CrateReleaseSource::new(selector)?;
            let manifest = source.manifest()?;
            let resolved = source.resolve()?;
            let line = format!(
                "source crates {} ({}, {})",
                resolved.version,
                if resolved.index_fetched {
                    "index fetched"
                } else {
                    "index cached"
                },
                if resolved.archive_fetched {
                    "archive fetched and verified"
                } else {
                    "archive cached"
                }
            );
            (manifest, Some(line))
        }
    };
    out.result_line(format!("release-kit {}", report.release_kit_version));
    if let Some(line) = source_line {
        out.result_line(line);
    }
    out.result_line(format!("payload sha256 {}", report.payload_sha256));
    for root in crate::payload_roots::PAYLOAD_ROOTS {
        let count = report
            .artifacts
            .iter()
            .filter(|a| a.path == root || a.path.starts_with(&format!("{root}/")))
            .count();
        let noun = if count == 1 { "file" } else { "files" };
        out.result_line(format!("{root}: {count} {noun}"));
    }
    out.emit(&report)
}

#[cfg(test)]
mod tests {
    use super::{Report, report};
    use crate::digest::Digest;
    use crate::release::{Artifact, aggregate};

    /// The complete `rk.payload/1` shape, held by snapshot against fixture
    /// values, beside the live test that checks the real digests.
    #[test]
    fn the_payload_report_schema_snapshot_holds() {
        let fixture = Report::new(
            "0.0.0".into(),
            1,
            vec![Artifact {
                path: "versions.toml".into(),
                sha256: Digest::of(b""),
            }],
        );
        assert_eq!(
            fixture.payload_sha256,
            aggregate(&fixture.artifacts),
            "the aggregate is computed, never supplied"
        );
        let mut fixture = fixture;
        fixture.payload_sha256 = Digest::of(b"");
        assert_eq!(
            serde_json::to_string(&fixture).expect("a report serializes"),
            r#"{"release_kit_version":"0.0.0","payload_schema":1,"payload_sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","artifacts":[{"path":"versions.toml","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}]}"#
        );
    }

    #[test]
    fn the_report_names_the_cargo_version_and_every_artifact() {
        let report = report();
        assert_eq!(report.release_kit_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(report.payload_schema, crate::release::PAYLOAD_SCHEMA);
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
