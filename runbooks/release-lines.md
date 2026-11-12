# Release-lines runbook

The eight steps of [release lines](../method/09-release-lines.md) as commands: the chapter owns each step's why, this page owns its how. `<line>` is the line's `major.minor`, and `<repo>` is the project path, filled in by `rk guide release-lines` where detection resolves it; a fix crossing to a line is `rk guide backport`, and this page is the line's own life. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## 1. Record the style

```bash
rk status --target .
# check: prints the lines style
# trunk style: this project ships from the trunk; rk upgrade --target . --release-style lines --apply changes it, and the diff is the arming line in the release workflow
# no style in the record: the record predates the parameter; the upgrade refuses until --release-style names one
```

## 2. Wire the repository for lines

Automated: `rk setup step protect-release-lines --apply`, once per repository.

```bash
rk setup check --target .
# check: protect-release-lines reports satisfied
```

Then the project's own CI takes the lines in its trigger filters, by hand: release-kit writes no such file and no check reports the gap.

On github:

```bash
grep -n -A8 '^on:' .github/workflows/ci.yml
# check: push and pull_request each admit a release/* ref — through no branch filter, a branches list carrying release/** with no ! pattern excluding it, or a branches-ignore list that does not match one
# a push declaring only tags or tags-ignore runs for no branch at all, whatever its other filters say
# a branches list naming the trunk alone: add 'release/**' beside it; a pull_request filter selects by the base branch, so a request opened against a line starts no run without it
```

On gitlab:

```bash
grep -n -B4 -A8 'rules:\|workflow:\|only:\|except:' .gitlab-ci.yml
# check: the branch path admits CI_COMMIT_BRANCH =~ /^release\//, and the merge-request path matches the line's own request — every merge_request_event, or one whose CI_MERGE_REQUEST_TARGET_BRANCH_NAME =~ /^release\//
# a rule reaching only the source branch matches neither path: CI_COMMIT_BRANCH is unset in a merge-request pipeline, and the request's source branch is the bot's, not the line
```

## 3. Open the line

The base is chosen and stated, never defaulted; the verb refuses without it.

On worktree:

```bash
git fetch origin --tags --force
rk lines open <line> --base "v<version>" --apply
rk worktree add release/<line> --apply && cd "../<project>@release-<line>"
git push -u origin release/<line>
# check: the line stands at its explicit base in its own seat, and the push publishes it with its upstream set
# already open: the open adopts the existing branch, the add adopts the existing seat, and each reports satisfied
```

On branches:

```bash
git fetch origin --tags --force
rk lines open <line> --base "v<version>" --apply && git checkout release/<line>
git push -u origin release/<line>
# check: the branch stands at its explicit base, and the push publishes it with its upstream set
# already open: the open adopts the existing branch and reports satisfied
```

## 4. Cross the fix

`rk guide backport` steps 4 to 6 own the crossing: fix on the trunk first, cherry-pick only that commit, wait for the line's own CI.

```bash
git log --oneline -1 origin/release/<line>
# check: the tip is the crossed fix and nothing else traveled
```

## 5. Mint the candidate

A candidate is `v<version>-rc.<n>` on the line. The rust, bash, and python pairs each carry a candidate job. Mint it with the tip you read: dispatch the release workflow on GitHub, `release-plz.yml` on a rust pair and `release.yml` on a bash or python pair, or run the manual `release-candidate` job of the line's pipeline on GitLab with `LINE` and `EXPECT`.

```bash
git fetch origin --tags --force
rk lines mint <line>
# check: previews the next candidate's version and names the tip to pass as expect
```

On github:

```bash
gh workflow run <workflow> --repo <repo> --ref release/<line> -f line=<line> -f expect=<sha>
gh run list --repo <repo> --workflow <workflow> --branch release/<line> --event workflow_dispatch --limit 1 --json databaseId -q '.[0].databaseId'
gh run watch <run-id> --repo <repo> --exit-status
# check: the dispatch run, not the push run the mint starts, ends in success
```

On gitlab:

```bash
glab ci list --branch release/<line>
# check: pick the pipeline, and run its manual release-candidate job with LINE=<line> and EXPECT=<sha>
glab ci trace release-candidate --pipeline-id <pipeline-id>
glab ci get --pipeline-id <pipeline-id> --with-job-details
# check: the release-candidate job itself reads success; the pipeline passes without it, because the job may fail
```

```bash
git fetch origin --tags --force
rk lines rc <line>
# check: names the newest candidate on the line and the next number a mint takes
# nothing listed: no candidate is tagged yet on this line
```

## 6. Validate the candidate

On a rust pair whose crate publishes to crates.io, a validator installs the candidate with `cargo install <crate> --version <version>-rc.<n>`. Where `profile.release.registry` is `none`, the validator installs it from its tag with `cargo install --git <repository URL> --tag v<version>-rc.<n>`. On a python pair that publishes to PyPI, `pip install <name>==<version>rc<n>` installs the candidate with no `--pre`. A bash pair, and a python pair under `none`, ship the candidate as the assets of its release page.

On github:

```bash
gh release view "v<version>-rc.<n>" --repo <repo> --json assets -q '[.assets[].name] | join(", ")'
# check: the candidate's installers exist; install one and use it — the human validation is the gate no command prints
```

On gitlab:

```bash
glab release view "v<version>-rc.<n>"
# check: the candidate's page stands and its description names the candidate on its first line, and on a rust pair the install command; install it and use it — the human validation is the gate no command prints
```

## 7. Answer a finding with the next rc

The finding crosses like any fix, as step 4 says, and the next mint takes the next number: run step 5 again, through its wait and its fetch.

```bash
git fetch origin --tags --force
rk lines rc <line>
# check: the newest candidate advanced by one; an rc number is single-use, because the tag protection makes a candidate immutable
```

## 8. Promote, then retire

### 8a. Promote the candidate

Promote through the same dispatch or job as step 5, with `promote=true` on GitHub or `PROMOTE=true` on GitLab. It writes the candidate's stable version, and it refuses while the line holds a commit the candidate did not test. Then run `rk guide release` steps 3 to 6 against the line. A line's request is never armed, so the merge is yours, per that runbook's step 4.

On github:

```bash
gh pr list --repo <repo> --state open --base release/<line>
# check: the line's release request proposes the validated version; on a bash or python pair its branch is rk-release/release/<line>
```

On gitlab:

```bash
glab mr list --target-branch release/<line>
# check: the line's release request proposes the validated version; on a bash or python pair its branch is rk-release/release/<line>
```

### 8b. Retire the line

Only when the line leaves production, and only behind its tags.

```bash
rk lines retire <line>
rk lines retire <line> --apply
# check: the preview names the seat and the local branch; the apply removes the seat before the branch and leaves the remote deletion to you
# refused: a commit the line holds is unreachable from its tags; tag it first, or the deletion garbage-collects it
git push origin --delete release/<line>
# check: the line's tags still resolve; the chapter owns why they make the deletion safe
```
