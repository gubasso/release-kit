# Migration runbook

The steps of [migration](../method/10-migration.md) as commands, in the chapter's order: the chapter owns each step's why, this page owns its how. Steps 1 to 5 run on a branch the operator owns, and step 6 runs after that branch merges. Where `rk setup` executes a step, the `Automated:` line names the verb, and [the setup runbook](./setup.md) carries its check; nothing there is restated here. `rk guide migration` fills in the project path, forge, and technology where detection resolves them. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## Prerequisites

The setup runbook's prerequisites, unchanged: `rk` on `PATH`, this binary's skills installed at user scope, the forge CLI authenticated, and a clean working tree. A migration adds two. The target classifies as brownfield, and the work runs on the operator's own branch, so the trunk keeps releasing through the predecessor until that branch merges.

```bash
rk assess --target .
# check: brownfield; greenfield takes rk guide setup instead, and needs-decision is the operator's answer before anything runs
rk status --target .
# check: no landing record; a recorded target routes by this report to rk guide landing instead
```

## 1. Inventory

```bash
rk migrate inventory --target .
# check: every predecessor file with its tool and kind, and the root files listed as in place; a predecessor file it does not list is a finding for the operator before the archive
```

## 2. Archive

### 2a. Land every listed file on the trunk

An edit to a predecessor file integrates onto the trunk before the archive, through the project's own path, and the local trunk branch is brought up to date.

```bash
git status --porcelain
# check: empty
git diff --name-only master
# check: names no file the inventory listed; the archive refuses a listed file whose bytes differ from its copy on the trunk, because the trunk must hold every byte it moves
```

### 2b. Open the migration

Gated: the operator runs this. The forge half needs the forge CLI authenticated; where it is not, the forge snapshots read unknown and are taken in step 3b.

```bash
rk migrate archive --target .
# check: the preview names every file it moves and the workspace it opens, and writes nothing
rk migrate archive --target . --apply
# check: the predecessor files left the target, .release-kit/config.toml names the migration, and each forge row reads taken or unknown
rk status --target .
# check: classification: migration
```

- a refusal naming files that differ from the trunk's copy: integrate their edits onto the trunk, update the local trunk branch, then run the archive again from the migration branch.

### 2c. Rebuild the archived files on another clone

Only where this host holds no workspace for the open migration, and the archive's configuration change is committed.

```bash
rk migrate archive --rebuild --target .
# check: would rebuild names every archived file, and nothing is written
rk migrate archive --rebuild --apply --target .
# check: rebuilt names every archived file, recreated from Git at the base commit, the registry's or the parent of the commit that recorded the key, which a squash integration keeps; a snapshot has no second copy and is not rebuilt
```

## 3. Land

### 3a. Land the greenfield instance

The landing is the whole landing procedure: [the landing runbook](./landing.md) steps 1 to 5, with `rk init` as the front in its step 4. The archive left no collision, so no `--replace` passes it. A predecessor bump mechanism is removed first through `rk self-depend clean`, as [the setup runbook](./setup.md)'s prerequisites carry it (check: `rk status --target .` reports the landing, and still `classification: migration`).

### 3b. Take the snapshots the landing adds

The landed profile can run destructive steps the unlanded target did not.

```bash
rk migrate status --target .
# check: no line names a missing forge snapshot
rk migrate archive --target . --apply
# check: each forge row reads taken; run it on a host that holds the forge credentials where a row reads unknown
```

## 4. Map

The agent reads one predecessor tool's sources together, from the workspace's `sources/`, and proposes a fate for each. The operator approves the map before anything is marked.

```bash
rk migrate mark <source> mapped --group <group> --target .
rk migrate mark <source> dropped --reason "<why>" --target .
rk migrate status --target .
# check: every source is mapped or dropped, and every mapped source names its group
```

## 5. Loop

One group at a time: write what the group carries into its destinations, then read each destination against its sources.

```bash
rk migrate mark <group> written --destination <path> --target .
rk migrate mark <group> verified --target .
rk migrate status --target .
# check: the group's sources read verified, and their checksums read same
```

Then the branch merges inside the request's authority (check: `rk status --target .` on the trunk reports the landing).

## 6. Forge

After the branch merges. Automated, in the order of [the setup runbook](./setup.md) steps 1 and 3: `rk setup --target . --apply`. Each destructive step uses the before-state the migration holds, and refuses where the registry holds none. Where the old protection on a retired branch keeps `single-trunk` from deleting it, its removal is the operator's, gated, and the migration's forge snapshot holds what it removed.

```bash
rk setup check --target .
# check: every step reports satisfied, or the limitation the forge document names
rk setup checkpoint --target .
# check: wrote .release-kit/setup-proof.json, from a host that holds the bot's key
```

Then publish through [the setup runbook](./setup.md) steps 5 to 7 where the setup proof is not current.

## 7. Close

```bash
rk migrate close --target .
# check: the migration can close; a refusal lists every unfinished source and a setup proof that is not current
rk migrate close --target . --apply
# check: .release-kit/config.toml names no migration, and rk status no longer reads migration
rk guide release
# check: one release cut end to end, with its verify step passing, is the migration's proof
```

The workspace stays. Removing it is the operator's call, once the first release proved the new path.
