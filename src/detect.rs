//! Forge, repository, and technology detection.
//!
//! One pass reads `git remote get-url origin`: the path is the project, the
//! host chooses the forge. An unrecognized host is never defaulted — a wrong
//! guess runs protection calls against the wrong API and fails partway
//! through a setup — so callers refuse and name the override flags instead.
//! The technology is read from the version file, exactly as the bindings
//! define it.

use std::path::Path;
use std::process::Command;

/// A supported forge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    /// github.com, driven through `gh`.
    Github,
    /// gitlab.com or a self-hosted GitLab, driven through `glab`.
    Gitlab,
}

impl Forge {
    /// The wire and directory name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
        }
    }

    /// Parse a `--forge` value.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "github" => Some(Self::Github),
            "gitlab" => Some(Self::Gitlab),
            _ => None,
        }
    }

    /// The forge CLI this forge is driven through.
    #[must_use]
    pub const fn cli(self) -> &'static str {
        match self {
            Self::Github => "gh",
            Self::Gitlab => "glab",
        }
    }

    /// Every supported forge, in a stable order.
    pub const ALL: [Self; 2] = [Self::Github, Self::Gitlab];
}

/// What one detection pass observed; every field is an observation, and
/// refusing on what is absent is the caller's decision.
#[derive(Debug, Default)]
pub struct Detection {
    /// The remote's host, where a remote exists and parses.
    pub host: Option<String>,
    /// The project path from the remote: no scheme, no `.git` suffix.
    pub repo: Option<String>,
    /// The forge the host maps to; `None` with a `host` present means the
    /// host is unrecognized.
    pub forge: Option<Forge>,
}

/// Read the `origin` remote of `dir` and map it, without judging.
///
/// The read answers for `dir` alone: the variables a running hook
/// exports are scrubbed, so an inherited `GIT_DIR` cannot answer with
/// another repository's remote.
#[must_use]
pub fn detect(dir: &Path) -> Detection {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .args(["-C"])
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .output();
    let url = match out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_owned(),
        _ => return Detection::default(),
    };
    let Some((host, path)) = split_remote(&url) else {
        return Detection::default();
    };
    let forge = forge_for_host(&host);
    Detection {
        host: Some(host),
        repo: Some(path),
        forge,
    }
}

/// The forge a host maps to. `gitlab.com` and hosts that name gitlab map to
/// GitLab; a self-hosted instance on a host name that says nothing needs
/// `--forge`.
#[must_use]
pub fn forge_for_host(host: &str) -> Option<Forge> {
    if host == "github.com" {
        return Some(Forge::Github);
    }
    if host == "gitlab.com" || host.starts_with("gitlab.") {
        return Some(Forge::Gitlab);
    }
    None
}

/// Host and project path from a git remote URL, for the URL and `scp`-like
/// forms. The path drops a leading slash and a `.git` suffix.
#[must_use]
pub fn split_remote(url: &str) -> Option<(String, String)> {
    let (host, raw_path) = if let Some((_, rest)) = url.split_once("://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let host = host.split(':').next()?;
        (host.to_owned(), path.to_owned())
    } else {
        let (authority, path) = url.split_once(':')?;
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        (host.to_owned(), path.to_owned())
    };
    let path = raw_path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_owned();
    (!host.is_empty() && !path.is_empty()).then_some((host, path))
}

/// The technology of a repository, read from its version file: `Cargo.toml`
/// means rust, `pyproject.toml` means python, a `VERSION` file means bash.
#[must_use]
pub fn tech_of(dir: &Path) -> Option<&'static str> {
    if dir.join("Cargo.toml").is_file() {
        Some("rust")
    } else if dir.join("pyproject.toml").is_file() {
        Some("python")
    } else if dir.join("VERSION").is_file() {
        Some("bash")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{Forge, forge_for_host, split_remote};

    #[test]
    fn a_remote_splits_into_host_and_path_in_both_forms() {
        assert_eq!(
            split_remote("https://github.com/owner/name.git"),
            Some(("github.com".into(), "owner/name".into()))
        );
        assert_eq!(
            split_remote("git@gitlab.com:group/sub/name.git"),
            Some(("gitlab.com".into(), "group/sub/name".into()))
        );
        assert_eq!(
            split_remote("ssh://git@github.com:22/owner/name.git"),
            Some(("github.com".into(), "owner/name".into()))
        );
        assert_eq!(split_remote("not a url"), None);
    }

    #[test]
    fn a_host_maps_to_its_forge_and_an_unknown_host_to_none() {
        assert_eq!(forge_for_host("github.com"), Some(Forge::Github));
        assert_eq!(forge_for_host("gitlab.com"), Some(Forge::Gitlab));
        assert_eq!(forge_for_host("gitlab.example.org"), Some(Forge::Gitlab));
        assert_eq!(forge_for_host("codeberg.org"), None);
    }
}
