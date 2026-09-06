---
name: rk-depend
description: Adds another project as a dependency of the current one through the rk CLI, after reading how that project distributes itself and how this one manages its tools. Use when asked to depend on, add, pin, or wire in a tool or library from another repository, or to add a flake input, mise tool, asdf tool, or devbox package for it. Triggers include depend, add as dependency, rk depend, dev dependency, and tool pin.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH. Seeding a manager file needs write access to the target; a prod dependency runs the technology's own command, which the operator approves.
---

# rk-depend

Take another project as a dependency of the current one. The CLI reads both sides: `rk depend assess` reports the source's channels and the target's managers offline, and `rk depend add` serves one landing. The chapter `rk method dependencies` owns the why, and the runbook `rk guide dependencies` owns the commands, by step number.

## Before acting

Read two shared files before the first action of a task, in this order, and hold both for the whole task.

1. `~/.local/state/release-kit/skills/shared/pre-flight-gate.md` — run it whatever the request carries. It checks this host with `rk doctor` and stops the task on what no plan can work around. No flag skips it.
2. `~/.local/state/release-kit/skills/shared/plan-gate.md` — it binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The two gates are why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, the pre-flight says whether this host can run it at all, and the plan gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the plan gate's approval turn only. Still run the pre-flight, still state the ordered plan before acting, and still validate it as phase 2 directs.

## Route to the canon

| Need                                        | Command                                         |
| ------------------------------------------- | ----------------------------------------------- |
| The two kinds, the matrix, and the sequence | `rk method dependencies`                        |
| The procedure, as commands                  | `rk guide dependencies`                         |
| Both sides read, every option laid out      | `rk depend assess --source <source> --json`     |
| One landing previewed                       | `rk depend add --source <source> --kind <kind>` |
| How this project obtains `rk` itself        | `rk devshell status`                            |

## Take the source

The source is the dependency's local checkout. A request that names a path passes it as `--source`. A request that names a URL takes runbook step 2a first: the shallow clone into the scratch directory is a gated operator step, and the cloned path is the source. Never pass a URL to the verb; it refuses.

## Decide

Runbook step 1 is the kind, and it is asked before the plan unless the request states it: a tool the developers run is `dev`, a library the code imports is `prod`. Ask with `AskUserQuestion`, with the consequence stated: dev lands in the target's tool manager, prod runs the technology's own command.

Read the assessment, then ask only what it leaves open:

- the manager, when the target carries several manager files or none; a lone manager chooses itself.
- the channel, when the chosen manager has more than one `fragment` option in the report; the first is the default.
- the version, when the request names one or the operator wants a release other than the source tree's; `--pin` takes `1.2.3`, `v1.2.3`, or the release URL.

An `already` line in the report means the target names the dependency in a manager file; present that before planning a second pin.

## Land

Route by the runbook's numbers. Step 5 is the preview, always run and read before step 6. Step 6 has four forms, and the report's `landing` field says which applies:

- `fragment` with the manager file absent: 6a, `--apply` seeds the file.
- `fragment` with the manager file present: 6b, the fragments applied by hand at their anchors, in the printed order, then the verb rerun until every fragment reads present.
- `manual`: 6c, the report's reason names what the operator supplies; write the line the report printed once it is known, and invent no attribute or plugin.
- `native`: 6d, the printed command is a gated operator step; the verb never edits a manifest.

Step 7 verifies from the manager's own shell or the manifest's own tree, and the report's freshness line names what moves the pin from then on. Commit the manager file with its lock in one change.

## When it goes wrong

| Symptom                          | Path                                                                             |
| -------------------------------- | -------------------------------------------------------------------------------- |
| `source-unknown`                 | the checkout declares no channel this binary reads; stop and say what is missing |
| `manual-only`                    | every pair is a hand edit; plan them from the reasons, with nothing to apply     |
| `version-unknown`                | the source declares no version; ask for the release and pass `--pin`             |
| exit 73 on `--apply`             | the file is the target's own; step 6b                                            |
| exit 64 naming `--manager`       | the target carries several managers or none; ask, then rerun                     |
| a pin the manager cannot resolve | runbook step 7a: `--pin` at a published release                                  |

## Defaults

- Never edit a manifest or manager file the target owns; serve the fragments and let the hand edit be the operator's.
- Never invent a nixpkgs attribute, an asdf plugin, or a hash; a manual reason is the report's answer.
- Never pass a URL as the source; the clone is a gated step.
- Report from the verb's output, never from an assumption about what a file holds.
