# Release

Day-to-day release of this repository. First time here: [setup.md](./setup.md).

`Cargo.toml` is the version source of truth. release-plz reads Conventional Commits, bumps the version, writes the changelog, tags, and publishes. Never author a tag, and never move a published one — a bad release is fixed by the next version.

A release takes two pull requests. The first, which release-plz opens against `develop`, carries the version bump and the changelog, and merging it publishes nothing. The second is the gate: automation cuts `release/v<version>` at that merged commit and opens it into `master`, which takes no direct push and requires a passing `test` check. Merging the gate is what tags and publishes. The gate branch is pinned to one commit, so work landing on `develop` while it is open never joins the release.

## At a glance

```bash
# 1. land the work
just check

# 2. push; release-plz opens the release request
git push origin develop

# 3. read the changelog in that request, and correct it on its branch before merging

# 4. merge the release request; the bump lands on develop
gh pr merge <release pr> --repo gubasso/release-kit --squash --delete-branch

# 5. the gate opens itself
gh pr list --repo gubasso/release-kit --base master --state open

# 6. merge the gate; this tags and publishes
gh pr checks <gate pr> --repo gubasso/release-kit --watch \
  && gh pr merge <gate pr> --repo gubasso/release-kit --merge --delete-branch

# 7. back-merge
git fetch origin --tags --force \
  && git checkout develop && git merge --ff-only origin/master && git push origin develop

# 8. wait for the build that creates the release page
gh run watch --repo gubasso/release-kit --exit-status <artifact run>

# 9. verify
```

Two steps are easy to skip and both bite. Step 3 is the last point a changelog correction reaches the release, and release-plz does not regenerate the entry as later work lands. Step 8 is why a check straight after the gate merge reports the release as not found: cargo-dist creates it after every platform builds, some minutes later.

## 1. Land the work

Release intent captured in the commit messages — `feat:` bumps minor, `fix:` bumps patch — the check suite green, and the package still publishable.

```bash
just check
cargo publish --dry-run --locked      # or: rk setup step package-check --target .
```

## 2. Push

release-plz opens or refreshes the release pull request against `develop`.

```bash
git push origin develop
# check: run succeeded, release pull request is open
gh run list --repo gubasso/release-kit --workflow release-plz.yml --limit 1
gh pr list --repo gubasso/release-kit --state open
```

## 3. Correct the changelog

Read the `CHANGELOG.md` entry in the release pull request and confirm it names every change the release carries. release-plz writes that entry when it opens the request and does not regenerate it as later work lands, so a request left open while commits keep landing ships a changelog that omits the newer work. Compare it against the range:

```bash
git fetch origin --tags --force
git log --oneline "v<previous version>^{commit}..origin/develop"
```

Correct it on the release pull request branch, before merging. That is the last point a correction reaches the release: merging cuts the gate at the merge commit, the gate branch stays pinned there, and a published entry can never be edited.

```bash
git fetch origin <release branch> && git switch --detach FETCH_HEAD
# edit CHANGELOG.md, then commit
git push origin HEAD:<release branch>
```

A correction pushed to that branch survives later commits, because release-plz refreshes the request by force-pushing onto the same branch.

## 4. Merge the release request

The bump and the changelog land on `develop`; nothing is published.

```bash
gh pr merge <release pr> --repo gubasso/release-kit --squash --delete-branch
```

## 5. Wait for the gate

That merge pushes `develop`, which now carries a version no tag names. The `open-release-gate` job cuts `release/v<version>` at the merged commit and opens it into `master`.

```bash
# check: a pull request titled "release v<version>", base master, head release/v<version>
gh pr list --repo gubasso/release-kit --base master --state open
```

If the job failed transiently and no gate appeared, re-running it from the Actions interface replays the same commit; that is the recovery path.

## 6. Merge the gate

Once its checks are green, and as a merge commit. `master-protection` refuses the merge on its own while `test` is failing, and `gh pr checks --watch` blocks until every check settles and exits non-zero on a failure. GitHub offers no fast-forward merge method, and a rebase or squash would make `master` diverge from `develop` permanently. This merge tags `v<version>` on `master`, publishes over OIDC, and starts the installer build.

```bash
gh pr checks <gate pr> --repo gubasso/release-kit --watch \
  && gh pr merge <gate pr> --repo gubasso/release-kit --merge --delete-branch
# check: post-merge run on master succeeded
gh run list --repo gubasso/release-kit --workflow release-plz.yml --limit 1
```

## 7. Back-merge

So `develop` reaches the tagged commit and the next release diffs cleanly. It is a fast-forward while `develop` has not moved since the gate was cut; if work landed meanwhile, drop `--ff-only` and take the merge commit.

```bash
git fetch origin --tags --force
git checkout develop && git merge --ff-only origin/master
git push origin develop
```

## 8. Wait for the artifact build

cargo-dist creates the GitHub release in its final job, after every platform has built, so for some minutes after the merge there is no release to look at and `gh release view` reports that it is not found. The tag lands sooner, around a minute in.

```bash
gh run watch --repo gubasso/release-kit --exit-status \
  "$(gh run list --repo gubasso/release-kit --workflow release.yml \
     --limit 1 --json databaseId -q '.[0].databaseId')"
```

## 9. Verify

```bash
# crates.io serves the new version
cargo info release-kit

# installers attached, never empty
gh release view v<version> --repo gubasso/release-kit --json assets \
  -q '[.assets[].name] | join(", ")'

# the local clone may predate the tag push
git fetch origin --tags --force

# all three agree
git rev-parse "v<version>^{commit}" origin/master origin/develop

# the installed binary reports it
rk --version
```

release-plz writes an annotated tag, so `v<version>` names a tag object rather than a commit; `^{commit}` is what makes the three values comparable.

## Worked example

One pass, releasing `v0.2.0` over `v0.1.0`. The placeholders above are resolved; the output lines are what each check prints when the step went right.

```bash
$ just check && git push origin develop
To https://github.com/gubasso/release-kit.git
   3b81e07..a91c2e0  develop -> develop

$ gh pr list --repo gubasso/release-kit --state open
12  chore: release v0.2.0  release-plz-2026-08-30T12-04-11Z  OPEN

$ git fetch origin --tags --force
$ git log --oneline "v0.1.0^{commit}..origin/develop"
a91c2e0 feat(guides): Land the manual-first release guides
3b81e07 test(distribution): Enforce the leak rule the spec already claimed
```

The entry in pull request 12 names both commits, so nothing needs correcting. Merge it:

```bash
$ gh pr merge 12 --repo gubasso/release-kit --squash --delete-branch

$ gh pr list --repo gubasso/release-kit --base master --state open
13  release v0.2.0  release/v0.2.0  OPEN
```

Pull request 13 is the gate, pinned at the merge commit. Merging it is the release:

```bash
$ gh pr checks 13 --repo gubasso/release-kit --watch
test  pass  2m14s
$ gh pr merge 13 --repo gubasso/release-kit --merge --delete-branch

$ git fetch origin --tags --force
$ git checkout develop && git merge --ff-only origin/master && git push origin develop
Updating a91c2e0..4f7b1d3
```

Then wait for the installers, and verify:

```bash
$ gh run watch --repo gubasso/release-kit --exit-status \
    "$(gh run list --repo gubasso/release-kit --workflow release.yml \
       --limit 1 --json databaseId -q '.[0].databaseId')"
✓ release · 4f7b1d3   Run release completed with 'success'

$ cargo info release-kit
release-kit #cli #release
version: 0.2.0

$ gh release view v0.2.0 --repo gubasso/release-kit --json assets \
    -q '[.assets[].name] | join(", ")'
release-kit-aarch64-apple-darwin.tar.xz, release-kit-x86_64-unknown-linux-gnu.tar.xz, ...

$ git rev-parse "v0.2.0^{commit}" origin/master origin/develop
4f7b1d3e8a9c2b5f1d4e7a0c3b6f9e2d5a8c1b4e
4f7b1d3e8a9c2b5f1d4e7a0c3b6f9e2d5a8c1b4e
4f7b1d3e8a9c2b5f1d4e7a0c3b6f9e2d5a8c1b4e
```

Three identical shas is the release. Anything else means step 7 did not land.

## The automated forms

`rk guide release` prints the technology- and forge-agnostic form of this page, filled in from detection. The `rk-release` skill drives the same nine steps for a coding agent. The recovery paths — a stuck gate, a bad published changelog, a yank, a hand-publish while CI is down — are `rk method recovery`.
