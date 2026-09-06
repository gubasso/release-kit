//! The texts `rk depend add` serves: one fragment set per manager, and
//! the seed file for a manager the target has no file for.
//!
//! Every text is an authored file under `blocks/`, per
//! `distribution:a-human-faced-artifact-is-authored-text`; each carries
//! `RK_DEP_*` tokens rendered with a plain replace, never a format, so a
//! `${system}` interpolation survives untouched. A fragment describes
//! where it goes in a form a coding agent can apply with no parser: a
//! closed placement vocabulary, a literal anchor substring, and a
//! three-state `present` from the lexical observation.

use serde::Serialize;

use crate::embedded::BLOCKS;

/// Every depend block, by name.
pub const BLOCK_NAMES: [&str; 12] = [
    "depend-flake-input.nix.in",
    "depend-flake-outputs-arg.nix.in",
    "depend-flake-package.nix.in",
    "depend-seed-flake.nix.in",
    "depend-mise-cargo.toml.in",
    "depend-mise-ubi.toml.in",
    "depend-mise-pipx.toml.in",
    "depend-mise-npm.toml.in",
    "depend-seed-mise.toml.in",
    "depend-asdf-line.in",
    "depend-devbox-flake.json.in",
    "depend-seed-devbox.json.in",
];

/// The values a block's tokens render to. A token whose value is absent
/// is left in place; the matrix selects no block that needs it.
#[derive(Debug, Clone, Default)]
pub struct Tokens {
    /// `RK_DEP_NAME`: the package name.
    pub name: String,
    /// `RK_DEP_INPUT`: the flake input name, a Nix identifier derived
    /// from the package name.
    pub input: String,
    /// `RK_DEP_VERSION`: the bare version.
    pub version: String,
    /// `RK_DEP_TAG`: the release tag.
    pub tag: String,
    /// `RK_DEP_OWNER_REPO`: the forge path.
    pub owner_repo: Option<String>,
    /// `RK_DEP_BIN`: the executable a prebuilt archive carries.
    pub bin: Option<String>,
    /// `RK_DEP_FLAKE_REF`: the flake reference at the tag.
    pub flake_ref: Option<String>,
    /// `RK_DEP_TOOL_LINE`: one rendered mise line, for the mise seed.
    pub tool_line: Option<String>,
}

/// One fragment: what to add, where, and whether it is already there.
#[derive(Debug, Clone, Serialize)]
pub struct Fragment {
    /// The stable name: `flake-input`, `outputs-argument`,
    /// `devshell-package`, `mise-tool`, `asdf-line`, or `devbox-package`.
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
    /// The attribute path, table, key, or file, as a reader names it.
    pub path: String,
    /// A literal substring that locates the anchor, where the observation
    /// found one; never a pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needle: Option<&'static str>,
}

/// The flake reference for a forge host and path at a tag, where the
/// host has a flake reference type: `github:` and `gitlab:`.
#[must_use]
pub fn flake_ref(host: Option<&str>, owner_repo: Option<&str>, tag: &str) -> Option<String> {
    let owner_repo = owner_repo?;
    let scheme = match host? {
        "github.com" => "github",
        "gitlab.com" => "gitlab",
        _ => return None,
    };
    Some(format!("{scheme}:{owner_repo}/{tag}"))
}

/// The Nix keywords an identifier may not be.
const NIX_KEYWORDS: [&str; 10] = [
    "assert", "else", "if", "in", "inherit", "let", "or", "rec", "then", "with",
];

/// A flake input name for a package name: a Nix identifier, derived
/// deterministically.
///
/// Every character outside `[A-Za-z0-9_-]` becomes `-`, runs collapse,
/// a name that cannot start an identifier takes the `dep-` prefix, and
/// a keyword takes the `-input` suffix.
#[must_use]
pub fn nix_input_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    let mut out = if trimmed.is_empty() {
        "dep".to_owned()
    } else {
        trimmed.to_owned()
    };
    if !out.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        out = format!("dep-{out}");
    }
    if NIX_KEYWORDS.contains(&out.as_str()) {
        out.push_str("-input");
    }
    out
}

/// Render every token whose value is known, with a plain replace.
#[must_use]
pub fn render(text: &str, tokens: &Tokens) -> String {
    let mut out = text
        .replace("RK_DEP_INPUT", &tokens.input)
        .replace("RK_DEP_NAME", &tokens.name)
        .replace("RK_DEP_VERSION", &tokens.version)
        .replace("RK_DEP_TAG", &tokens.tag);
    for (token, value) in [
        ("RK_DEP_OWNER_REPO", &tokens.owner_repo),
        ("RK_DEP_BIN", &tokens.bin),
        ("RK_DEP_FLAKE_REF", &tokens.flake_ref),
        ("RK_DEP_TOOL_LINE", &tokens.tool_line),
    ] {
        if let Some(value) = value {
            out = out.replace(token, value);
        }
    }
    out
}

/// One rendered fragment: no final newline, since a reader places it.
#[must_use]
pub fn fragment(name: &str, tokens: &Tokens) -> String {
    render(block(name), tokens)
        .trim_end_matches('\n')
        .to_owned()
}

/// One rendered seed file, its final newline kept.
#[must_use]
pub fn seed(name: &str, tokens: &Tokens) -> String {
    render(block(name), tokens)
}

/// One authored block, by name; the payload is compiled in, so a missing
/// name is a build defect the tests catch, never a runtime path.
#[must_use]
pub fn block(name: &str) -> &'static str {
    BLOCKS
        .get_file(name)
        .and_then(|file| file.contents_utf8())
        .unwrap_or_default()
}

/// The declaration of the flake input named `input`, in any of its
/// forms — `name = { … };`, `inputs.name.url = "…";`, a compact or a
/// quoted binding — as the text of its value.
#[must_use]
pub fn input_binding<'a>(text: &'a str, input: &str) -> Option<&'a str> {
    super::nix::input_declaration(text, input)
}

/// The first needle the text holds, as the literal a reader can search.
#[must_use]
pub fn first_found(text: &str, needles: &[&'static str]) -> Option<&'static str> {
    needles.iter().copied().find(|needle| text.contains(needle))
}

/// Whether a flake's outputs function head names `input`.
///
/// `Some(true)` where it does, `Some(false)` where the head is an
/// explicit set that lacks it, and `None` where the head binds its
/// inputs another way — an ellipsis or an `@` pattern — or no head was
/// found at all.
#[must_use]
pub fn outputs_argument_present(text: &str, input: &str) -> Option<bool> {
    let start = text.find("outputs")?;
    let rest = &text[start + "outputs".len()..];
    let head = &rest[..rest.find(':')?];
    if head.contains(input) {
        return Some(true);
    }
    if head.contains("...") || head.contains('@') || !head.contains('{') {
        return None;
    }
    Some(false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        BLOCK_NAMES, Tokens, block, flake_ref, fragment, nix_input_name, outputs_argument_present,
        render, seed,
    };

    fn full() -> Tokens {
        Tokens {
            name: "sample-tool".into(),
            input: "sample-tool".into(),
            version: "1.4.0".into(),
            tag: "v1.4.0".into(),
            owner_repo: Some("acme/sample-tool".into()),
            bin: Some("sam".into()),
            flake_ref: Some("github:acme/sample-tool/v1.4.0".into()),
            tool_line: Some("\"cargo:sample-tool\" = \"1.4.0\"".into()),
        }
    }

    /// SATISFIES dependencies:a-fragment-names-no-project-of-its-own
    #[test]
    fn every_depend_block_renders_all_its_tokens() {
        let tokens = full();
        for name in BLOCK_NAMES {
            let authored = block(name);
            assert!(!authored.is_empty(), "{name}: the block is authored");
            assert!(authored.ends_with('\n'), "{name}: one final newline");
            let rendered = render(authored, &tokens);
            assert!(!rendered.contains("RK_DEP_"), "{name}: every token renders");
        }
        assert_eq!(
            fragment("depend-flake-package.nix.in", &tokens),
            "sample-tool.packages.${system}.default"
        );
        assert_eq!(
            fragment("depend-mise-ubi.toml.in", &tokens),
            "\"ubi:acme/sample-tool\" = { version = \"1.4.0\", exe = \"sam\" }"
        );
        assert_eq!(
            fragment("depend-devbox-flake.json.in", &tokens),
            "\"github:acme/sample-tool/v1.4.0#default\""
        );
    }

    #[test]
    fn a_seed_keeps_its_final_newline_and_a_fragment_drops_it() {
        let tokens = full();
        let flake = seed("depend-seed-flake.nix.in", &tokens);
        assert!(flake.ends_with("}\n"));
        assert!(flake.contains("${system}"), "the interpolation survives");
        assert!(flake.contains("github:acme/sample-tool/v1.4.0"));
        let mise = seed("depend-seed-mise.toml.in", &tokens);
        assert_eq!(mise, "[tools]\n\"cargo:sample-tool\" = \"1.4.0\"\n");
        assert!(!fragment("depend-asdf-line.in", &tokens).ends_with('\n'));
        assert_eq!(
            fragment("depend-asdf-line.in", &tokens),
            "sample-tool 1.4.0"
        );
    }

    #[test]
    fn the_flake_ref_carries_no_owner_of_its_own() {
        assert_eq!(
            flake_ref(Some("github.com"), Some("acme/sample-tool"), "v1.4.0").as_deref(),
            Some("github:acme/sample-tool/v1.4.0")
        );
        assert_eq!(
            flake_ref(Some("gitlab.com"), Some("group/sample"), "1.0.0").as_deref(),
            Some("gitlab:group/sample/1.0.0")
        );
        assert_eq!(flake_ref(Some("codeberg.org"), Some("a/b"), "v1"), None);
        assert_eq!(flake_ref(Some("github.com"), None, "v1"), None);
        let without_ref = Tokens {
            flake_ref: None,
            ..full()
        };
        assert!(
            render(block("depend-flake-input.nix.in"), &without_ref).contains("RK_DEP_FLAKE_REF"),
            "an unknown value is left as its token, never invented"
        );
    }

    #[test]
    fn a_package_name_becomes_a_nix_identifier() {
        assert_eq!(nix_input_name("sample-tool"), "sample-tool");
        assert_eq!(nix_input_name("@acme/tool"), "acme-tool");
        assert_eq!(nix_input_name("my.tool"), "my-tool");
        assert_eq!(nix_input_name("7zip"), "dep-7zip");
        assert_eq!(nix_input_name("with"), "with-input");
        assert_eq!(nix_input_name("@@"), "dep");
        let scoped = Tokens {
            name: "@acme/tool".into(),
            input: nix_input_name("@acme/tool"),
            ..full()
        };
        let rendered = fragment("depend-flake-input.nix.in", &scoped);
        assert!(rendered.starts_with("acme-tool = {"), "{rendered}");
        assert!(!rendered.contains('@'));
    }

    #[test]
    fn an_input_binding_is_read_to_its_close() {
        use super::input_binding;
        let text = "inputs = {\n  acme-tool = {\n    url = \"github:other/thing/v1\";\n  };\n  nixpkgs.url = \"x\";\n};";
        let body = input_binding(text, "acme-tool").expect("a binding");
        assert!(body.contains("github:other/thing/v1"));
        assert!(!body.contains("nixpkgs"));
        assert!(input_binding(text, "nixpkgs").is_some_and(|v| v.contains("\"x\"")));
        assert!(
            input_binding(
                "inputs.acme-tool.url = \"github:other/thing/v1\";",
                "acme-tool"
            )
            .is_some_and(|v| v.contains("github:other/thing/v1")),
            "the dotted form is a declaration too"
        );
        assert_eq!(
            input_binding("packages = [ acme-tool ];", "acme-tool"),
            None
        );
    }

    #[test]
    fn the_outputs_head_is_judged_lexically() {
        assert_eq!(
            outputs_argument_present(
                "outputs = { self, nixpkgs, sample-tool }: {}",
                "sample-tool"
            ),
            Some(true)
        );
        assert_eq!(
            outputs_argument_present("outputs =\n    { self, nixpkgs }:\n    {}", "sample-tool"),
            Some(false)
        );
        assert_eq!(
            outputs_argument_present("outputs = { self, ... }: {}", "sample-tool"),
            None
        );
        assert_eq!(
            outputs_argument_present("{ inputs = {}; }", "sample-tool"),
            None
        );
    }
}
