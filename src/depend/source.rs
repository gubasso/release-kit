//! What the dependency's checkout declares about its distribution.
//!
//! Every signal is a file in the source tree or its git remote and tags:
//! a manifest names the package and its version, `dist-workspace.toml`
//! says release archives are built, a flake that serves `packages`
//! says the repository is a flake input. No registry is asked whether
//! the version is published; the assessment is offline.

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use super::{Channel, canonical_dir};
use crate::error::RkError;

/// Which shape the source's release tags take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TagStyle {
    /// `v1.2.3`.
    Prefixed,
    /// `1.2.3`.
    Bare,
    /// No tag, or no majority: the prefixed form is assumed.
    Unknown,
}

/// One channel and the evidence behind it.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelEvidence {
    /// The channel.
    pub channel: Channel,
    /// The files and facts that make it viable.
    pub evidence: Vec<String>,
}

/// What one manifest declares, whatever its format.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The package name.
    pub name: Option<String>,
    /// The declared version.
    pub version: Option<String>,
    /// The executables it installs.
    pub bins: Vec<String>,
    /// The repository URL it names.
    pub repository: Option<String>,
    /// `[package.metadata.binstall]`: `None` where absent, `Some(None)`
    /// for a table with no `pkg-url`, `Some(Some(url))` for its template.
    pub binstall: Option<Option<String>>,
}

/// Everything the offline pass reads from the source.
#[derive(Debug, Clone)]
pub struct Source {
    /// The checkout, canonical.
    pub path: Utf8PathBuf,
    /// `rust`, `python`, `node`, or `bash`, where a manifest says.
    pub tech: Option<&'static str>,
    /// The package name.
    pub name: Option<String>,
    /// The declared version.
    pub version: Option<String>,
    /// The executables the package installs.
    pub bins: Vec<String>,
    /// `owner/repo`, from the remote or the manifest.
    pub owner_repo: Option<String>,
    /// The remote's host.
    pub host: Option<String>,
    /// Whether `flake.nix` serves a package output.
    pub flake_package: bool,
    /// Whether `dist-workspace.toml` declares GitHub as its CI and does
    /// not send hosting elsewhere.
    pub dist_github: bool,
    /// Whether the manifest carries binstall metadata whose package URL
    /// is the GitHub release default, or names GitHub.
    pub binstall_github: bool,
    /// The shape of the release tags.
    pub tag_style: TagStyle,
    /// The viable channels, in the closed order.
    pub channels: Vec<ChannelEvidence>,
}

impl Source {
    /// The executable to name for a prebuilt archive: the first declared
    /// bin, else the package name.
    #[must_use]
    pub fn bin(&self) -> Option<&str> {
        self.bins
            .first()
            .map(String::as_str)
            .or(self.name.as_deref())
    }

    /// Whether a channel is viable.
    #[must_use]
    pub fn has(&self, channel: Channel) -> bool {
        self.channels.iter().any(|c| c.channel == channel)
    }
}

/// Read the source, offline.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a path that is not a directory and
/// [`RkError::Io`] where a present file does not read.
pub fn observe(path: &Utf8Path) -> Result<Source, RkError> {
    let path = canonical_dir(path, "source")?;
    let cargo = read_optional(&path.join("Cargo.toml"))?.map(|t| read_cargo(&t));
    let pyproject = read_optional(&path.join("pyproject.toml"))?.map(|t| read_pyproject(&t));
    let package = match std::fs::read(path.join("package.json")) {
        Ok(bytes) => Some(read_package_json(&bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(RkError::Io(e)),
    };
    let (tech, manifest) = match (&cargo, &pyproject, &package) {
        (Some(m), _, _) => (Some("rust"), m.clone()),
        (None, Some(m), _) => (Some("python"), m.clone()),
        (None, None, Some(m)) => (Some("node"), m.clone()),
        (None, None, None) => (
            path.join("VERSION").is_file().then_some("bash"),
            Manifest::default(),
        ),
    };
    let mut bins = manifest.bins.clone();
    if tech == Some("rust") && bins.is_empty() && path.join("src/main.rs").is_file() {
        bins.extend(manifest.name.clone());
    }
    let detected = crate::detect::detect(path.as_std_path());
    let (host, owner_repo) = match (detected.host, detected.repo) {
        (Some(host), Some(repo)) => (Some(host), Some(repo)),
        _ => manifest
            .repository
            .as_deref()
            .and_then(crate::detect::split_remote)
            .map_or((None, None), |(host, repo)| (Some(host), Some(repo))),
    };
    let flake_package =
        read_optional(&path.join("flake.nix"))?.is_some_and(|text| flake_serves_package(&text));
    let tags = git_tags(&path);
    let binstall_github =
        binstall_resolves_to_github(manifest.binstall.as_ref(), owner_repo.as_deref());
    let mut source = Source {
        path,
        tech,
        name: manifest.name,
        version: manifest.version,
        bins,
        owner_repo,
        host,
        flake_package,
        dist_github: false,
        binstall_github,
        tag_style: tag_style(&tags),
        channels: Vec::new(),
    };
    source.dist_github = read_optional(&source.path.join("dist-workspace.toml"))?
        .is_some_and(|text| dist_hosts_on_github(&text));
    source.channels = channels_of(&source);
    Ok(source)
}

/// The channels the evidence supports, in the closed order.
#[must_use]
pub fn channels_of(source: &Source) -> Vec<ChannelEvidence> {
    let mut out = Vec::new();
    let named = source.name.is_some();
    if named && source.tech == Some("rust") {
        out.push(ChannelEvidence {
            channel: Channel::Crates,
            evidence: vec!["Cargo.toml names a package".into()],
        });
    }
    if source.flake_package {
        out.push(ChannelEvidence {
            channel: Channel::Flake,
            evidence: vec!["flake.nix serves a packages output".into()],
        });
    }
    if named && source.tech == Some("python") {
        out.push(ChannelEvidence {
            channel: Channel::Pypi,
            evidence: vec!["pyproject.toml names a project".into()],
        });
    }
    if named && source.tech == Some("node") {
        out.push(ChannelEvidence {
            channel: Channel::Npm,
            evidence: vec!["package.json names a package".into()],
        });
    }
    let on_github = source.host.as_deref() == Some("github.com") && source.owner_repo.is_some();
    if on_github && (source.dist_github || source.binstall_github) {
        let mut evidence = Vec::new();
        if source.dist_github {
            evidence.push("dist-workspace.toml names github as its ci and hosting".into());
        }
        if source.binstall_github {
            evidence.push("Cargo.toml binstall metadata resolves to GitHub releases".into());
        }
        out.push(ChannelEvidence {
            channel: Channel::GithubRelease,
            evidence,
        });
    }
    out
}

/// What a `Cargo.toml` declares: the package, its bins, its repository.
#[must_use]
pub fn read_cargo(text: &str) -> Manifest {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Manifest::default();
    };
    let package = table.get("package").and_then(toml::Value::as_table);
    let field = |key: &str| {
        package
            .and_then(|p| p.get(key))
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    let bins = table
        .get("bin")
        .and_then(toml::Value::as_array)
        .map(|bins| {
            bins.iter()
                .filter_map(|bin| bin.get("name").and_then(toml::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let binstall = package
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("binstall"))
        .map(|binstall| {
            binstall
                .get("pkg-url")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        });
    Manifest {
        name: field("name"),
        version: field("version"),
        bins,
        repository: field("repository"),
        binstall,
    }
}

/// Whether binstall metadata resolves to this repository's GitHub
/// releases.
///
/// A table with no `pkg-url` takes cargo-binstall's GitHub default, and
/// a custom `pkg-url` counts only under `/releases/download/` of the
/// `{ repo }` template or of this repository's own path on `github.com`.
#[must_use]
pub fn binstall_resolves_to_github(
    binstall: Option<&Option<String>>,
    owner_repo: Option<&str>,
) -> bool {
    match binstall {
        None => false,
        Some(None) => true,
        Some(Some(url)) => {
            url.contains("/releases/download/")
                && (url.contains("{ repo }")
                    || url.contains("{repo}")
                    || owner_repo.is_some_and(|repo| {
                        url.starts_with(&format!("https://github.com/{repo}/releases/download/"))
                    }))
        }
    }
}

/// Whether a `dist-workspace.toml` hosts its releases on GitHub: `ci`
/// names `github`, and `hosting`, where set, still names it. A file
/// that does not parse or names another CI is not evidence.
#[must_use]
pub fn dist_hosts_on_github(text: &str) -> bool {
    let Ok(table) = text.parse::<toml::Table>() else {
        return false;
    };
    let dist = table.get("dist").and_then(toml::Value::as_table);
    let names = |key: &str| -> Option<Vec<String>> {
        let value = dist?.get(key)?;
        match value {
            toml::Value::String(s) => Some(vec![s.clone()]),
            toml::Value::Array(items) => Some(
                items
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            ),
            _ => Some(Vec::new()),
        }
    };
    let ci_github = names("ci").is_some_and(|ci| ci.iter().any(|c| c == "github"));
    let hosting_github = names("hosting").is_none_or(|h| h.iter().any(|c| c == "github"));
    ci_github && hosting_github
}

/// What a `pyproject.toml` declares: the project, its scripts, its
/// repository URL.
#[must_use]
pub fn read_pyproject(text: &str) -> Manifest {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Manifest::default();
    };
    let project = table.get("project").and_then(toml::Value::as_table);
    let field = |key: &str| {
        project
            .and_then(|p| p.get(key))
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    let bins = project
        .and_then(|p| p.get("scripts"))
        .and_then(toml::Value::as_table)
        .map(|scripts| scripts.keys().cloned().collect())
        .unwrap_or_default();
    let repository = project
        .and_then(|p| p.get("urls"))
        .and_then(toml::Value::as_table)
        .and_then(|urls| {
            ["Repository", "repository", "Source", "source", "Homepage"]
                .iter()
                .find_map(|key| urls.get(*key))
        })
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    Manifest {
        name: field("name"),
        version: field("version"),
        bins,
        repository,
        binstall: None,
    }
}

/// What a `package.json` declares: the package, its bins, its repository.
#[must_use]
pub fn read_package_json(bytes: &[u8]) -> Manifest {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Manifest::default();
    };
    let text = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    let bins = match value.get("bin") {
        Some(serde_json::Value::String(_)) => text("name").into_iter().collect(),
        Some(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
        _ => Vec::new(),
    };
    let repository = match value.get("repository") {
        Some(serde_json::Value::String(url)) => Some(url.clone()),
        Some(serde_json::Value::Object(map)) => {
            map.get("url").and_then(|v| v.as_str()).map(str::to_owned)
        }
        _ => None,
    }
    .map(|url| url.trim_start_matches("git+").to_owned());
    Manifest {
        name: text("name"),
        version: text("version"),
        bins,
        repository,
        binstall: None,
    }
}

/// Whether a flake serves a default package output, judged lexically.
///
/// Comments and string contents are scrubbed first. The evidence is a
/// `packages.default` or `packages.<system>.default` attribute path, or
/// a `packages =` binding whose
/// own value, up to its terminating `;`, is not a `mkShell` list and
/// binds a `default` attribute.
#[must_use]
pub fn flake_serves_package(text: &str) -> bool {
    let code = super::nix::scrub(text);
    let bytes = code.as_bytes();
    code.match_indices("packages").any(|(index, _)| {
        let rest = &code[index + "packages".len()..];
        if super::nix::is_default_package_path(&code, index) {
            return true;
        }
        if index > 0 && (bytes[index - 1] == b'.' || super::nix::is_ident(bytes[index - 1])) {
            return false;
        }
        let after = rest.trim_start();
        let Some(value) = after.strip_prefix('=') else {
            return false;
        };
        let value = super::nix::binding_value(value);
        !value.trim_start().starts_with('[')
            && super::nix::names_attribute(&super::nix::without_let_bindings(value), "default")
    })
}

/// The tag shape a tag list shows: the majority form, `Unknown` on a tie
/// or an empty list.
#[must_use]
pub fn tag_style(tags: &[String]) -> TagStyle {
    let prefixed = tags
        .iter()
        .filter(|tag| {
            tag.strip_prefix('v')
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
        .count();
    let bare = tags
        .iter()
        .filter(|tag| tag.starts_with(|c: char| c.is_ascii_digit()))
        .count();
    match prefixed.cmp(&bare) {
        std::cmp::Ordering::Greater => TagStyle::Prefixed,
        std::cmp::Ordering::Less => TagStyle::Bare,
        std::cmp::Ordering::Equal => TagStyle::Unknown,
    }
}

/// The source's tags, or none where it is not a repository: an
/// observation, never a failure, because the tag style has a default.
fn git_tags(path: &Utf8Path) -> Vec<String> {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .arg("-C")
        .arg(path.as_std_path())
        .args(["tag", "--list"])
        .output();
    match out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// A file's text, or `None` where it does not exist.
fn read_optional(path: &Utf8Path) -> Result<Option<String>, RkError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(RkError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::{
        Channel, ChannelEvidence, Source, TagStyle, binstall_resolves_to_github, channels_of,
        dist_hosts_on_github, flake_serves_package, read_cargo, read_package_json, read_pyproject,
        tag_style,
    };

    fn sample(tech: Option<&'static str>) -> Source {
        Source {
            path: Utf8PathBuf::from("/srv/sample"),
            tech,
            name: Some("sample-tool".into()),
            version: Some("1.4.0".into()),
            bins: vec!["sam".into()],
            owner_repo: Some("acme/sample-tool".into()),
            host: Some("github.com".into()),
            flake_package: false,
            dist_github: false,
            binstall_github: false,
            tag_style: TagStyle::Prefixed,
            channels: Vec::new(),
        }
    }

    fn channels(source: &Source) -> Vec<Channel> {
        channels_of(source)
            .iter()
            .map(|ChannelEvidence { channel, .. }| *channel)
            .collect()
    }

    #[test]
    fn a_cargo_manifest_yields_name_version_bins_and_repository() {
        let manifest = read_cargo(
            "[package]\nname = \"sample-tool\"\nversion = \"1.4.0\"\nrepository = \"https://github.com/acme/sample-tool\"\n\n[[bin]]\nname = \"sam\"\n\n[package.metadata.binstall]\npkg-url = \"x\"\n",
        );
        assert_eq!(manifest.name.as_deref(), Some("sample-tool"));
        assert_eq!(manifest.version.as_deref(), Some("1.4.0"));
        assert_eq!(manifest.bins, vec!["sam".to_owned()]);
        assert_eq!(
            manifest.repository.as_deref(),
            Some("https://github.com/acme/sample-tool")
        );
        assert_eq!(manifest.binstall, Some(Some("x".to_owned())));
        let default_table = read_cargo(
            "[package]
name = \"t\"
version = \"1.0.0\"

[package.metadata.binstall]
pkg-fmt = \"tgz\"
",
        );
        assert_eq!(default_table.binstall, Some(None));
        assert_eq!(
            read_cargo(
                "[package]
name = \"t\"
"
            )
            .binstall,
            None
        );
        let workspace = read_cargo("[workspace]\nmembers = [\"a\"]\n");
        assert_eq!(workspace.name, None);
        assert!(read_cargo("not = [toml").name.is_none());
    }

    #[test]
    fn a_pyproject_yields_scripts_as_bins() {
        let manifest = read_pyproject(
            "[project]\nname = \"sample\"\nversion = \"2.0.0\"\n\n[project.scripts]\nsam = \"sample:main\"\n\n[project.urls]\nRepository = \"https://github.com/acme/sample\"\n",
        );
        assert_eq!(manifest.name.as_deref(), Some("sample"));
        assert_eq!(manifest.bins, vec!["sam".to_owned()]);
        assert_eq!(
            manifest.repository.as_deref(),
            Some("https://github.com/acme/sample")
        );
    }

    #[test]
    fn a_package_json_yields_bin_in_both_forms() {
        let string_form = read_package_json(
            br#"{"name":"sample","version":"3.1.0","bin":"cli.js","repository":"git+https://github.com/acme/sample.git"}"#,
        );
        assert_eq!(string_form.bins, vec!["sample".to_owned()]);
        assert_eq!(
            string_form.repository.as_deref(),
            Some("https://github.com/acme/sample.git")
        );
        let map_form = read_package_json(
            br#"{"name":"sample","version":"3.1.0","bin":{"sam":"cli.js"},"repository":{"type":"git","url":"https://github.com/acme/sample"}}"#,
        );
        assert_eq!(map_form.bins, vec!["sam".to_owned()]);
        assert_eq!(map_form.version.as_deref(), Some("3.1.0"));
    }

    #[test]
    fn a_flake_that_serves_packages_default_is_recognized() {
        assert!(flake_serves_package(
            "packages.default = pkgs.callPackage ./nix/package.nix { };"
        ));
        assert!(flake_serves_package(
            "packages = eachSystem (pkgs: rec { tool = pkgs.callPackage ./nix/package.nix { }; default = tool; });"
        ));
        assert!(!flake_serves_package(
            "devShells.default = pkgs.mkShell { packages = [ pkgs.just ]; };"
        ));
        assert!(
            !flake_serves_package("# packages = disabled\noutputs = _: {};"),
            "a comment is not code"
        );
        assert!(
            !flake_serves_package(
                "packages = eachSystem (pkgs: { tool = pkgs.callPackage ./nix/package.nix { }; });"
            ),
            "a packages set without a default serves no default"
        );
        assert!(
            !flake_serves_package(
                "packages = eachSystem (pkgs: { tool = pkgs.hello; });\n      devShells.default = pkgs.mkShell { };"
            ),
            "a default after the binding closes is another binding's"
        );
        assert!(
            !flake_serves_package("/* packages.default was here */ outputs = _: {};"),
            "a block comment is not code"
        );
        assert!(
            !flake_serves_package("description = \"packages.default\"; outputs = _: {};"),
            "a string is not code"
        );
        assert!(
            flake_serves_package("packages = let x = pkgs.hello; in { default = x; };"),
            "a let binding's semicolons do not close the value"
        );
        assert!(
            !flake_serves_package("packages = { notdefault = pkgs.hello; };"),
            "default is matched as an attribute token"
        );
        assert!(
            flake_serves_package("packages = with pkgs; { default = hello; };"),
            "a with clause's semicolon does not close the value"
        );
        assert!(
            flake_serves_package("packages = assert true; { default = pkgs.hello; };"),
            "an assert clause's semicolon does not close the value"
        );
        assert!(
            !flake_serves_package(
                "packages = eachSystem (system: let default = pkgs.hello; in { tool = default; });"
            ),
            "a local let binding is not a default output"
        );
        assert!(flake_serves_package(
            "packages.x86_64-linux.default = pkgs.hello;"
        ));
        assert!(
            !flake_serves_package(
                "devShells.default = pkgs.mkShell { packages = [ tool-input.packages.${system}.default ]; };"
            ),
            "a devshell consuming another input's package exports none"
        );
        assert!(
            !flake_serves_package("mypackages.${system}.default = x;"),
            "a longer identifier is not the packages output"
        );
        assert!(!flake_serves_package(
            "my_packages = { default = pkgs.hello; };"
        ));
        assert!(!flake_serves_package(
            "my-packages = { default = pkgs.hello; };"
        ));
        assert!(flake_serves_package(
            "packages.${system}.default = pkgs.hello;"
        ));
        assert!(!flake_serves_package(
            "packages.x86_64-linux.tool = pkgs.hello;"
        ));
        assert!(
            !flake_serves_package(
                "packages = { tool = with pkgs; hello; };\n devShells.default = pkgs.mkShell { };"
            ),
            "a nested with clause does not leak the binding boundary"
        );
        assert!(!flake_serves_package("{ inputs = {}; outputs = _: {}; }"));
    }

    #[test]
    fn binstall_counts_only_as_a_release_asset_of_this_repository() {
        let repo = Some("acme/sample-tool");
        assert!(!binstall_resolves_to_github(None, repo));
        assert!(
            binstall_resolves_to_github(Some(&None), repo),
            "the GitHub default"
        );
        let url = |u: &str| Some(u.to_owned());
        assert!(binstall_resolves_to_github(
            Some(&url(
                "{ repo }/releases/download/v{ version }/{ name }-{ target }.tgz"
            )),
            repo
        ));
        assert!(binstall_resolves_to_github(
            Some(&url(
                "https://github.com/acme/sample-tool/releases/download/v{ version }/x.tgz"
            )),
            repo
        ));
        assert!(
            !binstall_resolves_to_github(
                Some(&url("{ repo }/archive/refs/tags/v{ version }.tar.gz")),
                repo
            ),
            "a source archive is not a release asset"
        );
        assert!(
            !binstall_resolves_to_github(
                Some(&url(
                    "https://raw.githubusercontent.com/acme/sample-tool/main/x.tgz"
                )),
                repo
            ),
            "raw content is not a release asset"
        );
        assert!(
            !binstall_resolves_to_github(
                Some(&url(
                    "https://github.com/other/thing/releases/download/v1/x.tgz"
                )),
                repo
            ),
            "another repository's release is not this source's"
        );
        assert!(
            !binstall_resolves_to_github(
                Some(&url(
                    "https://github.com/acme/sample-tool-fork/releases/download/v1/x.tgz"
                )),
                repo
            ),
            "a repository that merely starts with this one's path is another repository"
        );
        assert!(
            !binstall_resolves_to_github(
                Some(&url(
                    "https://example.org/github.com/acme/sample-tool/releases/download/v1/x.tgz"
                )),
                repo
            ),
            "an embedded host is another host"
        );
        assert!(!binstall_resolves_to_github(
            Some(&url("https://dl.example.org/{ name }-{ version }.tgz")),
            repo
        ));
    }

    #[test]
    fn dist_hosting_is_read_from_the_dist_table() {
        assert!(dist_hosts_on_github("[dist]\nci = \"github\"\n"));
        assert!(dist_hosts_on_github(
            "[dist]\nci = [\"github\"]\nhosting = [\"github\", \"axodotdev\"]\n"
        ));
        assert!(!dist_hosts_on_github(
            "[dist]\nci = \"github\"\nhosting = \"axodotdev\"\n"
        ));
        assert!(!dist_hosts_on_github(
            "[dist]\ntargets = [\"x86_64-unknown-linux-gnu\"]\n"
        ));
        assert!(!dist_hosts_on_github("not = [toml"));
    }

    #[test]
    fn the_tag_style_is_read_from_the_tags() {
        let owned = |tags: &[&str]| tags.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>();
        assert_eq!(tag_style(&owned(&["v1.0.0", "v1.1.0"])), TagStyle::Prefixed);
        assert_eq!(tag_style(&owned(&["1.0.0", "1.1.0"])), TagStyle::Bare);
        assert_eq!(tag_style(&owned(&["v1.0.0", "1.1.0"])), TagStyle::Unknown);
        assert_eq!(tag_style(&owned(&["release", "rc"])), TagStyle::Unknown);
        assert_eq!(tag_style(&[]), TagStyle::Unknown);
    }

    /// SATISFIES dependencies:the-channel-follows-the-source-evidence
    #[test]
    fn the_channels_follow_the_evidence() {
        let mut rust = sample(Some("rust"));
        assert_eq!(channels(&rust), [Channel::Crates]);
        rust.dist_github = true;
        rust.flake_package = true;
        assert_eq!(
            channels(&rust),
            [Channel::Crates, Channel::Flake, Channel::GithubRelease]
        );
        rust.host = Some("codeberg.org".into());
        assert_eq!(
            channels(&rust),
            [Channel::Crates, Channel::Flake],
            "release archives are a GitHub fact"
        );
        assert_eq!(channels(&sample(Some("python"))), [Channel::Pypi]);
        assert_eq!(channels(&sample(Some("node"))), [Channel::Npm]);
        let mut bare = sample(None);
        bare.name = None;
        assert!(channels(&bare).is_empty());
    }
}
