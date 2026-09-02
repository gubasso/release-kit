---
name: rk-migrate
description: Drives a repository from its current state to the current release-kit convention, mixing steps it runs itself with steps it gates for the operator. Use when asked to migrate a project onto release-kit, adopt the convention in an existing repository, upgrade a landed payload, move a gated two-branch release flow onto the trunk convention, or reconcile a drifted setup. Triggers include migrate, adopt release-kit, rk upgrade, trunk migration, and setup drift.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Forge mutations need the forge CLI authenticated with administration rights; the operator supplies credentials and registry actions.
---

# rk-migrate

Take a repository from wherever it is to the current release-kit convention. The work is a loop, not a script: observe, act on the smallest gap, re-observe, continue — and everything the operator must do by hand is gated, stated as an exact command, and verified after they run it.

## Before acting

Read `~/.local/state/release-kit/skills/shared/plan-gate.md` before the first action of a task, and hold it for the whole task. It binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The gate is why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, and the gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the approval turn only. Still state the ordered plan before acting, and still validate it as phase 2 directs.

## The loop

1. Observe: `rk doctor` for the host, `rk status --target .` for the landed payload (`--check` to judge), `rk setup check --target .` for the forge shape, `rk versions --check` for the pins.
2. Classify every finding as one of: run it now, gate it for the operator, or already satisfied.
3. Act on the automated findings, one at a time, smallest first.
4. Re-run the observation that found the gap. A check that cannot be read is not a pass, and a failed check is never continued past — it is the next gap.
5. Stop when `rk status --check`, `rk setup check`, and `rk versions --check` are green; the first release through `rk method operate` is the migration's proof.

## What runs unattended

| Finding                                               | Act                                                                                         |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| A target with no landing record                       | `rk adopt --target . --scopes <list> --apply`                                               |
| A landed payload older than the binary                | `rk upgrade --target . --apply`                                                             |
| A record without the scopes parameter                 | `rk upgrade --target . --scopes <list> --apply`, the list confirmed with the operator first |
| A missing hook block in `.pre-commit-config.yaml`     | The upgrade lands it; reconcile the config first, as below                                  |
| A missing or drifted forge protection                 | `rk setup step <name> --apply`                                                              |
| A squash title source that is not the request's title | `rk setup step protect-trunk --apply` re-asserts it                                         |
| An unfilled `TODO(release-kit)` sentinel              | Resolve it in the seeded file, from the project                                             |
| Stale installed skills                                | `rk skill install --apply`                                                                  |

Mechanical file edits can go to a subagent; every forge mutation stays in this loop, where its observation lives.

Before an upgrade lands the hook block into an existing `.pre-commit-config.yaml`, reconcile it as `rk guide setup` step 4 directs, and gate the choice between an existing hook and the landed one for the operator rather than stacking a second hook on one job.

## What waits for the operator

Gate each of these: print the exact command, say what it changes and why, wait, then re-observe before continuing.

- `rk setup step single-trunk --apply` — destructive; it deletes retired long-lived branches, and its ancestry guard refusing is a stop, not an obstacle.
- `install-bot` and `bot-secrets` — the bot identity and its credentials; `rk forge <name>` carries the walkthrough.
- Registry actions: the first hand publish, registering the trusted publisher, turning on enforcement. `rk guide setup` names each with its reason.
- Removing an existing protection from a live branch, on any forge.

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
