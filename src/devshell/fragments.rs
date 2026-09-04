//! The texts `rk devshell add` serves: three flake fragments, the
//! `.envrc` line, and the seed pair for a target that has neither file.
//!
//! Every text is an authored file under `blocks/`, per
//! `distribution:a-human-faced-artifact-is-authored-text`; each carries
//! an `RK_DEVSHELL_PIN` token rendered with a plain replace, never a
//! format, so the `${system}` interpolation survives untouched. The
//! fragments describe where they go in a form a coding agent can apply
//! with no parser: a closed placement vocabulary, a literal anchor
//! substring, and a three-state `present` from the lexical observation.

use serde::Serialize;

use super::pin::PIN_PREFIX;
use super::{Observed, pin};
use crate::embedded::BLOCKS;

/// The token every devshell block carries where the pinned URL goes.
const PIN_TOKEN: &str = "RK_DEVSHELL_PIN";

/// One fragment: what to add, where, and whether it is already there.
#[derive(Debug, Clone, Serialize)]
pub struct Fragment {
    /// The stable name: `flake-input`, `outputs-argument`,
    /// `devshell-package`, or `envrc-sync`.
    pub id: &'static str,
    /// The file it goes into, relative to the target.
    pub file: &'static str,
    /// What it is for, one phrase.
    pub role: &'static str,
    /// How it goes in: `insert-into-attrset`, `add-to-function-head`,
    /// `append-to-list`, or `append-line`.
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
    /// `attrset`, `function-head`, `list`, or `file`.
    pub kind: &'static str,
    /// The attribute path or the file, as a reader names it.
    pub path: &'static str,
    /// A literal substring that locates the anchor, where the observation
    /// found one; never a pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needle: Option<&'static str>,
}

/// The four fragments, in application order, judged against the target.
#[must_use]
pub fn fragments(tag: &str, observed: &Observed) -> Vec<Fragment> {
    let flake = observed.flake_text.as_deref();
    let envrc_present = observed.envrc.is_present();
    vec![
        Fragment {
            id: "flake-input",
            file: "flake.nix",
            role: "the pinned release-kit input",
            placement: "insert-into-attrset",
            anchor: Anchor {
                kind: "attrset",
                path: "inputs",
                needle: flake.and_then(|text| first_found(text, &["inputs = {", "inputs ="])),
            },
            text: fragment("devshell-input.nix.in", tag),
            present: Some(flake.is_some() && !matches!(observed.scan, pin::Scan::None)),
        },
        Fragment {
            id: "outputs-argument",
            file: "flake.nix",
            role: "the release-kit argument of the outputs function",
            placement: "add-to-function-head",
            anchor: Anchor {
                kind: "function-head",
                path: "outputs",
                needle: flake.and_then(|text| first_found(text, &["outputs =", "outputs"])),
            },
            text: fragment("devshell-outputs-arg.nix.in", tag),
            present: flake.map_or(Some(false), outputs_argument_present),
        },
        Fragment {
            id: "devshell-package",
            file: "flake.nix",
            role: "the rk package in the default devshell",
            placement: "append-to-list",
            anchor: Anchor {
                kind: "list",
                path: "devShells.<system>.default.packages",
                needle: flake.and_then(|text| first_found(text, &["packages = [", "devShells"])),
            },
            text: fragment("devshell-package.nix.in", tag),
            present: flake.map_or(Some(false), devshell_package_present),
        },
        Fragment {
            id: "envrc-sync",
            file: ".envrc",
            role: "the daily sync on directory entry",
            placement: "append-line",
            anchor: Anchor {
                kind: "file",
                path: ".envrc",
                needle: None,
            },
            text: envrc_line(),
            present: Some(envrc_present && observed.envrc_sync),
        },
    ]
}

/// The whole seed flake, pinned at `tag`.
#[must_use]
pub fn seed_flake(tag: &str) -> String {
    render(block("devshell-seed-flake.nix.in"), tag)
}

/// The whole seed `.envrc`.
#[must_use]
pub fn seed_envrc() -> String {
    block("devshell-seed-envrc.in").to_owned()
}

/// The one `.envrc` line, without its newline.
#[must_use]
pub fn envrc_line() -> String {
    block("devshell-envrc-line.in")
        .trim_end_matches('\n')
        .to_owned()
}

/// The pinned flake-input URL for a tag: the grammar's prefix and the tag.
#[must_use]
pub fn pinned_url(tag: &str) -> String {
    format!("{PIN_PREFIX}{tag}")
}

/// Render one block's token with a plain replace; a seed file keeps its
/// final newline.
fn render(text: &str, tag: &str) -> String {
    text.replace(PIN_TOKEN, &pinned_url(tag))
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
    let package = block("devshell-package.nix.in").trim_end_matches('\n');
    let prefix = package.split("${").next().unwrap_or(package);
    if text.contains(prefix) {
        return Some(true);
    }
    text.contains("devShells").then_some(false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::{
        PIN_TOKEN, block, devshell_package_present, envrc_line, fragment, outputs_argument_present,
        render, seed_envrc, seed_flake,
    };
    use crate::devshell::pin::{PIN_PREFIX, Scan, scan};

    /// The authored input fragment and the source grammar agree: the
    /// rendered block is exactly what the matcher reads back.
    #[test]
    fn the_pin_matcher_matches_the_authored_input_fragment() {
        let text = fragment("devshell-input.nix.in", "v0.2.16");
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
        for name in [
            "devshell-input.nix.in",
            "devshell-outputs-arg.nix.in",
            "devshell-package.nix.in",
            "devshell-envrc-line.in",
            "devshell-seed-flake.nix.in",
            "devshell-seed-envrc.in",
        ] {
            let rendered = render(block(name), "v9.9.9");
            assert!(!rendered.contains(PIN_TOKEN), "{name}: the token renders");
            assert!(!rendered.is_empty(), "{name}: the block is authored");
        }
        let package = fragment("devshell-package.nix.in", "v9.9.9");
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
