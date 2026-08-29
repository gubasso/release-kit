//! The run journal: the audit record of one conversation with a forge.
//!
//! It answers, after the fact and from another session, what ran, with what
//! arguments, against what, and what came back. Per run: `meta.json`,
//! `events.jsonl`, `transcript.txt`, and the materialized scripts — removed
//! on clean completion, kept on failure. Retention is bounded: a run count
//! cap enforced at the start of each new run, oldest pruned first. The
//! journal is audit evidence and never resumable state.

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use serde::Serialize;

use crate::applog;
use crate::atomic;

/// The version of the `meta.json` shape.
pub const META_SCHEMA: &str = "rk.run-meta/1";

/// How many runs the journal root keeps; run N+1 prunes the oldest.
pub const RUNS_KEPT: usize = 20;

/// One materialized script, proven by digest.
#[derive(Debug, Serialize)]
pub struct ScriptRecord {
    /// The script's path relative to the run directory.
    pub path: String,
    /// The digest of the bytes that ran, equal to the embedded bytes.
    pub sha256: String,
}

/// How a secret was handled: the fact of the handling, never the value and
/// never a fingerprint of one.
#[derive(Debug, Serialize)]
pub struct SecretHandling {
    /// The environment variable the secret arrived in.
    pub secret: String,
    /// Whether a value was present.
    pub present: bool,
    /// Where the value came from.
    pub source: &'static str,
    /// How it reached the forge CLI.
    pub transport: &'static str,
    /// Whether outputs were redacted for it.
    pub redacted: bool,
}

/// The `meta.json` document.
#[derive(Debug, Serialize)]
pub struct Meta {
    /// The shape version of this document.
    pub schema: &'static str,
    /// The run's identifier, equal to its directory name.
    pub run_id: String,
    /// The binary that ran.
    pub rk_version: &'static str,
    /// The subcommand.
    pub command: String,
    /// The invocation's arguments as typed; a secret never appears in argv.
    pub argv: Vec<String>,
    /// The process that owns the run, for liveness while unfinished.
    pub pid: u32,
    /// The target repository.
    pub target: String,
    /// The forge acted on.
    pub forge: String,
    /// The project path.
    pub repo: String,
    /// Wall-clock UTC start.
    pub started: String,
    /// Wall-clock UTC end, absent while running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<String>,
    /// The process exit code, absent while running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// The failure reason, absent on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Every script this run materialized, with its digest.
    pub scripts: Vec<ScriptRecord>,
    /// Every secret this run handled.
    pub secrets: Vec<SecretHandling>,
}

/// One open journal.
#[derive(Debug)]
pub struct Journal {
    /// The run's directory.
    pub dir: PathBuf,
    meta: Meta,
    events: Option<fs::File>,
    transcript: Option<fs::File>,
}

impl Journal {
    /// Create the run directory — before any remote mutation — pruning the
    /// oldest runs past the cap first.
    ///
    /// # Errors
    ///
    /// Any filesystem failure; the caller decides whether that refuses the
    /// run (apply) or only costs the record (preview and check).
    pub fn create(command: &str, target: &str, forge: &str, repo: &str) -> std::io::Result<Self> {
        let root = runs_root().ok_or_else(|| {
            std::io::Error::other("neither XDG_STATE_HOME nor HOME is set; no journal root")
        })?;
        fs::create_dir_all(&root)?;
        let _ = prune_to(RUNS_KEPT.saturating_sub(1));
        let run_id = new_run_id();
        let dir = root.join(&run_id);
        fs::create_dir(&dir)?;
        restrict_dir(&dir);
        let events = fs::File::create(dir.join("events.jsonl"))?;
        let transcript = fs::File::create(dir.join("transcript.txt"))?;
        restrict_file(&dir.join("events.jsonl"));
        restrict_file(&dir.join("transcript.txt"));
        let meta = Meta {
            schema: META_SCHEMA,
            run_id,
            rk_version: env!("CARGO_PKG_VERSION"),
            command: command.to_owned(),
            argv: std::env::args().skip(1).collect(),
            pid: std::process::id(),
            target: target.to_owned(),
            forge: forge.to_owned(),
            repo: repo.to_owned(),
            started: applog::now_utc(),
            ended: None,
            exit_code: None,
            reason: None,
            scripts: Vec::new(),
            secrets: Vec::new(),
        };
        let journal = Self {
            dir,
            meta,
            events: Some(events),
            transcript: Some(transcript),
        };
        journal.write_meta();
        Ok(journal)
    }

    /// The run's identifier.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.meta.run_id
    }

    /// The directory the run's scripts materialize into.
    #[must_use]
    pub fn scripts_dir(&self) -> PathBuf {
        self.dir.join("scripts")
    }

    /// Append one already-serialized event line.
    pub fn event_line(&mut self, line: &str) {
        if let Some(file) = &mut self.events {
            let _ = writeln!(file, "{line}");
        }
    }

    /// Append raw bytes to the transcript, in arrival order.
    pub fn transcript(&mut self, bytes: &[u8]) {
        if let Some(file) = &mut self.transcript {
            let _ = file.write_all(bytes);
        }
    }

    /// Record one materialized script's digest.
    pub fn record_script(&mut self, path: String, sha256: String) {
        self.meta.scripts.push(ScriptRecord { path, sha256 });
        self.write_meta();
    }

    /// Record one secret's handling.
    pub fn record_secret(&mut self, secret: &str, present: bool) {
        self.meta.secrets.push(SecretHandling {
            secret: secret.to_owned(),
            present,
            source: "environment",
            transport: "stdin",
            redacted: true,
        });
        self.write_meta();
    }

    /// Close the run: record the terminal status, and remove the
    /// materialized scripts on clean completion so a failed run keeps
    /// exactly the script that ran beside the transcript of what it did.
    pub fn finish(&mut self, exit_code: i32, reason: Option<&str>) {
        self.meta.ended = Some(applog::now_utc());
        self.meta.exit_code = Some(exit_code);
        self.meta.reason = reason.map(str::to_owned);
        self.write_meta();
        self.events = None;
        self.transcript = None;
        if exit_code == 0 {
            let _ = fs::remove_dir_all(self.scripts_dir());
        }
    }

    fn write_meta(&self) {
        if let Ok(text) = serde_json::to_string_pretty(&self.meta) {
            let _ = atomic::write(&self.dir.join("meta.json"), text.as_bytes());
        }
        restrict_file(&self.dir.join("meta.json"));
    }
}

/// The journal root: `<state root>/runs`.
#[must_use]
pub fn runs_root() -> Option<PathBuf> {
    applog::state_root().map(|root| root.join("runs"))
}

/// Every run directory name, oldest first. The id opens with the UTC
/// timestamp, so lexical order is age order.
#[must_use]
pub fn list_run_ids() -> Vec<String> {
    let Some(root) = runs_root() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids
}

/// Remove the oldest runs past `keep`; the count removed.
///
/// A run whose record carries no terminal status may still be mutating a
/// forge, and unlinking a live run's scripts and record would leave it
/// running unobserved — so an unfinished run is spared until it is old
/// enough that it can only be the debris of a crash.
#[must_use]
pub fn prune_to(keep: usize) -> usize {
    let Some(root) = runs_root() else { return 0 };
    let ids = list_run_ids();
    let excess = ids.len().saturating_sub(keep);
    let mut removed = 0;
    for id in ids.into_iter().take(excess) {
        let dir = root.join(&id);
        if !prunable(&dir) {
            continue;
        }
        if fs::remove_dir_all(&dir).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// How long an unfinished run is presumed live before it counts as crash
/// debris the pruner may take.
const UNFINISHED_GRACE: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Whether a run directory may be pruned: any finished run; an unfinished
/// one only once its owning process is provably gone, or — where the host
/// cannot answer that — once it is older than the grace period. A stale pid
/// reused by another process reads as alive and merely delays the prune,
/// which errs in the safe direction.
fn prunable(dir: &std::path::Path) -> bool {
    let meta = fs::read(dir.join("meta.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    if meta
        .as_ref()
        .is_some_and(|meta| !meta["exit_code"].is_null())
    {
        return true;
    }
    // Only a readable procfs answer decides: `Ok(false)` proves the owner
    // is gone, `Ok(true)` proves it may be alive, and an error — a hardened
    // mount, a permission failure — falls through to the grace period
    // rather than reading as an exited owner.
    if let Some(pid) = meta.as_ref().and_then(|meta| meta["pid"].as_u64()) {
        if std::path::Path::new("/proc/self").is_dir() {
            match std::path::Path::new(&format!("/proc/{pid}")).try_exists() {
                Ok(true) => return false,
                Ok(false) => return true,
                Err(_) => {}
            }
        }
    }
    fs::metadata(dir)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > UNFINISHED_GRACE)
}

/// A fresh run id: the UTC timestamp, then a suffix from the clock's
/// nanoseconds and the process id, so two concurrent runs on one host get
/// distinct directories.
fn new_run_id() -> String {
    let stamp = applog::now_utc().replace(':', "-");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!(
        "{stamp}-{:08x}",
        u64::from(nanos) ^ (u64::from(std::process::id()) << 20)
    )
}

/// 0700 on the run directory: the journal can hold command transcripts.
fn restrict_dir(dir: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// 0600 on a journal file: data, not an executable.
fn restrict_file(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{META_SCHEMA, Meta, ScriptRecord, SecretHandling};

    /// The `rk.run-meta/1` schema, held by snapshot.
    #[test]
    fn the_meta_schema_snapshot_holds() {
        let meta = Meta {
            schema: META_SCHEMA,
            run_id: "2026-08-29T14-02-11Z-0000abcd".into(),
            rk_version: "0.1.0",
            command: "setup".into(),
            argv: vec!["setup".into(), "--target".into(), ".".into()],
            pid: 4242,
            target: ".".into(),
            forge: "github".into(),
            repo: "acme/widget".into(),
            started: "2026-08-29T14:02:11Z".into(),
            ended: Some("2026-08-29T14:02:12Z".into()),
            exit_code: Some(0),
            reason: None,
            scripts: vec![ScriptRecord {
                path: "scripts/github/default-branch".into(),
                sha256: "ab".into(),
            }],
            secrets: vec![SecretHandling {
                secret: "RK_BOT_PRIVATE_KEY".into(),
                present: true,
                source: "environment",
                transport: "stdin",
                redacted: true,
            }],
        };
        assert_eq!(
            serde_json::to_string(&meta).expect("meta serializes"),
            r#"{"schema":"rk.run-meta/1","run_id":"2026-08-29T14-02-11Z-0000abcd","rk_version":"0.1.0","command":"setup","argv":["setup","--target","."],"pid":4242,"target":".","forge":"github","repo":"acme/widget","started":"2026-08-29T14:02:11Z","ended":"2026-08-29T14:02:12Z","exit_code":0,"scripts":[{"path":"scripts/github/default-branch","sha256":"ab"}],"secrets":[{"secret":"RK_BOT_PRIVATE_KEY","present":true,"source":"environment","transport":"stdin","redacted":true}]}"#
        );
    }
}
