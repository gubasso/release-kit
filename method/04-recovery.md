# 04 — Recovery

What to do when a release goes wrong. Every path here ends back on the happy path of [operate](./03-operate.md); none of them re-authors a published version.

## A published version is defective

Fix forward, then withdraw. The fix merges to the trunk as ordinary work and ships as the next version through the normal sequence. Withdrawing the defective version — `cargo yank`, `npm deprecate`, PyPI's yank — is the second half: fixing forward stops nothing until the bad version is withdrawn, because resolvers keep serving it to new consumers. Withdrawal is reversible; deletion, where a registry even offers it, is not the tool.

The tag stays. It names what was published, and release-tag immutability exists precisely so that a bad release remains inspectable.

## The release request merged and nothing published

The release job fires on the push that lands the version bump. When a transient failure — a rate limit, a network error — leaves the bump merged but no tag pushed and nothing published, re-run the release job from the CI interface on that same trunk commit. The job is idempotent: it checks whether the tag and the published version already exist and does only what is missing.

## The changelog shipped wrong

A published entry is never edited. Land the correction on the trunk as an ordinary commit amending the changelog file; it ships with the next release. The prevention is step 3 of [operate](./03-operate.md), the only point a correction reaches the release it describes.

## An older line needs a patch

The trunk has rolled forward, and a user on an older version needs only the fix. When the line's branch does not exist, cut it retroactively from the tag — `release/<major>.<minor>` at the line's latest `v<major>.<minor>.<z>` — because the tag pins the exact commit the line shipped from. Then fix on the trunk first and cherry-pick the one commit onto the branch; [branch for release](./07-branch-for-release.md) owns the sequence, the rc validation, and the four ways the pattern breaks.

## CI is down and the release cannot wait

Publish by hand, in three moves, and only for a version whose release request already merged:

1. Turn off the registry's require-trusted-publishing enforcement. While it is on, every token publish is rejected, so the manual path cannot succeed — and its failure message points at credentials, not at the switch.
2. Publish with a token scoped like the bootstrap one: new versions of exactly this package, shortest expiry. Revoke it immediately after.
3. Turn enforcement back on, and let the next automated release prove the OIDC path still works.

A hand-published artifact carries no provenance. The signature is minted by the run that builds, so uploading a file by hand — even the exact file CI built — leaves the attestation lookup answering with nothing, and any consumer that verifies before installing refuses that one version while the releases on either side of it install fine. Treat the manual upload as temporary: once CI returns, re-run the artifact workflow on the same tag so the artifacts are rebuilt and attested through the normal path.

The tag still comes from automation once CI returns; a hand-published version with no tag is reconciled by re-running the release workflow on `master`, never by tagging manually.

## The artifact build failed

The publish and the artifact build are separate workflows, so a failed artifact build leaves a published version with an empty release page. Re-run the artifact workflow on the same tag. Nothing about the published version changes; the artifacts attach when the build succeeds, and the re-run mints the provenance the failed one never produced.
