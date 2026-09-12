# Reconcile runbook

The steps of [reconcile](../method/12-reconcile.md) as commands, in the chapter's order: the chapter owns each step's why, this page owns its how. `rk guide reconcile` prints this page and `rk reconcile` is the engine. The two live in different namespaces, and a reader meeting both in one session reads the guide and runs the engine. `rk guide reconcile` fills in the project path, forge, and technology where detection resolves them. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

A plan holds bytes from the target, some of them not public. It is ephemeral: it lives under the state root, it is never committed, and it is never pasted into an issue, a request, or a review. Quote a plan's id, its classification, its readiness, and its operation lines. Quote nothing else from it.

## Prerequisites

`rk` on `PATH`, a clean working tree, and the target's landing state known: `rk status --target .` reports a record or reports none. A request that names a release other than this binary's needs the network once, for the bundle. A request that names the forge needs the forge remote reachable.

## 1. Observe

[The migration runbook](./migration.md) step 1 carries the observation list, unchanged. The plan cites what it observed, so nothing observed there is read twice.

```bash
rk status --target .
# check: the record's version and every drift, missing, sentinel, and pending line; none is a fault yet
rk self-depend status --target .
# check: the manager that names release-kit and its pin, or no manager; step 8 reads the same report
```

## 2. Request a plan

```bash
rk reconcile plan --target .
# check: prints the plan id, the classification, the readiness, the operations by kind, every precondition that is not satisfied, and the fingerprint, and reports the plan as stored
rk reconcile plan --target . --to <version>
# check: the same shape toward that release, through the registry once; a second request reads the cache
rk reconcile plan --target . --observe forge
# check: the forge-observed precondition reads satisfied and cites the forge
```

- `--to` names a release whose payload schema is newer than this binary's: the plan is blocked naming the engine to install. Install it, then return to this step.
- the recorded release's bundle is not in the cache: the baseline reads not observed with its reason. `--fetch` reads it through the registry, and without it the `partial-baseline` decision in step 4 is the operator's.

## 3. Read the classification and the readiness

```bash
rk reconcile show <plan-id>
# check: the classification is setup, migration, upgrade, drift, or invalid, and the readiness is ready, needs-decision, or blocked
```

- `setup`: continue here; [the setup runbook](./setup.md) step 4 owns what surrounds the landing.
- `migration`: [the migration runbook](./migration.md) owns the inventory and the gated steps around this landing. Return here for steps 4 to 7.
- `upgrade` with no operation: the target is at this release. Stop.
- `drift`: go to step 5 before anything else.
- `invalid`: the record cannot be read by this engine. A record newer than the engine names the engine to install. Any other invalid record is a finding for the operator, and nothing below runs.
- `blocked`: the report names each unsatisfied required precondition. Resolve it, then return to step 2 for a fresh plan.

## 4. Resolve each decision

The plan states each decision with its consequences before it asks. Read them from the `show` output, then answer by id in a fresh request. A decision the request already answered, such as `--workflow` or `--style` on a first landing, is not asked again.

```bash
rk reconcile plan --target . --decide <id>=<answer>
# check: the readiness is ready, the decision line is gone, and the fingerprint changed
```

- `workflow-mode=<worktree|branches>`: the working-copy mode on a first landing with no configuration. [Worktrees](../method/08-worktrees.md) owns the choice.
- `release-style=<trunk|lines>`: the release style on a record that predates it. [Release lines](../method/09-release-lines.md) owns the choice.
- `partial-guidance=accept`: the bundle describes every release above one version and the record is below it. Accept, and read `CHANGELOG.md` for the releases between.
- `partial-baseline=<accept|fetch>`: the recorded release's bundle could not be read, so a seeded file's baseline is unknown. `accept` only where step 1 showed no tuned seeded file. `fetch` reads the bundle through the registry, the same as `--fetch` on step 2.
- `pin-manager=<wire|host>`: a manager file is present and names no release-kit. `wire` adds the pin through step 8, and `host` keeps the host install.
- `release-activity=<history|migrate>`: tags or a long-lived branch no mechanism explains. `history` says the activity is past and the landing is a setup. `migrate` says a mechanism is live, and [the migration runbook](./migration.md) owns what follows.
- more than one decision: pass `--decide` once per decision in one request. Each request stores a new plan, and the last id is the one step 6 takes.

## 5. Reconcile what the plan refuses

### 5a. An owned file the target edited

The plan is blocked on `owned-file-unedited:<path>`, and its evidence carries the three digests: the record's, the destination's, and the baseline's. The edit is the target's to move, never the plan's to keep.

```bash
git diff -- <path>
# check: the edit, where it is uncommitted; a committed edit shows against the record's digest in the plan's destinations list
rk snippet <tech>/<forge>/<path>
# check: the candidate's bytes; what the edit wanted goes into the target's own configuration, a class P key, or a change to release-kit
git checkout -- <path>                    # or restore the recorded bytes by hand
rk reconcile plan --target .
# check: the classification is upgrade and the precondition is gone
```

### 5b. A seeded file the target tuned

A seeded file is the target's after the first landing. The plan keeps it and moves its baseline in the record, and the `show` output lists no operation for it.

```bash
rk reconcile show <plan-id>
# check: the tuned file appears in no write-file line; the write-record line carries the moved baseline
```

- the file appears as a write-file: its recorded kind is rendered, not seeded, and 5a applies.

### 5c. A generated artifact behind its configuration

The plan writes a configuration a tool generates from, and the `generator-at-pin` precondition names the generator and its pin. The regenerate is the operator's step after the apply, and [the setup runbook](./setup.md) step 4b carries it.

```bash
rk reconcile show <plan-id>
# check: generator-at-pin is satisfied, or advisory where no generated artifact is in the operations; a required unsatisfied line names the generator to install at its pin
```

## 6. Apply

```bash
rk reconcile apply <plan-id>
# check: prints each operation as applied, each postcondition with its outcome, and the journal's run id
```

- `no longer matches its inputs`, exit 73: the record, a destination, the bundle, or a decision moved since the plan. Nothing was written. Return to step 2.
- `waits on a decision`, exit 73: return to step 4 with the ids named.
- `is blocked`, exit 73: return to step 3.
- exit 74 naming a destination: a rename stopped part way. Every destination holds its previous bytes or its new ones, the record was not written, and `rk runs show <id>` names which landed. Return to step 2.
- `postcondition-failed`, exit 1: the writes landed and a check after them did not hold. Read step 7 before anything else.

## 7. Read the result

```bash
rk runs show <run-id>
# check: the plan id, the fingerprint, and every operation's outcome
rk status --target .
# check: the record names the release, no drift line, and every sentinel the landing left to answer
grep -rn 'TODO(release-kit)' . --exclude-dir=.git
# check: prints nothing once each sentinel is answered
rk status --check --target .
# check: exits 0; the status-check-clean postcondition reported the same verdict at apply time and never failed the apply on it
```

Commit the written files, the record included, through the trunk's one path. The plan itself is not among them.

## 8. Move the pin

Only where the target obtains `rk` through a manager, per step 1.

```bash
rk self-depend status --target .
# check: the wired manager's version names the release the plan landed; a one-fact manager moved as an update-pin operation in step 6
rk self-depend sync --target . --apply
# check: on the flake pair, the tag and the lock node moved together and the shell builds; the report names from and to
```

- no manager names release-kit: nothing to move. A host install takes the release through the host's own path.
- the pin moves in its own commit: a `build(deps)` commit before the landing is the recommended policy the chapter names. Nothing refuses the pin and the landing in one commit.
