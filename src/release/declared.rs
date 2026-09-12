//! What a bundle declares beyond its bytes: the compatibility file and
//! the guidance files, read through the seam and parsed once.
//!
//! `compatibility.toml` is small by design. A bundle that carries none
//! declares no requirement beyond the payload schema, which is what
//! [`Compatibility::default`] says. A guidance file is one release's
//! operator step, named by the version that introduces it and carrying
//! the destinations it concerns, so the planner can filter it against a
//! target.

use serde::{Deserialize, Serialize};

use crate::error::RkError;

use super::{ReleaseManifest, ReleaseSource};

/// The compatibility file's path inside a bundle.
pub const COMPATIBILITY_PATH: &str = "compatibility.toml";

/// The guidance root inside a bundle.
pub const GUIDANCE_ROOT: &str = "guidance";

/// What the bundle needs beyond the payload schema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Compatibility {
    /// The declaration's own shape version.
    pub schema: u32,
    /// The engine axis.
    pub engine: Engine,
    /// The guidance coverage the bundle claims.
    pub guidance: GuidanceDecl,
    /// Per-forge floors, by forge name.
    pub forge: std::collections::BTreeMap<String, Floor>,
    /// Releases a landing must pass through.
    pub intermediate: Vec<Intermediate>,
}

/// The engine axis: the oldest engine that may land this bundle.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Engine {
    /// The minimum engine version, where the schema alone is too coarse.
    pub minimum: Option<String>,
}

/// The guidance coverage the bundle claims.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuidanceDecl {
    /// The release above which every release is described: it ships a
    /// guidance file or needs no step.
    pub since: Option<String>,
    /// Releases that changed a landed destination and needed no step.
    pub no_steps: Vec<String>,
}

/// One forge's version floor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Floor {
    /// The floor, as `major.minor`.
    pub minimum: Option<String>,
}

/// One release a landing must pass through.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Intermediate {
    /// The version.
    pub version: String,
    /// Why it cannot be skipped.
    pub reason: String,
}

/// Read the compatibility file through the seam, or the default where the
/// bundle carries none.
///
/// # Errors
///
/// Returns the source's failure where the artifact exists and its bytes
/// cannot be read, and [`RkError::Other`] where they do not parse.
pub fn compatibility(
    source: &dyn ReleaseSource,
    manifest: &ReleaseManifest,
) -> Result<Compatibility, RkError> {
    if manifest.artifact(COMPATIBILITY_PATH).is_none() {
        return Ok(Compatibility::default());
    }
    let bytes = super::read(source, manifest, COMPATIBILITY_PATH)?;
    parse_compatibility(&String::from_utf8_lossy(&bytes))
}

/// Parse the compatibility file's text.
///
/// # Errors
///
/// Returns [`RkError::Other`] where the text is not the declared shape.
pub fn parse_compatibility(text: &str) -> Result<Compatibility, RkError> {
    toml::from_str(text).map_err(|error| anyhow::anyhow!("{COMPATIBILITY_PATH}: {error}").into())
}

/// The closed set of actions a guidance file may name.
pub const GUIDANCE_ACTIONS: [&str; 2] = ["operator-step", "plan-operation"];

/// One release's guidance, parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuidanceFile {
    /// The version that introduces the change, from the file name.
    pub version: String,
    /// The heading.
    pub title: String,
    /// The landed paths the step concerns.
    pub destinations: Vec<String>,
    /// `operator-step` or `plan-operation`.
    pub action: String,
    /// The body below the field list.
    pub body: String,
}

/// Whether the bundle carries a guidance root at all. A bundle from
/// before the root existed carries none, and its history is unavailable
/// rather than empty.
#[must_use]
pub fn carries_guidance(manifest: &ReleaseManifest) -> bool {
    manifest.under(GUIDANCE_ROOT).next().is_some()
}

/// Every guidance file the bundle carries, parsed, sorted by version.
///
/// # Errors
///
/// Returns the source's failure for bytes that cannot be read, and
/// [`RkError::Other`] for a file that does not parse.
pub fn guidance(
    source: &dyn ReleaseSource,
    manifest: &ReleaseManifest,
) -> Result<Vec<GuidanceFile>, RkError> {
    let mut files = Vec::new();
    for (rest, artifact) in manifest.under(GUIDANCE_ROOT) {
        let Some(stem) = rest.strip_suffix(".md") else {
            continue;
        };
        if stem == "README" || stem.contains('/') {
            continue;
        }
        let bytes = source.blob(&artifact.sha256)?;
        files.push(parse_guidance(stem, &String::from_utf8_lossy(&bytes))?);
    }
    files.sort_by_key(|file| version_key(&file.version));
    Ok(files)
}

/// Parse one guidance file.
///
/// # Errors
///
/// Returns [`RkError::Other`] for a file without a heading, without the
/// two fields, with an action outside the closed set, or with an empty
/// destination list.
pub fn parse_guidance(version: &str, text: &str) -> Result<GuidanceFile, RkError> {
    let fail =
        |what: &str| -> RkError { anyhow::anyhow!("{GUIDANCE_ROOT}/{version}.md: {what}").into() };
    if version_key(version).is_none() {
        return Err(fail("the file name is not a version"));
    }
    let mut lines = text.lines();
    let title = lines
        .by_ref()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| line.strip_prefix("# "))
        .ok_or_else(|| fail("the first line is not a `# ` heading"))?
        .trim()
        .to_owned();
    let mut destinations: Option<Vec<String>> = None;
    let mut action: Option<String> = None;
    let mut body = String::new();
    let mut in_fields = true;
    for line in lines {
        if in_fields {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(field) = line.strip_prefix("- ") {
                if let Some((key, value)) = field.split_once(':') {
                    match key.trim() {
                        "destinations" => {
                            destinations = Some(
                                value
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_owned)
                                    .collect(),
                            );
                            continue;
                        }
                        "action" => {
                            action = Some(value.trim().to_owned());
                            continue;
                        }
                        _ => {}
                    }
                }
            }
            in_fields = false;
        }
        body.push_str(line);
        body.push('\n');
    }
    let destinations = destinations.ok_or_else(|| fail("no `- destinations:` field"))?;
    if destinations.is_empty() {
        return Err(fail("the destinations field names no path"));
    }
    let action = action.ok_or_else(|| fail("no `- action:` field"))?;
    if !GUIDANCE_ACTIONS.contains(&action.as_str()) {
        return Err(fail(&format!(
            "action `{action}` is not one of {}",
            GUIDANCE_ACTIONS.join(", ")
        )));
    }
    Ok(GuidanceFile {
        version: version.to_owned(),
        title,
        destinations,
        action,
        body: body.trim().to_owned(),
    })
}

/// A version as a comparable key: the numeric components, with a leading
/// `v` and any pre-release or build suffix dropped. `None` for text with
/// no leading number.
#[must_use]
pub fn version_key(version: &str) -> Option<Vec<u64>> {
    let core = version.strip_prefix('v').unwrap_or(version);
    let core = core.split(['-', '+']).next().unwrap_or(core);
    let parts: Vec<u64> = core
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()?;
    (!parts.is_empty()).then_some(parts)
}

/// Whether `a` is below `b`, as versions. Text that is not a version
/// compares as not below, so an unreadable value never passes a floor.
#[must_use]
pub fn version_below(a: &str, b: &str) -> bool {
    match (version_key(a), version_key(b)) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Compatibility, parse_compatibility, parse_guidance, version_below, version_key};

    #[test]
    fn a_bundle_without_compatibility_requires_only_its_schema() {
        let empty = Compatibility::default();
        assert!(empty.engine.minimum.is_none());
        assert!(empty.guidance.since.is_none());
        assert!(empty.forge.is_empty());
        assert!(empty.intermediate.is_empty());
        let parsed = parse_compatibility("schema = 1\n").expect("a bare file parses");
        assert_eq!(parsed.engine, empty.engine);
        assert_eq!(parsed.forge, empty.forge);
        assert_eq!(parsed.intermediate, empty.intermediate);
        let full = parse_compatibility(
            "schema = 1\n[engine]\nminimum = \"0.4.0\"\n[guidance]\nsince = \"0.3.0\"\nno_steps = [\"0.3.1\"]\n[forge.gitlab]\nminimum = \"18.2\"\n[[intermediate]]\nversion = \"0.5.0\"\nreason = \"the record changed shape\"\n",
        )
        .expect("a full file parses");
        assert_eq!(full.engine.minimum.as_deref(), Some("0.4.0"));
        assert_eq!(full.guidance.since.as_deref(), Some("0.3.0"));
        assert_eq!(full.guidance.no_steps, ["0.3.1"]);
        assert_eq!(full.forge["gitlab"].minimum.as_deref(), Some("18.2"));
        assert_eq!(full.intermediate[0].version, "0.5.0");
    }

    #[test]
    fn this_bundles_compatibility_parses() {
        let parsed = parse_compatibility(crate::embedded::COMPATIBILITY).expect("this file parses");
        assert_eq!(parsed.schema, 1);
        assert!(parsed.guidance.since.is_some());
        assert_eq!(parsed.forge["gitlab"].minimum.as_deref(), Some("18.2"));
    }

    #[test]
    fn a_guidance_file_parses_its_fields_and_body() {
        let file = parse_guidance(
            "0.3.19",
            "# release-kit 0.3.19\n\n- destinations: .envrc, flake.nix\n- action: operator-step\n\n## What changed\n\nText.\n",
        )
        .expect("the file parses");
        assert_eq!(file.title, "release-kit 0.3.19");
        assert_eq!(file.destinations, [".envrc", "flake.nix"]);
        assert_eq!(file.action, "operator-step");
        assert!(file.body.starts_with("## What changed"));
        assert!(parse_guidance("0.3.19", "# t\n\n- action: operator-step\n").is_err());
        assert!(
            parse_guidance(
                "0.3.19",
                "# t\n\n- destinations: \n- action: operator-step\n"
            )
            .is_err()
        );
        assert!(parse_guidance("0.3.19", "# t\n\n- destinations: a\n- action: shrug\n").is_err());
        assert!(
            parse_guidance(
                "next",
                "# t\n\n- destinations: a\n- action: operator-step\n"
            )
            .is_err()
        );
    }

    #[test]
    fn versions_compare_by_their_numbers() {
        assert_eq!(version_key("v1.2.3-rc.1"), Some(vec![1, 2, 3]));
        assert_eq!(version_key("18.2"), Some(vec![18, 2]));
        assert_eq!(version_key("latest"), None);
        assert!(version_below("0.3.18", "0.3.19"));
        assert!(version_below("18.1.9", "18.2"));
        assert!(!version_below("18.2.0", "18.2"));
        assert!(!version_below("latest", "1.0.0"));
    }
}
