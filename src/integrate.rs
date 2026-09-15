//! The pure half of `rk integrate`: the transaction's judgments and the
//! evidence a local integration leaves behind.
//!
//! This module spawns nothing. It decides what a branch name, a seat, a
//! pair of observed tips, and a stored ledger permit, and it renders and
//! parses the ledger. `crate::commands::integrate` is the half that runs
//! git and `pre-commit`.
//!
//! The evidence is clone-local, under the common git directory, because a
//! local integration is a fact about one checkout: the branch it names
//! exists in that clone alone, the prune verbs that read it act in that
//! clone alone, and a committed file would carry a per-checkout fact into
//! every other clone. The common git directory rather than a worktree's
//! own is what makes one ledger serve every linked seat.
//!
//! SATISFIES git:a-local-integration-is-a-transaction

use serde::{Deserialize, Serialize};

/// The ledger's path below the common git directory.
pub const LEDGER_PATH: &str = "rk/integrations.json";

/// The ledger's shape version.
const LEDGER_SCHEMA: &str = "rk.integrations/1";

/// One local integration, as the prune verbs read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The branch that was integrated.
    pub branch: String,
    /// The branch's tip at the moment it was integrated. A prune reads
    /// this: a branch that took another commit afterwards no longer
    /// matches, and its work is not on the trunk.
    pub branch_tip: String,
    /// The squash commit this integration wrote onto the trunk.
    pub trunk_commit: String,
    /// When the integration completed, UTC.
    pub at: String,
}

/// Every local integration this clone recorded, newest last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// The shape version this document declares.
    #[serde(default = "ledger_schema")]
    pub schema: String,
    /// One entry per branch; a re-integration replaces its predecessor.
    #[serde(default)]
    pub entries: Vec<Entry>,
}

fn ledger_schema() -> String {
    LEDGER_SCHEMA.to_owned()
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            schema: ledger_schema(),
            entries: Vec::new(),
        }
    }
}

impl Ledger {
    /// Parse a stored ledger.
    ///
    /// An absent file is an empty ledger, because a clone that never
    /// integrated locally has recorded nothing. Malformed content refuses
    /// rather than reading as empty: an empty ledger authorizes no
    /// deletion, so a silent downgrade would be safe, but it would also
    /// hide a defect that loses real evidence.
    ///
    /// # Errors
    ///
    /// The detail of what could not be read: invalid JSON, or a schema
    /// this binary does not know.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let ledger: Self = serde_json::from_str(text)
            .map_err(|source| format!("the integration ledger is not readable: {source}"))?;
        if ledger.schema != LEDGER_SCHEMA {
            return Err(format!(
                "the integration ledger declares schema {}, and this binary knows {LEDGER_SCHEMA}",
                ledger.schema
            ));
        }
        Ok(ledger)
    }

    /// The stored form, one trailing newline.
    ///
    /// # Errors
    ///
    /// A serialization defect in this binary.
    pub fn render(&self) -> Result<String, String> {
        let mut text = serde_json::to_string_pretty(self)
            .map_err(|source| format!("the integration ledger does not serialize: {source}"))?;
        text.push('\n');
        Ok(text)
    }

    /// Record one integration, replacing any earlier entry for the same
    /// branch: a branch name is reused, and the ledger answers for the
    /// branch standing now.
    pub fn record(&mut self, entry: Entry) {
        self.entries.retain(|held| held.branch != entry.branch);
        self.entries.push(entry);
    }

    /// The proof this ledger offers for one branch at one observed tip.
    ///
    /// The tip must match exactly, re-observed by the caller at the
    /// moment of action. A branch that advanced after its integration has
    /// work the trunk does not carry, so it is not a candidate at all.
    #[must_use]
    pub fn proof(&self, branch: &str, tip: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.branch == branch && entry.branch_tip == tip)
    }

    /// Whether this ledger holds any entry for one branch, at whatever
    /// tip. The prune reports use it to tell a branch that was never
    /// integrated from one that advanced after it was.
    #[must_use]
    pub fn names(&self, branch: &str) -> bool {
        self.entries.iter().any(|entry| entry.branch == branch)
    }

    /// Forget one branch's entry, after the branch itself is gone.
    pub fn forget(&mut self, branch: &str) {
        self.entries.retain(|entry| entry.branch != branch);
    }
}

/// Why a branch name cannot be integrated, or `None` where it can.
///
/// The trunk is refused before the grammar, because the trunk is the
/// destination and naming it is a different mistake from naming something
/// off the grammar. The grammar is the one owner in
/// [`crate::projection::BRANCH_GRAMMAR`], through
/// [`crate::worktree::matches_grammar`], so the desk, the landed hook,
/// and this verb judge one language.
#[must_use]
pub fn refuse_branch_name(branch: &str, trunk: &str) -> Option<String> {
    if branch == trunk {
        return Some(format!(
            "{branch} is the trunk; integration moves a short-lived branch onto it"
        ));
    }
    if !crate::worktree::matches_grammar(branch) {
        return Some(format!(
            "{branch} is neither <type>/<slug> nor <issue-id>-<slug>, so no landed hook would admit its commits"
        ));
    }
    None
}

/// How the local trunk stands against its remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrunkState {
    /// No remote answered, or the two name one commit.
    Level,
    /// The remote is ahead: the local trunk fast-forwards onto it.
    Behind,
    /// The local trunk carries integrations nobody pushed yet, which is
    /// the ordinary state under local integration.
    Ahead,
    /// Neither reaches the other.
    Diverged,
}

/// Where the local trunk stands against the remote tip, if any.
#[must_use]
pub const fn trunk_state(
    level: bool,
    local_reaches_remote: bool,
    remote_reaches_local: bool,
) -> TrunkState {
    if level {
        TrunkState::Level
    } else if local_reaches_remote {
        TrunkState::Behind
    } else if remote_reaches_local {
        TrunkState::Ahead
    } else {
        TrunkState::Diverged
    }
}

/// Why a diverged trunk cannot be integrated onto.
///
/// Only divergence refuses. A trunk behind its remote fast-forwards, and
/// a trunk ahead of it is the ordinary state under local integration:
/// integrations accumulate and the operator pushes when they decide to.
/// A divergence is never merged here, because choosing between the fetch
/// and the push is the operator's and a merge would put a second parent
/// on a trunk this convention keeps linear.
#[must_use]
pub fn refuse_trunk_state(state: TrunkState) -> Option<String> {
    matches!(state, TrunkState::Diverged).then(|| {
        "the local trunk and its remote diverged; neither reaches the other, so this command \
         refuses rather than merging them"
            .to_owned()
    })
}

/// Why the gate's verdict blocks the integration, or `None`.
#[must_use]
pub fn refuse_moved_trunk(before: &str, now: &str) -> Option<String> {
    (before != now).then(|| {
        format!(
            "the trunk moved from {} to {} while the gate ran, so the gate judged a trunk that is gone",
            short(before),
            short(now)
        )
    })
}

/// The seven-character form a report shows.
#[must_use]
pub fn short(oid: &str) -> String {
    oid.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::{Entry, Ledger, refuse_branch_name, refuse_moved_trunk, refuse_trunk_state};

    fn entry(branch: &str, tip: &str) -> Entry {
        Entry {
            branch: branch.to_owned(),
            branch_tip: tip.to_owned(),
            trunk_commit: "c".repeat(40),
            at: "2026-09-15T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn an_absent_ledger_reads_as_empty_and_proves_nothing() {
        let ledger = Ledger::parse("").expect("absence is empty");
        assert!(ledger.entries.is_empty());
        assert_eq!(ledger.proof("feat/x", &"a".repeat(40)), None);
        assert!(!ledger.names("feat/x"));
    }

    #[test]
    fn a_ledger_round_trips_and_refuses_an_unknown_schema() {
        let mut ledger = Ledger::default();
        ledger.record(entry("feat/x", &"a".repeat(40)));
        let text = ledger.render().expect("it serializes");
        assert_eq!(Ledger::parse(&text).expect("it reads back"), ledger);
        let error = Ledger::parse(r#"{"schema":"rk.integrations/99","entries":[]}"#)
            .expect_err("a newer schema refuses");
        assert!(error.contains("rk.integrations/1"), "{error}");
        let error = Ledger::parse("{").expect_err("malformed content refuses");
        assert!(error.contains("not readable"), "{error}");
    }

    #[test]
    fn a_proof_needs_the_tip_the_integration_recorded() {
        let mut ledger = Ledger::default();
        let tip = "a".repeat(40);
        ledger.record(entry("feat/x", &tip));
        assert!(ledger.proof("feat/x", &tip).is_some());
        // One more commit after the integration: the work is not on the
        // trunk, so the branch is not a candidate at all.
        assert_eq!(ledger.proof("feat/x", &"b".repeat(40)), None);
        assert!(ledger.names("feat/x"));
        // Evidence for one branch never confirms another.
        assert_eq!(ledger.proof("feat/y", &tip), None);
    }

    #[test]
    fn a_re_integration_replaces_its_predecessor() {
        let mut ledger = Ledger::default();
        ledger.record(entry("feat/x", &"a".repeat(40)));
        ledger.record(entry("feat/x", &"b".repeat(40)));
        assert_eq!(ledger.entries.len(), 1);
        assert!(ledger.proof("feat/x", &"b".repeat(40)).is_some());
        ledger.forget("feat/x");
        assert!(ledger.entries.is_empty());
    }

    #[test]
    fn the_trunk_and_a_misshapen_branch_each_refuse_by_name() {
        assert!(
            refuse_branch_name("master", "master")
                .expect("the trunk refuses")
                .contains("trunk")
        );
        let error = refuse_branch_name("wip", "master").expect("the grammar refuses");
        assert!(error.contains("<type>/<slug>"), "{error}");
        assert_eq!(refuse_branch_name("feat/x", "master"), None);
        assert_eq!(refuse_branch_name("123-slug", "master"), None);
    }

    #[test]
    fn only_a_diverged_trunk_refuses() {
        use super::{TrunkState, trunk_state};
        // level, behind, and ahead all proceed; a trunk ahead of its
        // remote is the ordinary state under local integration.
        assert_eq!(trunk_state(true, false, false), TrunkState::Level);
        assert_eq!(trunk_state(false, true, false), TrunkState::Behind);
        assert_eq!(trunk_state(false, false, true), TrunkState::Ahead);
        assert_eq!(trunk_state(false, false, false), TrunkState::Diverged);
        for state in [TrunkState::Level, TrunkState::Behind, TrunkState::Ahead] {
            assert_eq!(refuse_trunk_state(state), None, "{state:?}");
        }
        let error = refuse_trunk_state(TrunkState::Diverged).expect("divergence refuses");
        assert!(error.contains("refuses rather than merging"), "{error}");
    }

    #[test]
    fn a_trunk_that_moved_under_the_gate_refuses() {
        assert_eq!(refuse_moved_trunk("a", "a"), None);
        let error =
            refuse_moved_trunk(&"a".repeat(40), &"b".repeat(40)).expect("a moved trunk refuses");
        assert!(error.contains("aaaaaaa"), "{error}");
        assert!(error.contains("bbbbbbb"), "{error}");
    }
}
