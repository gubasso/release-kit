# Migration runbook

The steps of [migration](../method/10-migration.md) as commands, in the chapter's order: the chapter owns each step's why, this page owns its how. Every step is a gap an observation reported, and every step ends by rerunning the observation that found it; a step whose observation already reads green is recorded as satisfied and skipped. Where `rk setup` executes a step, the `Automated:` line names the verb, and [the setup runbook](./setup.md) carries its check; nothing there is restated here. `rk guide migration` fills in the project path, forge, and technology where detection resolves them. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## Prerequisites

The setup runbook's prerequisites, unchanged: `rk` on `PATH`, this binary's skills installed at user scope, the forge CLI authenticated, and a clean working tree. A migration adds one: the plan. The findings inventory the chapter shapes is written and approved before the first `--apply` below, and every gated step is in it with its exact command.

## 1. Classify the target and read what it runs

```bash
rk assess --target .
# check: the verdict and its evidence; brownfield is this runbook's subject, greenfield takes rk guide setup instead, and needs-decision is the operator's answer before any plan
rk status --target .
# check: none, or the landing with its version, drift, sentinels, and invariants; a recorded target routes by this report, not by the verdict
rk setup check --target .
# check: what the forge enforces today, step by step
rk versions --check
# check: each pin against its source; an update is a review item, never an incident
rk devshell status --target .
# check: how the project obtains rk, and every leftover of a predecessor bump mechanism by file and line
```

Every line of those reports that is not green is one entry in the inventory. Nothing below runs without its entry.

## 2. Record the landing

### 2a. Adopt a target with no record

Adoption verifies the disk against one rendered candidate and never blesses the disk. `--workflow` and `--style` choose the candidate; `--style` is required because it changes the release workflow's bytes.

```bash
rk adopt --target . --scopes <scope,scope> --workflow <mode> --style <style>
# check: every rendered destination reports matches; a differs line names a destination to bring to the candidate's bytes
rk snippet <tech>/<forge>/<path>
# check: the candidate's bytes for one destination; rk payload lists them all with their digests
rk adopt --target . --scopes <scope,scope> --workflow <mode> --style <style> --apply
# check: wrote .release-kit/manifest.json, and nothing else changed
```

- a refusal naming the two marked blocks: the alignment is still owed; bring `AGENTS.md` and `.pre-commit-config.yaml` to the candidate's blocks, then rerun. It is never an error to force past.
- `--workflow branches` is the default, the compatibility-safe reading of a pre-record target; the mode change to `worktree`, where wanted, is its own entry through 2c.

### 2b. Upgrade a recorded target

```bash
rk upgrade --target .
# check: the preview lists every action; a conflict line names a release-kit-owned file the target edited
rk upgrade --target . --apply
# check: rewrote .release-kit/manifest.json; every sentinel left to fill is listed
```

- a record without the scopes parameter: `--scopes <scope,scope>`, the list confirmed with the operator first.
- a record without the style parameter: `--style <style>`, asked of the operator first, because arming an existing project's release request changes what a green trunk does.
- a hook block lacking the `rk-message` content guard: the upgrade re-renders the block; a hand-edited block is reconciled first, per 2d.

### 2c. Change a landing parameter

```bash
rk upgrade --target . --workflow <mode>            # or --style <style>
rk upgrade --target . --workflow <mode> --apply
# check: the two blocks and the record moved; the committed diff is the visible change
```

On worktree:

The transition for branches open across the change is [the worktree runbook](./worktree.md) step 1c, one adoption per open bare branch, after the change reaches the trunk.

### 2d. Reconcile the hooks and answer the sentinels

Before the block lands into an existing `.pre-commit-config.yaml`, [the setup runbook](./setup.md) step 4d owns the reconciliation: a hook already doing one of the block's jobs is the operator's choice, never a second hook on one job.

```bash
grep -rn 'TODO(release-kit)' . --exclude-dir=.git
# check: prints nothing once each sentinel is answered from the project
rk status --check --target .
# check: exits 0; every drift, missing file, sentinel, and invariant line is an entry until it does
```

### 2e. Prove the landed files where the default branch is today

The project's own checks run on the landed files before any branch moves; [the setup runbook](./setup.md) step 4c lands them where the trunk takes a direct write, and step 4f where it does not.

## 3. Take the trunk

A repository already on one branch has this step satisfied; `rk assess` reports the long-lived branches it found.

### 3a. Remove the old release-branch protection

Gated: the operator runs this. The old protection's pull-request-only rule refuses the fast-forward in 3b, and removing a protection from a live branch is never an agent's move.

On github:

```bash
gh api "repos/<repo>/rulesets" -q '.[] | "\(.id) \(.name)"'
# check: names the ruleset guarding the retired release branch
gh api "repos/<repo>/rulesets/<id>" -q '.conditions.ref_name'
# check: the include list names the retired branch alone; a ruleset that also names a live branch is edited, never deleted
gh api "repos/<repo>/rulesets/<id>" > ../ruleset-<id>.json
# check: the whole ruleset is held outside the repository, so a deletion can be recreated from it
gh api -X DELETE "repos/<repo>/rulesets/<id>"
gh api "repos/<repo>/branches/<old-release-branch>/protection" >/dev/null 2>&1 && gh api -X DELETE "repos/<repo>/branches/<old-release-branch>/protection"
# check: the ruleset list no longer names it, and the classic protection read 404s
```

- the ruleset also names a live branch: keep it, and update it in place instead of deleting it. The update body carries only the writable fields, with the retired branch taken out of the include list:

```bash
jq '{name, target, enforcement, bypass_actors, conditions, rules} | .conditions.ref_name.include -= ["refs/heads/<old-release-branch>"]' ../ruleset-<id>.json | gh api -X PUT "repos/<repo>/rulesets/<id>" --input -
gh api "repos/<repo>/rulesets/<id>" -q '.conditions.ref_name.include'
# check: names only live branches; the saved JSON still recreates the ruleset as it stood
```

On gitlab:

```bash
glab api "projects/:id/protected_branches"
# check: names the retired release branch
glab api -X DELETE "projects/:id/protected_branches/<old-release-branch>"
# check: the list no longer names it
```

### 3b. Fast-forward the trunk to the integrated tip

```bash
git fetch origin
git switch master && git merge --ff-only origin/<old-default-branch>
git push origin master
# check: the push lands, and CI proves the landing on the trunk rather than on the old branch
```

- the merge refuses: the trunk holds a commit the old default does not, so the two diverged; that is a finding for the inventory, not a `--no-ff` to add.

### 3c. Make the trunk the default

Automated: `rk setup step default-branch --target . --apply`; [the setup runbook](./setup.md) step 1a carries its check.

## 4. Protect the trunk and the tags

Automated, in order: `rk setup step protect-trunk --target . --apply --required-check <name>`, then `protect-tags`, then `protect-release-lines` only where an older line exists, then `auto-merge`; [the setup runbook](./setup.md) step 3 carries each check and the GitHub ordering that lands the title check first. A squash title or message source that is not the request's title and body is re-asserted by the same `protect-trunk` apply.

```bash
rk setup check --target .
# check: every step reports satisfied, or the limitation the forge document names
```

## 5. Retire every other long-lived branch

### 5a. Remove the old integration branch's protection

Gated, exactly as 3a, against the integration branch: `single-trunk` cannot delete a branch its protection keeps.

### 5b. Close the one-way door

Gated: the operator runs this. Automated: `rk setup step single-trunk --target . --apply` — its guard deletes a candidate only when it is an ancestor of the trunk and fails closed on anything else; a refusal is the next finding, never an obstacle.

```bash
rk setup step single-trunk --target .
# check: the preview names each candidate and whether the trunk holds it
rk setup step single-trunk --target . --apply
# check: prints nothing for the retired branches
```

### 5c. Keep the trunk sole

Automated: `rk setup step merge-cleanup --target . --apply`, then `rk setup step branch-reminder --target . --apply`; [the setup runbook](./setup.md) steps 1c and 1d carry the checks.

```bash
rk branches prune --target .
# check: the clone's own copies of the retired branches are candidates; deleting each is the operator's call
```

## 6. Wire the development environment

### 6a. Replace the predecessor bump mechanism

Only where `rk devshell status` reported a predecessor, and gated: what the cleanup removes is committed first.

```bash
git status --porcelain
# check: empty; the cleanup removes committed files, so nothing it touches is uncommitted
rk devshell clean --target .
rk devshell clean --target . --apply
# check: the scripts and suites are removed; every manual entry is edited by hand, and the rerun reports an empty leftovers list
```

### 6b. Pin rk in the devshell

[The setup runbook](./setup.md)'s prerequisites carry the pin's whole order: add, apply the fragments, commit the pair, sync, allow.

```bash
rk devshell status --target . --json
# check: ready, with an empty leftovers list; nothing else in the tree names an rk version
```

### 6c. Install this binary's skills

```bash
rk skill install
rk skill install --apply
# check: rk doctor reports the three skill probes ok
```

## 7. Close on evidence

```bash
rk status --check --target .
rk setup check --target .
rk versions --check
rk devshell status --target .
# check: each exits 0 with nothing left to name; the inventory's every entry is satisfied by an observation, not by a word
pre-commit install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push
# check: the three hook types are installed in this clone
rk guide release
# check: one release cut end to end, with its verify step passing, is the migration's proof
```

### 7a. The divergent rerun

An interrupted migration resumes from the inventory: rerun step 1 in full, mark every entry an observation now satisfies, and continue from the first that is not. A gap step 1 reports that the inventory does not name returns to planning before anything runs.
