//! `rk skill`: the agent skills at user scope.
//!
//! Install writes each skill's `SKILL.md` under `~/.claude/skills/`, which
//! Claude Code reads, and `~/.agents/skills/`, which other coding agents
//! read. Both operations preview by default and touch only the files the
//! payload names, so anything a user added alongside survives.

use std::fs;
use std::path::PathBuf;

use crate::cli::skill::{SkillAction, SkillArgs};
use crate::embedded;
use crate::error::RkError;

/// The user-scope roots skills install under.
const SCOPES: [&str; 2] = [".claude/skills", ".agents/skills"];

/// Dispatch the skill action.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown skill name,
/// [`RkError::Refused`] on a differing destination without `--force`, and
/// [`RkError::Io`] on filesystem failure.
pub fn run(args: &SkillArgs) -> Result<(), RkError> {
    match &args.action {
        SkillAction::List => {
            for (name, _) in skills() {
                println!("{name}");
            }
            Ok(())
        }
        SkillAction::Show { name } => skills().into_iter().find(|(n, _)| n == name).map_or_else(
            || {
                Err(RkError::NotFound {
                    kind: "skill",
                    name: name.clone(),
                })
            },
            |(_, contents)| {
                print!("{contents}");
                Ok(())
            },
        ),
        SkillAction::Install { apply, force } => install(*apply, *force),
        SkillAction::Uninstall { apply } => uninstall(*apply),
    }
}

/// The embedded skills as `(name, contents)`.
fn skills() -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = embedded::SKILLS
        .dirs()
        .filter_map(|dir| {
            let name = dir.path().to_string_lossy().into_owned();
            let file = dir.get_file(format!("{name}/SKILL.md"))?;
            Some((name, file.contents_utf8().unwrap_or_default()))
        })
        .collect();
    out.sort();
    out
}

/// The home directory, from the environment.
fn home() -> Result<PathBuf, RkError> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| RkError::Refused("neither HOME nor USERPROFILE is set".into()))
}

fn install(apply: bool, force: bool) -> Result<(), RkError> {
    let home = home()?;
    if !apply {
        println!("DRY RUN: rk skill install writes these files; re-run with --apply");
        for scope in SCOPES {
            for (name, _) in skills() {
                println!(
                    "{}",
                    home.join(scope).join(&name).join("SKILL.md").display()
                );
            }
        }
        return Ok(());
    }
    for scope in SCOPES {
        for (name, contents) in skills() {
            let dir = home.join(scope).join(&name);
            let dest = dir.join("SKILL.md");
            // Bytes, not strings: a destination that fails to read as UTF-8
            // still exists, and only a missing file may be written over
            // silently; any other read failure propagates.
            match fs::read(&dest) {
                Ok(found) if found == contents.as_bytes() => {
                    println!("unchanged {}", dest.display());
                    continue;
                }
                Ok(_) if !force => {
                    return Err(RkError::Refused(format!(
                        "{} differs from the payload; re-run with --force to overwrite",
                        dest.display()
                    )));
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            fs::create_dir_all(&dir)?;
            fs::write(&dest, contents)?;
            println!("wrote {}", dest.display());
        }
    }
    Ok(())
}

fn uninstall(apply: bool) -> Result<(), RkError> {
    let home = home()?;
    for scope in SCOPES {
        for (name, _) in skills() {
            let dir = home.join(scope).join(&name);
            let dest = dir.join("SKILL.md");
            if !dest.is_file() {
                continue;
            }
            if apply {
                fs::remove_file(&dest)?;
                // The directory goes only when the skill file was its last
                // entry, so anything a user added alongside survives.
                if fs::read_dir(&dir)?.next().is_none() {
                    fs::remove_dir(&dir)?;
                }
                println!("removed {}", dest.display());
            } else {
                println!("DRY RUN: would remove {}", dest.display());
            }
        }
    }
    Ok(())
}
