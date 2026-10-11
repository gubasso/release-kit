---
name: rk-setup
description: Lands the release-kit release workflow in a project through the rk CLI, migrates, adopts, upgrades, repairs, and configures it, sets up the forge and the registry it releases through, and provisions the rk binary it runs on. Use when asked to set up a release workflow, release automation, trusted publishing, release-plz, release-please, git-cliff, changelog automation, or trunk-based release automation, to migrate a project that already releases through another tool onto release-kit, to take a landed project to the installed release, to repair setup drift, to change a recorded answer or an opt-in capability, or to install or pin rk for a project. Triggers include release-kit, rk init, rk stage, rk upgrade, rk adopt, rk setup, release setup, migrate to release-kit, adopt release-kit, upgrade release-kit, setup drift, vulnerability reporting, SECURITY.md, and pin rk.
license: CC-BY-4.0
compatibility: Provisions the rk binary where the host lacks it. Landing files into a target needs write access to that repository. Forge mutations need the forge CLI authenticated with administration rights; the operator supplies credentials and registry actions.
---

# rk-setup

Set up the release-kit convention in a project, keep a landed project current with the installed `rk`, and provision the `rk` it runs on. The state of the target decides which tasks are possible, the request picks among them, and the operator confirms each task before it runs.

## Pre-flight

1. Before any other action, read `references/pre-flight-gate.md` and run it whatever the request carries. No flag skips it.
2. Stop on any row below. State the evidence and the operator's step, and write nothing. The evidence comes from `rk status --target . --json` unless the row names another source.

| Stop                                     | Owner       | Evidence                      | Operator's step                  |
| ---------------------------------------- | ----------- | ----------------------------- | -------------------------------- |
| A failed `hard` probe                    | shared gate | `rk doctor`                   | its `next` line                  |
| A failed `skill-gate` or `skill-sources` | shared gate | `rk doctor`                   | its `next` line                  |
| A failed `skill-roots` probe             | shared gate | `rk doctor`                   | install on the host that owns it |
| Not the repository the request means     | shared gate | `git remote get-url origin`   | name the repository              |
| A diagnostic, not a report               | this skill  | the command exits on `reason` | repair the file it names         |
| Not a non-bare Git work tree             | this skill  | `git_work_tree` is false      | make the target a Git work tree  |

An unreadable record never counts as an absent one, so never land over it.

1. Hold these observations for the gate. None of them ends the run. Read `alignment` before `classification`, because a record newer than the binary reads `classification` null.

| Observation                                            | Evidence                                  | Routes to                               |
| ------------------------------------------------------ | ----------------------------------------- | --------------------------------------- |
| No `rk` on PATH                                        | `command -v rk`, by hand                  | `provision`, then this pre-flight again |
| A record newer than the binary                         | `alignment` `target-newer`                | `provision`                             |
| No pin and no host decision, at `setup` or `migration` | `rk self-depend status --target . --json` | `provision`, offered first              |
| A pin with no sync line, at `setup` or `migration`     | `wired` and `envrc_sync` in that report   | `provision`, offered first              |
| The state of the target                                | `classification`                          | the gate                                |
| The setup steps and the setup proof                    | `rk setup status --target . --json`       | `forge-settings` and `publish`          |

1. Hand the gate the status report, each failed `soft` probe as a plan constraint, and the self-depend and setup reports. The gate reads nothing else.

## General rules

1. Route, never restate. Find the why with `rk method <chapter>`, the commands with `rk guide <runbook>`, a technology with `rk binding <tech>`, a forge with `rk forge <name>`, and the whole command surface with `rk usage`.
2. Decide from the `--json` output of a verb, never from its human text. Never read "already current" from the words of an upgrade preview.
3. Leave rendered bytes to release-kit. Never edit a file or a region `rk` renders, and never hand-edit a generated artifact workflow: change its configuration and regenerate, as the binding directs. Never author a tag.
4. Use an `rk` verb first. Where no verb covers a step, use the forge CLI and say that the step runs outside the convention.
5. Report from observation. Report each step's outcome from what a check observed, never from the operator's word alone. A credential this runtime cannot reach leaves a step `unknown`, which is an observation boundary. Never ask for a secret so that a check can pass.
6. Keep one record per repository and the skills at user scope. A target carries one release-kit record at its root and no skill file, because `rk skill install` puts the skills under the user's home.
7. Let the repository record how it obtains `rk`. The `provision` task proposes the method, the operator picks it, and every install runs as a gated step.
8. Keep no state of your own, with one exception. The target, its record, the stage receipt, and Git are the records, and a later run reads the state from `rk status --target . --json`. A migration's workspace is the exception: it keeps the archived files, the snapshots, and the registry across runs.

## Gate

Read the state from `classification`, never from the request, and the intent from the request. A cell is one ordered task list, and its tasks share one plan and one approval.

| State            | set up or none                      | upgrade   | fix       | change a setting            | forge setup                         | pin or environment |
| ---------------- | ----------------------------------- | --------- | --------- | --------------------------- | ----------------------------------- | ------------------ |
| `setup`          | `forge-settings`, `land`, `publish` | ask       | ask       | `land`                      | `forge-settings`, `land`, `publish` | `provision`        |
| `migration`      | `migrate`                           | ask       | ask       | `migrate`                   | `migrate`                           | `provision`        |
| `needs-decision` | ask                                 | ask       | ask       | ask                         | ask                                 | `provision`        |
| `adopt`          | `adopt`, then the setup tasks       | `adopt`   | `adopt`   | `adopt`, then `configure`   | `adopt`, then the setup tasks       | `provision`        |
| `upgrade`        | `upgrade`                           | `upgrade` | `upgrade` | `upgrade`, then `configure` | `upgrade`, then the setup tasks     | `provision`        |
| `drift`          | `repair`                            | `repair`  | `repair`  | `repair`, then `configure`  | `repair`, then the setup tasks      | `provision`        |
| `current`        | the setup tasks, else done          | done      | done      | `configure`                 | the setup tasks                     | `provision`        |

- Put `provision` first where an observation routes to it. With no `rk`, it is the only task. After it, run the pre-flight again and read this table in the same run.
- On a first landing, keep the order `rk guide setup` takes: `forge-settings` covers its §0 to §3, `land` its §4, and `publish` its §5 to §8. On a recorded target, the landing task comes first, then `configure`, then `forge-settings`, then `publish`, because `configure` needs a record at the binary's version.
- The setup tasks are `forge-settings` where the setup report names a step that is not `satisfied`, then `publish` where it reads the proof `absent` or `stale`. `migrate` runs both itself, once its branch merged.
- Ask: state the evidence and the options, wait, then read this table again with no new pre-flight. For `needs-decision`, the operator says what the tags or the second branch are.
- Hand off: a release, a release line, or a release recovery goes to the `rk-release` skill.
- Out of scope: starting work, integrating a branch, and pruning are the project's own workflow. Say so in one line, and name the helper verb that exists: `rk issue start`, `rk integrate`, `rk worktree`, or `rk branches prune`.
- Done: report from observation. For an upgrade asked at `current`, state "The record matches `rk <version>`. To move further, ask to move the pin.", which is a request that runs `provision`, then `upgrade`.

The operator confirms every step:

1. Present "State: <state>. Request: <intent>. Tasks: <list>. Proceed?" and wait. A redirect reads this table again with the new intent.
2. Before each task, print "Next: <task>. It will <one line>. Proceed?" and wait. Print one progress line at each move between the pre-flight, the execution, and the post-flight of a task.
3. After the task pre-flights of the cell, read `references/plan-gate.md` and hold it for the rest of the run. The plan names each task, the verbs it runs, and the files it writes.

## Tasks

After the gate picks a task, read its file into this context. A task file holds the pre-flight, the execution, and the post-flight of one task, and it links no other file.

- If the run includes `provision`, read `references/provision.md`.
- If the run includes `land`, read `references/land.md`.
- If the run includes `migrate`, read `references/migrate.md`.
- If the run includes `adopt`, read `references/adopt.md`.
- If the run includes `upgrade`, read `references/upgrade.md`.
- If the run includes `repair`, read `references/repair.md`.
- If the run includes `configure`, read `references/configure.md`.
- If the run includes `forge-settings`, read `references/forge-settings.md`.
- If the run includes `publish`, read `references/publish.md`.
- If the run includes `land`, `migrate`, `adopt`, `upgrade`, `repair`, or `configure`, read `references/landing.md`, the pipeline they share.

Compose them this way:

- `land`, `adopt`, `upgrade`, `repair`, and `configure` run their task pre-flight, then the Stage, Compare, Prepare, Land, and Prove steps of the pipeline, then their task post-flight. `migrate` runs the pipeline inside its own Land step.
- Where the gate composes `configure` after a landing task, its answers join that task's Stage step, so the run lands once. `configure` runs its own landing only at `current`.

## Post-flight

Run these once, after the last task post-flight, in this order.

1. Check. Run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`, the suite every integration runs. Where the pre-flight found no `pre-commit`, skip the second command and state in the report that the suite did not run.
2. Clean. Where this run staged and every check passed, print `rk stage clean <path>`, wait, and run it on a yes. It removes the stage alone. Saved copies, forge snapshots, and a migration workspace stay, and the report lists each path.
3. Report. Fill the template from observation, with every answer the operator gave.

On a failed check, keep the stage, report, and route: an `rk` violation goes to a new run, whose gate picks `repair`, and a failing project check is the project's own. Never repair inside the post-flight.

```text
Result: <the exact rk status --check line, or the first failing line>
Tasks:
  - <task>: <files written or removed>; proof: <task post-flight evidence>
Answers recorded:
  - <question>: <answer>
Git and forge actions:
  - <action>: <run under the request | printed for the operator>
Setup proof: <compatible | stale | absent | unknown, because <reason>>
Saved before changing:
  - <path>: <what it holds>
Still the operator's:
  - <exact command>: <what it changes>
Stage: <cleaned | kept at <path>, because <reason>>
Next: <one action>
```

Print "Git and forge actions" and "Saved before changing" only where the run has an entry for them.
