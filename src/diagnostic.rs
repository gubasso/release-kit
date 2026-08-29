//! Diagnostics as data: the closed reason vocabulary and the typed error
//! report.
//!
//! An error is a value with named parts, rendered at the boundary — never
//! a pre-formatted string assembled where the failure happened. The exit
//! code stays the coarse machine signal; the `reason` beside it is the
//! fine one, drawn from one closed vocabulary agents can branch on the
//! way scripts branch on exit codes. The vocabulary is append-only: a
//! reason is never renamed and never reused.

use serde::Serialize;

/// The version of the JSON diagnostic's shape.
pub const DIAGNOSTIC_SCHEMA: &str = "rk.diagnostic/1";

/// The closed reason vocabulary, the machine twin of the exit-code matrix.
///
/// Many reasons map to one exit code — that is the point: the code carries
/// the category, the reason carries the instance. Classification is honest
/// or absent: a failure nothing can classify further stays [`Reason::Io`]
/// or [`Reason::Internal`] rather than guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// Semantically invalid arguments or names.
    Usage,
    /// The named target does not exist or is not usable as a target.
    TargetNotFound,
    /// No forge could be detected from the repository's remote.
    ForgeUndetected,
    /// The detected or named forge is not one the binary supports.
    ForgeUnsupported,
    /// A prerequisite probe failed before any side effect.
    PrerequisiteUnmet,
    /// The forge CLI is present and not authenticated.
    ForgeAuthentication,
    /// The forge refused the caller's permissions.
    ForgePermission,
    /// The forge rate-limited the call.
    ForgeRateLimit,
    /// The forge failed in a way a retry can cure.
    ForgeTemporary,
    /// The remote state changed under the run.
    RemoteConflict,
    /// The command refused an action it judged destructive.
    DestructiveRefusal,
    /// The state found differs from what would permit the action.
    StateDrift,
    /// A record or document declares a schema this binary does not know.
    UnsupportedSchema,
    /// A mutating run could not create its journal.
    JournalUnavailable,
    /// A child process could not be spawned.
    SubprocessSpawn,
    /// A child process ran and failed without a finer classification.
    SubprocessFailed,
    /// Filesystem failure.
    Io,
    /// A defect in this binary.
    Internal,
}

/// Every reason, in declaration order; a test asserts against this so an
/// addition is deliberate and a rename impossible.
pub const REASONS: [Reason; 18] = [
    Reason::Usage,
    Reason::TargetNotFound,
    Reason::ForgeUndetected,
    Reason::ForgeUnsupported,
    Reason::PrerequisiteUnmet,
    Reason::ForgeAuthentication,
    Reason::ForgePermission,
    Reason::ForgeRateLimit,
    Reason::ForgeTemporary,
    Reason::RemoteConflict,
    Reason::DestructiveRefusal,
    Reason::StateDrift,
    Reason::UnsupportedSchema,
    Reason::JournalUnavailable,
    Reason::SubprocessSpawn,
    Reason::SubprocessFailed,
    Reason::Io,
    Reason::Internal,
];

impl Reason {
    /// The kebab-case wire form, identical to the serde rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::TargetNotFound => "target-not-found",
            Self::ForgeUndetected => "forge-undetected",
            Self::ForgeUnsupported => "forge-unsupported",
            Self::PrerequisiteUnmet => "prerequisite-unmet",
            Self::ForgeAuthentication => "forge-authentication",
            Self::ForgePermission => "forge-permission",
            Self::ForgeRateLimit => "forge-rate-limit",
            Self::ForgeTemporary => "forge-temporary",
            Self::RemoteConflict => "remote-conflict",
            Self::DestructiveRefusal => "destructive-refusal",
            Self::StateDrift => "state-drift",
            Self::UnsupportedSchema => "unsupported-schema",
            Self::JournalUnavailable => "journal-unavailable",
            Self::SubprocessSpawn => "subprocess-spawn",
            Self::SubprocessFailed => "subprocess-failed",
            Self::Io => "io",
            Self::Internal => "internal",
        }
    }
}

/// One failure, with its parts named.
///
/// A hint that is not known is omitted rather than invented, so every
/// optional field serializes only when present.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// The shape version of the JSON rendering.
    pub schema: &'static str,
    /// One entry from the closed vocabulary.
    pub reason: Reason,
    /// What happened, one line.
    pub message: String,
    /// What would have had to be true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// The exact command or change that fixes it, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Whether rerunning as-is can succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<bool>,
    /// What the run left behind, stated plainly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_state: Option<String>,
    /// The step it happened in, where there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
}

impl Diagnostic {
    /// A diagnostic carrying only its reason and message; the builder
    /// methods add what is actually known.
    #[must_use]
    pub fn new(reason: Reason, message: impl Into<String>) -> Self {
        Self {
            schema: DIAGNOSTIC_SCHEMA,
            reason,
            message: message.into(),
            expected: None,
            action: None,
            retry: None,
            target_state: None,
            step: None,
        }
    }

    /// State what would have had to be true.
    #[must_use]
    pub fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Name the command or change that fixes it.
    #[must_use]
    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// State what the run left behind.
    #[must_use]
    pub fn target_state(mut self, state: impl Into<String>) -> Self {
        self.target_state = Some(state.into());
        self
    }

    /// The human rendering: the five questions in order — what happened,
    /// what was expected, what to do, how to resume, what state the target
    /// is in — with unknown lines absent rather than invented.
    #[must_use]
    pub fn render_human(&self) -> String {
        use std::fmt::Write as _;
        let mut text = format!("error: {}", self.message);
        if let Some(expected) = &self.expected {
            let _ = write!(text, "\n  expected  {expected}");
        }
        if let Some(action) = &self.action {
            let _ = write!(text, "\n  next      {action}");
        }
        if let Some(retry) = self.retry {
            let answer = if retry {
                "rerunning as-is can succeed"
            } else {
                "rerunning as-is fails the same way"
            };
            let _ = write!(text, "\n  retry     {answer}");
        }
        if let Some(state) = &self.target_state {
            let _ = write!(text, "\n  state     {state}");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Diagnostic, REASONS, Reason};

    /// The vocabulary is closed and append-only: this list is the one a
    /// deliberate addition extends, and a rename fails here first.
    #[test]
    fn the_reason_vocabulary_is_closed() {
        let wire: Vec<&str> = REASONS.iter().map(|reason| reason.as_str()).collect();
        assert_eq!(
            wire,
            [
                "usage",
                "target-not-found",
                "forge-undetected",
                "forge-unsupported",
                "prerequisite-unmet",
                "forge-authentication",
                "forge-permission",
                "forge-rate-limit",
                "forge-temporary",
                "remote-conflict",
                "destructive-refusal",
                "state-drift",
                "unsupported-schema",
                "journal-unavailable",
                "subprocess-spawn",
                "subprocess-failed",
                "io",
                "internal",
            ]
        );
    }

    #[test]
    fn the_serde_rendering_and_the_wire_form_agree() {
        for reason in REASONS {
            let json = serde_json::to_string(&reason).expect("a reason serializes");
            assert_eq!(json, format!("\"{}\"", reason.as_str()));
        }
    }

    /// The `rk.diagnostic/1` schema, held by snapshot: a field rename or
    /// removal fails here and becomes a schema-version bump instead of a
    /// silent parser break at some agent.
    #[test]
    fn the_diagnostic_schema_snapshot_holds() {
        let full = Diagnostic::new(Reason::StateDrift, "what happened")
            .expected("what would have had to be true")
            .action("the command that fixes it")
            .target_state("what the run left behind");
        assert_eq!(
            serde_json::to_string(&full).expect("a diagnostic serializes"),
            r#"{"schema":"rk.diagnostic/1","reason":"state-drift","message":"what happened","expected":"what would have had to be true","action":"the command that fixes it","target_state":"what the run left behind"}"#
        );
        let bare = Diagnostic::new(Reason::Io, "disk fell over");
        assert_eq!(
            serde_json::to_string(&bare).expect("a diagnostic serializes"),
            r#"{"schema":"rk.diagnostic/1","reason":"io","message":"disk fell over"}"#,
            "an unknown hint must be omitted, not serialized as null"
        );
    }

    #[test]
    fn the_human_rendering_answers_only_what_is_known() {
        let bare = Diagnostic::new(Reason::Io, "disk fell over");
        assert_eq!(bare.render_human(), "error: disk fell over");
        let full = Diagnostic::new(Reason::StateDrift, "the target drifted")
            .expected("a clean target")
            .action("rk init --apply")
            .target_state("nothing was written");
        assert_eq!(
            full.render_human(),
            "error: the target drifted\n  expected  a clean target\n  next      rk init --apply\n  state     nothing was written"
        );
    }
}
