# 10 — Migration

How a repository moves from wherever it releases today to this convention, without losing a release path and without a step nobody can retrace. [Setup](./02-setup.md) bootstraps a bare repository once, in order; this chapter owns the target that already ships somehow — a release tool of its own, a gated two-branch flow, a payload landed before the record existed, or a landing that drifted — and the loop that carries it across. The command form of this chapter is the migration runbook, `rk guide migration`; `rk assess` reads the verdict the first step depends on.

## Classification before anything lands

A migration begins with a verdict about the target, read from evidence rather than judged by feel.

- greenfield: no release mechanism and no release history — no tool configuration, no payload destination on disk, no tag, no second long-lived branch. Land the workflow through [setup](./02-setup.md); there is nothing to migrate.
- brownfield: a release mechanism is in place — another tool's configuration, a payload destination already present, a landed block. The mechanism is migrated, never overwritten and never left as a second release path beside the new one.
- needs-decision: release activity no recognized mechanism explains — tags with no tool behind them, or a long-lived branch beside the trunk. The operator says what it is before any plan claims to know.

A recorded landing is a fourth state and not a classification: it routes to `rk status`, and from there to `rk upgrade` or to the drift the report names. No record does not mean no release: a brownfield target with no record is the common case, and treating it as green is how a repository ends up with two release paths at once.

## The findings are the plan

The plan's body is a findings inventory: one entry per gap the observations report, written before the first change and held for the whole task. Each entry carries the finding, its disposition, the command that closes it, the observation that verifies it, and its state. The dispositions are three:

- run it now: an `rk` verb with a preview, applied in the loop, smallest first.
- gate it for the operator: a destructive step, a credential, a registry action, or a removal on a live branch — printed as its exact command, waited on, and re-observed before anything continues.
- already satisfied: an observation that already reads green, recorded so a later reader sees it was checked and not skipped.

What the inventory never carries is a second record of what ran. `rk runs` journals every executed setup step with its outcome, and the landing record states what landed; the inventory names both and copies neither. A discovery — a gap no observation reported, a leftover the plan did not name, a removal nobody approved — returns to planning. It is never handled inline, because a scope widened one finding at a time was never approved at any size.

## The sequence

1. Classify the target and read what it runs. `rk assess` gives the verdict and the evidence; `rk status` the landing and its drift; `rk setup check` what the forge enforces; `rk versions --check` the pins; `rk devshell status` how the project obtains `rk` and what a predecessor mechanism left. Every later step is a gap one of these reported, and every step ends by rerunning the observation that found it.
2. Record the landing. A target with no record is verified against one rendered candidate and recorded by `rk adopt`, after its marked blocks are brought to the candidate's bytes; a recorded target older than the binary, or missing a parameter the record now carries, is taken forward by `rk upgrade`. The hooks are reconciled before the block lands, the sentinels are answered from the project, and the project's own checks prove the landed files on whatever branch is the default today.
3. Take the trunk. Where the repository still runs an integration branch and a gated release branch, the fast-forward that makes `master` the integrated tip is refused by the old release-branch protection, so that protection goes first, gated; then the trunk takes the tip, CI proves the landing there, and the forge makes it the default. A repository already on one branch has this step satisfied.
4. Protect the trunk and the tags. The trunk protection, the tag protection, the line protection where older lines exist, and the merge switch, exactly as [setup](./02-setup.md) states them; the squash sources are asserted rather than assumed, because a title taken from a branch commit is the changelog's quality lost silently.
5. Retire every other long-lived branch. The old integration branch's protection goes first, gated, and then the one-way door: `single-trunk` deletes a candidate only when the trunk already holds every commit of it, and its refusal is a stop, never an obstacle to force past. Merge cleanup and the post-merge reminder close the loop so no retired branch comes back.
6. Wire the development environment. A project that obtains `rk` by a host install or a hand-rolled bump takes the devshell pin as a replacement: the predecessor is removed first, because a native mechanism wired over an unremoved one is a failed migration rather than a partial one, and the target reads as ready only with nothing of the predecessor left. The skills this binary carries are installed at user scope in the same pass.
7. Close on evidence. `rk status --check`, `rk setup check`, `rk versions --check`, and `rk devshell status` read green together, the hooks are installed in the clone, and the first release cut through [operate](./03-operate.md) is the migration's proof — not the last checked entry.

## The loop

Execution is a loop, not a script: observe, act on the smallest gap, re-observe, continue.

- A check that cannot be read is not a pass. A forge the CLI cannot reach, a probe that did not run, a report that errored: each is the next gap, named as such.
- A failed check is never continued past. The step that failed is the whole of the next action.
- Every gated step's outcome is reported from observation, never from the operator's word alone.
- The repository stays releasable at every stop: a migration interrupted between steps breaks no existing flow, so each step leaves the old path working until the new one is proven.

## Recoverability before removal

Three steps remove something a mistake cannot rebuild: the second branch's deletion, a protection's removal from a live branch, and the predecessor bump mechanism's cleanup. Each one runs only while what it removes is held elsewhere — a branch whose every commit the trunk already contains, a protection whose replacement already stands, files whose bytes version control already committed. An untracked or locally modified file in the cleanup's path is committed first, and a branch the ancestry guard refuses keeps its work until someone moves it. A migration with no recovery path for a removal removes nothing.

## Changing a landing parameter

A workflow-mode or release-style change is a named migration, never a side effect of a code-change request: `rk upgrade --workflow <mode>` or `rk upgrade --style <style>`, previewed, applied on approval, committed through the trunk's one path. [Worktrees](./08-worktrees.md) owns the transition for branches open across a mode change, and [release lines](./09-release-lines.md) owns what the style changes; this chapter adds only that the change is an inventory entry with its own verification, not a line in another entry.

## Boundary tests

- A target with a release tool and no record is brownfield, not greenfield: classify by the mechanism, never by the record.
- A target with tags and no mechanism is a question, not a verdict: the operator says what the tags are.
- A recorded target routes by its status report, whatever the corpus verdict says: every healthy landing reads as brownfield, and that is not a migration.
- A refusal from `single-trunk` is the migration's next finding, not a flag to add.
- A predecessor mechanism still wired beside the devshell pin is a failed step, not a partial one: the cleanup's leftovers list is what says done.
- A gap found mid-step returns to planning: the inventory is what was approved, at its own size.
