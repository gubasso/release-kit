# Worktree runbook

The four steps of [worktrees](../method/08-worktrees.md) as commands: the chapter owns each step's why, this page owns its how. Mode-free by design — it documents the worktree form, which both workflow modes use; only step 1 notes where the `worktree` mode makes this the only path to a commit. `<project>` is the main checkout's directory name, and `<repo>` is the project path, filled in by `rk guide worktree` where detection resolves it. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## 1. Create the worktree

```bash
rk worktree add <type>/<slug>            # preview: names the derived path and the source
rk worktree add <type>/<slug> --apply
cd ../<project>-<type>-<slug>
# check: the apply prints the absolute path, and rk worktree list shows the seat
```

On worktree:

In this mode the main checkout commits nothing, so this step is the only path to a commit: every code-changing branch takes its worktree before its first commit.

### 1a. Adopt the forge-minted branch

```bash
rk worktree add <issue-id>-<slug> --apply
# check: the source line reports remote; the local tracking branch is created from origin/<branch>, never recreated from the trunk
```

### 1b. Cut a release line's worktree

```bash
rk worktree add release/<line> --base "v<version>" --apply
# check: the line takes its explicit base; without one the add refuses, because a line is cut from a tag, never the tip
```

### 1c. Adopt a bare branch

When moving a branch — or the mode — out of the main checkout:

```bash
git switch master                        # in the main checkout, so the branch is free
rk worktree add <branch> --apply
# check: the source line reports existing, adopted; the work travels with the branch, nothing is lost
```

## 2. Prepare its environment

A worktree is a fresh checkout: copy the untracked environment files the project needs, run its setup, and arm the hooks once per clone.

```bash
pre-commit install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push
# check: reports the hook types installed; armed hooks fire in every linked worktree through the common git dir, so once per clone is enough
```

On rust:

Each worktree builds into its own `target/` by default. A shared `CARGO_TARGET_DIR` stays correct under cargo's lock and serializes parallel builds; per-worktree targets trade disk for parallelism.

## 3. Land through the one path

Commit, push, pull request, squash merge — the trunk's one path, unchanged by the seat: `rk guide setup` step 4 owns the path and `rk guide release` step 1 the landing. Nothing is restated here.

## 4. Prune after the merge

```bash
git fetch --prune origin
rk worktree prune                        # check: the merged worktree is a candidate
rk worktree prune --verify               # check: confirmed against the merged request
rk worktree prune --apply                # check: pruned; the branch and its configuration went with it
```

### 4a. The divergent rerun

An interrupted cleanup recovers with `git worktree list` and `git worktree repair`, then `rk worktree prune` again. A kept row's reason names its unblocking: commit or stash for dirt, `git worktree unlock <path>` for a lock you own. A `branch-delete-failed` row means the worktree is gone and the branch survives with its work; `rk worktree add <branch> --apply` re-seats it.
