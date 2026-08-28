//! The agent skills: the payload, the user-scope record, and the installer.
//!
//! Skills land under the invoking user's home and never into a target
//! repository. An agent resolves a skill by name across scopes, so a second
//! copy under one name is a second entry offering the same skill, with no way
//! for the operator to tell which one runs. One installed binary already
//! serves every repository, and the skills routing into it belong at the same
//! scope.
//!
//! That places every destination outside the reach of `rk init`, which has a
//! target directory to compare against and a landing to refuse. Here there is
//! no target and no manifest, so [`record`] stands in for one: it answers the
//! single question the installer cannot otherwise answer — are these bytes
//! ones we wrote?

pub mod installer;
pub mod record;

use std::fmt;

use crate::embedded;
use crate::error::RkError;

/// One embedded skill: its directory name and its `SKILL.md` text.
#[derive(Debug)]
pub struct Skill {
    /// The directory name, which is also the skill's `name` frontmatter.
    pub name: String,
    /// The authored `SKILL.md`, byte-identical to the file under `skills/`.
    pub text: &'static str,
}

/// Every embedded skill, sorted by name.
///
/// # Errors
///
/// Returns [`RkError::Other`] when a skill directory carries no readable
/// UTF-8 `SKILL.md`. That is a defect in the payload this binary was built
/// from, not something a caller can correct.
pub fn all() -> Result<Vec<Skill>, RkError> {
    let mut out = Vec::new();
    for dir in embedded::SKILLS.dirs() {
        let name = dir.path().to_string_lossy().into_owned();
        let text = dir
            .get_file(format!("{name}/SKILL.md"))
            .and_then(include_dir::File::contents_utf8)
            .ok_or_else(|| anyhow::anyhow!("payload skill carries no UTF-8 SKILL.md: {name}"))?;
        out.push(Skill { name, text });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The lowercase hex alphabet, indexed by nibble.
const HEX: [u8; 16] = *b"0123456789abcdef";

/// A SHA-256 digest, in its 64-character lowercase hex form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Digest(String);

impl Digest {
    /// Digest a byte string.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        use sha2::Digest as _;
        let mut hex = String::with_capacity(64);
        for byte in sha2::Sha256::digest(bytes) {
            hex.push(char::from(HEX[usize::from(byte >> 4)]));
            hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Self(hex)
    }

    /// Parse a 64-character lowercase hex digest, or reject it.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        hex.then(|| Self(text.to_owned()))
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Digest, all};

    #[test]
    fn a_digest_round_trips_through_its_hex_form() {
        // The published SHA-256 of the empty string, so the hex encoding is
        // checked against a value this crate did not compute.
        let empty = Digest::of(b"");
        assert_eq!(
            empty.to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(Digest::parse(&empty.to_string()), Some(empty));
    }

    #[test]
    fn a_malformed_digest_is_rejected() {
        for text in ["", "abc", &"g".repeat(64), &"A".repeat(64), &"a".repeat(63)] {
            assert!(Digest::parse(text).is_none(), "'{text}' parsed as a digest");
        }
    }

    #[test]
    fn the_payload_carries_every_authored_skill() {
        let skills = all().expect("the embedded skills read");
        assert!(!skills.is_empty(), "the payload carries no skills");
        for skill in &skills {
            assert!(
                skill.text.contains(&format!("name: {}", skill.name)),
                "{}: the frontmatter name differs from the directory",
                skill.name
            );
        }
    }
}
