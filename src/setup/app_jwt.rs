//! Authenticating to the forge as the bot App itself.
//!
//! GitHub serves the installation-reading endpoints to App credentials
//! only: `GET /repos/{owner}/{repo}/installation` takes a JWT signed with
//! the App's private key, and no personal access token of any class is
//! accepted there. So the `install-bot` step observes as the App: `rk`
//! builds the RS256 signing input, has the OpenSSL CLI sign it with the
//! key bytes on standard input, and carries the resulting token to the
//! forge through `curl`, in a header read from standard input — the JWT
//! is a credential, and `forge-setup:a-secret-never-reaches-argv` binds
//! it like any other.
//!
//! Both spawns deliberately bypass the run's journaling executor: the
//! executor records child output, the signer's output is the token's
//! third segment, and a `curl` made verbose by a host configuration would
//! echo the very header it was handed. Neither child's streams reach a
//! journal, an event, or a transcript; the answers surface only as the
//! classified [`AppApi`] and the step states built from it.

use std::ffi::OsString;
use std::io::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use zeroize::Zeroizing;

use super::context::Ctx;
use super::process::{self, Exec};
use super::secrets;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// What exporting the credentials would enable, named wherever they are
/// absent.
pub const REMEDIATION: &str = "export RK_BOT_APP_ID and RK_BOT_PRIVATE_KEY_FILE, the second naming the App's .pem, or verify the installation by eye at github.com/settings/installations";

/// The two halves of the App identity. The caller resolves both — the id
/// from the environment, the key from the run's one read of the named
/// file — so this module never opens anything itself.
pub struct AppCredentials {
    /// The numeric App id, which becomes the token's `iss`.
    pub app_id: String,
    /// The validated private key's bytes.
    pub key_bytes: Zeroizing<Vec<u8>>,
}

/// The App id from the environment, absent when unset.
///
/// # Errors
///
/// Refuses an `RK_BOT_APP_ID` that is not the numeric id — the value
/// lands in a JSON claim, so anything else would sign a malformed token.
pub fn app_id() -> Result<Option<String>, RkError> {
    let Some(app_id) = secrets::value_of("RK_BOT_APP_ID") else {
        return Ok(None);
    };
    let numeric = app_id
        .to_str()
        .filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()));
    let Some(app_id) = numeric else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                "RK_BOT_APP_ID is not a numeric App id",
            )
            .expected("the App ID from the App's settings page, digits only")
            .action("copy the App ID, not the Client ID; the setup guide's step 5 collects it")
            .step("install-bot"),
        ));
    };
    Ok(Some(app_id.to_owned()))
}

/// Mint a short-lived RS256 JWT for the App: `iss` is the App id, `iat`
/// sits sixty seconds back against clock drift, and `exp` nine minutes
/// out, inside the forge's ten-minute cap.
///
/// # Errors
///
/// A failure is a one-line detail for the caller to report — an
/// observation maps it to `unknown`, an apply to a refusal — never a
/// token that might be wrong.
pub fn mint(ctx: &Ctx, credentials: &AppCredentials) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "the system clock is before the epoch".to_owned())?
        .as_secs();
    let header = base64url(br#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = base64url(
        format!(
            r#"{{"iat":{},"exp":{},"iss":"{}"}}"#,
            now.saturating_sub(60),
            now + 540,
            credentials.app_id
        )
        .as_bytes(),
    );
    let input = format!("{header}.{claims}");
    let signature = sign(ctx, credentials, input.as_bytes())?;
    Ok(format!("{input}.{}", base64url(&signature)))
}

/// The minimal environment a helper child receives: the search path and
/// nothing else. Each helper holds one credential on its standard input,
/// and an environment carrying any other — the forge CLI tokens the
/// setup's own children inherit — would put that one within reach of a
/// helper's error stream, which only the helper's own credential is
/// scrubbed against.
fn helper_env() -> Vec<(OsString, OsString)> {
    std::env::var_os("PATH")
        .map(|path| vec![(OsString::from("PATH"), path)])
        .unwrap_or_default()
}

/// What the carrying child adds to that: the variables naming where this
/// host keeps its certificate authorities.
///
/// They earn their exception by naming public files, and only the carrier
/// takes them. A `curl` that locates its trust store by environment rather
/// than by a compiled-in path — a Nix-provided one on a host whose
/// distribution keeps its own bundle elsewhere is the ordinary case —
/// verifies no certificate at all once they are cleared, and the call
/// fails before the forge answers. The signer receives none of them: it
/// opens no connection and has nothing to verify. Proxy variables stay
/// out of both, because a proxy URL can carry a credential of its own,
/// which is the one thing these environments must not hold.
fn carrier_env() -> Vec<(OsString, OsString)> {
    const TRUST: [&str; 4] = [
        "CURL_CA_BUNDLE",
        "SSL_CERT_DIR",
        "SSL_CERT_FILE",
        "NIX_SSL_CERT_FILE",
    ];
    let mut env = helper_env();
    env.extend(
        TRUST
            .iter()
            .filter_map(|name| std::env::var_os(name).map(|value| (OsString::from(*name), value))),
    );
    env
}

/// Sign the token's input with the App key, through the OpenSSL CLI: the
/// key bytes travel on standard input, the non-secret signing input as a
/// private scratch file, and no child is told the key's path.
fn sign(ctx: &Ctx, credentials: &AppCredentials, input: &[u8]) -> Result<Vec<u8>, String> {
    let scratch = scratch_input(input)?;
    let program = std::env::var_os("RK_OPENSSL_BIN").unwrap_or_else(|| "openssl".into());
    let exec = Exec {
        program,
        args: [
            "dgst",
            "-sha256",
            "-binary",
            "-sign",
            "/dev/stdin",
            scratch.file.as_str(),
        ]
        .map(OsString::from)
        .to_vec(),
        env: helper_env(),
        cwd: ctx.target.as_std_path().to_path_buf(),
        stdin: Some(credentials.key_bytes.clone()),
    };
    // Chunks are dropped as they stream: the signature is credential
    // material, and nothing here may record it.
    let outcome = process::run(&exec, |_, _| {})
        .map_err(|source| format!("openssl did not spawn: {source}; install OpenSSL"))?;
    if !outcome.success() {
        // The child held the key on its standard input, so its error
        // stream is scrubbed against the key bytes before one line of it
        // can reach a diagnostic or a journal.
        let stderr = process::redact(
            &outcome.stderr,
            std::slice::from_ref(&credentials.key_bytes),
        );
        return Err(format!(
            "openssl could not sign the App JWT: {}",
            last_line(&stderr)
        ));
    }
    if outcome.stdout.is_empty() {
        return Err("openssl signed the App JWT to an empty signature".to_owned());
    }
    Ok(outcome.stdout)
}

/// The signing input, written under a fresh owner-only scratch directory
/// that lives exactly as long as the signing spawn. The bytes are not
/// secret — an algorithm, two timestamps, and the public App id — but the
/// directory is 0700 anyway, because a looser scratch is a habit.
struct ScratchInput {
    dir: std::path::PathBuf,
    file: camino::Utf8PathBuf,
}

impl Drop for ScratchInput {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn scratch_input(input: &[u8]) -> Result<ScratchInput, String> {
    let dir = std::env::temp_dir().join(format!("rk-app-jwt-{}", std::process::id()));
    let make = || -> std::io::Result<()> {
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    };
    make().map_err(|source| format!("no scratch directory for the JWT input: {source}"))?;
    let path = dir.join("signing-input");
    std::fs::write(&path, input)
        .map_err(|source| format!("the JWT input did not write: {source}"))?;
    let Ok(file) = camino::Utf8PathBuf::from_path_buf(path) else {
        return Err("the scratch path is not valid UTF-8".to_owned());
    };
    Ok(ScratchInput { dir, file })
}

/// One read-only forge answer, asked as the App itself.
pub enum AppApi {
    /// The call succeeded and parsed.
    Ok(Value),
    /// The forge answered 404: the thing is not there.
    Missing,
    /// The forge refused the App credentials.
    Refused(String),
    /// The call failed for another reason.
    Failed(String),
}

/// `GET https://api.github.com/{path}` with the JWT as a bearer token.
///
/// The call goes through `curl`, spawned directly rather than through the
/// run's executor so no stream of it can be journaled: the Authorization
/// header arrives on standard input via `-H @-`, so the token never
/// reaches an argument list, `-q` leads the arguments so no `.curlrc` can
/// turn on an echoing verbosity or reshape the call, and the trailing
/// `-w` line carries the status the answer is classified by.
#[must_use]
pub fn api_get(ctx: &Ctx, jwt: &str, path: &str) -> AppApi {
    let mut headers = Zeroizing::new(Vec::new());
    let _ = write!(
        headers,
        "Authorization: Bearer {jwt}\nAccept: application/vnd.github+json\nX-GitHub-Api-Version: 2022-11-28\n"
    );
    let program = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let exec = Exec {
        program,
        args: [
            "-q",
            "-sS",
            "--max-time",
            "10",
            "-H",
            "@-",
            "-w",
            "\n%{http_code}",
            &format!("https://api.github.com/{path}"),
        ]
        .map(OsString::from)
        .to_vec(),
        env: carrier_env(),
        cwd: ctx.target.as_std_path().to_path_buf(),
        stdin: Some(headers),
    };
    let outcome = match process::run(&exec, |_, _| {}) {
        Ok(outcome) => outcome,
        Err(source) => {
            return AppApi::Failed(format!("curl did not spawn: {source}; install curl"));
        }
    };
    // The child held the bearer header on its standard input, so both of
    // its streams are scrubbed against the token and its signature before
    // one line of either can reach a diagnostic or an output stream.
    let needles = [
        jwt.as_bytes(),
        jwt.rsplit('.').next().unwrap_or(jwt).as_bytes(),
    ];
    let stderr = process::redact(&outcome.stderr, &needles);
    let stdout = process::redact(&outcome.stdout, &needles);
    if !outcome.success() {
        return AppApi::Failed(format!(
            "curl could not reach the forge (exit {}): {}",
            outcome.exit_code,
            first_line(&stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&stdout);
    let (body, status) = stdout
        .trim_end()
        .rsplit_once('\n')
        .unwrap_or_else(|| ("", stdout.trim_end()));
    match status {
        "200" => serde_json::from_str::<Value>(body).map_or_else(
            |_| AppApi::Failed("the forge answer did not parse as JSON".into()),
            AppApi::Ok,
        ),
        "404" => AppApi::Missing,
        "401" | "403" => AppApi::Refused(format!(
            "the forge refused the App credentials ({status}); RK_BOT_APP_ID and the key file must name the same App"
        )),
        other => AppApi::Failed(format!("the forge answered {other}")),
    }
}

/// The first non-empty line of a byte stream, which is where `curl` names
/// what went wrong; the lines after it are prose pointing at a web page,
/// so the last line of a failure carries none of the reason.
fn first_line(bytes: &[u8]) -> String {
    non_empty_line(bytes, End::First)
}

/// The last non-empty line of a byte stream, where a CLI puts its verdict.
fn last_line(bytes: &[u8]) -> String {
    non_empty_line(bytes, End::Last)
}

/// Which end of a stream a line is taken from.
#[derive(Clone, Copy)]
enum End {
    /// The first non-empty line.
    First,
    /// The last non-empty line.
    Last,
}

/// One non-empty line of a byte stream, taken from the named end.
fn non_empty_line(bytes: &[u8], end: End) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    match end {
        End::First => lines.next(),
        End::Last => lines.next_back(),
    }
    .unwrap_or("no output")
    .trim()
    .to_owned()
}

/// RFC 4648 base64url without padding, which is what a JWT's segments
/// carry.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut word: u32 = 0;
        for (index, byte) in chunk.iter().enumerate() {
            word |= u32::from(*byte) << (16 - 8 * index);
        }
        for position in 0..=chunk.len() {
            let sextet = (word >> (18 - 6 * position)) & 0x3f;
            let Ok(index) = usize::try_from(sextet) else {
                continue;
            };
            out.push(char::from(ALPHABET[index]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64url;

    /// The RFC 4648 vectors, in the url-safe alphabet, unpadded.
    #[test]
    fn base64url_matches_the_rfc_vectors() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(b"foob"), "Zm9vYg");
        assert_eq!(base64url(b"fooba"), "Zm9vYmE");
        assert_eq!(base64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64url(&[0xfb, 0xef, 0xff]), "--__");
        assert_eq!(base64url(&[0xff, 0xff, 0xfe]), "___-");
    }

    /// A JWT header encodes to the value every RS256 example shows.
    #[test]
    fn the_fixed_header_encodes_stably() {
        assert_eq!(
            base64url(br#"{"alg":"RS256","typ":"JWT"}"#),
            "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9"
        );
    }
}
