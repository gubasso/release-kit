//! What the target manages its development tools with.
//!
//! Four managers are recognized by their files: a flake, a mise
//! configuration in the precedence mise documents, asdf's
//! `.tool-versions`, and `devbox.json`. The observation reads the files
//! and reports where the dependency's name already appears; it judges
//! nothing else.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use super::{Manager, canonical_dir};
use crate::error::RkError;

/// The mise configuration paths, in mise's precedence order, the
/// highest first: a file, or a `conf.d` directory whose `*.toml` entries
/// all load. A local override file is left out: it is not committed.
pub const MISE_FILES: [&str; 9] = [
    "mise.toml",
    ".mise.toml",
    "mise/config.toml",
    "mise/conf.d",
    ".mise/config.toml",
    ".mise/conf.d",
    ".config/mise.toml",
    ".config/mise/config.toml",
    ".config/mise/conf.d",
];

/// One manager and the file that declares it.
#[derive(Debug, Clone, Serialize)]
pub struct ManagerFile {
    /// The manager.
    pub manager: Manager,
    /// The file, relative to the target.
    pub file: String,
    /// Its text.
    #[serde(skip)]
    pub text: String,
}

/// Where a manager file already names the dependency.
#[derive(Debug, Clone, Serialize)]
pub struct Already {
    /// The manager.
    pub manager: Manager,
    /// The file, relative to the target.
    pub file: String,
    /// The first line naming it, 1-based.
    pub line: usize,
}

/// Everything the offline pass reads from the target.
#[derive(Debug, Clone)]
pub struct Target {
    /// The target, canonical.
    pub path: Utf8PathBuf,
    /// `rust`, `python`, `node`, or `bash`, where a manifest says.
    pub tech: Option<&'static str>,
    /// The managers present, in the closed order.
    pub managers: Vec<ManagerFile>,
    /// Whether `.envrc` carries `use flake`.
    pub envrc_use_flake: bool,
    /// Where the dependency is already named.
    pub already: Vec<Already>,
}

impl Target {
    /// The file a manager declares itself in, where present.
    #[must_use]
    pub fn file_of(&self, manager: Manager) -> Option<&ManagerFile> {
        self.managers.iter().find(|m| m.manager == manager)
    }

    /// The default file a manager is seeded into when absent.
    #[must_use]
    pub const fn default_file(manager: Manager) -> &'static str {
        match manager {
            Manager::Flake => "flake.nix",
            Manager::Mise => "mise.toml",
            Manager::Asdf => ".tool-versions",
            Manager::Devbox => "devbox.json",
        }
    }
}

/// Read the target, offline.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a path that is not a directory and
/// [`RkError::Io`] where a present file does not read.
pub fn observe(path: &Utf8Path, dep_name: Option<&str>) -> Result<Target, RkError> {
    let path = canonical_dir(path, "target")?;
    let managers = manager_files(&path, dep_name)?;
    let envrc_use_flake = match std::fs::read_to_string(path.join(".envrc")) {
        Ok(text) => text.lines().any(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("use") && words.next() == Some("flake")
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(RkError::Io(e)),
    };
    let already = dep_name
        .map(|name| {
            managers
                .iter()
                .filter_map(|m| {
                    first_mention(&m.text, name).map(|line| Already {
                        manager: m.manager,
                        file: m.file.clone(),
                        line,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Target {
        tech: tech_or_node(&path),
        path,
        managers,
        envrc_use_flake,
        already,
    })
}

/// The manager files present, one per manager, in the closed order.
///
/// For mise, the first path in precedence wins, and inside a `conf.d`
/// directory the file that already names the dependency wins over the
/// first in name order.
///
/// # Errors
///
/// Returns [`RkError::Io`] where a present file does not read.
pub fn manager_files(dir: &Utf8Path, dep_name: Option<&str>) -> Result<Vec<ManagerFile>, RkError> {
    let mut out = Vec::new();
    for manager in Manager::ALL {
        let candidates: &[&str] = match manager {
            Manager::Flake => &["flake.nix"],
            Manager::Mise => &MISE_FILES,
            Manager::Asdf => &[".tool-versions"],
            Manager::Devbox => &["devbox.json"],
        };
        for candidate in candidates {
            let Some(file) = first_config(dir, candidate, dep_name)? else {
                continue;
            };
            out.push(ManagerFile {
                manager,
                text: std::fs::read_to_string(dir.join(&file))?,
                file,
            });
            break;
        }
    }
    Ok(out)
}

/// The configuration file a candidate path resolves to: the file
/// itself, or, under a `conf.d` directory, the first `*.toml` in name
/// order that already names the dependency, else the first in name order.
fn first_config(
    dir: &Utf8Path,
    candidate: &str,
    dep_name: Option<&str>,
) -> Result<Option<String>, RkError> {
    let path = dir.join(candidate);
    if path.is_file() {
        return Ok(Some(candidate.to_owned()));
    }
    if !candidate.ends_with("conf.d") || !path.is_dir() {
        return Ok(None);
    }
    let mut names: Vec<String> = std::fs::read_dir(&path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "toml"))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    if let Some(name) = dep_name {
        for file in &names {
            if first_mention(&std::fs::read_to_string(path.join(file))?, name).is_some() {
                return Ok(Some(format!("{candidate}/{file}")));
            }
        }
    }
    Ok(names.first().map(|name| format!("{candidate}/{name}")))
}

/// The first line naming `name` as a word, 1-based.
#[must_use]
pub fn first_mention(text: &str, name: &str) -> Option<usize> {
    let boundary = |c: Option<char>| {
        c.is_none_or(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
    };
    text.lines()
        .position(|line| {
            line.match_indices(name).any(|(index, _)| {
                boundary(line[..index].chars().next_back())
                    && boundary(line[index + name.len()..].chars().next())
            })
        })
        .map(|index| index + 1)
}

/// The technology, with `node` read from `package.json` after the
/// version files the bindings define.
#[must_use]
pub fn tech_or_node(dir: &Utf8Path) -> Option<&'static str> {
    crate::detect::tech_of(dir.as_std_path())
        .or_else(|| dir.join("package.json").is_file().then_some("node"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{MISE_FILES, Manager, first_mention, manager_files, tech_or_node};

    #[test]
    fn the_first_mise_file_in_precedence_wins() {
        let dir = tempfile::tempdir().expect("a scratch dir");
        let root = camino::Utf8Path::from_path(dir.path()).expect("utf-8");
        std::fs::create_dir_all(root.join(".config/mise")).expect("mkdir");
        std::fs::write(root.join(".config/mise/config.toml"), "[tools]\n").expect("writes");
        std::fs::write(root.join(".mise.toml"), "[tools]\nnode = '24'\n").expect("writes");
        std::fs::write(root.join("devbox.json"), "{}\n").expect("writes");
        let files = manager_files(root, None).expect("reads");
        let names: Vec<(Manager, &str)> =
            files.iter().map(|m| (m.manager, m.file.as_str())).collect();
        assert_eq!(
            names,
            [
                (Manager::Mise, ".mise.toml"),
                (Manager::Devbox, "devbox.json")
            ]
        );
        assert_eq!(MISE_FILES[0], "mise.toml");
        std::fs::remove_file(root.join(".mise.toml")).expect("removes");
        std::fs::remove_file(root.join(".config/mise/config.toml")).expect("removes");
        std::fs::create_dir_all(root.join(".mise/conf.d")).expect("mkdir");
        std::fs::write(root.join(".mise/conf.d/tools.toml"), "[tools]\n").expect("writes");
        std::fs::write(root.join(".mise/conf.d/env.toml"), "[env]\n").expect("writes");
        std::fs::write(root.join(".mise/conf.d/README"), "").expect("writes");
        let files = manager_files(root, None).expect("reads");
        assert_eq!(
            files[0].file, ".mise/conf.d/env.toml",
            "a conf.d directory is mise ownership, its first toml in name order"
        );
        std::fs::write(
            root.join(".mise/conf.d/tools.toml"),
            "[tools]\n\"cargo:sample-tool\" = \"1.0.0\"\n",
        )
        .expect("writes");
        let files = manager_files(root, Some("sample-tool")).expect("reads");
        assert_eq!(
            files[0].file, ".mise/conf.d/tools.toml",
            "the file that already names the dependency is the destination"
        );
    }

    #[test]
    fn a_mention_is_found_by_line() {
        let text = "[tools]\nnode = '24'\n\"cargo:sample-tool\" = \"1.4.0\"\n";
        assert_eq!(first_mention(text, "sample-tool"), Some(3));
        assert_eq!(
            first_mention(text, "sample"),
            None,
            "a prefix is not a name"
        );
        assert_eq!(first_mention("", "sample-tool"), None);
    }

    #[test]
    fn node_is_read_from_package_json_after_the_version_files() {
        let dir = tempfile::tempdir().expect("a scratch dir");
        let root = camino::Utf8Path::from_path(dir.path()).expect("utf-8");
        assert_eq!(tech_or_node(root), None);
        std::fs::write(root.join("package.json"), "{}").expect("writes");
        assert_eq!(tech_or_node(root), Some("node"));
        std::fs::write(root.join("Cargo.toml"), "").expect("writes");
        assert_eq!(tech_or_node(root), Some("rust"));
    }
}
