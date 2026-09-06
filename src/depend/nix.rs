//! Lexical helpers over Nix text, shared by the source and target
//! observations.
//!
//! No Nix parser is embedded: these scanners know comments, strings,
//! bracket depth, `let … in`, and attribute tokens, which is what a
//! presence judgement needs and no more. Where they cannot judge, the
//! callers report a manual pair rather than a guess.

/// Whether a byte can continue a Nix identifier.
#[must_use]
pub const fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'\'')
}

/// Whether `text[index..]` starts the whole word `word`.
fn word_at(text: &str, index: usize, word: &str) -> bool {
    let bytes = text.as_bytes();
    let end = index + word.len();
    text[index..].starts_with(word)
        && (index == 0 || !is_ident(bytes[index - 1]))
        && bytes.get(end).is_none_or(|b| !is_ident(*b))
}

/// Nix text with block comments, line comments, and string contents
/// blanked, so a word inside any of them never reads as syntax.
#[must_use]
pub fn scrub(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            let end = text[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |e| i + 2 + e + 2);
            out.extend(std::iter::repeat_n(' ', end - i));
            i = end;
        } else if bytes[i] == b'#' {
            let end = text[i..].find('\n').map_or(bytes.len(), |e| i + e);
            out.extend(std::iter::repeat_n(' ', end - i));
            i = end;
        } else if bytes[i..].starts_with(b"''") {
            let end = text[i + 2..]
                .find("''")
                .map_or(bytes.len(), |e| i + 2 + e + 2);
            out.extend(std::iter::repeat_n(' ', end - i));
            i = end;
        } else if bytes[i] == b'\x22' {
            // 0x22 is the double quote, spelled as a byte so the source
            // scan in `embedded` never reads it as a string that opens here.
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'\x22' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(bytes.len());
            out.extend(std::iter::repeat_n(' ', end - i));
            i = end;
        } else {
            let c = text[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// The text of one binding's value, up to the `;` that closes it.
///
/// The closing `;` stands at bracket depth zero, outside every
/// `let … in`, and past the `;` that each `with <expr>;` or
/// `assert <expr>;` clause owns at the depth where the clause began.
#[must_use]
pub fn binding_value(text: &str) -> &str {
    let mut depth = 0usize;
    let mut lets = 0usize;
    let mut clauses: Vec<(usize, usize)> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' | b'(' => depth += 1,
            b'}' | b']' | b')' => {
                depth = depth.saturating_sub(1);
                clauses.retain(|(d, _)| *d <= depth);
            }
            b';' => {
                if clauses.last() == Some(&(depth, lets)) {
                    clauses.pop();
                } else if depth == 0 && lets == 0 {
                    return &text[..i];
                }
            }
            b'l' if word_at(text, i, "let") => lets += 1,
            b'i' if word_at(text, i, "in") => {
                lets = lets.saturating_sub(1);
                clauses.retain(|(_, l)| *l <= lets);
            }
            b'w' if word_at(text, i, "with") => clauses.push((depth, lets)),
            b'a' if word_at(text, i, "assert") => clauses.push((depth, lets)),
            _ => {}
        }
        i += 1;
    }
    text
}

/// Whether `text` binds a default package path at `index`.
///
/// The path is `packages.<system>.default`, the system segment an
/// identifier or an interpolation, or the bare `packages.default`. It
/// stands as a whole token, not as a segment of a longer path, and an
/// `=` follows it.
#[must_use]
pub fn is_default_package_path(text: &str, index: usize) -> bool {
    let bound = |tail: &str| tail.trim_start().starts_with('=');
    if index > 0 && {
        let previous = text.as_bytes()[index - 1];
        previous == b'.' || is_ident(previous)
    } {
        return false;
    }
    let rest = &text[index..];
    let Some(rest) = rest.strip_prefix("packages.") else {
        return false;
    };
    if let Some(tail) = rest.strip_prefix("default") {
        return !tail.starts_with(|c: char| is_ident(c as u8)) && bound(tail);
    }
    let after_system = rest.strip_prefix("${").map_or_else(
        || {
            let bytes = rest.as_bytes();
            let end = bytes
                .iter()
                .position(|b| !is_ident(*b))
                .unwrap_or(bytes.len());
            (end > 0).then(|| &rest[end..])
        },
        |interpolated| interpolated.find('}').map(|end| &interpolated[end + 1..]),
    );
    after_system.is_some_and(|tail| {
        tail.strip_prefix(".default")
            .is_some_and(|t| !t.starts_with(|c: char| is_ident(c as u8)) && bound(t))
    })
}

/// The text with every `let … in` binding list blanked, so a local
/// binding never reads as an attribute of the value the expression
/// returns.
#[must_use]
pub fn without_let_bindings(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut lets = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if word_at(text, i, "let") {
            lets += 1;
        } else if word_at(text, i, "in") && lets > 0 {
            lets -= 1;
            out.push_str("  ");
            i += 2;
            continue;
        }
        let c = text[i..].chars().next().unwrap_or(' ');
        out.push(if lets > 0 && !c.is_whitespace() {
            ' '
        } else {
            c
        });
        i += c.len_utf8();
    }
    out
}

/// The declaration of the flake input `input` inside a flake text.
///
/// Both forms are read: `inputs.<input>… = …;`, and `<input> = …;`
/// inside the body of `inputs = { … };`. Comments and strings are
/// scrubbed for the search and the raw text is returned, so the URL
/// survives.
#[must_use]
pub fn input_declaration<'a>(raw: &'a str, input: &str) -> Option<&'a str> {
    let code = scrub(raw);
    attribute_positions(&code, "inputs")
        .into_iter()
        .find_map(|(index, after)| {
            if let Some(path) = after.strip_prefix('.') {
                let dotted = path.strip_prefix(input)?;
                if dotted.starts_with(|c: char| is_ident(c as u8)) && !dotted.starts_with('.') {
                    return None;
                }
                let range = attribute_range(&code[index..], "inputs")?;
                return Some(&raw[index + range.start..index + range.end]);
            }
            let body = attribute_range(&code[index..], "inputs")?;
            let body_text = &code[index + body.start..index + body.end];
            let inner = attribute_range(body_text, input)?;
            Some(&raw[index + body.start + inner.start..index + body.start + inner.end])
        })
}

/// Whether `text` binds the attribute `name`: the name as a whole token,
/// bare or quoted, followed by `=`.
#[must_use]
pub fn names_attribute(text: &str, name: &str) -> bool {
    attribute_positions(text, name)
        .into_iter()
        .any(|(_, after)| after.trim_start().starts_with('='))
}

/// The value of the attribute `name` where `text` binds it, as
/// `name = <value>;` or through a path `name.<rest> = <value>;`: the text
/// after the `=` up to the `;` that closes the binding.
#[must_use]
pub fn attribute_value<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    attribute_range(text, name).map(|range| &text[range])
}

/// The byte range of the attribute `name`'s value in `text`, as
/// [`attribute_value`] slices it.
#[must_use]
pub fn attribute_range(text: &str, name: &str) -> Option<std::ops::Range<usize>> {
    attribute_positions(text, name)
        .into_iter()
        .find_map(|(_, after)| {
            let after = after.trim_start();
            let rest = after.strip_prefix('.').map_or(after, |path| {
                let bytes = path.as_bytes();
                let end = bytes
                    .iter()
                    .position(|b| !(is_ident(*b) || *b == b'.'))
                    .unwrap_or(bytes.len());
                path[end..].trim_start()
            });
            let value = rest.strip_prefix('=')?;
            let start = text.len() - value.len();
            Some(start..start + binding_value(value).len())
        })
}

/// Every position where `name` stands as an attribute token, bare or in
/// double quotes, with the text that follows it.
fn attribute_positions<'a>(text: &'a str, name: &str) -> Vec<(usize, &'a str)> {
    let bytes = text.as_bytes();
    text.match_indices(name)
        .filter_map(|(index, _)| {
            let end = index + name.len();
            let quoted =
                index > 0 && bytes[index - 1] == b'\x22' && bytes.get(end) == Some(&b'\x22');
            let bare = (index == 0 || !is_ident(bytes[index - 1]))
                && bytes.get(end).is_none_or(|b| !is_ident(*b));
            if quoted {
                Some((index, &text[end + 1..]))
            } else if bare {
                Some((index, &text[end..]))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        attribute_value, binding_value, input_declaration, names_attribute, scrub,
        without_let_bindings,
    };

    #[test]
    fn the_scrub_blanks_comments_and_strings_only() {
        let text = "a = \"x # y\"; # note\n/* block */ b = ''multi\nline''; c = 1;";
        let out = scrub(text);
        assert_eq!(out.len(), text.len());
        assert!(out.contains("a =") && out.contains("b =") && out.contains("c = 1;"));
        assert!(!out.contains("note") && !out.contains("block") && !out.contains("multi"));
        assert!(
            !out.contains("# y"),
            "a hash inside a string is string content"
        );
    }

    #[test]
    fn a_binding_value_survives_let_and_brackets() {
        assert_eq!(
            binding_value("{ a = 1; b = 2; }; rest"),
            "{ a = 1; b = 2; }"
        );
        assert_eq!(
            binding_value("let x = pkgs.hello; in { default = x; }; devShells = {};"),
            "let x = pkgs.hello; in { default = x; }"
        );
        assert_eq!(
            binding_value("eachSystem (pkgs: { tool = 1; }); more;"),
            "eachSystem (pkgs: { tool = 1; })"
        );
        assert_eq!(
            binding_value("with pkgs; { default = hello; }; next;"),
            "with pkgs; { default = hello; }"
        );
        assert_eq!(
            binding_value("assert x; with pkgs; hello; next;"),
            "assert x; with pkgs; hello"
        );
        assert_eq!(
            binding_value("{ tool = with pkgs; hello; }; devShells.default = 1;"),
            "{ tool = with pkgs; hello; }",
            "a nested clause closes with its bracket"
        );
        assert_eq!(
            binding_value("let x = with pkgs; hello; in { tool = x; }; devShells.default = 1;"),
            "let x = with pkgs; hello; in { tool = x; }",
            "a clause inside a let closes with the let"
        );
        assert_eq!(binding_value("no terminator"), "no terminator");
        assert_eq!(binding_value("inherit (x) a; b;"), "inherit (x) a");
    }

    #[test]
    fn a_let_binding_is_not_an_attribute_of_the_value() {
        let text = "eachSystem (system: let default = pkgs.hello; in { tool = default; })";
        let stripped = without_let_bindings(text);
        assert_eq!(stripped.len(), text.len());
        assert!(!names_attribute(&stripped, "default"));
        assert!(names_attribute(
            &without_let_bindings("let x = 1; in { default = x; }"),
            "default"
        ));
    }

    #[test]
    fn an_input_declaration_is_scoped_to_the_inputs() {
        let braces = "{ inputs = {\n  sample-tool = {\n    url = \"github:other/thing/v1\";\n  };\n }; outputs = _: {}; }";
        assert!(
            input_declaration(braces, "sample-tool")
                .is_some_and(|v| v.contains("github:other/thing/v1"))
        );
        let dotted =
            "{ inputs.sample-tool.url = \"github:other/thing/v1\"; inputs.nixpkgs.url = \"n\"; }";
        assert!(
            input_declaration(dotted, "sample-tool")
                .is_some_and(|v| v.contains("github:other/thing/v1"))
        );
        assert!(
            input_declaration(dotted, "sample").is_none(),
            "a prefix is not the input"
        );
        let output = "{ inputs = { nixpkgs.url = \"n\"; }; outputs = { self, nixpkgs }: { packages.x86_64-linux.sample-tool = nixpkgs.hello; }; }";
        assert_eq!(
            input_declaration(output, "sample-tool"),
            None,
            "an output is not an input"
        );
        let commented = "{ inputs = {\n  # sample-tool = { url = \"github:other/thing/v1\"; };\n  nixpkgs.url = \"n\";\n }; }";
        assert_eq!(
            input_declaration(commented, "sample-tool"),
            None,
            "a comment is not a declaration"
        );
    }

    #[test]
    fn a_default_package_path_is_system_qualified_or_bare() {
        use super::is_default_package_path;
        for text in [
            "packages.default = x;",
            "packages.x86_64-linux.default = x;",
            "packages.${system}.default = x;",
            "packages.${pkgs.system}.default =\n  x;",
        ] {
            assert!(is_default_package_path(text, 0), "{text}");
        }
        for text in [
            "packages.defaultTool = x;",
            "packages.x86_64-linux.defaults = x;",
            "packages.x86_64-linux.tool = x;",
            "packages = { };",
            "packages.a.b.default = x;",
            "packages.${system}.default.meta = x;",
            "packages.${system}.default ]",
        ] {
            assert!(!is_default_package_path(text, 0), "{text}");
        }
        let reference = "tool-input.packages.${system}.default = x;";
        assert!(
            !is_default_package_path(reference, "tool-input.".len()),
            "a segment of a longer path is a reference"
        );
        assert!(
            !is_default_package_path("mypackages.default = x;", 2),
            "a longer identifier is not the packages output"
        );
    }

    #[test]
    fn an_attribute_is_a_whole_token() {
        assert!(names_attribute("{ default = x; }", "default"));
        assert!(names_attribute("{ \"default\" = x; }", "default"));
        assert!(!names_attribute("{ notdefault = x; }", "default"));
        assert!(!names_attribute("{ default-tool = x; }", "default"));
        assert!(
            !names_attribute("f default", "default"),
            "a value is not a binding"
        );
    }

    #[test]
    fn an_attribute_value_is_read_in_every_declaration_form() {
        let braces = "inputs = {\n  acme-tool = {\n    url = \"github:other/thing/v1\";\n  };\n  nixpkgs.url = \"x\";\n};";
        let body = attribute_value(braces, "acme-tool").expect("a binding");
        assert!(body.contains("github:other/thing/v1") && !body.contains("nixpkgs"));
        let dotted =
            "inputs.acme-tool.url = \"github:other/thing/v1\";\ninputs.nixpkgs.url = \"n\";";
        assert!(
            attribute_value(dotted, "acme-tool")
                .expect("dotted")
                .contains("github:other/thing/v1")
        );
        let compact = "inputs={acme-tool={url=\"github:other/thing/v1\";};};";
        assert!(
            attribute_value(compact, "acme-tool")
                .expect("compact")
                .contains("github:other/thing/v1")
        );
        let quoted = "inputs = { \"acme-tool\" = { url = \"github:other/thing/v1\"; }; };";
        assert!(
            attribute_value(quoted, "acme-tool")
                .expect("quoted")
                .contains("github:other/thing/v1")
        );
        assert_eq!(
            attribute_value(braces, "tool"),
            None,
            "a suffix is not the name"
        );
        assert_eq!(
            attribute_value("packages = [ acme-tool ];", "acme-tool"),
            None,
            "a value is not a binding"
        );
    }
}
