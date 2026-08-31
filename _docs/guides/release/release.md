# Release

Day-to-day release of this repository. First time here: [setup.md](./setup.md).

`Cargo.toml` is the version source of truth. release-plz reads Conventional Commits on `master`, maintains one release pull request carrying the bump and the changelog, and merging that PR is the release: the bump push tags and publishes. Never author a tag, and never move a published one — a bad release is fixed by the next version.

## At a glance

```bash
# 1. land the work through squash-merged PRs; master stays releasable
just check

# 2. the bot maintains the release PR against master
gh pr list --repo gubasso/release-kit --state open

# 3. read the changelog on that PR; correct it on its branch if needed

# 4. merge the release PR; this is the release
gh pr checks <release pr> --repo gubasso/release-kit --watch \
  && gh pr merge <release pr> --repo gubasso/release-kit --squash --delete-branch

# 5. wait for the installer build
gh run watch --repo gubasso/release-kit --exit-status <artifact run>

# 6. verify
```

Two timing traps. Step 3: the bot refreshes the PR by force-push while only its own commits sit on the branch, and closes-and-reopens the PR when a human commit lands there — so correct the changelog last, or expect to re-apply the correction. Step 5: cargo-dist creates the release page after every platform builds, so a check straight after the merge reports the release as not found for some minutes.

## 1. Land the work

Release intent captured in the squash titles — `feat:` bumps minor, `fix:` bumps patch — the check suite green, and the package still publishable.

```bash
just check
cargo publish --dry-run --locked
```

## 2. Watch the release PR

Every push to `master` refreshes it: the proposed version and the regenerated changelog entry always reflect the trunk's tip.

```bash
gh pr list --repo gubasso/release-kit --state open
```

## 3. Correct the changelog

Compare the entry against the commit range, and correct it on the PR's branch if it misses anything:

```bash
git fetch origin --tags --force
git log --oneline "v<previous version>^{commit}..origin/master"
```

A human commit on the bot's branch makes the next refresh close and reopen the PR, dropping the correction — so this is the last step before merging, not an early one.

## 4. Merge the release PR

This is the release decision. `test` gates the merge; squash is the only allowed method, so `master` stays linear.

```bash
gh pr checks <release pr> --repo gubasso/release-kit --watch \
  && gh pr merge <release pr> --repo gubasso/release-kit --squash --delete-branch
```

The bump push runs `release-plz-release`: publish `<version>` to crates.io over OIDC, then push annotated tag `v<version>` with the App token. The tag starts `release.yml`.

## 5. Wait for the artifact build

```bash
gh run watch --repo gubasso/release-kit --exit-status \
  "$(gh run list --repo gubasso/release-kit --workflow release.yml \
     --limit 1 --json databaseId -q '.[0].databaseId')"
```

## 6. Verify

```bash
cargo info release-kit
gh release view v<version> --repo gubasso/release-kit --json assets \
  -q '[.assets[].name] | join(", ")'
git fetch origin --tags --force
git rev-parse "v<version>^{commit}" origin/master
rk --version
```

Two identical SHAs is the release: the tag and the trunk name the same commit. Anything else means the tag push failed; re-run the release workflow on `master`.

## Worked example

One pass, releasing v0.2.0 over v0.1.0.

```bash
$ gh pr merge 41 --repo gubasso/release-kit --squash --delete-branch   # the feat PR
$ gh pr list --repo gubasso/release-kit --state open
42  chore: release v0.2.0  release-plz-2026-09-02T10-11-04Z  OPEN

$ git fetch origin --tags --force
$ git log --oneline "v0.1.0^{commit}..origin/master"
a91c2e0 feat(guides): Land the manual-first release guides
3b81e07 test(distribution): Enforce the leak rule the spec already claimed
```

The entry on PR 42 names both commits; nothing to correct. Merge it:

```bash
$ gh pr checks 42 --repo gubasso/release-kit --watch
test  pass  2m14s
$ gh pr merge 42 --repo gubasso/release-kit --squash --delete-branch

$ gh run watch --repo gubasso/release-kit --exit-status \
    "$(gh run list --repo gubasso/release-kit --workflow release.yml \
       --limit 1 --json databaseId -q '.[0].databaseId')"
✓ release · c3d8e10   Run release completed with 'success'

$ cargo info release-kit
version: 0.2.0
$ git rev-parse "v0.2.0^{commit}" origin/master
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d
```

## Backport: a patch for an older line

Only when someone on an older version needs a fix without the newer work. This is style B of the method; the walkthrough chapter owns the reasoning.

```bash
# 1. the line's branch, cut retroactively from its tag if it does not exist
git checkout -b release/0.2 v0.2.0 && git push -u origin release/0.2

# 2. fix on master first: test + fix, PR, squash-merge -> commit U

# 3. cherry-pick only the fix
git checkout release/0.2 && git cherry-pick U && git push

# 4. CI verifies the branch; the bump lands on the branch and automation tags v0.2.1
# 5. verify, then delete the branch when the line dies -- the tag keeps the commits
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
