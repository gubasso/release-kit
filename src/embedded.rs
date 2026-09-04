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

/// The whole texts the binary writes outside `snippets/`.
///
/// The spliced blocks and the host-side hook body, authored as files so
/// no human-faced artifact lives as a source literal; the readers in
/// `src/landing.rs` and `src/setup/branch_reminder.rs` embed each file
/// by name.
pub static BLOCKS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/blocks");

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
        "blocks" => &BLOCKS,
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

    /// Every authored block ends in exactly one newline — the one the
    /// repository's hooks enforce and the readers strip — so the bytes a
    /// reader composes are identical to what the authored file holds
    /// above that newline, and no landed target reads as drift.
    #[test]
    fn every_block_is_authored_with_one_final_newline() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("blocks");
        for file in super::BLOCKS.files() {
            let name = file.path().to_string_lossy().into_owned();
            let disk = std::fs::read(root.join(&name)).expect("an embedded block exists on disk");
            assert_eq!(disk, file.contents(), "{name}: embed and disk disagree");
            let text = std::str::from_utf8(file.contents()).expect("a block is UTF-8");
            assert!(text.ends_with('\n'), "{name}: a block ends in a newline");
            assert!(
                !text.ends_with("\n\n"),
                "{name}: a block ends in exactly one newline"
            );
        }
    }

    /// No whole human-faced artifact lives as a Rust literal: every text
    /// the binary writes into a target or host is authored under
    /// `blocks/`, per `distribution:a-human-faced-artifact-is-authored-text`.
    /// Two nets, both over production code only — everything above a
    /// file's first `#[cfg(test)]`: a structural one that fails any
    /// string literal spanning three or more source lines, whatever its
    /// name, because a whole artifact body is multi-line and a message is
    /// not; and a needle list holding the retired const names out and
    /// pinning the one-line artifact signatures the structural net cannot
    /// tell from a message.
    #[test]
    fn no_artifact_body_lives_as_a_source_literal() {
        let needles = [
            "## Releases",
            "Installed by rk setup step branch-reminder",
            "This project works in worktrees:",
            "Branches are worked in the main checkout",
            "stages: [commit-msg]",
            "ROUTING_BLOCK",
            "ROUTING_WORKTREE_LINE",
            "ROUTING_BRANCHES_LINE",
            "HOOKS_BLOCK",
            "WORKTREE_GUARD_ENTRY",
            "HOOK_BODY",
            "use flake",
            "rk devshell sync --apply",
            "release-kit.packages.",
            "inputs.nixpkgs.follows",
        ];
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        scan(&src, &needles, &mut offenders);
        assert!(
            offenders.is_empty(),
            "an artifact body belongs under blocks/, not in the sources: {offenders:?}"
        );
    }

    fn scan(dir: &std::path::Path, needles: &[&str], offenders: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("the source tree is readable") {
            let entry = entry.expect("a directory entry is readable");
            let path = entry.path();
            if path.is_dir() {
                scan(&path, needles, offenders);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file is UTF-8");
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            for (index, line) in production.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for needle in needles {
                    if line.contains(needle) {
                        offenders.push(format!("{}:{}: {needle}", path.display(), index + 1));
                    }
                }
            }
            for (line, span) in multiline_literals(production) {
                offenders.push(format!(
                    "{}:{line}: a string literal spanning {span} lines",
                    path.display()
                ));
            }
        }
    }

    /// The interpolation glues the decoded-break net exempts, by their
    /// exact source text: the two splice compositions in
    /// `src/landing.rs` and the header block in `src/setup/app_jwt.rs`.
    /// Growing this list is a reviewed act; a whole artifact body never
    /// belongs on it.
    const GLUE: [&str; 3] = [
        "{}\\n\\n{block}\\n",
        "{HOOK_TYPES_LINE}\\n\\nrepos:\\n{block}\\n",
        concat!(
            "Authorization: Bearer {jwt}\\nAccept: application/vnd.github+json\\n",
            "X-GitHub-Api-Version: 2022-11-28\\n"
        ),
    ];

    /// Every string literal in `text` whose decoded value spans three or
    /// more lines, as `(starting line, decoded line count)`. A hand
    /// scanner over the token stream: line comments are skipped, raw
    /// literals end at their matching quote-and-hashes delimiter however
    /// many hashes open them, and quoted literals honor backslash
    /// escapes, so an artifact written on one source line as `\n`
    /// escapes counts by what it decodes to, not by how it is typed.
    fn multiline_literals(text: &str) -> Vec<(usize, usize)> {
        let bytes = text.as_bytes();
        let mut spans = Vec::new();
        let mut line = 1;
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    line += 1;
                    i += 1;
                }
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'r' if matches!(bytes.get(i + 1), Some(&b'#' | &b'"')) => {
                    let hashes = bytes[i + 1..]
                        .iter()
                        .take_while(|byte| **byte == b'#')
                        .count();
                    if bytes.get(i + 1 + hashes) != Some(&b'"') {
                        i += 1;
                        continue;
                    }
                    let body = i + hashes + 2;
                    let close = format!("\"{}", "#".repeat(hashes));
                    let end = text[body..]
                        .find(&close)
                        .map_or(bytes.len(), |at| body + at);
                    let physical = text[i..end].matches('\n').count();
                    if physical >= 2 {
                        spans.push((line, physical + 1));
                    }
                    line += physical;
                    i = (end + close.len()).min(bytes.len());
                }
                b'"' => {
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j] != b'"' {
                        j += if bytes[j] == b'\\' { 2 } else { 1 };
                    }
                    let segment = &text[i + 1..j.min(bytes.len())];
                    let physical = segment.matches('\n').count();
                    let decoded = physical + segment.matches("\\n").count();
                    // A literal spanning source lines is judged whole. A
                    // one-source-line literal is judged by its decoded
                    // breaks, with the few known interpolation glues
                    // allowlisted by their exact source text: a whole
                    // artifact is static authored text, and anything new
                    // that decodes to three lines answers here.
                    if physical >= 2 || (decoded >= 2 && !GLUE.contains(&segment)) {
                        spans.push((line, decoded + 1));
                    }
                    line += physical;
                    i = j + 1;
                }
                _ => i += 1,
            }
        }
        spans
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
