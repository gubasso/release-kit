# Release runbook

The six steps of [operate](../method/03-operate.md) as commands: the chapter owns each step's why, this page owns its how. This is what a person follows with a release request open. `<repo>` is the project path, filled in by `rk guide release` where detection resolves it. `<release pr>` and `<release mr>` exist only once a bot has opened them, change every release, and are never substituted: a stale number merges someone else's work, where a visible placeholder fails loudly. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## At a glance

On github:

```bash
# 1. land the work through squash-merged pull requests
just check                                    # or the binding's check command
rk setup step package-check --target . --apply
# 2. read the release request the bot keeps open
gh pr list --repo <repo> --state open
# 3. hold it, or correct the changelog on an unarmed request
gh pr view <release pr> --repo <repo> --json autoMergeRequest
# 4. let it merge; on an unarmed request, merge it
gh pr checks <release pr> --repo <repo> --watch
gh pr merge <release pr> --repo <repo> --squash --delete-branch   # unarmed only
# 5. wait for the publish workflow on the merge commit, then, on rust, the artifact workflow on its tag
# 6. verify
```

On gitlab:

```bash
# 1. land the work through squash-merged merge requests
just check                                    # or the binding's check command
rk setup step package-check --target . --apply
# 2. read the release request the bot keeps open
glab mr list
# 3. hold it, or correct the changelog on an unarmed request
glab mr view <release mr>
# 4. let it merge; on an unarmed request, merge it
glab ci status --wait
glab mr merge <release mr> --squash --remove-source-branch   # unarmed only
# 5. wait for the release pipeline
# 6. verify
```

Four traps, and the chapter's [own warnings](../method/03-operate.md) explain the first three. An armed request has no correction window, and a stop is a disarm run before the last check turns green, never a close raced after it. Step 3 is the last point a changelog correction reaches an unarmed release, and a correction does not survive a later refresh. Step 5 is why a check run straight after the merge reports the release as missing under a dedicated artifact builder: the release page arrives only when the slowest platform build finishes. And where a target left its artifact workflow reporting on pull requests, its newest run is usually not this release's: select runs by commit, never by recency — a target on the current landed workflows has tag runs alone, and `rk status --check` faults the configuration that produces the others.

## 1. Land the work

Release intent captured in the squash titles, each on a change the bot attributes to the package as [the binding](../bindings/README.md) defines, the local check suite green, and the package still publishable — a failure found here costs seconds where the same failure after the merge costs the recovery chapter. Where the check runs a pre-commit sweep from the trunk's checkout, name the commit-time branch guard out of it: `SKIP=no-commit-to-branch`, the same form the landed hook block's comment gives a CI sweep.

```bash
just check                                    # or the binding's check command
rk setup step package-check --target . --apply
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

## 3. Hold, or correct the changelog

The style decides what this step is: on an armed request it is the hold window, on an unarmed one the correction window. The chapter owns why, and the forge document owns how the bot refreshes the request.

### 3a. Read whether the request stands armed

On github:

```bash
gh pr view <release pr> --repo <repo> --json autoMergeRequest \
  -q '.autoMergeRequest.enabledBy.login // "not armed"'
# check: prints the arming login on an armed request, or "not armed"
```

On gitlab:

```bash
glab mr view <release mr>
# check: the view reports auto-merge as enabled, or does not
```

Armed and this release ships: go to 4. Armed and it must not ship yet: 3b. Not armed: 3c.

### 3b. Hold the release

Run it before the last check turns green; the chapter owns why a later stop is a withdrawal instead.

On github:

```bash
gh pr merge <release pr> --repo <repo> --disable-auto
# check: 3a now prints "not armed"; the next bot refresh may re-arm, so say where the team reads that the release is held
# already merged: nothing is held; rk method recovery owns the withdrawal
```

On gitlab:

```bash
glab api -X POST "projects/:id/merge_requests/<release mr>/cancel_merge_when_pipeline_succeeds"
# check: 3a no longer reports auto-merge; the next bot refresh may re-arm, so say where the team reads that the release is held
# already merged: nothing is held; rk method recovery owns the withdrawal
```

### 3c. Compare the entry against the range

```bash
git fetch origin --tags --force
git log --oneline "v<previous version>^{commit}..origin/master"
# check: every commit that should appear in the entry is listed
```

Nothing missing: skip to step 4. Something missing: continue — and on a request still armed, run 3b first, because a correction pushed to an armed request races the checks it restarts.

### 3d. Correct it on the request's branch

On branches:

```bash
gh pr checkout <release pr> --repo <repo>        # or glab mr checkout <release mr>
```

On worktree:

```bash
rk worktree add "<bot branch>" --apply && cd "../<project>@<bot branch flattened>"
# check: the source line reports remote — the bot's branch, release-plz-<suffix> on rust or rk-release/<base> on bash and python, fits the grammar's release arms and is seated from origin's tip, never recreated from the trunk; step 4's merge retires the worktree through step 4 of rk guide worktree
```

Then edit the changelog and push the correction:

```bash
git commit -am "docs(changelog): Complete the entry for v<version>"
git push
```

### 3e. Confirm the request survived

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

Then read the entry the request carries now, because a refresh regenerates it:

```bash
git fetch origin "<bot branch>"
git show FETCH_HEAD:CHANGELOG.md | sed -n '/^## \[<version>\]/,/^## \[/p'
# CHANGELOG.md, or the changelog file the binding writes
# check: the entry carries the correction you pushed in 3d
```

A new number, or an entry without the correction, means a refresh took the fix: redo the correction on the request it left and merge without waiting.

## 4. Let it merge, or merge it

This is the release. A request behind the trunk waits for the bot to recompute it and for the refreshed checks to pass; [the drawn model](../method/06-release-from-trunk.md#why-an-armed-release-waits) shows what releases that wait. The named check and freshness gate the merge either way, and squash is the only allowed method, so `master` stays linear.

On trunk:

The request stands armed: the forge merges it once the last required check passes and the request carries the trunk's tip, and 4a is a watch, not an action. A request disarmed in 3b takes the unarmed form until the bot re-arms it.

On lines:

The merge is yours: watch the checks, then merge, because a line's request is never armed.

### 4a. Watch the checks, and merge where the merge is yours

On github:

```bash
gh pr checks <release pr> --repo <repo> --watch
# check: on an armed request the forge merges when the last check passes and the branch is current; there is nothing to run
# not armed, or disarmed: merge it yourself once the checks pass
gh pr merge <release pr> --repo <repo> --squash --delete-branch
```

On gitlab:

```bash
glab ci status --wait
# check: on an armed request the forge merges when the pipeline passes; there is nothing to run
# not armed, or disarmed: merge it yourself once the pipeline passes
glab mr merge <release mr> --squash --remove-source-branch
```

### 4b. Bind the merge commit

Steps 5 and 6 correlate against it.

```bash
git fetch origin && git rev-parse origin/master
# check: prints the SHA the release runs on
```

The push that lands the bump runs the binding's release path. On rust, release-plz tags `v<version>` and publishes, and the tag starts the artifact build where the binding has one. On bash and python, the release jobs run on this merge commit alone: `rk release build` builds, CI attests and uploads where the pair has a registry, and `rk release publish` creates the release page and the tag last.

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
# bash and python: release.yml is both workflows and runs on the merge commit alone, so the first watch is the whole wait and the second finds no run
```

""Where a job waits longer than the runner queue usually takes, tell a queued job from a stuck one before you wait more. [The GitHub forge document](../forges/github.md) states the same check.

On github:

A queued job is evidence, not a verdict:

```bash
gh run view <id> --repo <repo> --json jobs -q '.jobs[] | select(.status == "queued") | .name'
gh api repos/<repo>/actions/runners -q '.runners[] | select(.status == "online") | {name, labels: [.labels[].name]}'
# check: a job on a hosted label waits for capacity; a job on self-hosted labels needs one online runner that carries every label it requests
# no such runner: the job stays queued until a matching runner comes online, and fails after 24 hours in the queue
```

On gitlab:

```bash
glab ci status --wait
```

A `pending` job can be queued or stuck, and the REST job fields read the same for both. One GraphQL query decides it, on `pending` jobs alone, because a `created` job waits on an earlier stage or a `needs:` job and not on a runner:

```bash
glab api graphql -f query='{ project(fullPath: "<repo>") { jobs(statuses: [PENDING]) { nodes { name status stuck } } } }'
# check: stuck false means the runner queue is slow and waiting is correct; stuck true means no runner will take this job, and the cause is runner configuration, not patience
```

The `--event push` filter narrows the second watch to the tag push, which is the only event the artifact workflow runs on under the landed workflows, and it keeps the watch off the request runs a target that tuned the run mode still produces. The `(rust, gitlab)` pair has no artifact builder, so there is no dedicated build to wait for and no installers to expect on the release page; [the Rust binding](../bindings/rust.md) states it.

## 6. Verify

### 6a. The tag and the trunk name the same commit

Universal, and the release itself. `^{commit}` is what makes the two values comparable whatever kind of tag the release wrote: release-plz writes an annotated tag, which names a tag object, and the forge release that `rk release publish` creates writes a lightweight tag, which names the commit.

```bash
git fetch origin --tags --force
git cat-file -t "v<version>"
# check: prints tag on rust, and commit on bash and python
git rev-parse "v<version>^{commit}" origin/master
# check: two identical SHAs
```

### 6b. The registry serves exactly this version

On rust:

```bash
cargo info <crate> | grep -m1 -i '^version'
# check: prints <version>
```

On python:

```bash
curl -fsS -o /dev/null -w '%{http_code}\n' "https://pypi.org/pypi/<package>/<pep 440 version>/json"
# check: prints 200; <pep 440 version> spells a candidate 1.4.3rc2, never 1.4.3-rc.2
# the registry answer none: nothing goes to PyPI, and this check does not apply
```

### 6c. The release page carries its artifacts

On github:

```bash
gh release view v<version> --repo <repo> --json assets \
  -q '[.assets[].name] | join(", ")'
# check: the artifact list is not empty, where the binding declares that surface
```

On gitlab:

```bash
glab release view v<version>
# check: the assets list the package files the pair builds, where the binding declares that surface
```

### 6d. An installed binary reports the new version

On rust/github:

```bash
curl -LsSf "https://github.com/<repo>/releases/download/v<version>/<crate>-installer.sh" | sh
# check: the installed binary reports <version>; a bare version call only reads whatever is already on PATH
```

### 6e. The provenance verifies

The run that built each release artifact signed it, and the pair's own verifier proves it — this is the check that turns the provenance invariant from a comment into a rule.

On rust/github:

```bash
tmp="$(mktemp -d)"
gh release download "v<version>" --repo <repo> --dir "$tmp"
( for artifact in "$tmp"/*; do
    gh attestation verify "$artifact" --repo <repo> \
      --source-digest "$(git rev-parse "v<version>^{commit}")" \
      --signer-workflow "<repo>/.github/workflows/<artifact workflow>" \
      || exit 1
  done )
# check: exits 0 — every downloaded file verifies, the curled installers included, and one failure fails the whole loop; the release artifacts are whatever a consumer downloads, not just the archives
# the two flags bind the evidence to this release: a repo-only verify would also accept a valid attestation some other run of some other workflow minted over identical bytes
```

On bash/github:

```bash
tmp="$(mktemp -d)"
gh release download "v<version>" --repo <repo> --dir "$tmp" --pattern '*.tar.gz'
gh attestation verify "$tmp"/*.tar.gz --repo <repo> \
  --source-digest "$(git rev-parse "v<version>^{commit}")" \
  --signer-workflow "<repo>/.github/workflows/<artifact workflow>"
# check: the tarball verifies against the release commit and the signing workflow; the .sha256 beside it is verification evidence, not a release artifact
```

On bash/gitlab:

```bash
curl -fsSL -o "<name>.tar.gz" "<package file url>"
curl -fsSL -o "<name>.tar.gz.sigstore.json" "<bundle file url>"
cosign verify-blob-attestation --type slsaprovenance1 \
  --bundle "<name>.tar.gz.sigstore.json" \
  --certificate-oidc-issuer https://gitlab.com \
  --certificate-identity "https://gitlab.com/<repo>//.gitlab-ci.yml@<built ref>" \
  "<name>.tar.gz"
# check: prints Verified OK; the release page links both files
# <built ref>: the certificate names the ref the release was built from — refs/heads/master for a trunk release, refs/heads/release/<line> for a line release
# self-managed instance: the pipeline stated the keyless-signing boundary and released without provenance; there is no bundle, and this check does not apply
```

On python/github:

```bash
tmp="$(mktemp -d)"
gh release download "v<version>" --repo <repo> --dir "$tmp" --pattern '*.tar.gz' --pattern '*.whl'
gh attestation verify "$tmp"/* --repo <repo> \
  --source-digest "$(git rev-parse "v<version>^{commit}")" \
  --signer-workflow "<repo>/.github/workflows/<artifact workflow>"
# check: the sdist and the wheel on the release page verify against the release commit and the signing workflow
pypi-attestations verify pypi --repository "https://github.com/<repo>" "pypi:<distribution filename>"
# check: prints OK, once per sdist and wheel the release published
# the registry answer none: nothing goes to PyPI, and this check does not apply
```

On python/gitlab:

```bash
curl -fsSL -o "<distribution filename>" "<package file url>"
curl -fsSL -o "<distribution filename>.sigstore.json" "<bundle file url>"
cosign verify-blob-attestation --type slsaprovenance1 \
  --bundle "<distribution filename>.sigstore.json" \
  --certificate-oidc-issuer https://gitlab.com \
  --certificate-identity "https://gitlab.com/<repo>//.gitlab-ci.yml@<built ref>" \
  "<distribution filename>"
# check: prints Verified OK, once per sdist and wheel; the release page links each file and its bundle
# <built ref>: the ref the release was built from, refs/heads/master for a trunk release and refs/heads/release/<line> for a line release
# self-managed instance: the pipeline stated the keyless-signing boundary and released without provenance; there is no bundle, and this check does not apply
pypi-attestations verify pypi --repository "https://gitlab.com/<repo>" "pypi:<distribution filename>"
# check: prints OK, once per sdist and wheel; PyPI documents attestations from a GitLab CI/CD trusted publisher, signed by pypi-attestations, and no live release of this pair has run the check yet
# the registry answer none, or a self-managed instance: nothing goes to PyPI, and this check does not apply
```

On rust/gitlab:

The pair declares no provenance surface — crates.io stores none, and the pair builds and attaches no binaries; [the Rust binding](../bindings/rust.md) states it, and this check does not apply.

### 6f. Repair a failed tag push

Anything but 6a's two identical SHAs means the tag never reached the forge: rerun the publish workflow on the merge commit — on rust never the artifact workflow, which only builds from a tag that already exists. On bash and python the release job is the publish workflow, and a rerun at the same commit resumes where it stopped. A rerun runs the configuration of its own commit, so it recovers a transient failure and never a defect a later commit fixed. Where the fix landed after the bump and no tag and no release exist, take the un-bump of [recovery](../method/04-recovery.md#the-release-request-merged-and-nothing-published) instead.

On github:

```bash
gh run rerun --repo <repo> --failed \
  "$(gh run list --repo <repo> --workflow <publish workflow> \
     --commit "$(git rev-parse origin/master)" --limit 1 --json databaseId -q '.[0].databaseId')"
```

## An older line

A patch-only release for users on an older version is not this sequence: a fix crosses by `rk guide backport`, the line's own life — candidate cycle and retirement included — is `rk guide release-lines`, and [branch for release](../method/07-branch-for-release.md) owns the path's why; [recovery](../method/04-recovery.md) carries the entry point for a line whose branch does not exist yet.
