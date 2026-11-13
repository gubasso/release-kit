# Worktree runbook

This runbook gives the four steps of [worktrees](../method/08-worktrees.md) as commands. The chapter owns the why of each step, and this page owns its how.

The page reads the same under every mode, by design. It documents the worktree form, which both checkout modes use. Only step 1 notes where the `worktree` mode makes this the only path to a commit. Step 3 hands the integration to its own runbook. `<project>` is the directory name of the main checkout, and `<repo>` is the project path. `rk guide worktree` fills both in where detection resolves them.

The operator runs the commands. An agent serves a runbook and states the command. It runs one only where the operator's request named that step.

## 1. Create the worktree

```bash
rk worktree add <type>/<slug>            # preview: names the derived path and the source
rk worktree add <type>/<slug> --apply
cd ../<project>@<type>-<slug>
# check: the apply prints the absolute path, and rk worktree list shows the seat
```

On worktree:

In this mode the main checkout commits nothing, so this step is the only path to a commit. Every branch that changes code takes its worktree before its first commit.

### 1a. Adopt the forge-minted branch

Work that an issue names starts from `rk issue start <issue>`. That verb mints the branch at the forge and seats it here. [The issue runbook](./issue.md) owns that procedure. If the branch already exists and only the seat is missing, run this:

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

To move a branch, or the mode, out of the main checkout, run this:

```bash
git switch master                        # in the main checkout, so the branch is free
rk worktree add <branch> --apply
# check: the source line reports existing, adopted; the work travels with the branch, nothing is lost
```

## 2. Prepare its environment

A worktree is a fresh checkout. Copy the untracked environment files that the project needs. Run its setup. Arm the hooks once per clone.

```bash
prek install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push --hook-type pre-merge-commit
# check: names each hook it installed; on git 2.54 or newer, git hook list pre-merge-commit names one too; armed hooks fire in every linked worktree through the common git dir, so once per clone is enough
```

On rust:

Each worktree builds into its own `target/` by default. A shared `CARGO_TARGET_DIR` stays correct under the lock of cargo, and it serializes parallel builds. A target for each worktree trades disk space for parallel builds.

## 3. Integrate

The work of the seat reaches the trunk through the authority that the record names. [The integration runbook](./integration.md) owns both paths: `rk guide integration`. This page restates neither.

## 4. Prune after the integration

```bash
git fetch --prune origin                 # forge integration alone; a local integration reached no remote
rk worktree prune                        # check: the retired worktree is reported
rk worktree prune --verify               # check: confirmed, and the row names which proof did it
rk worktree prune --apply                # check: pruned; the branch and its configuration went with it
```

The verify names one of three proofs. Each removal rests on one of them:

- `request`: a merged request whose recorded head equals the tip of the branch. Forge integration leaves this proof.
- `local-integration`: a recorded local integration whose branch tip equals the tip, and whose commit the trunk reaches. `rk integrate` leaves this proof under either merge form.
- `reachable-tip`: the trunk reaches the current tip of the branch, and the branch moved from the commit that the creation entry of its reflog records. It needs no record and no forge answer. So it holds after each of these:
  - A local rebase integration.
  - A GitLab fast-forward merge.
  - A hand `git merge --ff-only`.

  This proof never prunes a fresh branch still at its creation commit. A branch whose creation entry expired gets no such proof.

A squash writes a new commit, and a GitHub rebase merge writes new commits with new SHAs. After either one, the trunk does not reach the tip. The merged request or the recorded integration stays the proof. [Integration](./integration.md) owns which one a target leaves, and [worktrees](../method/08-worktrees.md) owns why each proof is admissible. `rk branches prune --verify` names the same three proofs for a branch with no seat.

### 4a. The divergent rerun

If a cleanup stopped partway, run `git worktree list`. Then run `git worktree repair`. Then run `rk worktree prune` again. The reason of a kept row names what unblocks it. For uncommitted work, commit or stash it. For a lock that you own, run `git worktree unlock <path>`. A `branch-delete-failed` row means that the worktree is gone and the branch survives with its work. `rk worktree add <branch> --apply` seats it again.

## 5. Promote to a workspace (optional)

A workspace is a directory for this project alone. It holds these:

- The main checkout.
- The worktrees.
- Whatever else the project keeps beside them.

The [chapter](../method/08-worktrees.md) states the layouts, and this is one of them. Promotion moves each seat, at any time:

```bash
mkdir ../<workspace>
git worktree move ../<project>@<flat> ../<workspace>/<project>@<flat>   # each linked worktree; git refuses to move the main one
mv ../<project> ../<workspace>/<project>                                # the main checkout last, as a plain move
git -C ../<workspace>/<project> worktree repair                         # check: repair reports each re-linked seat, or nothing
rk worktree list                                                        # check: every seat listed, none off-path, none missing
```
