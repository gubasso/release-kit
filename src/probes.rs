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

use crate::detect::Forge;
use crate::skills::record::{RECORD_PATH, Record};
use crate::skills::{AGENTS_ROOT, CLAUDE_ROOT, Digest};

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
pub const SKILL_PROBES: [&str; 1] = ["skill-shadow"];

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
    crate::requirements::Tool::Git.program()
}

/// One owner for the nix binary every devshell launch spawns: the lock
/// refresh, the system probe, and the build. `RK_NIX_BIN` substitutes it.
#[must_use]
pub fn nix_bin() -> std::ffi::OsString {
    crate::requirements::Tool::Nix.program()
}

/// One owner for the direnv binary, which nothing here spawns but the
/// doctor probes: it is what loads a consumer's devshell on entry.
/// `RK_DIRENV_BIN` substitutes it.
#[must_use]
pub fn direnv_bin() -> std::ffi::OsString {
    std::env::var_os("RK_DIRENV_BIN")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "direnv".into())
}

/// Nix answers; `rk self-depend sync` updates and builds the pinned devshell
/// with it. Soft, and deliberately outside the wrapper: wrapping nix
/// would put it inside the package's own closure on every host.
#[must_use]
pub fn nix() -> ProbeResult {
    tool(
        "nix",
        "RK_NIX_BIN",
        "nix",
        "Nix; rk self-depend sync updates and builds the pinned devshell with it",
        &["--version"],
    )
}

/// direnv answers; it loads the devshell on directory entry.
#[must_use]
pub fn direnv() -> ProbeResult {
    tool(
        "direnv",
        "RK_DIRENV_BIN",
        "direnv",
        "direnv; it loads the devshell on directory entry",
        &["version"],
    )
}

/// One owner for the POSIX shell every setup step spawns through.
///
/// `RK_SH_BIN` substitutes it, which is also what keeps tests hermetic on
/// a host whose `sh` is not the one under test.
#[must_use]
pub fn sh_bin() -> std::ffi::OsString {
    crate::requirements::Tool::Sh.program()
}

/// One owner for the git-cliff binary the release engine spawns.
///
/// `RK_GIT_CLIFF_BIN` substitutes it, the same contract `RK_GIT_BIN`
/// states. The engine compares the binary's version with the
/// `versions.toml` pin before it trusts any output.
#[must_use]
pub fn git_cliff_bin() -> std::ffi::OsString {
    crate::requirements::Tool::GitCliff.program()
}

/// One owner for the uv binary the python driver adapter spawns.
///
/// `RK_UV_BIN` substitutes it. The engine runs the uv the job installed,
/// and uv enforces the target's own `required-version`.
#[must_use]
pub fn uv_bin() -> std::ffi::OsString {
    crate::requirements::Tool::Uv.program()
}

/// Run the whole catalog, in its stable order.
#[must_use]
pub fn run_all() -> Vec<ProbeResult> {
    let mut probes = vec![
        shell(),
        git(),
        state_root(),
        skill_shadow(),
        git_remote(),
        forge_cli(
            "gh-auth",
            "RK_GH_BIN",
            "gh",
            "the GitHub CLI",
            "gh auth login",
            // `gh auth status` fails when any stored account is broken,
            // even while the active one works; `--active` judges only the
            // credential this tool would use, and the gh floor is the
            // release that added it.
            &[&["auth", "status", "--active"]],
        ),
        forge_cli(
            "glab-auth",
            "RK_GLAB_BIN",
            "glab",
            "the GitLab CLI",
            "glab auth login",
            &[&["auth", "status"]],
        ),
        forge_cli_floor(Forge::Github),
        forge_cli_floor(Forge::Gitlab),
        registry(),
        nix(),
        direnv(),
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
    ];
    probes.extend(hook_probes());
    probes
}

/// The hooks of the repository at the current directory, a report for a
/// project that uses rk: whether the `pre-merge-commit` hook that
/// `rk integrate` asks git to run is installed, and whether each hook
/// manager that wrote an installed hook answers. A hook no known manager
/// wrote is the project's own. Nothing outside a repository, and nothing
/// here changes what a verb runs.
fn hook_probes() -> Vec<ProbeResult> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    let Some(installed) = crate::hooks::installed(&cwd) else {
        return Vec::new();
    };
    let names = |filter: &dyn Fn(crate::hooks::Writer) -> bool| -> Vec<&'static str> {
        installed
            .iter()
            .filter(|hook| filter(hook.writer))
            .map(|hook| hook.event)
            .collect()
    };
    let mut probes = vec![if installed
        .iter()
        .any(|hook| hook.event == "pre-merge-commit")
    {
        ProbeResult::ok(
            "pre-merge-commit-hook",
            ProbeClass::Soft,
            "a pre-merge-commit hook is installed, the gate rk integrate asks git to run",
        )
    } else {
        ProbeResult::failed(
            "pre-merge-commit-hook",
            ProbeClass::Soft,
            "no pre-merge-commit hook is installed, so rk integrate has no gate to run in this clone",
            "install it with the project's hook manager, for example prek install --hook-type pre-merge-commit",
        )
    }];
    for (id, writer) in [
        ("prek", crate::hooks::Writer::Prek),
        ("pre-commit", crate::hooks::Writer::PreCommit),
    ] {
        let hooks = names(&|found| found == writer);
        if hooks.is_empty() {
            continue;
        }
        let answers = Command::new(id)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success());
        let wrote = hooks.join(", ");
        probes.push(if answers {
            ProbeResult::ok(id, ProbeClass::Soft, format!("{id} runs; it wrote the {wrote} hooks"))
        } else {
            ProbeResult::failed(
                id,
                ProbeClass::Soft,
                format!("{id} does not run, and the {wrote} hooks call it"),
                format!("install {id} on the host, or reinstall the hooks with the project's hook manager"),
            )
        });
    }
    let configured = names(&|found| found == crate::hooks::Writer::Configured);
    if !configured.is_empty() {
        let mut events = configured.clone();
        events.dedup();
        probes.push(ProbeResult::ok(
            "configured-hooks",
            ProbeClass::Soft,
            format!(
                "git's configuration defines {} hooks for {}",
                configured.len(),
                events.join(", ")
            ),
        ));
    }
    let own = names(&|found| found == crate::hooks::Writer::Own);
    if !own.is_empty() {
        probes.push(ProbeResult::ok(
            "own-hooks",
            ProbeClass::Soft,
            format!(
                "the {} hooks are the project's own, written by no known hook manager",
                own.join(", ")
            ),
        ));
    }
    probes
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
    let bin = std::env::var(env_override)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_bin.to_owned());
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

/// The crates.io index answers, so `rk versions --check` and
/// `rk self-depend sync` can compare a pin against the registry. Soft: a
/// host that cannot reach it is a constraint on the two verbs that fetch,
/// never a broken install, and every offline verb runs without it.
fn registry() -> ProbeResult {
    let id = "registry";
    let answered = crate::http::send(
        &crate::http::Request::get(
            "https://index.crates.io/config.json",
            std::time::Duration::from_secs(5),
        )
        .following(),
    )
    .is_ok_and(|answer| answer.ok());
    if answered {
        ProbeResult::ok(id, ProbeClass::Soft, "the crates.io index answers")
    } else {
        ProbeResult::failed(
            id,
            ProbeClass::Soft,
            "the crates.io index does not answer",
            "reach the network before rk versions --check or rk self-depend sync; every offline verb runs without it",
        )
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

/// No user-scope copy of an embedded skill shadows the project's landed
/// one.
///
/// Claude Code resolves a name collision as "personal over project", and
/// Codex lists both copies, so a `SKILL.md` an earlier release installed
/// under the home wins over the skills every landing now carries. A folder
/// without `SKILL.md`, such as one that keeps the operator's notes after
/// an uninstall, passes. The probe creates nothing.
///
/// SATISFIES distribution:the-doctor-names-a-shadowing-user-copy
fn skill_shadow() -> ProbeResult {
    let id = SKILL_PROBES[0];
    let Ok(home) = crate::skills::home() else {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            "neither HOME nor USERPROFILE is set, so no user-scope skill root resolves",
            "export HOME",
        );
    };
    let Ok(skills) = crate::skills::all() else {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            "this binary's embedded skills do not read",
            "reinstall rk; the sources it was built from are defective",
        );
    };
    let record = Record::load(&home.join(RECORD_PATH));
    let mut vouched = Vec::new();
    let mut edited = Vec::new();
    for root in [CLAUDE_ROOT, AGENTS_ROOT] {
        for skill in &skills {
            let manual = home.join(root).join(&skill.name).join("SKILL.md");
            if std::fs::symlink_metadata(&manual).is_err() {
                continue;
            }
            let ours = std::fs::read(&manual)
                .is_ok_and(|bytes| record.wrote(&manual, &Digest::of(&bytes)));
            if ours {
                vouched.push(manual);
            } else {
                edited.push(manual);
            }
        }
    }
    if let Some(first) = edited.first() {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!(
                "{} user-scope SKILL.md file(s) shadow the project's landed skills, and the uninstall kept them as your edits, the first at {first}",
                edited.len()
            ),
            format!(
                "read {first} and each other kept copy, keep what you need elsewhere, and remove them by hand"
            ),
        );
    }
    if let Some(first) = vouched.first() {
        return ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!(
                "{} user-scope SKILL.md file(s) that an earlier release installed shadow the project's landed skills, the first at {first}",
                vouched.len()
            ),
            "rk skill uninstall --apply",
        );
    }
    ProbeResult::ok(
        id,
        ProbeClass::Soft,
        format!("no user-scope copy of an rk skill under {home} shadows a project's landed one"),
    )
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
    let bin = std::env::var(env_override)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_bin.to_owned());
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

/// One owner for a forge CLI's binary name, honoring the override that
/// keeps the tests hermetic.
#[must_use]
pub fn forge_bin(forge: Forge) -> String {
    crate::requirements::Tool::ForgeCli(forge)
        .program()
        .to_string_lossy()
        .into_owned()
}

/// The version probe's stable name.
const fn version_probe_id(forge: Forge) -> &'static str {
    match forge {
        Forge::Github => "gh-version",
        Forge::Gitlab => "glab-version",
    }
}

/// The first `<major>.<minor>.<patch>` run in a version line.
///
/// `gh version 2.19.0 (2022-10-25)` and `glab 1.114.0 (4d7c6cd)` both
/// resolve. Nothing else in the crate parses a version string.
#[must_use]
pub fn parse_cli_version(text: &str) -> Option<(u32, u32, u32)> {
    let bytes = text.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if !bytes[start].is_ascii_digit() {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        let run = &text[start..end];
        let mut parts = run.split('.');
        let parsed = (|| {
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next()?.parse().ok()?;
            let patch = parts.next()?.parse().ok()?;
            Some((major, minor, patch))
        })();
        if let Some(version) = parsed {
            return Some(version);
        }
        start = end.max(start + 1);
    }
    None
}

/// Run `<bin> --version` and parse it. A spawn failure, a non-zero exit,
/// or output carrying no version run all answer `None`, because none of
/// them proves a version this binary can trust.
#[must_use]
pub fn forge_cli_version(bin: &str) -> Option<(u32, u32, u32)> {
    let out = Command::new(bin).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_cli_version(&String::from_utf8_lossy(&out.stdout))
}

/// A forge CLI is present and at or above the floor this binary calls.
///
/// Soft: a host that never starts work from an issue does not need it.
fn forge_cli_floor(forge: Forge) -> ProbeResult {
    let id = version_probe_id(forge);
    let bin = forge_bin(forge);
    let name = forge.cli();
    let floor = forge.cli_floor();
    match forge_cli_version(&bin) {
        Some(found) if found >= floor => ProbeResult::ok(
            id,
            ProbeClass::Soft,
            format!("{name} {} is at or above {}", show(found), show(floor)),
        ),
        Some(found) => ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!(
                "{name} {} is below the {} rk calls",
                show(found),
                show(floor)
            ),
            forge.cli_upgrade(),
        ),
        None => ProbeResult::failed(
            id,
            ProbeClass::Soft,
            format!("{name} does not answer --version with a version"),
            format!("install {name}"),
        ),
    }
}

/// `<major>.<minor>.<patch>` for a message.
fn show((major, minor, patch): (u32, u32, u32)) -> String {
    format!("{major}.{minor}.{patch}")
}

#[cfg(test)]
mod tests {
    use super::{parse_cli_version, remote_host};

    /// Both forge CLIs print their version in a different shape, and a
    /// line carrying no version resolves to nothing rather than to a
    /// guess.
    #[test]
    fn a_version_line_parses_from_both_forge_clis() {
        assert_eq!(
            parse_cli_version("gh version 2.19.0 (2022-10-25)"),
            Some((2, 19, 0))
        );
        assert_eq!(
            parse_cli_version("glab 1.114.0 (4d7c6cd)\n"),
            Some((1, 114, 0))
        );
        assert_eq!(parse_cli_version("gh version 2.99.0"), Some((2, 99, 0)));
        assert_eq!(parse_cli_version("no version here"), None);
        assert_eq!(parse_cli_version("gh version 2.19"), None);
    }

    /// The floor comparison orders by component, so no string comparison
    /// survives it.
    #[test]
    fn a_floor_comparison_orders_by_component() {
        assert!((2, 100, 0) > (2, 99, 0));
        assert!((2, 9, 0) < (2, 19, 0));
        assert!((2, 19, 0) >= (2, 19, 0));
    }

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
