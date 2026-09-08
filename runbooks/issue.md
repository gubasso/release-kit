# Issue runbook

Starting work from an issue, with the forge naming the branch. The why is the short-lived branch section of [the model](../method/00-model.md), which states the two forms and prefers the minted one; this page owns the how. Mode-free to read: the verb reads the recorded workflow mode and seats accordingly, and each step says what it does under each. `<project>` is the main checkout's directory name and `<repo>` is the project path, filled in by `rk guide issue` where detection resolves it. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## 1. Preview the start

```bash
rk issue start <issue>
# check: the report names the issue, the branch, and the seat, and says nothing was touched
```

`<issue>` is a number, `#<number>`, or the issue's URL. A URL naming another project refuses before any call, because the branch would be minted on one project and seated in another. `--forge` and `--repo` fill in what detection could not read; either one contradicting the clone's own remote refuses for the same reason.

## 2. Read the report

The branch line says where the name came from.

- `already linked at the forge`: the forge carries a branch for this issue, and the apply adopts it.
- `minted at the forge`: this run created it.
- `not minted yet`: nothing is linked, and the apply asks the forge for a name.

On github:

A preview of an issue with no linked branch prints no branch name. GitHub names the branch when it mints it, so no honest preview can print one before the apply.

On gitlab:

A preview always prints the name, because rk renders it from the project's own template. A detail line names the template that was read, and it names the two cases the name comes out differently: a confidential issue, whose branch never carries the title, and a title carrying characters outside the transliteration table.

## 3. Apply

```bash
rk issue start <issue> --apply
# check: the report prints the seat — a worktree path, or the branch now checked out
```

On worktree:

The branch is seated at `../<project>@<issue-id>-<slug>`, through the same derivation and the same refusals `rk worktree add` uses.

On branches:

The branch is checked out in the main checkout. A dirty working tree makes git refuse the checkout, and git's own reason is what the report carries.

## 4. Prepare the seat

A worktree is a fresh checkout: copy the untracked environment files the project needs, run its setup, and arm the hooks once per clone.

```bash
pre-commit install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push
# check: reports the hook types installed
```

Running the verb twice is safe. A second run adopts what the forge already carries, mints nothing, and reports the standing seat.

## 5. Land through the one path

Commit, push, pull request, squash merge — the trunk's one path, unchanged by how the branch was named. `rk guide setup` step 4 owns the path and `rk guide release` step 1 the landing. Nothing is restated here.

### 5a. The divergent rerun

Each of these stops the run and leaves the clone unchanged. The destination is what the report names.

- The forge CLI is absent or below the floor. The message names the version found, the floor, and the upgrade. `rk doctor` reports the same thing as `gh-version` and `glab-version`.
- On GitLab, the project's branch name template renders a name this verb cannot use. The message names the rendered name and the setting that produced it. Two things can be wrong with it: the landed grammar refuses it, or GitLab would not link it to the issue, which needs the issue number followed by a hyphen at the start. Change the template at Settings, Repository, Branch defaults, Branch name template, or pass a name to `rk worktree add` instead.
- The forge carries the branch and this clone cannot reach its tip. Run `git fetch origin`, then rerun. The apply refuses rather than seat a same-named branch cut from the trunk, which would share none of the forge's history.
- The forge answers a failure. The reason comes from the forge's own answer: authenticate the CLI, get access to the project, wait for a rate limit, or correct the issue number. A rerun is the fix only where the reason says the failure was transient.
- The issue carries more than one linked branch. The report names every one it did not take. The name this run renders wins where the forge carries it, so a steady project keeps taking the same branch; otherwise the first in sort order wins.
- The forge cannot say which branches the issue owns. The run stops rather than mint, because an unanswered read is not proof that no branch exists, and minting on that guess is what leaves one issue with two branches.
- The branch is already seated somewhere other than its derived path. The report names the seat and the move that frees it, exactly as `rk worktree add` does.
