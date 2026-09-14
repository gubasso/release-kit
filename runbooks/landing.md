# Landing runbook

The steps of [landing](../method/13-landing.md) as commands, in the chapter's order: the chapter owns each step's why, this page owns its how. `rk guide landing` prints this page and fills in the project path, forge, and technology where detection resolves them. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

Every stage is kept from step 1 through step 4. Nothing below copies a byte out of a stage into the target, and no command below takes a stage as input. Where a step below names `<stage>`, that is a path `rk stage` printed. Keep an inventory of every such path the task creates, because step 5 cleans each one by name.

## Prerequisites

- `rk` on `PATH`: `rk --version` answers, and the version is the one the operator chose to install.
- The target is a repository: `git rev-parse --show-toplevel` prints its root, and `git status --porcelain` is empty. The landing writes files Git must be able to show as one diff.
- The landing state is known: `rk status --target .` reports a record or reports none.
- A migration adds one: the findings inventory [the migration runbook](./migration.md) shapes is written and approved before the first `--apply` below.

## 1. Stage

### 1a. Verify the installed version and how the project obtains it

```bash
rk --version
# check: prints the version the operator installed. A version nobody chose is a stop, and the request decides whether an update comes first
rk self-depend status --target .
# check: names the manager that pins rk, or no manager. A host install takes its updates through the host's own path
rk status --target .
# check: none, or the record with its rk_version. A record newer than this binary is a stop that names the version to install
```

- the request did not authorize an update and the target sits behind the release it wants: report the installed state and stop here. Choosing, installing, or fetching another version is the operator's move through the project's manager.
- the record predates 0.5.0 and the operator will update from a 0.4.x binary: take 1b before the update, with that binary.

### 1b. Close legacy operation state before the update

Only where the installed binary is a 0.4.x release. That binary is the only one that reads its own stored operations, so this substep runs before the tool is replaced. The state root is `${XDG_STATE_HOME:-$HOME/.local/state}/release-kit`, the same derivation `rk` uses, and `rk doctor` prints it in its `state-root` probe.

```bash
rk reconcile list
# check: every stored plan, by id. A plan the operator still means to apply is active: apply it with this binary, or wait until the operator does
# a plan the operator declines to apply: that decision reclassifies it as inactive. No command abandons a plan, and none is needed
rk runs list
# check: every run journal, by id. Each is inactive history, because a journal records a run that already ended
rk doctor
# check: the state-root probe prints the root every path below hangs from
```

Derive each inactive path from the listings, one class at a time, and write it down with why it is obsolete.

- a stored plan: `<state root>/plans/<plan-id>/`, one directory per id `rk reconcile list` printed. Obsolete because the next binary stores no plan.
- a run journal: `<state root>/runs/<run-id>/`, one directory per id `rk runs list` printed. Obsolete where the operator keeps no audit interest in it.
- the release cache: `<state root>/release/`, listed directly, because no verb lists it. Obsolete because the next binary fetches no release.

Remove nothing here. Step 5b owns the removal, and only under explicit cleanup authorization.

### 1c. Write the candidate

```bash
rk stage --target .
# check: prints stage: <stage>, the parameters, the landing record's schema, one candidate line per destination, and the omitted, collision, retired, seeded present, and state present lines
# a first landing: pass --tech <tech>, and --workflow, --style, and --nix where the request answers them, so the stage renders the candidate the landing will render
# an exact directory wanted: pass --output <dir>. It must be absent or empty, and RK_STAGE_ROOT is the base otherwise
# exit 73 naming an existing nonempty output: choose another directory, or clean the old stage through step 5a first
```

Add `<stage>` to the inventory of stage paths, and hold it for every step below.

## 2. Investigate

### 2a. Read the stage

```bash
cat <stage>/stage.json
# check: schema is rk.stage/1, rk_version is the binary's, and target is this repository
# check: each candidates entry carries destination, kind, placement, sha256, sources, and region_sha256 where the placement is region
# check: each omissions and collisions entry carries destination and reason, and each retired entry is a destination alone
ls -R <stage>/artifacts <stage>/reference
# check: artifacts/ holds each candidate at its target-relative path. reference/ holds CHANGELOG.md, guidance/, method/, bindings/, runbooks/, forges/, skills/rk-setup, and skill-shared/
```

### 2b. Compare the candidate with the working tree

```bash
for f in $(cd <stage>/artifacts && find . -type f | sed 's|^\./||'); do diff -uN -- "$f" "<stage>/artifacts/$f"; done
# check: an absent file prints the whole candidate, because -N reads it as empty. A differing file prints the edit the landing will replace or refuse. No output means the file already holds the candidate's bytes
cat .release-kit/manifest.json
# check: present or absent. Where present, each destination's kind says whether the landing replaces or preserves it
git log --oneline -- <path>
git diff HEAD -- <path>
# check: for each differing destination, who wrote it and why. An edit with no story is shown to the operator as such
```

State the evidence class the chapter names: receipt and history, receipt alone, history alone, or neither. Under history alone or neither, every ownership decision is a question for the operator and no file is brought to the candidate silently.

### 2c. Read the release's knowledge

`<installed>` is the version `rk --version` printed in 1a, and `<recorded>` is the receipt's `rk_version`. The changelog opens each release with a `## [<version>]` heading, newest first.

```bash
awk -v from='## [<installed>]' -v to='## [<recorded>]' 'index($0, from) == 1 {p = 1} index($0, to) == 1 {exit} p' <stage>/reference/CHANGELOG.md
# check: prints the entries from the installed version down to the recorded one, and excludes the recorded release itself. A target with no record reads the whole file
ls <stage>/reference/guidance
# check: one file per release that needs an operator step. Read every file above <recorded> whose destinations field names a path this target has
```

The `destinations` field of each guidance file decides which files this target reads, and the changelog prose decides nothing about that. Read `<stage>/reference/method`, `bindings`, `runbooks`, and `forges` only for the topics the comparison raised. The source at the exact release tag is the last resort, and trunk never stands in for the installed release.

### 2d. Prepare the target

Each edit here stays inside the request's authority, and each is one inventory entry. A collision the stage named is a file the landing refuses: bring it to the candidate's bytes by hand where the operator owns the edit, keep the operator's content in the target's own configuration where a class P key carries it, or record a target already at the projection through `rk adopt` in step 3. Copy nothing out of `<stage>/artifacts` wholesale, and offer `stage.json` to no verb.

```bash
git status --porcelain
# check: every preparation edit is visible, and nothing here is committed on the trunk
```

- a retired destination: it stays on disk and leaves the receipt at step 3. Removing it is the operator's decision, recorded in the inventory.
- a hook already doing one of the block's jobs: [the setup runbook](./setup.md) step 4d owns that choice.
- a recorded generated file the target edited: the landing replaces it and prints `replaced`. Show the operator the edit that will go, and move its intent into the target's own configuration or a class P key before step 3.

## 3. Land

The production verb renders again from the binary and the target. It never opens `<stage>`.

```bash
rk init --tech <tech> --target .                  # a target with no record and no release mechanism
rk upgrade --target .                             # a recorded target
# check: the preview lists each destination with its word: created, replaced, matched, preserved, drift, or released. A collision line names a file step 2d still owes
rk init --tech <tech> --target . --apply          # or rk upgrade --target . --apply
# check: the same words, then added or updated .release-kit/config.toml, then wrote or rewrote .release-kit/manifest.json last. Every sentinel left to fill is listed
rk adopt --target . --workflow <mode> --style <style>   # a target already at the candidate, with no record
# check: matches for each rendered and seeded destination that holds the candidate's bytes, differs for a seeded file the target tuned, and state for a state file. No sentinel is listed, because adoption writes no payload file
rk adopt --target . --workflow <mode> --style <style> --apply
# check: wrote .release-kit/manifest.json and added .release-kit/config.toml, and nothing else. Every payload destination is unchanged
```

- exit 73 naming collisions: nothing was written. Return to step 2d with the named files.
- exit 73 from `rk adopt` naming a destination as `expected and missing` or differing: the target is not at the candidate. Return to step 2d, or land through `rk init` where the target holds no release mechanism.
- exit 73 naming the style: a pre-style record. Pass `--style <style>`, asked of the operator where the config is silent.
- exit 73 naming a newer `rk_version`: the target is ahead of this binary, and the operator installs the named version.
- exit 74 naming completed paths: a rename stopped part way. Whole files stand beside the previous receipt. Read `git status`, then run the same command again.

## 4. Verify

### 4a. Compare the real diff with the staged benchmark

```bash
git status --porcelain
git diff --stat
# check: the changed paths are the created and replaced destinations step 3 printed, plus .release-kit/config.toml and .release-kit/manifest.json
for f in $(cd <stage>/artifacts && find . -type f | sed 's|^\./||'); do diff -q -- "$f" "<stage>/artifacts/$f"; done
# check: silent for every created and replaced destination. A preserved or drift file can differ, because the landing kept the target's bytes
```

- a created or replaced destination differs from its artifact: a preparation edit or a parameter changed between the stage and the landing. Inspect the difference. Where the benchmark itself must be refreshed, stage again with `rk stage --target . --output <dir>`, and add that path to the inventory of stage paths too. Every path in the inventory is cleaned in step 5a.

### 4b. Run the checks

```bash
grep -rn 'TODO(release-kit)' . --exclude-dir=.git
# check: prints nothing once each sentinel is answered
rk status --check --target .
# check: exits 0. A drift, missing, sentinel, or invariant line is the next finding
rk setup check --target .
# check: what the forge enforces still matches the landed files. A fault here is a finding, and the setup runbook carries its step
```

Then run the project's own checks. A landed configuration another tool generates from is regenerated first, per [the setup runbook](./setup.md) step 4b.

- a check fails: repair the finding in the target, then run the production verb of step 3 again. Production renders afresh every time, so a repair is never a hand copy from the stage.

## 5. Clean

### 5a. Remove every stage

Only after step 4 passed in full, and only where the request's authority includes cleanup. Until then each stage is recoverable evidence. Run the command once per path in the inventory of stage paths, the one `rk stage` printed in 1c and every one 4a added.

```bash
rk stage clean <stage>
# check: prints removed <stage> and the recovery command that stages the candidate again. Repeat for the next path in the inventory until none is left
# exit 73 naming the path: it is a symlink, a repository root, a home or filesystem root, a directory without stage.json, or a receipt whose stage_root differs from the argument. Nothing was removed
```

### 5b. Remove inactive legacy state

Only where 1b recorded paths, only under explicit cleanup authorization, and only after 1b confirmed that no active old operation remains. No `rk` verb removes this state and no verb takes a directory as a recursive target, so each path is removed by hand and by name. `<root>` is the state root 1b derived, and `<path>` is one path 1b recorded.

```bash
root="${XDG_STATE_HOME:-$HOME/.local/state}/release-kit"
resolved="$(realpath -e -- "<path>")"
case "$resolved" in "$root"/plans/*|"$root"/runs/*|"$root"/release) ;; *) echo "outside the legacy classes: $resolved" >&2; false;; esac
# check: prints nothing. A path that resolves outside the three classes under the state root is not legacy state, and nothing below runs for it
find "$resolved" -mindepth 1 -xdev -print
# check: the complete list of entries the removal will delete, with no link followed. Review every line with the operator, and stop where one is not the stored plan, run journal, or cached release 1b recorded
find "$resolved" -mindepth 1 -xdev -depth -delete && rmdir -- "$resolved"
# check: the same list is gone and the directory with it. The -delete consumes exactly the entries the review printed, and the directory goes last
```

Repeat for each recorded path, one at a time. Then commit the landed files, the receipt included, through the trunk's one path. The stage and the legacy state are not among them.
