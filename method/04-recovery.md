# 04 — Recovery

What to do when a release goes wrong. Every path here ends back on the happy path of [operate](./03-operate.md); none of them re-authors a published version.

## A published version is defective

Fix forward, then withdraw. The fix merges to `develop` as ordinary work and ships as the next version through the normal sequence. Withdrawing the defective version — `cargo yank`, `npm deprecate`, PyPI's yank — is the second half: fixing forward stops nothing until the bad version is withdrawn, because resolvers keep serving it to new consumers. Withdrawal is reversible; deletion, where a registry even offers it, is not the tool.

The tag stays. It names what was published, and release-tag immutability exists precisely so that a bad release remains inspectable.

## The gate never opened

The gate job cuts the release branch only on the push that bumps the committed version. When a transient failure — a rate limit, a network error — leaves the release request merged but no gate open, re-run the gate job from the CI interface on that same commit. The job is idempotent: it replays the same commit, finds the branch or creates it, and opens the pull request only if none is open.

## The changelog shipped wrong

A published entry is never edited. Land the correction on `develop` as an ordinary commit amending the changelog file; it ships with the next release. The prevention is step 3 of [operate](./03-operate.md), the only point a correction reaches the release it describes.

## CI is down and the release cannot wait

Publish by hand, in three moves, and only for a release the gate already approved:

1. Turn off the registry's require-trusted-publishing enforcement. While it is on, every token publish is rejected, so the manual path cannot succeed — and its failure message points at credentials, not at the switch.
2. Publish with a token scoped like the bootstrap one: new versions of exactly this package, shortest expiry. Revoke it immediately after.
3. Turn enforcement back on, and let the next automated release prove the OIDC path still works.

The tag still comes from automation once CI returns; a hand-published version with no tag is reconciled by re-running the release workflow on `master`, never by tagging manually.

## The artifact build failed

The publish and the artifact build are separate workflows, so a failed artifact build leaves a published version with an empty release page. Re-run the artifact workflow on the same tag. Nothing about the published version changes; the artifacts attach when the build succeeds.
