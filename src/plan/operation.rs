//! The closed set of things an apply may do.
//!
//! Every operation names digests and never bytes: the bytes live in the
//! plan's blob store by digest, so the JSON view stays bounded and an
//! apply reads exactly the bytes the plan was approved with. Every
//! operation with a `before` names the digest the target must still hold
//! at apply time. There is no operation that runs a command, and there
//! will not be one.

use serde::{Deserialize, Serialize};

use crate::digest::Digest;
use crate::landing::Kind;

/// One typed change an apply makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Operation {
    /// Write a whole file at `path`.
    WriteFile {
        /// The destination, relative to the target.
        path: String,
        /// The declared ownership kind of the destination.
        kind: Kind,
        /// The digest the destination holds now, or absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        before: Option<Digest>,
        /// The digest of the bytes written.
        after: Digest,
    },
    /// Splice a marked block into the document at `path`.
    SpliceBlock {
        /// The document, relative to the target.
        path: String,
        /// The begin marker that names the block.
        marker: String,
        /// The digest of the block the document holds now, or absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        before: Option<Digest>,
        /// The digest of the block written.
        after: Digest,
    },
    /// Remove a file release-kit owns.
    RemoveOwnedFile {
        /// The destination, relative to the target.
        path: String,
        /// The digest the destination must still hold.
        before: Digest,
    },
    /// Write the landing record, last.
    WriteRecord {
        /// The digest of the record now, or absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        before: Option<Digest>,
        /// The digest of the record written.
        after: Digest,
    },
    /// Move the `rk` pin through the wired manager.
    UpdatePin {
        /// The manager that records the pin.
        manager: String,
        /// The version the manager records now.
        before: String,
        /// The version the manager records after.
        after: String,
    },
}

impl Operation {
    /// The kind word, for summaries and the fingerprint.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::WriteFile { .. } => "write-file",
            Self::SpliceBlock { .. } => "splice-block",
            Self::RemoveOwnedFile { .. } => "remove-owned-file",
            Self::WriteRecord { .. } => "write-record",
            Self::UpdatePin { .. } => "update-pin",
        }
    }

    /// The path the operation touches, where it touches one.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::WriteFile { path, .. }
            | Self::SpliceBlock { path, .. }
            | Self::RemoveOwnedFile { path, .. } => Some(path),
            Self::WriteRecord { .. } | Self::UpdatePin { .. } => None,
        }
    }

    /// The canonical line the fingerprint hashes: the kind, the path, the
    /// declared kind where there is one, and the two digests.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Self::WriteFile {
                path,
                kind,
                before,
                after,
            } => format!(
                "write-file\t{path}\t{}\t{}\t{after}",
                kind.as_str(),
                digest_or_absent(before.as_ref())
            ),
            Self::SpliceBlock {
                path,
                marker,
                before,
                after,
            } => format!(
                "splice-block\t{path}\t{marker}\t{}\t{after}",
                digest_or_absent(before.as_ref())
            ),
            Self::RemoveOwnedFile { path, before } => {
                format!("remove-owned-file\t{path}\t{before}")
            }
            Self::WriteRecord { before, after } => {
                format!(
                    "write-record\t{}\t{after}",
                    digest_or_absent(before.as_ref())
                )
            }
            Self::UpdatePin {
                manager,
                before,
                after,
            } => format!("update-pin\t{manager}\t{before}\t{after}"),
        }
    }
}

/// The word an absent digest takes in a canonical line.
fn digest_or_absent(digest: Option<&Digest>) -> String {
    digest.map_or_else(|| "absent".to_owned(), ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::Operation;
    use crate::digest::Digest;
    use crate::landing::Kind;

    /// The wire shape of each variant, held by snapshot; the tag word is
    /// what an agent branches on.
    #[test]
    fn every_operation_serializes_under_its_tag() {
        let a = Digest::of(b"a");
        let b = Digest::of(b"b");
        let cases = [
            (
                Operation::WriteFile {
                    path: "SECURITY.md".into(),
                    kind: Kind::Rendered,
                    before: None,
                    after: b.clone(),
                },
                format!(
                    r#"{{"op":"write-file","path":"SECURITY.md","kind":"rendered","after":"{b}"}}"#
                ),
            ),
            (
                Operation::SpliceBlock {
                    path: "AGENTS.md".into(),
                    marker: "<!-- BEGIN release-kit -->".into(),
                    before: Some(a.clone()),
                    after: b.clone(),
                },
                format!(
                    r#"{{"op":"splice-block","path":"AGENTS.md","marker":"<!-- BEGIN release-kit -->","before":"{a}","after":"{b}"}}"#
                ),
            ),
            (
                Operation::RemoveOwnedFile {
                    path: "old.yml".into(),
                    before: a.clone(),
                },
                format!(r#"{{"op":"remove-owned-file","path":"old.yml","before":"{a}"}}"#),
            ),
            (
                Operation::WriteRecord {
                    before: Some(a.clone()),
                    after: b.clone(),
                },
                format!(r#"{{"op":"write-record","before":"{a}","after":"{b}"}}"#),
            ),
            (
                Operation::UpdatePin {
                    manager: "mise".into(),
                    before: "0.1.0".into(),
                    after: "0.2.0".into(),
                },
                r#"{"op":"update-pin","manager":"mise","before":"0.1.0","after":"0.2.0"}"#
                    .to_owned(),
            ),
        ];
        for (operation, expected) in cases {
            assert_eq!(
                serde_json::to_string(&operation).expect("an operation serializes"),
                expected
            );
            assert!(operation.canonical().starts_with(operation.kind()));
        }
    }
}
