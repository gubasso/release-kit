# 03 — Operate

Cutting a release from the trunk, end to end. The binding supplies the concrete commands; the sequence is the same everywhere. The command form of this chapter is the release runbook, `rk guide release`.

## The sequence

1. Land the work through squash-merged pull requests, each with its release intent captured in the squash title, and the local check suite green.
2. Read the release request. The bot keeps one pull request open against `master`; every merged pull request refreshes it, so the proposed version and the changelog entry always describe the trunk's tip.
3. Decide whether this release ships now. Where the request stands armed — the trunk style's default — the merge follows the last green check, so no human edit on its branch survives to the release: a hold is one disarm command run before the checks finish, and a changelog that reads wrong is answered where it is generated, in the squash titles and bodies the landed gates already judge. Where the request is not armed — a release line, or a release deliberately held — this is the changelog-correction window: correct on the request's branch, in a worktree where the project's mode requires one, last, because the bot refreshes the request as work lands — by force-push while only its own commits sit on the branch, or by closing it and opening a fresh one — and a correction made early is dropped while a correction made last rides into the merge.
4. Let it merge, or merge it. On an armed request the forge merges once the last required check is green and there is nothing to run; on an unarmed one, merge it once its check is green. Either way that merge is the release: automation tags `v<version>` on the push that lands the bump and runs the binding's publish path — a registry publish over OIDC, a dedicated artifact build on the tag, or a tarball attached to the release page, as the binding declares.
5. Wait for the artifact build before verifying anything. Where the binding runs a dedicated artifact builder, it creates the release page in its final job, after every platform builds, so for some minutes after the merge there is no release to look at and a check fails on timing rather than on the release. A binding whose publish stage carries the artifacts has nothing separate to wait for.
6. Verify what the binding ships. Universally: the tag and `master` resolve to the same commit. Then per binding: a registry serves the new version, a release page carries its artifacts and is not empty, an installed binary reports the new version, and the provenance verifies — the run that built each artifact in the release payload signed it, proven with the pair's own verifier — each check applying only where the binding declares that surface. A pair that honestly declares no provenance has nothing to verify there, and a check that failed on it would be wrong; an invariant with no verification step is a comment, which is why the release is not done until this one passes.

## The two steps that get missed

Step 3 is where a release is stopped, and how depends on the style. Armed, the window closes when the last check goes green — a stop after that is a withdrawal rather than an abandon, and [recovery](./04-recovery.md) owns it. Unarmed, it is the only point a changelog correction reaches the release it describes: compare the entry against the commit range since the previous tag, correct, and merge with nothing landing in between.

Step 5 is why a check run straight after the merge reports the release as missing under a dedicated artifact builder. The tag lands within about a minute; the release page arrives only when the slowest platform build finishes.

## When an older line needs the fix

This sequence releases the trunk's tip; a patch-only release for users on an older version is a different path. [Branch for release](./07-branch-for-release.md) owns it, and [recovery](./04-recovery.md) carries the entry point for a line whose branch does not exist yet.
