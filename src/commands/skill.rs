//! `rk skill`: the agent skills at user scope.
//!
//! This handler resolves three things — the home directory, the roots the
//! chosen agent implies, and the record that sits beside them — and prints
//! what the installer reports. Install and uninstall semantics live in
//! `skills::installer`; nothing here decides them.

use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};

use crate::cli::skill::{Agent, Scope, SkillAction, SkillArgs};
use crate::error::RkError;
use crate::skills;
use crate::skills::installer;
use crate::skills::record::RECORD_PATH;

/// The root Claude Code reads, relative to the home directory.
const CLAUDE_ROOT: &str = ".claude/skills";

/// The root Codex, Gemini CLI, and Copilot read, relative to the home
/// directory.
const AGENTS_ROOT: &str = ".agents/skills";

/// Dispatch the skill action.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown skill name,
/// [`RkError::Refused`] when the home is unusable or a destination cannot be
/// touched, and [`RkError::Io`] on filesystem failure.
pub fn run(args: &SkillArgs) -> Result<(), RkError> {
    match &args.action {
        SkillAction::List => {
            for skill in skills::all()? {
                println!("{}", skill.name);
            }
            Ok(())
        }
        SkillAction::Show { name } => show(name),
        SkillAction::Install {
            agent,
            scope,
            apply,
            force,
        } => {
            let (roots, record) = destinations(*agent, *scope)?;
            report(installer::install(&roots, &record, *apply, *force)?);
            Ok(())
        }
        SkillAction::Uninstall {
            agent,
            scope,
            apply,
        } => {
            let (roots, record) = destinations(*agent, *scope)?;
            report(installer::uninstall(&roots, &record, *apply)?);
            Ok(())
        }
    }
}

/// Print one skill's `SKILL.md`, byte-identical to the authored file.
fn show(name: &str) -> Result<(), RkError> {
    skills::all()?
        .into_iter()
        .find(|skill| skill.name == name)
        .map_or_else(
            || {
                Err(RkError::NotFound {
                    kind: "skill",
                    name: name.to_owned(),
                })
            },
            |skill| {
                print!("{}", skill.text);
                Ok(())
            },
        )
}

/// The roots a run touches, and the record that vouches for them.
fn destinations(agent: Agent, scope: Scope) -> Result<(Vec<Utf8PathBuf>, Utf8PathBuf), RkError> {
    let Scope::User = scope;
    let home = home()?;
    Ok((roots(&home, agent), home.join(RECORD_PATH)))
}

/// The skill roots under `home`, in the order an apply writes them.
fn roots(home: &Utf8Path, agent: Agent) -> Vec<Utf8PathBuf> {
    let mut roots = Vec::new();
    if matches!(agent, Agent::Claude | Agent::All) {
        roots.push(home.join(CLAUDE_ROOT));
    }
    if matches!(agent, Agent::Codex | Agent::All) {
        roots.push(home.join(AGENTS_ROOT));
    }
    roots
}

/// The home directory, from the environment.
///
/// A home that is not UTF-8 refuses rather than proceeding: the record names
/// its destinations as text, so a path it cannot write down is a path it
/// cannot later vouch for.
fn home() -> Result<Utf8PathBuf, RkError> {
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

/// Print what the installer reported, one line each.
fn report(lines: Vec<String>) {
    for line in lines {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::roots;
    use crate::cli::skill::Agent;

    #[test]
    fn each_agent_selects_its_own_roots() {
        let home = Utf8Path::new("/home/u");
        assert_eq!(roots(home, Agent::Claude), ["/home/u/.claude/skills"]);
        assert_eq!(roots(home, Agent::Codex), ["/home/u/.agents/skills"]);
        assert_eq!(
            roots(home, Agent::All),
            ["/home/u/.claude/skills", "/home/u/.agents/skills"]
        );
    }
}
