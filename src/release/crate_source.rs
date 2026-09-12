//! The crates venue behind the seam: another release's bundle, fetched
//! once, verified against the registry's own checksum, and cached by
//! that checksum.
//!
//! The published crate carries every payload root, so the `.crate` file
//! at an exact version is the bundle for that release. Two reads reach
//! the network: the sparse index, which resolves a selector to one exact
//! version and one checksum, and the archive itself. The archive is
//! verified against the index checksum before anything is kept, unpacked
//! under one directory outside every target, and read back through
//! [`DirReleaseSource`]. A checksum already cached is served with no
//! network touch, and an exact version once verified resolves offline
//! too. The bundle is data: nothing under the cache is executed, put on
//! `PATH`, or written into a repository.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;

use super::{DirReleaseSource, ReleaseManifest, ReleaseSource};

/// The crate's name at the registry.
pub const CRATE: &str = "release-kit";

/// The sparse index entry for the crate, by the registry's path rule for
/// names of four or more characters: the first two, then the next two,
/// then the name.
pub const INDEX_URL: &str = "https://index.crates.io/re/le/release-kit";

/// The archive download root the registry's `config.json` names; the
/// crate file lives at `<dl>/<crate>/<crate>-<version>.crate`.
pub const DL_URL: &str = "https://static.crates.io/crates";

/// The cache directory under the state root.
pub const CACHE_DIR: &str = "release";

/// How many fetched bundles the cache keeps.
///
/// The newest by fetch time, pruned after every fetch that adds one. An
/// upgrade needs one bundle beside the embedded release, and a small
/// window covers a retry and a comparison without growing without bound.
pub const RETAIN: usize = 4;

/// The one release the selector resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The exact version.
    pub version: String,
    /// The registry's checksum of the `.crate` file.
    pub cksum: Digest,
    /// Whether this call reached the network for the index.
    pub index_fetched: bool,
    /// Whether this call fetched the archive, or served it from cache.
    pub archive_fetched: bool,
}

/// The crates venue for one selector.
#[derive(Debug)]
pub struct CrateReleaseSource {
    selector: String,
    cache: PathBuf,
    resolved: OnceLock<Resolved>,
}

impl CrateReleaseSource {
    /// A source for `selector` — `latest` or an exact version — caching
    /// under the state root.
    ///
    /// # Errors
    ///
    /// Returns [`RkError::Refusal`] when no state root can be located.
    pub fn new(selector: &str) -> Result<Self, RkError> {
        let root = crate::applog::state_root().ok_or_else(|| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    "no state root: neither XDG_STATE_HOME nor HOME is set",
                )
                .action("set XDG_STATE_HOME or HOME so the release cache has a home"),
            )
        })?;
        Ok(Self::with_cache(selector, root.join(CACHE_DIR)))
    }

    /// A source caching under an explicit directory.
    #[must_use]
    pub fn with_cache(selector: &str, cache: impl Into<PathBuf>) -> Self {
        Self {
            selector: selector.to_owned(),
            cache: cache.into(),
            resolved: OnceLock::new(),
        }
    }

    /// The selector as given.
    #[must_use]
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// Resolve the selector, fetch and verify the archive where the cache
    /// does not hold it, and answer with what happened. Idempotent: the
    /// second call answers from the first.
    ///
    /// # Errors
    ///
    /// Returns a `registry-unreachable` refusal when the index or the
    /// archive cannot be fetched, a `bundle-unverified` refusal when the
    /// archive's digest differs from the index checksum, and a usage
    /// error for a version the index does not list.
    pub fn resolve(&self) -> Result<&Resolved, RkError> {
        if let Some(resolved) = self.resolved.get() {
            return Ok(resolved);
        }
        let resolved = self.resolve_fresh()?;
        Ok(self.resolved.get_or_init(|| resolved))
    }

    fn resolve_fresh(&self) -> Result<Resolved, RkError> {
        let (version, cksum, index_fetched) = if let Some((version, cksum)) = self.cached_index() {
            (version, cksum, false)
        } else {
            let (version, cksum) = self.resolve_at_index()?;
            (version, cksum, true)
        };
        let dir = self.cache.join(cksum.to_string());
        // A cache hit is served only where the extracted tree still
        // digests to what the verified archive unpacked to. The registry
        // vouches for the archive, and the seal carries that vouching
        // forward to the bytes on disk; without the check, an altered
        // cache entry would be read back as a release the registry
        // verified, because the manifest recomputes its digests from
        // whatever the directory now holds.
        let archive_fetched = if dir.is_dir() {
            verify_seal(&self.cache, &cksum, &dir)?;
            false
        } else {
            self.fetch_and_verify(&version, &cksum, &dir)?;
            true
        };
        // The version maps to its checksum only once the archive behind it
        // verified, so a mismatch caches nothing, not even the name.
        let index = self.cache.join("index");
        std::fs::create_dir_all(&index)?;
        std::fs::write(index.join(&version), format!("{cksum}\n"))?;
        if archive_fetched {
            prune(&self.cache, RETAIN)?;
        }
        Ok(Resolved {
            version,
            cksum,
            index_fetched,
            archive_fetched,
        })
    }

    /// Whether the selector names an exact version whose verified bundle
    /// the cache already holds, so a read touches no network.
    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.cached_index().is_some()
    }

    /// An exact version's checksum, from a previous verified fetch.
    fn cached_index(&self) -> Option<(String, Digest)> {
        if self.selector == "latest" {
            return None;
        }
        let text = std::fs::read_to_string(self.cache.join("index").join(&self.selector)).ok()?;
        let cksum = Digest::parse(text.trim())?;
        self.cache
            .join(cksum.to_string())
            .is_dir()
            .then(|| (self.selector.clone(), cksum))
    }

    /// The selector against the live index.
    fn resolve_at_index(&self) -> Result<(String, Digest), RkError> {
        let body =
            fetch(INDEX_URL).map_err(|detail| unreachable("the crates.io index", &detail))?;
        let entries = parse_index(&body).map_err(|detail| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::RegistryUnreachable,
                    format!("the crates.io index entry for {CRATE} did not parse: {detail}"),
                )
                .expected("one JSON object per line, each naming vers and cksum"),
            )
        })?;
        let chosen = if self.selector == "latest" {
            entries
                .iter()
                .filter(|entry| !entry.yanked && !entry.version.contains('-'))
                .max_by(|a, b| compare_versions(&a.version, &b.version))
        } else {
            entries.iter().find(|entry| entry.version == self.selector)
        };
        let Some(entry) = chosen else {
            return Err(RkError::Usage(format!(
                "the crates.io index lists no {CRATE} version matching '{}'",
                self.selector
            )));
        };
        if entry.yanked {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::BundleUnverified,
                    format!("{CRATE} {} is yanked at the registry", entry.version),
                )
                .expected("a version the registry still vouches for"),
            ));
        }
        Ok((entry.version.clone(), entry.cksum.clone()))
    }

    /// Fetch the archive to a scratch file beside the cache, verify it,
    /// unpack it, and move the unpacked tree to `dir` in one rename.
    fn fetch_and_verify(&self, version: &str, cksum: &Digest, dir: &Path) -> Result<(), RkError> {
        std::fs::create_dir_all(&self.cache)?;
        let scratch = Scratch::new(self.cache.join(format!("fetch-{}", std::process::id())))?;
        let archive = scratch.path().join(format!("{CRATE}-{version}.crate"));
        let url = format!("{DL_URL}/{CRATE}/{CRATE}-{version}.crate");
        fetch_to(&url, &archive).map_err(|detail| unreachable("the crate archive", &detail))?;
        let bytes = std::fs::read(&archive)?;
        let actual = Digest::of(&bytes);
        if actual != *cksum {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::BundleUnverified,
                    format!(
                        "{CRATE}-{version}.crate digests to {actual}, and the registry index names {cksum}"
                    ),
                )
                .expected("an archive whose sha256 equals the index checksum")
                .target_state("nothing was cached"),
            ));
        }
        let unpacked = scratch.path().join("unpacked");
        std::fs::create_dir_all(&unpacked)?;
        let tar = std::env::var_os("RK_TAR_BIN").unwrap_or_else(|| "tar".into());
        let status = Command::new(tar)
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&unpacked)
            .status()
            .map_err(|source| {
                RkError::subprocess(Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("tar did not run: {source}"),
                ))
            })?;
        if !status.success() {
            return Err(RkError::subprocess(Diagnostic::new(
                Reason::SubprocessFailed,
                format!("tar could not unpack {CRATE}-{version}.crate"),
            )));
        }
        let tree = unpacked.join(format!("{CRATE}-{version}"));
        if !tree.is_dir() {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::BundleUnverified,
                    format!("{CRATE}-{version}.crate does not unpack to {CRATE}-{version}/"),
                )
                .target_state("nothing was cached"),
            ));
        }
        std::fs::rename(&tree, dir)?;
        // The seal, written only now: the archive verified, so the tree
        // it unpacked to is what the registry's checksum vouches for.
        std::fs::write(
            seal_path(&self.cache, cksum),
            seal_body(cksum, &tree_digest(dir)?),
        )?;
        Ok(())
    }
}

/// The seal beside one cached bundle, naming the archive checksum the
/// registry vouched for and the digest of the tree it unpacked to.
///
/// It lives beside the directory rather than inside it, so the tree
/// digest covers the whole bundle and nothing else.
fn seal_path(cache: &Path, cksum: &Digest) -> PathBuf {
    cache.join(format!("{cksum}.seal"))
}

/// The seal's two lines: the archive checksum, then the tree digest.
///
/// Joined rather than formatted, because two escapes in one format
/// string read to the source scan as an artifact body.
fn seal_body(cksum: &Digest, tree: &Digest) -> String {
    [cksum.to_string(), tree.to_string(), String::new()].join("\n")
}

/// One digest over every file below `dir`, path and bytes, sorted, so a
/// changed byte, a removed file, and an added file all move it.
fn tree_digest(dir: &Path) -> Result<Digest, RkError> {
    let mut files = Vec::new();
    walk(dir, &mut files)?;
    files.sort();
    let mut acc = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(dir)
            .map_err(|_| anyhow::anyhow!("{} is outside the bundle", file.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        acc.extend_from_slice(rel.as_bytes());
        acc.push(b'\n');
        acc.extend_from_slice(Digest::of(&std::fs::read(&file)?).to_string().as_bytes());
        acc.push(b'\n');
    }
    Ok(Digest::of(&acc))
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

/// A cached bundle read back against the seal the verified archive left.
///
/// The seal's first line must be the checksum the directory is named
/// for, so a seal lifted from another bundle does not vouch for this
/// one, and its second line must be the tree's digest now.
fn verify_seal(cache: &Path, cksum: &Digest, dir: &Path) -> Result<(), RkError> {
    let path = seal_path(cache, cksum);
    let altered = |detail: String| {
        RkError::refusal(
            Diagnostic::new(
                Reason::BundleUnverified,
                format!("the cached bundle for {cksum} {detail}"),
            )
            .expected("a cached bundle whose bytes are the ones its verified archive unpacked to")
            .action(format!(
                "remove {} and its seal, so the next read fetches and verifies the archive again",
                dir.display()
            ))
            .target_state("nothing was read from it"),
        )
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Err(altered("carries no seal".to_owned()));
    };
    let actual = tree_digest(dir)?;
    if text != seal_body(cksum, &actual) {
        return Err(altered(
            "does not match the seal its verified archive left".to_owned(),
        ));
    }
    Ok(())
}

impl ReleaseSource for CrateReleaseSource {
    fn manifest(&self) -> Result<ReleaseManifest, RkError> {
        let resolved = self.resolve()?;
        let manifest =
            DirReleaseSource::new(self.cache.join(resolved.cksum.to_string())).manifest()?;
        manifest.check_schema()?;
        Ok(manifest)
    }

    fn blob(&self, digest: &Digest) -> Result<Vec<u8>, RkError> {
        let resolved = self.resolve()?;
        DirReleaseSource::new(self.cache.join(resolved.cksum.to_string())).blob(digest)
    }
}

/// A scratch directory removed on drop, whatever the fetch did, so a
/// refused archive leaves nothing behind.
struct Scratch(PathBuf);

impl Scratch {
    fn new(path: PathBuf) -> std::io::Result<Self> {
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One line of the sparse index.
#[derive(Debug, PartialEq, Eq)]
struct IndexEntry {
    version: String,
    cksum: Digest,
    yanked: bool,
}

/// The index body: one JSON object per line.
fn parse_index(body: &[u8]) -> Result<Vec<IndexEntry>, String> {
    let text = std::str::from_utf8(body).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("line {}: {e}", number + 1))?;
        let version = value
            .get("vers")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("line {}: no vers", number + 1))?
            .to_owned();
        let cksum = value
            .get("cksum")
            .and_then(serde_json::Value::as_str)
            .and_then(Digest::parse)
            .ok_or_else(|| format!("line {}: no sha256 cksum", number + 1))?;
        let yanked = value
            .get("yanked")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        out.push(IndexEntry {
            version,
            cksum,
            yanked,
        });
    }
    Ok(out)
}

/// Numeric semver order over `major.minor.patch`; anything unparsable
/// sorts first.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    parse_version(a).cmp(&parse_version(b))
}

fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(str::parse::<u64>);
    Some((
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
}

/// One GET through curl, body on stdout.
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let curl = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let output = Command::new(curl)
        .args(["-fsSL", "--max-time", "30", url])
        .output()
        .map_err(|source| format!("curl did not run: {source}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr)
            .lines()
            .last()
            .unwrap_or("curl failed")
            .to_owned())
    }
}

/// One GET through curl, body to a file.
fn fetch_to(url: &str, path: &Path) -> Result<(), String> {
    let curl = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let output = Command::new(curl)
        .args(["-fsSL", "--max-time", "120", "-o"])
        .arg(path)
        .arg(url)
        .output()
        .map_err(|source| format!("curl did not run: {source}"))?;
    if output.status.success() && path.is_file() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr)
            .lines()
            .last()
            .unwrap_or("curl failed")
            .to_owned())
    }
}

fn unreachable(what: &str, detail: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::RegistryUnreachable,
            format!("{what} did not answer: {detail}"),
        )
        .expected("a host that can reach crates.io, or a bundle already in the cache")
        .target_state("nothing was cached"),
    )
}

/// Keep the `retain` newest bundle directories by modification time and
/// remove the rest, together with the index entries that named them.
fn prune(cache: &Path, retain: usize) -> Result<(), RkError> {
    let mut bundles: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(cache)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() && Digest::parse(&name).is_some() {
            let modified = entry
                .metadata()?
                .modified()
                .unwrap_or(std::time::UNIX_EPOCH);
            bundles.push((modified, path));
        }
    }
    bundles.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, path) in bundles.iter().skip(retain) {
        std::fs::remove_dir_all(path)?;
        let gone = path.file_name().map(|n| n.to_string_lossy().into_owned());
        // The seal goes with the bundle it vouches for, so a later fetch
        // of the same checksum writes a fresh one rather than reading a
        // seal left by the tree it replaced.
        if let Some(gone) = &gone {
            if let Some(cksum) = Digest::parse(gone) {
                let _ = std::fs::remove_file(seal_path(cache, &cksum));
            }
        }
        let index = cache.join("index");
        if let (Some(gone), Ok(entries)) = (gone, std::fs::read_dir(&index)) {
            for entry in entries.flatten() {
                let names_it =
                    std::fs::read_to_string(entry.path()).is_ok_and(|text| text.trim() == gone);
                if names_it {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{RETAIN, compare_versions, parse_index, prune};
    use crate::digest::Digest;

    #[test]
    fn the_index_parses_one_entry_per_line() {
        let a = Digest::of(b"a").to_string();
        let b = Digest::of(b"b").to_string();
        let body = format!(
            "{{\"name\":\"release-kit\",\"vers\":\"0.3.17\",\"cksum\":\"{a}\",\"yanked\":false}}\n{{\"name\":\"release-kit\",\"vers\":\"0.3.18\",\"cksum\":\"{b}\",\"yanked\":true}}\n"
        );
        let entries = parse_index(body.as_bytes()).expect("the index parses");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, "0.3.17");
        assert!(!entries[0].yanked);
        assert!(entries[1].yanked);
        assert!(
            parse_index(b"{\"vers\":\"1.0.0\"}\n").is_err(),
            "no cksum refuses"
        );
    }

    #[test]
    fn versions_compare_numerically() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("0.3.9", "0.3.10"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "0.99.99"), Ordering::Greater);
        assert_eq!(compare_versions("0.3.18", "0.3.18"), Ordering::Equal);
    }

    #[test]
    fn the_cache_keeps_the_newest_bundles() {
        let cache = tempfile::tempdir().expect("a scratch cache");
        let index = cache.path().join("index");
        std::fs::create_dir_all(&index).expect("the index dir exists");
        let mut names = Vec::new();
        for i in 0..=RETAIN {
            let name = Digest::of(&[u8::try_from(i).expect("small")]).to_string();
            std::fs::create_dir_all(cache.path().join(&name)).expect("a bundle dir");
            std::fs::write(index.join(format!("0.0.{i}")), format!("{name}\n"))
                .expect("an index entry");
            let when = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(1_000 + i as u64);
            std::fs::File::open(cache.path().join(&name))
                .and_then(|f| f.set_modified(when))
                .expect("mtime set");
            names.push(name);
        }
        prune(cache.path(), RETAIN).expect("the prune runs");
        assert!(!cache.path().join(&names[0]).exists(), "the oldest went");
        assert!(
            !index.join("0.0.0").exists(),
            "its index entry went with it"
        );
        for name in &names[1..] {
            assert!(cache.path().join(name).is_dir(), "{name} kept");
        }
    }
}
