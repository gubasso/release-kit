//! The bot credentials a setup step consumes, and what the environment is
//! allowed to carry.
//!
//! An identifier and a short-lived token are values: the forge CLIs' own
//! convention carries them in the environment, and rotating one is a
//! command. Key material is not. An App private key downloads exactly once,
//! lives until a browser replaces it, and an environment is a poor vault
//! for it: the block is readable at `/proc/<pid>/environ`, and every later
//! child of that shell inherits it. So the
//! operator names the key's path to `rk`, and `rk` reads the file and
//! writes the bytes to the step's standard input. The path goes no further
//! than `rk`: no child is told it, so no child can open it. Where the
//! path comes from is [`key_path`]'s answer.
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

use super::key_path::{self, Source};
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
    /// Which source named the path.
    pub source: Source,
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

/// The validated private key file, where a source names one.
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
pub fn resolve_key_file(
    target: &Utf8Path,
    app_id: Option<&str>,
) -> Result<Option<KeyFile>, RkError> {
    match look_up_key_file(target, app_id)? {
        KeyLookup::Absent(_) => Ok(None),
        KeyLookup::Found(key) => Ok(Some(key)),
        KeyLookup::Unavailable(unavailable) => Err(unavailable.refusal()),
    }
}

/// What the key lookup turned out to be.
pub enum KeyLookup {
    /// No key is there to read: no source names a file, or nothing sits
    /// at the default path yet. The text names what to do.
    Absent(String),
    /// A named file the current process cannot read: a path a source
    /// named that does not exist, or a file it may not open or read.
    /// Under a read-only check that is an unmet requirement with a
    /// remedy, not a defect.
    Unavailable(Unavailable),
    /// A readable, owner-only PEM private key.
    Found(KeyFile),
}

/// A named key file the current process cannot read, with the refusal a
/// mutating run gives for it.
pub struct Unavailable {
    /// The path, as resolved as far as it could be.
    pub path: String,
    /// Why it cannot be read, one line.
    pub reason: String,
    /// Which source named the path.
    pub source: Source,
    action: &'static str,
}

impl Unavailable {
    /// What was found, one line.
    #[must_use]
    pub fn message(&self) -> String {
        format!("{} is unreadable: {}", self.path, self.reason)
    }

    /// The unmet requirement and its remedy, as a check reports it.
    ///
    /// SATISFIES bot-key:an-unreadable-key-is-an-unmet-requirement
    #[must_use]
    pub fn requirement(&self) -> String {
        format!(
            "bot private key not readable at {} ({}): import it with `rk setup import-key <file>`, or point {PRIVATE_KEY_FILE} at it",
            self.path, self.reason
        )
    }

    /// The refusal a run that needs the key gives.
    #[must_use]
    pub fn refusal(self) -> RkError {
        refuse(self.message(), self.action)
    }
}

/// What a missing default key says to do.
#[must_use]
pub fn import_remedy(path: &Utf8Path) -> String {
    format!(
        "no bot private key at {path}: import the App's .pem with `rk setup import-key <file>`, or point {PRIVATE_KEY_FILE} at it"
    )
}

/// What a missing App ID says to do.
pub const NO_APP_ID: &str = "no bot App ID is resolved, so no key path can be named: set setup.bot.app_id in .release-kit/config.toml, or export RK_BOT_APP_ID";

/// Look the key file up by the fixed order of its sources, and tell a
/// file the current process cannot read from a file that is wrong.
///
/// A wrong file is refused in every mode: an unsafe mode, a directory or
/// a pipe, an oversized or empty file, a file inside the target, or bytes
/// that are not a PEM private key. A file that cannot be read is
/// [`KeyLookup::Unavailable`], and the caller decides what that means.
/// Nothing at the default path is [`KeyLookup::Absent`]: no source named
/// that file, so a missing one means no key was imported yet.
///
/// SATISFIES setup-proof:an-unavailable-credential-is-an-observation-boundary
/// SATISFIES bot-key:three-sources-name-the-key-in-a-fixed-order
///
/// # Errors
///
/// Refuses a stale contents variable, a wrong user configuration file,
/// and every wrong key file, as [`resolve_key_file`] names them.
pub fn look_up_key_file(target: &Utf8Path, app_id: Option<&str>) -> Result<KeyLookup, RkError> {
    refuse_legacy_key()?;
    let named = match key_path::resolve(app_id)? {
        key_path::Resolution::NoAppId => return Ok(KeyLookup::Absent(NO_APP_ID.to_owned())),
        key_path::Resolution::Named(named) => named,
    };
    let source = named.source;
    let path = match resolve_path(&named.raw, target)? {
        Resolved::Path(path) => path,
        Resolved::Missing(path) if source == Source::Default => {
            return Ok(KeyLookup::Absent(import_remedy(&path)));
        }
        Resolved::Missing(path) => {
            return Ok(KeyLookup::Unavailable(Unavailable {
                path: path.into_string(),
                reason: "no such file".to_owned(),
                source,
                action: "name an existing .pem",
            }));
        }
        Resolved::Unreachable { path, reason } => {
            return Ok(KeyLookup::Unavailable(Unavailable {
                path: path.into_string(),
                reason,
                source,
                action: "name an existing .pem",
            }));
        }
    };
    read_key(path, source, true)
}

/// Read and judge the key at a resolved path. `owner_only` asks for the
/// mode check, which every lookup makes and an import of a fresh download
/// skips.
pub(crate) fn read_key(
    path: Utf8PathBuf,
    source: Source,
    owner_only: bool,
) -> Result<KeyLookup, RkError> {
    let unavailable = |path: &Utf8PathBuf, reason: String, action: &'static str| {
        Ok(KeyLookup::Unavailable(Unavailable {
            path: path.to_string(),
            reason,
            source,
            action,
        }))
    };

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
    let file = match options.open(&path) {
        Ok(file) => file,
        Err(err) => return unavailable(&path, err.to_string(), "name an existing .pem"),
    };
    let meta = match file.metadata() {
        Ok(meta) => meta,
        Err(err) => return unavailable(&path, err.to_string(), "name an existing .pem"),
    };

    // rk reads this handle and hands its bytes on, so a source that yields
    // them once, or has none of its own, is wrong here.
    if !meta.is_file() {
        return Err(refuse(
            format!("{path} is not a regular file"),
            "name the .pem itself, not a directory, a device, or a pipe",
        ));
    }

    #[cfg(unix)]
    if owner_only {
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
    #[cfg(not(unix))]
    let _ = owner_only;

    // Bounded by the read itself rather than by the length the metadata
    // reported: one byte past the cap is enough to know, and the cap holds
    // even where a handle's reported length and its contents disagree.
    let mut bytes = Zeroizing::new(Vec::new());
    if let Err(err) = file.take(MAX_KEY_BYTES + 1).read_to_end(&mut bytes) {
        return unavailable(&path, err.to_string(), "name a readable .pem");
    }
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

    Ok(KeyLookup::Found(KeyFile {
        path,
        bytes,
        source,
    }))
}

/// A named path, resolved, or a name the current process cannot reach.
pub(crate) enum Resolved {
    Path(Utf8PathBuf),
    /// Nothing exists at the path.
    Missing(Utf8PathBuf),
    /// The path cannot be resolved for another reason.
    Unreachable {
        path: Utf8PathBuf,
        reason: String,
    },
}

/// The canonical path a source named, refused where the name itself is
/// wrong: before anything is opened, and before a diagnostic could leak
/// what the file holds.
pub(crate) fn resolve_path(raw: &OsString, target: &Utf8Path) -> Result<Resolved, RkError> {
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

    let path = match std::fs::canonicalize(&named) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Resolved::Missing(named));
        }
        Err(err) => {
            return Ok(Resolved::Unreachable {
                path: named,
                reason: err.to_string(),
            });
        }
    };
    let Ok(path) = Utf8PathBuf::from_path_buf(path) else {
        return Err(refuse(
            format!("{named} resolves to a path that is not valid UTF-8"),
            "name the .pem by a UTF-8 path",
        ));
    };

    // A key inside the repository is one `git add .` from being published.
    if let Ok(inside) = std::fs::canonicalize(target)
        && path.as_std_path().starts_with(&inside)
    {
        return Err(refuse(
            format!("{path} is inside the repository being set up"),
            "keep the .pem outside the working tree",
        ));
    }

    Ok(Resolved::Path(path))
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
    if text.is_empty() || !text.len().is_multiple_of(4) {
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
            .expected("a readable, owner-only PEM private key at the bot key path")
            .action(action)
            .step("bot-secrets"),
    )
}

#[cfg(test)]
mod tests {
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
            source: Source::Default,
        };
        let rendered = format!("{key:?}");
        assert!(rendered.contains("/keys/bot.pem"));
        assert!(!rendered.contains("BEGIN"));
    }
}
