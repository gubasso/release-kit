---
name: rk-setup
description: Lands the release-kit workflow in a project through the rk CLI, and keeps a landed project current from the version the operator installed. Use when asked to set up a release workflow, release automation, trusted publishing, release-plz, release-please, git-cliff, changelog automation, or trunk-based release automation, to migrate a project that already releases through another tool onto release-kit, to stage, upgrade, or adopt a landing, or to adapt the release-kit convention to a project's technology. Triggers include vulnerability reporting, SECURITY.md, release-kit, rk init, rk stage, rk upgrade, rk adopt, release setup, migrate to release-kit, adopt release-kit, upgrade release-kit, setup drift, and release workflow setup.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Landing files into a target needs write access to that repository. Forge mutations need the forge CLI authenticated with administration rights; the operator supplies credentials and registry actions.
---

# rk-setup

Land the release-kit convention in a project, or take a landed project to the release the operator installed. One arrival path serves a first landing, an adoption, and an upgrade: the installed `rk` stages its own candidate, this skill studies the stage against the working tree, the receipt, and Git, and the same `rk` renders again into production. `rk method landing` owns the why, and `rk guide landing` owns the commands by step number. This skill runs the gates, reads the evidence, prepares what the landing needs, and reports what each verb returned.

## Before acting

Read two shared files before the first action of a task, in this order, and hold both for the whole task.

1. `~/.local/state/release-kit/skills/shared/pre-flight-gate.md` — run it whatever the request carries. It checks this host with `rk doctor`, reads the installed version and the target's record, and stops the task on what no plan can work around. No flag skips it.
2. `~/.local/state/release-kit/skills/shared/plan-gate.md` — it binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The two gates are why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, the pre-flight says whether this host can run it at all, and the plan gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the plan gate's approval turn only. Still run the pre-flight, still state the ordered plan before acting, and still validate it as phase 2 directs.

## Route to the canon

| Need                                                            | Command                                             |
| --------------------------------------------------------------- | --------------------------------------------------- |
| Judge this host                                                 | `rk doctor`                                         |
| List method chapters, read one                                  | `rk method --list`, `rk method <chapter>`           |
| List bindings, read one                                         | `rk binding --list`, `rk binding <tech>`            |
| Read a forge's specifics                                        | `rk forge <name>`                                   |
| The stage, investigate, land, verify, clean procedure           | `rk guide landing`                                  |
| The setup procedure around a first landing, as commands         | `rk guide setup`                                    |
| The migration procedure around a landing, as commands           | `rk guide migration`                                |
| The installed version, and the state it reports                 | `rk --version`, `rk status --target .`              |
| Classify a target that carries no record                        | `rk assess --target .`                              |
| Write this binary's candidate and its knowledge into a stage    | `rk stage --target .`                               |
| Remove one stage that names itself                              | `rk stage clean <path>`                             |
| Land, upgrade, or record a target, previewed then applied       | `rk init`, `rk upgrade`, `rk adopt`, with `--apply` |
| List the landable files, print one                              | `rk snippet --list`, `rk snippet <tech>/<path>`     |
| Print the pinned-tool registry                                  | `rk versions`                                       |
| List the executable setup steps                                 | `rk setup --list`                                   |
| Judge a message against the content guards                      | `rk message --check`                                |
| The merged branches this clone still holds                      | `rk branches prune`                                 |
| The worktree lifecycle, as commands                             | `rk guide worktree`                                 |
| Start work an issue names, and its procedure                    | `rk issue start <issue>`, `rk guide issue`          |
| The release line's whole life, and its verbs                    | `rk guide release-lines`, `rk lines --help`         |
| The working-copy forms and the mode                             | `rk method worktrees`                               |
| How this project obtains `rk`, per manager                      | `rk self-depend status`                             |
| The fragments for one manager and venue pair, and the seed file | `rk self-depend add`                                |
| The predecessor bump mechanism's removal                        | `rk self-depend clean`                              |
| Another project as a dependency                                 | `rk depend assess`                                  |

## Installation scope

Skills and the agent setup install at user scope only — `rk skill install --apply`, once per user; no system mode exists, by decision. The setup runbook's prerequisites own the step and the two roots. Where a file lands for a third-party application is the target project's own decision, made against that application's documentation with a dated citation — never generalized from another application.

## Start work an issue names

A request naming an issue — an issue URL, or "fix", "address", "implement", or "work on" plus an issue — starts from `rk issue start <issue>`, and the plan's first step is that command. The forge names the branch and rk seats it the way the project's recorded workflow mode says. Three rules bind this.

- Never write a predicted branch name into the plan. The name is whatever the forge mints, and stating a guess is the mistake this verb exists to prevent. Write "the branch the forge mints for issue <n>".
- The preview is what the plan presents for approval, and the apply is what execution runs. That is the plan gate's own two-phase shape, so no new gate appears here.
- Minting a branch at the forge is a forge action, so it happens only where the operator's request named starting work on that issue.

## The five steps

1. Run the gates. The pre-flight gate's closing lines hand over the installed version, the record's `rk_version`, and whether the target carries a record.
2. Read the version and stop where no update was authorized. `rk guide landing` step 1a reads how the project obtains `rk`. This skill never chooses, installs, updates, downgrades, or fetches a version: where the request did not authorize an update, report the installed state and end the task before any migration work. Where the installed binary is a 0.4.x release the operator will replace, `rk guide landing` step 1b runs first, with that binary.
3. Stage and investigate, `rk guide landing` steps 1c and 2. Keep the stage path for the whole task. Name the evidence class from the section below before proposing any edit, and prepare the target only within the request's authority.
4. Land, `rk guide landing` step 3. The production verb renders again from the binary and the target. Read each word it prints and quote them, and take a refusal back to the investigation, never to a flag.
5. Verify and clean, `rk guide landing` steps 4 and 5. Compare the real diff with the stage, run the checks, and repair through another fresh production run. Keep an inventory of every stage path the task created. Show the exact `rk stage clean <path>` for each one only after everything passed, and run them only where the request's authority includes cleanup. Then raise the decision below, before the task closes.

## How the target obtains `rk`

A landing that leaves this unasked leaves the pin where it found it. A target with a pin and no line keeps the release it pinned until somebody bumps it by hand. A target with no manager takes whatever version the host installed, for every project on that machine. `rk self-depend status` reports both, this is the only place that reads it, and the report carries the operator's answer either way.

- Raise it where the landing recorded the target for the first time, through `rk init` or `rk adopt`. An upgrade of a recorded target asks nothing: the question was answered once, and repeating it is noise.
- Read `rk self-depend status --target . --json` and hold `state`, `wired`, `envrc`, and `envrc_sync`. The report is the only source: no manager list of this skill's own.
- Where `state` is `no-manager`, nothing pins `rk` here. Preview `rk self-depend add --manager <candidate> --target . --json` for each manager the status listed, and keep the ones whose `support` is not `manual`. A manual pair names a reason and writes nothing, so offering it offers a refusal.
- Ask with `AskUserQuestion` and offer three answers: one of the managers that survived, which puts the version in the repository where a diff shows it; a host install, which serves one version to every project on that machine; and the operator wiring it themselves, which ends the question. Say what each costs before the operator picks. On a manager, preview `rk self-depend add --manager <chosen> --target . --json` with the answer and not with `wired`, because a no-manager report names no `wired` manager to reuse. Then run the same call with `--apply` as a gated step, which seeds the manager file, and read `rk self-depend status` back: it names the chosen manager under `wired`, or the wiring did not happen.
- One candidate would answer the second question by itself. The flake pair's apply seeds an `.envrc` carrying the sync line where the target has none, so it wires the pin and places the line in one move. Say that, and ask the freshness question before that apply rather than after it. Where the operator wants the pin without the line, they drop the line from the seeded file, and the status reports `envrc_sync` false again.
- Where `wired` names a manager and `envrc_sync` is false, the pin is there and nothing moves it. Ask with `AskUserQuestion`, and state what the line does: on directory entry, at most once a day, it asks the wired manager's pin to move forward, and it leaves a diff to review. How many files that diff carries is the manager's own answer, which the sync report names. Do not predict it.
- Where the operator accepts, run `rk self-depend add --manager <wired> --target . --json` with the manager the report named. An unqualified call resolves the manager from the files present, so it refuses a target carrying more than one.
- Place the served line as a gated step: print the exact edit, wait, then apply it. The verb writes only a file the target lacks, so an `.envrc` already there takes the line by the operator's approval alone.
- The binary may serve the line and may not place it, so nothing else in this loop asks. A refusal ends the decision and goes in the report.
- Ask once. An answer already recorded closes the question for this landing, whichever answer it was.

## Which chapter the arrival loads

- `greenfield`, from `rk assess`: `rk method setup` and `rk guide setup`. The landing is that runbook's step 4a. Steps 1 to 3 surround it on the forge side and steps 5 to 8 on the registry side. How the project obtains `rk` is none of them: `rk guide setup` carries it in the Prerequisites, with the whole pin order. The workflow mode, the release style, and the Nix opt-in are asked with the consequences `rk method model` and `rk method worktrees` state, before the plan is approved. The gate job and the required check are step 3c's decision. A `package-check` limitation is read out, not treated as done; `rk method setup` owns why and `rk binding <tech>` names the inspection.
- `brownfield`, from `rk assess`: `rk method migration` and `rk guide migration`. The stage's collision, retired, and omitted lines are the inventory's first entries, one per finding with its disposition, and that runbook's step 1 reports the rest. The landing is its step 2, taken through `rk guide landing`, and the migration runbook owns everything around it. Before approval, state that the plan can need the gated steps below, each a `rk setup step` outside the landing.
- `needs-decision`, from `rk assess`: stop and ask. The operator says what the tags or the second branch are before any plan claims to know, and `rk method migration` owns the verdict.
- `recorded`, from `rk status`: `rk method landing` alone, through `rk upgrade`. A preview that prints only `matched` and `preserved` words means the target is at this release: stop and say so. The changelog and the guidance in the stage's `reference/` are the operator's reading for the releases between.
- `newer`, a record whose `rk_version` is above the binary's: stop. `rk upgrade` refuses and names the version to install, and installing it is the operator's move.

## What the evidence allows

State one class from `rk method landing` before the first edit, and let it bound what the plan can propose.

- Receipt and useful history: propose bringing each collision to the candidate, with its author and reason quoted from `git log`.
- Receipt without useful history: propose by kind and digest, and show every edit since the record as a question.
- History without receipt: propose only what the log attributes, and ask about every file it does not.
- Neither: a best-effort report. Every uncertain ownership decision is a question with `AskUserQuestion`, and no file is brought to the candidate without an answer.

Under every class: never copy the stage tree into the target, and never offer `stage.json` or the stage path to `rk init`, `rk upgrade`, or `rk adopt`. Candidate documentation the stage lists under `artifacts/` is an ordinary destination. The `reference/` tree teaches this skill and is copied into no project.

## Legacy operation state, from a 0.4.x release

- Before the update, with the installed binary: finish every active stored operation, or record the operator's decision to abandon it, per `rk guide landing` step 1b. Abandonment is that decision and no command. An active operation stops the update.
- Record every inactive plan, result, run journal, and release cache path under the state root as local evidence, with why each is obsolete. Do not describe their contents, and teach the new binary nothing about them.
- After the landing passed, show those exact paths. Remove them only under explicit cleanup authorization, only after confirming no active old operation remains, and only by name, per `rk guide landing` step 5b. No `rk` verb removes them.

## What waits for the operator, under the brownfield arrival

Gate each of these: print the exact command, say what it changes and why, wait, then re-observe before continuing.

- Removing a protection from a live branch, on any forge — `rk guide migration` steps 3a and 5a.
- `rk setup step single-trunk --apply` — that runbook's step 5b; destructive, and its ancestry guard refusing is a stop, not an obstacle.
- `install-bot` and `bot-secrets` — the bot identity and its credentials; `rk forge <name>` carries the walkthrough.
- Registry actions: the first hand publish, registering the trusted publisher, turning on enforcement. `rk guide setup` names each with its reason.
- The release style, on a record that predates it and whose config leaves it unanswered: `rk upgrade` refuses until `--style` answers it. Ask it the way the greenfield arrival states, because arming an existing project's release request changes what a green trunk does.
- The development environment, where the project obtains `rk` by a host install or a hand-rolled bump — `rk guide migration` step 6, with the replacement as the default; the migration is not done while the cleanup's `leftovers` list is non-empty.
- Regenerating what a landed configuration generates — `rk guide landing` step 4b names the order, and `rk guide setup` step 4b carries the command.
- The predecessor's removal itself, a retired destination's removal, and every other removal: what it removes is committed first, per the migration chapter's recoverability section.
- A workspace promotion under worktree mode: state the layout from `rk method worktrees`, render the commands from `rk guide worktree` step 5, and run none of them.

## Defaults

- Never run the setup steps out of order; each one names what the next depends on.
- Never author a tag, and never edit a generated artifact workflow by hand; change its configuration and regenerate, as the binding directs.
- Never propose a second request-reporting workflow as a gated check; `needs` resolves inside one file, so the gate reaches a job only in its own.
- Never answer provenance with a signing scheme of your own; take what the channel offers by default, and where it offers nothing, say so.
- Prefer an rk verb over a raw forge call; where no verb covers the gap, use the forge CLI and say that the loop is outside the convention there.
- Report every gated step's outcome from observation, never from the operator's word alone.
- Leave the repository releasable at every stop: a landing interrupted between steps must break no existing flow.
- Never widen an approved scope inline: a mode or style change, a second branch, a workspace move is its own plan, approved at its own size.
- A gap the stage reports that the inventory does not name returns to planning before anything runs.
- A refusal from `rk init`, `rk upgrade`, or `rk adopt` is a finding for the investigation, never a flag to add.
