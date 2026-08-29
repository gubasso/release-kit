//! `rk versions`: the pinned-tool registry, and its freshness check.
//!
//! Plain `rk versions` prints the registry exactly as authored, offline.
//! `--check` is the canon-side freshness answer and the one verb allowed
//! to fetch: it consults each pin's check URL and reports per pin, where
//! an unreachable or unparsable source is a reported result at exit 0,
//! not an error — and it never edits `versions.toml`, because a pin
//! update is a reviewed change in this repository. The fetch goes through
//! `curl`, resolved like the forge CLIs with `RK_CURL_BIN` as the
//! override, so the check needs no HTTP stack of its own and a test can
//! substitute the network.

use serde::Serialize;

use crate::cli::versions::VersionsArgs;
use crate::error::RkError;
use crate::output::Output;
use crate::{embedded, registry};

/// One pin's check result.
#[derive(Debug, Serialize)]
struct PinResult {
    /// The tool's registry name.
    tool: String,
    /// The pinned version.
    pinned: String,
    /// `current`, `update-available`, `source-unreachable`, or
    /// `source-unparsable`.
    result: &'static str,
    /// The version the source serves, where one was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    available: Option<String>,
}

/// The machine form of a check report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// One result per pin, in registry order.
    pins: Vec<PinResult>,
}

/// Print the registry, or check each pin upstream under `--check`.
///
/// # Errors
///
/// Returns [`RkError::Other`] only when the report cannot serialize; an
/// unreachable or unparsable source is a reported result, not a failure.
pub fn run(args: &VersionsArgs) -> Result<(), RkError> {
    if !args.check {
        Output::human().result_raw(embedded::VERSIONS);
        return Ok(());
    }
    let out = Output::new(args.json);
    let mut results = Vec::new();
    for pin in registry::pins() {
        let result = pin.check.as_deref().map_or_else(
            || PinResult {
                tool: pin.name.clone(),
                pinned: pin.version.clone(),
                result: "source-unreachable",
                available: None,
            },
            |url| check_one(&pin.name, &pin.version, url),
        );
        out.result_line(match (&result.result, &result.available) {
            (&"update-available", Some(available)) => format!(
                "update-available {} {} pinned, {available} at the source",
                result.tool, result.pinned
            ),
            _ => format!("{} {} {}", result.result, result.tool, result.pinned),
        });
        results.push(result);
    }
    out.next(&[
        "a pin update is a reviewed change to versions.toml, with its checked date".to_owned(),
    ]);
    out.emit(&Report {
        schema: "rk.versions-check/1",
        pins: results,
    })
}

/// Fetch one check URL and classify the answer.
fn check_one(tool: &str, pinned: &str, url: &str) -> PinResult {
    let result = |result, available| PinResult {
        tool: tool.to_owned(),
        pinned: pinned.to_owned(),
        result,
        available,
    };
    let curl = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let fetched = std::process::Command::new(curl)
        .args(["-fsSL", "--max-time", "10", url])
        .output();
    let body = match fetched {
        Ok(output) if output.status.success() => output.stdout,
        _ => return result("source-unreachable", None),
    };
    let Some(available) = latest_version(&body) else {
        return result("source-unparsable", None);
    };
    if is_current(pinned, &available) {
        result("current", Some(available))
    } else {
        result("update-available", Some(available))
    }
}

/// The latest version a source's JSON names: `max_stable_version` from a
/// crates.io answer, `tag_name` from a forge's releases answer.
fn latest_version(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let raw = value
        .get("crate")
        .and_then(|krate| krate.get("max_stable_version"))
        .or_else(|| value.get("tag_name"))
        .and_then(serde_json::Value::as_str)?;
    // A tag may prefix the number — `v2.13.1`, or a name before it — so
    // the version starts at the first digit.
    let start = raw.find(|c: char| c.is_ascii_digit())?;
    Some(raw[start..].to_owned())
}

/// Whether the pin already matches the source: exactly, or — for a pin
/// naming only a major, as the action pins do — by major version.
fn is_current(pinned: &str, available: &str) -> bool {
    if pinned == available {
        return true;
    }
    !pinned.contains('.') && available.split('.').next() == Some(pinned)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{PinResult, Report, is_current, latest_version};

    #[test]
    fn a_source_version_is_read_from_both_answer_shapes() {
        assert_eq!(
            latest_version(br#"{"crate":{"max_stable_version":"0.3.170"}}"#),
            Some("0.3.170".to_owned())
        );
        assert_eq!(
            latest_version(br#"{"tag_name":"v2.13.1"}"#),
            Some("2.13.1".to_owned())
        );
        assert_eq!(
            latest_version(br#"{"tag_name":"release-plz-v0.3.160"}"#),
            Some("0.3.160".to_owned())
        );
        assert_eq!(latest_version(b"not json"), None);
        assert_eq!(latest_version(br#"{"unrelated":true}"#), None);
    }

    #[test]
    fn a_major_only_pin_is_current_within_its_major() {
        assert!(is_current("0.3.160", "0.3.160"));
        assert!(!is_current("0.3.160", "0.3.170"));
        assert!(is_current("4", "4.3.1"));
        assert!(!is_current("4", "5.0.0"));
    }

    /// The complete `rk.versions-check/1` shape, held by snapshot.
    #[test]
    fn the_versions_check_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.versions-check/1",
            pins: vec![PinResult {
                tool: "release-plz".into(),
                pinned: "0.3.160".into(),
                result: "update-available",
                available: Some("0.3.170".into()),
            }],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.versions-check/1","pins":[{"tool":"release-plz","pinned":"0.3.160","result":"update-available","available":"0.3.170"}]}"#
        );
    }
}
