# 04 — Recovery

This chapter says what to do when a release goes wrong. Every path here ends back on the happy path of [operate](./03-operate.md). None of them re-authors a published version.

## A published version is defective

Fix forward, then withdraw. The fix merges to the trunk as ordinary work. It ships as the next version through the normal sequence.

Withdrawing the defective version is the second half. The tools are `cargo yank`, `npm deprecate`, and PyPI's yank. Fixing forward stops nothing until the bad version is withdrawn, because resolvers keep serving it to new consumers. Withdrawal is reversible. Deletion, where a registry offers it, is not the tool.

The tag stays. It names what was published. Release-tag immutability exists so that a bad release remains inspectable.

## The release request merged and nothing published

The release job fires on the push that lands the version bump. A transient failure, such as a rate limit or a network error, can leave the bump merged with no tag pushed and nothing published. In that case, re-run the release job from the CI interface on that same trunk commit. The job is idempotent. It checks whether the tag and the published version already exist, and it does only what is missing.

This applies to a target whose release intent is `automatic`. A rerun runs the configuration of its own commit. So a rerun recovers a transient failure, and never a defect that a later commit fixed.

Where the fix landed after the bump, and no tag and no release exist, undo the bump:

1. Return the version to its previous value through an ordinary pull request or merge request.
2. On a bash target, return `VERSION` to its previous value.
3. On a python target, run `uv version <previous> --no-sync`. It writes `pyproject.toml` and `uv.lock` together.
4. Remove the changelog entry in the same request.

The build of the bash and python pipelines then finds the restored version already released at an earlier commit, and it releases nothing. The next `rk release request` proposes the failed version again against the current pipeline. This holds for a first release and for a later one.

The precondition is the fork: no tag, no release, nothing published. Where a tag or a release exists, the un-bump rewrites something public and is wrong, and the next section applies.

In the bash and python pipelines, a failed build, provenance, or upload leaves no tag on either forge. Only `rk release publish` creates the tag, together with the forge release. So this fork is the recovery for every failure before that step.

A python release that uploaded to PyPI and stopped before the publish is published, so the fork does not apply. Re-run the release job at the same commit. The build compares its rebuild with the files PyPI serves. The upload skips them, and the publish creates the page and the tag.

## The tag exists and the release does not

`rk release status` reports `tag-without-release` for a release tag whose page is missing. It reports the same where the pair builds artifacts and the page carries none. It reports this only when no complete release of the line is newer.

First, make sure that the tag names the merged release commit, the bump that the release request landed. Then take the routes on GitHub in this order:

1. A cause can be transient, such as a cancelled runner or a network error. In that case, re-run the failed jobs of the `release.yml` run. The tag push started that run. The run attaches the artifacts and mints their provenance.
2. Where the cause was in the code, such as a target that does not build, fix it on the trunk. Release the next version. Record the gap where the project keeps its known issues: the version shipped without its release page. Once a newer release of the line is complete, the symptom drops out.

On GitHub, a bash or python pipeline starts no run on the tag push. Its release workflow runs on the branch push alone. That one run at the bump commit builds, attests, uploads where the pair has a registry, and publishes. Where the cause was transient, re-run the failed `release` job of that branch-push run. Where it was in the code, take route 2.

On GitLab, take the routes in the same order. A bash or python pipeline reaches this state in three ways:

- A release save failed after GitLab created the tag.
- A pipeline ran before the tag moved to the release create.
- The tag names another commit, and `publish` refuses it.

The routes on GitLab:

1. Where the tag names the pipeline's own commit, re-run `publish`. It creates the release on that commit.
2. Where the tag names the bump commit and the cause was transient, retry the failed job of the tag's pipeline on the same commit. Do this while the artifacts of `build` and `provenance` persist. `publish` uploads what the package registry lacks, and links what the release page lacks. It needs the artifacts of both jobs. Where they expired, retry these jobs in order, all on the same commit:
   1. `build`
   2. `provenance`, on that build
   3. `pypi`, where it runs
   4. `publish`
3. Where the cause was in the pipeline or the code, or the tag names another commit, fix it on the trunk. Release the next version, as on GitHub.

Never move or delete a protected tag, on either forge. The gated `dist-build` job of the Rust binding is the prevention. It builds every target before the merge, so a target that the code cannot build fails the request instead of the release.

## A clone holds a stale release tag

`rk release status` reports `local-tag-differs` for a release tag that the clone holds at another commit than the forge. It reports `local-tag-not-on-forge` for a release tag that the clone holds and the forge does not. Both come from a tag that moved or went away on the forge after the clone fetched it.

`git fetch` never moves a tag that already exists in the clone. It also does not prune a tag that it fetched by auto-follow. So the clone keeps the old tag with no warning. The forge holds the true release tags, because a release tag is derived and never authored.

Refresh every release tag of the clone from the forge:

```sh
git fetch --prune origin '+refs/tags/v*:refs/tags/v*'
```

The `+` refspec replaces each tag that differs. `--prune` removes each local `v*` tag that the forge does not hold. To refresh one tag, run `git fetch origin '+refs/tags/<tag>:refs/tags/<tag>'`. Both commands take `origin` to be the forge repository that the record names.

Each command changes only the clone. It moves no tag on the forge, so it keeps the rule never to move or delete a protected tag.

## The changelog shipped wrong

A published entry is never edited. Land the correction on the trunk as an ordinary commit to the changelog file. It ships with the next release. The prevention is step 3 of [operate](./03-operate.md), the only point where a correction reaches the release it describes.

## An older line needs a patch

The trunk rolled forward, and a user on an older version needs only the fix. When the line's branch does not exist, cut it from the tag. The branch is `release/<major>.<minor>`, at the line's latest `v<major>.<minor>.<z>`. The tag pins the exact commit that the line shipped from.

Then fix on the trunk first, and cherry-pick the one commit onto the branch. [Branch for release](./07-branch-for-release.md) owns the sequence, the rc validation, and the four ways the pattern breaks.

## CI is down and the release cannot wait

Publish by hand, and only for a version whose release request already merged:

1. Turn off the registry's require-trusted-publishing enforcement. While it is on, the registry rejects every token publish, so the manual path cannot succeed. Its failure message points at credentials, not at the switch.
2. Publish with a token scoped like the bootstrap one: new versions of exactly this package, and the shortest expiry.
3. Revoke the token immediately after.
4. Turn enforcement back on. The next automated release then proves that the OIDC path still works.

A hand-published artifact carries no provenance. The run that builds mints the signature. So a file uploaded by hand, even the exact file CI built, leaves the attestation lookup with no answer. A consumer that verifies before it installs refuses that one version, while the releases on either side of it install. Treat the manual upload as temporary. Once CI returns, re-run the artifact workflow on the same tag. The normal path then rebuilds and attests the artifacts.

The tag still comes from automation once CI returns. To repair a hand-published version with no tag, re-run the release workflow on `master`. Never tag it by hand.

## The artifact build failed

On the rust pair, the publish and the artifact build are separate workflows. So a failed artifact build leaves a published version with an empty release page. Re-run the artifact workflow on the same tag. Nothing about the published version changes. The artifacts attach when the build succeeds, and the re-run mints the provenance that the failed one never produced.

The bash and python pipelines build before they publish. So a failed build leaves no published version, and the section on a merged request that published nothing applies.

## An integration left its seat off its branch

The gate of `rk integrate` points the seat's `HEAD` at the trunk commit while the `pre-merge-commit` hook runs. It points `HEAD` back at the branch on every ordinary exit. In each case below, the branch ref never moves, and nothing reaches the trunk. The next `rk integrate` of that branch refuses until the seat is on its branch again.

A signal that stops the command while the hook runs leaves `HEAD` at the trunk commit, with the branch's tree staged. To repair it, run `git -C <seat> symbolic-ref HEAD refs/heads/<branch>`. Then integrate again.

A hook that commits leaves `HEAD` at the hook's own commit, and the gate refuses. The index and the files then hold the tree of that commit. The repair below leaves the commit on no ref, so keep it first:

1. Read what the hook left in the seat.
2. To keep the hook's commit, run `git -C <seat> branch <name> HEAD`.
3. Run `git -C <seat> symbolic-ref HEAD refs/heads/<branch>`.
4. Run `git -C <seat> reset -q --hard`. It restores the index and the files to the branch tip. The hook's work stays on the branch that step 2 made.
5. Make sure that `git -C <seat> status --porcelain` prints nothing.
6. Integrate again.

A hook that stages a change leaves the index changed, and the gate refuses. To repair it:

1. Read what the hook staged in the seat.
2. Run `git -C <seat> symbolic-ref HEAD refs/heads/<branch>`.
3. Run `git -C <seat> reset -q` to restore the index to the branch tip.
4. Commit or discard each change that the hook left in the files.
5. Make sure that `git -C <seat> status --porcelain` prints nothing.
6. Integrate again.
