//! The bot credentials a setup step consumes, and what the operator's
//! environment is allowed to carry.
//!
//! An identifier and a short-lived token are values: the forge CLIs' own
//! convention carries them in the environment, and rotating one is a
//! command. Key material is not. An App private key downloads exactly once,
//! lives until a browser replaces it, and an environment is a poor vault
//! for it: the block is readable at `/proc/<pid>/environ`, and every later
//! child of that shell inherits it. So the
//! operator names the key's path to `rk`, and `rk` reads the file and
//! writes the bytes to the step's standard input. The path goes no further
//! than `rk`: no child is told it, so no child can open it.
//!
//! `rk` reads the file exactly once: to refuse a wrong one before anything
//! is written to the forge, to hold the redaction needle that keeps the
//! journal's `redacted` claim honest, and to be the bytes the step sends.
//! One read means the file that was validated is the file that is stored —
//! nothing between the check and the forge can substitute another. That
//! read lands in a [`Zeroizing`] buffer, scrubbed on drop, and is never
//! exported, echoed, or recorded.

use std::ffi::OsString;
use std::io::Read as _;

use camino::{Utf8Path, Utf8PathBuf};
use zeroize::Zeroizing;

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// The variable that once carried the key's contents. It is refused now,
/// rather than ignored: a stale export is the leak this module exists to
/// end, and silence would let it stand.
pub const LEGACY_PRIVATE_KEY: &str = "RK_BOT_PRIVATE_KEY";

/// The variable naming the App private key file.
pub const PRIVATE_KEY_FILE: &str = "RK_BOT_PRIVATE_KEY_FILE";

/// The variables whose value the environment may carry: an App identifier,
/// which the App's settings page shows, and a project access token, which
/// the forge mints and a command rotates.
pub const VALUE_VARS: [&str; 2] = ["RK_BOT_APP_ID", "RK_BOT_TOKEN"];

/// The largest file this accepts as a private key. An App key is a few
/// kilobytes; the cap is what stops a mistyped path from being slurped.
const MAX_KEY_BYTES: u64 = 64 * 1024;

/// A validated private key file.
///
/// The bytes are what the step transmits and what the redactor holds. No
/// consumer ever learns the path, which is why the path here serves a
/// diagnostic and nothing else.
pub struct KeyFile {
    /// The canonical path, for a diagnostic that must name the file.
    pub path: Utf8PathBuf,
    /// The file's bytes, scrubbed when this is dropped.
    pub bytes: Zeroizing<Vec<u8>>,
}

impl std::fmt::Debug for KeyFile {
    /// The path only: a derived `Debug` would print key material into any
    /// log that formats a context.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyFile")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// The environment's value for `name`, absent when unset or empty.
#[must_use]
pub fn value_of(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

/// Refuse a stale `RK_BOT_PRIVATE_KEY` export wherever a run starts, so
/// `rk setup`, `rk setup step`, and `rk setup check` all catch it rather
/// than one step alone.
///
/// # Errors
///
/// Refuses when the environment carries the key's contents.
pub fn refuse_legacy_key() -> Result<(), RkError> {
    if value_of(LEGACY_PRIVATE_KEY).is_none() {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!("{LEGACY_PRIVATE_KEY} carries key material"),
        )
        .expected("the key's path in the environment, never the key's contents")
        .action(format!(
            "unset {LEGACY_PRIVATE_KEY}, then export {PRIVATE_KEY_FILE} with the path to the .pem"
        )),
    ))
}

/// The validated private key file, where the operator named one.
///
/// Every refusal happens before the step spawns, so a wrong path, a wrong
/// mode, or a wrong encoding never reaches the forge. What the encoding
/// wraps is the forge's judgment: this refuses a file that is not a PEM
/// private key, not a key the forge would reject.
///
/// # Errors
///
/// Refuses a stale contents variable, and a named file that is missing,
/// unreadable, not a regular file, readable beyond its owner, empty,
/// oversized, inside the target, or not a PEM private key.
pub fn resolve_key_file(target: &Utf8Path) -> Result<Option<KeyFile>, RkError> {
    refuse_legacy_key()?;
    let Some(raw) = value_of(PRIVATE_KEY_FILE) else {
        return Ok(None);
    };
    let path = resolve_path(&raw, target)?;

    // One handle answers every question that follows. Asking the path twice
    // — once for metadata, once for contents — would let a replacement
    // satisfy the checks with one object and supply the bytes of another;
    // what is checked here is what is read here.
    //
    // The open must not block, because what it opens is not yet known to be
    // a file: a FIFO with no writer would hang the run instead of earning
    // the refusal below. `O_NONBLOCK` changes nothing for the regular file
    // this is supposed to be, and its value comes from the target's own ABI
    // — it varies by architecture, not only by operating system.
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(&path).map_err(|err| {
        refuse(
            format!("{path} is unreadable: {err}"),
            "name an existing .pem",
        )
    })?;
    let meta = file.metadata().map_err(|err| {
        refuse(
            format!("{path} is unreadable: {err}"),
            "name an existing .pem",
        )
    })?;

    // rk reads this handle and hands its bytes on, so a source that yields
    // them once, or has none of its own, is wrong here.
    if !meta.is_file() {
        return Err(refuse(
            format!("{path} is not a regular file"),
            "name the .pem itself, not a directory, a device, or a pipe",
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(refuse(
                format!(
                    "{path} is readable by group or other ({:04o})",
                    mode & 0o7777
                ),
                format!("chmod 600 {path}"),
            ));
        }
    }

    // Bounded by the read itself rather than by the length the metadata
    // reported: one byte past the cap is enough to know, and the cap holds
    // even where a handle's reported length and its contents disagree.
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(MAX_KEY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| {
            refuse(
                format!("{path} is unreadable: {err}"),
                "name a readable .pem",
            )
        })?;
    if bytes.len() as u64 > MAX_KEY_BYTES {
        return Err(refuse(
            format!("{path} is larger than {MAX_KEY_BYTES} bytes"),
            "name the .pem itself; a private key is a few kilobytes",
        ));
    }
    if bytes.is_empty() {
        return Err(refuse(
            format!("{path} is empty"),
            "name the downloaded .pem",
        ));
    }
    if !is_private_key_pem(&bytes) {
        return Err(refuse(
            format!("{path} is not a PEM-encoded private key"),
            "name the key the App's settings page downloaded, not a public key or an id",
        ));
    }

    Ok(Some(KeyFile { path, bytes }))
}

/// The canonical path the operator named, refused where the name itself is
/// wrong: before anything is opened, and before a diagnostic could leak
/// what the file holds.
fn resolve_path(raw: &OsString, target: &Utf8Path) -> Result<Utf8PathBuf, RkError> {
    let Ok(named) = Utf8PathBuf::from_path_buf(raw.clone().into()) else {
        return Err(refuse(
            format!("{PRIVATE_KEY_FILE} is not valid UTF-8"),
            "name the .pem by a UTF-8 path",
        ));
    };

    // A quoted `export RK_BOT_PRIVATE_KEY_FILE="~/key.pem"` leaves the tilde
    // for a program to expand, and no program does.
    if named.as_str().starts_with('~') {
        return Err(refuse(
            format!("{named} begins with an unexpanded tilde"),
            "name the .pem by an absolute path, or leave the tilde unquoted for the shell",
        ));
    }

    let path = std::fs::canonicalize(&named).map_err(|err| {
        refuse(
            format!("{named} is unreadable: {err}"),
            "name an existing .pem",
        )
    })?;
    let Ok(path) = Utf8PathBuf::from_path_buf(path) else {
        return Err(refuse(
            format!("{named} resolves to a path that is not valid UTF-8"),
            "name the .pem by a UTF-8 path",
        ));
    };

    // A key inside the repository is one `git add .` from being published.
    if let Ok(inside) = std::fs::canonicalize(target) {
        if path.as_std_path().starts_with(&inside) {
            return Err(refuse(
                format!("{path} is inside the repository being set up"),
                "keep the .pem outside the working tree",
            ));
        }
    }

    Ok(path)
}

/// Whether the bytes are the RFC 7468 textual encoding of a private key.
///
/// The check is the encoding, not the key: `rk` stores the file and the
/// forge parses it, so nothing here decodes a key or judges an algorithm.
/// What it does assert is everything RFC 7468 gives — a begin line whose
/// label ends in `PRIVATE KEY`, base64 between the boundaries, and an end
/// line carrying the same label. That grammar admits no header fields, so
/// neither does this. Anything less accepts a file holding the right
/// markers around the wrong content, and the forge would store it happily;
/// the failure would surface as a release that cannot authenticate, weeks
/// later.
fn is_private_key_pem(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut lines = text.lines().map(str::trim);
    let Some(label) = lines.find_map(|line| boundary_label(line, "BEGIN")) else {
        return false;
    };
    if !label.ends_with("PRIVATE KEY") {
        return false;
    }
    let mut body = String::new();
    for line in lines {
        if let Some(end) = boundary_label(line, "END") {
            return end == label && is_base64(&body);
        }
        body.push_str(line);
    }
    false
}

/// Whether `text` is non-empty base64, in the alphabet and padding RFC 4648
/// gives: a multiple of four characters, padding only at the end, and at
/// most two padding characters.
fn is_base64(text: &str) -> bool {
    if text.is_empty() || text.len() % 4 != 0 {
        return false;
    }
    let payload = text.trim_end_matches('=');
    if text.len() - payload.len() > 2 {
        return false;
    }
    payload
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/')
}

/// The label of a `-----BEGIN <label>-----` or `-----END <label>-----`
/// line, where the line is exactly one and its label is non-empty.
fn boundary_label<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let label = line
        .strip_prefix("-----")?
        .strip_suffix("-----")?
        .strip_prefix(keyword)?
        .strip_prefix(' ')?;
    (!label.is_empty() && !label.contains('-')).then_some(label)
}

/// One refusal, in this module's shape.
fn refuse(message: impl Into<String>, action: impl Into<String>) -> RkError {
    RkError::refusal(
        Diagnostic::new(Reason::PrerequisiteUnmet, message)
            .expected(format!(
                "{PRIVATE_KEY_FILE} naming a readable, owner-only PEM private key"
            ))
            .action(action)
            .step("bot-secrets"),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// PEM armor around `label`, assembled rather than written out: a
    /// literal header here is what the repository's own private-key scan
    /// is for, and it should keep firing on real ones.
    fn armored(label: &str) -> Vec<u8> {
        format!("-----BEGIN {label}-----\n{BODY}\n-----END {label}-----\n").into_bytes()
    }

    /// A base64 body, which RFC 7468 requires between the boundaries.
    const BODY: &str = "c2VrcmV0LXBlbS1ieXRlcyE=";

    #[test]
    fn armor_is_the_shape_the_check_accepts() {
        assert!(is_private_key_pem(&armored("RSA PRIVATE KEY")));
        assert!(is_private_key_pem(&armored("PRIVATE KEY")));
        assert!(is_private_key_pem(&armored("ENCRYPTED PRIVATE KEY")));
        assert!(!is_private_key_pem(&armored("PUBLIC KEY")));
        assert!(!is_private_key_pem(&armored("CERTIFICATE")));
        assert!(!is_private_key_pem(b"314159\n"));
        assert!(!is_private_key_pem(&[0xff, 0xfe, 0x00]));
    }

    #[test]
    fn armor_that_is_only_the_two_markers_is_refused() {
        // The markers in any arrangement are not armor: a boundary is a
        // whole line, the labels must match, and something must sit
        // between them.
        let begin = |label: &str| format!("-----BEGIN {label}-----");
        let end = |label: &str| format!("-----END {label}-----");
        let key = "PRIVATE KEY";

        let split_marker = format!("-----BEGIN\n{key}-----\n{BODY}\n");
        assert!(!is_private_key_pem(split_marker.as_bytes()));

        let mismatched = format!("{}\n{BODY}\n{}\n", begin("RSA PRIVATE KEY"), end(key));
        assert!(!is_private_key_pem(mismatched.as_bytes()));

        let unterminated = format!("{}\n{BODY}\n", begin(key));
        assert!(!is_private_key_pem(unterminated.as_bytes()));

        let bodyless = format!("{}\n{}\n", begin(key), end(key));
        assert!(!is_private_key_pem(bodyless.as_bytes()));

        let inline = format!("a {} inline\n{BODY}\n{}\n", begin(key), end(key));
        assert!(!is_private_key_pem(inline.as_bytes()));
    }

    #[test]
    fn a_body_that_is_not_base64_is_refused() {
        // Matching boundaries around arbitrary text are not a key, and the
        // forge would store them without complaint.
        let key = "PRIVATE KEY";
        let wrap = |body: &str| {
            format!("-----BEGIN {key}-----\n{body}\n-----END {key}-----\n").into_bytes()
        };
        assert!(!is_private_key_pem(&wrap("x")));
        assert!(!is_private_key_pem(&wrap("sekret-pem-bytes")));
        assert!(!is_private_key_pem(&wrap("c2Vrcm V0")));
        assert!(!is_private_key_pem(&wrap("c2VrcmV0=b")));
        assert!(is_private_key_pem(&wrap(BODY)));
        // A wrapped body joins into one base64 string, as an encoder emits.
        assert!(is_private_key_pem(&wrap("c2Vrcm\nV0LXBl\nbS1ieXRlcyE=")));
        // RFC 7468's grammar admits no header fields, so neither does this;
        // a colon buys a line nothing.
        assert!(!is_private_key_pem(&wrap(&format!(
            "Proc-Type: 4,ENCRYPTED\n{BODY}"
        ))));
        assert!(!is_private_key_pem(&wrap("garbage:\nstill-garbage:\nQUJD")));
        assert!(!is_private_key_pem(&wrap(&format!("empty:\n{BODY}"))));
    }

    #[test]
    fn a_key_file_debug_prints_no_key_material() {
        let key = KeyFile {
            path: Utf8PathBuf::from("/keys/bot.pem"),
            bytes: Zeroizing::new(armored("PRIVATE KEY")),
        };
        let rendered = format!("{key:?}");
        assert!(rendered.contains("/keys/bot.pem"));
        assert!(!rendered.contains("BEGIN"));
    }
}
