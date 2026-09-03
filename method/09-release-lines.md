# 09 — Release lines

The line as an object, across its whole life. [Branch for release](./07-branch-for-release.md) argues the style and walks one fix crossing over; this chapter owns the line itself — the recorded style, the cut, the candidate cycle, the promotion, and the retirement. The command form of this chapter is the release-lines runbook, `rk guide release-lines`.

## The sequence

1. Record the style. A project that keeps older lines lands `--style lines`: its release request is never armed, because a line's release is a promotion of something a human validated, and `rk status` is where a clone, a CI job, and an agent read that answer.
2. Wire the repository for lines, once: `release/*` takes no force-push and no deletion while a line is alive, and the landed release workflow already watches `release/**`, so a line gets its own release request with no further configuration.
3. Open the line from the tag it patches. The base is chosen and stated — never the trunk's tip by default — and a line can be cut retroactively, days later, from any commit or tag.
4. Cross the fix from the trunk. Fix on the trunk first, cherry-pick only that commit; [branch for release](./07-branch-for-release.md) owns the path and its four failure modes.
5. Mint a candidate. Automation tags `v<version>-rc.<n>` on the line; the tag builds the installers and publishes to no registry, so a candidate can never shadow the version it precedes.
6. Validate the candidate by using it. Install what the rc built and exercise it; the point of the style is that a human, not a check, says the line is ready.
7. Answer a finding with the next rc. A finding lands on the trunk, crosses by cherry-pick, and mints the next number: an rc number is single-use, because the tag protection makes a candidate immutable.
8. Promote, then retire. The line's release request is merged by hand once the candidate stands, and when the line leaves production its branch is deleted only after a tag names every commit it holds, because a tag outlives its branch and is what makes the deletion safe.

## Why the request is never armed on a line

The trunk style's standing arm exists because every trunk commit is already releasable, so a green check is the whole decision. A line inverts that: the thing being released is a candidate a human validated by hand, and the checks cannot see the validation. An armed line request would promote on CI's word alone, which is exactly the judgment this style exists to keep human. The style parameter is what the landed workflow reads: `lines` renders no arming step, so nothing on a line merges itself.

## The cost of a line

Every fix that crosses runs CI twice — once guarding the trunk, once guarding the line — and each finding costs a full rc cycle: cherry-pick, tag, rebuild, revalidate. That duplicated pipeline per active line is the argument for defaulting to the trunk, stated where [branch for release](./07-branch-for-release.md) walks it.

## When a line is the wrong answer

The five-question table in [the model](./00-model.md) decides. Every user movable forward, several ships a week, nobody pinned: release from the trunk, and let the armed request ship it. A line answers pinned self-hosted versions, a support contract on an old line, or a sign-off gate before a ship — and it is cut the day someone actually needs it, retroactively from the tag, never ahead of the need.
