//! The git-cliff calls: the bumped version, the changelog, and the notes.
//!
//! The engine runs the git-cliff binary and links no library. It reads
//! the pin from the embedded `versions.toml` and refuses a binary at any
//! other version, so the changelog a release writes moves only with the
//! registry row. It never passes `--bump`: git-cliff proposes a number,
//! and the engine decides it.
//!
//! SATISFIES release-engine:git-cliff-runs-at-its-pin

use camino::Utf8Path;

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// Run git-cliff in `root` with `args`, its failure typed.
fn run(root: &Utf8Path, args: &[&str]) -> Result<String, RkError> {
    let mut command = std::process::Command::new(crate::probes::git_cliff_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let output = command
        .args(args)
        .current_dir(root.as_std_path())
        .output()
        .map_err(|source| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    format!("git-cliff did not run: {source}"),
                )
                .expected("git-cliff on PATH at the version versions.toml pins")
                .action("install git-cliff at its pin, or put it in the recorded development environment")
                .target_state("unchanged"),
            )
        })?;
    if !output.status.success() {
        return Err(RkError::subprocess(
            Diagnostic::new(
                Reason::SubprocessFailed,
                format!(
                    "git-cliff {} failed: {}",
                    args.join(" "),
                    super::last_line(&output.stderr)
                ),
            )
            .expected("git-cliff to read the landed cliff.toml and the history"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The version `versions.toml` pins.
#[must_use]
pub fn pin() -> Option<String> {
    crate::registry::version_of("git-cliff")
}

/// Refuse unless the git-cliff on hand runs at the pin.
///
/// # Errors
///
/// [`Reason::ToolOffPin`] where the binary reports another version, and
/// a refusal where it does not run.
pub fn require_pin(root: &Utf8Path) -> Result<String, RkError> {
    let pinned = pin().unwrap_or_default();
    let answer = run(root, &["--version"])?;
    let found = crate::probes::parse_cli_version(&answer);
    let wanted = crate::probes::parse_cli_version(&pinned);
    if found.is_none() || found != wanted {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::ToolOffPin,
                format!(
                    "git-cliff reports {:?}, and versions.toml pins {pinned}",
                    answer.trim()
                ),
            )
            .expected(format!("git-cliff {pinned}"))
            .action(format!(
                "install git-cliff {pinned}, or move the recorded development environment to it"
            ))
            .target_state("unchanged"),
        ));
    }
    Ok(pinned)
}

/// git-cliff's proposal for the next version, from the landed
/// `cliff.toml` and the history.
///
/// # Errors
///
/// git-cliff's failure.
pub fn bumped(root: &Utf8Path) -> Result<String, RkError> {
    let answer = run(root, &["--bumped-version"])?;
    Ok(answer
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_owned())
}

/// Rewrite `CHANGELOG.md` with `tag` as the newest section.
///
/// # Errors
///
/// git-cliff's failure.
pub fn changelog(root: &Utf8Path, tag: &str) -> Result<(), RkError> {
    run(root, &["--tag", tag, "-o", "CHANGELOG.md"]).map(drop)
}

/// Write the notes of the unreleased commits, as `tag`, into `out`. The
/// tag does not exist yet, so the notes name it.
///
/// # Errors
///
/// git-cliff's failure.
pub fn notes(root: &Utf8Path, tag: &str, out: &Utf8Path) -> Result<(), RkError> {
    run(root, &["--unreleased", "--tag", tag, "-o", out.as_str()]).map(drop)
}
