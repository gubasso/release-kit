# Landing runbook

The steps of [landing](../method/13-landing.md) as commands, in the chapter's order: the chapter owns each step's why, this page owns its how. `rk guide landing` prints this page and fills in the project path, forge, and technology where detection resolves them. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

The stage is kept from step 1 through step 4. Nothing below copies a byte out of it into the target, and no command below takes it as input. Where a step below names `<stage>`, that is the path step 1 printed.

## Prerequisites

- `rk` on `PATH`: `rk --version` answers, and the version is the one the operator chose to install.
- The target is a repository: `git rev-parse --show-toplevel` prints its root, and `git status --porcelain` is empty. The landing writes files Git must be able to show as one diff.
- The landing state is known: `rk status --target .` reports a record or reports none.
- A migration adds one: the findings inventory [the migration runbook](./migration.md) shapes is written and approved before the first `--apply` below.

## 1. Stage

### 1a. Verify the installed version and how the project obtains it

```bash
rk --version
# check: prints the version the operator installed; a version nobody chose is a stop, and the request decides whether an update comes first
rk self-depend status --target .
# check: names the manager that pins rk, or no manager; a host install takes its updates through the host's own path
rk status --target .
# check: none, or the record with its rk_version; a record newer than this binary is a stop that names the version to install
```

- the request did not authorize an update and the target sits behind the release it wants: report the installed state and stop here. Choosing, installing, or fetching another version is the operator's move through the project's manager.
- the record predates 0.5.0 and the operator will update from a 0.4.x binary: take 1b before the update, with that binary.

### 1b. Close legacy operation state before the update

Only where the installed binary is a 0.4.x release. That binary is the only one that reads its own stored operations, so this substep runs before the tool is replaced.

```bash
rk reconcile list
# check: no stored plan waits on an apply; a plan the operator still means to apply is finished with this binary, and every other one is abandoned by name
rk runs list
# check: the run journals, listed for the record
rk doctor
# check: the state-root probe prints the root; the plans, results, run journals, and release caches under it are the legacy state
```

Write each inactive path down as local evidence, with why it is obsolete. Remove nothing here: step 5b owns the removal, and only under explicit cleanup authorization.

### 1c. Write the candidate

```bash
rk stage --target .
# check: prints stage: <stage>, the parameters, the landing record's schema, one candidate line per destination, and the omitted, collision, retired, seeded present, and state present lines
# a first landing: pass --tech <tech>, and --workflow, --style, and --nix where the request answers them, so the stage renders the candidate the landing will render
# an exact directory wanted: pass --output <dir>; it must be absent or empty, and RK_STAGE_ROOT is the base otherwise
# exit 73 naming an existing nonempty output: choose another directory, or clean the old stage through step 5a first
```

Hold `<stage>` for every step below.

## 2. Investigate

### 2a. Read the stage

```bash
cat <stage>/stage.json
# check: rk_version is the binary's, target is this repository, and every candidate, omission, collision, and retired entry has a reason you can restate
ls -R <stage>/artifacts <stage>/reference
# check: artifacts/ holds each candidate at its target-relative path; reference/ holds CHANGELOG.md, guidance/, method/, bindings/, runbooks/, forges/, skills/rk-setup, and skill-shared/
```

### 2b. Compare the candidate with the working tree

```bash
for f in $(cd <stage>/artifacts && find . -type f | sed 's|^\./||'); do diff -u -- "$f" "<stage>/artifacts/$f"; done
# check: an absent file prints the whole candidate; a differing file prints the edit the landing will replace or refuse; no output means the file already holds the candidate's bytes
cat .release-kit/manifest.json
# check: present or absent; where present, each destination's kind says whether the landing replaces or preserves it
git log --oneline -- <path>
git diff HEAD -- <path>
# check: for each differing destination, who wrote it and why; an edit with no story is shown to the operator as such
```

State the evidence class the chapter names: receipt and history, receipt alone, history alone, or neither. Under history alone or neither, every ownership decision is a question for the operator and no file is brought to the candidate silently.

### 2c. Read the release's knowledge

```bash
sed -n '1,80p' <stage>/reference/CHANGELOG.md
# check: the entries between the record's rk_version and this binary's are read; each names a landed destination it touched
ls <stage>/reference/guidance
# check: one file per release that needs an operator step; read every file above the record's rk_version whose destinations this target has
```

Read `<stage>/reference/method`, `bindings`, `runbooks`, and `forges` only for the topics the comparison raised. The source at the exact release tag is the last resort, and trunk never stands in for the installed release.

### 2d. Prepare the target

Each edit here stays inside the request's authority, and each is one inventory entry. A collision the stage named is a file the landing refuses: bring it to the candidate's bytes by hand where the operator owns the edit, keep the operator's content in the target's own configuration where a class P key carries it, or record a target already at the projection through `rk adopt` in step 3. Copy nothing out of `<stage>/artifacts` wholesale, and offer `stage.json` to no verb.

```bash
git status --porcelain
# check: every preparation edit is visible; nothing here is committed on the trunk
```

- a retired destination: it stays on disk and leaves the receipt at step 3. Removing it is the operator's decision, recorded in the inventory.
- a hook already doing one of the block's jobs: [the setup runbook](./setup.md) step 4d owns that choice.

## 3. Land

The production verb renders again from the binary and the target. It never opens `<stage>`.

```bash
rk init --tech <tech> --target .                  # a target with no record and no release mechanism
rk upgrade --target .                             # a recorded target
rk adopt --target . --workflow <mode> --style <style>   # a target already at the candidate, with no record
# check: the preview lists each destination with its word: created, replaced, matched, preserved, drift, or released; a collision line names a file step 2d still owes
rk init --tech <tech> --target . --apply          # or rk upgrade --target . --apply, or rk adopt ... --apply
# check: the same words, then the receipt written last; every sentinel left to fill is listed
```

- exit 73 naming collisions: nothing was written. Return to step 2d with the named files.
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
# check: silent for every created and replaced destination; a preserved or drift file may differ, because the landing kept the target's bytes
```

- a created or replaced destination differs from its artifact: a preparation edit or a parameter changed between the stage and the landing. Inspect the difference, then stage again with `rk stage --target . --output <dir>` where the benchmark itself must be refreshed.

### 4b. Run the checks

```bash
grep -rn 'TODO(release-kit)' . --exclude-dir=.git
# check: prints nothing once each sentinel is answered
rk status --check --target .
# check: exits 0; a drift, missing, sentinel, or invariant line is the next finding
rk setup check --target .
# check: what the forge enforces still matches the landed files; a fault here is a finding, and the setup runbook carries its step
```

Then run the project's own checks. A landed configuration another tool generates from is regenerated first, per [the setup runbook](./setup.md) step 4b.

- a check fails: repair the finding in the target, then run the production verb of step 3 again. Production renders afresh every time, so a repair is never a hand copy from the stage.

## 5. Clean

### 5a. Remove the stage

Only after step 4 passed in full, and only where the request's authority includes cleanup. Until then the stage is recoverable evidence.

```bash
rk stage clean <stage>
# check: prints removed <stage> and the recovery command that stages the candidate again
# exit 73 naming the path: it is a symlink, a repository root, a home or filesystem root, a directory without stage.json, or a receipt whose stage_root differs from the argument; nothing was removed
```

### 5b. Remove inactive legacy state

Only where 1b recorded paths, only under explicit cleanup authorization, and only after 1b confirmed that no active old operation remains. No `rk` verb removes this state and no verb takes a directory as a recursive target, so each path is removed by hand and by name.

```bash
ls -d <path>
# check: the path is one 1b recorded, and nothing else lives under it
rm -r -- <path>
# check: the path is gone; repeat for each recorded path, one at a time
```

Commit the landed files, the receipt included, through the trunk's one path. The stage and the legacy state are not among them.
