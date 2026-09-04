//! The pin matcher: the one line in a consumer's `flake.nix` that names
//! the release-kit tag.
//!
//! Pure text in, values out: no file access and no spawning, so every
//! caller — the offline observation, the preview, the transaction — reads
//! one grammar. The matcher is anchored at both ends. Without the front
//! anchor a commented example or a URL inside prose counts as a pin, and
//! the "exactly one, or refuse" rule then counts the wrong thing; without
//! the back anchor a subdirectory reference matches. The crate carries no
//! regex engine, so the matcher is hand-written.

/// The flake-input URL prefix every consumer pin begins with; the tag
/// follows it. A grammar in the same class as the branch grammar: a
/// source constant, never a payload text.
pub const PIN_PREFIX: &str = "github:gubasso/release-kit/";

/// One matched pin line, with the byte range of the tag alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// The one-based line the pin sits on.
    pub line: usize,
    /// The tag between the prefix and the closing quote.
    pub tag: String,
    /// The byte offset of the tag's first byte in the scanned text.
    pub start: usize,
    /// The byte offset one past the tag's last byte.
    pub end: usize,
}

/// What a scan of `flake.nix` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scan {
    /// No line names the release-kit input.
    None,
    /// One line names the input with no tag: a real state, reported and
    /// never rewritten. Carries the one-based line.
    Unpinned(usize),
    /// Exactly one pinned line.
    One(Pin),
    /// More than one line matches; the count is what the refusal names.
    Many(usize),
}

/// Scan a flake's text for the release-kit input line.
#[must_use]
pub fn scan(text: &str) -> Scan {
    let mut pins = Vec::new();
    let mut unpinned = None;
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        match match_line(line) {
            Some(Match::Pinned { tag, start, end }) => pins.push(Pin {
                line: index + 1,
                tag,
                start: offset + start,
                end: offset + end,
            }),
            Some(Match::Unpinned) if unpinned.is_none() => unpinned = Some(index + 1),
            Some(Match::Unpinned) | None => {}
        }
        offset += line.len();
    }
    match (pins.len(), unpinned) {
        (0, None) => Scan::None,
        (0, Some(line)) => Scan::Unpinned(line),
        (1, None) => Scan::One(pins.remove(0)),
        (n, None) => Scan::Many(n),
        (n, Some(_)) => Scan::Many(n + 1),
    }
}

/// The same text with one pin's tag replaced. Only the tag's bytes
/// change: indentation, quoting, line endings, and a trailing comment
/// survive byte for byte.
#[must_use]
pub fn rewrite(text: &str, pin: &Pin, tag: &str) -> String {
    let mut out = String::with_capacity(text.len() + tag.len());
    out.push_str(&text[..pin.start]);
    out.push_str(tag);
    out.push_str(&text[pin.end..]);
    out
}

/// The double quote, as a code point: the source scan that keeps whole
/// artifacts out of the sources reads a quote literal as a string start.
const QUOTE: char = '\u{22}';

/// One line's classification.
enum Match {
    Pinned {
        tag: String,
        start: usize,
        end: usize,
    },
    Unpinned,
}

/// Classify one line. It matches only when every anchor holds after the
/// leading whitespace: `url`, `=`, a quote, the prefix, a tag holding no
/// `/`, the closing quote, `;`, and then nothing but an optional comment.
fn match_line(line: &str) -> Option<Match> {
    let rest = line.trim_start().strip_prefix("url")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(QUOTE)?;
    let value_start = line.len() - rest.len();
    let close = rest.find(QUOTE)?;
    let value = &rest[..close];
    let after = rest[close + 1..].trim_start();
    let after = after.strip_prefix(';')?;
    let after = after.trim_start();
    if !(after.is_empty() || after.starts_with('#')) {
        return None;
    }
    let bare = PIN_PREFIX.trim_end_matches('/');
    if value == bare {
        return Some(Match::Unpinned);
    }
    let tag = value.strip_prefix(PIN_PREFIX)?;
    if tag.is_empty() || tag.contains('/') {
        return None;
    }
    let start = value_start + PIN_PREFIX.len();
    Some(Match::Pinned {
        tag: tag.to_owned(),
        start,
        end: start + tag.len(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::{PIN_PREFIX, Pin, Scan, rewrite, scan};

    fn one(text: &str) -> Pin {
        match scan(text) {
            Scan::One(pin) => pin,
            other => panic!("expected one pin, found {other:?}"),
        }
    }

    #[test]
    fn the_pin_matcher_is_anchored_at_both_ends() {
        let pinned = format!("  url = \"{PIN_PREFIX}v0.2.16\";\n");
        assert_eq!(one(&pinned).tag, "v0.2.16");
        let commented = format!("  # url = \"{PIN_PREFIX}v0.2.16\";\n");
        assert_eq!(scan(&commented), Scan::None, "a comment line is not a pin");
        let follows = "  inputs.nixpkgs.follows = \"nixpkgs\";\n";
        assert_eq!(scan(follows), Scan::None, "a follows line is not a pin");
        let subdir = format!("  url = \"{PIN_PREFIX}v1/subdir\";\n");
        assert_eq!(
            scan(&subdir),
            Scan::None,
            "a subdirectory reference is not a pin"
        );
        let longer_owner = format!("  url = \"github:other-{}v0.2.16\";\n", &PIN_PREFIX[7..]);
        assert_eq!(
            scan(&longer_owner),
            Scan::None,
            "a longer owner is not a pin"
        );
        let prose = format!("  description = \"see {PIN_PREFIX}v0.2.16\";\n");
        assert_eq!(scan(&prose), Scan::None, "a URL inside prose is not a pin");
        let trailing = format!("  url = \"{PIN_PREFIX}v0.2.16\"; # the version\n");
        assert_eq!(
            one(&trailing).tag,
            "v0.2.16",
            "a trailing comment is allowed"
        );
        let no_semicolon = format!("  url = \"{PIN_PREFIX}v0.2.16\"\n");
        assert_eq!(
            scan(&no_semicolon),
            Scan::None,
            "the back anchor is the semicolon"
        );
    }

    #[test]
    fn an_unpinned_url_is_reported_not_rewritten() {
        let text = format!(
            "inputs = {{\n  release-kit.url = \"x\";\n  url = \"{}\";\n}}\n",
            PIN_PREFIX.trim_end_matches('/')
        );
        assert_eq!(scan(&text), Scan::Unpinned(3));
    }

    #[test]
    fn the_rewrite_changes_only_the_tag_substring() {
        let text = format!("{{\r\n\turl =\t\"{PIN_PREFIX}v0.2.15\";   # keep\r\n}}\r\n");
        let pin = one(&text);
        let rewritten = rewrite(&text, &pin, "v0.2.16");
        assert_eq!(rewritten, text.replace("v0.2.15", "v0.2.16"));
        assert_eq!(one(&rewritten).tag, "v0.2.16");
        assert_eq!(pin.line, 2);
    }

    #[test]
    fn two_pin_lines_count_as_two() {
        let text = format!("url = \"{PIN_PREFIX}v1.0.0\";\nurl = \"{PIN_PREFIX}v2.0.0\";\n");
        assert_eq!(scan(&text), Scan::Many(2));
        let mixed = format!(
            "url = \"{PIN_PREFIX}v1.0.0\";\nurl = \"{}\";\n",
            PIN_PREFIX.trim_end_matches('/')
        );
        assert_eq!(
            scan(&mixed),
            Scan::Many(2),
            "an unpinned line beside a pin is ambiguity"
        );
    }
}
