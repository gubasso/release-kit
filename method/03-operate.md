# 03 — Operate

Cutting a release from the trunk, end to end. The binding supplies the concrete commands; the sequence is the same everywhere. The command form of this chapter is the release runbook, `rk guide release`.

## The sequence

1. Land the work through squash-merged pull requests, each with its release intent captured in the squash title, and the local check suite green.
2. Read the release request. The bot keeps one pull request open against `master`; every merged pull request refreshes it, so the proposed version and the changelog entry always describe the trunk's tip.
3. Correct the changelog on the request's branch, last. The bot refreshes the request as work lands — by force-push while only its own commits sit on the branch, or by closing it and opening a fresh one — and a human correction survives neither refresh, so a correction made early is dropped by the next landed work and a correction made last rides into the merge.
4. Merge the release request once its check is green. This is the release: automation tags `v<version>` on the push that lands the bump and runs the binding's publish path — a registry publish over OIDC, a dedicated artifact build on the tag, or a tarball attached to the release page, as the binding declares.
5. Wait for the artifact build before verifying anything. Where the binding runs a dedicated artifact builder, it creates the release page in its final job, after every platform builds, so for some minutes after the merge there is no release to look at and a check fails on timing rather than on the release. A binding whose publish stage carries the artifacts has nothing separate to wait for.
6. Verify what the binding ships. Universally: the tag and `master` resolve to the same commit. Then per binding: a registry serves the new version, a release page carries its artifacts and is not empty, an installed binary reports the new version — each check applying only where the binding declares that surface.

## The two steps that get skipped

Step 3 is the only window in which the changelog can still be corrected, and a correction does not survive a later refresh. Compare the entry against the commit range since the previous tag, correct, and merge with nothing landing in between.

Step 5 is why a check run straight after the merge reports the release as missing under a dedicated artifact builder. The tag lands within about a minute; the release page arrives only when the slowest platform build finishes.

## When an older line needs the fix

This sequence releases the trunk's tip; a patch-only release for users on an older version is a different path. [Branch for release](./07-branch-for-release.md) owns it, and [recovery](./04-recovery.md) carries the entry point for a line whose branch does not exist yet.
