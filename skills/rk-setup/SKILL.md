---
name: rk-setup
description: Lands the release-kit workflow in a project through the rk CLI, and keeps a landed project current. Use when asked to set up a release workflow, release automation, trusted publishing, release-plz, release-please, git-cliff, changelog automation, or trunk-based release automation, to migrate a project that already releases through another tool onto release-kit, to upgrade or reconcile a landed payload, or to adapt the release-kit convention to a project's technology. Triggers include vulnerability reporting, SECURITY.md, release-kit, rk init, rk upgrade, rk reconcile, release setup, migrate to release-kit, adopt release-kit, setup drift, and release workflow setup.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Landing files into a target needs write access to that repository. Forge mutations need the forge CLI authenticated with administration rights; the operator supplies credentials and registry actions.
---

# rk-setup

Land the release-kit convention in a project, or take a landed project to the current release. One engine computes every landing: `rk reconcile plan` states what a landing will do, the plan's classification names the chapter that owns the procedure around it, and `rk reconcile apply` executes exactly that plan or refuses. This skill runs the gates, requests the plan, presents it, routes the approved work to apply, and reads the result. `rk method reconcile` owns the why, and `rk guide reconcile` owns the commands by step number.

## Before acting

Read two shared files before the first action of a task, in this order, and hold both for the whole task.

1. `~/.local/state/release-kit/skills/shared/pre-flight-gate.md` — run it whatever the request carries. It checks this host with `rk doctor`, requests the plan, and stops the task on what no plan can work around. No flag skips it.
2. `~/.local/state/release-kit/skills/shared/plan-gate.md` — it binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The two gates are why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, the pre-flight says whether this host can run it at all, and the plan gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the plan gate's approval turn only. Still run the pre-flight, still state the ordered plan before acting, and still validate it as phase 2 directs.

## Route to the canon

| Need                                                            | Command                                         |
| --------------------------------------------------------------- | ----------------------------------------------- |
| Judge this host's readiness                                     | `rk doctor`                                     |
| List method chapters, read one                                  | `rk method --list`, `rk method <chapter>`       |
| List bindings, read one                                         | `rk binding --list`, `rk binding <tech>`        |
| Read a forge's specifics                                        | `rk forge <name>`                               |
| The plan, decide, apply procedure, as commands                  | `rk guide reconcile`                            |
| The setup procedure around a first landing, as commands         | `rk guide setup`                                |
| The migration procedure around a landing, as commands           | `rk guide migration`                            |
| Compute and store the plan                                      | `rk reconcile plan --target .`                  |
| Render a stored plan                                            | `rk reconcile show <plan-id>`                   |
| Execute a stored plan                                           | `rk reconcile apply <plan-id>`                  |
| A landed target's own report                                    | `rk status --target .`                          |
| List the landable files, print one                              | `rk snippet --list`, `rk snippet <tech>/<path>` |
| Print the pinned-tool registry                                  | `rk versions`                                   |
| List the executable setup steps                                 | `rk setup --list`                               |
| Judge a message against the content guards                      | `rk message --check`                            |
| The merged branches this clone still holds                      | `rk branches prune`                             |
| The worktree lifecycle, as commands                             | `rk guide worktree`                             |
| Start work an issue names, and its procedure                    | `rk issue start <issue>`, `rk guide issue`      |
| The release line's whole life, and its verbs                    | `rk guide release-lines`, `rk lines --help`     |
| The working-copy forms and the mode                             | `rk method worktrees`                           |
| How this project obtains `rk`, per manager                      | `rk self-depend status`                         |
| The fragments for one manager and venue pair, and the seed file | `rk self-depend add`                            |
| The predecessor bump mechanism's removal                        | `rk self-depend clean`                          |
| The pin moved to the latest release                             | `rk self-depend sync`                           |
| Another project as a dependency                                 | `rk depend assess`                              |

## Installation scope

Skills and the agent setup install at user scope only — `rk skill install --apply`, once per user; no system mode exists, by decision. The setup runbook's prerequisites own the step and the two roots. Where a file lands for a third-party application is the target project's own decision, made against that application's documentation with a dated citation — never generalized from another application.

## Start work an issue names

A request naming an issue — an issue URL, or "fix", "address", "implement", or "work on" plus an issue — starts from `rk issue start <issue>`, and the plan's first step is that command. The forge names the branch and rk seats it the way the project's recorded workflow mode says. Three rules bind this.

- Never write a predicted branch name into the plan. The name is whatever the forge mints, and stating a guess is the mistake this verb exists to prevent. Write "the branch the forge mints for issue <n>".
- The preview is what the plan presents for approval, and the apply is what execution runs. That is the plan gate's own two-phase shape, so no new gate appears here.
- Minting a branch at the forge is a forge action, so it happens only where the operator's request named starting work on that issue.

`rk guide issue` renders the whole procedure, and `rk method model` owns why the name comes from the forge.

## The five steps

1. Run the gates. The pre-flight gate's closing line requests the first plan; hold its id, its classification, and its readiness.
2. Request the plan for the release the request names: `rk reconcile plan --target . --json`, with `--to <version>` where the request names a release and `--observe forge` where a forge fact is in question. Pass every answer the request already carries, `--workflow`, `--style`, `--nix`, so the plan asks nothing twice. `rk guide reconcile` step 2 carries the flags and what each divergence means.
3. Present it. The classification names the chapter to load, per the section below, and the readiness names what stands between the plan and the apply. The typed change summary is the `operations` list by kind and count. Each entry of the `decisions` list is shown with its consequences, and `AskUserQuestion` asks only what the plan names. A `blocked` plan is presented as blocked with each unsatisfied required precondition, and the task stops there; `rk guide reconcile` steps 3 and 5 name each resolution. A plan holds bytes from the target: quote its id, classification, readiness, and operation lines, and nothing else, as that runbook's header states.
4. Route approved work to `rk reconcile apply <plan-id>`. An answered decision is selected by re-planning with `--decide <id>=<answer>`, `rk guide reconcile` step 4, and the new id is the one apply takes. A request the operator already authorized is not asked again. Every refusal at apply is that runbook's step 6, and none is a flag to add.
5. Read the result from observation, `rk guide reconcile` step 7: the run's journal entry, the status report, the sentinels, then `rk status --check --target .`. Move the pin as step 8 directs. Report what each command returned, never the operator's word.

## Which chapter the classification loads

- `setup`: `rk method setup` and `rk guide setup`. The plan is that runbook's step 4, the landing; steps 1 to 3 surround it on the forge side and steps 5 to 8 on the registry side, and the chapter owns each step's why. The decisions a first landing names, the workflow mode, the release style, and the Nix opt-in, are asked with the consequences `rk method model` and `rk method worktrees` state, before the plan is approved. The development environment follows `rk guide setup` step 6. The hooks question is step 4 of that runbook. The gate job and the required check are step 3c's decision: propose the gate that step shows and take the operator's answer for each job in its list. A `package-check` limitation is read out, not treated as done; `rk method setup` owns why and `rk binding <tech>` names the inspection.
- `migration`: `rk method migration` and `rk guide migration`. The plan's findings are the inventory's first entries, one per finding with its disposition, and that runbook's step 1 reports the rest. The landing is its step 2, taken through `rk guide reconcile` steps 4 to 7, and the migration runbook owns everything around it. Before approval, state that the plan may need the gated steps below, each a `rk setup step` outside the plan's operations.
- `upgrade`: `rk method reconcile` alone. No operation means the target is at this release: stop and say so. The plan's guidance steps are the operator's reading for the releases between; `partial-guidance` is a decision, and `CHANGELOG.md` is the reading behind it.
- `drift`: `rk method reconcile`. The plan is blocked until the owned file is reconciled, `rk guide reconcile` step 5a; never widen the plan to keep the edit.
- `invalid`: stop. A record newer than the engine names the engine to install; any other invalid record is a finding for the operator.

## What waits for the operator, under the migration classification

Gate each of these: print the exact command, say what it changes and why, wait, then re-observe before continuing.

- Removing a protection from a live branch, on any forge — `rk guide migration` steps 3a and 5a.
- `rk setup step single-trunk --apply` — that runbook's step 5b; destructive, and its ancestry guard refusing is a stop, not an obstacle.
- `install-bot` and `bot-secrets` — the bot identity and its credentials; `rk forge <name>` carries the walkthrough.
- Registry actions: the first hand publish, registering the trusted publisher, turning on enforcement. `rk guide setup` names each with its reason.
- The release style, on a record that predates it and whose config leaves it unanswered — the plan names it as the `release-style` decision; ask it the way the setup classification states, because arming an existing project's release request changes what a green trunk does.
- The development environment, where the project obtains `rk` by a host install or a hand-rolled bump — `rk guide migration` step 6, with the replacement as the default; the migration is not done while the cleanup's `leftovers` list is non-empty.
- Regenerating what a landed configuration generates — `rk guide migration` step 2e; the plan's `generator-at-pin` precondition names the generator and its pin.
- The predecessor's removal itself, and every other removal: what it removes is committed first, per the chapter's recoverability section.
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
- A gap the plan reports that the inventory does not name returns to planning before anything runs.
- A refusal from `rk reconcile apply` is a fresh plan to read, never a flag to add.
