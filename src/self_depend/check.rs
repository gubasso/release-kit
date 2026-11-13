//! The version check the `.envrc` line runs on every directory entry.
//!
//! It reads one cache file per checkout and reads the network only when a
//! refresh is due: seven days after the last success, or one hour after a
//! failed attempt. A due refresh takes an OS file lock beside the cache
//! first; a check that finds the lock held reads the cache it has, because
//! another process is refreshing. It writes no file but its cache and the
//! lock beside it, and it never moves a pin.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::discover::{self, Discovery};

/// How long a success stays fresh.
pub const SUCCESS_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// How long a failed attempt waits before the next one.
pub const RETRY_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// One checkout's cached answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cache {
    /// When the last attempt started, RFC 3339 UTC.
    pub attempted: Option<String>,
    /// When the last successful attempt finished, RFC 3339 UTC.
    pub succeeded: Option<String>,
    /// The latest release that success found.
    pub latest: Option<String>,
    /// Why the last attempt failed, where it failed.
    pub error: Option<String>,
}

/// The cache and its lock for one checkout.
#[derive(Debug, Clone)]
pub struct Paths {
    /// The cache file.
    pub cache: PathBuf,
    /// The lock file beside it.
    pub lock: PathBuf,
}

impl Paths {
    /// The paths for the checkout `key`, where a cache root resolves.
    #[must_use]
    pub fn for_key(key: &str) -> Option<Self> {
        let dir = super::check_cache_dir()?;
        Some(Self {
            cache: dir.join(format!("{key}.json")),
            lock: dir.join(format!("{key}.lock")),
        })
    }
}

/// What one check did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The cache answered; no refresh was due.
    Cached,
    /// A refresh read the network and recorded its answer.
    Refreshed,
    /// A refresh was due, and another process held the lock.
    Locked,
    /// The cache cannot be written, so nothing was read.
    Unwritable(String),
}

impl Outcome {
    /// The outcome's word in the machine report.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Cached => "cached",
            Self::Refreshed => "refreshed",
            Self::Locked => "locked",
            Self::Unwritable(_) => "unwritable",
        }
    }
}

/// A parsed time, or `None` for an absent or unreadable one.
fn time(text: Option<&str>) -> Option<SystemTime> {
    humantime::parse_rfc3339(text?).ok()
}

/// RFC 3339 UTC to the second.
#[must_use]
pub fn stamp(at: SystemTime) -> String {
    humantime::format_rfc3339_seconds(at).to_string()
}

/// How long ago `then` was, zero for a time in the future.
fn age(now: SystemTime, then: SystemTime) -> Duration {
    now.duration_since(then).unwrap_or_default()
}

/// Whether the cache asks for a network read at `now`.
///
/// After an attempt that failed, or that never finished, a read is due
/// only one hour after that attempt, whatever an older success says. An
/// attempt never finished where it is later than the last success, or
/// where no success exists, and no error was recorded. Otherwise a read is
/// due where the cache is missing, holds no success, or holds a success
/// seven days old or more. Times compare as times, and an unreadable time
/// counts as missing.
#[must_use]
pub fn due(cache: Option<&Cache>, now: SystemTime) -> bool {
    let Some(cache) = cache else {
        return true;
    };
    let attempted = time(cache.attempted.as_deref());
    let succeeded = time(cache.succeeded.as_deref());
    let unfinished = match (attempted, succeeded) {
        (Some(attempted), Some(succeeded)) => attempted > succeeded,
        (Some(_), None) => true,
        (None, _) => false,
    };
    if cache.error.is_some() || unfinished {
        return attempted.is_none_or(|attempted| age(now, attempted) >= RETRY_INTERVAL);
    }
    succeeded.is_none_or(|succeeded| age(now, succeeded) >= SUCCESS_INTERVAL)
}

/// When the next attempt after a failure may read the network.
#[must_use]
pub fn next_try(cache: &Cache) -> Option<String> {
    time(cache.attempted.as_deref()).map(|attempted| stamp(attempted + RETRY_INTERVAL))
}

/// The cache at `path`, or `None` where it is absent or unreadable.
#[must_use]
pub fn read(path: &Path) -> Option<Cache> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// Write the cache atomically.
fn write(path: &Path, cache: &Cache) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(cache).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    crate::atomic::write(path, &bytes)
}

/// What one run of the check found.
#[derive(Debug, Clone)]
pub struct Run {
    /// What the run did.
    pub outcome: Outcome,
    /// The cache the warnings read from.
    pub cache: Option<Cache>,
    /// Whether this run read the network, whatever it could record.
    pub fetched: bool,
}

/// Run the check for one checkout: read the cache, and refresh it where a
/// refresh is due and the lock is free.
#[must_use]
pub fn run(paths: &Paths, now: SystemTime) -> Run {
    let (outcome, cache, fetched) = refresh(paths, now);
    Run {
        outcome,
        cache,
        fetched,
    }
}

/// The refresh behind [`run`], as the outcome, the cache, and whether the
/// network was read.
fn refresh(paths: &Paths, now: SystemTime) -> (Outcome, Option<Cache>, bool) {
    let cached = read(&paths.cache);
    if !due(cached.as_ref(), now) {
        return (Outcome::Cached, cached, false);
    }
    if let Some(parent) = paths.cache.parent()
        && let Err(source) = fs::create_dir_all(parent)
    {
        return (Outcome::Unwritable(source.to_string()), cached, false);
    }
    let lock = match fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&paths.lock)
    {
        Ok(lock) => lock,
        Err(source) => return (Outcome::Unwritable(source.to_string()), cached, false),
    };
    // An OS lock, released when the file closes, so a process that dies
    // holding it frees it: no age, no owner file, and no takeover.
    match lock.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => return (Outcome::Locked, cached, false),
        Err(fs::TryLockError::Error(source)) => {
            return (Outcome::Unwritable(source.to_string()), cached, false);
        }
    }
    // Another process may have refreshed between the read and the lock.
    let cached = read(&paths.cache);
    if !due(cached.as_ref(), now) {
        return (Outcome::Cached, cached, false);
    }
    let mut cache = cached.unwrap_or_default();
    cache.attempted = Some(stamp(now));
    // The attempt is recorded before the read, so a read that hangs or
    // dies still postpones the next one. A cache that cannot be written
    // reads nothing.
    if let Err(source) = write(&paths.cache, &cache) {
        return (
            Outcome::Unwritable(source.to_string()),
            read(&paths.cache),
            false,
        );
    }
    match discover::latest_tag() {
        Discovery::Tag(tag) => {
            cache.succeeded = Some(stamp(SystemTime::now()));
            cache.latest = Some(tag);
            cache.error = None;
        }
        Discovery::Unreachable(reason) => cache.error = Some(reason),
        Discovery::Unparsable(answer) => {
            cache.error = Some(format!("the release page answered no tag: {answer}"));
        }
    }
    if let Err(source) = write(&paths.cache, &cache) {
        return (Outcome::Unwritable(source.to_string()), Some(cache), true);
    }
    drop(lock);
    (Outcome::Refreshed, Some(cache), true)
}

/// The warnings one check prints, from the cache it read, for the pin
/// `pin`.
#[must_use]
pub fn warnings(
    pin: Option<&str>,
    cache: Option<&Cache>,
    outcome: &Outcome,
    path: &Path,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Outcome::Unwritable(reason) = outcome {
        lines.push(format!(
            "rk cannot record its version check at {}: {reason}.",
            path.display()
        ));
    }
    let Some(cache) = cache else {
        return lines;
    };
    if let (Some(pin), Some(latest)) = (pin, cache.latest.as_deref())
        && discover::version_order(latest, pin).is_gt()
    {
        lines.push(format!(
            "rk {} is out, this project pins {}. To move: rk self-depend sync --apply",
            latest.trim_start_matches('v'),
            pin.trim_start_matches('v')
        ));
    }
    if let Some(error) = &cache.error {
        lines.push(format!(
            "rk could not check for a newer release: {error}. Next try after {}.",
            next_try(cache).unwrap_or_else(|| "the next entry".to_owned())
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{Cache, due, stamp};

    fn ago(secs: u64) -> String {
        stamp(SystemTime::now() - Duration::from_secs(secs))
    }

    #[test]
    fn a_success_is_fresh_for_seven_days() {
        let now = SystemTime::now();
        let cache = |secs| Cache {
            attempted: Some(ago(secs)),
            succeeded: Some(ago(secs)),
            latest: Some("v0.9.15".to_owned()),
            error: None,
        };
        assert!(!due(Some(&cache(6 * 86_400)), now));
        assert!(due(Some(&cache(7 * 86_400)), now));
        assert!(due(None, now));
    }

    #[test]
    fn a_failure_waits_one_hour_whatever_the_success() {
        let now = SystemTime::now();
        let cache = |failed| Cache {
            attempted: Some(ago(failed)),
            succeeded: Some(ago(8 * 86_400)),
            latest: Some("v0.9.15".to_owned()),
            error: Some("no answer".to_owned()),
        };
        assert!(!due(Some(&cache(60)), now));
        assert!(!due(Some(&cache(59 * 60)), now));
        assert!(due(Some(&cache(60 * 60)), now));
    }

    #[test]
    fn an_unfinished_attempt_waits_one_hour() {
        let now = SystemTime::now();
        let first = |secs| Cache {
            attempted: Some(ago(secs)),
            ..Cache::default()
        };
        assert!(!due(Some(&first(60)), now));
        assert!(due(Some(&first(60 * 60)), now));
        let overdue = |secs| Cache {
            attempted: Some(ago(secs)),
            succeeded: Some(ago(8 * 86_400)),
            latest: Some("v0.9.15".to_owned()),
            error: None,
        };
        assert!(!due(Some(&overdue(60)), now));
        assert!(due(Some(&overdue(60 * 60)), now));
    }

    #[test]
    fn an_unreadable_time_counts_as_missing() {
        let cache = Cache {
            attempted: Some("yesterday".to_owned()),
            succeeded: Some("last week".to_owned()),
            latest: None,
            error: None,
        };
        assert!(due(Some(&cache), SystemTime::now()));
    }
}
