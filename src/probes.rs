//! The environment probe catalog.
//!
//! One catalog, read by every caller that needs to know whether this host
//! is ready: `rk doctor` runs it whole, and a mutating command guards the
//! subset it depends on at entry, so the per-command guards and the
//! doctor cannot drift apart. Each probe answers with a status, a
//! message, and — on failure — the remediation printed verbatim wherever
//! the probe is consulted.

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::skills::record::{RECORD_PATH, Record};
use crate::skills::{AGENTS_ROOT, CLAUDE_ROOT, Digest, SHARED_ROOT};

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

/// The probes judging the skill installation itself, in catalog order.
///
/// Declared here rather than derived by running the catalog: the shared plan
/// gate's pre-flight phase must name each of these, and the test holding it to
/// that must not have to spawn a forge CLI or write into the operator's home
/// to learn what they are.
pub const SKILL_PROBES: [&str; 3] = ["skill-roots", "skill-gate", "skill-payload"];

/// Every executable a Hard probe requires, paired with the nixpkgs package
/// whose `bin/` supplies it in the installed package's wrapper.
///
/// `nix/package.nix` mirrors this list by hand — Nix cannot read this
/// registry, and generating one list from the other is more machinery than
/// two entries earn — so the mirror test in `tests/cli.rs` holds the two
/// lists to agreement and a divergence fails by name instead of shipping.
pub const HARD_RUNTIME_TOOLS: [(&str, &str); 2] = [("git", "git"), ("sh", "bash")];

/// One owner for the git binary every production launcher spawns.
///
/// `RK_GIT_BIN` substitutes it — the same contract every soft tool's
/// override states, and what lets an operator's own git win over the one
/// the installed package's wrapper supplies.
#[must_use]
pub fn git_bin() -> std::ffi::OsString {
    std::env::var_os("RK_GIT_BIN").unwrap_or_else(|| "git".into())
}

/// One owner for the POSIX shell every setup step spawns through.
///
/// `RK_SH_BIN` substitutes it, which is also what keeps tests hermetic on
/// a host whose `sh` is not the one under test.
#[must_use]
pub fn sh_bin() -> std::ffi::OsString {
    std::env::var_os("RK_SH_BIN").unwrap_or_else(|| "sh".into())
}

/// Run the whole catalog, in its stable order.
#[must_use]
pub fn run_all() -> Vec<ProbeResult> {
    vec![
        shell(),
        git(),
        state_root(),
        skill_roots(),
        skill_gate(),
        skill_payload(),
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
    match Command::new(sh_bin()).args(["-c", "exit 0"]).status() {
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

/// Version control answers; every branch, worktree, landing, and setup
/// verb launches it.
fn git() -> ProbeResult {
    let id = "git";
    match Command::new(git_bin()).arg("--version").output() {
        Ok(out) if out.status.success() => ProbeResult::ok(id, ProbeClass::Hard, "git runs"),
        Ok(_) => ProbeResult::failed(
            id,
            ProbeClass::Hard,
            "git does not answer --version",
            "repair the git on PATH, or point RK_GIT_BIN at a working one",
        ),
        Err(_) => ProbeResult::failed(id, ProbeClass::Hard, "git is not on PATH", "install git"),
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

/// The destinations `rk skill install` writes accept writes: the two agent
/// roots and the shared root, all under the invoking user's home.
///
/// A root can exist and still refuse, which is what a read-only bind of an
/// agent directory produces, so what is tested is the nearest existing
/// ancestor — the directory an install would actually have to write
/// through. The probe creates nothing: a preview must still be able to
/// report a root as absent, and a probe that made it exist would take that
/// answer away.
fn skill_roots() -> ProbeResult {
    let id = SKILL_PROBES[0];
    let Ok(home) = crate::skills::home() else {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            "neither HOME nor USERPROFILE is set, so no skill root resolves",
            "export HOME",
        );
    };
    let mut refused = Vec::new();
    for root in [CLAUDE_ROOT, AGENTS_ROOT, SHARED_ROOT] {
        let root = home.join(root);
        let Some(existing) = nearest_existing(&root) else {
            refused.push(format!("no ancestor of {root} exists"));
            continue;
        };
        if let Err(source) = accepts_a_write(&existing) {
            refused.push(format!("{existing} is not writable: {source}"));
        }
    }
    if refused.is_empty() {
        ProbeResult::ok(
            id,
            ProbeClass::Soft,
            format!("the skill roots under {home} accept writes"),
        )
    } else {
        ProbeResult::failed(
            id,
            ProbeClass::Soft,
            refused.join("; "),
            format!("make the skill roots under {home} writable"),
        )
    }
}

/// The artifacts every skill shares are installed, and are this binary's.
///
/// This is the probe that answers the one failure a shared home produces.
/// The agent roots and the shared root are separate directories, so a
/// container, a sandbox, or a sync that carries one and not the other
/// leaves every skill resolvable by name and unable to read the gates it is
/// told to read first. A skill that cannot read them runs neither its
/// pre-flight nor its plan phase, which is the whole reason they are files
/// rather than prose.
fn skill_gate() -> ProbeResult {
    let id = SKILL_PROBES[1];
    let Ok(home) = crate::skills::home() else {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            "neither HOME nor USERPROFILE is set, so the shared root does not resolve",
            "export HOME",
        );
    };
    let root = home.join(SHARED_ROOT);
    let record = Record::load(&home.join(RECORD_PATH));
    let planned: Vec<(Utf8PathBuf, &'static [u8])> = crate::skills::shared()
        .into_iter()
        .map(|artifact| (root.join(&artifact.path), artifact.bytes))
        .collect();
    let found = judge(planned, &record);
    if let Some(first) = found.missing.first() {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!("a shared artifact every skill reads before acting is not installed: {first}"),
            "rk skill install --apply",
        );
    }
    if !found.differing.is_empty() {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!(
                "{} shared artifact(s) under {root} are not this binary's",
                found.differing.len()
            ),
            reinstall(found.all_recorded),
        );
    }
    ProbeResult::ok(
        id,
        ProbeClass::Soft,
        format!("{root} holds this binary's shared artifacts"),
    )
}

/// The skills installed under this home are the ones this binary carries.
///
/// One binary serves every repository, so a skill under an agent root and
/// the `rk` on PATH are two artifacts that can be updated apart: a home
/// shared with a container, a sandbox, or another machine can hold skills
/// some other build installed. The probe names that drift rather than
/// leaving an agent to follow instructions the binary no longer answers.
fn skill_payload() -> ProbeResult {
    let id = SKILL_PROBES[2];
    let Ok(home) = crate::skills::home() else {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            "neither HOME nor USERPROFILE is set, so no agent root resolves",
            "export HOME",
        );
    };
    let Ok(skills) = crate::skills::all() else {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            "this binary's embedded skills do not read",
            "reinstall rk; the payload it was built from is defective",
        );
    };
    let record = Record::load(&home.join(RECORD_PATH));
    let mut planned = Vec::new();
    for root in [CLAUDE_ROOT, AGENTS_ROOT] {
        let root = home.join(root);
        // An absent agent root is a choice, not a defect: `--agent` selects
        // one family and leaves the other's root untouched.
        if !root.is_dir() {
            continue;
        }
        for skill in &skills {
            planned.push((
                root.join(&skill.name).join("SKILL.md"),
                skill.text.as_bytes(),
            ));
        }
    }
    if planned.is_empty() {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!("no agent skill root exists under {home}"),
            "rk skill install --apply",
        );
    }
    let found = judge(planned, &record);
    if let Some(first) = found.missing.first() {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!(
                "{} of this binary's skills are not installed, the first at {first}",
                found.missing.len()
            ),
            "rk skill install --apply",
        );
    }
    if !found.differing.is_empty() {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!(
                "{} installed skill(s) are not this binary's; rk is {}",
                found.differing.len(),
                env!("CARGO_PKG_VERSION")
            ),
            reinstall(found.all_recorded),
        );
    }
    ProbeResult::ok(
        id,
        ProbeClass::Soft,
        format!(
            "{} installed skill destination(s) are this binary's",
            found.matching
        ),
    )
}

/// What sits at each destination the payload names.
struct Installed {
    /// Destinations the payload names that hold no readable file.
    missing: Vec<Utf8PathBuf>,
    /// Destinations holding bytes that are not this binary's.
    differing: Vec<Utf8PathBuf>,
    /// How many destinations hold exactly this binary's bytes.
    matching: usize,
    /// Whether the record vouches for every differing destination, which
    /// makes the difference a stale install rather than the operator's own
    /// edit — and decides whether the fix needs `--force`.
    all_recorded: bool,
}

/// Judge each destination the payload names against what sits on disk.
fn judge(planned: Vec<(Utf8PathBuf, &'static [u8])>, record: &Record) -> Installed {
    let mut found = Installed {
        missing: Vec::new(),
        differing: Vec::new(),
        matching: 0,
        all_recorded: true,
    };
    for (destination, bytes) in planned {
        match std::fs::read(&destination) {
            Ok(held) if held == bytes => found.matching += 1,
            Ok(held) => {
                if !record.wrote(&destination, &Digest::of(&held)) {
                    found.all_recorded = false;
                }
                found.differing.push(destination);
            }
            Err(_) => found.missing.push(destination),
        }
    }
    found
}

/// The install that corrects a difference. Bytes the record vouches for are
/// an older release's and go without asking; bytes it cannot account for are
/// the operator's own, and overwriting those is what `--force` is.
const fn reinstall(all_recorded: bool) -> &'static str {
    if all_recorded {
        "rk skill install --apply"
    } else {
        "rk skill install --apply --force"
    }
}

/// The nearest ancestor of `path`, itself included, that exists as a
/// directory.
fn nearest_existing(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut current = Some(path);
    while let Some(dir) = current {
        if dir.is_dir() {
            return Some(dir.to_owned());
        }
        current = dir.parent();
    }
    None
}

/// A directory accepts a write, leaving nothing behind.
fn accepts_a_write(dir: &Utf8Path) -> std::io::Result<()> {
    let probe = dir.join(format!(".rk-probe-{}", std::process::id()));
    let written = std::fs::write(&probe, b"probe");
    let _ = std::fs::remove_file(&probe);
    written
}

/// The working directory's `origin` remote parses to a host, which is
/// what forge and slug detection read.
fn git_remote() -> ProbeResult {
    let id = "git-remote";
    let out = Command::new(git_bin())
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
