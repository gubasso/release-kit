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

/// Which of a child's streams a chunk came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildStream {
    /// The child's stdout.
    Stdout,
    /// The child's stderr.
    Stderr,
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
    /// Which stream a `child_output` chunk came from.
    pub stream: Option<ChildStream>,
    /// The chunk's bytes, base64-encoded so invalid UTF-8 travels
    /// losslessly; the journal transcript keeps the raw bytes in order.
    pub data_b64: Option<String>,
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
            stream: None,
            data_b64: None,
        }
    }

    /// A `child_output` event carrying one chunk of a child's output.
    #[must_use]
    pub fn child_output(mut self, stream: ChildStream, bytes: &[u8]) -> Self {
        self.kind = EventKind::ChildOutput;
        self.stream = Some(stream);
        self.data_b64 = Some(base64(bytes));
        self
    }
}

/// Standard base64 with padding, encode only — the one direction this
/// binary needs, so no dependency earns its place for it.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (idx, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if idx <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> shift) as usize & 0x3f]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{ChildStream, Event, EventKind, base64};

    /// The `rk.events/1` schema, held by snapshot: a field rename fails
    /// here first and becomes a schema-version bump, not a silent parser
    /// break at some agent.
    #[test]
    fn the_event_schema_snapshot_holds() {
        let mut event =
            Event::opening(0, "2026-08-29T14:10:31Z".into(), "01K5NQ7X".into(), "setup");
        assert_eq!(
            serde_json::to_string(&event).expect("an event serializes"),
            r#"{"schema":"rk.events/1","seq":0,"time":"2026-08-29T14:10:31Z","run_id":"01K5NQ7X","command":"setup","type":"schema","step":null,"status":null,"reason":null,"exit_code":null,"duration_ms":null,"stream":null,"data_b64":null}"#
        );
        event.seq = 12;
        event.kind = EventKind::StepFinished;
        event.step = Some("protect-tags".into());
        event.status = Some("satisfied".into());
        event.exit_code = Some(0);
        event.duration_ms = Some(418);
        assert_eq!(
            serde_json::to_string(&event).expect("an event serializes"),
            r#"{"schema":"rk.events/1","seq":12,"time":"2026-08-29T14:10:31Z","run_id":"01K5NQ7X","command":"setup","type":"step_finished","step":"protect-tags","status":"satisfied","reason":null,"exit_code":0,"duration_ms":418,"stream":null,"data_b64":null}"#
        );
    }

    /// A child's chunk travels with its stream tag and its bytes intact —
    /// invalid UTF-8 included, which is why the payload is base64.
    #[test]
    fn a_child_output_event_carries_the_chunk_losslessly() {
        let event = Event::opening(3, "2026-08-29T14:10:31Z".into(), "01K5NQ7X".into(), "setup")
            .child_output(ChildStream::Stderr, &[0x66, 0x6f, 0x6f, 0xff, 0xfe]);
        assert_eq!(
            serde_json::to_string(&event).expect("an event serializes"),
            r#"{"schema":"rk.events/1","seq":3,"time":"2026-08-29T14:10:31Z","run_id":"01K5NQ7X","command":"setup","type":"child_output","step":null,"status":null,"reason":null,"exit_code":null,"duration_ms":null,"stream":"stderr","data_b64":"Zm9v//4="}"#
        );
    }

    /// The RFC 4648 vectors, so the hand-rolled encoder is checked against
    /// values this crate did not compute.
    #[test]
    fn the_base64_encoder_matches_the_rfc_vectors() {
        for (input, expected) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input), expected);
        }
    }
}
