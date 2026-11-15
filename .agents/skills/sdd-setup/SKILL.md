---
name: sdd-setup
description: Lands, migrates, upgrades, repairs, and tunes a spec-driven-docs instance through the sdd CLI, and provisions the sdd binary it runs on. Use when asked to install or set up spec-driven docs in a project that documents nothing, to migrate a project that already documents itself onto the convention, to take a landed instance to the installed release, to diagnose drift and sdd verify failures, to change what the gates judge or the lint configuration, or to install or pin sdd for a project. Triggers include spec-driven-docs, sdd init, sdd stage, sdd status, sdd verify, sdd upgrade, migrate to sdd, adopt spec-driven docs, convert docs conventions, docs governance instance, reserve a path from the gates, and pin sdd.
license: CC-BY-4.0
compatibility: Requires prek, which runs the delivered gates the landing wires into the project's hook configuration. Provisions the sdd binary where the host lacks it. Every sdd verb runs offline and lands the version of sdd that is installed.
---

# sdd-setup

Set up, migrate, upgrade, repair, and tune a spec-driven-docs instance, and provision the `sdd` it runs on. The state of the target decides which tasks are possible, the request picks among them, and the operator confirms each task before it runs.

Where a file this skill names is not on disk, as in a repository with no instance yet, read it from the binary: `sdd skill show sdd-setup --file <path>`, for example `--file references/pre-flight-gate.md`.

## Pre-flight

1. Before any other action, read `references/pre-flight-gate.md` and run it whatever the request carries. No flag skips it.
2. Stop on any row below. State the evidence and the operator's step, and write nothing. The evidence comes from `sdd status --target . --json` unless the row names another source.

| Stop                                   | Owner       | Evidence                                       | Operator's step          |
| -------------------------------------- | ----------- | ---------------------------------------------- | ------------------------ |
| A failed `hard` probe                  | shared gate | `sdd doctor`                                   | its `next` line          |
| Skills and `sdd` from different builds | shared gate | the `skill-payload` probe                      | its `next` line          |
| No repository, or not the one meant    | shared gate | the report, or `git rev-parse --show-toplevel` | name the repository      |
| An unreadable record or declaration    | this skill  | a diagnostic, or `classification` `invalid`    | repair the file it names |

An unreadable record never counts as an absent one, so never land seeds over it.

1. Hold these observations for the gate. None of them ends the run. Read `alignment` before `classification`, because an instance newer than the binary reads `classification` `null`.

| Observation                                            | Evidence                                   | Routes to                               |
| ------------------------------------------------------ | ------------------------------------------ | --------------------------------------- |
| No `sdd` on PATH                                       | `command -v sdd`, by hand                  | `provision`, then this pre-flight again |
| An instance newer than the binary                      | `alignment` `instance-newer`               | `provision`                             |
| No pin and no host decision, at `setup` or `migration` | `sdd self-depend status --target . --json` | `provision`, offered first              |
| The state of the target                                | `classification`                           | the gate                                |

1. Hand the gate the status report, its `paths`, each failed `soft` probe other than `git` and `prek` as a plan constraint, each failed `git` or `prek` probe as an install step for the operator, and the `self-depend` report. The gate reads nothing else.

## General rules

1. Route, never restate. Find an answer with `sdd docs <topic>`, a rule with `sdd spec <domain>`, and the home of a fact with `sdd docs placement`. Never copy a chapter into this skill or into a plan.
2. Take every path from the `paths` section of `sdd status --target . --json`. Never spell a path by hand.
3. Decide from the `--json` output of a verb, never from its human text.
4. Leave managed bytes to the canon: never edit a managed file or the inside of a marker block, the agents block included. `sdd init` manages this block, which routes an author to `sdd method writing-style` or the selected source.
5. Keep one instance at the repository root. The instance carries the skills: `sdd init` and `sdd upgrade` land them into its skill roots at the version the project pins, and the project commits them. Never install a skill under the home.
6. Let the repository record how it obtains `sdd`. The `provision` task proposes the method, the operator picks it, and every install is a gated step. What lands is the `sdd` the recorded method provides.
7. Keep no state of your own. The target repository, the stage receipt, and Git are the records, and a later run reads the state from `sdd status --target . --json`. The one exception is a migration, which spans many runs: its workspace keeps the archived sources and the registry, and `sdd migrate status` reads them.
8. Follow the fix a version refusal names. Every command that reads or writes an instance compares the record with the binary first, and names `sdd init`, the upgrade through this skill of the `sdd` in the devshell, or the `sdd` of the project's own environment. Run that fix, and never work around the refusal.

## Gate

Read the state from `classification`, never from the request, and the intent from the request. A cell is one ordered task list, and its tasks share one plan and one approval.

| State            | set up or none | upgrade   | fix       | gates or lint                | env       |
| ---------------- | -------------- | --------- | --------- | ---------------------------- | --------- |
| `setup`          | `land`         | ask       | ask       | `land`                       | `land`    |
| `migration`      | `migrate`      | ask       | ask       | `migrate`                    | `migrate` |
| `needs-decision` | ask            | ask       | ask       | ask                          | ask       |
| `upgrade`        | `upgrade`      | `upgrade` | `upgrade` | `upgrade`, then `tune-gates` | no task   |
| `drift`          | `repair`       | `repair`  | `repair`  | `repair`, then `tune-gates`  | no task   |
| `current`        | done           | done      | done      | `tune-gates`                 | no task   |

- Put `provision` first where an observation routes to it. With no `sdd`, it is the only task. After it, run the pre-flight again and read this table in the same run. A host answer that `provision` holds for the Stage step settles the pin row for this run.
- Run `tune-gates` only on an instance at the binary's version, which is why `upgrade` and `repair` come first in their cells.
- Send "no task" straight to the post-flight, whose environment step does the work.
- Ask: state the evidence and the options, wait, then read this table again with no new pre-flight. For `needs-decision`, the operator names the home of the documentation.
- Hand off: a gate failure inside a document goes to the `sdd-write-docs` skill.
- Done: report from observation. For an upgrade asked at `current`, state that the instance matches the installed `sdd`, and that moving the pin is a request that runs `provision`, then `upgrade`.

The operator confirms every step:

1. Present "State: <state>. Request: <intent>. Tasks: <list>. Proceed?" and wait. A redirect reads this table again with the new intent. Where the cell holds `migrate` and no migration is open, warn once with it: the repository holds no documentation between the archive and the close, and where that work happens is the operator's choice.
2. Judge each item of the request against the state and the rules. An item that fits becomes a plan constraint. An item that does not fit the state gets the evidence and the task that fits, then a question. An item that breaks a rule gets the rule ID, then a question.
3. Before each task, print "Next: <task>. It will <one line>. Proceed?" and wait. Print one progress line at each move between the pre-flight, the execution, and the post-flight of a task.
4. After the task pre-flights of the cell, read `references/plan-gate.md` and hold it for the rest of the run. The plan names each task, the verbs it runs, and the files it writes, and it covers every task execution and post-flight.

## Tasks

After the gate picks a task, read its file into this context. A task file holds the pre-flight, the execution, and the post-flight of one task, and it links no other task file.

- If the run includes `provision`, read `references/provision.md`.
- If the run includes `land`, read `references/land.md`.
- If the run includes `migrate`, read `references/migrate.md`.
- If the run includes `upgrade`, read `references/upgrade.md` with `sdd skill show sdd-setup --file references/upgrade.md`.
- If the run includes `repair`, read `references/repair.md`.
- If the run includes `tune-gates`, `land`, or `migrate`, read `references/tune-gates.md`.
- If the run includes `land`, `migrate`, `upgrade`, or `repair`, read `references/landing.md`, the pipeline those four share.

Compose them this way:

- `land`, `migrate`, `upgrade`, and `repair` run their task pre-flight, then Stage, Compare, Prepare, Land, and Prove from the pipeline, then their task post-flight.
- `land` and `migrate` run the `tune-gates` pre-flight before the Stage step. Its reserved paths join the choices of the Stage step, its lint edits run in the Prepare step, and its proof runs after the Land step. It writes nothing where it finds nothing and the operator reserves nothing.
- After `upgrade` or `repair`, and on its own, `tune-gates` runs whole, after the landing.

## Post-flight

Run these once, after the last task post-flight, in this order.

1. Env. Run `sdd env --target . --apply <item>` for each item the operator approved in the plan, with `--value <path>` for `docs-scratch-variable`, and skip this step where none was. Where the run landed or moved an instance, or the request asked for it, the plan read `sdd env --target . --json` and asked about each unwired item.
2. Check. Run `sdd verify --target .`, then `prek run --all-files`. Where the second command fails because the host lacks prek, ask the operator to install it, then run the command again.
3. Clean. Where this run staged and every check passed, print `sdd stage clean <path>`, wait, and run it on a yes. It removes the stage alone. Saved copies and a migration workspace stay, and the report lists each path.
4. Report. Fill the template from observation, with every answer the operator gave.

On a failed check, keep the stage, report, and route. A rule ID inside a document goes to the `sdd-write-docs` skill. Drift or a managed conflict goes to a new run, whose gate picks `repair`. Never repair inside the post-flight.

```text
Result: <the exact sdd verify line, or the first failing line>
Tasks:
  - <task>: <files written or removed>; proof: <task post-flight evidence>
Answers recorded:
  - <question>: <answer>
Git actions:
  - <action>: <run, under <the workflow term> | printed for the operator>
Saved before changing:
  - <path>: <what it holds>
Still the operator's:
  - <exact command>: <what it changes>
Stage: <cleaned | kept at <path>, because <reason>>
Next: <one action>
```

Print "Git actions" and "Saved before changing" only where the run has an entry for them.
