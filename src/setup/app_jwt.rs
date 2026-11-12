//! Authenticating to the forge as the bot App itself.
//!
//! GitHub serves the installation-reading endpoints to App credentials
//! only: `GET /repos/{owner}/{repo}/installation` takes a JWT signed with
//! the App's private key, and no personal access token of any class is
//! accepted there. So the `install-bot` step observes as the App: `rk`
//! builds the RS256 signing input, signs it in-process with the key bytes
//! it already holds, and carries the resulting token to the forge through
//! `crate::http`, in a header — the JWT is a credential, and
//! `forge-setup:a-secret-never-reaches-argv` binds it like any other. No
//! child process holds the key or the token, so neither can reach an
//! argument list, an environment, or a stream.
//!
//! The answers surface only as the classified [`AppApi`] and the step
//! states built from it: no request or response reaches a journal, an
//! event, or a transcript, and a failure's reason is scrubbed against the
//! token before it is returned.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use zeroize::Zeroizing;

use super::context::Ctx;
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
pub fn app_id(configured: Option<&str>) -> Result<Option<String>, RkError> {
    // The environment wins: an operator running against another App for
    // one command must not be overruled by a committed file. The
    // committed identifier is the fallback, so the common case needs no
    // export at all.
    let from_env = secrets::value_of("RK_BOT_APP_ID");
    let held = from_env
        .as_ref()
        .and_then(|value| value.to_str())
        .or(configured);
    let Some(app_id) = held else {
        return Ok(None);
    };
    let numeric =
        Some(app_id).filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()));
    let Some(app_id) = numeric else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                "the bot App id is not numeric",
            )
            .expected("the App ID from the App's settings page, digits only")
            .action("set setup.bot.app_id in .release-kit/config.toml, or export RK_BOT_APP_ID; copy the App ID, not the Client ID, as the setup guide's step 5 says")
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
pub fn mint(_ctx: &Ctx, credentials: &AppCredentials) -> Result<String, String> {
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
    let signature = sign(credentials, input.as_bytes())?;
    Ok(format!("{input}.{}", base64url(&signature)))
}

/// Sign the token's input with the App key, in-process: RSASSA-PKCS1-v1_5
/// with SHA-256, which is RS256. The key is the PEM the App's settings
/// page issues, PKCS#1 or PKCS#8. Its bytes are read from what the run
/// already holds, and no error names them.
fn sign(credentials: &AppCredentials, input: &[u8]) -> Result<Vec<u8>, String> {
    use rustls_pki_types::pem::PemObject as _;
    let der = rustls_pki_types::PrivateKeyDer::from_pem_slice(&credentials.key_bytes)
        .map_err(|_| "the App key is not a PEM private key".to_owned())?;
    let pair = match &der {
        rustls_pki_types::PrivateKeyDer::Pkcs1(key) => {
            ring::rsa::KeyPair::from_der(key.secret_pkcs1_der())
        }
        rustls_pki_types::PrivateKeyDer::Pkcs8(key) => {
            ring::rsa::KeyPair::from_pkcs8(key.secret_pkcs8_der())
        }
        _ => return Err("the App key is not an RSA private key".to_owned()),
    }
    .map_err(|rejected| format!("the App key is not a usable RSA key: {rejected}"))?;
    let mut signature = Zeroizing::new(vec![0; pair.public().modulus_len()]);
    pair.sign(
        &ring::signature::RSA_PKCS1_SHA256,
        &ring::rand::SystemRandom::new(),
        input,
        &mut signature,
    )
    .map_err(|_| "the App key could not sign the JWT".to_owned())?;
    Ok(signature.to_vec())
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
/// The call goes through `crate::http` in-process, with a ten-second
/// bound: the Authorization header lives in this process alone, and a
/// failure's reason is scrubbed against the token and its signature
/// before it is returned.
#[must_use]
pub fn api_get(_ctx: &Ctx, jwt: &str, path: &str) -> AppApi {
    let bearer = Zeroizing::new(format!("Bearer {jwt}"));
    let url = format!("https://api.github.com/{path}");
    let headers = [
        ("Authorization", bearer.as_str()),
        ("Accept", "application/vnd.github+json"),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    let request = crate::http::Request {
        method: crate::http::Method::Get,
        url: &url,
        headers: &headers,
        body: &[],
        timeout: std::time::Duration::from_secs(10),
        // The curl call this replaced followed no redirect.
        follow: false,
    };
    let scrub = |text: &str| -> String {
        let signature = jwt.rsplit('.').next().unwrap_or(jwt);
        text.replace(jwt, "[redacted]")
            .replace(signature, "[redacted]")
    };
    let answer = match crate::http::send(&request) {
        Ok(answer) => answer,
        Err(reason) => {
            return AppApi::Failed(format!(
                "the request did not reach the forge: {}",
                scrub(&reason)
            ));
        }
    };
    match answer.status {
        200 => serde_json::from_slice::<Value>(&answer.body).map_or_else(
            |_| AppApi::Failed("the forge answer did not parse as JSON".into()),
            AppApi::Ok,
        ),
        404 => AppApi::Missing,
        status @ (401 | 403) => AppApi::Refused(format!(
            "the forge refused the App credentials ({status}); RK_BOT_APP_ID and the key file must name the same App"
        )),
        other => AppApi::Failed(format!("the forge answered {other}")),
    }
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
    use super::{AppCredentials, base64url, mint};

    /// The inverse of [`base64url`], for reading a token back.
    fn unbase64url(text: &str) -> Vec<u8> {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut bits: u32 = 0;
        let mut count = 0;
        let mut out = Vec::new();
        for byte in text.bytes() {
            let value = ALPHABET
                .iter()
                .position(|c| *c == byte)
                .expect("a base64url byte");
            bits = (bits << 6) | u32::try_from(value).expect("fits");
            count += 6;
            if count >= 8 {
                count -= 8;
                out.push(u8::try_from((bits >> count) & 0xff).expect("a byte"));
            }
        }
        out
    }

    /// The token rk signs in-process verifies against the key pair's public
    /// half under RS256, and carries the claims GitHub requires: `iss`, the
    /// App id, `iat` a minute back, and `exp` inside the ten-minute cap.
    #[test]
    fn the_app_jwt_verifies_with_the_public_key() {
        let key = <rsa::PrivateKey as rsa::Generate>::generate();
        let pem = rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_pem(&key, rsa::pkcs1::LineEnding::LF)
            .expect("the key encodes");
        let public = rsa::pkcs1::EncodeRsaPublicKey::to_pkcs1_der(&key.to_public_key())
            .expect("the public key encodes");
        let ctx = crate::setup::context::Ctx::for_tests(
            camino::Utf8PathBuf::from("/srv/widget"),
            "acme/widget".to_owned(),
            crate::detect::Forge::Github,
            std::path::PathBuf::from("gh"),
            None,
        );
        let credentials = AppCredentials {
            app_id: "314159".to_owned(),
            key_bytes: zeroize::Zeroizing::new(pem.as_bytes().to_vec()),
        };
        let token = mint(&ctx, &credentials).expect("the token signs");
        let (input, signature) = token.rsplit_once('.').expect("three segments");
        ring::signature::UnparsedPublicKey::new(
            &ring::signature::RSA_PKCS1_2048_8192_SHA256,
            public.as_bytes(),
        )
        .verify(input.as_bytes(), &unbase64url(signature))
        .expect("the signature verifies with the public key");
        let (header, claims) = input.split_once('.').expect("a header and claims");
        assert_eq!(unbase64url(header), br#"{"alg":"RS256","typ":"JWT"}"#);
        let claims: serde_json::Value =
            serde_json::from_slice(&unbase64url(claims)).expect("the claims parse");
        assert_eq!(claims["iss"], "314159");
        let iat = claims["iat"].as_u64().expect("an iat");
        let exp = claims["exp"].as_u64().expect("an exp");
        assert_eq!(exp - iat, 600, "a minute back and nine minutes out");
    }

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
