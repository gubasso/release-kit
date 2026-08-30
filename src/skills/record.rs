//! The user-scope skill record: what this tool last wrote outside a project.
//!
//! Skill destinations live under the invoking user's home, where no target
//! repository reaches, so without a record the installer's only reference is
//! the payload it currently carries. That makes a copy an older release wrote
//! indistinguishable from a file the user edited, and every release touching a
//! skill then refuses on destinations nobody touched. The record closes that
//! gap and nothing else: one digest per destination, written after a
//! successful apply, read to answer one question — are these bytes ones we
//! wrote?
//!
//! It is state, not a manifest. Nothing verifies against it, and every
//! unreadable shape resolves to an empty record, so a lost one costs only the
//! benefit of the doubt. The format is the one `sha256sum` prints, under a
//! version line: a file that cheap to lose earns no parser that can fail in
//! more than one way.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use camino::{Utf8Path, Utf8PathBuf};

use crate::skills::Digest;

/// Where the record sits, relative to the home directory.
///
/// Home-relative rather than `XDG_STATE_HOME`-relative on purpose: the
/// destinations it speaks for are `$HOME/.claude` and `$HOME/.agents`, which
/// no XDG variable moves. A record reachable under a different home than the
/// roots it vouches for would be worse than no record at all.
pub const RECORD_PATH: &str = ".local/state/release-kit/skills.sha256";

/// The first line of a record this binary understands.
const HEADER: &str = "# release-kit skill record v1";

/// The separator `sha256sum` writes between a digest and its path.
const SEPARATOR: &str = "  ";

/// The digests this tool last wrote to user-scope skill destinations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Record {
    /// Destination path to the digest written there.
    pub written: BTreeMap<Utf8PathBuf, Digest>,
}

impl Record {
    /// Read the record at `path`, or an empty one.
    ///
    /// Every failure resolves to an empty record: absent, unreadable,
    /// malformed, and written under a header this binary does not know all
    /// mean the same thing to a caller — nothing here can vouch for a
    /// destination. Refusing instead would let a corrupt state file block an
    /// install that has a `--force` it does not need.
    #[must_use]
    pub fn load(path: &Utf8Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    /// Parse a record body, or reject it whole.
    fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        if lines.next()? != HEADER {
            return None;
        }
        let mut written = BTreeMap::new();
        for line in lines.filter(|line| !line.is_empty()) {
            // Split at the digest's fixed width rather than on the first
            // separator, so a destination path holding two spaces still
            // round-trips.
            let (digest, rest) = line.split_at_checked(64)?;
            let path = rest.strip_prefix(SEPARATOR)?;
            written.insert(Utf8PathBuf::from(path), Digest::parse(digest)?);
        }
        Some(Self { written })
    }

    /// Whether this record says it wrote `digest` at `destination`.
    #[must_use]
    pub fn wrote(&self, destination: &Utf8Path, digest: &Digest) -> bool {
        self.written.get(destination) == Some(digest)
    }

    /// Serialize in the `sha256sum` shape, under the version header.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = format!("{HEADER}\n");
        for (destination, digest) in &self.written {
            // Writing into a String cannot fail; the result is discarded so
            // no caller has to handle an error that cannot happen.
            let _ = writeln!(text, "{digest}{SEPARATOR}{destination}");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use camino::Utf8PathBuf;

    use super::{HEADER, Record};
    use crate::skills::Digest;

    fn record() -> Record {
        let mut record = Record::default();
        record.written.insert(
            Utf8PathBuf::from("/home/<user>/.claude/skills/rk-setup/SKILL.md"),
            Digest::of(b"one"),
        );
        record.written.insert(
            // Two spaces in the path: the separator must not decide the split.
            Utf8PathBuf::from("/home/<user>/two  spaces/SKILL.md"),
            Digest::of(b"two"),
        );
        record
    }

    #[test]
    fn a_record_round_trips_through_its_text_form() {
        let original = record();
        let parsed = Record::parse(&original.to_text()).expect("the record parses");
        assert_eq!(parsed, original);
    }

    #[test]
    fn a_record_vouches_only_for_the_digest_it_holds() {
        let record = record();
        let destination = Utf8PathBuf::from("/home/<user>/.claude/skills/rk-setup/SKILL.md");
        assert!(record.wrote(&destination, &Digest::of(b"one")));
        assert!(!record.wrote(&destination, &Digest::of(b"edited")));
        assert!(!record.wrote(
            Utf8PathBuf::from("/elsewhere").as_path(),
            &Digest::of(b"one")
        ));
    }

    #[test]
    fn every_unreadable_shape_resolves_to_an_empty_record() {
        let good = record().to_text();
        for text in [
            String::new(),
            "not a header\n".to_string(),
            "# release-kit skill record v2\n".to_string(),
            good.replace(HEADER, "# release-kit skill record v0"),
            // A truncated digest, and a line missing its separator.
            format!("{HEADER}\nabc  /path\n"),
            format!("{HEADER}\n{} /path\n", Digest::of(b"one")),
        ] {
            assert_eq!(
                Record::parse(&text),
                None,
                "'{text}' parsed as a valid record"
            );
        }
    }

    #[test]
    fn a_missing_record_loads_empty() {
        assert_eq!(
            Record::load(Utf8PathBuf::from("/no/such/record").as_path()),
            Record::default()
        );
    }
}
