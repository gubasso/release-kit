//! `rk skill`: the reader of the agent skills, and the removal of the
//! copies an earlier release installed under the home.
//!
//! Every landing carries the skills, so `install` only names the landing.
//! `uninstall` resolves the home, the roots the chosen agent implies, and
//! the record that sits beside them, and renders what the installer
//! reports, in both the human and the machine form.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::cli::skill::{Agent, SkillAction, SkillArgs};
use crate::error::RkError;
use crate::output::Output;
use crate::skills;
use crate::skills::installer::{self, Action, Layout};
use crate::skills::record::RECORD_PATH;
use crate::skills::{AGENTS_ROOT, CLAUDE_ROOT, LEGACY_SHARED_ROOT, home};

/// The machine form of an install or uninstall report.
#[derive(Debug, Serialize)]
struct Report<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// `install` or `uninstall`.
    command: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// Everything the run did, or would do.
    actions: &'a [Action],
    /// What plausibly follows.
    next: &'a [String],
}

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
            let out = Output::human();
            for skill in skills::all()? {
                out.result_line(&skill.name);
            }
            Ok(())
        }
        SkillAction::Show { name, file } => show(name, file.as_deref()),
        SkillAction::Install { apply, json, .. } => retired_install(Output::new(*json), *apply),
        SkillAction::Uninstall { agent, apply, json } => {
            let layout = layout(*agent)?;
            let actions = installer::uninstall(&layout, *apply)?;
            render(Output::new(*json), "uninstall", *apply, &actions)
        }
    }
}

/// What `rk skill install` says now that every landing carries the skills.
const RETIRED_INSTALL: &str = "rk lands its skills into each project through `rk init` and `rk upgrade`. To remove the copies that an earlier release wrote under your home, run `rk skill uninstall --apply`.";

/// The retired install: the text and exit 0 for a preview, a refusal
/// under `--apply`, and nothing written either way.
///
/// SATISFIES distribution:a-skill-has-one-owner
fn retired_install(out: Output, apply: bool) -> Result<(), RkError> {
    if apply {
        return Err(RkError::Usage(RETIRED_INSTALL.to_owned()));
    }
    out.result_line(RETIRED_INSTALL);
    let next = vec!["rk skill uninstall --apply".to_owned()];
    out.next(&next);
    out.emit(&Report {
        schema: "rk.skill/1",
        command: "install",
        mode: "preview",
        actions: &[],
        next: &next,
    })
}

/// Render what the installer reported: the human lines by default, the
/// `rk.skill/1` object under `--json`.
fn render(
    out: Output,
    command: &'static str,
    apply: bool,
    actions: &[Action],
) -> Result<(), RkError> {
    if apply {
        for action in actions {
            out.result_line(applied_line(command, action));
        }
    } else {
        out.result_line(format!(
            "DRY RUN: rk skill {command} {} these files; re-run with --apply",
            if command == "install" {
                "writes"
            } else {
                "removes"
            }
        ));
        for action in actions {
            out.result_line(planned_line(action));
        }
    }
    let next = next_lines(command, apply);
    out.next(&next);
    out.emit(&Report {
        schema: "rk.skill/1",
        command,
        mode: if apply { "apply" } else { "preview" },
        actions,
        next: &next,
    })
}

/// The one human line for a planned action.
fn planned_line(action: &Action) -> String {
    match action {
        Action::Write { destination } | Action::Remove { destination } => destination.to_string(),
        Action::Sweep { destination } => {
            format!("sweep (no longer carried by this binary) {destination}")
        }
        Action::KeptEdited { destination } => format!("keep (edited by you) {destination}"),
        other => format!("{other:?}"),
    }
}

/// The one human line for a performed action, byte-identical to what the
/// installer printed before the boundary existed.
fn applied_line(command: &str, action: &Action) -> String {
    match action {
        Action::Write { destination } => format!("wrote {destination}"),
        Action::Unchanged { destination } => format!("unchanged {destination}"),
        Action::Sweep { destination } => format!("swept {destination}"),
        Action::SweepFailed { destination, error } => {
            format!("could not sweep {destination}; remove it by hand: {error}")
        }
        Action::Remove { destination } => format!("removed {destination}"),
        Action::KeptEdited { destination } => format!("kept (edited by you) {destination}"),
        Action::KeptDirectory { directory } => format!("kept (not empty) {directory}"),
        Action::RecordUnwritten { record } => {
            if command == "install" {
                format!(
                    "note: could not record the installed digests at {record}; a later install may ask for --force"
                )
            } else {
                format!(
                    "note: could not update the record at {record}; a later install may ask for --force"
                )
            }
        }
    }
}

/// What plausibly follows each outcome.
fn next_lines(command: &str, apply: bool) -> Vec<String> {
    match (command, apply) {
        (_, false) => vec![format!("rk skill {command} --apply")],
        (_, true) => vec![
            "rk init and rk upgrade land the skills into each project".to_owned(),
            "rk doctor reports any user-scope copy that is left".to_owned(),
        ],
    }
}

/// Print one skill's `SKILL.md`, byte-identical to the authored file, or
/// one file of its package, byte for byte.
///
/// SATISFIES distribution:a-skill-is-readable-before-a-landing
fn show(name: &str, file: Option<&str>) -> Result<(), RkError> {
    let out = Output::human();
    if let Some(path) = file {
        let package = skills::packages()?
            .into_iter()
            .find(|package| package.name == name)
            .ok_or_else(|| RkError::NotFound {
                kind: "skill",
                name: name.to_owned(),
            })?;
        let found = package
            .files
            .iter()
            .find(|candidate| candidate.path == path)
            .ok_or_else(|| RkError::NotFound {
                kind: "skill file",
                name: format!("{name}/{path}"),
            })?;
        out.result_bytes(found.bytes);
        return Ok(());
    }
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
                out.result_raw(skill.text);
                Ok(())
            },
        )
}

/// The roots a run touches, and the record that vouches for them.
fn layout(agent: Agent) -> Result<Layout, RkError> {
    let home = home()?;
    Ok(Layout {
        roots: roots(&home, agent),
        legacy_shared: matches!(agent, Agent::All).then(|| home.join(LEGACY_SHARED_ROOT)),
        record: home.join(RECORD_PATH),
    })
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

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::roots;
    use crate::cli::skill::Agent;
    use crate::skills::installer::Action;

    #[test]
    fn each_agent_selects_its_own_roots() {
        let home = Utf8Path::new("/home/<user>");
        assert_eq!(roots(home, Agent::Claude), ["/home/<user>/.claude/skills"]);
        assert_eq!(roots(home, Agent::Codex), ["/home/<user>/.agents/skills"]);
        assert_eq!(
            roots(home, Agent::All),
            ["/home/<user>/.claude/skills", "/home/<user>/.agents/skills"]
        );
    }

    /// The complete `rk.skill/1` report shape, held by snapshot.
    #[test]
    fn the_skill_report_schema_snapshot_holds() {
        let actions = vec![Action::Write {
            destination: "/home/<user>/.claude/skills/rk-setup/SKILL.md".into(),
        }];
        let next = vec!["rk skill list names the installed skills".to_owned()];
        let report = super::Report {
            schema: "rk.skill/1",
            command: "install",
            mode: "apply",
            actions: &actions,
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.skill/1","command":"install","mode":"apply","actions":[{"action":"write","destination":"/home/<user>/.claude/skills/rk-setup/SKILL.md"}],"next":["rk skill list names the installed skills"]}"#
        );
    }

    /// The `rk.skill/1` action shape, held by snapshot across every
    /// variant, so the whole tagged-union vocabulary is exact and a
    /// rename in any one arm fails here first.
    #[test]
    fn the_skill_action_schema_snapshot_holds() {
        let cases: Vec<(Action, &str)> = vec![
            (
                Action::Write {
                    destination: "/h/SKILL.md".into(),
                },
                r#"{"action":"write","destination":"/h/SKILL.md"}"#,
            ),
            (
                Action::Unchanged {
                    destination: "/h/SKILL.md".into(),
                },
                r#"{"action":"unchanged","destination":"/h/SKILL.md"}"#,
            ),
            (
                Action::Sweep {
                    destination: "/h/SKILL.md".into(),
                },
                r#"{"action":"sweep","destination":"/h/SKILL.md"}"#,
            ),
            (
                Action::SweepFailed {
                    destination: "/h/SKILL.md".into(),
                    error: "permission denied".into(),
                },
                r#"{"action":"sweep-failed","destination":"/h/SKILL.md","error":"permission denied"}"#,
            ),
            (
                Action::Remove {
                    destination: "/h/SKILL.md".into(),
                },
                r#"{"action":"remove","destination":"/h/SKILL.md"}"#,
            ),
            (
                Action::KeptEdited {
                    destination: "/h/SKILL.md".into(),
                },
                r#"{"action":"kept-edited","destination":"/h/SKILL.md"}"#,
            ),
            (
                Action::KeptDirectory {
                    directory: "/h/rk-setup".into(),
                },
                r#"{"action":"kept-directory","directory":"/h/rk-setup"}"#,
            ),
            (
                Action::RecordUnwritten {
                    record: "/h/skills.sha256".into(),
                },
                r#"{"action":"record-unwritten","record":"/h/skills.sha256"}"#,
            ),
        ];
        for (action, expected) in cases {
            assert_eq!(
                serde_json::to_string(&action).expect("an action serializes"),
                expected
            );
        }
    }
}
