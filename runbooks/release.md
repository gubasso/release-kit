# Release runbook

The nine steps of [operate](../method/03-operate.md) as commands. The reasoning stays in the chapter; this page is what a person follows with a gate open. `<repo>` is the project path, filled in by `rk guide release` where detection resolves it. `<release pr>`, `<gate pr>`, and `<artifact run>` exist only once a bot has opened them, change every release, and are never substituted.

## At a glance

On github:

```bash
# 1. land the work
just check                                    # or the binding's check command
rk setup step package-check --target .
# 2. push; the bot opens the release request
git push origin develop
# 3. read the changelog in that request; correct it on its branch before merging
# 4. merge the release request; the bump lands on develop
gh pr merge <release pr> --repo <repo> --squash --delete-branch
# 5. the gate opens itself
gh pr list --repo <repo> --base master --state open
# 6. merge the gate; this tags and publishes
gh pr checks <gate pr> --repo <repo> --watch \
  && gh pr merge <gate pr> --repo <repo> --merge --delete-branch
# 7. back-merge
git fetch origin --tags --force \
  && git merge --ff-only origin/master && git push origin develop
# 8. wait for the artifact build
gh run watch --repo <repo> --exit-status <artifact run>
# 9. verify
```

On gitlab:

```bash
# 1. land the work
just check                                    # or the binding's check command
rk setup step package-check --target .
# 2. push; the bot opens the release request
git push origin develop
# 3. read the changelog in that request; correct and merge with nothing landing in between
# 4. merge the release request; the bump lands on develop
glab mr merge <release mr> --squash --remove-source-branch
# 5. the gate opens itself
glab mr list --target-branch master
# 6. merge the gate; this tags and publishes
glab ci status --wait \
  && glab mr merge <gate mr> --remove-source-branch
# 7. back-merge
git fetch origin --tags --force \
  && git merge --ff-only origin/master && git push origin develop
# 8. wait for the release pipeline
glab ci status --wait
# 9. verify
```

Two steps are easy to skip, and the chapter's [own warnings](../method/03-operate.md) explain both. Step 3 is the last point a changelog correction reaches the release. Step 8 is why a check run straight after the gate merge reports the release as missing under a dedicated artifact builder: the release page arrives only when the slowest platform build finishes.

## 1. Land the work

Release intent captured in the commit messages, the local check suite green, and the package still publishable — packaging breaks long after setup, and a failure found here costs seconds where the same failure after the gate merge costs the recovery chapter.

```bash
just check                                    # or the binding's check command
rk setup step package-check --target .
```

## 2. Push

The bot opens or refreshes the release request against `develop`: the version bump and the changelog entry, publishing nothing.

On github:

```bash
git push origin develop
gh pr list --repo <repo> --state open         # check: the release request is open
```

On gitlab:

```bash
git push origin develop
glab mr list                                  # check: the release request is open
```

## 3. Correct the changelog

Read the entry in the release request and compare it against the commit range since the previous tag; the bot writes the entry when it opens the request and does not regenerate it as later work lands.

```bash
git fetch origin --tags --force
git log --oneline "v<previous version>^{commit}..origin/develop"
```

Correct it on the request's branch before merging; this is the last point a correction reaches the release.

On github:

A correction pushed to the request's branch survives later commits, because the bot refreshes the request by force-pushing onto the same branch.

On gitlab:

The bot cannot force-push a refresh here: when new work lands it closes the open request and opens a fresh one, and a correction on the closed request goes with it. Correct and merge, with nothing landing in between.

## 4. Merge the release request

The bump and the changelog land on `develop`; nothing is published.

On github:

```bash
gh pr merge <release pr> --repo <repo> --squash --delete-branch
```

On gitlab:

```bash
glab mr merge <release mr> --squash --remove-source-branch
```

## 5. Wait for the gate

The merge leaves `develop` carrying a version no tag names, so automation cuts `release/v<version>` at the merged commit and opens it into `master`.

On github:

```bash
gh pr list --repo <repo> --base master --state open   # check: a request titled "release v<version>"
```

On gitlab:

```bash
glab mr list --target-branch master                   # check: a request titled "release v<version>"
```

## 6. Merge the gate

Once its checks are green, as a merge commit. This is the release: merging tags `v<version>` on `master` and runs the binding's publish path.

On github:

```bash
gh pr checks <gate pr> --repo <repo> --watch \
  && gh pr merge <gate pr> --repo <repo> --merge --delete-branch
```

On gitlab:

```bash
glab ci status --wait \
  && glab mr merge <gate mr> --remove-source-branch
```

## 7. Back-merge

So `develop` reaches the tagged commit and the next release diffs cleanly. A fast-forward while `develop` has not moved; drop `--ff-only` and take the merge commit when work landed meanwhile.

```bash
git fetch origin --tags --force
git checkout develop && git merge --ff-only origin/master
git push origin develop
```

## 8. Wait for the artifact build

Where the binding runs a dedicated artifact builder, it creates the release page in its final job, after every platform builds; for some minutes after the merge there is no release to look at.

On github:

```bash
gh run watch --repo <repo> --exit-status <artifact run>
```

On gitlab:

```bash
glab ci status --wait
```

The `(rust, gitlab)` pair has no artifact builder, so there is no dedicated build to wait for and no installers to expect on the release page; [the Rust binding](../bindings/rust.md) states it.

## 9. Verify

Universally: the tag and `master` resolve to the same commit, and that commit is an ancestor of `develop`. The bot writes an annotated tag, so `v<version>` names a tag object; `^{commit}` is what makes the three values comparable.

```bash
git fetch origin --tags --force
git rev-parse "v<version>^{commit}" origin/master origin/develop
```

Then per binding, each check applying only where the binding declares that surface: a registry serves the new version, a release page carries its artifacts and is not empty, an installed binary reports the new version.
