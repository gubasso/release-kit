//! The compile-time payload: everything the binary serves or lands.
//!
//! `include_dir!` embeds each authored root at compile time, so the binary
//! and the canon it carries cannot drift. Which roots exist is declared
//! once, in [`crate::payload_roots`], read here, by `build.rs` for change
//! tracking, and by the packaging test; a test below holds this module to
//! that inventory.

use include_dir::{Dir, include_dir};

pub use crate::payload_roots::PAYLOAD_ROOTS;

/// The technology-agnostic method chapters.
pub static METHOD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/method");

/// The per-technology bindings.
pub static BINDINGS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/bindings");

/// The human-facing runbooks `rk guide` renders.
pub static RUNBOOKS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/runbooks");

/// The per-forge documents answering the fifth axis.
pub static FORGES: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/forges");

/// The setup scripts, one subtree per forge, executed by `rk setup` and
/// landed nowhere.
pub static SETUP: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/setup");

/// The deterministic files `rk init` lands, one subtree per technology,
/// laid out exactly as they land in a target repository.
pub static SNIPPETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/snippets");

/// The agent skills, one directory per skill.
pub static SKILLS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/skills");

/// The artifacts every skill shares, installed once outside the skill roots.
pub static SKILL_SHARED: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/skill-shared");

/// The pinned-tool registry.
pub static VERSIONS: &str = include_str!("../versions.toml");

/// The root license statement naming both halves.
pub static LICENSE: &str = include_str!("../LICENSE");

/// The MIT text covering the distribution.
pub static LICENSE_MIT: &str = include_str!("../LICENSE-MIT");

/// The CC BY 4.0 text covering the method.
pub static LICENSE_CC_BY: &str = include_str!("../LICENSE-CC-BY-4.0");

/// The sentinel marker a landed file may carry; `rk init --apply` reports
/// every line holding one so nothing lands half-configured silently.
pub const SENTINEL: &str = "TODO(release-kit)";

/// Collect every file under `dir`, depth-first, as `(path, contents)` with
/// the path relative to the embedded root, sorted by path.
pub(crate) fn walk<'a>(dir: &Dir<'a>) -> Vec<(String, &'a [u8])> {
    let mut out = Vec::new();
    for file in dir.files() {
        out.push((file.path().to_string_lossy().into_owned(), file.contents()));
    }
    for sub in dir.dirs() {
        out.extend(walk(sub));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The files one payload root carries, as `(path, bytes)` with the path
/// carrying the root as its first segment, or `None` for a name the
/// inventory does not declare.
#[must_use]
pub fn root_files(root: &str) -> Option<Vec<(String, &'static [u8])>> {
    let dir = match root {
        "method" => &METHOD,
        "bindings" => &BINDINGS,
        "runbooks" => &RUNBOOKS,
        "forges" => &FORGES,
        "snippets" => &SNIPPETS,
        "setup" => &SETUP,
        "skills" => &SKILLS,
        "skill-shared" => &SKILL_SHARED,
        "versions.toml" => return Some(vec![(root.to_owned(), VERSIONS.as_bytes())]),
        _ => return None,
    };
    Some(
        walk(dir)
            .into_iter()
            .map(|(path, bytes)| (format!("{root}/{path}"), bytes))
            .collect(),
    )
}

/// Every artifact the payload carries, root by root in inventory order,
/// sorted by path within each root.
///
/// The license files are deliberately absent: they are crate metadata the
/// registry requires, not authored payload, and `rk license` serves them.
#[must_use]
pub fn artifacts() -> Vec<(String, &'static [u8])> {
    PAYLOAD_ROOTS
        .iter()
        .filter_map(|root| root_files(root))
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{PAYLOAD_ROOTS, artifacts, root_files};

    /// The inventory and this module must name the same roots: a root
    /// embedded here but absent from the inventory would be served without
    /// change tracking, and a development build would then carry stale
    /// bytes; a root declared but not embedded is a name `rk payload`
    /// would report and nothing would serve.
    #[test]
    fn the_inventory_and_the_embed_declare_the_same_roots() {
        let source = include_str!("embedded.rs");
        let mut embedded: Vec<String> = source
            .lines()
            .filter_map(|line| {
                let (_, rest) = line.split_once("include_dir!(\"$CARGO_MANIFEST_DIR/")?;
                let (root, _) = rest.split_once('"')?;
                Some(root.to_owned())
            })
            .collect();
        embedded.extend(source.lines().filter_map(|line| {
            let (_, rest) = line.split_once("include_str!(\"../")?;
            let (name, _) = rest.split_once('"')?;
            (!name.starts_with("LICENSE")).then(|| name.to_owned())
        }));
        embedded.sort();
        let mut declared: Vec<String> = PAYLOAD_ROOTS.iter().map(ToString::to_string).collect();
        declared.sort();
        assert_eq!(
            embedded, declared,
            "src/embedded.rs and src/payload_roots.rs disagree on the payload roots"
        );
    }

    #[test]
    fn every_declared_root_serves_at_least_one_file() {
        for root in PAYLOAD_ROOTS {
            let files = root_files(root).expect("a declared root resolves");
            assert!(!files.is_empty(), "{root}: the root carries no file");
            for (path, _) in &files {
                assert!(
                    path == root || path.starts_with(&format!("{root}/")),
                    "{path}: an artifact path must carry its root"
                );
            }
        }
        assert!(root_files("no-such-root").is_none());
    }

    #[test]
    fn the_artifact_list_is_stable_and_complete() {
        let listed = artifacts();
        let total: usize = PAYLOAD_ROOTS
            .iter()
            .map(|root| root_files(root).expect("a declared root resolves").len())
            .sum();
        assert_eq!(listed.len(), total);
        assert!(
            listed.iter().any(|(path, _)| path == "versions.toml"),
            "the single-file root must appear as itself"
        );
    }
}
