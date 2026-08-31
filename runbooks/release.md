# Release runbook

The six steps of [operate](../method/03-operate.md) as commands. The reasoning stays in the chapter; this page is what a person follows with a release request open. `<repo>` is the project path, filled in by `rk guide release` where detection resolves it. `<release pr>` and `<artifact run>` exist only once a bot has opened them, change every release, and are never substituted.

## At a glance

On github:

```bash
# 1. land the work through squash-merged pull requests
just check                                    # or the binding's check command
rk setup step package-check --target .
# 2. read the release request the bot keeps open
gh pr list --repo <repo> --state open
# 3. correct the changelog on its branch, last
# 4. merge the release request; this is the release
gh pr checks <release pr> --repo <repo> --watch \
  && gh pr merge <release pr> --repo <repo> --squash --delete-branch
# 5. wait for the artifact build
gh run watch --repo <repo> --exit-status <artifact run>
# 6. verify
```

On gitlab:

```bash
# 1. land the work through squash-merged merge requests
just check                                    # or the binding's check command
rk setup step package-check --target .
# 2. read the release request the bot keeps open
glab mr list
# 3. correct the changelog on its branch, then merge with nothing landing in between
# 4. merge the release request; this is the release
glab ci status --wait \
  && glab mr merge <release mr> --squash --remove-source-branch
# 5. wait for the release pipeline
glab ci status --wait
# 6. verify
```

Two steps are easy to skip, and the chapter's [own warnings](../method/03-operate.md) explain both. Step 3 is the last point a changelog correction reaches the release, and a correction does not survive a later refresh. Step 5 is why a check run straight after the merge reports the release as missing under a dedicated artifact builder: the release page arrives only when the slowest platform build finishes.

## 1. Land the work

Release intent captured in the squash titles, the local check suite green, and the package still publishable — packaging breaks long after setup, and a failure found here costs seconds where the same failure after the merge costs the recovery chapter.

```bash
just check                                    # or the binding's check command
rk setup step package-check --target .
```

## 2. Read the release request

The bot keeps one request open against `master`: the version bump and the changelog entry, publishing nothing. Every merged pull request refreshes it, so the proposed version and the entry describe the trunk's tip.

On github:

```bash
gh pr list --repo <repo> --state open         # check: the release request is open
```

On gitlab:

```bash
glab mr list                                  # check: the release request is open
```

## 3. Correct the changelog

Read the entry in the release request and compare it against the commit range since the previous tag.

```bash
git fetch origin --tags --force
git log --oneline "v<previous version>^{commit}..origin/master"
```

Correct it on the request's branch, last: a correction does not survive a later refresh, so nothing lands between the correction and the merge.

On github:

While only the bot's own commits sit on the request's branch, it refreshes by force-push; once a correction is pushed there, the next refresh closes the request and opens a fresh one, and the correction goes with it.

On gitlab:

The bot cannot force-push a refresh here: whenever new work lands it closes the open request and opens a fresh one, and a correction on the closed request goes with it.

## 4. Merge the release request

This is the release decision. The named check gates the merge, and squash is the only allowed method, so `master` stays linear.

On github:

```bash
gh pr checks <release pr> --repo <repo> --watch \
  && gh pr merge <release pr> --repo <repo> --squash --delete-branch
```

On gitlab:

```bash
glab ci status --wait \
  && glab mr merge <release mr> --squash --remove-source-branch
```

The push that lands the bump runs the binding's release path: automation tags `v<version>` and publishes, and the tag starts the artifact build where the binding has one.

## 5. Wait for the artifact build

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

## 6. Verify

Universally: the tag and `master` resolve to the same commit. The bot writes an annotated tag, so `v<version>` names a tag object; `^{commit}` is what makes the two values comparable.

```bash
git fetch origin --tags --force
git rev-parse "v<version>^{commit}" origin/master
```

Then per binding, each check applying only where the binding declares that surface: a registry serves the new version, a release page carries its artifacts and is not empty, an installed binary reports the new version.

## An older line

A patch-only release for users on an older version is not this sequence. [Branch for release](../method/07-branch-for-release.md) owns the path: fix on the trunk first, cherry-pick to the line's branch, and let the same automation tag there; [recovery](../method/04-recovery.md) carries the entry point for a line whose branch does not exist yet.
