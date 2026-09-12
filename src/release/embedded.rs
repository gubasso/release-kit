//! The bundle compiled into this binary, behind the seam.
//!
//! The only source the front verbs read in the ordinary case: the
//! embedded roots of `src/embedded.rs`, described by the same manifest
//! `rk payload --json` prints. It answers both methods from memory, so a
//! landing through the seam costs one digest pass over the payload and no
//! I/O.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::digest::Digest;
use crate::embedded;
use crate::error::RkError;

use super::{Artifact, PAYLOAD_SCHEMA, ReleaseManifest, ReleaseSource, unknown_digest};

/// The embedded release.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddedReleaseSource;

/// The one manifest over the embedded bytes, computed once per process.
fn manifest() -> &'static ReleaseManifest {
    static MANIFEST: OnceLock<ReleaseManifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let artifacts = embedded::artifacts()
            .into_iter()
            .map(|(path, bytes)| Artifact {
                path,
                sha256: Digest::of(bytes),
            })
            .collect();
        ReleaseManifest::new(
            env!("CARGO_PKG_VERSION").to_owned(),
            PAYLOAD_SCHEMA,
            artifacts,
        )
    })
}

/// The embedded bytes by digest, computed once per process.
fn blobs() -> &'static HashMap<Digest, &'static [u8]> {
    static BLOBS: OnceLock<HashMap<Digest, &'static [u8]>> = OnceLock::new();
    BLOBS.get_or_init(|| {
        embedded::artifacts()
            .into_iter()
            .map(|(_, bytes)| (Digest::of(bytes), bytes))
            .collect()
    })
}

impl EmbeddedReleaseSource {
    /// The manifest, borrowed for the process's lifetime.
    #[must_use]
    pub fn manifest_ref() -> &'static ReleaseManifest {
        manifest()
    }
}

impl ReleaseSource for EmbeddedReleaseSource {
    fn manifest(&self) -> Result<ReleaseManifest, RkError> {
        Ok(manifest().clone())
    }

    fn blob(&self, digest: &Digest) -> Result<Vec<u8>, RkError> {
        blobs()
            .get(digest)
            .map(|bytes| bytes.to_vec())
            .ok_or_else(|| unknown_digest(digest))
    }
}

#[cfg(test)]
mod tests {
    use super::EmbeddedReleaseSource;
    use crate::digest::Digest;
    use crate::release::ReleaseSource;

    #[test]
    fn the_embedded_source_serves_every_artifact_it_names() {
        let source = EmbeddedReleaseSource;
        let manifest = source
            .manifest()
            .expect("the embedded bundle describes itself");
        assert_eq!(manifest.release_kit_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(manifest.payload_schema, crate::release::PAYLOAD_SCHEMA);
        assert!(!manifest.artifacts.is_empty());
        for artifact in &manifest.artifacts {
            let bytes = source.blob(&artifact.sha256).expect("a named blob reads");
            assert_eq!(Digest::of(&bytes), artifact.sha256, "{}", artifact.path);
        }
        assert!(source.blob(&Digest::of(b"absent")).is_err());
    }
}
