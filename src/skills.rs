//! The agent skills: the embedded files, the user-scope record, and the installer.
//!
//! Skills land under the invoking user's home and never into a target
//! repository. An agent resolves a skill by name across scopes, so a second
//! copy under one name is a second entry offering the same skill, with no way
//! for the operator to tell which one runs. One installed binary already
//! serves every repository, and the skills routing into it belong at the same
//! scope.
//!
//! That places every destination outside the reach of `rk init`, which has a
//! target directory to compare against and a landing to refuse. Here there is
//! no target and no manifest, so [`record`] stands in for one: it answers the
//! single question the installer cannot otherwise answer — are these bytes
//! ones we wrote?

pub mod installer;
pub mod record;

use std::path::PathBuf;

use camino::Utf8PathBuf;

pub use crate::digest::Digest;
use crate::embedded;
use crate::error::RkError;

/// The root Claude Code reads, relative to the home directory.
pub const CLAUDE_ROOT: &str = ".claude/skills";

/// The root Codex, Gemini CLI, and Copilot read, relative to the home
/// directory.
pub const AGENTS_ROOT: &str = ".agents/skills";

/// The root an earlier release installed the shared gates to, relative to
/// the home directory.
///
/// No install writes here any more: each package carries its own copy of
/// the gates. An install and an uninstall still scan it, so the copies an
/// earlier release wrote are swept where the record vouches for them.
pub const LEGACY_SHARED_ROOT: &str = ".local/state/release-kit/skills/shared";

/// The directory inside a package that holds its reference files, the shared
/// gates' copies among them.
pub const REFERENCES_DIR: &str = "references";

/// The home directory, from the environment.
///
/// A home that is not UTF-8 refuses rather than proceeding: the record names
/// its destinations as text, so a path it cannot write down is a path it
/// cannot later vouch for.
///
/// # Errors
///
/// Returns [`RkError::Refused`] when no home variable is set, and when the
/// home directory is not UTF-8.
pub fn home() -> Result<Utf8PathBuf, RkError> {
    let raw = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| RkError::Refused("neither HOME nor USERPROFILE is set".into()))?;
    Utf8PathBuf::from_path_buf(PathBuf::from(raw)).map_err(|path| {
        RkError::Refused(format!(
            "the home directory is not UTF-8: {}",
            path.display()
        ))
    })
}

/// One embedded skill: its directory name and its `SKILL.md` text.
#[derive(Debug)]
pub struct Skill {
    /// The directory name, which is also the skill's `name` frontmatter.
    pub name: String,
    /// The authored `SKILL.md`, byte-identical to the file under `skills/`.
    pub text: &'static str,
}

/// Every embedded skill, sorted by name.
///
/// # Errors
///
/// Returns [`RkError::Other`] when a skill directory carries no readable
/// UTF-8 `SKILL.md`. That is a defect in the sources this binary was built
/// from, not something a caller can correct.
pub fn all() -> Result<Vec<Skill>, RkError> {
    let mut out = Vec::new();
    for dir in embedded::SKILLS.dirs() {
        let name = dir.path().to_string_lossy().into_owned();
        let text = dir
            .get_file(format!("{name}/SKILL.md"))
            .and_then(include_dir::File::contents_utf8)
            .ok_or_else(|| anyhow::anyhow!("embedded skill carries no UTF-8 SKILL.md: {name}"))?;
        out.push(Skill { name, text });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// One shared artifact: its file name, and its bytes.
#[derive(Debug)]
pub struct SharedArtifact {
    /// The file name under `skill-shared/`, which is also its name under a
    /// package's `references/`.
    pub path: String,
    /// The authored bytes, byte-identical to the file under `skill-shared/`.
    pub bytes: &'static [u8],
}

/// Every artifact the skills share, sorted by path.
///
/// Authored once under `skill-shared/` and copied into every package, so a
/// gate fix still lands in one file while each installed skill reads only
/// inside its own directory.
#[must_use]
pub fn shared() -> Vec<SharedArtifact> {
    embedded::walk(&embedded::SKILL_SHARED)
        .into_iter()
        .map(|(path, bytes)| SharedArtifact { path, bytes })
        .collect()
}

/// One file of an installed skill package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    /// The path relative to the skill directory, as it lands.
    pub path: String,
    /// The embedded bytes that belong there.
    pub bytes: &'static [u8],
    /// Whether the file is a copy of a shared artifact rather than the
    /// skill's own.
    pub shared: bool,
}

/// One installed skill: its name and every file its directory carries.
#[derive(Debug)]
pub struct Package {
    /// The skill's directory name.
    pub name: String,
    /// `SKILL.md` first, then the reference files, sorted by path.
    pub files: Vec<PackageFile>,
}

/// Why a skill's authored files do not compose into a package.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackageError {
    /// The skill directory carries no `SKILL.md`.
    #[error("{skill}: the skill carries no SKILL.md")]
    NoManual {
        /// The skill directory.
        skill: String,
    },
    /// A file sits where a package carries nothing: anything other than
    /// `SKILL.md` and Markdown files directly under `references/`.
    #[error("{skill}: {path} is neither SKILL.md nor a Markdown file directly under references/")]
    Unlisted {
        /// The skill directory.
        skill: String,
        /// The file, relative to the skill directory.
        path: String,
    },
    /// A skill's own reference file takes the name of a shared artifact, so
    /// two sources claim one destination.
    #[error("{skill}: {path} takes the name of a shared artifact under skill-shared/")]
    Collision {
        /// The skill directory.
        skill: String,
        /// The file, relative to the skill directory.
        path: String,
    },
}

/// Compose one package from a skill's own files and the shared artifacts.
///
/// `own` holds paths relative to the skill directory. The package carries
/// `SKILL.md`, each Markdown file directly under `references/`, and every
/// shared artifact copied under `references/`. Anything else, and any name
/// two sources claim, is an error: a package that silently kept one copy
/// would ship a gate that no longer matches its source.
///
/// # Errors
///
/// Returns the first [`PackageError`] the files carry.
pub fn compose(
    skill: &str,
    own: &[(String, &'static [u8])],
    shared: &[SharedArtifact],
) -> Result<Vec<PackageFile>, PackageError> {
    let mut manual = None;
    let mut references = Vec::new();
    for (path, bytes) in own {
        if path == "SKILL.md" {
            manual = Some(PackageFile {
                path: path.clone(),
                bytes,
                shared: false,
            });
            continue;
        }
        let listed = path
            .strip_prefix(&format!("{REFERENCES_DIR}/"))
            .is_some_and(|name| {
                !name.contains('/')
                    && std::path::Path::new(name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            });
        if !listed {
            return Err(PackageError::Unlisted {
                skill: skill.to_owned(),
                path: path.clone(),
            });
        }
        references.push(PackageFile {
            path: path.clone(),
            bytes,
            shared: false,
        });
    }
    let manual = manual.ok_or_else(|| PackageError::NoManual {
        skill: skill.to_owned(),
    })?;
    for artifact in shared {
        let path = format!("{REFERENCES_DIR}/{}", artifact.path);
        if references.iter().any(|file| file.path == path) {
            return Err(PackageError::Collision {
                skill: skill.to_owned(),
                path,
            });
        }
        references.push(PackageFile {
            path,
            bytes: artifact.bytes,
            shared: true,
        });
    }
    references.sort_by(|a, b| a.path.cmp(&b.path));
    let mut files = vec![manual];
    files.extend(references);
    Ok(files)
}

/// Every embedded skill as the package an install writes, sorted by name.
///
/// # Errors
///
/// Returns [`RkError::Other`] when a skill's files do not compose. That is a
/// defect in the sources this binary was built from, and a test holds every
/// embedded skill to composing.
pub fn packages() -> Result<Vec<Package>, RkError> {
    let shared = shared();
    let mut out = Vec::new();
    for skill in all()? {
        let prefix = format!("{}/", skill.name);
        let own: Vec<(String, &'static [u8])> = embedded::walk(&embedded::SKILLS)
            .into_iter()
            .filter_map(|(path, bytes)| {
                path.strip_prefix(&prefix)
                    .map(|relative| (relative.to_owned(), bytes))
            })
            .collect();
        let files =
            compose(&skill.name, &own, &shared).map_err(|error| anyhow::anyhow!("{error}"))?;
        out.push(Package {
            name: skill.name,
            files,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{PackageError, SharedArtifact, all, compose, packages, shared};

    #[test]
    fn the_binary_carries_the_shared_plan_gate() {
        let shared = shared();
        assert!(
            shared
                .iter()
                .any(|artifact| artifact.path == "plan-gate.md"),
            "the binary carries no shared plan gate"
        );
    }

    #[test]
    fn the_binary_carries_every_authored_skill() {
        let skills = all().expect("the embedded skills read");
        assert!(!skills.is_empty(), "the binary carries no skills");
        for skill in &skills {
            assert!(
                skill.text.contains(&format!("name: {}", skill.name)),
                "{}: the frontmatter name differs from the directory",
                skill.name
            );
        }
    }

    /// Every embedded skill composes, and each package carries every shared
    /// artifact under `references/`.
    #[test]
    fn every_skill_package_composes() {
        let packages = packages().expect("every embedded skill composes");
        for package in &packages {
            assert_eq!(package.files[0].path, "SKILL.md", "{}", package.name);
            for artifact in shared() {
                let path = format!("references/{}", artifact.path);
                assert!(
                    package.files.iter().any(|file| file.path == path
                        && file.shared
                        && file.bytes == artifact.bytes),
                    "{}: {path} is not carried",
                    package.name
                );
            }
        }
    }

    fn gate() -> Vec<SharedArtifact> {
        vec![SharedArtifact {
            path: "plan-gate.md".to_owned(),
            bytes: b"the shared gate\n",
        }]
    }

    #[test]
    fn a_name_both_sources_claim_fails_the_package() {
        let own = vec![
            ("SKILL.md".to_owned(), b"manual\n".as_slice()),
            ("references/plan-gate.md".to_owned(), b"mine\n".as_slice()),
        ];
        assert_eq!(
            compose("rk-demo", &own, &gate()),
            Err(PackageError::Collision {
                skill: "rk-demo".to_owned(),
                path: "references/plan-gate.md".to_owned(),
            })
        );
    }

    #[test]
    fn a_file_outside_the_package_shape_fails_the_package() {
        for path in ["scripts/check.sh", "references/deep/task.md", "notes.md"] {
            let own = vec![
                ("SKILL.md".to_owned(), b"manual\n".as_slice()),
                (path.to_owned(), b"x\n".as_slice()),
            ];
            assert_eq!(
                compose("rk-demo", &own, &gate()),
                Err(PackageError::Unlisted {
                    skill: "rk-demo".to_owned(),
                    path: path.to_owned(),
                }),
                "{path}"
            );
        }
    }

    #[test]
    fn a_package_carries_its_references_and_the_shared_copies() {
        let own = vec![
            ("SKILL.md".to_owned(), b"manual\n".as_slice()),
            ("references/task.md".to_owned(), b"task\n".as_slice()),
        ];
        let files = compose("rk-demo", &own, &gate()).expect("composes");
        let paths: Vec<(&str, bool)> = files
            .iter()
            .map(|file| (file.path.as_str(), file.shared))
            .collect();
        assert_eq!(
            paths,
            [
                ("SKILL.md", false),
                ("references/plan-gate.md", true),
                ("references/task.md", false),
            ]
        );
    }
}
