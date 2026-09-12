//! The texts `rk self-depend add` serves: one fragment set per manager
//! and venue pair, the `.envrc` line, and the seed file for a manager the
//! target has no file for.
//!
//! Every text is an authored file under `blocks/`, per
//! `distribution:a-human-faced-artifact-is-authored-text`; each carries
//! `RK_DEVSHELL_*` tokens rendered with a plain replace, never a format,
//! so the `${system}` interpolation survives untouched. The fragments
//! describe where they go in a form a coding agent can apply with no
//! parser: a closed placement vocabulary, a literal anchor substring, and
//! a three-state `present` from the lexical observation.

use serde::Serialize;

use super::manager::Manager;
use super::matrix::Pair;
use super::pin::PIN_PREFIX;
use super::venue::Venue;
use super::{Observed, pin};
use crate::embedded::BLOCKS;

/// Every self-depend block, by name.
pub const BLOCK_NAMES: [&str; 12] = [
    "self-depend-input.nix.in",
    "self-depend-outputs-arg.nix.in",
    "self-depend-package.nix.in",
    "self-depend-seed-flake.nix.in",
    "self-depend-envrc-line.in",
    "self-depend-seed-envrc.in",
    "self-depend-mise-entry.toml.in",
    "self-depend-mise-ubi.toml.in",
    "self-depend-mise-seed.toml.in",
    "self-depend-devbox-entry.json.in",
    "self-depend-devbox-seed.json.in",
    "self-depend-asdf-line.in",
];

/// The token every block carries where the pinned flake URL goes.
const PIN_TOKEN: &str = "RK_DEVSHELL_PIN";
/// The token where the bare version goes.
const VERSION_TOKEN: &str = "RK_DEVSHELL_VERSION";
/// The token where the forge path goes.
const REPO_TOKEN: &str = "RK_DEVSHELL_REPO";
/// The token where one rendered mise line goes, in the mise seed.
const TOOL_LINE_TOKEN: &str = "RK_DEVSHELL_TOOL_LINE";

/// One fragment: what to add, where, and whether it is already there.
#[derive(Debug, Clone, Serialize)]
pub struct Fragment {
    /// The stable name: `flake-input`, `outputs-argument`,
    /// `devshell-package`, `mise-tool`, `devbox-package`, `asdf-line`,
    /// or `envrc-sync`.
    pub id: &'static str,
    /// The file it goes into, relative to the target.
    pub file: String,
    /// What it is for, one phrase.
    pub role: &'static str,
    /// How it goes in: `insert-into-attrset`, `add-to-function-head`,
    /// `append-to-list`, `insert-into-table`, `append-to-array`, or
    /// `append-line`.
    pub placement: &'static str,
    /// Where it goes in the file.
    pub anchor: Anchor,
    /// The text to add, rendered.
    pub text: String,
    /// Whether the file already carries it; omitted where the file could
    /// not be judged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub present: Option<bool>,
}

/// Where a fragment goes.
#[derive(Debug, Clone, Serialize)]
pub struct Anchor {
    /// `attrset`, `function-head`, `list`, `table`, `array`, or `file`.
    pub kind: &'static str,
    /// The attribute path, table, array, or file, as a reader names it.
    pub path: &'static str,
    /// A literal substring that locates the anchor, where the observation
    /// found one; never a pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needle: Option<&'static str>,
}

/// The fragments one pair serves, in application order, judged against
/// the target. Every pair ends with the `.envrc` line: the sync runs on
/// directory entry whatever manager moves the pin.
#[must_use]
pub fn fragments(pair: Pair, tag: &str, observed: &Observed) -> Vec<Fragment> {
    let entry = observed.entry(pair.manager);
    let file = entry
        .and_then(|entry| entry.file.clone())
        .unwrap_or_else(|| pair.manager.default_file().to_owned());
    let text = entry.and_then(|entry| entry.text.as_deref());
    let named = entry.is_some_and(|entry| entry.read.names());
    let mut out = match pair {
        Pair {
            manager: Manager::Flake,
            venue: Venue::Flake,
        } => flake_fragments(tag, observed),
        Pair {
            manager: Manager::Mise,
            venue: Venue::Crates | Venue::GithubRelease,
        } => vec![Fragment {
            id: "mise-tool",
            file,
            role: "the pinned rk tool entry",
            placement: "insert-into-table",
            anchor: Anchor {
                kind: "table",
                path: "tools",
                needle: text.and_then(|t| first_found(t, &["[tools]"])),
            },
            text: fragment(mise_block(pair.venue), tag),
            present: Some(named),
        }],
        Pair {
            manager: Manager::Devbox,
            venue: Venue::Flake,
        } => vec![Fragment {
            id: "devbox-package",
            file,
            role: "the rk flake package entry",
            placement: "append-to-array",
            anchor: Anchor {
                kind: "array",
                path: "packages",
                needle: text.and_then(|t| first_found(t, &["\"packages\""])),
            },
            text: fragment("self-depend-devbox-entry.json.in", tag),
            present: Some(named),
        }],
        Pair {
            manager: Manager::Asdf,
            ..
        } => vec![Fragment {
            id: "asdf-line",
            file,
            role: "the line the plugin would take, once a plugin exists",
            placement: "append-line",
            anchor: Anchor {
                kind: "file",
                path: ".tool-versions",
                needle: None,
            },
            text: fragment("self-depend-asdf-line.in", tag),
            present: Some(named),
        }],
        Pair { .. } => Vec::new(),
    };
    out.push(Fragment {
        id: "envrc-sync",
        file: ".envrc".to_owned(),
        role: "the daily sync on directory entry",
        placement: "append-line",
        anchor: Anchor {
            kind: "file",
            path: ".envrc",
            needle: None,
        },
        text: envrc_line(),
        present: Some(observed.envrc.is_present() && observed.envrc_sync),
    });
    out
}

/// The three flake fragments, judged against `flake.nix`.
fn flake_fragments(tag: &str, observed: &Observed) -> Vec<Fragment> {
    let flake = observed.flake_text.as_deref();
    vec![
        Fragment {
            id: "flake-input",
            file: "flake.nix".to_owned(),
            role: "the pinned release-kit input",
            placement: "insert-into-attrset",
            anchor: Anchor {
                kind: "attrset",
                path: "inputs",
                needle: flake.and_then(|text| first_found(text, &["inputs = {", "inputs ="])),
            },
            text: fragment("self-depend-input.nix.in", tag),
            present: Some(flake.is_some() && !matches!(observed.scan, pin::Scan::None)),
        },
        Fragment {
            id: "outputs-argument",
            file: "flake.nix".to_owned(),
            role: "the release-kit argument of the outputs function",
            placement: "add-to-function-head",
            anchor: Anchor {
                kind: "function-head",
                path: "outputs",
                needle: flake.and_then(|text| first_found(text, &["outputs =", "outputs"])),
            },
            text: fragment("self-depend-outputs-arg.nix.in", tag),
            present: flake.map_or(Some(false), outputs_argument_present),
        },
        Fragment {
            id: "devshell-package",
            file: "flake.nix".to_owned(),
            role: "the rk package in the default devshell",
            placement: "append-to-list",
            anchor: Anchor {
                kind: "list",
                path: "devShells.<system>.default.packages",
                needle: flake.and_then(|text| first_found(text, &["packages = [", "devShells"])),
            },
            text: fragment("self-depend-package.nix.in", tag),
            present: flake.map_or(Some(false), devshell_package_present),
        },
    ]
}

/// The mise block for a venue: the cargo backend for the crate, the ubi
/// backend for the release archive.
const fn mise_block(venue: Venue) -> &'static str {
    match venue {
        Venue::GithubRelease => "self-depend-mise-ubi.toml.in",
        Venue::Crates | Venue::Flake => "self-depend-mise-entry.toml.in",
    }
}

/// The whole seed file for a pair whose manager file is absent, pinned
/// at `tag`; `None` for a pair the matrix renders no fragment for.
#[must_use]
pub fn seed(pair: Pair, tag: &str) -> Option<String> {
    match pair {
        Pair {
            manager: Manager::Flake,
            venue: Venue::Flake,
        } => Some(seed_flake(tag)),
        Pair {
            manager: Manager::Mise,
            venue: Venue::Crates | Venue::GithubRelease,
        } => {
            let line = fragment(mise_block(pair.venue), tag);
            Some(
                render(block("self-depend-mise-seed.toml.in"), tag).replace(TOOL_LINE_TOKEN, &line),
            )
        }
        Pair {
            manager: Manager::Devbox,
            venue: Venue::Flake,
        } => Some(render(block("self-depend-devbox-seed.json.in"), tag)),
        Pair { .. } => None,
    }
}

/// The whole seed flake, pinned at `tag`.
#[must_use]
pub fn seed_flake(tag: &str) -> String {
    render(block("self-depend-seed-flake.nix.in"), tag)
}

/// The whole seed `.envrc`, for the flake pair alone: `use flake` is
/// what puts `rk` on the path there, and no other manager loads through
/// direnv by default.
#[must_use]
pub fn seed_envrc() -> String {
    block("self-depend-seed-envrc.in").to_owned()
}

/// The one `.envrc` line, without its newline.
#[must_use]
pub fn envrc_line() -> String {
    block("self-depend-envrc-line.in")
        .trim_end_matches('\n')
        .to_owned()
}

/// The pinned flake-input URL for a tag: the grammar's prefix and the tag.
#[must_use]
pub fn pinned_url(tag: &str) -> String {
    format!("{PIN_PREFIX}{tag}")
}

/// Render one block's tokens with a plain replace; a seed file keeps its
/// final newline.
fn render(text: &str, tag: &str) -> String {
    let bare = tag.strip_prefix('v').unwrap_or(tag);
    text.replace(PIN_TOKEN, &pinned_url(tag))
        .replace(VERSION_TOKEN, bare)
        .replace(REPO_TOKEN, Venue::owner_repo())
}

/// One rendered fragment: no final newline, since a reader places it.
fn fragment(name: &str, tag: &str) -> String {
    render(block(name), tag).trim_end_matches('\n').to_owned()
}

/// One authored block, by name; the payload is compiled in, so a missing
/// name is a build defect the tests catch, never a runtime path.
fn block(name: &str) -> &'static str {
    BLOCKS
        .get_file(name)
        .and_then(|file| file.contents_utf8())
        .unwrap_or_default()
}

/// The first needle the text holds, as the literal a reader can search.
fn first_found(text: &str, needles: &[&'static str]) -> Option<&'static str> {
    needles.iter().copied().find(|needle| text.contains(needle))
}

/// Whether the outputs function head names `release-kit`: `Some(true)`
/// where it does, `Some(false)` where the head is an explicit set that
/// lacks it, and `None` where the head binds its inputs another way — an
/// ellipsis or an `@` pattern — or no head was found at all.
fn outputs_argument_present(text: &str) -> Option<bool> {
    let start = text.find("outputs")?;
    let rest = &text[start + "outputs".len()..];
    let head = &rest[..rest.find(':')?];
    if head.contains("release-kit") {
        return Some(true);
    }
    if head.contains("...") || head.contains('@') || !head.contains('{') {
        return None;
    }
    Some(false)
}

/// Whether the flake already takes the package: `Some(true)` where the
/// package reference appears, `Some(false)` where a devshell exists
/// without it, and `None` where no devshell was found to judge.
fn devshell_package_present(text: &str) -> Option<bool> {
    let package = block("self-depend-package.nix.in").trim_end_matches('\n');
    let prefix = package.split("${").next().unwrap_or(package);
    if text.contains(prefix) {
        return Some(true);
    }
    text.contains("devShells").then_some(false)
}

#[cfg(test)]
mod tests {
    use super::{
        BLOCK_NAMES, PIN_TOKEN, Pair, REPO_TOKEN, TOOL_LINE_TOKEN, VERSION_TOKEN, block,
        devshell_package_present, envrc_line, fragment, outputs_argument_present, render, seed,
        seed_envrc, seed_flake,
    };
    use crate::self_depend::manager::Manager;
    use crate::self_depend::pin::{PIN_PREFIX, Scan, scan};
    use crate::self_depend::venue::Venue;

    /// The authored input fragment and the source grammar agree: the
    /// rendered block is exactly what the matcher reads back.
    #[test]
    fn the_pin_matcher_matches_the_authored_input_fragment() {
        let text = fragment("self-depend-input.nix.in", "v0.2.16");
        match scan(&text) {
            Scan::One(pin) => assert_eq!(pin.tag, "v0.2.16"),
            other => panic!("the fragment must scan as one pin: {other:?}"),
        }
        match scan(&seed_flake("v0.2.16")) {
            Scan::One(pin) => assert_eq!(pin.tag, "v0.2.16"),
            other => panic!("the seed must scan as one pin: {other:?}"),
        }
    }

    #[test]
    fn every_fragment_renders_its_tag_and_keeps_the_system_interpolation() {
        for name in BLOCK_NAMES {
            let authored = block(name);
            assert!(!authored.is_empty(), "{name}: the block is authored");
            assert!(authored.ends_with('\n'), "{name}: one final newline");
            let rendered = render(authored, "v9.9.9");
            for token in [PIN_TOKEN, VERSION_TOKEN, REPO_TOKEN] {
                assert!(!rendered.contains(token), "{name}: {token} renders");
            }
        }
        let package = fragment("self-depend-package.nix.in", "v9.9.9");
        assert_eq!(package, "release-kit.packages.${system}.default");
        let seed = seed_flake("v9.9.9");
        assert!(seed.contains("${system}"), "the interpolation survives");
        assert!(seed.contains(&format!("{PIN_PREFIX}v9.9.9")));
        assert!(seed.ends_with("}\n"), "a seed file keeps its final newline");
        assert!(seed_envrc().ends_with('\n'));
        assert!(
            !envrc_line().ends_with('\n'),
            "a fragment carries no newline"
        );
        assert!(seed_envrc().ends_with(&format!("{}\n", envrc_line())));
    }

    /// Every fragment and seed a rendering pair serves comes from a file
    /// under `blocks/`, and none from a source literal.
    #[test]
    fn no_fragment_is_a_source_literal() {
        assert_eq!(
            fragment("self-depend-mise-entry.toml.in", "v0.2.16"),
            "\"cargo:release-kit\" = \"0.2.16\""
        );
        assert_eq!(
            fragment("self-depend-mise-ubi.toml.in", "0.2.16"),
            "\"ubi:gubasso/release-kit\" = { version = \"0.2.16\", exe = \"rk\" }"
        );
        assert_eq!(
            fragment("self-depend-devbox-entry.json.in", "v0.2.16"),
            "\"github:gubasso/release-kit/v0.2.16#default\""
        );
        assert_eq!(
            fragment("self-depend-asdf-line.in", "v0.2.16"),
            "release-kit 0.2.16"
        );
        let mise = Pair {
            manager: Manager::Mise,
            venue: Venue::Crates,
        };
        assert_eq!(
            seed(mise, "v0.2.16").as_deref(),
            Some("[tools]\n\"cargo:release-kit\" = \"0.2.16\"\n")
        );
        assert!(
            block("self-depend-mise-seed.toml.in").contains(TOOL_LINE_TOKEN),
            "the mise seed takes its line from the entry block"
        );
        let devbox = Pair {
            manager: Manager::Devbox,
            venue: Venue::Flake,
        };
        assert_eq!(
            seed(devbox, "v0.2.16").as_deref(),
            Some(
                "{\n  \"packages\": [\n    \"github:gubasso/release-kit/v0.2.16#default\"\n  ]\n}\n"
            )
        );
        let flake = Pair {
            manager: Manager::Flake,
            venue: Venue::Flake,
        };
        assert_eq!(
            seed(flake, "v0.2.16").as_deref(),
            Some(seed_flake("v0.2.16").as_str())
        );
        for manual in [
            Pair {
                manager: Manager::Asdf,
                venue: Venue::Crates,
            },
            Pair {
                manager: Manager::Flake,
                venue: Venue::Crates,
            },
            Pair {
                manager: Manager::Mise,
                venue: Venue::Flake,
            },
        ] {
            assert_eq!(seed(manual, "v0.2.16"), None, "{manual:?} seeds nothing");
        }
    }

    #[test]
    fn the_outputs_head_is_judged_lexically() {
        assert_eq!(
            outputs_argument_present("outputs = { self, nixpkgs, release-kit }: {}"),
            Some(true)
        );
        assert_eq!(
            outputs_argument_present("outputs =\n    { self, nixpkgs }:\n    {}"),
            Some(false)
        );
        assert_eq!(
            outputs_argument_present("outputs = { self, ... }: {}"),
            None,
            "an ellipsis binds the input another way"
        );
        assert_eq!(outputs_argument_present("outputs = inputs: {}"), None);
        assert_eq!(outputs_argument_present("{ inputs = {}; }"), None);
    }

    #[test]
    fn the_devshell_package_is_judged_lexically() {
        assert_eq!(
            devshell_package_present(
                "devShells = { default = mkShell { packages = [ release-kit.packages.${system}.default ]; }; }"
            ),
            Some(true)
        );
        assert_eq!(
            devshell_package_present("devShells = { default = mkShell { packages = [ just ]; }; }"),
            Some(false)
        );
        assert_eq!(devshell_package_present("packages = {}"), None);
    }
}
