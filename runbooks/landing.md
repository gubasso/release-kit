# Landing runbook

The steps of [landing](../method/13-landing.md) as commands, in the chapter's order: the chapter owns each step's why, this page owns its how. `rk guide landing` prints this page and fills in the project path, forge, and technology where detection resolves them. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

Every stage is kept from step 1 through step 5 and the checks after it. Nothing below copies a byte out of a stage into the target: the production verb's `--stage` reads the stage's parameters alone. Where a step below names `<stage>`, that is a path `rk stage` printed. Keep an inventory of every such path the task creates, because the close after step 5 cleans each one by name.

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
# a first landing: pass --technology, --release-mode, and the checkout-mode, release-style, merge-form, and capability flags the request answers, so the stage renders the candidate the landing will render
# the Git workflow with no release: pass --release-mode none --integration local --checkout-mode linked-worktree --workflow-guards, the four answers rk init --workflow-only sets (4a)
# an exact directory wanted: pass --output <dir>. It must be absent or empty, and RK_STAGE_ROOT is the base otherwise
# exit 73 naming an existing nonempty output: choose another directory, or clean the old stage through the close first
```

Add `<stage>` to the inventory of stage paths, and hold it for every step below.

## 2. Compare

### 2a. Read the stage

```bash
cat <stage>/stage.json
# check: schema is rk.stage/5, rk_version is the binary's, and target is this repository
# check: each candidates entry carries destination, kind, placement, sha256, sources, and region_sha256 where the placement is region
# check: each omissions and collisions entry carries destination and reason, and each retired entry is a destination alone
ls -R <stage>/artifacts <stage>/reference
# check: artifacts/ holds each candidate at its target-relative path. reference/ holds CHANGELOG.md, guidance/, method/, bindings/, runbooks/, forges/, and skills/ with one package per skill
rk profile --target .
# check: every domain value with its source, each capability with its status, and the destinations the selected ones land
rk binding <tech>
# check: the release driver's own four axes and the registry facts the candidate rests on
```

### 2b. Compare the candidate with the target

```bash
rk stage compare <stage> --target .
# check: one row per destination the stage carries or the record names, each with its state (equal, absent, differs, retired), its kind (rendered, region, seeded, state), its provenance where it differs, and the word the landing will print
# check: history names whether Git answered. Where it did not, every differing row reads reduced or unknown, and the report says why
git log --oneline -- <path>
git diff HEAD -- <path>
# check: for each row that reads collision, who wrote the file and why. An edit with no story is shown to the operator as such
```

Judge only a row whose word is `collision`. Where its `replaceable` is true, ask the operator about it: keep the edit, which saves a copy and then replaces the file, or discard it, which replaces it. Where it is false, the landing refuses the path whatever is named, and the operator fixes it by hand first. Provenance explains a row and never makes it a conflict. A row with no receipt entry says whether its history shows an edit after the commit that brought it: the group of untouched files takes one answer, and each edited file takes its own.

### 2c. Read the release's knowledge

The changelog opens each release with a `## [<version>]` heading, newest first. `rk --version` prints `rk <version>`, so the bare version is cut from it, and the recorded version is read from the receipt's one `rk_version` line. Nothing here needs more than `sh`, `sed`, `awk`, and `rk`.

```bash
installed="$(rk --version | sed 's/^rk //')"
recorded="$(sed -n 's/.*"rk_version": *"\([^"]*\)".*/\1/p' .release-kit/manifest.json)"
awk -v from="## [$installed]" -v to="## [$recorded]" 'index($0, from) == 1 {p = 1} index($0, to) == 1 {exit} p' <stage>/reference/CHANGELOG.md
# check: prints the entries from the installed version down to the recorded one, and excludes the recorded release itself
cat <stage>/reference/CHANGELOG.md
# a target with no record: this line replaces the two above, because every release is between nothing and the installed one
ls <stage>/reference/guidance
# check: one file per release that needs an operator step. Read every file above the recorded version whose destinations field names a path this target has
```

The `destinations` field of each guidance file decides which files this target reads, and the changelog prose decides nothing about that. Read `<stage>/reference/method`, `bindings`, `runbooks`, and `forges` only for the topics the comparison raised. The source at the exact release tag is the last resort, and trunk never stands in for the installed release.

## 3. Prepare

Each edit here stays inside the request's authority, and each is one inventory entry. Gather the operator's answer for every conflict step 2b raised, and the task's own items. Keep the operator's content in the target's own configuration where a class P key carries it. Copy nothing out of `<stage>/artifacts`, and offer `stage.json` to no verb but through `--stage`.

```bash
git status --porcelain
# check: every preparation edit is visible, and nothing here is committed on the trunk
```

- a conflict the operator keeps or discards: name it with `--replace <path>` in step 4. The landing saves its bytes first, then replaces it, and the report names where the copy went.
- a retired destination: it stays on disk and leaves the receipt at step 4. Removing it is the operator's decision, recorded in the inventory.
- a hook already doing one of the block's jobs: [the setup runbook](./setup.md) step 4d owns that choice.

## 4. Land

The production verb renders again from the binary and the target, with the answers `--stage` reads from the stage's receipt. It reads no byte of `<stage>`.

```bash
rk init --stage <stage> --target .                # a target with no record and no release mechanism
rk upgrade --stage <stage> --target .             # a recorded target
# check: the preview lists each destination with its word: created, replaced, matched, preserved, drift, or released. A collision line names a conflict step 3 has not answered
rk init --stage <stage> --target . --apply        # or rk upgrade, adding --replace <path> for each answered conflict
# check: the same words, then added or updated .release-kit/config.toml, then wrote or rewrote .release-kit/manifest.json last. Every replaced conflict prints where its bytes were saved, and every sentinel left to fill is listed
rk adopt --stage <stage> --target .               # a target landed before the record existed
# check: matches for each rendered and seeded destination that holds the candidate's bytes, differs for a seeded file the target tuned, and state for a state file. No sentinel is listed, because adoption writes no candidate file
rk adopt --stage <stage> --target . --apply       # adding --replace <path> for each answered conflict
# check: wrote .release-kit/manifest.json and added .release-kit/config.toml. Only the named files changed, each copy saved first
```

- exit 73 naming collisions: nothing was written. Return to step 3 with the named conflicts.
- exit 64 naming `--replace <path>`: the path is no conflict this landing has. Read the row in step 2b again.
- exit 73 naming the stage's version or target: the stage came from another binary or another target. Stage again with this binary, then compare.
- exit 73 from `rk adopt` naming a destination as `expected and missing` or differing: the operator has not named it. Return to step 3, or land through `rk init` where the target holds no release mechanism.
- exit 64 naming the style: a pre-style record. Pass `--release-style <style>`, asked of the operator where the config is silent.
- exit 73 naming the release automation: the profile asks for a release this binary does not carry at its driver and forge. The refusal names the tuples it does carry; the answer is an available pair or another release mode, never a flag that forces past it.
- exit 73 naming a newer `rk_version`: the target is ahead of this binary, and the operator installs the named version.
- exit 74 naming completed paths: a rename stopped part way. Whole files stand beside the previous receipt. Read `git status`, then run the same command again.
- exit 73 with the reason `preset-contradiction`: a flag beside `--workflow-only` states another answer than one of its four. Drop that flag, or drop the preset and state each answer by its own flag.

### 4a. A landing with no release

A project that wants the Git workflow alone takes it with one preset. `--workflow-only` sets `release.mode = "none"`, `git.integration = "local"`, `git.checkout_mode = "linked-worktree"`, and `capabilities.workflow_guards = true`, and `--merge-form <squash|rebase>` combines with it. `--stage` carries every answer a stage resolved, so it refuses the preset beside it: a staged landing passes the four answers to `rk stage` instead, as 1c names.

```bash
rk init --workflow-only --target .                # preview, with the four answers resolved
rk init --workflow-only --target . --apply
# check: AGENTS.md, GLOSSARY.md, .pre-commit-config.yaml, .release-kit/config.toml, and .release-kit/manifest.json, and no sentinel to fill
rk upgrade --workflow-only --target . --apply     # a recorded target taking the same four answers
```

The landing writes no version file, no changelog, no release automation, and no forge gate, because no request reaches the trunk under local integration with no release. The routing block takes its workflow variant, with no `## Releases` heading, and the glossary names no release term. The hook block carries no tag guard, and its commit hook prefers a scope rather than requiring one. It adds the trunk guards of local integration: `no-commit-to-branch` at the pre-commit stage, and the pre-push hooks `rk-no-force-push-to-trunk` and `rk-no-merge-commit-to-trunk`. The configuration leaves out the tag protection keys, `[setup.bot]`, and `[security]` unless `--reporting-policy` asks for the policy. The record and the configuration state the four answers and nothing that names the preset. [Landing](../method/13-landing.md) owns the preset.

## 5. Prove

### 5a. Compare again

```bash
rk stage compare <stage> --target .
# check: every rendered and region row reads equal. A preserved or drift row can differ, because the landing kept the target's bytes, and so can a retired row, which the landing released to the target
rk status --target . --json
# check: classification is current, or drift where a sentinel is still unanswered
```

- a rendered row still differs: a preparation edit or an answer changed between the stage and the landing. Stage again with `rk stage --target . --output <dir>`, add that path to the inventory of stage paths, and land from it.

### 5b. Remove inactive legacy state

Only where 1b recorded paths, only under explicit cleanup authorization, and only after 1b confirmed that no active old operation remains. No `rk` verb removes this state and no verb takes a directory as a recursive target, so each path is reviewed and then removed by hand and by name. `<path>` is one path 1b recorded. Define both functions once, then take the two actions below for each path, in order: the listing, the operator's review, and only then the removal.

```bash
legacy_path_ok() {
  root="${XDG_STATE_HOME:-$HOME/.local/state}/release-kit"
  case "$1" in
    "$root"/plans/?*|"$root"/runs/?*|"$root"/release) [ -d "$1" ] ;;
    *) echo "outside the legacy classes, nothing listed or removed: $1" >&2; return 1 ;;
  esac
}
list_legacy_path() {
  resolved="$(realpath -e -- "$1")" && legacy_path_ok "$resolved" || return 1
  find "$resolved" -mindepth 1 -xdev -print
}
remove_legacy_path() {
  resolved="$(realpath -e -- "$1")" && legacy_path_ok "$resolved" || return 1
  find "$resolved" -mindepth 1 -xdev -depth -delete && rmdir -- "$resolved"
}
list_legacy_path "<path>"
# check: every line is one of the plan, run, or cache entries the inventory named, with no link followed, and nothing was removed. Stop on anything else, and remove nothing for this path
# exit 1 naming the path: it resolves outside the three classes under the state root and is not legacy state. Nothing was listed or removed
```

The operator reviews that list before the next command runs. The removal is a separate action, taken only after the review accepted every line.

```bash
remove_legacy_path "<path>"
# check: exits 0 and the path is gone with the entries the review listed, the directory last. The guard ran again before the -delete
# exit 1 naming the path: refused by the same guard, and nothing was removed
```

Repeat for each recorded path, one at a time.

## After the landing

The checks every run ends with, and the stage cleanup after them. The setup skill's general post-flight runs them.

```bash
rk status --check --target .
# check: exits 0. A drift, missing, sentinel, or invariant line is the next finding, repaired through another fresh production invocation
```

Then run the project's own checks. A landed configuration another tool generates from is regenerated first, per [the setup runbook](./setup.md) step 4b. Only after every check passed, and only where the request's authority includes cleanup, remove each stage. Saved copies and snapshots live outside the stage and stay.

```bash
rk stage clean <stage>
# check: prints removed <stage> and the recovery command that stages the candidate again. Repeat for the next path in the inventory until none is left
# exit 73 naming the path: it is a symlink, a repository root, a home or filesystem root, a directory without stage.json, or a receipt whose stage_root differs from the argument. Nothing was removed
```

The landed files, the receipt included, reach the trunk through the project's own workflow. The stage, the saved copies, and the legacy state are not among them.
