# 06 — Release from trunk

The default style. Precondition: every user can be moved forward at will — a service, an internal tool, a CLI installed from a registry — so exactly one version is alive in the world, and a fix reaches users by rolling forward, never by patching backwards.

## The shared setup

One product, one team, one starting point; [branch for release](./07-branch-for-release.md) reuses it.

```text
master:  A──B──C──D
                  |
               v1.0.0 shipped, running everywhere
```

Work continues, and the trunk moves on:

```text
master:  A──B──C──D──E──F──G──H
                     |  |  |  └─ Carol: new export format, behind a flag, off
                     |  |  └──── Bob: performance fix
                     |  └─────── Alice: OAuth login
                     └────────── Bob: dependency bump
```

Then a customer reports that v1.0.0 crashes on an empty CSV upload. The two styles diverge entirely on how that report is answered.

## Reproduce on the trunk

```bash
git checkout master && git pull
```

Alice reproduces the bug at the trunk's tip, commit H, and writes the failing test first. Reproducing at the tip, not at the v1.0.0 tag, is the point: once the bug lives at H, no thinking about versions is needed at all.

## Fix on the trunk, in a short-lived branch

```bash
rk worktree add fix/PROJ-412-empty-csv --apply && cd ../<project>@fix-PROJ-412-empty-csv
```

Or, in the branches mode, `git checkout -b fix/PROJ-412-empty-csv` in the main checkout — [worktrees](./08-worktrees.md) owns the difference. The name follows [the model's](./00-model.md) branch forms — the type prefix a reviewer routes by, the ticket key the tracker matches. Test, fix, commit, open the pull request; once CI is green and the review lands, squash-merge and delete the branch.

```text
master:  A──B──C──D──E──F──G──H──I
                                 └─ the fix
```

Branch lifetime: under an hour.

## Release

The bot has been maintaining the release request all along, and merging it is the release: automation tags the push that lands the bump and publishes.

```text
master:  A──B──C──D──E──F──G──H──I──R
                  |                 |
               v1.0.0            v1.1.0 tagged at R, published, artifacts building
```

There is no next step. No cherry-pick, no branch to clean up, no second merge. The fix reached the customer by moving forward — together with Alice's OAuth and Bob's fixes, in the same push.

## What made this legal

Shipping the fix meant shipping E, F, G, and H too. That is acceptable only under three standing conditions:

- Every commit on the trunk was already releasable. Carol's unfinished export went out inside H — dark behind its flag, with no effect.
- No code freeze existed. Alice and Bob kept merging while the fix was in flight, and whatever landed before the release request merged simply shipped with it.
- The version follows the trunk. The release is v1.1.0 rather than v1.0.1, because it contains features, not only the fix. A truly patch-only release is unreachable in this style; that is what [branch for release](./07-branch-for-release.md) exists for.

## A timeline

```text
09:14  bug reported
09:30  reproduced at the trunk's tip; failing test written
09:52  pull request opened
10:05  CI green, review approved, squash-merged as commit I
10:07  release request merged; automation tags v1.1.0
10:19  publish and artifact pipeline finish
       Bob merged an unrelated pull request at 09:58; it shipped in the same
       release, and nobody cared.
```

Long-lived branches touched: one.
