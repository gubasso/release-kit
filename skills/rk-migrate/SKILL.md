---
name: rk-migrate
description: Drives a repository from its current state to the current release-kit convention, mixing steps it runs itself with steps it gates for the operator. Use when asked to migrate a project onto release-kit, adopt the convention in an existing repository, upgrade a landed payload, move a gated two-branch release flow onto the trunk convention, or reconcile a drifted setup. Triggers include migrate, adopt release-kit, rk upgrade, trunk migration, and setup drift.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Forge mutations need the forge CLI authenticated with administration rights; the operator supplies credentials and registry actions.
---

# rk-migrate

Take a repository from wherever it releases today to the current release-kit convention. The procedure this skill drives is `rk method migration`, with `rk guide migration` as its commands; read the chapter once per task and hold it. The work is a loop, not a script: observe, act on the smallest gap, re-observe, continue — and everything the operator must do by hand is gated, stated as an exact command, and verified after they run it.

## Before acting

Read two shared files before the first action of a task, in this order, and hold both for the whole task.

1. `~/.local/state/release-kit/skills/shared/pre-flight-gate.md` — run it whatever the request carries. It checks this host with `rk doctor` and stops the task on what no plan can work around. No flag skips it.
2. `~/.local/state/release-kit/skills/shared/plan-gate.md` — it binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The two gates are why this skill is safe to run: every verb the runbook names writes files, changes a forge, or publishes a version, the pre-flight says whether this host can run it at all, and the plan gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the plan gate's approval turn only. Still run the pre-flight, still state the ordered plan before acting, and still validate it as phase 2 directs.

## Detect

The pre-flight's last step already ran `rk assess --target . --json` and `rk status --target .`; this skill routes on what they returned, and the chapter's first section defines the verdicts.

- `brownfield` with no record is this skill's subject: the runbook, start to end.
- `greenfield` belongs to the rk-setup skill, because there is nothing to migrate; hand off and stop.
- `needs-decision` is the operator's call, asked with `AskUserQuestion` and the report's evidence — the tags, the branches — before any plan claims to know what the release activity is.
- A recorded target routes by `rk status`, whatever the verdict says: an older record or a missing parameter is runbook step 2b, a mode or style change is step 2c, and drift is the finding the report names.

## The inventory

The plan's body is the findings inventory the chapter shapes: one entry per gap runbook step 1 reports, with its disposition — run it now, gate it for the operator, already satisfied — its command by runbook step number, and the observation that verifies it. The inventory names `rk runs` for what the setup steps did and the landing record for what landed, and copies neither. A discovery returns to planning and is never handled inline.

Mechanical file edits — answering a sentinel, aligning a block to the candidate's bytes — can go to a subagent; every forge mutation stays in this loop, where its observation lives.

## What waits for the operator

Gate each of these: print the exact command, say what it changes and why, wait, then re-observe before continuing.

- Removing a protection from a live branch, on any forge — runbook steps 3a and 5a.
- `rk setup step single-trunk --apply` — runbook step 5b; destructive, and its ancestry guard refusing is a stop, not an obstacle.
- `install-bot` and `bot-secrets` — the bot identity and its credentials; `rk forge <name>` carries the walkthrough.
- Registry actions: the first hand publish, registering the trusted publisher, turning on enforcement. `rk guide setup` names each with its reason.
- The release style, on a record that predates it — runbook step 2b. Ask it with `AskUserQuestion` the way rk-setup's step 6 states, because arming an existing project's release request changes what a green trunk does.
- The development environment, where the project obtains `rk` by a host install or a hand-rolled bump — runbook step 6. Ask it with `AskUserQuestion` the way rk-setup's step 6 states, with the replacement as the default; the migration is not done while the cleanup's `leftovers` list is non-empty.
- The predecessor's removal itself, and every other removal: what it removes is committed first, per the chapter's recoverability section.
- A workspace promotion under worktree mode: the skill may state the layout from `rk method worktrees` and render the commands from `rk guide worktree` step 5, and runs none of them; moving directories on the operator's disk is never a code change.

## When it goes wrong

- A check that cannot be read is not a pass, and a failed check is never continued past: it is the next entry, and the step that failed is the whole of the next action.
- A refusal from `rk adopt` naming the two marked blocks is the alignment still owed — runbook step 2a — never an error to force past.
- A refusal from `rk upgrade` naming a conflict is a release-kit-owned file the target edited: reconcile it as `rk guide setup` step 4d directs, then rerun; never widen the plan to keep the edit.
- An interrupted migration resumes from the inventory: runbook step 7a reruns the observations and continues from the first entry they do not satisfy.
- A gap the observations report that the inventory does not name returns to planning before anything runs.

## Defaults

- Never author a tag, and never hand-edit a generated artifact workflow.
- Prefer an rk verb over a raw forge call; where no verb covers the gap, use the forge CLI and say that the loop is outside the convention there.
- Report every gated step's outcome from observation, never from the operator's word alone.
- Leave the repository releasable at every stop: a migration interrupted between steps must break no existing flow.
- Never widen an approved scope inline: a mode or style change, a second branch, a workspace move is its own entry, approved at its own size.
