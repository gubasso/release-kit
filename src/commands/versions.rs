//! `rk versions`: the pinned-tool registry, and its freshness check.
//!
//! Plain `rk versions` prints the registry exactly as authored, offline.
//! `--check` is the canon-side freshness answer and, beside `rk devshell
//! sync`, one of the two verbs that fetch: it consults each pin's check
//! URL and reports per pin, where
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
    /// `current`, `update-available`, `source-unreachable`,
    /// `source-unparsable`, or — for a pin whose freshness lives in its
    /// ref alone — `no-version-source`.
    result: &'static str,
    /// The version the source serves, where one was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    available: Option<String>,
    /// The immutable execution commit, where the pin is an action.
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    /// How the discovery ref moves, from the registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_class: Option<String>,
    /// `ref-unmoved`, `ref-moved`, `ref-unreachable`, or
    /// `ref-unparsable`, for a pin carrying an action and a commit.
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_result: Option<&'static str>,
    /// The commit the discovery ref names today, where it was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_commit: Option<String>,
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
        let mut result = pin.check.as_deref().map_or_else(
            || PinResult {
                tool: pin.name.clone(),
                pinned: pin.version.clone(),
                // A pin can live without a version source only where its
                // freshness signal is the discovery ref itself.
                result: if pin.action.is_some() && pin.commit.is_some() {
                    "no-version-source"
                } else {
                    "source-unreachable"
                },
                available: None,
                commit: None,
                ref_class: None,
                ref_result: None,
                ref_commit: None,
            },
            |url| check_one(&pin.name, &pin.version, url),
        );
        if let (Some(action), Some(commit)) = (&pin.action, &pin.commit) {
            let (ref_result, ref_commit) = resolve_ref(action, commit);
            result.commit = Some(commit.clone());
            result.ref_class.clone_from(&pin.ref_class);
            result.ref_result = Some(ref_result);
            result.ref_commit = ref_commit;
        }
        out.result_line(match (&result.result, &result.available) {
            (&"update-available", Some(available)) => format!(
                "update-available {} {} pinned, {available} at the source",
                result.tool, result.pinned
            ),
            _ => format!("{} {} {}", result.result, result.tool, result.pinned),
        });
        if let Some(ref_result) = result.ref_result {
            let reference = pin
                .action
                .as_deref()
                .and_then(|action| action.split_once('@'))
                .map_or_else(String::new, |(_, reference)| reference.to_owned());
            out.result_line(match (ref_result, &result.ref_commit) {
                // Movement of a discovery ref is normal and by design: it
                // is an update signal the pinned commit already contains,
                // never something the tool can call an attack.
                ("ref-moved", Some(now)) => format!(
                    "ref-moved {}: {reference} now names {now}; an update to review, not an incident",
                    result.tool
                ),
                ("ref-unmoved", _) => format!(
                    "ref-unmoved {}: {reference} still names the pinned commit",
                    result.tool
                ),
                _ => format!("{ref_result} {}: {reference}", result.tool),
            });
        }
        results.push(result);
    }
    out.next(&[
        "a pin update is a reviewed change to versions.toml, with its checked date".to_owned(),
    ]);
    out.emit(&Report {
        schema: "rk.versions-check/2",
        pins: results,
    })
}

/// Resolve an action's discovery ref to the commit it names today and
/// compare it against the pinned execution commit.
fn resolve_ref(action: &str, pinned_commit: &str) -> (&'static str, Option<String>) {
    let Some((repo, reference)) = action.split_once('@') else {
        return ("ref-unparsable", None);
    };
    let url = format!("https://api.github.com/repos/{repo}/commits/{reference}");
    let curl = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let fetched = std::process::Command::new(curl)
        .args(["-fsSL", "--max-time", "10", &url])
        .output();
    let body = match fetched {
        Ok(output) if output.status.success() => output.stdout,
        _ => return ("ref-unreachable", None),
    };
    let Some(sha) = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("sha")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
    else {
        return ("ref-unparsable", None);
    };
    if sha == pinned_commit {
        ("ref-unmoved", Some(sha))
    } else {
        ("ref-moved", Some(sha))
    }
}

/// The page bound for a paged answer: at 100 entries a page, ten pages is
/// a thousand tags, past any project this registry pins. A source deeper
/// than that reads as unreachable rather than as a version this check
/// silently guessed.
const MAX_PAGES: u32 = 10;

/// One GET, or `None` where the fetch failed.
fn fetch(url: &str) -> Option<Vec<u8>> {
    let curl = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let fetched = std::process::Command::new(curl)
        .args(["-fsSL", "--max-time", "10", url])
        .output();
    match fetched {
        Ok(output) if output.status.success() => Some(output.stdout),
        _ => None,
    }
}

/// The same URL at a later page, in the query form every paged source here
/// takes.
fn paged(url: &str, page: u32) -> String {
    let joiner = if url.contains('?') { '&' } else { '?' };
    format!("{url}{joiner}page={page}")
}

/// Fetch one check URL and classify the answer.
///
/// An object answer — a crates.io crate, a forge's latest release — is one
/// GET. An array answer is a page of a list, and the endpoint documents no
/// ordering, so reading one page would compute the greatest of an arbitrary
/// subset and could report a stale pin as current. Every page is therefore
/// read, to the bound above, and the greatest version across all of them
/// wins.
///
/// Reaching the bound with a page still full is not an answer: the list
/// continues past what was read, so the greatest version is unknown. That
/// reads as `source-unreachable`, the same as a failed fetch, rather than
/// as the greatest of the pages that happened to fit.
fn check_one(tool: &str, pinned: &str, url: &str) -> PinResult {
    let result = |result, available| PinResult {
        tool: tool.to_owned(),
        pinned: pinned.to_owned(),
        result,
        available,
        commit: None,
        ref_class: None,
        ref_result: None,
        ref_commit: None,
    };
    let Some(body) = fetch(url) else {
        return result("source-unreachable", None);
    };
    let mut best = latest_version(&body);
    if is_page(&body) && !is_empty_page(&body) {
        let mut ended = false;
        for page in 2..=MAX_PAGES {
            let Some(body) = fetch(&paged(url, page)) else {
                return result("source-unreachable", None);
            };
            // Every page of a list is a list. A later page that answers
            // some other shape is a source this reader does not
            // understand, and feeding it to the object parser would mint
            // a version out of an answer that names no tag.
            if !is_page(&body) {
                return result("source-unparsable", None);
            }
            if is_empty_page(&body) {
                ended = true;
                break;
            }
            best = greater(best, latest_version(&body));
        }
        if !ended {
            return result("source-unreachable", None);
        }
    }
    let Some(available) = best else {
        return result("source-unparsable", None);
    };
    if is_current(pinned, &available) {
        result("current", Some(available))
    } else {
        result("update-available", Some(available))
    }
}

/// Whether the answer is one page of a list rather than a single object.
fn is_page(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body).is_ok_and(|value| value.is_array())
}

/// Whether the answer is a page past the end of the list.
fn is_empty_page(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .is_ok_and(|value| value.as_array().is_some_and(Vec::is_empty))
}

/// The greater of two versions, comparing numerically component by
/// component, with an absent version losing to any present one.
fn greater(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => {
            if numeric_parts(&right) > numeric_parts(&left) {
                Some(right)
            } else {
                Some(left)
            }
        }
        (some, None) | (None, some) => some,
    }
}

/// The latest version a source's JSON names, across the three answer
/// shapes: `max_stable_version` from a crates.io answer, `tag_name` from a
/// forge's releases answer, and the greatest `name` from a forge's tags
/// answer, which is an array.
///
/// The tags shape exists for a project that publishes tags and cuts no
/// releases, so no releases answer names its latest version. The greatest
/// is taken rather than the first, because the endpoint promises no
/// ordering.
fn latest_version(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if let Some(tags) = value.as_array() {
        return tags
            .iter()
            .filter_map(|tag| tag.get("name").and_then(serde_json::Value::as_str))
            .filter_map(number_from)
            .max_by(|left, right| numeric_parts(left).cmp(&numeric_parts(right)));
    }
    let raw = value
        .get("crate")
        .and_then(|krate| krate.get("max_stable_version"))
        .or_else(|| value.get("tag_name"))
        .and_then(serde_json::Value::as_str)?;
    number_from(raw)
}

/// A ref name from its first digit on: a tag may prefix the number —
/// `v2.13.1`, or a name before it.
fn number_from(raw: &str) -> Option<String> {
    let start = raw.find(|c: char| c.is_ascii_digit())?;
    Some(raw[start..].to_owned())
}

/// A version's numeric components, so that 2.10 orders above 2.9 rather
/// than below it; the first component that is not a number ends the list,
/// which keeps a suffixed variant below its plain sibling.
fn numeric_parts(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map_while(|part| part.parse::<u64>().ok())
        .collect()
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

    /// The tags shape, for a project that publishes tags and cuts no
    /// releases. The greatest wins, not the first, because the endpoint
    /// promises no ordering; and 2.10 is above 2.9, not below it.
    #[test]
    fn a_tags_answer_reads_the_greatest_name() {
        assert_eq!(
            latest_version(br#"[{"name":"2.35.0"},{"name":"2.35.2"},{"name":"2.34.8"}]"#),
            Some("2.35.2".to_owned())
        );
        assert_eq!(
            latest_version(br#"[{"name":"2.9.0"},{"name":"2.10.0"}]"#),
            Some("2.10.0".to_owned())
        );
        assert_eq!(
            latest_version(br#"[{"name":"v1.2.3"}]"#),
            Some("1.2.3".to_owned())
        );
        assert_eq!(latest_version(b"[]"), None);
        assert_eq!(latest_version(br#"[{"sha":"abc"}]"#), None);
    }

    #[test]
    fn a_major_only_pin_is_current_within_its_major() {
        assert!(is_current("0.3.160", "0.3.160"));
        assert!(!is_current("0.3.160", "0.3.170"));
        assert!(is_current("4", "4.3.1"));
        assert!(!is_current("4", "5.0.0"));
    }

    /// The complete `rk.versions-check/2` shape, held by snapshot.
    #[test]
    fn the_versions_check_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.versions-check/2",
            pins: vec![PinResult {
                tool: "release-plz".into(),
                pinned: "0.3.160".into(),
                result: "update-available",
                available: Some("0.3.170".into()),
                commit: Some("2eb1d8bcb770b4c48ccfaad919734b38b51958c9".into()),
                ref_class: Some("moving-minor-tag".into()),
                ref_result: Some("ref-unmoved"),
                ref_commit: Some("2eb1d8bcb770b4c48ccfaad919734b38b51958c9".into()),
            }],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.versions-check/2","pins":[{"tool":"release-plz","pinned":"0.3.160","result":"update-available","available":"0.3.170","commit":"2eb1d8bcb770b4c48ccfaad919734b38b51958c9","ref_class":"moving-minor-tag","ref_result":"ref-unmoved","ref_commit":"2eb1d8bcb770b4c48ccfaad919734b38b51958c9"}]}"#
        );
    }
}
