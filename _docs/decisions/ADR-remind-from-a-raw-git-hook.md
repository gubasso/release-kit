# Remind from a raw git hook

## Context and Problem Statement

A squash merge retires a branch on the forge, but the clone's own copy survives with its upstream marked gone, and no git event fires when the forge merges. The nearest local event is the pull that fetches the result, which is a merge, so `post-merge` fires with the gone marker freshly true. The reminder needs a home that reaches that event without violating what the landed hook block is allowed to carry.

## Considered Options

- `a raw .git/hooks/post-merge file, written by rk setup step branch-reminder` — chosen.
- `a hook in the landed pre-commit block` — rejected: `landing:a-landed-hook-serves-the-release-convention-alone` bars general hygiene from the block, and `post-merge` is not among the landed `default_install_hook_types`, so the splice could not even install its stage.
- `a forge-side automation` — rejected: nothing the forge runs can delete or report refs in an operator's clone.
- `a shell-prompt integration` — rejected: it puts repository scanning on every prompt in every directory, for an event that happens at pull time.

## Decision Outcome

Chosen option: `a raw .git/hooks/post-merge file, written by rk setup step branch-reminder` — `.git/hooks` is host-side state outside the worktree, so it was never landable payload, and writing where `git rev-parse --git-path hooks` points honors worktrees and `core.hooksPath`. The step refuses over a hook it did not write, per `maintenance:a-foreign-hook-is-never-clobbered`, and the hook only reminds. Enforced by `maintenance:the-reminder-never-blocks-a-pull`.

## Consequences

- Good: the reminder fires at the one moment the gone marker is fresh, costs nothing when the clone is clean, and survives however the merge happened on the forge.
- Bad: `.git/hooks` does not travel with clones, so every clone runs the step once; and `pre-commit install --hook-type post-merge` would move the file aside, which the step's drift observation surfaces on the next check.

## Status

Implemented — `rk setup step branch-reminder` writes it; `src/setup/branch_reminder.rs` holds the body.
