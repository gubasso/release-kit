//! The post-merge reminder hook `rk setup step branch-reminder` writes.
//!
//! No git event fires when the forge squash-merges and deletes a branch;
//! the nearest local event is the pull that fetches the result, which is a
//! merge, so `post-merge` fires with the `[gone]` marker freshly true.
//! The hook only reminds — the quiet prunes print nothing when the clone
//! is clean and never delete — and it never blocks a pull. Each call is
//! guarded by a capability probe on its own verb (`rk <verb> --help`),
//! not by `command -v rk`: the probe answers the question the hook
//! actually has — can this `rk` prune this resource? — so a missing
//! binary, one too old for the verb, and one that renamed it all fail
//! identically and print nothing, while the real invocations keep their
//! stderr so a genuine refusal still reaches the operator. The body is a
//! Rust const rather than a `setup/<forge>/` script: it belongs to no
//! forge, and the forge trees hold one script per forge step by the
//! parity rule.

use std::path::PathBuf;

use camino::Utf8Path;

/// The marker line a reminder hook carries; its absence makes a hook
/// foreign, and a foreign hook is never written over.
pub const MARKER: &str = "# release-kit branch reminder";

/// The whole hook, byte for byte.
///
/// No `set -eu` on purpose: the contract
/// is exit 0 always, and the `|| :` plus the final line hold it. The
/// probes are per verb, because during a transition a binary exists that
/// carries one prune verb and not the other; one probe for both would
/// silence the half that works or admit the half that does not.
pub const HOOK_BODY: &str = "#!/bin/sh
# release-kit branch reminder
# Installed by rk setup step branch-reminder; rerunning that step rewrites it.
# After a merge arrives, report the branches and worktrees the forge already
# merged and retired. Prints nothing when there are none, never blocks a pull.
if rk branches prune --help >/dev/null 2>&1; then
  rk branches prune --quiet || :
fi
if rk worktree prune --help >/dev/null 2>&1; then
  rk worktree prune --quiet || :
fi
exit 0
";

/// What the hook file holds today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookState {
    /// The file is this binary's body, executable.
    Installed,
    /// The marker is present but the body or the mode drifted.
    Drifted,
    /// A post-merge hook exists without the marker; it is someone else's.
    Foreign,
    /// No post-merge hook exists.
    Absent,
    /// The hooks directory or the file could not be read.
    Unreadable(String),
}

/// Where git will look for the post-merge hook: `rev-parse --git-path`
/// answers through gitfiles, linked worktrees, and `core.hooksPath`, and
/// a relative answer is relative to the target it ran in.
///
/// # Errors
///
/// The detail of a git that did not run or did not answer.
pub fn hook_path(target: &Utf8Path) -> Result<PathBuf, String> {
    let mut command = std::process::Command::new("git");
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let answered = command
        .arg("-C")
        .arg(target.as_std_path())
        .args(["rev-parse", "--git-path", "hooks"])
        .output()
        .map_err(|source| format!("git did not run: {source}"))?;
    if !answered.status.success() {
        return Err(format!("{target} is not a git repository"));
    }
    let hooks = String::from_utf8_lossy(&answered.stdout).trim().to_owned();
    if hooks.is_empty() {
        return Err("git named no hooks directory".to_owned());
    }
    let hooks = PathBuf::from(hooks);
    let hooks = if hooks.is_absolute() {
        hooks
    } else {
        target.as_std_path().join(hooks)
    };
    Ok(hooks.join("post-merge"))
}

/// Read the hook file and judge it against this binary's body.
#[must_use]
pub fn observe_hook(target: &Utf8Path) -> HookState {
    let path = match hook_path(target) {
        Ok(path) => path,
        Err(detail) => return HookState::Unreadable(detail),
    };
    // Judge the entry itself, not what it points at: a symlink - dangling
    // or not - is another manager's installation style, and a read
    // through it would misclassify the dangling case as absent and let
    // the atomic writer's rename replace the link.
    match std::fs::symlink_metadata(&path) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return HookState::Absent,
        Err(source) => return HookState::Unreadable(format!("{}: {source}", path.display())),
        Ok(meta) if !meta.is_file() => return HookState::Foreign,
        Ok(_) => {}
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) => return HookState::Unreadable(format!("{}: {source}", path.display())),
    };
    if !String::from_utf8_lossy(&bytes).contains(MARKER) {
        return HookState::Foreign;
    }
    let executable = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::metadata(&path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            true
        }
    };
    if bytes == HOOK_BODY.as_bytes() && executable {
        HookState::Installed
    } else {
        HookState::Drifted
    }
}

#[cfg(test)]
mod tests {
    /// The body opens with a shebang, carries the marker, probes each
    /// verb separately before its quiet prune, and ends by succeeding
    /// whatever happened above.
    #[test]
    fn the_hook_body_carries_the_marker_and_never_fails() {
        assert!(super::HOOK_BODY.starts_with("#!/bin/sh\n"));
        assert!(super::HOOK_BODY.contains(super::MARKER));
        for verb in ["branches", "worktree"] {
            assert!(
                super::HOOK_BODY
                    .contains(&format!("if rk {verb} prune --help >/dev/null 2>&1; then")),
                "the {verb} call is guarded by its own capability probe"
            );
            assert!(
                super::HOOK_BODY.contains(&format!("rk {verb} prune --quiet || :")),
                "the {verb} prune runs quiet and never fails the pull"
            );
        }
        assert!(
            !super::HOOK_BODY.contains("command -v"),
            "a presence check answers the wrong question; the probe is per verb"
        );
        assert!(super::HOOK_BODY.ends_with("exit 0\n"));
    }
}
