//! The environment probe catalog.
//!
//! One catalog, read by every caller that needs to know whether this host
//! is ready: `rk doctor` runs it whole, and a mutating command guards the
//! subset it depends on at entry, so the per-command guards and the
//! doctor cannot drift apart. Each probe answers with a status, a
//! message, and — on failure — the remediation printed verbatim wherever
//! the probe is consulted.

use std::process::Command;

use serde::Serialize;

/// How a failure weighs at the doctor level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeClass {
    /// No mutating command can work without this.
    Hard,
    /// Needed only by some commands or some forges.
    Soft,
}

/// What a probe found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeStatus {
    /// The probe passed.
    Ok,
    /// The probe failed; the remediation says what fixes it.
    Failed,
}

/// One probe's answer.
#[derive(Debug, Serialize)]
pub struct ProbeResult {
    /// The probe's stable name.
    pub id: &'static str,
    /// How the failure weighs.
    pub class: ProbeClass,
    /// What was found.
    pub status: ProbeStatus,
    /// What was found, one line.
    pub message: String,
    /// The exact fix, when the probe failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl ProbeResult {
    fn ok(id: &'static str, class: ProbeClass, message: impl Into<String>) -> Self {
        Self {
            id,
            class,
            status: ProbeStatus::Ok,
            message: message.into(),
            remediation: None,
        }
    }

    fn failed(
        id: &'static str,
        class: ProbeClass,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            id,
            class,
            status: ProbeStatus::Failed,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }
}

/// Run the whole catalog, in its stable order.
#[must_use]
pub fn run_all() -> Vec<ProbeResult> {
    vec![
        shell(),
        state_root(),
        git_remote(),
        forge_cli(
            "gh-auth",
            "RK_GH_BIN",
            "gh",
            "the GitHub CLI",
            "gh auth login",
            // `gh auth status` fails when any stored account is broken,
            // even while the active one works; `--active` judges only the
            // credential this tool would use. Older gh lacks the flag, so
            // the bare form is the fallback.
            &[&["auth", "status", "--active"], &["auth", "status"]],
        ),
        forge_cli(
            "glab-auth",
            "RK_GLAB_BIN",
            "glab",
            "the GitLab CLI",
            "glab auth login",
            &[&["auth", "status"]],
        ),
        tool(
            "openssl",
            "RK_OPENSSL_BIN",
            "openssl",
            "OpenSSL; install-bot signs the App JWT with it",
            &["version"],
        ),
        tool(
            "curl",
            "RK_CURL_BIN",
            "curl",
            "curl; install-bot reads the installation and rk versions --check fetches with it",
            &["--version"],
        ),
        tool(
            "cosign",
            "RK_COSIGN_BIN",
            "cosign",
            "cosign; the release verify step checks a GitLab provenance bundle with it",
            &["version"],
        ),
        tool(
            "pypi-attestations",
            "RK_PYPI_ATTESTATIONS_BIN",
            "pypi-attestations",
            "pypi-attestations; the release verify step checks a PyPI distribution's attestations with it",
            &["--help"],
        ),
    ]
}

/// A helper binary answers its version call. `env_override` names the
/// substitute, which is also what keeps tests hermetic; presence is the
/// whole question, because the tools here take no configuration.
fn tool(
    id: &'static str,
    env_override: &str,
    default_bin: &str,
    label: &str,
    args: &[&str],
) -> ProbeResult {
    let bin = std::env::var(env_override).unwrap_or_else(|_| default_bin.to_owned());
    match Command::new(&bin).args(args).output() {
        Ok(out) if out.status.success() => {
            ProbeResult::ok(id, ProbeClass::Soft, format!("{default_bin} runs"))
        }
        Ok(_) => ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!("{default_bin} does not answer {}", args.join(" ")),
            format!("repair {label}"),
        ),
        Err(_) => ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!("{default_bin} is not on PATH"),
            format!("install {label}"),
        ),
    }
}

/// A POSIX shell runs; every setup step spawns through it.
fn shell() -> ProbeResult {
    let id = "sh";
    match Command::new("sh").args(["-c", "exit 0"]).status() {
        Ok(status) if status.success() => ProbeResult::ok(id, ProbeClass::Hard, "sh runs"),
        Ok(status) => ProbeResult::failed(
            id,
            ProbeClass::Hard,
            format!("sh exited {status}"),
            "repair the POSIX shell on PATH",
        ),
        Err(source) => ProbeResult::failed(
            id,
            ProbeClass::Hard,
            format!("sh does not spawn: {source}"),
            "install a POSIX shell on PATH",
        ),
    }
}

/// The XDG state root accepts writes; the log and every run journal live
/// under it.
fn state_root() -> ProbeResult {
    let id = "state-root";
    let Some(root) = crate::applog::state_root() else {
        return ProbeResult::failed(
            id,
            ProbeClass::Hard,
            "neither XDG_STATE_HOME nor HOME is set",
            "export HOME, or XDG_STATE_HOME",
        );
    };
    let display = root.display().to_string();
    let probe = root.join(format!(".probe-{}", std::process::id()));
    let written = std::fs::create_dir_all(&root).and_then(|()| std::fs::write(&probe, b"probe"));
    let _ = std::fs::remove_file(&probe);
    match written {
        Ok(()) => ProbeResult::ok(id, ProbeClass::Hard, format!("{display} is writable")),
        Err(source) => ProbeResult::failed(
            id,
            ProbeClass::Hard,
            format!("{display} is not writable: {source}"),
            format!("make {display} writable"),
        ),
    }
}

/// The working directory's `origin` remote parses to a host, which is
/// what forge and slug detection read.
fn git_remote() -> ProbeResult {
    let id = "git-remote";
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output();
    let url = match out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_owned(),
        _ => {
            return ProbeResult::failed(
                id,
                ProbeClass::Soft,
                "the working directory has no origin remote",
                "pass --repo <owner/name> where a command needs the slug",
            );
        }
    };
    // The raw remote never reaches the message: a malformed URL can carry
    // userinfo — `https://user:token@…` — and a probe result lands in
    // captured output and CI logs, where a credential must never appear.
    remote_host(&url).map_or_else(
        || {
            ProbeResult::failed(
                id,
                ProbeClass::Soft,
                "the origin remote does not parse to a host",
                "pass --repo <owner/name> where a command needs the slug",
            )
        },
        |host| ProbeResult::ok(id, ProbeClass::Soft, format!("origin resolves to {host}")),
    )
}

/// The host in a git remote URL, for the `scp`-like and URL forms.
fn remote_host(url: &str) -> Option<String> {
    if let Some(rest) = url.split_once("://").map(|(_, rest)| rest) {
        let authority = rest.split('/').next()?;
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let host = host.split(':').next()?;
        return (!host.is_empty()).then(|| host.to_owned());
    }
    let (authority, path) = url.split_once(':')?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    (!host.is_empty() && !path.is_empty()).then(|| host.to_owned())
}

/// A forge CLI is present and authenticated. `env_override` names the
/// variable that substitutes the binary, which is also what keeps tests
/// hermetic. `attempts` is tried in order and the first success wins, so
/// a probe can prefer a sharper flag and still work where the CLI
/// predates it.
fn forge_cli(
    id: &'static str,
    env_override: &str,
    default_bin: &str,
    label: &str,
    login: &str,
    attempts: &[&[&str]],
) -> ProbeResult {
    let bin = std::env::var(env_override).unwrap_or_else(|_| default_bin.to_owned());
    let mut spawned = false;
    for args in attempts {
        match Command::new(&bin).args(*args).output() {
            Ok(out) if out.status.success() => {
                return ProbeResult::ok(
                    id,
                    ProbeClass::Soft,
                    format!("{default_bin} is authenticated"),
                );
            }
            Ok(_) => spawned = true,
            Err(_) => {}
        }
    }
    if spawned {
        ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!("{default_bin} is not authenticated"),
            format!("run {login}"),
        )
    } else {
        ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!("{default_bin} is not on PATH"),
            format!("install {label}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::remote_host;

    #[test]
    fn a_remote_host_parses_from_both_url_forms() {
        assert_eq!(
            remote_host("https://github.com/owner/name.git").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            remote_host("git@gitlab.com:group/sub/name.git").as_deref(),
            Some("gitlab.com")
        );
        assert_eq!(
            remote_host("ssh://git@github.com:22/owner/name.git").as_deref(),
            Some("github.com")
        );
        assert_eq!(remote_host("not a url"), None);
    }
}
