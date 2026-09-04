---
name: rk-migrate
description: Drives a repository from its current state to the current release-kit convention, mixing steps it runs itself with steps it gates for the operator. Use when asked to migrate a project onto release-kit, adopt the convention in an existing repository, upgrade a landed payload, move a gated two-branch release flow onto the trunk convention, or reconcile a drifted setup. Triggers include migrate, adopt release-kit, rk upgrade, trunk migration, and setup drift.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Forge mutations need the forge CLI authenticated with administration rights; the operator supplies credentials and registry actions.
---

# rk-migrate

Take a repository from wherever it is to the current release-kit convention. The work is a loop, not a script: observe, act on the smallest gap, re-observe, continue — and everything the operator must do by hand is gated, stated as an exact command, and verified after they run it.

## Before acting

Read two shared files before the first action of a task, in this order, and hold both for the whole task.

1. `~/.local/state/release-kit/skills/shared/pre-flight-gate.md` — run it whatever the request carries. It checks this host with `rk doctor` and stops the task on what no plan can work around. No flag skips it.
2. `~/.local/state/release-kit/skills/shared/plan-gate.md` — it binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The two gates are why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, the pre-flight says whether this host can run it at all, and the plan gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the plan gate's approval turn only. Still run the pre-flight, still state the ordered plan before acting, and still validate it as phase 2 directs.

## The loop

1. Observe: `rk doctor` for the host, `rk status --target .` for the landed payload (`--check` to judge), `rk setup check --target .` for the forge shape, `rk versions --check` for the pins.
2. Classify every finding as one of: run it now, gate it for the operator, or already satisfied.
3. Act on the automated findings, one at a time, smallest first.
4. Re-run the observation that found the gap. A check that cannot be read is not a pass, and a failed check is never continued past — it is the next gap.
5. Stop when `rk status --check`, `rk setup check`, and `rk versions --check` are green; the first release through `rk method operate` is the migration's proof.

## What runs unattended

| Finding                                                     | Act                                                                                                                                                                                                                                          |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A target with no landing record                             | Preview first: `rk adopt --target . --scopes <list> --workflow <mode> --style <style>` lists what differs from the selected candidate; align, then `--apply` (below)                                                                         |
| A landed payload older than the binary                      | `rk upgrade --target . --apply`                                                                                                                                                                                                              |
| A record without the scopes parameter                       | `rk upgrade --target . --scopes <list> --apply`, the list confirmed with the operator first                                                                                                                                                  |
| A record without the style parameter                        | `rk upgrade --target . --style <style> --apply`, the style asked of the operator first (below)                                                                                                                                               |
| A forge that forbids a request merging itself               | `rk setup step auto-merge --apply`                                                                                                                                                                                                           |
| A missing hook block in `.pre-commit-config.yaml`           | The upgrade lands it; reconcile the config first, as below                                                                                                                                                                                   |
| A missing or drifted forge protection                       | `rk setup step <name> --apply`                                                                                                                                                                                                               |
| A squash title source that is not the request's title       | `rk setup step protect-trunk --apply` re-asserts it                                                                                                                                                                                          |
| A squash message source that is not the request's body      | `rk setup step protect-trunk --apply` re-asserts it                                                                                                                                                                                          |
| A hook block lacking the `rk-message` content guard         | `rk upgrade --target . --apply` re-renders the block; reconcile a hand-edited block first                                                                                                                                                    |
| An unfilled `TODO(release-kit)` sentinel                    | Resolve it in the seeded file, from the project                                                                                                                                                                                              |
| Stale installed skills                                      | `rk skill install --apply`                                                                                                                                                                                                                   |
| A flake with no release-kit input                           | `rk devshell add --target .` prints the fragments; apply them in the listed order, then `rk devshell sync --caller operator --apply`                                                                                                         |
| A predecessor bump mechanism, named by `rk devshell status` | `rk devshell clean --target . --apply`, then the `manual` entries by hand; the migration is not done while `leftovers` is non-empty, because a native mechanism wired over an unremoved predecessor is a failed migration, not a partial one |

Mechanical file edits can go to a subagent; every forge mutation stays in this loop, where its observation lives.

Before an upgrade lands the hook block into an existing `.pre-commit-config.yaml`, reconcile it as `rk guide setup` step 4 directs, and gate the choice between an existing hook and the landed one for the operator rather than stacking a second hook on one job.

An adoption is a verification pass against one rendered candidate, and `--workflow` selects which candidate — `branches` by default, the compatibility-safe reading of a pre-record target; it never blesses the disk. Run the preview first: it lists every destination that differs from the selected candidate, and the pre-adoption alignment is to bring the two marked blocks to the candidate's bytes — `rk payload` and `rk snippet` print them — then re-run and apply. A refusal naming the blocks is that alignment still owed, not an error to force past.

The skill reads the recorded mode and style from `rk status` and routes by both. A mode or style change is a named migration, never a side effect of a code-change request: `rk upgrade --workflow <mode>` or `rk upgrade --style <style>` previewed, applied on the operator's approval, committed through a pull request — then the transition for branches open across the change, each step stated and gated: main checkout to `master` and pulled, each open bare branch adopted with `rk worktree add <branch> --apply`. Under worktree mode, an off-path worktree or a bare-worked branch is a named step the same way — `rk worktree add` seats a bare branch, and `git worktree move`, named by the add refusal, brings an off-path seat home. The skill may also state the container layout from the worktree chapter as an option and render the promotion commands from the worktree runbook, and runs none of them: moving directories on the operator's disk is never a code change.

## What waits for the operator

Gate each of these: print the exact command, say what it changes and why, wait, then re-observe before continuing.

- `rk setup step single-trunk --apply` — destructive; it deletes retired long-lived branches, and its ancestry guard refusing is a stop, not an obstacle.
- `install-bot` and `bot-secrets` — the bot identity and its credentials; `rk forge <name>` carries the walkthrough.
- Registry actions: the first hand publish, registering the trusted publisher, turning on enforcement. `rk guide setup` names each with its reason.
- Removing an existing protection from a live branch, on any forge.
- The release style, on a record that predates it. Ask it with `AskUserQuestion` the way rk-setup's step 6 states, because arming an existing project's release request changes what a green trunk does; then `rk upgrade --style <style>`, previewed, and the visible diff is the arming line in the release workflow.
- The development environment, where the project obtains `rk` by a host install or a hand-rolled bump. Ask it with `AskUserQuestion` the way rk-setup's step 6 states, with the replacement as the default: the devshell pin supersedes what is there, `rk devshell clean` removes what it can judge, and `rk guide setup` carries the procedure in its order.

## From the retired two-branch flow

A repository still running an integration branch plus a gated release branch migrates in this order, because each protection refuses the step after it:

1. Land the current payload's files on the old default branch; prove them with the project's own checks.
2. Gate: remove the old release-branch protection — its pull-request-only rule refuses the fast-forward that comes next.
3. Fast-forward the trunk to the integrated tip; CI proves the landing there, not on the old branch.
4. `rk setup step default-branch --apply` makes the trunk the default.
5. `rk setup step protect-trunk --apply --required-check <name>`, then `protect-tags`; `protect-release-lines` only where older lines exist.
6. Gate: remove the old integration branch's protection, then the one-way door — `rk setup step single-trunk --apply`.
7. `rk setup check --target .` and `rk status --check --target .` green end the migration.

## Defaults

- Never author a tag, and never hand-edit a generated artifact workflow.
- Prefer an rk verb over a raw forge call; where no verb covers the gap, use the forge CLI and say that the loop is outside the convention there.
- Report every gated step's outcome from observation, never from the operator's word alone.
- Leave the repository releasable at every stop: a migration interrupted between steps must break no existing flow.
