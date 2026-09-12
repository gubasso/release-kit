//! The seam between the engine and any release bundle.
//!
//! A release bundle is the payload of one release-kit release: every root
//! `src/payload_roots.rs` declares, at that release's bytes. The engine
//! reads a bundle through [`ReleaseSource`] and through nothing else, so
//! the same planner describes the release compiled into this binary, a
//! release fetched from the crates venue, and a directory a test wrote.
//! The trait is two methods on purpose: the manifest, and a blob by
//! digest. Every method on the seam is a promise every source must keep.
//!
//! `payload_schema` is the protocol version between an engine and a
//! bundle. An engine reads any bundle whose schema is at or below its own,
//! and refuses a newer one by naming the engine version to install: that
//! is the whole compatibility rule, and the only case where a newer
//! binary must be obtained.

pub mod crate_source;
pub mod dir;
pub mod embedded;

use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;

pub use crate_source::CrateReleaseSource;
pub use dir::DirReleaseSource;
pub use embedded::EmbeddedReleaseSource;

/// The version of the manifest's shape and of the bundle protocol.
///
/// The one number an engine compares before it reads a bundle. The
/// constant is declared with this exact spelling because a bundle's own
/// copy is read back out of its sources by [`dir::declared_schema`].
pub const PAYLOAD_SCHEMA: u32 = 1;

/// One artifact of a bundle and the digest of its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// The artifact's path, carrying its payload root as the first segment.
    pub path: String,
    /// SHA-256 of the bytes.
    pub sha256: Digest,
}

/// What identifies one release bundle: the `rk.payload/1` document, as
/// `rk payload --json` has always emitted it, promoted to a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    /// The release's version.
    pub release_kit_version: String,
    /// The bundle protocol version.
    pub payload_schema: u32,
    /// One digest over the ordered artifact list, identifying the payload
    /// as a whole.
    pub payload_sha256: Digest,
    /// Every artifact, in root order and sorted within each root.
    pub artifacts: Vec<Artifact>,
}

impl ReleaseManifest {
    /// Build the manifest over an artifact list already in root order.
    #[must_use]
    pub fn new(release_kit_version: String, payload_schema: u32, artifacts: Vec<Artifact>) -> Self {
        Self {
            release_kit_version,
            payload_schema,
            payload_sha256: aggregate(&artifacts),
            artifacts,
        }
    }

    /// The artifact at one path.
    #[must_use]
    pub fn artifact(&self, path: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|artifact| artifact.path == path)
    }

    /// The artifacts under one directory prefix, as `(path below the
    /// prefix, artifact)`, in manifest order.
    pub fn under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, &'a Artifact)> {
        self.artifacts.iter().filter_map(move |artifact| {
            artifact
                .path
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix('/'))
                .map(|rest| (rest, artifact))
        })
    }

    /// The immediate child directories of one prefix, deduplicated, in
    /// manifest order.
    #[must_use]
    pub fn dirs_under(&self, prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (rest, _) in self.under(prefix) {
            if let Some((dir, _)) = rest.split_once('/') {
                if !out.iter().any(|known| known == dir) {
                    out.push(dir.to_owned());
                }
            }
        }
        out
    }

    /// Refuse a bundle whose protocol is newer than this engine's.
    ///
    /// # Errors
    ///
    /// Returns [`RkError::Refusal`] naming the bundle's version as the
    /// engine to install when its schema exceeds [`PAYLOAD_SCHEMA`].
    pub fn check_schema(&self) -> Result<(), RkError> {
        check_schema(self.payload_schema, &self.release_kit_version)
    }
}

/// The protocol rule, as one function both sides of the boundary test.
///
/// # Errors
///
/// Returns [`RkError::Refusal`] when `schema` exceeds [`PAYLOAD_SCHEMA`].
pub fn check_schema(schema: u32, version: &str) -> Result<(), RkError> {
    if schema <= PAYLOAD_SCHEMA {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::UnsupportedSchema,
            format!(
                "the bundle for release-kit {version} declares payload schema {schema}, and this engine reads schema {PAYLOAD_SCHEMA} at most"
            ),
        )
        .expected("a bundle whose payload schema is at or below the engine's")
        .action(format!(
            "install release-kit {version} or newer; an engine reads any bundle at or below its own schema, and no older engine can read this one"
        )),
    ))
}

/// The aggregate digest: SHA-256 over one `<path>\n<sha256>\n` record per
/// artifact, in list order. Any change to any artifact, any rename, and
/// any reordering of the roots changes it.
#[must_use]
pub fn aggregate(artifacts: &[Artifact]) -> Digest {
    let mut lines = String::new();
    for artifact in artifacts {
        lines.push_str(&artifact.path);
        lines.push('\n');
        lines.push_str(&artifact.sha256.to_string());
        lines.push('\n');
    }
    Digest::of(lines.as_bytes())
}

/// One release bundle, read by the engine.
pub trait ReleaseSource {
    /// The bundle's manifest.
    ///
    /// # Errors
    ///
    /// A source that cannot describe itself — an unreadable directory, an
    /// unverifiable fetch — fails here, before any blob is asked for.
    fn manifest(&self) -> Result<ReleaseManifest, RkError>;

    /// The bytes behind one digest the manifest names.
    ///
    /// # Errors
    ///
    /// Returns [`RkError::NotFound`] for a digest the bundle does not
    /// carry, and the source's own failure for bytes it cannot read.
    fn blob(&self, digest: &Digest) -> Result<Vec<u8>, RkError>;
}

impl ReleaseSource for &dyn ReleaseSource {
    fn manifest(&self) -> Result<ReleaseManifest, RkError> {
        (**self).manifest()
    }

    fn blob(&self, digest: &Digest) -> Result<Vec<u8>, RkError> {
        (**self).blob(digest)
    }
}

/// The bytes of one artifact by path, through the manifest.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for a path the manifest does not name,
/// and the source's failures otherwise.
pub fn read(
    source: &dyn ReleaseSource,
    manifest: &ReleaseManifest,
    path: &str,
) -> Result<Vec<u8>, RkError> {
    let artifact = manifest.artifact(path).ok_or_else(|| RkError::NotFound {
        kind: "artifact",
        name: path.to_owned(),
    })?;
    source.blob(&artifact.sha256)
}

/// The blob refusal every source shares for a digest it does not carry.
pub(crate) fn unknown_digest(digest: &Digest) -> RkError {
    RkError::NotFound {
        kind: "blob",
        name: digest.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Artifact, PAYLOAD_SCHEMA, ReleaseManifest, check_schema};
    use crate::digest::Digest;

    fn manifest() -> ReleaseManifest {
        ReleaseManifest::new(
            "0.0.0".into(),
            PAYLOAD_SCHEMA,
            vec![
                Artifact {
                    path: "snippets/_shared/github/SECURITY.md".into(),
                    sha256: Digest::of(b"a"),
                },
                Artifact {
                    path: "snippets/rust/github/release-plz.toml".into(),
                    sha256: Digest::of(b"b"),
                },
                Artifact {
                    path: "versions.toml".into(),
                    sha256: Digest::of(b"c"),
                },
            ],
        )
    }

    #[test]
    fn an_engine_reads_a_bundle_at_or_below_its_schema() {
        assert!(check_schema(PAYLOAD_SCHEMA, "9.9.9").is_ok());
        assert!(check_schema(0, "0.0.1").is_ok());
        assert!(manifest().check_schema().is_ok());
    }

    #[test]
    fn an_engine_refuses_a_newer_schema_naming_the_engine_to_install() {
        let err = check_schema(PAYLOAD_SCHEMA + 1, "9.9.9").expect_err("a newer schema refuses");
        assert_eq!(err.exit_code(), 73);
        assert_eq!(err.reason(), crate::diagnostic::Reason::UnsupportedSchema);
        let text = err.to_string();
        assert!(text.contains("release-kit 9.9.9"), "{text}");
        assert!(
            text.contains(&format!("schema {}", PAYLOAD_SCHEMA + 1)),
            "{text}"
        );
        let action = err
            .diagnostic()
            .action
            .expect("the refusal names the engine to install");
        assert!(action.contains("install release-kit 9.9.9"), "{action}");
    }

    #[test]
    fn the_manifest_lists_under_a_prefix() {
        let manifest = manifest();
        assert_eq!(manifest.dirs_under("snippets"), ["_shared", "rust"]);
        assert_eq!(manifest.dirs_under("snippets/rust"), ["github"]);
        let under: Vec<&str> = manifest
            .under("snippets/rust/github")
            .map(|(p, _)| p)
            .collect();
        assert_eq!(under, ["release-plz.toml"]);
        assert!(manifest.artifact("versions.toml").is_some());
        assert!(manifest.artifact("versions").is_none());
    }
}
