//! What a predecessor bump mechanism left in a target.
//!
//! The wiring `rk devshell` lands is a replacement, never an addition: two
//! mechanisms over the same two files fight or silently undo each other.
//! This catalog is the hand-rolled recipe this repository itself
//! published, plus the host install it supersedes. A file entry matches
//! on its content and never on its name alone, so an unrelated file of
//! the same name is never removed; a line entry names the file, the line,
//! and the matched text. The needles are grammars, in the same class as
//! the pin prefix: source constants, never payload text.

use camino::Utf8Path;
use serde::Serialize;

use super::has_sync_line;
use super::pin::PIN_PREFIX;
use crate::error::RkError;

/// What the cleanup does with one leftover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// Delete the file: it exists only for the predecessor mechanism.
    RemoveFile,
    /// Rewrite `.envrc`: the invocation lines go, the sync line takes
    /// the first one's place.
    ReplaceLine,
    /// Remove nothing; the report names the file, the line, and the reason.
    Manual,
}

/// One artifact of a predecessor mechanism.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Leftover {
    /// The catalog entry: `bump-script`, `autobump-script`, `bump-suite`,
    /// `autobump-suite`, `envrc-invocation`, `envrc-switch`,
    /// `just-recipe`, `devshell-tooling`, or `host-install`.
    pub id: &'static str,
    /// The file, relative to the target.
    pub file: String,
    /// The one-based line, for a line entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The matched line, trimmed, for a line entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// What the cleanup does with it.
    pub action: Action,
    /// Why that action, one phrase.
    pub reason: &'static str,
}

/// The lines a predecessor's `.envrc` invocation names.
const ENVRC_NEEDLES: [&str; 2] = ["rk-bump", "rk-autobump"];

/// The host install lines the devshell pin supersedes.
const HOST_INSTALL_NEEDLES: [&str; 2] = ["cargo install release-kit", "cargo binstall release-kit"];

/// The switch the predecessor read from `.envrc.local`.
const SWITCH_NEEDLE: &str = "RK_SKIP_AUTOBUMP";

/// The files a host install line is looked for in, beside the workflows.
const HOST_INSTALL_FILES: [&str; 4] = ["README.md", "justfile", ".envrc", ".gitlab-ci.yml"];

/// Scan a target for every leftover the catalog knows, in catalog order.
///
/// # Errors
///
/// Returns [`RkError::Io`] where the workflows directory exists and does
/// not list; an unreadable file is no match.
pub fn scan(target: &Utf8Path) -> Result<Vec<Leftover>, RkError> {
    let mut found = Vec::new();
    let read = |rel: &str| std::fs::read_to_string(target.join(rel)).ok();
    for (id, rel, needle) in [
        ("bump-script", "scripts/rk-bump.sh", PIN_PREFIX),
        ("autobump-script", "scripts/rk-autobump.sh", "rk-bump.sh"),
        ("bump-suite", "tests/rk-bump.bats", "rk-bump"),
        ("autobump-suite", "tests/rk-autobump.bats", "rk-autobump"),
    ] {
        if read(rel).is_some_and(|text| text.contains(needle)) {
            found.push(Leftover {
                id,
                file: rel.to_owned(),
                line: None,
                text: None,
                action: Action::RemoveFile,
                reason: "the file exists only for the predecessor bump mechanism",
            });
        }
    }
    if let Some(text) = read(".envrc") {
        for (number, line) in lines_holding(&text, &ENVRC_NEEDLES) {
            found.push(Leftover {
                id: "envrc-invocation",
                file: ".envrc".to_owned(),
                line: Some(number),
                text: Some(line),
                action: Action::ReplaceLine,
                reason: "the invocation gives way to the sync line",
            });
        }
    }
    for rel in [".envrc.local", ".envrc.local.example"] {
        if let Some(text) = read(rel) {
            for (number, line) in lines_holding(&text, &[SWITCH_NEEDLE]) {
                found.push(Leftover {
                    id: "envrc-switch",
                    file: rel.to_owned(),
                    line: Some(number),
                    text: Some(line),
                    action: Action::Manual,
                    reason: "the switch is now RK_DEVSHELL_SYNC=0, in a file the operator owns",
                });
            }
        }
    }
    if let Some(text) = read("justfile") {
        for (index, line) in text.lines().enumerate() {
            if is_recipe_head(line, "rk-bump") {
                found.push(Leftover {
                    id: "just-recipe",
                    file: "justfile".to_owned(),
                    line: Some(index + 1),
                    text: Some(line.trim().to_owned()),
                    action: Action::Manual,
                    reason: "a recipe body carries structure a line scan cannot judge",
                });
            }
        }
    }
    if let Some(text) = read("flake.nix") {
        for (number, line) in list_members_named(&text, &["flock", "bats"]) {
            found.push(Leftover {
                id: "devshell-tooling",
                file: "flake.nix".to_owned(),
                line: Some(number),
                text: Some(line),
                action: Action::Manual,
                reason: "a Nix package list carries structure a line scan cannot judge",
            });
        }
    }
    let mut host_files: Vec<String> = HOST_INSTALL_FILES.iter().map(|s| (*s).to_owned()).collect();
    host_files.extend(workflow_files(target)?);
    for rel in host_files {
        if let Some(text) = read(&rel) {
            for (number, line) in lines_holding(&text, &HOST_INSTALL_NEEDLES) {
                found.push(Leftover {
                    id: "host-install",
                    file: rel.clone(),
                    line: Some(number),
                    text: Some(line),
                    action: Action::Manual,
                    reason: "an install line sits in prose or a CI step a line scan cannot judge",
                });
            }
        }
    }
    Ok(found)
}

/// Rewrite an `.envrc` so the sync line replaces the invocation.
///
/// Every line naming the predecessor invocation goes, and the sync line
/// takes the first removed line's place unless the file already carries
/// one. Every other line is byte-identical. `None` where no line names
/// the invocation.
#[must_use]
pub fn swap_envrc(text: &str, sync_line: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len() + sync_line.len() + 2);
    let mut removed = 0;
    let needs_line = !has_sync_line(text);
    for line in text.split_inclusive('\n') {
        if ENVRC_NEEDLES.iter().any(|needle| line.contains(needle)) {
            if removed == 0 && needs_line {
                out.push_str(sync_line);
                out.push_str(if line.ends_with("\r\n") { "\r\n" } else { "\n" });
            }
            removed += 1;
        } else {
            out.push_str(line);
        }
    }
    (removed > 0).then_some(out)
}

/// Every `(line number, trimmed line)` holding one of the needles.
fn lines_holding(text: &str, needles: &[&str]) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| needles.iter().any(|needle| line.contains(needle)))
        .map(|(index, line)| (index + 1, line.trim().to_owned()))
        .collect()
}

/// Whether a justfile line opens a recipe named `name`: the name at the
/// start, then its parameters or nothing, then the colon.
fn is_recipe_head(line: &str, name: &str) -> bool {
    let Some(rest) = line.strip_prefix(name) else {
        return false;
    };
    let Some(head) = rest.split(':').next() else {
        return false;
    };
    rest.contains(':') && (head.is_empty() || head.starts_with(' ') || head.starts_with('\t'))
}

/// Every `(line number, trimmed line)` of a Nix text that names one of
/// the packages as a list member: a whitespace-separated token — brackets
/// stripped — that is an attribute path ending in the package name, at a
/// bracket depth above zero. The depth runs across the whole text, so a
/// multi-line assignment such as `formatter =` over `pkgs.bats;` is not a
/// list member, and a continuation line inside `[ ... ]` is. Comments
/// are skipped.
fn list_members_named(text: &str, packages: &[&str]) -> Vec<(usize, String)> {
    let mut depth = 0usize;
    let mut found = Vec::new();
    let scrubbed = scrub_nix(text);
    for (index, (line, code)) in text.lines().zip(scrubbed.lines()).enumerate() {
        let mut named = false;
        for token in code.split_whitespace() {
            let opened = token.matches('[').count();
            let closed = token.matches(']').count();
            let stripped = token.trim_matches(|c| matches!(c, '[' | ']' | '(' | ')' | ';'));
            if depth + opened > closed
                && packages
                    .iter()
                    .any(|package| is_package_path(stripped, package))
            {
                named = true;
            }
            depth = (depth + opened).saturating_sub(closed);
        }
        if named {
            found.push((index + 1, line.trim().to_owned()));
        }
    }
    found
}

/// The double quote as a code point: the source scan that keeps whole
/// artifacts out of the sources reads a quote literal as a string start.
const QUOTE: char = '\u{22}';

/// The Nix text with every string and comment blanked to spaces, line
/// breaks kept, so a bracket inside `"..."`, `\'\'...\'\'`, a `#` line
/// comment, or a `/* */` block comment never counts as syntax.
fn scrub_nix(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let blank = |out: &mut String, slice: &str| {
        for c in slice.chars() {
            out.push(if c == '\n' { '\n' } else { ' ' });
        }
    };
    while i < bytes.len() {
        let rest = &text[i..];
        let skip = if rest.starts_with('#') {
            rest.find('\n').unwrap_or(rest.len())
        } else if rest.starts_with("/*") {
            rest.find("*/").map_or(rest.len(), |at| at + 2)
        } else if let Some(body) = rest.strip_prefix("\'\'") {
            body.find("\'\'").map_or(rest.len(), |at| at + 4)
        } else if rest.starts_with(QUOTE) {
            let mut j = 1;
            while j < rest.len() && !rest[j..].starts_with(QUOTE) {
                j += if rest[j..].starts_with('\\') { 2 } else { 1 };
            }
            (j + 1).min(rest.len())
        } else {
            0
        };
        if skip == 0 {
            let c = rest.chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        } else {
            blank(&mut out, &rest[..skip]);
            i += skip;
        }
    }
    out
}

/// Whether a token is an attribute path whose last segment is `package`.
fn is_package_path(token: &str, package: &str) -> bool {
    token.strip_suffix(package).is_some_and(|head| {
        (head.is_empty() || head.ends_with('.'))
            && head
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    })
}

/// The workflow files under `.github/workflows`, relative to the target.
fn workflow_files(target: &Utf8Path) -> Result<Vec<String>, RkError> {
    let dir = target.join(".github/workflows");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<String> = dir
        .read_dir_utf8()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .map(|entry| format!(".github/workflows/{}", entry.file_name()))
        .collect();
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use camino::Utf8PathBuf;

    use super::{Action, is_recipe_head, list_members_named, scan, swap_envrc};
    use crate::devshell::pin::PIN_PREFIX;

    #[test]
    fn a_catalog_file_matches_on_its_content_and_not_on_its_name_alone() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let target = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8");
        std::fs::create_dir_all(target.join("scripts")).expect("scripts creates");
        std::fs::create_dir_all(target.join("tests")).expect("tests creates");
        std::fs::write(
            target.join("scripts/rk-bump.sh"),
            "#!/bin/sh\necho unrelated\n",
        )
        .expect("writes");
        std::fs::write(
            target.join("tests/rk-bump.bats"),
            "@test unrelated { true; }\n",
        )
        .expect("writes");
        assert!(
            scan(&target).expect("scans").is_empty(),
            "the name alone never decides"
        );
        std::fs::write(
            target.join("scripts/rk-bump.sh"),
            format!("#!/bin/sh\nPIN_PREFIX=\"{PIN_PREFIX}\"\n"),
        )
        .expect("writes");
        std::fs::write(target.join("tests/rk-bump.bats"), "load rk-bump\n").expect("writes");
        let found = scan(&target).expect("scans");
        let ids: Vec<&str> = found.iter().map(|l| l.id).collect();
        assert_eq!(ids, ["bump-script", "bump-suite"]);
        assert!(found.iter().all(|l| l.action == Action::RemoveFile));
    }

    #[test]
    fn the_envrc_swap_keeps_every_other_line() {
        let text = "use flake\r\n# keep\r\n# The bump runs rk-autobump on entry\r\nscripts/rk-autobump.sh || true\r\nexport FOO=1\r\n";
        let swapped = swap_envrc(text, "rk devshell sync --apply || true").expect("a swap");
        assert_eq!(
            swapped,
            "use flake\r\n# keep\r\nrk devshell sync --apply || true\r\nexport FOO=1\r\n"
        );
        let already = "rk devshell sync --apply || true\nscripts/rk-bump.sh\n";
        assert_eq!(
            swap_envrc(already, "rk devshell sync --apply || true").expect("a swap"),
            "rk devshell sync --apply || true\n",
            "an existing sync line is not doubled"
        );
        assert_eq!(swap_envrc("use flake\n", "x"), None);
    }

    #[test]
    fn the_line_matchers_are_bounded() {
        assert!(is_recipe_head("rk-bump:", "rk-bump"));
        assert!(is_recipe_head("rk-bump tag='':", "rk-bump"));
        assert!(!is_recipe_head("rk-bump-all:", "rk-bump"));
        assert!(!is_recipe_head("    rk-bump", "rk-bump"));
        let members = |text: &str| list_members_named(text, &["flock", "bats"]);
        assert_eq!(
            members("packages = [\n  pkgs.flock\n  bats # the suites\n];\n"),
            [
                (2, "pkgs.flock".to_owned()),
                (3, "bats # the suites".to_owned())
            ]
        );
        assert_eq!(members("packages = [ flock bats ];\n").len(), 1);
        assert_eq!(
            members("packages = [\n  \"]\"\n  pkgs.flock # ] in a comment\n];\n"),
            [(3, "pkgs.flock # ] in a comment".to_owned())],
            "a bracket in a string or a comment is not syntax"
        );
        assert_eq!(
            members("packages = [\n  nixpkgs.legacyPackages.x86_64-linux.bats\n];\n").len(),
            1
        );
        for not_a_member in [
            "combats\n",
            "[ flock-of-seagulls ]\n",
            "# flock is gone\n",
            "[ checks.flockTest ]\n",
            "description = \"needs flock\";\n",
            "[ pkgs.bats-core ]\n",
            "formatter = pkgs.bats;\n",
            "someTool = pkgs.flock;\n",
            "formatter =\n  pkgs.bats;\n",
            "someTool =\n  pkgs.flock;\n",
            "packages = with pkgs; [\n];\nformatter = pkgs.bats;\n",
            "description = \"[\";\nformatter = pkgs.bats;\n",
            "/* [ */\nformatter = pkgs.flock;\n",
            "x = \'\'[\'\';\nformatter = pkgs.bats;\n",
        ] {
            assert!(members(not_a_member).is_empty(), "{not_a_member:?}");
        }
    }
}
