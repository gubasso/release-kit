# Release

Day-to-day release of a repository carrying the release-kit workflow. First time here: [setup.md](./setup.md).

## Coordinates

Export these once; every command below reads them. [README.md](./README.md) says what each one is.

```bash
export OWNER=<account or organization that owns the repository>
export REPO=<repository name>
export CRATE=<package name as published to the registry>
```

Everything else appears literally, because the convention fixes it: the trunk is `master`, the release lines are `release/*`, the required check is `test`, the publish workflow is `release-plz.yml`, and the artifact workflow is `release.yml`.

`Cargo.toml` is the version source of truth. release-plz reads Conventional Commits on the trunk, maintains one release pull request carrying the bump and the changelog, and merging that PR is the release: the bump push tags and publishes. Never author a tag, and never move a published one — a bad release is fixed by the next version.

## At a glance

```bash
# 1. land the work through squash-merged PRs; the trunk stays releasable
just check

# 2. the bot maintains the release PR against the trunk
gh pr list --repo "$OWNER/$REPO" --state open

# 3. read the changelog on that PR; correct it on its branch if needed

# 4. merge the release PR; this is the release
gh pr checks <release pr> --repo "$OWNER/$REPO" --watch \
  && gh pr merge <release pr> --repo "$OWNER/$REPO" --squash --delete-branch

# 5. wait for release-plz.yml on the merge commit, then release.yml on the tag it pushed

# 6. verify the version, the assets, the annotated tag, and two equal SHAs
```

Three traps. Step 3: the bot refreshes the PR by force-push while only its own commits sit on the branch, and closes-and-reopens it when a human commit lands there — so correct the changelog last, or expect to re-apply the correction. Step 5: `release.yml` also runs on pull requests, so its newest run is usually not this release's; select by commit. Step 5 again: cargo-dist creates the release page after every platform builds, so a check straight after the merge reports the release as not found for some minutes.

## 1. Land the work

Release intent captured in the squash titles — `feat:` bumps minor, `fix:` bumps patch — the check suite green, and the package still publishable. The trunk takes no direct push, so each change reaches it the way [setup.md](./setup.md) step 10 lays out.

```bash
just check
cargo publish --dry-run --locked
# check: both exit 0; the trunk is releasable
```

## 2. Watch the release PR

Every push to the trunk refreshes it: the proposed version and the regenerated changelog entry always reflect the trunk's tip.

```bash
gh pr list --repo "$OWNER/$REPO" --state open
# check: a release request from release-plz is listed, proposing the next version
# none listed: nothing releasable has landed since the last release, or release-plz.yml has not run yet
```

## 3. Correct the changelog

1. Compare the entry against the commit range.

   ```bash
   git fetch origin --tags --force
   git log --oneline "v<previous version>^{commit}..origin/master"
   # check: every commit that should appear in the entry is listed
   ```

   Nothing missing: skip to step 4. Something missing: continue.

2. Check the PR's branch out.

   ```bash
   gh pr checkout <release pr> --repo "$OWNER/$REPO"
   ```

3. Edit `CHANGELOG.md`, then push the correction.

   ```bash
   git commit -am "docs(changelog): Complete the entry for v<version>"
   git push
   ```

4. Confirm the bot did not take the PR out from under the fix.

   ```bash
   gh pr view <release pr> --repo "$OWNER/$REPO" --json state,number -q '.state, .number'
   # check: still OPEN and the same number; a new number means the bot reopened it and took the fix
   ```

A human commit on the bot's branch makes the next refresh close and reopen the PR, dropping the correction — so this is the last step before merging, not an early one. Reopened means redo the correction on the new PR and merge without waiting.

## 4. Merge the release PR

This is the release decision. `test` gates the merge; squash is the only allowed method, so the trunk stays linear.

1. Wait for the check, then merge on its success.

   ```bash
   gh pr checks <release pr> --repo "$OWNER/$REPO" --watch \
     && gh pr merge <release pr> --repo "$OWNER/$REPO" --squash --delete-branch
   ```

2. Note the merge commit; steps 5 and 6 correlate against it.

   ```bash
   git fetch origin && git rev-parse origin/master
   # check: prints the SHA the release runs on
   ```

The bump push runs `release-plz-release`: publish `<version>` to crates.io over OIDC, then push annotated tag `v<version>` with the App token. The tag starts `release.yml`.

## 5. Wait for the publish, then for the artifact build

Two workflows in sequence, and each one has to be selected by what it ran on. `release.yml` also runs on pull requests, so the newest run of it is often not this release's.

1. Bind the merge commit both waits select on.

   ```bash
   SHA="$(git rev-parse origin/master)"
   ```

2. Wait for the publish and the tag push, on that commit.

   ```bash
   gh run watch --repo "$OWNER/$REPO" --exit-status \
     "$(gh run list --repo "$OWNER/$REPO" --workflow release-plz.yml \
        --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
   # check: completes with 'success'
   ```

3. Wait for the installer build, on the tag that publish pushed.

   ```bash
   gh run watch --repo "$OWNER/$REPO" --exit-status \
     "$(gh run list --repo "$OWNER/$REPO" --workflow release.yml \
        --event push --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
   # check: completes with 'success'
   ```

   An empty id here means the tag has not landed yet: rerun the command after a few seconds, and if it stays empty go to step 6's tag check, which names the failure.

## 6. Verify

1. The registry serves exactly this version.

   ```bash
   cargo info "$CRATE" | grep -m1 -i '^version'
   # check: prints <version>
   ```

2. The release page carries the installers.

   ```bash
   gh release view v<version> --repo "$OWNER/$REPO" --json assets \
     -q '[.assets[].name] | join(", ")'
   # check: the shell and powershell installers and the five target archives
   ```

3. The tag is annotated, not lightweight.

   ```bash
   git fetch origin --tags --force
   git cat-file -t "v<version>"
   # check: prints tag
   ```

4. The tag and the trunk name the same commit.

   ```bash
   git rev-parse "v<version>^{commit}" origin/master
   # check: two identical SHAs
   ```

5. The published installer serves the new binary.

   ```bash
   curl -LsSf "https://github.com/$OWNER/$REPO/releases/download/v<version>/$CRATE-installer.sh" \
     | sh && rk --version
   # check: prints <version>; a bare rk --version only reads whatever is already on PATH
   ```

Two identical SHAs is the release. Anything else means the tag push failed: rerun `release-plz.yml` on the merge commit, which is the workflow that publishes and tags — never `release.yml`, which only builds from a tag that already exists.

```bash
gh run rerun --repo "$OWNER/$REPO" \
  "$(gh run list --repo "$OWNER/$REPO" --workflow release-plz.yml \
     --commit "$(git rev-parse origin/master)" --limit 1 --json databaseId -q '.[0].databaseId')" --failed
```

## Worked example

One pass, releasing v0.2.0 over v0.1.0.

```bash
$ gh pr merge 41 --repo "$OWNER/$REPO" --squash --delete-branch   # the feat PR
$ gh pr list --repo "$OWNER/$REPO" --state open
42  chore: release v0.2.0  release-plz-2026-09-02T10-11-04Z  OPEN

$ git fetch origin --tags --force
$ git log --oneline "v0.1.0^{commit}..origin/master"
a91c2e0 feat(guides): Land the manual-first release guides
3b81e07 test(distribution): Enforce the leak rule the spec already claimed
```

The entry on PR 42 names both commits; nothing to correct. Merge it:

```bash
$ gh pr checks 42 --repo "$OWNER/$REPO" --watch
test  pass  2m14s
$ gh pr merge 42 --repo "$OWNER/$REPO" --squash --delete-branch
$ git fetch origin && SHA="$(git rev-parse origin/master)" && echo "$SHA"
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d

$ gh run watch --repo "$OWNER/$REPO" --exit-status \
    "$(gh run list --repo "$OWNER/$REPO" --workflow release-plz.yml \
       --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
✓ release-plz · c3d8e10   Run release-plz completed with 'success'

$ gh run watch --repo "$OWNER/$REPO" --exit-status \
    "$(gh run list --repo "$OWNER/$REPO" --workflow release.yml \
       --event push --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
✓ Release · c3d8e10   Run Release completed with 'success'

$ cargo info "$CRATE" | grep -m1 -i '^version'
version: 0.2.0
$ git fetch origin --tags --force && git cat-file -t v0.2.0
tag
$ git rev-parse "v0.2.0^{commit}" origin/master
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d
```

The `--event push` filter on the second watch is what keeps it off the pull-request `plan` run `release.yml` also produces.

## Backport: a patch for an older line

Only when someone on an older version needs a fix without the newer work. This is style B of the method; the walkthrough chapter owns the reasoning.

The line is a second trunk, so it needs the same protection, the same release request, and the same verification as the trunk. Only the branch cut and the cherry-pick are new.

1. Cut the line from its tag, if it does not exist yet.

   ```bash
   git fetch origin --tags --force
   git checkout -b release/0.2 v0.2.0 && git push -u origin release/0.2
   # check: the branch is created at the tag and pushed with its upstream set
   # already cut: the checkout refuses the existing branch; git checkout release/0.2 and go to 2
   ```

2. Protect it, once per account, covering every `release/*` line.

   ```bash
   rk setup step protect-release-lines --target . --apply
   # check: reports applied the first time and satisfied after; 3 proves the shape either way
   ```

3. Assert the shape the forge enforces.

   ```bash
   id="$(gh api repos/$OWNER/$REPO/rulesets --jq '.[] | select(.name=="release-lines") | .id')"
   gh api "repos/$OWNER/$REPO/rulesets/$id" --jq '{refs: .conditions.ref_name.include, rules: ([.rules[].type] | sort)}'
   # check: refs ["refs/heads/release/*"], rules deletion non_fast_forward
   ```

4. Fix on the trunk first: test plus fix, one PR, squash-merged. Call the resulting commit U.

5. Cherry-pick only that commit onto the line.

   ```bash
   git checkout release/0.2 && git cherry-pick U && git push
   ```

6. Wait for the line's CI on the pushed commit.

   ```bash
   gh run watch --repo "$OWNER/$REPO" --exit-status \
     "$(gh run list --repo "$OWNER/$REPO" --workflow ci.yml \
        --commit "$(git rev-parse HEAD)" --limit 1 --json databaseId -q '.[0].databaseId')"
   # check: completes with 'success'
   ```

7. Read the line's own release request, which the bot opens because `release-plz.yml` runs on `release/**` too.

   ```bash
   gh pr list --repo "$OWNER/$REPO" --state open --base release/0.2
   ```

8. Run steps 3, 4, 5, and 6 of this page against the line, with `--base release/0.2` and `origin/release/0.2` wherever they name the trunk.

9. Delete the branch when the line dies; the tag keeps the commits.

   ```bash
   git push origin --delete release/0.2
   ```

```text
master:      ──o──o──U──o──>          the fix keeps rolling forward too
                    │
                    └─ cherry-pick
                              ↓
release/0.2:  v0.2.0 ●────────●
                            v0.2.1    only the fix; nothing else moved
```

Never fix on the branch first, never merge the branch back, never reuse it for the next minor.

## The automated forms

`rk guide release` prints the technology- and forge-agnostic form of this page. The `rk-release` skill drives the same steps for a coding agent. Recovery paths are `rk method recovery`.
