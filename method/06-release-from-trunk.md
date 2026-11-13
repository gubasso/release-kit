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

## Why a stale release waits

The gate is judged at each merge attempt. A refusal runs no workflow: freshness is a merge precondition, not a workflow trigger.

```text
merge attempt
  ├─ pull request, squash only, no bypass
  ├─ required project gate and pr-title check pass
  └─ head carries the trunk's current tip     # freshness clause added by strictness
       no  → refuse; the request waits; no workflow dispatched
       yes → merge when the other conditions hold
```

The race uses issue 107's recorded clock and request states, with the strict gate applied to the final attempt. The loose gate permits that attempt and ships v0.1.10 with the breaking change inside its range but absent from its changelog; under `0.y`, the minor position is the break signal.

```text
clock     event                       trunk state         release request state
19:05:45  #26 opens                   #25 at 117e126      v0.1.10; covers #25
19:12:47  breaking #27 merges         #27 at de4efdd      head 96639f6; still v0.1.10
19:12:50  release-plz run for #27     #27                 release-pr job succeeds;
          succeeds                                        head still 96639f6
19:19:49  #26 merge attempt           #27                 behind; strict gate REFUSES
                                                          # loose gate merges here
```

The repair continues the model: a successful job alone does not prove the request refreshed. The bot must rebuild the request on the new trunk tip and recompute its version and changelog. The bot-only GitHub branch takes the force-push path; a non-bot commit makes the bot close and reopen instead. The clock below is an offset from the trunk move, measured on one observed repair; the first step is the bot's own run and the second is the project's gate, so a project reads its own gate duration into the third column.

```text
offset  event                                   trunk state    release request state
+0s     breaking #27 merges                     #27            behind; the merge is refused
+40s    bot refresh rebuilds on #27             #27            v0.2.0; covers #25 AND #27
        # recompute, not a plain branch update
+7m16s  refreshed checks pass                   #27            tested with #27; current
+7m27s  the operator's merge lands               #27──R         merged; v0.2.0 tags R

resulting trunk                            changelog at R
117e126 ── de4efdd ── R (v0.2.0)           #25: feature
   #25        #27                          #27: breaking change
              breaking                     minor bump signals the break
# The trunk range, version, and entry now agree.
```

An ordinary request needs its author's deliberate update when it falls behind. Nothing updates the branch by itself.

```text
local check → push → request checks run → trunk moves
                                              |
                                        nothing runs here
                                        # strictness dispatches nothing
                                              |
               merge ← the same checks again ← gh pr update-branch
                       # one run per workflow that triggers on a request,
                       # and the slowest of them is the whole wait
```

| Cost or outcome                                | Loose policy    | Strict policy                                                                                             |
| ---------------------------------------------- | --------------- | --------------------------------------------------------------------------------------------------------- |
| Request-check runs, one intervening trunk move | One initial set | That set again after the update, one run per triggering workflow; another move can require another update |
| Commands per ordinary merge after pushing      | Merge           | Same, plus one branch update per stale attempt                                                            |
| Human acts per release                         | One merge       | One merge; the bot rebuilt the request within 40 seconds of the trunk move                                |
| Wrong version from this stale-merge race       | Can ship        | Refused until the bot recomputes on the trunk's tip                                                       |

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

## What asks for a release

The release request opens only where two things hold. A file the package ships changed since the last release, because a change outside the package changes nothing a user receives. And a commit that made that change is typed `feat`, `fix`, or `perf`, or marks a breaking change, because the type says whether the change is worth a release. A `build:` or `chore:` commit on a shipped file waits, and rides in the changelog of the next request that a releasing commit opens. The fix above is a `fix:` on shipped code, so it asks for one.

Dependencies follow the same rule. An update that changes what ships is `fix(deps)`, and one for development only is `chore(deps-dev)` or `build(deps-dev)`. A security update always releases: it is `fix(deps)`, also where it changes only a shipped lock file, and its body names the advisory ID, `RUSTSEC-`, `GHSA-`, or `CVE-`, which the landed commit-message hook and title check hold to a releasing type.

## Release

The bot has been maintaining the release request all along, and the fix's merge refreshed it. Nothing merges it before the operator's order, and one release is four steps:

1. Read the open release request and its changelog section.
2. If the changelog needs a correction, push it as the last commit on the request's branch, because the bot regenerates the request on every trunk push.
3. Run `rk release merge --apply`. It reads the request, the trunk tip, and the request's head, then dispatches the merge job with exactly those values. The job runs as the release App, which no ruleset bypasses, proves that the operator ordered it and that nothing moved, and merges with the head pinned.
4. Read the release state, and make sure that the tag, the release page, and the assets exist.

That merge is the release: automation tags the push that lands the bump and publishes. The merge runs as the App rather than as the operator because under local integration the operator's own credential bypasses the trunk ruleset, and the strict check must hold at the write. A release line keeps the merge by a person.

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

## Holding one

A release that must not ship needs no action: the request waits until the operator orders the merge, and it keeps collecting changes meanwhile. After the merge there is nothing to stop — the version is public — and the answer is the withdrawal in [recovery](./04-recovery.md). A project that needs a validated candidate on an older line is describing [branch for release](./07-branch-for-release.md)'s precondition, not this one's.

## A timeline

```text
09:14  bug reported
09:30  reproduced at the trunk's tip; failing test written
09:52  pull request opened
10:05  CI green, review approved, squash-merged as commit I
10:07  the release request's check goes green; the operator runs
       rk release merge --apply, and automation tags v1.1.0
10:19  publish and artifact pipeline finish
       Bob merged an unrelated pull request at 09:58; it shipped in the same
       release, and nobody cared.
```

Long-lived branches touched: one.
