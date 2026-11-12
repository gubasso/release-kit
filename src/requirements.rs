//! The tools `rk` itself runs, and the check every verb makes before it
//! acts.
//!
//! A tool here is a dependency of the `rk` application: the host supplies
//! it, and a verb that runs it checks it at the start of the run, before
//! its first read of a forge and its first write. A missing tool, or one
//! below the floor this binary calls, stops the verb with one error that
//! names every such tool. Which tools a verb checks comes from the verb,
//! its flags, and the target's record, never from which tools the host
//! happens to carry.
//!
//! A project's own hook manager is not here. `rk integrate` asks git to
//! run the clone's `pre-merge-commit` hook, and git runs whatever the
//! project installed.
//!
//! SATISFIES tool-requirements:a-verb-checks-its-tools-first

use std::process::Command;

use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// One tool `rk` runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// git, which every verb that reads or writes a repository runs.
    Git,
    /// The POSIX shell every setup step and recorded build command runs
    /// through.
    Sh,
    /// A forge CLI: gh or glab.
    ForgeCli(Forge),
    /// Nix, which `rk self-depend sync` updates and builds a devshell with.
    Nix,
    /// cargo, which `rk integrate` asks for a crate's package listing.
    Cargo,
    /// git-cliff, which the release engine computes a version and a
    /// changelog with.
    GitCliff,
    /// uv, which the python driver adapter runs.
    Uv,
}

/// git's floor: `git hook run`, which the integration gate runs, first
/// shipped in git 2.36.0.
pub const GIT_FLOOR: (u32, u32, u32) = (2, 36, 0);

impl Tool {
    /// The tool's binary name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Sh => "sh",
            Self::ForgeCli(forge) => forge.cli(),
            Self::Nix => "nix",
            Self::Cargo => "cargo",
            Self::GitCliff => "git-cliff",
            Self::Uv => "uv",
        }
    }

    /// The environment variable that substitutes the binary, where one
    /// exists.
    #[must_use]
    pub const fn override_var(self) -> Option<&'static str> {
        match self {
            Self::Git => Some("RK_GIT_BIN"),
            Self::Sh => Some("RK_SH_BIN"),
            Self::ForgeCli(forge) => Some(forge.cli_override()),
            Self::Nix => Some("RK_NIX_BIN"),
            Self::Cargo => None,
            Self::GitCliff => Some("RK_GIT_CLIFF_BIN"),
            Self::Uv => Some("RK_UV_BIN"),
        }
    }

    /// The program this process spawns for the tool: the override where
    /// it is set and not empty, else the binary name, which `PATH`
    /// resolves. Every spawn of the tool takes its program from here, so
    /// the check judges the program that runs.
    #[must_use]
    pub fn program(self) -> std::ffi::OsString {
        self.override_var()
            .and_then(std::env::var_os)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| self.name().into())
    }

    /// Whether rk runs the tool with the target as the working directory,
    /// so a relative override or a relative `PATH` entry resolves there.
    /// git and the forge CLI run from the caller's directory.
    #[must_use]
    pub const fn runs_in_target(self) -> bool {
        matches!(
            self,
            Self::Sh | Self::Nix | Self::Cargo | Self::GitCliff | Self::Uv
        )
    }

    /// The call that answers whether the tool runs, and prints its
    /// version where the tool has a floor.
    const fn presence_args(self) -> &'static [&'static str] {
        match self {
            Self::Sh => &["-c", ":"],
            _ => &["--version"],
        }
    }

    /// The lowest version this binary calls, where the tool has one.
    #[must_use]
    pub const fn floor(self) -> Option<(u32, u32, u32)> {
        match self {
            Self::Git => Some(GIT_FLOOR),
            Self::ForgeCli(forge) => Some(forge.cli_floor()),
            _ => None,
        }
    }

    /// What the operator does where the tool does not run.
    fn install(self) -> String {
        self.override_var().map_or_else(
            || format!("install {} on PATH", self.name()),
            |var| format!("install {} on PATH, or point {var} at it", self.name()),
        )
    }

    /// What the operator does where the tool runs below its floor.
    fn upgrade(self) -> String {
        match self {
            Self::ForgeCli(forge) => forge.cli_upgrade().to_owned(),
            _ => format!("upgrade {}", self.name()),
        }
    }
}

/// Why one tool cannot serve the verb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmet {
    /// The tool.
    pub tool: Tool,
    /// What is wrong with it, for the message.
    pub problem: String,
    /// What the operator does about it.
    pub action: String,
}

/// Check one tool: it runs, and it answers a version at or above its
/// floor where it has one. `None` where it serves.
#[must_use]
pub fn check(tool: Tool, target: Option<&std::path::Path>) -> Option<Unmet> {
    check_program(tool, &tool.program(), target)
}

/// [`check`] for one program a caller already resolved, such as the
/// absolute path of the forge CLI it is about to run, so the check judges
/// that exact file.
#[must_use]
pub fn check_program(
    tool: Tool,
    program: &std::ffi::OsStr,
    target: Option<&std::path::Path>,
) -> Option<Unmet> {
    judge(tool, program, target, None)
}

/// [`check`] under the search path a child receives, where rk hands the
/// child another `PATH` than its own, so the check searches as the spawn
/// searches.
#[must_use]
pub fn check_under_path(
    tool: Tool,
    target: Option<&std::path::Path>,
    path: &std::ffi::OsStr,
) -> Option<Unmet> {
    judge(tool, &tool.program(), target, Some(path))
}

fn judge(
    tool: Tool,
    program: &std::ffi::OsStr,
    target: Option<&std::path::Path>,
    path: Option<&std::ffi::OsStr>,
) -> Option<Unmet> {
    let name = tool.name();
    let mut command = Command::new(program);
    if let Some(dir) = target.filter(|_| tool.runs_in_target()) {
        command.current_dir(dir);
    }
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let answer = command
        .args(tool.presence_args())
        .stdin(std::process::Stdio::null())
        .output();
    let output = match answer {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return Some(Unmet {
                tool,
                problem: format!(
                    "{name} does not answer {}: {}",
                    tool.presence_args().join(" "),
                    crate::maintenance::last_line(&output.stderr)
                ),
                action: tool.install(),
            });
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            let problem = match tool.override_var().and_then(|var| {
                std::env::var(var)
                    .ok()
                    .filter(|value| !value.is_empty())
                    .map(|value| (var, value))
            }) {
                Some((var, value)) => {
                    format!("{name} is missing: {var} names {value}, which does not exist")
                }
                None => format!("{name} is missing: it is not on PATH"),
            };
            return Some(Unmet {
                tool,
                problem,
                action: tool.install(),
            });
        }
        Err(source) => {
            return Some(Unmet {
                tool,
                problem: format!("{name} does not start: {source}"),
                action: tool.install(),
            });
        }
    };
    let floor = tool.floor()?;
    let show = |(major, minor, patch): (u32, u32, u32)| format!("{major}.{minor}.{patch}");
    match crate::probes::parse_cli_version(&String::from_utf8_lossy(&output.stdout)) {
        Some(found) if found >= floor => None,
        Some(found) => Some(Unmet {
            tool,
            problem: format!(
                "{name} {} is below the {} rk calls",
                show(found),
                show(floor)
            ),
            action: tool.upgrade(),
        }),
        None => Some(Unmet {
            tool,
            problem: format!("{name} answers no version to hold against {}", show(floor)),
            action: tool.upgrade(),
        }),
    }
}

/// Every tool of `tools` that cannot serve, each once, judged where rk
/// runs it: a tool that runs in the target is checked from `target`.
#[must_use]
pub fn unmet(tools: &[Tool], target: Option<&std::path::Path>) -> Vec<Unmet> {
    let mut seen: Vec<Tool> = Vec::new();
    tools
        .iter()
        .copied()
        .filter(|tool| {
            let fresh = !seen.contains(tool);
            seen.push(*tool);
            fresh
        })
        .filter_map(|tool| check(tool, target))
        .collect()
}

/// The refusal that names every tool of `unmet`, for `verb`, which runs
/// `tools`.
#[must_use]
pub fn refusal(verb: &str, tools: &[Tool], unmet: &[Unmet]) -> RkError {
    let problems: Vec<&str> = unmet.iter().map(|unmet| unmet.problem.as_str()).collect();
    let actions: Vec<&str> = unmet.iter().map(|unmet| unmet.action.as_str()).collect();
    let mut names: Vec<&str> = Vec::new();
    for tool in tools {
        if !names.contains(&tool.name()) {
            names.push(tool.name());
        }
    }
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!(
                "{verb} runs tools this host cannot serve: {}",
                problems.join("; ")
            ),
        )
        .expected(format!(
            "every tool {verb} runs on PATH, at or above its floor: {}",
            names.join(", ")
        ))
        .action(actions.join("; "))
        .target_state("unchanged"),
    )
}

/// Check every tool a verb runs, before it acts.
///
/// One error names each tool that cannot serve the verb. A tool that rk
/// runs in the target is checked from `target`, where a relative override
/// or `PATH` entry resolves the way the spawn resolves it.
///
/// # Errors
///
/// [`Reason::PrerequisiteUnmet`] naming every missing or too-old tool,
/// with the target unchanged.
pub fn require_in(
    verb: &str,
    tools: &[Tool],
    target: Option<&std::path::Path>,
) -> Result<(), RkError> {
    let found = unmet(tools, target);
    if found.is_empty() {
        return Ok(());
    }
    Err(refusal(verb, tools, &found))
}

/// [`require_in`] for tools that all run from the caller's directory.
///
/// # Errors
///
/// As [`require_in`].
pub fn require(verb: &str, tools: &[Tool]) -> Result<(), RkError> {
    require_in(verb, tools, None)
}

#[cfg(test)]
mod tests {
    use super::{GIT_FLOOR, Tool};
    use crate::detect::Forge;

    /// The floors this binary calls: git for `git hook run`, gh for
    /// `gh auth status --active`.
    #[test]
    fn the_floors_are_the_releases_the_calls_need() {
        assert_eq!(GIT_FLOOR, (2, 36, 0));
        assert_eq!(Tool::Git.floor(), Some((2, 36, 0)));
        assert_eq!(Tool::ForgeCli(Forge::Github).floor(), Some((2, 57, 0)));
        assert_eq!(Tool::Nix.floor(), None);
    }

    /// Each tool names the override the rest of the binary honors.
    #[test]
    fn each_tool_names_its_override() {
        assert_eq!(Tool::Git.override_var(), Some("RK_GIT_BIN"));
        assert_eq!(
            Tool::ForgeCli(Forge::Gitlab).override_var(),
            Some("RK_GLAB_BIN")
        );
        assert_eq!(Tool::Cargo.override_var(), None);
    }
}
