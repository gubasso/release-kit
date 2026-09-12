//! A release bundle laid out as a directory, behind the seam.
//!
//! The directory is the unpacked crate of one release: `Cargo.toml` at its
//! root, the payload roots beside it, and the sources the crate ships.
//! The crate source reads its cache through this type, and a test writes
//! a directory to serve any bundle it needs, so the two share one reader
//! and one set of failures. Nothing here is executed: the directory is
//! data, read root by root in inventory order.

use std::path::{Path, PathBuf};

use crate::digest::Digest;
use crate::error::RkError;
use crate::payload_roots::PAYLOAD_ROOTS;

use super::{Artifact, ReleaseManifest, ReleaseSource, unknown_digest};

/// One bundle under a directory.
#[derive(Debug, Clone)]
pub struct DirReleaseSource {
    root: PathBuf,
}

impl DirReleaseSource {
    /// A source over `root`, which is read on each call and never cached:
    /// the callers hold the manifest they need, and a directory is cheap.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory the bundle is read from.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every artifact under the payload roots, root by root in inventory
    /// order, sorted by path within each root, with its bytes.
    fn artifacts(&self) -> Result<Vec<(String, Vec<u8>)>, RkError> {
        let mut out = Vec::new();
        for root in PAYLOAD_ROOTS {
            let path = self.root.join(root);
            if path.is_file() {
                out.push((root.to_string(), std::fs::read(&path)?));
                continue;
            }
            if !path.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            walk(&path, &mut files)?;
            files.sort();
            for file in files {
                let rel = file
                    .strip_prefix(&self.root)
                    .map_err(|_| anyhow::anyhow!("{} is outside the bundle", file.display()))?
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, std::fs::read(&file)?));
            }
        }
        Ok(out)
    }
}

/// Every regular file below `dir`, recursively.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

impl ReleaseSource for DirReleaseSource {
    fn manifest(&self) -> Result<ReleaseManifest, RkError> {
        let version = declared_version(&self.root)?;
        let schema = declared_schema(&self.root)?;
        let artifacts = self
            .artifacts()?
            .into_iter()
            .map(|(path, bytes)| Artifact {
                path,
                sha256: Digest::of(&bytes),
            })
            .collect();
        Ok(ReleaseManifest::new(version, schema, artifacts))
    }

    fn blob(&self, digest: &Digest) -> Result<Vec<u8>, RkError> {
        self.artifacts()?
            .into_iter()
            .map(|(_, bytes)| bytes)
            .find(|bytes| Digest::of(bytes) == *digest)
            .ok_or_else(|| unknown_digest(digest))
    }
}

/// The version the bundle's `Cargo.toml` declares.
///
/// # Errors
///
/// Returns [`RkError::Other`] when the manifest is absent, unparsable, or
/// names no package version.
pub fn declared_version(root: &Path) -> Result<String, RkError> {
    let path = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| anyhow::anyhow!("{}: {source}", path.display()))?;
    let table: toml::Table = text
        .parse()
        .map_err(|source| anyhow::anyhow!("{}: {source}", path.display()))?;
    table
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{}: no package version", path.display()).into())
}

/// The `payload_schema` the bundle's own sources declare.
///
/// A bundle carries no schema file: the number is the `PAYLOAD_SCHEMA`
/// constant of the release it is, declared in exactly one source file
/// under `src/` with the spelling `const PAYLOAD_SCHEMA: u32 = <n>;`. This
/// reads it back, so the same rule reaches every release that ever
/// declared the constant, this one included; a test in this crate holds
/// the spelling.
///
/// # Errors
///
/// Returns [`RkError::Other`] when no source declares the constant or
/// when two do.
pub fn declared_schema(root: &Path) -> Result<u32, RkError> {
    let src = root.join("src");
    let mut files = Vec::new();
    if src.is_dir() {
        walk(&src, &mut files)?;
    }
    files.sort();
    let mut found = Vec::new();
    for file in files {
        if file.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file)?;
        for line in text.lines() {
            let bare = line.trim_start();
            let bare = bare.strip_prefix("pub ").unwrap_or(bare);
            let Some(rest) = bare.strip_prefix(SCHEMA_DECLARATION) else {
                continue;
            };
            let digits: String = rest
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(value) = digits.parse::<u32>() {
                found.push(value);
            }
        }
    }
    match found.as_slice() {
        [one] => Ok(*one),
        [] => {
            Err(anyhow::anyhow!("{}: no source declares the payload schema", root.display()).into())
        }
        many => Err(anyhow::anyhow!(
            "{}: {} sources declare the payload schema",
            root.display(),
            many.len()
        )
        .into()),
    }
}

/// The declaration's spelling, up to the number, with or without `pub`:
/// the releases before the seam declared it private in the payload verb.
const SCHEMA_DECLARATION: &str = "const PAYLOAD_SCHEMA: u32 =";

#[cfg(test)]
mod tests {
    use super::{DirReleaseSource, SCHEMA_DECLARATION, declared_schema, declared_version};
    use crate::digest::Digest;
    use crate::release::ReleaseSource;

    /// A bundle directory with the minimum a source needs: a manifest, one
    /// source declaring the schema, and a few payload files.
    fn bundle(version: &str, schema: u32) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a scratch bundle exists");
        let root = dir.path();
        std::fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname = \"release-kit\"\nversion = \"{version}\"\n"),
        )
        .expect("the manifest writes");
        std::fs::create_dir_all(root.join("src")).expect("src exists");
        std::fs::write(
            root.join("src/lib.rs"),
            format!("pub {SCHEMA_DECLARATION} {schema};\n"),
        )
        .expect("the source writes");
        std::fs::write(root.join("versions.toml"), "schema = 1\n").expect("the registry writes");
        for (path, body) in [
            ("snippets/_shared/github/SECURITY.md", "# Security\n"),
            ("snippets/rust/github/release-plz.toml", "[workspace]\n"),
            (
                "blocks/agents-block.md.in",
                "<!-- BEGIN release-kit -->\n<!-- END release-kit -->\n",
            ),
        ] {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().expect("a parent")).expect("dirs exist");
            std::fs::write(file, body).expect("the file writes");
        }
        dir
    }

    #[test]
    fn the_fixture_source_serves_a_directory() {
        let dir = bundle("1.2.3", 1);
        let source = DirReleaseSource::new(dir.path());
        let manifest = source.manifest().expect("the directory describes itself");
        assert_eq!(manifest.release_kit_version, "1.2.3");
        assert_eq!(manifest.payload_schema, 1);
        let paths: Vec<&str> = manifest.artifacts.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "snippets/_shared/github/SECURITY.md",
                "snippets/rust/github/release-plz.toml",
                "blocks/agents-block.md.in",
                "versions.toml",
            ],
            "roots in inventory order, files sorted within each"
        );
        let bytes = source
            .blob(&Digest::of(b"[workspace]\n"))
            .expect("a named blob reads");
        assert_eq!(bytes, b"[workspace]\n");
        assert!(source.blob(&Digest::of(b"absent")).is_err());
    }

    #[test]
    fn the_declarations_are_read_back_from_the_bundle() {
        let dir = bundle("0.9.0", 7);
        assert_eq!(declared_version(dir.path()).expect("a version"), "0.9.0");
        assert_eq!(declared_schema(dir.path()).expect("a schema"), 7);
        std::fs::write(
            dir.path().join("src/other.rs"),
            format!("{SCHEMA_DECLARATION} 8;\n"),
        )
        .expect("the second source writes");
        assert!(
            declared_schema(dir.path()).is_err(),
            "two declarations refuse"
        );
    }

    /// This crate declares the constant the way the reader expects, so
    /// the bundle this release publishes reads back its own schema.
    #[test]
    fn this_crate_declares_its_schema_readably() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            declared_schema(root).expect("this crate declares its schema once"),
            crate::release::PAYLOAD_SCHEMA
        );
        assert_eq!(
            declared_version(root).expect("this crate declares its version"),
            env!("CARGO_PKG_VERSION")
        );
    }
}
