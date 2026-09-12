//! The evidence ledger: every observed value, with who observed it, when,
//! how, and what it digested to.
//!
//! Provenance attaches to claims and not to the document: a field in the
//! plan that depends on the record, the disk, the bundle, and a fetch at
//! once cites each one through `evidence_refs`, rather than the plan
//! carrying one stamp that describes none of them.

use serde::Serialize;

use crate::digest::Digest;

/// What class of thing an evidence item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    /// The landing record at the target.
    Record,
    /// The committed configuration at the target.
    Configuration,
    /// One destination's bytes at the target.
    Destination,
    /// The repository's own state: git, markers, the version file.
    Repository,
    /// A release bundle, read through the seam.
    Bundle,
    /// The engine and the host it runs on.
    Host,
    /// The pin a tool manager records for `rk`.
    Pin,
    /// A fact read from the forge.
    Forge,
}

/// One observed value.
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceItem {
    /// A stable id other fields cite.
    pub id: String,
    /// What class of thing was observed.
    pub kind: EvidenceKind,
    /// What produced the observation: the engine's own reader, a git
    /// call, a source name.
    pub producer: String,
    /// When it was observed, RFC 3339.
    pub observed_at: String,
    /// The digest of what was observed, where the observation is bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<Digest>,
    /// How it was collected, one line.
    pub method: String,
}

/// The ledger under construction: items appended in observation order,
/// each id unique.
#[derive(Debug, Default)]
pub struct Ledger {
    items: Vec<EvidenceItem>,
}

impl Ledger {
    /// An empty ledger.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Record one observation and answer its id, for the field that
    /// cites it.
    pub fn observe(
        &mut self,
        id: impl Into<String>,
        kind: EvidenceKind,
        producer: impl Into<String>,
        observed_at: &str,
        sha256: Option<Digest>,
        method: impl Into<String>,
    ) -> String {
        let id = id.into();
        debug_assert!(
            !self.items.iter().any(|item| item.id == id),
            "evidence id {id} is already in the ledger"
        );
        self.items.push(EvidenceItem {
            id: id.clone(),
            kind,
            producer: producer.into(),
            observed_at: observed_at.to_owned(),
            sha256,
            method: method.into(),
        });
        id
    }

    /// The items, in observation order.
    #[must_use]
    pub fn into_items(self) -> Vec<EvidenceItem> {
        self.items
    }

    /// Whether the ledger carries `id`.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.items.iter().any(|item| item.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::{EvidenceKind, Ledger};
    use crate::digest::Digest;

    #[test]
    fn an_observation_answers_the_id_a_field_cites() {
        let mut ledger = Ledger::new();
        let id = ledger.observe(
            "record",
            EvidenceKind::Record,
            "rk",
            "2026-01-01T00:00:00Z",
            Some(Digest::of(b"{}")),
            "read .release-kit/manifest.json",
        );
        assert_eq!(id, "record");
        assert!(ledger.has("record"));
        let items = ledger.into_items();
        assert_eq!(items.len(), 1);
        assert_eq!(
            serde_json::to_string(&items[0]).expect("serializes"),
            format!(
                r#"{{"id":"record","kind":"record","producer":"rk","observed_at":"2026-01-01T00:00:00Z","sha256":"{}","method":"read .release-kit/manifest.json"}}"#,
                Digest::of(b"{}")
            )
        );
    }
}
