# Issue runbook

This runbook starts work from an issue, and the forge names the branch. The short-lived branch section of [the model](../method/00-model.md) owns the why. It states the two forms and prefers the minted one. This page owns the how.

The page reads the same under every mode. The verb reads the recorded workflow mode and seats the branch to match, and each step says what it does under each mode. `<project>` is the directory name of the main checkout, and `<repo>` is the project path. `rk guide issue` fills both in where detection resolves them.

The operator runs the commands. An agent serves a runbook and states the command. It runs one only where the operator's request named that step.

## 1. Preview the start

```bash
rk issue start <issue>
# check: the report names the issue, the branch, and the seat, and says nothing was touched
```

`<issue>` takes one of three forms:

- A number.
- `#<number>`.
- The URL of the issue.

A URL that names another project refuses before any call. Otherwise the branch would be minted on one project and seated in another.

The verb compares the hosts as they are written. An instance can serve SSH under a separate endpoint, for example `git@ssh.example.com` for `example.com`. Such an instance refuses its own web URL. There, pass the issue number. It names the same issue and asks no question about hosts.

`--forge` and `--repo` fill in what detection could not read. If either one contradicts the clone's own remote, the run refuses for the same reason.

## 2. Read the report

The branch line says where the name came from.

- `already linked at the forge`: the forge carries a branch for this issue, and the apply adopts it.
- `minted at the forge`: this run created it.
- `not minted yet`: nothing is linked, and the apply asks the forge for a name.

On github:

A preview of an issue with no linked branch prints no branch name. GitHub names the branch when it mints it, so no honest preview can print one before the apply.

On gitlab:

A preview always prints the name, because rk renders it from the project's own template. A detail line names the template that rk read. It also names the two cases where the name comes out differently:

- A confidential issue, whose branch never carries the title.
- A title that carries characters outside the transliteration table.

## 3. Apply

```bash
rk issue start <issue> --apply
# check: the report prints the seat — a worktree path, or the branch now checked out
```

On worktree:

The verb seats the branch at `../<project>@<issue-id>-<slug>`. It uses the same derivation and the same refusals as `rk worktree add`.

On branches:

The verb checks the branch out in the main checkout, whichever worktree of the repository you named. It asks for a clean checkout. Uncommitted work there refuses the run before the verb creates anything at the forge.

## 4. Prepare the seat

A worktree is a fresh checkout. Copy the untracked environment files that the project needs. Run its setup. Arm the hooks once per clone.

```bash
prek install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push --hook-type pre-merge-commit
# check: names each hook it installed; on git 2.54 or newer, git hook list pre-merge-commit names one too
```

A second run of the verb is safe. It does three things:

- It adopts what the forge already carries.
- It mints nothing.
- It reports the standing seat.

## 5. Land through the one path

The one path to the trunk has four stages:

- Commit.
- Push.
- Pull request.
- Squash merge.

The way the branch got its name does not change it. `rk guide setup` step 4 owns the path, and `rk guide release` step 1 owns the landing. This page restates neither.

### 5a. The divergent rerun

Each of these cases stops the run and leaves the clone unchanged. Go where the report names.

- The forge CLI is absent or below the floor. The message names three things:
  - The version found.
  - The floor.
  - The upgrade.

  `rk doctor` reports the same thing as `gh-version` and `glab-version`.
- On GitLab, the project's branch name template renders a name this verb cannot use. The message names the rendered name and the setting that produced it. Two things can be wrong with it. The landed grammar refuses it, or GitLab does not link it to the issue. A link needs the issue number and a hyphen at the start. Change the template at Settings, Repository, Branch defaults, Branch name template. Or pass a name to `rk worktree add` instead.
- The forge carries the branch and this clone cannot reach its tip. Run `git fetch origin`. Then rerun the verb. The apply refuses to seat a branch of the same name cut from the trunk. That branch shares none of the history that the forge holds.
- This clone could not refresh from the forge. Run `git fetch origin`. Then rerun the verb. A remote-tracking ref that an older fetch left behind is not the tip that the forge holds now. So the apply refuses to seat from it.
- Under the branches mode, the branch is already checked out in another worktree. The report names that worktree and the switch that frees it, and nothing is created at the forge.
- Under the branches mode, the main checkout carries uncommitted work. Commit or stash it. Then rerun the verb. This mode checks the branch out there, so it asks for a clean checkout. That is stricter than git alone. The verb creates nothing at the forge in either case.
- The clone's origin, or the issue URL where the clone names no host, is a GitHub host other than github.com. This verb reaches github.com alone. Start the branch with `gh issue develop --repo <host>/<owner>/<name>`. Then seat it with `rk worktree add`.
- The forge answers a failure. The reason comes from the forge's own answer. Take the one action that the reason names:
  - Authenticate the CLI.
  - Get access to the project.
  - Wait for a rate limit.
  - Correct the issue number.

  A rerun is the fix only where the reason says that the failure was transient.
- The issue carries more than one linked branch. The report names every one it did not take. Where the forge carries the name that this run renders, that name wins, so a steady project keeps taking the same branch. Otherwise the first in sort order wins.
- The forge cannot say which branches the issue owns. The run stops and mints nothing. An unanswered read is not proof that no branch exists. A mint on that guess leaves one issue with two branches.
- The branch is already seated somewhere other than its derived path. The report names the seat and the move that frees it, exactly as `rk worktree add` does.
