# Release runbook

The six steps of [operate](../method/03-operate.md) as commands: the chapter owns each step's why, this page owns its how. This is what a person follows with a release request open. `<repo>` is the project path, filled in by `rk guide release` where detection resolves it. `<release pr>` and `<release mr>` exist only once a bot has opened them, change every release, and are never substituted: a stale number merges someone else's work, where a visible placeholder fails loudly. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

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
# 5. wait for the publish workflow on the merge commit, then the artifact workflow on its tag
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
# 6. verify
```

Three traps, and the chapter's [own warnings](../method/03-operate.md) explain the first two. Step 3 is the last point a changelog correction reaches the release, and a correction does not survive a later refresh. Step 5 is why a check run straight after the merge reports the release as missing under a dedicated artifact builder: the release page arrives only when the slowest platform build finishes. And where the artifact workflow also runs on pull requests, its newest run is usually not this release's: select runs by commit, never by recency.

## 1. Land the work

Release intent captured in the squash titles, the local check suite green, and the package still publishable — a failure found here costs seconds where the same failure after the merge costs the recovery chapter. Where the check runs a pre-commit sweep from the trunk's checkout, name the commit-time branch guard out of it: `SKIP=no-commit-to-branch`, the same form the landed hook block's comment gives a CI sweep.

```bash
just check                                    # or the binding's check command
rk setup step package-check --target .
# check: both exit 0; the trunk is releasable
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

Correct on the request's branch, last, with nothing landing between the correction and the merge; the chapter owns why an early correction is dropped, and the forge document owns how the bot refreshes the request.

### 3a. Compare the entry against the range

```bash
git fetch origin --tags --force
git log --oneline "v<previous version>^{commit}..origin/master"
# check: every commit that should appear in the entry is listed
```

Nothing missing: skip to step 4. Something missing: continue.

### 3b. Correct it on the request's branch

On github:

```bash
gh pr checkout <release pr> --repo <repo>
```

On gitlab:

```bash
glab mr checkout <release mr>
```

Then edit the changelog and push the correction:

```bash
git commit -am "docs(changelog): Complete the entry for v<version>"
git push
```

### 3c. Confirm the request survived

On github:

```bash
gh pr view <release pr> --repo <repo> --json state,number -q '.state, .number'
# check: still OPEN and the same number
```

On gitlab:

```bash
glab mr view <release mr>
# check: still open under the same number
```

A new number means the bot reopened the request and took the fix: redo the correction on the new request and merge without waiting.

## 4. Merge the release request

This is the release decision. The named check gates the merge, and squash is the only allowed method, so `master` stays linear.

### 4a. Wait for the checks, then merge

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

### 4b. Bind the merge commit

Steps 5 and 6 correlate against it.

```bash
git fetch origin && git rev-parse origin/master
# check: prints the SHA the release runs on
```

The push that lands the bump runs the binding's release path: automation tags `v<version>` and publishes, and the tag starts the artifact build where the binding has one.

## 5. Wait for the artifact build

Where the binding runs a dedicated artifact builder, the release page arrives only when its final job finishes; each wait selects its run by what it ran on, never by recency.

On github:

```bash
SHA="$(git rev-parse origin/master)"
gh run watch --repo <repo> --exit-status \
  "$(gh run list --repo <repo> --workflow <publish workflow> \
     --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
gh run watch --repo <repo> --exit-status \
  "$(gh run list --repo <repo> --workflow <artifact workflow> \
     --event push --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
# check: each completes with 'success'; the binding names both workflow files
# empty second id: the tag has not landed yet; rerun after a few seconds, and if it stays empty go to 6a, which names the failure
```

On gitlab:

```bash
glab ci status --wait
```

The `--event push` filter keeps the second watch off the runs the artifact workflow also produces on pull requests. The `(rust, gitlab)` pair has no artifact builder, so there is no dedicated build to wait for and no installers to expect on the release page; [the Rust binding](../bindings/rust.md) states it.

## 6. Verify

### 6a. The tag and the trunk name the same commit

Universal, and the release itself: the bot writes an annotated tag, so `v<version>` names a tag object, and `^{commit}` is what makes the two values comparable.

```bash
git fetch origin --tags --force
git cat-file -t "v<version>"
# check: prints tag
git rev-parse "v<version>^{commit}" origin/master
# check: two identical SHAs
```

### 6b. The registry serves exactly this version

On rust:

```bash
cargo info <crate> | grep -m1 -i '^version'
# check: prints <version>
```

### 6c. The release page carries its artifacts

On github:

```bash
gh release view v<version> --repo <repo> --json assets \
  -q '[.assets[].name] | join(", ")'
# check: the artifact list is not empty, where the binding declares that surface
```

### 6d. An installed binary reports the new version

On github:

```bash
curl -LsSf "https://github.com/<repo>/releases/download/v<version>/<crate>-installer.sh" | sh
# check: the installed binary reports <version>; a bare version call only reads whatever is already on PATH
```

### 6e. Repair a failed tag push

Anything but 6a's two identical SHAs means the tag push failed: rerun the publish workflow on the merge commit — never the artifact workflow, which only builds from a tag that already exists.

On github:

```bash
gh run rerun --repo <repo> --failed \
  "$(gh run list --repo <repo> --workflow <publish workflow> \
     --commit "$(git rev-parse origin/master)" --limit 1 --json databaseId -q '.[0].databaseId')"
```

## An older line

A patch-only release for users on an older version is not this sequence: the command form is `rk guide backport`, and [branch for release](../method/07-branch-for-release.md) owns the path's why; [recovery](../method/04-recovery.md) carries the entry point for a line whose branch does not exist yet.
