//! The NDJSON event envelope for long-running commands.
//!
//! Bounded commands emit one JSON object; long-running ones emit one
//! complete event per line, opening with a [`EventKind::Schema`] event
//! that names the version, so a consumer knows what it is reading before
//! the first step event arrives. The compatibility rules are stated once
//! and held by test: a consumer ignores unknown fields and unknown event
//! types, field names are never renamed, and new event types append.

use serde::Serialize;

use crate::diagnostic::Reason;

/// The version of the event envelope's shape.
pub const EVENTS_SCHEMA: &str = "rk.events/1";

/// Every event type the stream can carry; additions append, and no
/// variant is ever renamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// The opening event naming the schema version.
    Schema,
    /// One step began.
    StepStarted,
    /// One step ended, with its status and duration.
    StepFinished,
    /// One chunk of a child process's output, tagged with its stream.
    ChildOutput,
    /// The run ended.
    RunFinished,
}

/// One event on the stream.
///
/// Every field is present on every event — absent values serialize as
/// `null` rather than disappearing — so a consumer can parse one shape.
#[derive(Debug, Serialize)]
pub struct Event {
    /// The envelope version, on every line.
    pub schema: &'static str,
    /// Monotonic sequence number within the run.
    pub seq: u64,
    /// Wall-clock UTC, RFC 3339.
    pub time: String,
    /// The run this event belongs to.
    pub run_id: String,
    /// The subcommand emitting the stream.
    pub command: &'static str,
    /// What happened.
    #[serde(rename = "type")]
    pub kind: EventKind,
    /// The step concerned, where there is one.
    pub step: Option<String>,
    /// The step's result, on a finish event.
    pub status: Option<String>,
    /// The reason, on a failure.
    pub reason: Option<Reason>,
    /// The child's exit code, where one exists.
    pub exit_code: Option<i32>,
    /// How long the step took, on a finish event.
    pub duration_ms: Option<u64>,
}

impl Event {
    /// The opening event of a stream: everything nullable is null.
    #[must_use]
    pub const fn opening(seq: u64, time: String, run_id: String, command: &'static str) -> Self {
        Self {
            schema: EVENTS_SCHEMA,
            seq,
            time,
            run_id,
            command,
            kind: EventKind::Schema,
            step: None,
            status: None,
            reason: None,
            exit_code: None,
            duration_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Event, EventKind};

    /// The `rk.events/1` schema, held by snapshot: a field rename fails
    /// here first and becomes a schema-version bump, not a silent parser
    /// break at some agent.
    #[test]
    fn the_event_schema_snapshot_holds() {
        let mut event =
            Event::opening(0, "2026-08-29T14:10:31Z".into(), "01K5NQ7X".into(), "setup");
        assert_eq!(
            serde_json::to_string(&event).expect("an event serializes"),
            r#"{"schema":"rk.events/1","seq":0,"time":"2026-08-29T14:10:31Z","run_id":"01K5NQ7X","command":"setup","type":"schema","step":null,"status":null,"reason":null,"exit_code":null,"duration_ms":null}"#
        );
        event.seq = 12;
        event.kind = EventKind::StepFinished;
        event.step = Some("protect-tags".into());
        event.status = Some("satisfied".into());
        event.exit_code = Some(0);
        event.duration_ms = Some(418);
        assert_eq!(
            serde_json::to_string(&event).expect("an event serializes"),
            r#"{"schema":"rk.events/1","seq":12,"time":"2026-08-29T14:10:31Z","run_id":"01K5NQ7X","command":"setup","type":"step_finished","step":"protect-tags","status":"satisfied","reason":null,"exit_code":0,"duration_ms":418}"#
        );
    }
}
