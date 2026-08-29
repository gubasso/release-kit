//! The application log: one rotating file for the whole binary.
//!
//! One-line logfmt records at the XDG state root, filtered by `RUST_LOG`
//! because the ecosystem already owns that variable, rotated by size. The
//! file is the only default destination — the terminal never sees log
//! records — and a log write that fails costs nothing but the record: a
//! diagnostic surface must never fail the command it observes.

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rotate when the log grows past this many bytes; one previous file is
/// kept as `release-kit.log.1`.
const ROTATE_AT: u64 = 1_048_576;

/// The state directory every run-shaped artifact lives under:
/// `${XDG_STATE_HOME:-$HOME/.local/state}/release-kit`.
#[must_use]
pub fn state_root() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(base.join("release-kit"))
}

/// Append one info record for a finished command, best effort.
pub fn record(op: &str, status: &str, dur_ms: u128) {
    if !info_enabled(std::env::var("RUST_LOG").ok().as_deref()) {
        return;
    }
    let Some(root) = state_root() else { return };
    write_record(&root, op, status, dur_ms);
}

/// Append the record under `root`, rotating first when the file is past
/// the size bound. Every failure is swallowed: the log is best effort.
fn write_record(root: &std::path::Path, op: &str, status: &str, dur_ms: u128) {
    let path = root.join("release-kit.log");
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > ROTATE_AT) {
        let _ = fs::rename(&path, root.join("release-kit.log.1"));
    }
    let line = format!(
        "ts={} level=info target=rk op={op} status={status} dur_ms={dur_ms}\n",
        now_utc()
    );
    let _ = fs::create_dir_all(root);
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(line.as_bytes()));
}

/// Whether `RUST_LOG` allows info records: the last directive naming this
/// binary or naming no target decides, and no variable means info.
fn info_enabled(rust_log: Option<&str>) -> bool {
    let Some(spec) = rust_log else { return true };
    let mut enabled = true;
    for directive in spec.split(',') {
        let (target, level) = directive
            .split_once('=')
            .map_or((None, directive), |(target, level)| (Some(target), level));
        if target.is_some_and(|t| {
            let t = t.trim();
            t != "rk" && t != "release_kit"
        }) {
            continue;
        }
        match level.trim().to_ascii_lowercase().as_str() {
            "off" | "error" | "warn" => enabled = false,
            "info" | "debug" | "trace" => enabled = true,
            _ => {}
        }
    }
    enabled
}

/// Wall-clock UTC now, RFC 3339 to the second.
fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    rfc3339(i64::try_from(secs).unwrap_or(0))
}

/// Format seconds since the epoch as RFC 3339 UTC.
fn rfc3339(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let in_day = epoch_secs.rem_euclid(86_400);
    let (year, month, day) = ymd_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        in_day / 3600,
        in_day % 3600 / 60,
        in_day % 60
    )
}

/// Civil date from days since the epoch, by the standard era arithmetic.
const fn ymd_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_point = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_point + 2) / 5 + 1;
    let month = if month_point < 10 {
        month_point + 3
    } else {
        month_point - 9
    };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{info_enabled, rfc3339, write_record};

    #[test]
    fn the_timestamp_matches_known_epochs() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        // date -u -d @1793289600: 2026-10-29 16:00:00 UTC.
        assert_eq!(rfc3339(1_793_289_600), "2026-10-29T16:00:00Z");
        // A leap day: date -u -d @1709164800 is 2024-02-29.
        assert_eq!(rfc3339(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn rust_log_filters_info_records() {
        assert!(info_enabled(None));
        assert!(info_enabled(Some("info")));
        assert!(info_enabled(Some("debug")));
        assert!(!info_enabled(Some("off")));
        assert!(!info_enabled(Some("warn")));
        assert!(!info_enabled(Some("error")));
        assert!(info_enabled(Some("other_crate=off")));
        assert!(!info_enabled(Some("rk=warn")));
        assert!(info_enabled(Some("warn,rk=info")));
    }

    #[test]
    fn a_record_appends_one_logfmt_line() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        write_record(dir.path(), "init", "ok", 12);
        write_record(dir.path(), "skill", "state-drift", 3);
        let text =
            std::fs::read_to_string(dir.path().join("release-kit.log")).expect("the log reads");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].contains("level=info target=rk op=init status=ok dur_ms=12"),
            "{}",
            lines[0]
        );
        assert!(lines[0].starts_with("ts="), "{}", lines[0]);
    }
}
