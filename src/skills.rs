//! The agent skills: the payload, the user-scope record, and the installer.
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

pub use crate::digest::Digest;
use crate::embedded;
use crate::error::RkError;

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
/// UTF-8 `SKILL.md`. That is a defect in the payload this binary was built
/// from, not something a caller can correct.
pub fn all() -> Result<Vec<Skill>, RkError> {
    let mut out = Vec::new();
    for dir in embedded::SKILLS.dirs() {
        let name = dir.path().to_string_lossy().into_owned();
        let text = dir
            .get_file(format!("{name}/SKILL.md"))
            .and_then(include_dir::File::contents_utf8)
            .ok_or_else(|| anyhow::anyhow!("payload skill carries no UTF-8 SKILL.md: {name}"))?;
        out.push(Skill { name, text });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// One shared artifact: its path under the shared root, and its bytes.
#[derive(Debug)]
pub struct SharedArtifact {
    /// The path relative to the shared root, as it lands.
    pub path: String,
    /// The authored bytes, byte-identical to the file under `skill-shared/`.
    pub bytes: &'static [u8],
}

/// Every artifact the skills share, sorted by path.
///
/// These land once, outside the agent skill roots, because every skill names
/// the same absolute path for them. A copy per skill would be one file to
/// correct per agent root per skill; one copy is one.
#[must_use]
pub fn shared() -> Vec<SharedArtifact> {
    embedded::walk(&embedded::SKILL_SHARED)
        .into_iter()
        .map(|(path, bytes)| SharedArtifact { path, bytes })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{all, shared};

    #[test]
    fn the_payload_carries_the_shared_plan_gate() {
        let shared = shared();
        assert!(
            shared
                .iter()
                .any(|artifact| artifact.path == "plan-gate.md"),
            "the payload carries no shared plan gate"
        );
    }

    #[test]
    fn the_payload_carries_every_authored_skill() {
        let skills = all().expect("the embedded skills read");
        assert!(!skills.is_empty(), "the payload carries no skills");
        for skill in &skills {
            assert!(
                skill.text.contains(&format!("name: {}", skill.name)),
                "{}: the frontmatter name differs from the directory",
                skill.name
            );
        }
    }
}
