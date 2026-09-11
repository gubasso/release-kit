//! The version a dependency is pinned at, and the tag that names it.
//!
//! The source tree's declared version is the default and `--pin` the
//! one override; the tag form follows the shape the source's own tags
//! show. No registry is asked whether the version is published.

use serde::Serialize;

use super::source::{Source, TagStyle};
use crate::error::RkError;

/// The resolved pin.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    /// The bare version, `1.2.3`.
    pub version: String,
    /// The tag that names it in the source's own shape.
    pub tag: String,
    /// `argument` or `source-tree`.
    pub origin: &'static str,
}

/// Resolve the pin from the argument, else from the source tree.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for an argument that is not a version and
/// for a source that declares none while no argument was given.
pub fn resolve(source: &Source, argument: Option<&str>) -> Result<Resolved, RkError> {
    if let Some(raw) = argument {
        let Some(tag) = crate::devshell::normalize_tag(raw) else {
            return Err(RkError::Usage(format!(
                "--pin {raw} is not a version: pass 1.2.3, v1.2.3, or the release URL"
            )));
        };
        let version = tag.trim_start_matches('v').to_owned();
        return Ok(Resolved {
            tag: tag_for(&version, source.tag_style),
            version,
            origin: "argument",
        });
    }
    let Some(version) = source.version.clone() else {
        return Err(RkError::Usage(
            "the source declares no version; pass --pin".into(),
        ));
    };
    Ok(Resolved {
        tag: tag_for(&version, source.tag_style),
        version,
        origin: "source-tree",
    })
}

/// The tag for a version in the source's shape; the prefixed form where
/// the tags say nothing.
#[must_use]
pub fn tag_for(version: &str, style: TagStyle) -> String {
    match style {
        TagStyle::Bare => version.to_owned(),
        TagStyle::Prefixed | TagStyle::Unknown => format!("v{version}"),
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::{TagStyle, resolve, tag_for};
    use crate::depend::source::Source;
    use crate::error::RkError;

    fn source(version: Option<&str>, style: TagStyle) -> Source {
        Source {
            path: Utf8PathBuf::from("/srv/sample"),
            tech: Some("rust"),
            name: Some("sample-tool".into()),
            version: version.map(str::to_owned),
            bins: Vec::new(),
            owner_repo: None,
            host: None,
            flake_package: false,
            dist_github: false,
            binstall_github: false,
            tag_style: style,
            channels: Vec::new(),
        }
    }

    /// SATISFIES dependencies:the-version-comes-from-the-source-tree
    #[test]
    fn the_tag_follows_the_source_tag_style() {
        assert_eq!(tag_for("1.4.0", TagStyle::Prefixed), "v1.4.0");
        assert_eq!(tag_for("1.4.0", TagStyle::Bare), "1.4.0");
        assert_eq!(tag_for("1.4.0", TagStyle::Unknown), "v1.4.0");
        let resolved = resolve(&source(Some("1.4.0"), TagStyle::Bare), None).expect("resolves");
        assert_eq!(resolved.version, "1.4.0");
        assert_eq!(resolved.tag, "1.4.0");
        assert_eq!(resolved.origin, "source-tree");
    }

    /// SATISFIES dependencies:the-version-comes-from-the-source-tree
    #[test]
    fn the_argument_overrides_the_tree() {
        let tree = source(Some("1.4.0"), TagStyle::Prefixed);
        for raw in [
            "2.0.0",
            "v2.0.0",
            "https://github.com/acme/sample/releases/tag/v2.0.0",
        ] {
            let resolved = resolve(&tree, Some(raw)).expect("resolves");
            assert_eq!(resolved.version, "2.0.0", "{raw}");
            assert_eq!(resolved.tag, "v2.0.0", "{raw}");
            assert_eq!(resolved.origin, "argument");
        }
        assert!(matches!(
            resolve(&tree, Some("latest")),
            Err(RkError::Usage(_))
        ));
    }

    #[test]
    fn no_version_is_a_usage_error() {
        assert!(matches!(
            resolve(&source(None, TagStyle::Unknown), None),
            Err(RkError::Usage(_))
        ));
    }
}
