//! SHA-256 digests in their hex text form.
//!
//! One digest type serves every record and report in the binary: the
//! user-scope skill record, and the payload identity `rk payload` prints.
//! Computing at runtime over the embedded bytes keeps `build.rs` free of
//! code generation and makes a digest necessarily equal to what the
//! binary actually carries, which a build-time table would only claim.

use std::fmt;

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

impl serde::Serialize for Digest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Digest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("'{text}' is not a 64-character hex sha256"))
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::Digest;

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
}
