# 03 — Operate

Cutting a release, end to end. The binding supplies the concrete commands; the sequence is the same everywhere.

## The sequence

1. Land the work on `develop` with its release intent captured, and the local check suite green.
2. Push. The bot opens or refreshes the release request: the version bump and the changelog entry, as one pull request against `develop`.
3. Read the changelog entry in that request and correct it on the request's branch before merging. The bot writes the entry when it opens the request and does not regenerate it as later work lands, so a request left open while commits keep landing ships a changelog that omits the newer work. This is the last point a correction reaches the release: the gate pins at the merge commit, and a published entry is never edited.
4. Merge the release request. The bump and the changelog land on `develop`; nothing is published.
5. Wait for the gate. That merge leaves `develop` carrying a version no tag names, so automation cuts `release/v<version>` at the merged commit and opens it into `master`.
6. Merge the gate once its checks are green, as a merge commit. This is the release: automation tags `v<version>` on `master` and runs the binding's publish and artifact path — a registry publish over OIDC, a dedicated artifact build on the tag, or a tarball attached to the release page, as the binding declares.
7. Back-merge `master` into `develop`, so the next release diffs cleanly. While `develop` has not moved it is a fast-forward that lands both branches on the tagged commit; when work landed meanwhile it is a merge commit, and the tagged commit becomes an ancestor of `develop` rather than its tip.
8. Wait for the artifact build before verifying anything. Where the binding runs a dedicated artifact builder, it creates the release page in its final job, after every platform builds, so for some minutes after the merge there is no release to look at and a check fails on timing rather than on the release. A binding whose publish stage carries the artifacts has nothing separate to wait for.
9. Verify what the binding ships. Universally: the tag and `master` resolve to the same commit, and that commit is an ancestor of `develop` — the same commit exactly when the back-merge fast-forwarded. Then per binding: a registry serves the new version, a release page carries its artifacts and is not empty, an installed binary reports the new version — each check applying only where the binding declares that surface.

## The two steps that get skipped

Step 3 is the only window in which the changelog can still be corrected, and the bot drops entries whenever work lands while the request is open. Compare the entry against the commit range since the previous tag before merging.

Step 8 is why a check run straight after the gate merge reports the release as missing under a dedicated artifact builder. The tag lands within about a minute; the release page arrives only when the slowest platform build finishes.

## While the gate is open

The gate branch is pinned to one commit, so work landing on `develop` while the gate is open never joins the release. Merging `develop` work does not require closing the gate; it simply ships in the next one.
