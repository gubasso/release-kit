//! What the target manages its development tools with.
//!
//! The managers and their detection live on the self-depend manager
//! axis, so `rk depend` and `rk self-depend` read one list through one
//! sweep. This observation adds what `rk depend` alone needs: the
//! technology, the `.envrc` load, and where the dependency's name already
//! appears. It judges nothing else.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use super::{Manager, canonical_dir};
use crate::error::RkError;
pub use crate::self_depend::manager::{MISE_FILES, ManagerFile, first_mention, manager_files};

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
        manager.default_file()
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

/// The technology, with `node` read from `package.json` after the
/// version files the bindings define.
#[must_use]
pub fn tech_or_node(dir: &Utf8Path) -> Option<&'static str> {
    crate::detect::tech_of(dir.as_std_path())
        .or_else(|| dir.join("package.json").is_file().then_some("node"))
}

#[cfg(test)]
mod tests {
    use super::{Manager, Target, tech_or_node};

    #[test]
    fn the_default_file_is_the_managers_own() {
        for manager in Manager::ALL {
            assert_eq!(Target::default_file(manager), manager.default_file());
        }
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
