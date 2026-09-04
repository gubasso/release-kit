//! The one network call the devshell verb makes: which release is the
//! latest.
//!
//! The answer is the redirect of the forge's `releases/latest` page. It
//! costs no API quota and no token, and it excludes prereleases, which
//! the tag list does not. The fetch goes through `curl`, resolved like
//! every other soft tool with `RK_CURL_BIN` as the override, so a test
//! substitutes the network and the binary needs no HTTP stack.

use std::cmp::Ordering;

use super::normalize_tag;
use super::pin::PIN_PREFIX;

/// What discovery found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// The latest release's tag, normalized.
    Tag(String),
    /// The source did not answer; the detail is curl's last line.
    Unreachable(String),
    /// The source answered something that names no tag.
    Unparsable(String),
}

/// The page whose redirect names the latest release, derived from the
/// pin grammar so the project path has one owner.
#[must_use]
pub fn latest_url() -> String {
    let path = PIN_PREFIX
        .trim_start_matches("github:")
        .trim_end_matches('/');
    format!("https://github.com/{path}/releases/latest")
}

/// The latest release's tag, through one curl call that follows the
/// redirect and prints the effective URL alone.
#[must_use]
pub fn latest_tag() -> Discovery {
    let curl = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let fetched = std::process::Command::new(curl)
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-o",
            "/dev/null",
            "-w",
            "%{url_effective}",
            &latest_url(),
        ])
        .output();
    let output = match fetched {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return Discovery::Unreachable(crate::maintenance::last_line(&output.stderr));
        }
        Err(source) => return Discovery::Unreachable(format!("curl did not run: {source}")),
    };
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    match url.rsplit_once("/releases/tag/") {
        Some((_, tail)) => {
            normalize_tag(tail).map_or_else(|| Discovery::Unparsable(url.clone()), Discovery::Tag)
        }
        None => Discovery::Unparsable(url),
    }
}

/// Semantic version order over two tags, with or without the leading `v`.
///
/// Numeric components compare as numbers, a release outranks its own
/// prerelease, and prerelease identifiers compare the way semver says.
/// Anything the grammar cannot read falls back to text order.
#[must_use]
pub fn version_order(left: &str, right: &str) -> Ordering {
    let (left_core, left_pre) = split_version(left);
    let (right_core, right_pre) = split_version(right);
    let core = left_core
        .iter()
        .zip(right_core.iter())
        .map(|(a, b)| a.cmp(b))
        .find(|order| order.is_ne())
        .unwrap_or_else(|| left_core.len().cmp(&right_core.len()));
    if core.is_ne() {
        return core;
    }
    match (left_pre, right_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => prerelease_order(a, b),
    }
}

/// One component: a number, or an identifier that sorts after every number.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Component {
    Number(u64),
    Text(String),
}

/// The core components and the prerelease suffix of a tag.
fn split_version(tag: &str) -> (Vec<Component>, Option<&str>) {
    let trimmed = tag.trim();
    let bare = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let bare = bare.split('+').next().unwrap_or(bare);
    let (core, pre) = bare
        .split_once('-')
        .map_or((bare, None), |(core, pre)| (core, Some(pre)));
    (core.split('.').map(component).collect(), pre)
}

fn component(text: &str) -> Component {
    text.parse()
        .map_or_else(|_| Component::Text(text.to_owned()), Component::Number)
}

/// Prerelease order: identifier by identifier, numbers before text,
/// and the shorter list first where every shared identifier ties.
fn prerelease_order(left: &str, right: &str) -> Ordering {
    let a: Vec<Component> = left.split('.').map(component).collect();
    let b: Vec<Component> = right.split('.').map(component).collect();
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| x.cmp(y))
        .find(|order| order.is_ne())
        .unwrap_or_else(|| a.len().cmp(&b.len()))
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::{latest_url, version_order};

    #[test]
    fn the_version_order_is_semantic_not_textual() {
        assert_eq!(version_order("v0.2.16", "v0.2.16"), Ordering::Equal);
        assert_eq!(version_order("0.2.16", "v0.2.16"), Ordering::Equal);
        assert_eq!(version_order("v0.2.9", "v0.2.16"), Ordering::Less);
        assert_eq!(version_order("v0.10.0", "v0.9.9"), Ordering::Greater);
        assert_eq!(version_order("v1.0.0", "v1.0.0-rc.1"), Ordering::Greater);
        assert_eq!(version_order("v1.0.0-rc.1", "v1.0.0-rc.2"), Ordering::Less);
        assert_eq!(
            version_order("v1.0.0-alpha", "v1.0.0-alpha.1"),
            Ordering::Less
        );
        assert_eq!(version_order("v1.0.0-1", "v1.0.0-beta"), Ordering::Less);
        assert_eq!(version_order("v1.0.0+build", "v1.0.0"), Ordering::Equal);
        assert_eq!(version_order("v1.0", "v1.0.0"), Ordering::Less);
    }

    #[test]
    fn the_latest_url_derives_from_the_pin_grammar() {
        assert_eq!(
            latest_url(),
            "https://github.com/gubasso/release-kit/releases/latest"
        );
    }
}
