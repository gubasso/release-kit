# 04 — Recovery

What to do when a release goes wrong. Every path here ends back on the happy path of [operate](./03-operate.md); none of them re-authors a published version.

## A published version is defective

Fix forward, then withdraw. The fix merges to the trunk as ordinary work and ships as the next version through the normal sequence. Withdrawing the defective version — `cargo yank`, `npm deprecate`, PyPI's yank — is the second half: fixing forward stops nothing until the bad version is withdrawn, because resolvers keep serving it to new consumers. Withdrawal is reversible; deletion, where a registry even offers it, is not the tool.

The tag stays. It names what was published, and release-tag immutability exists precisely so that a bad release remains inspectable.

## The release request merged and nothing published

The release job fires on the push that lands the version bump. When a transient failure — a rate limit, a network error — leaves the bump merged but no tag pushed and nothing published, re-run the release job from the CI interface on that same trunk commit. The job is idempotent: it checks whether the tag and the published version already exist and does only what is missing.

This applies to a target whose release intent is `automatic`. A rerun runs the configuration of its own commit, so it recovers a transient failure and never a defect that a later commit fixed. Where the fix landed after the bump, and no tag and no release exist, return `VERSION` to its previous value and remove the changelog entry through an ordinary pull request or merge request. The bash pipelines' bump guard finds the restored version already released at an earlier commit and releases nothing, and the bot proposes the failed version again against the current pipeline. This holds for a first release and for a later one. The precondition is the fork: no tag, no release, nothing published. Where a tag or a release exists, the un-bump rewrites something public and is wrong, and the next section applies. In the bash pipelines a failed build, provenance, or upload leaves no tag on either forge, because only the final release-create request creates it, so this fork is the recovery for every failure before that request.

## The tag exists and the release does not

`rk release status` reports `tag-without-release` for a release tag whose page is missing, or carries no artifact where the pair builds artifacts, when no complete release of its line is newer. First make sure that the tag names the merged release commit, the bump the release request landed. Then take the routes on GitHub in this order:

1. Where the cause was transient, such as a cancelled runner or a network error, re-run the failed jobs of the `release.yml` run that the tag push started. The run attaches the artifacts and mints their provenance.
2. Where the cause was in the code, such as a target that does not build, fix it on the trunk and release the next version. Record the gap where the project keeps its known issues: the version shipped without its release page. Once a newer release of the line is complete, the symptom drops out.

On GitHub, a bash pipeline starts no run on the tag push: its release workflow runs on the branch push alone, and that one run at the bump commit builds, attests, and publishes. Where the cause was transient, re-run the failed `tag-and-attach` job of that branch-push run; where it was in the code, take route 2.

On GitLab, take the routes in the same order. A bash pipeline reaches this state when a release save failed after GitLab created the tag, when a pipeline ran before the tag moved to the release create, or when the tag names another commit and `attach` refuses it.

1. Where the tag names the pipeline's own commit, a rerun of `attach` creates the release on it.
2. Where the tag names the bump commit and the cause was transient, retry the failed job of the tag's pipeline on the same commit while the artifacts of `build` and `provenance` persist, because `attach` uploads what the package registry lacks and links what the release page lacks. `attach` needs both jobs' artifacts, so where they expired, retry `build`, then `provenance` on that build, then `attach`, all on the same commit.
3. Where the cause was in the pipeline or the code, or the tag names another commit, fix it on the trunk and release the next version, as on GitHub.

Never move or delete a protected tag, on either forge. The gated `dist-build` job of the Rust binding is the prevention: it builds every target before the merge, so a target the code cannot build fails the request instead of the release.

## A clone holds a stale release tag

`rk release status` reports `local-tag-differs` for a release tag that the clone holds at another commit than the forge, and `local-tag-not-on-forge` for a release tag that the clone holds and the forge does not. Both come from a tag that moved or went away on the forge after the clone fetched it. `git fetch` never moves a tag that already exists in the clone, and it does not prune a tag that it fetched by auto-follow, so the clone keeps the old tag with no warning. The forge holds the true release tags, because a release tag is derived and never authored.

Refresh every release tag of the clone from the forge:

```sh
git fetch --prune origin '+refs/tags/v*:refs/tags/v*'
```

The `+` refspec replaces each tag that differs, and `--prune` removes each local `v*` tag that the forge does not hold. To refresh one tag, run `git fetch origin '+refs/tags/<tag>:refs/tags/<tag>'`. Both commands take `origin` to be the forge repository that the record names. Each command changes only the clone. It moves no tag on the forge, so it keeps the rule never to move or delete a protected tag.

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

The tag still comes from automation once CI returns; a hand-published version with no tag is repaired by re-running the release workflow on `master`, never by tagging manually.

## The artifact build failed

The publish and the artifact build are separate workflows, so a failed artifact build leaves a published version with an empty release page. Re-run the artifact workflow on the same tag. Nothing about the published version changes; the artifacts attach when the build succeeds, and the re-run mints the provenance the failed one never produced.
