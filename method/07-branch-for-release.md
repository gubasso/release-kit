# 07 — Branch for release

The style for older lines. Precondition: users cannot simply be moved forward — customers run pinned versions on their own infrastructure, a support contract covers an old line, or a sign-off gate stands before a ship. The [shared setup](./06-release-from-trunk.md) carries the same product and team; here the trunk is at H, and Carol's export feature is, in this scenario, unfinished and not flagged.

## Cutting the branch

A few days before the planned v1.1.0 release:

```bash
git checkout -b release/1.1 <commit> && git push -u origin release/1.1
```

```text
master:  A──B──C──D──E──F──G──H──J──K──L──…   work continues, no freeze
                              |
release/1.1:                  └──●            hardening only
```

Cutting the branch slows nobody down: J, K, and L merge to the trunk that same afternoon.

The branch point is chosen, and chosen need not mean latest. With Carol's unflagged half-finished work sitting at G, the branch is cut from F rather than H: the release branch is a snapshot of a trusted point on the trunk, and it can even be created retroactively, days later, from any commit or tag.

## Harden, then release

Validation runs against the branch. Automation tags `v1.1.0-rc.1` there, which builds the installers and publishes nothing to any registry; a human installs them and uses them. Suppose validation finds a pagination bug.

The wrong instinct is to fix it on `release/1.1`. Instead, the fix lands on the trunk first — test, fix, pull request, squash-merge as commit M — and then that one commit crosses:

```bash
git checkout release/1.1 && git cherry-pick M && git push
```

```text
master:  A──B──…──H──J──K──L──M
                  |           └────── cherry-pick
                  ↓                      ↓
release/1.1:      ●──────────────────────●
                rc.1                   rc.2
```

J, K, and L did not travel: a cherry-pick is not a merge, and only M crossed. CI now runs twice — once on the trunk guarding M, once on the branch guarding the cherry-pick. A duplicated pipeline per active line is the real cost of this style, and the reason not to use it without the precondition. When validation passes, automation tags `v1.1.0` on the branch and the release ships.

## The patch release

Two weeks later the empty-CSV bug is reported against v1.1.0, and the trunk is far ahead at T. The move is the same, one-directional: reproduce and fix on the trunk first — commit U — then cherry-pick U onto the branch; with CI green there, automation tags `v1.1.1`.

```text
master:  ──H──J──K──L──M──…──T──U
                     |           └── cherry-pick
                     ↓                  ↓
release/1.1:  ●──────●──────────────────●
                  v1.1.0             v1.1.1
```

Users on the 1.1 line get only the CSV fix; nothing else moved. That capability is the entire reason this style exists.

## The next release, and the death of the old line

When v1.2 is due, `release/1.1` is not reused: `release/1.2` is cut fresh from the trunk, and activity moves there while the old line's cherry-picks trend to zero. When 1.1 is out of production entirely:

```bash
git push origin --delete release/1.1
```

The tags must exist before the branch dies, or its commits dangle and are garbage-collected; a tag outlives its branch, which is what makes the deletion safe and the line recoverable. And `release/1.1` is never merged into `master`: everything on it arrived by cherry-pick from the trunk.

## The four ways the pattern breaks

- Fixing on the release branch, then merging down to the trunk. One merge is forgotten, and the bug regresses at the next branch cut. The one exception: a bug that truly cannot reproduce on the trunk is fixed on the branch and merged down, with the regression risk accepted knowingly.
- Merging the trunk into the release branch instead of cherry-picking. Pulling everything since the cut means the branch was cut on the wrong day.
- Keeping one eternal release branch across versions. Each line gets a fresh branch from the trunk.
- Merging one release branch into another. Never; each line takes its cherry-picks from the trunk independently.
