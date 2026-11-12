//! The one owner of the HTTP requests `rk` sends.
//!
//! `rk` sends every request in-process, through one blocking client with
//! a TLS stack that links no system OpenSSL and needs no system library.
//! No other module builds a client. The client trusts the platform's
//! certificate store, plus one bundle file, the first that
//! `SSL_CERT_FILE`, `NIX_SSL_CERT_FILE`, or `CURL_CA_BUNDLE` names, plus
//! each directory `SSL_CERT_DIR` names. It reads its proxy per request
//! and per redirect: `https_proxy`, then `HTTPS_PROXY`, for https,
//! `http_proxy`, then `HTTP_PROXY`, for http, then `all_proxy`, then
//! `ALL_PROXY`, and none where `no_proxy`, else `NO_PROXY`, names the
//! host, its domain, or an address range it falls in. Each call carries
//! its own timeout over the whole request, and states whether it follows
//! a redirect, as each curl call it replaced did.
//!
//! `RK_HTTP_ORIGIN` is the one test seam: an `http://` origin on
//! `127.0.0.1`, `::1`, or `localhost` that every request and every
//! redirect is sent to instead, with no proxy, and with the host it named
//! in the `X-Rk-Original-Host` header. Any other value refuses by name, so
//! the seam can never carry a token to a remote host.
//!
//! SATISFIES tool-requirements:rk-sends-its-own-requests

use std::ffi::OsString;
use std::time::Duration;

/// The user agent every request names; crates.io requires one.
pub const USER_AGENT: &str = concat!("release-kit/", env!("CARGO_PKG_VERSION"));

/// The test seam's variable.
pub const ORIGIN_VAR: &str = "RK_HTTP_ORIGIN";

/// The header that carries the host a request named, under the seam.
pub const ORIGINAL_HOST_HEADER: &str = "X-Rk-Original-Host";

/// One answer: its status, its body, and the URL that answered after
/// every redirect.
#[derive(Debug, Clone)]
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    /// The body, whole.
    pub body: Vec<u8>,
    /// The URL that answered, after the redirects the client followed.
    pub final_url: String,
}

impl Response {
    /// Whether the status is a success, 2xx.
    #[must_use]
    pub const fn ok(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

/// One request's method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// GET.
    Get,
    /// POST, with a body.
    Post,
}

/// One request: what a call site asks the client to send.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The method.
    pub method: Method,
    /// The URL.
    pub url: &'a str,
    /// Extra headers, beside the user agent.
    pub headers: &'a [(&'a str, &'a str)],
    /// The body of a POST.
    pub body: &'a [u8],
    /// The bound on the whole request, redirects included.
    pub timeout: Duration,
    /// Whether a redirect is followed. Where it is not, a 3xx is the
    /// answer.
    pub follow: bool,
}

/// The most redirects one request follows.
const MAX_REDIRECTS: usize = 10;

impl<'a> Request<'a> {
    /// A GET with no extra header, which follows no redirect.
    #[must_use]
    pub const fn get(url: &'a str, timeout: Duration) -> Self {
        Self {
            method: Method::Get,
            url,
            headers: &[],
            body: &[],
            timeout,
            follow: false,
        }
    }

    /// The same request, following redirects.
    #[must_use]
    pub const fn following(self) -> Self {
        Self {
            follow: true,
            ..self
        }
    }
}

/// Send one request and read its whole answer. Every status is an
/// answer, so a 404 is `Ok`; an `Err` is a request that got no answer,
/// with the reason.
///
/// # Errors
///
/// The reason where the request got no answer: an unresolvable host, a
/// refused or timed-out connection, a TLS failure, or a seam value that
/// is not a local origin.
pub fn send(request: &Request<'_>) -> Result<Response, String> {
    send_with(request, &|name| std::env::var_os(name))
}

/// [`send`] with the environment it reads given, so a test can name the
/// certificate and proxy variables without touching the process.
///
/// # Errors
///
/// As [`send`].
pub fn send_with(
    request: &Request<'_>,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Result<Response, String> {
    let deadline = std::time::Instant::now() + request.timeout;
    let roots = roots(env);
    let seam = seam(env)?;
    let mut url = request.url.to_owned();
    let mut method = request.method;
    for _ in 0..=MAX_REDIRECTS {
        let remaining = deadline
            .checked_duration_since(std::time::Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or_else(|| "the request timed out".to_owned())?;
        let (sent_to, original_host) = match &seam {
            Some(origin) => {
                let (to, host) = through_seam(origin, &url)?;
                (to, Some(host))
            }
            None => (url.clone(), None),
        };
        // The seam stays on the loopback: it never takes a proxy.
        let proxy = if seam.is_some() {
            None
        } else {
            proxy_for(&url, env)?
        };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(remaining))
            .user_agent(USER_AGENT)
            .proxy(proxy)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::new_with_certs(&roots))
                    .unversioned_rustls_crypto_provider(std::sync::Arc::new(
                        rustls::crypto::ring::default_provider(),
                    ))
                    .build(),
            )
            .build()
            .into();
        let mut headers: Vec<(&str, &str)> = request.headers.to_vec();
        if let Some(host) = original_host.as_deref() {
            headers.push((ORIGINAL_HOST_HEADER, host));
        }
        let answered = match method {
            Method::Get => {
                let mut builder = agent.get(&sent_to);
                for (name, value) in &headers {
                    builder = builder.header(*name, *value);
                }
                builder.call()
            }
            Method::Post => {
                let mut builder = agent.post(&sent_to);
                for (name, value) in &headers {
                    builder = builder.header(*name, *value);
                }
                builder.send(request.body)
            }
        };
        let mut response = answered.map_err(|error| describe(&error))?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if request.follow
            && matches!(status, 301 | 302 | 303 | 307 | 308)
            && let Some(location) = location
        {
            url = join(&url, &location)?;
            if status == 303 || (matches!(status, 301 | 302) && method == Method::Post) {
                method = Method::Get;
            }
            continue;
        }
        let body = response
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .map_err(|error| describe(&error))?;
        return Ok(Response {
            status,
            body,
            final_url: url,
        });
    }
    Err(format!(
        "{} redirected more than {MAX_REDIRECTS} times",
        request.url
    ))
}

/// The URL a redirect names, resolved against the URL that sent it by
/// RFC 3986, section 5.2: the reference's own parts where it carries
/// them, the base's otherwise, a merged path for a relative one, and dot
/// segments removed. The fragment is dropped, because no request sends
/// one.
fn join(base: &str, location: &str) -> Result<String, String> {
    let base = UriParts::parse(base);
    let Some(scheme) = base.scheme else {
        return Err("the redirecting URL names no scheme".to_owned());
    };
    let reference = UriParts::parse(location);
    let (scheme, authority, path, query) = match (reference.scheme, reference.authority) {
        (Some(own), authority) => (
            own,
            authority,
            remove_dot_segments(reference.path),
            reference.query,
        ),
        (None, Some(authority)) => (
            scheme,
            Some(authority),
            remove_dot_segments(reference.path),
            reference.query,
        ),
        (None, None) => resolve_relative(scheme, &base, &reference),
    };
    let mut out = format!("{scheme}:");
    if let Some(authority) = authority {
        out.push_str("//");
        out.push_str(authority);
    }
    out.push_str(&path);
    if let Some(query) = query {
        out.push('?');
        out.push_str(query);
    }
    Ok(out)
}

/// The target of a reference with no scheme and no authority, against
/// `base`, by RFC 3986, section 5.2.2.
fn resolve_relative<'a>(
    scheme: &'a str,
    base: &UriParts<'a>,
    reference: &UriParts<'a>,
) -> (&'a str, Option<&'a str>, String, Option<&'a str>) {
    if reference.path.is_empty() {
        return (
            scheme,
            base.authority,
            base.path.to_owned(),
            reference.query.or(base.query),
        );
    }
    let path = if reference.path.starts_with('/') {
        reference.path.to_owned()
    } else if base.authority.is_some() && base.path.is_empty() {
        format!("/{}", reference.path)
    } else {
        let dir = base.path.rfind('/').map_or("", |at| &base.path[..=at]);
        format!("{dir}{}", reference.path)
    };
    (
        scheme,
        base.authority,
        remove_dot_segments(&path),
        reference.query,
    )
}

/// The five parts of a URI reference, split as RFC 3986, appendix B
/// splits one.
struct UriParts<'a> {
    scheme: Option<&'a str>,
    authority: Option<&'a str>,
    path: &'a str,
    query: Option<&'a str>,
}

impl<'a> UriParts<'a> {
    fn parse(text: &'a str) -> Self {
        let text = text.split_once('#').map_or(text, |(before, _)| before);
        // A scheme is the text before the first `:`, where no `/`, `?`,
        // or `#` comes before it.
        let (scheme, rest) = match text.find([':', '/', '?']) {
            Some(at) if text.as_bytes()[at] == b':' && at > 0 => {
                (Some(&text[..at]), &text[at + 1..])
            }
            _ => (None, text),
        };
        let (authority, rest) = rest.strip_prefix("//").map_or((None, rest), |after| {
            let end = after.find(['/', '?']).unwrap_or(after.len());
            (Some(&after[..end]), &after[end..])
        });
        let (path, query) = match rest.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (rest, None),
        };
        Self {
            scheme,
            authority,
            path,
            query,
        }
    }
}

/// A path with its `.` and `..` segments removed, by RFC 3986, section
/// 5.2.4.
fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_owned();
    let mut output = String::new();
    while !input.is_empty() {
        if input.starts_with("../") {
            input.replace_range(..3, "");
        } else if input.starts_with("./") {
            input.replace_range(..2, "");
        } else if input.starts_with("/./") {
            input.replace_range(..3, "/");
        } else if input == "/." {
            input.replace_range(.., "/");
        } else if input.starts_with("/../") {
            input.replace_range(..4, "/");
            output.truncate(output.rfind('/').unwrap_or(0));
        } else if input == "/.." {
            input.replace_range(.., "/");
            output.truncate(output.rfind('/').unwrap_or(0));
        } else if input == "." || input == ".." {
            input.clear();
        } else {
            let start = usize::from(input.starts_with('/'));
            let end = input[start..]
                .find('/')
                .map_or(input.len(), |at| at + start);
            output.push_str(&input[..end]);
            input.replace_range(..end, "");
        }
    }
    output
}

/// A transport error in words, with no header value: a header can carry a
/// token, and ureq's errors name the URL and the cause alone.
fn describe(error: &ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => "the request timed out".to_owned(),
        other => other.to_string(),
    }
}

/// The seam's origin, where `RK_HTTP_ORIGIN` names one, judged once.
fn seam(env: &dyn Fn(&str) -> Option<OsString>) -> Result<Option<String>, String> {
    let Some(origin) = env(ORIGIN_VAR).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let origin = origin.to_string_lossy().trim_end_matches('/').to_owned();
    if !local_origin(&origin) {
        return Err(format!(
            "{ORIGIN_VAR} names {origin}, which is not an http:// origin on 127.0.0.1, ::1, or localhost; it is a test seam and never sends a request elsewhere"
        ));
    }
    Ok(Some(origin))
}

/// `url` sent to the seam's `origin` instead, and the host it named.
fn through_seam(origin: &str, url: &str) -> Result<(String, String), String> {
    let (_, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("{url} names no scheme"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    Ok((format!("{origin}/{path}"), host.to_owned()))
}

/// The URL to send to, and the host it named where the test seam
/// redirects it.
#[cfg(test)]
fn route(
    url: &str,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Result<(String, Option<String>), String> {
    seam(env)?.map_or_else(
        || Ok((url.to_owned(), None)),
        |origin| through_seam(&origin, url).map(|(to, host)| (to, Some(host))),
    )
}

/// Whether `origin` is `http://` on a loopback host, with an optional
/// port and nothing else.
fn local_origin(origin: &str) -> bool {
    let Some(authority) = origin.strip_prefix("http://") else {
        return false;
    };
    if authority.contains(['/', '@', '?', '#']) {
        return false;
    }
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((host, rest)) = bracketed.split_once(']') else {
            return false;
        };
        if !(rest.is_empty() || port_only(rest)) {
            return false;
        }
        host
    } else {
        match authority.split_once(':') {
            Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => {
                host
            }
            Some(_) => return false,
            None => authority,
        }
    };
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

/// Whether `rest` is `:<digits>`.
fn port_only(rest: &str) -> bool {
    rest.strip_prefix(':')
        .is_some_and(|port| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
}

/// The proxy a request to `url` goes through, read the way curl reads
/// it, with the lowercase name first: `https_proxy`, `HTTPS_PROXY`, then
/// `all_proxy`, `ALL_PROXY` for an https URL, `http_proxy`, `HTTP_PROXY`,
/// then `all_proxy`, `ALL_PROXY` for an http one, and none where
/// `no_proxy`, else `NO_PROXY`, names the host.
fn proxy_for(
    url: &str,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Result<Option<ureq::Proxy>, String> {
    let set = |name: &str| {
        env(name)
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty())
    };
    let (scheme, rest) = url.split_once("://").unwrap_or(("http", url));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.strip_prefix('[').map_or_else(
        || host.split(':').next().unwrap_or_default(),
        |bracketed| bracketed.split(']').next().unwrap_or_default(),
    );
    if let Some(list) = set("no_proxy").or_else(|| set("NO_PROXY"))
        && bypasses(&list, host)
    {
        return Ok(None);
    }
    let names: &[&str] = if scheme == "https" {
        &["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"]
    } else {
        &["http_proxy", "HTTP_PROXY", "all_proxy", "ALL_PROXY"]
    };
    let Some(proxy) = names.iter().find_map(|name| set(name)) else {
        return Ok(None);
    };
    ureq::Proxy::new(&proxy)
        .map(Some)
        .map_err(|error| format!("the proxy the environment names does not parse: {error}"))
}

/// Whether a no-proxy list names `host`, as curl reads one: `*` names
/// every host, a name entry names the host itself and every host under
/// it, with or without a leading dot, an address entry names that
/// address, and an `<address>/<bits>` entry names every address in that
/// range.
fn bypasses(list: &str, host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let address: Option<std::net::IpAddr> = host.parse().ok();
    list.split(',')
        .map(|entry| entry.trim().to_ascii_lowercase())
        .filter(|entry| !entry.is_empty())
        .any(|entry| {
            if entry == "*" {
                return true;
            }
            let entry = entry.trim_start_matches('[').replace(']', "");
            if let Some(address) = address {
                return in_range(&entry, address);
            }
            let entry = entry.trim_start_matches('.');
            host == entry || host.ends_with(&format!(".{entry}"))
        })
}

/// Whether `address` is the address `entry` names, or falls in the
/// `<address>/<bits>` range it names.
fn in_range(entry: &str, address: std::net::IpAddr) -> bool {
    let (base, bits) = match entry.split_once('/') {
        Some((base, bits)) => match bits.parse::<u32>() {
            Ok(bits) => (base, Some(bits)),
            Err(_) => return false,
        },
        None => (entry, None),
    };
    let Ok(base) = base.parse::<std::net::IpAddr>() else {
        return false;
    };
    match (base, address) {
        (std::net::IpAddr::V4(base), std::net::IpAddr::V4(address)) => {
            let bits = bits.unwrap_or(32);
            if bits > 32 {
                return false;
            }
            let mask = u32::MAX.checked_shl(32 - bits).unwrap_or(0);
            u32::from(base) & mask == u32::from(address) & mask
        }
        (std::net::IpAddr::V6(base), std::net::IpAddr::V6(address)) => {
            let bits = bits.unwrap_or(128);
            if bits > 128 {
                return false;
            }
            let mask = u128::MAX.checked_shl(128 - bits).unwrap_or(0);
            u128::from(base) & mask == u128::from(address) & mask
        }
        _ => false,
    }
}

/// The certificates the client trusts: the platform's store, plus one
/// bundle file, the first that `SSL_CERT_FILE`, `NIX_SSL_CERT_FILE`, or
/// `CURL_CA_BUNDLE` names, plus each directory `SSL_CERT_DIR` names.
fn roots(env: &dyn Fn(&str) -> Option<OsString>) -> Vec<ureq::tls::Certificate<'static>> {
    let set = |name: &str| env(name).filter(|value| !value.is_empty());
    let mut certs = platform_store();
    if let Some(bundle) = ["SSL_CERT_FILE", "NIX_SSL_CERT_FILE", "CURL_CA_BUNDLE"]
        .iter()
        .find_map(|name| set(name))
    {
        certs.extend(
            rustls_native_certs::load_certs_from_paths(Some(std::path::Path::new(&bundle)), None)
                .certs,
        );
    }
    if let Some(dirs) = set("SSL_CERT_DIR") {
        for dir in std::env::split_paths(&dirs).filter(|path| !path.as_os_str().is_empty()) {
            certs.extend(rustls_native_certs::load_certs_from_paths(None, Some(&dir)).certs);
        }
    }
    let mut seen = std::collections::HashSet::new();
    certs
        .iter()
        .filter(|der| seen.insert(der.as_ref().to_vec()))
        .map(|der| ureq::tls::Certificate::from_der(der.as_ref()).to_owned())
        .collect()
}

/// The platform's own store, which no trust variable narrows. On a Unix
/// other than macOS that is every certificate in the system's certificate
/// directories. Elsewhere the operating system's store answers.
#[cfg(all(unix, not(target_os = "macos")))]
fn platform_store() -> Vec<rustls_pki_types::CertificateDer<'static>> {
    openssl_probe::candidate_cert_dirs()
        .flat_map(|dir| rustls_native_certs::load_certs_from_paths(None, Some(dir)).certs)
        .collect()
}

/// The platform's own store. The operating system's store answers here,
/// and rustls-native-certs reads `SSL_CERT_FILE` and `SSL_CERT_DIR` in
/// place of it where this process sets either.
#[cfg(not(all(unix, not(target_os = "macos"))))]
fn platform_store() -> Vec<rustls_pki_types::CertificateDer<'static>> {
    rustls_native_certs::load_native_certs().certs
}

#[cfg(test)]
mod tests {
    use super::{Request, bypasses, local_origin, route, send_with};
    use std::ffi::OsString;
    use std::io::{BufRead as _, Write as _};
    use std::time::Duration;

    /// A one-shot local server: each accepted connection gets `answer`'s
    /// reply to the request line it read, and the request lines are kept.
    fn serve(
        answer: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port binds");
        let port = listener.local_addr().expect("an address").port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = std::sync::Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = std::io::BufReader::new(stream.try_clone().expect("clones"));
                let mut head = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    head.push_str(&line);
                }
                record.lock().expect("locks").push(head.clone());
                if let Some(reply) = answer(&head) {
                    let _ = stream.write_all(reply.as_bytes());
                }
            }
        });
        (port, seen)
    }

    fn reply(status: &str, extra: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
            body.len()
        )
    }

    fn no_env(_: &str) -> Option<OsString> {
        None
    }

    /// Every status is an answer: a success and a not-found each come back
    /// with their status and body, and every request names rk's agent.
    #[test]
    fn a_status_is_an_answer() {
        let (port, seen) = serve(|head| {
            Some(if head.starts_with("GET /there ") {
                reply("200 OK", "", "found")
            } else {
                reply("404 Not Found", "", "missing")
            })
        });
        let there = format!("http://127.0.0.1:{port}/there");
        let answer =
            send_with(&Request::get(&there, Duration::from_secs(5)), &no_env).expect("an answer");
        assert_eq!(
            (answer.status, answer.body.as_slice()),
            (200, b"found".as_slice())
        );
        let gone = format!("http://127.0.0.1:{port}/gone");
        let answer =
            send_with(&Request::get(&gone, Duration::from_secs(5)), &no_env).expect("an answer");
        assert_eq!(answer.status, 404);
        assert!(
            seen.lock()
                .expect("locks")
                .iter()
                .all(|head| head.contains("release-kit/")),
            "every request names the agent"
        );
    }

    /// A redirect is followed, and the URL that answered is reported.
    #[test]
    fn a_redirect_reports_the_url_that_answered() {
        let (port, _) = serve(move |head| {
            Some(if head.starts_with("GET /latest ") {
                reply("302 Found", "Location: /releases/tag/v1.2.3\r\n", "")
            } else {
                reply("200 OK", "", "")
            })
        });
        let url = format!("http://127.0.0.1:{port}/latest");
        let answer = send_with(
            &Request::get(&url, Duration::from_secs(5)).following(),
            &no_env,
        )
        .expect("an answer");
        assert!(
            answer.final_url.ends_with("/releases/tag/v1.2.3"),
            "{}",
            answer.final_url
        );
        // A request that follows no redirect answers with the redirect.
        let answer =
            send_with(&Request::get(&url, Duration::from_secs(5)), &no_env).expect("an answer");
        assert_eq!(answer.status, 302);
        assert_eq!(answer.final_url, url);
    }

    /// A redirect resolves as RFC 3986 resolves a reference: a query
    /// alone keeps the path, dot segments go, and a `://` inside a query
    /// names no scheme.
    #[test]
    fn a_redirect_resolves_as_a_uri_reference() {
        let base = "http://a/b/c/d;p?q";
        for (reference, target) in [
            ("g", "http://a/b/c/g"),
            ("./g", "http://a/b/c/g"),
            ("g/", "http://a/b/c/g/"),
            ("/g", "http://a/g"),
            ("//g", "http://g"),
            ("?y", "http://a/b/c/d;p?y"),
            ("g?y", "http://a/b/c/g?y"),
            ("#s", "http://a/b/c/d;p?q"),
            ("", "http://a/b/c/d;p?q"),
            (".", "http://a/b/c/"),
            ("..", "http://a/b/"),
            ("../g", "http://a/b/g"),
            ("../..", "http://a/"),
            ("../../../g", "http://a/g"),
            ("/./g", "http://a/g"),
            ("g;x=1/../y", "http://a/b/c/y"),
            ("https://other/x?y", "https://other/x?y"),
            (
                "/next?return=http://127.0.0.1:9/x",
                "http://a/next?return=http://127.0.0.1:9/x",
            ),
        ] {
            assert_eq!(
                super::join(base, reference).as_deref(),
                Ok(target),
                "{reference}"
            );
        }
    }

    /// The three references a local server sends reach the resource they
    /// name.
    #[test]
    fn a_redirect_reaches_the_resource_it_names() {
        let (port, seen) = serve(|head| {
            Some(if head.starts_with("GET /a/b/item?old=1 ") {
                reply("302 Found", "Location: ?new=2\r\n", "")
            } else if head.starts_with("GET /a/b/item?new=2 ") {
                reply("302 Found", "Location: ../release\r\n", "")
            } else if head.starts_with("GET /a/release ") {
                reply(
                    "302 Found",
                    "Location: /next?return=http://127.0.0.1:9/x\r\n",
                    "",
                )
            } else if head.starts_with("GET /next?return=http://127.0.0.1:9/x ") {
                reply("200 OK", "", "arrived")
            } else {
                reply("404 Not Found", "", "")
            })
        });
        let url = format!("http://127.0.0.1:{port}/a/b/item?old=1");
        let answer = send_with(
            &Request::get(&url, Duration::from_secs(5)).following(),
            &no_env,
        )
        .expect("an answer");
        let heads = seen.lock().expect("locks").clone();
        assert_eq!(answer.body, b"arrived", "{heads:?}");
    }

    /// Under the seam, a redirect to another host and a proxy the
    /// environment names both stay off: every hop goes to the seam's
    /// origin, and the proxy sees nothing.
    #[test]
    fn the_seam_keeps_every_hop_on_the_loopback() {
        let (elsewhere, elsewhere_seen) = serve(|_| Some(reply("200 OK", "", "escaped")));
        let (proxy, proxy_seen) = serve(|_| Some(reply("200 OK", "", "proxied")));
        let (origin, origin_seen) = serve(move |head| {
            Some(if head.starts_with("GET /start ") {
                reply(
                    "302 Found",
                    &format!("Location: http://127.0.0.1:{elsewhere}/leak\r\n"),
                    "",
                )
            } else {
                reply("200 OK", "", "stayed")
            })
        });
        let seam = format!("http://127.0.0.1:{origin}");
        let proxy = format!("http://127.0.0.1:{proxy}");
        let env = |name: &str| match name {
            "RK_HTTP_ORIGIN" => Some(OsString::from(&seam)),
            "https_proxy" | "HTTPS_PROXY" | "http_proxy" | "ALL_PROXY" => {
                Some(OsString::from(&proxy))
            }
            _ => None,
        };
        let answer = send_with(
            &Request::get("https://api.github.com/start", Duration::from_secs(5)).following(),
            &env,
        )
        .expect("an answer");
        assert_eq!(answer.body, b"stayed");
        assert!(
            elsewhere_seen.lock().expect("locks").is_empty(),
            "no escape"
        );
        assert!(proxy_seen.lock().expect("locks").is_empty(), "no proxy");
        let heads = origin_seen.lock().expect("locks").clone();
        assert!(
            heads.iter().any(|head| head.starts_with("GET /leak ")
                && head.contains(&format!("127.0.0.1:{elsewhere}"))),
            "the redirect came back to the seam, naming its host: {heads:?}"
        );
    }

    /// The proxy is chosen for each hop: a bypassed loopback request that
    /// redirects to another host reaches the proxy for that host.
    #[test]
    fn each_redirect_takes_its_own_proxy() {
        let (proxy, proxy_seen) = serve(|_| Some(reply("403 Forbidden", "", "")));
        let (origin, _) = serve(|_| {
            Some(reply(
                "302 Found",
                "Location: http://example.invalid/next\r\n",
                "",
            ))
        });
        let proxy = format!("http://127.0.0.1:{proxy}");
        let env = |name: &str| match name {
            "http_proxy" => Some(OsString::from(&proxy)),
            "no_proxy" => Some(OsString::from("127.0.0.0/8")),
            _ => None,
        };
        let url = format!("http://127.0.0.1:{origin}/start");
        let _ = send_with(
            &Request::get(&url, Duration::from_secs(5)).following(),
            &env,
        );
        let heads = proxy_seen.lock().expect("locks").clone();
        assert_eq!(heads.len(), 1, "{heads:?}");
        assert!(
            heads[0].starts_with("CONNECT example.invalid:80 "),
            "{heads:?}"
        );
    }

    /// `HTTP_PROXY` carries an http request where `http_proxy` is unset,
    /// and the lowercase name wins where both are set.
    #[test]
    fn http_proxy_is_read_in_either_case() {
        let (upper, upper_seen) = serve(|_| Some(reply("403 Forbidden", "", "")));
        let (lower, lower_seen) = serve(|_| Some(reply("403 Forbidden", "", "")));
        let upper = format!("http://127.0.0.1:{upper}");
        let lower = format!("http://127.0.0.1:{lower}");
        let only_upper = |name: &str| (name == "HTTP_PROXY").then(|| OsString::from(&upper));
        let _ = send_with(
            &Request::get("http://example.invalid/a", Duration::from_secs(5)),
            &only_upper,
        );
        assert_eq!(upper_seen.lock().expect("locks").len(), 1);
        let both = |name: &str| match name {
            "HTTP_PROXY" => Some(OsString::from(&upper)),
            "http_proxy" => Some(OsString::from(&lower)),
            _ => None,
        };
        let _ = send_with(
            &Request::get("http://example.invalid/b", Duration::from_secs(5)),
            &both,
        );
        assert_eq!(upper_seen.lock().expect("locks").len(), 1);
        assert_eq!(lower_seen.lock().expect("locks").len(), 1);
    }

    /// A host that gives no answer is an error with a reason: a refused
    /// connection, a connection closed with no answer, and a server that
    /// answers past the timeout.
    #[test]
    fn no_answer_is_an_error_with_a_reason() {
        let refused = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port binds");
            listener.local_addr().expect("an address").port()
        };
        let url = format!("http://127.0.0.1:{refused}/");
        assert!(send_with(&Request::get(&url, Duration::from_secs(5)), &no_env).is_err());
        let (port, _) = serve(|_| None);
        let url = format!("http://127.0.0.1:{port}/");
        assert!(send_with(&Request::get(&url, Duration::from_secs(5)), &no_env).is_err());
        let (port, _) = serve(|_| {
            std::thread::sleep(Duration::from_secs(3));
            Some(reply("200 OK", "", "late"))
        });
        let url = format!("http://127.0.0.1:{port}/");
        let err = send_with(&Request::get(&url, Duration::from_secs(1)), &no_env)
            .expect_err("the answer comes past the timeout");
        assert!(err.contains("timed out"), "{err}");
    }

    /// A proxy the environment names carries the request: `http_proxy`
    /// for an http URL, `HTTPS_PROXY` for an https one, each through a
    /// tunnel the proxy is asked for, and `NO_PROXY` takes a host out.
    #[test]
    fn the_client_honors_https_proxy() {
        let (port, seen) = serve(|_| Some(reply("403 Forbidden", "", "")));
        let proxy = format!("http://127.0.0.1:{port}");
        let env = |name: &str| {
            (name == "http_proxy" || name == "HTTPS_PROXY").then(|| OsString::from(&proxy))
        };
        let err = send_with(
            &Request::get("http://example.invalid/x", Duration::from_secs(5)),
            &env,
        )
        .expect_err("the proxy refuses the tunnel");
        assert!(err.contains("proxy"), "{err}");
        let _ = send_with(
            &Request::get("https://example.invalid/y", Duration::from_secs(5)),
            &env,
        );
        let lines = seen.lock().expect("locks").clone();
        assert!(
            lines
                .iter()
                .any(|head| head.starts_with("CONNECT example.invalid:80 ")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|head| head.starts_with("CONNECT example.invalid:443 ")),
            "{lines:?}"
        );
        let bypass = |name: &str| match name {
            "http_proxy" => Some(OsString::from(&proxy)),
            "NO_PROXY" => Some(OsString::from("example.invalid")),
            _ => None,
        };
        let before = seen.lock().expect("locks").len();
        let _ = send_with(
            &Request::get("http://example.invalid/z", Duration::from_secs(2)),
            &bypass,
        );
        assert_eq!(
            seen.lock().expect("locks").len(),
            before,
            "NO_PROXY took the host out"
        );
    }

    /// A test CA, as PEM, and a local HTTPS server on `localhost` whose
    /// certificate it signs, as its port.
    fn tls_server() -> (String, u16) {
        let ca_key = rcgen::KeyPair::generate().expect("a CA key");
        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("CA params");
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca = rcgen::CertifiedIssuer::self_signed(ca_params, ca_key).expect("a CA");
        let server_key = rcgen::KeyPair::generate().expect("a server key");
        let server = rcgen::CertificateParams::new(vec!["localhost".to_owned()])
            .expect("server params")
            .signed_by(&server_key, &ca)
            .expect("a server certificate");
        let config = std::sync::Arc::new(
            rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .expect("protocols")
            .with_no_client_auth()
            .with_single_cert(
                vec![server.der().clone()],
                rustls_pki_types::PrivateKeyDer::Pkcs8(server_key.serialize_der().into()),
            )
            .expect("a server config"),
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port binds");
        let port = listener.local_addr().expect("an address").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let Ok(mut connection) =
                    rustls::ServerConnection::new(std::sync::Arc::clone(&config))
                else {
                    continue;
                };
                let mut tls = rustls::Stream::new(&mut connection, &mut stream);
                let mut reader = std::io::BufReader::new(&mut tls);
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                    line.clear();
                }
                let _ = tls.write_all(reply("200 OK", "", "trusted").as_bytes());
                let _ = tls.flush();
            }
        });
        (ca.pem(), port)
    }

    /// An environment that names each path of `pairs` under its variable.
    fn paths_env<'a>(
        pairs: Vec<(&'static str, &'a std::path::Path)>,
    ) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |asked| {
            pairs
                .iter()
                .find(|(name, _)| *name == asked)
                .map(|(_, value)| value.as_os_str().to_owned())
        }
    }

    /// A self-signed CA no server uses, as PEM.
    fn other_ca() -> String {
        let key = rcgen::KeyPair::generate().expect("a key");
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("params");
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        rcgen::CertifiedIssuer::self_signed(params, key)
            .expect("a CA")
            .pem()
    }

    /// A local HTTPS server whose certificate a test CA signs: trusted
    /// where a certificate variable names the CA, and refused where none
    /// does. Each of the four variables is held.
    #[test]
    fn the_client_trusts_ssl_cert_file() {
        let (ca, port) = tls_server();
        let dir = tempfile::tempdir().expect("a scratch dir");
        let bundle = dir.path().join("ca.pem");
        std::fs::write(&bundle, &ca).expect("the bundle writes");
        let certs = dir.path().join("certs");
        std::fs::create_dir(&certs).expect("the dir creates");
        std::fs::write(certs.join("ca.pem"), &ca).expect("the cert writes");
        let url = format!("https://localhost:{port}/");
        let request = Request::get(&url, Duration::from_secs(5));
        assert!(
            send_with(&request, &no_env).is_err(),
            "the test CA is trusted with no variable"
        );
        for (name, value) in [
            ("SSL_CERT_FILE", bundle.as_os_str()),
            ("SSL_CERT_DIR", certs.as_os_str()),
            ("NIX_SSL_CERT_FILE", bundle.as_os_str()),
            ("CURL_CA_BUNDLE", bundle.as_os_str()),
        ] {
            let env = |asked: &str| (asked == name).then(|| value.to_owned());
            let answer = send_with(&request, &env).unwrap_or_else(|err| panic!("{name}: {err}"));
            assert_eq!(answer.body, b"trusted", "{name}");
        }
    }

    /// The variables combine: the platform store, one bundle file, the
    /// first set, and the directories. A bundle of another CA first hides
    /// the later file, and an empty directory hides nothing.
    #[test]
    fn the_trust_variables_combine() {
        let (ca, port) = tls_server();
        let dir = tempfile::tempdir().expect("a scratch dir");
        let bundle = dir.path().join("ca.pem");
        std::fs::write(&bundle, &ca).expect("the bundle writes");
        let certs = dir.path().join("certs");
        std::fs::create_dir(&certs).expect("the dir creates");
        std::fs::write(certs.join("ca.pem"), &ca).expect("the cert writes");
        let other = dir.path().join("other.pem");
        std::fs::write(&other, other_ca()).expect("the bundle writes");
        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).expect("the dir creates");
        let url = format!("https://localhost:{port}/");
        let request = Request::get(&url, Duration::from_secs(5));
        for (label, pairs, trusted) in [
            (
                "an empty directory beside the Nix bundle",
                vec![
                    ("NIX_SSL_CERT_FILE", bundle.as_path()),
                    ("SSL_CERT_DIR", empty.as_path()),
                ],
                true,
            ),
            (
                "an empty directory beside the curl bundle",
                vec![
                    ("CURL_CA_BUNDLE", bundle.as_path()),
                    ("SSL_CERT_DIR", empty.as_path()),
                ],
                true,
            ),
            (
                "SSL_CERT_FILE first, naming another CA",
                vec![
                    ("SSL_CERT_FILE", other.as_path()),
                    ("NIX_SSL_CERT_FILE", bundle.as_path()),
                ],
                false,
            ),
            (
                "a directory adds to another bundle",
                vec![
                    ("SSL_CERT_FILE", other.as_path()),
                    ("SSL_CERT_DIR", certs.as_path()),
                ],
                true,
            ),
        ] {
            let env = paths_env(pairs);
            assert_eq!(send_with(&request, &env).is_ok(), trusted, "{label}");
        }
        // The platform store stays beside a named bundle.
        let platform = super::platform_store().len();
        let env = paths_env(vec![("SSL_CERT_FILE", bundle.as_path())]);
        assert!(super::roots(&env).len() > platform || platform == 0);
        assert!(super::roots(&env).len() >= platform);
    }

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    /// The seam takes a loopback http origin and nothing else, so it
    /// can never carry a request to a remote host.
    #[test]
    fn the_seam_takes_a_loopback_http_origin_alone() {
        for good in [
            "http://127.0.0.1:8080",
            "http://localhost:1",
            "http://[::1]:9",
            "http://127.0.0.1",
        ] {
            assert!(local_origin(good), "{good}");
        }
        for bad in [
            "https://127.0.0.1:8080",
            "http://example.com",
            "http://127.0.0.1.example.com",
            "http://user@127.0.0.1",
            "http://127.0.0.1:80/path",
            "http://localhost:x",
            "http://[::2]:9",
        ] {
            assert!(!local_origin(bad), "{bad}");
        }
        let env = env_of(&[("RK_HTTP_ORIGIN", "http://evil.example")]);
        let err = route("https://api.github.com/app", &env).expect_err("a remote origin refuses");
        assert!(err.contains("RK_HTTP_ORIGIN"), "{err}");
        let env = env_of(&[("RK_HTTP_ORIGIN", "http://127.0.0.1:4000/")]);
        let (url, host) = route("https://api.github.com/app/installations?x=1", &env)
            .expect("a local origin routes");
        assert_eq!(url, "http://127.0.0.1:4000/app/installations?x=1");
        assert_eq!(host.as_deref(), Some("api.github.com"));
    }

    /// `NO_PROXY` names a host and every host under it.
    #[test]
    fn no_proxy_names_a_host_and_its_subdomains() {
        assert!(bypasses("127.0.0.0/8", "127.0.0.1"));
        assert!(bypasses("10.1.0.0/16, example.com", "10.1.2.3"));
        assert!(!bypasses("10.1.0.0/16", "10.2.0.1"));
        assert!(bypasses("127.0.0.1", "127.0.0.1"));
        assert!(bypasses("::1", "::1"));
        assert!(bypasses("[fd00::]/8", "fd12::1"));
        assert!(!bypasses("example.com", "127.0.0.1"));
        assert!(bypasses("example.com", "example.com"));
        assert!(bypasses(".example.com", "api.example.com"));
        assert!(bypasses("foo, example.com", "a.b.example.com"));
        assert!(bypasses("*", "anything"));
        assert!(!bypasses("example.com", "notexample.com"));
        assert!(!bypasses("", "example.com"));
    }

    /// `no_proxy` wins over `NO_PROXY`, as curl reads them.
    #[test]
    fn the_lowercase_no_proxy_wins() {
        let env = env_of(&[
            ("http_proxy", "http://127.0.0.1:9"),
            ("no_proxy", "example.invalid"),
            ("NO_PROXY", "other.invalid"),
        ]);
        assert!(
            super::proxy_for("http://example.invalid/", &env)
                .expect("parses")
                .is_none()
        );
        assert!(
            super::proxy_for("http://other.invalid/", &env)
                .expect("parses")
                .is_some()
        );
    }
}
