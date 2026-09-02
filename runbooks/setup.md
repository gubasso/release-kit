# Setup runbook

The steps of [setup](../method/02-setup.md) as commands, once per repository, in the chapter's order: the chapter owns each step's why, this page owns its how. A substep carries its command and the check the command prints; where `rk setup` executes a step, the `Automated:` line names the verb, and `rk setup script <name>` prints the exact calls it runs, which is its form by hand. `rk guide setup` fills in the project path, forge, and technology where detection resolves them. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## Prerequisites

Every step below assumes these; each is a probe, not a change.

- `rk` on `PATH`: `rk --version`
- The forge CLI, `openssl`, and `curl`: `rk doctor` reports each ok
- OS keyring where the forge bootstrap needs one: `secret-tool --version`, on a host shell and never in a container
- Clean trunk: `git status --porcelain` empty, `HEAD` equal to the remote trunk

On github:

`gh auth status` names the repository's owner with `repo` scope.

On rust:

The registry account is signed in at crates.io with a verified email, and `cargo info <crate>` 404s unless the account already owns the crate; [the binding](../bindings/rust.md) says why the email gates the first publish.

## 0. Gate the package metadata

Before anything that needs credentials or cannot be undone; the binding names what the registry rejects here.

```bash
rk setup step package-check --target .
# check: exits 0; the package is publishable with no token spent
```

## 1. Make the trunk the sole long-lived branch

The full apply covers this section and the next two in order:

```bash
rk setup --target .                                      # preview every step
rk setup --target . --apply --required-check <name>      # run them in order
rk setup check --target .                                # prove what was applied
```

On github:

`--required-check <name>` names the CI job the trunk's protection requires, and the protection refuses without it: a wrong or missing check name does not fail, it hangs the merge button with nothing saying why. `gh api repos/<repo>/commits/HEAD/check-runs` lists the names the project's own workflow reports.

On gitlab:

No check is named here: the forge requires the whole pipeline through one project setting, so `--required-check` is refused as a usage error rather than silently discarded.

The substeps below are the same steps one at a time, for a partial run or a rerun.

### 1a. Make the trunk the default branch

Automated: `rk setup step default-branch --apply`; a repository with no such branch gets it created at the current default's tip first.

On github:

```bash
gh repo view <repo> --json defaultBranchRef -q .defaultBranchRef.name
# check: prints master, whether the run set it or found it set
```

### 1b. Retire every other long-lived branch

Automated: `rk setup step single-trunk --apply` — the one destructive step; its guard deletes a candidate only when it is an ancestor of the trunk and fails closed on anything else.

On github:

```bash
gh api "repos/<repo>/git/ref/heads/<candidate>"
# check: 404s for each retired candidate; the script names the candidates it retires
```

### 1c. Delete every branch its merge retires

Automated: `rk setup step merge-cleanup --apply`. One switch on the repository, so it holds however anyone merges; a merged branch left behind becomes another long-lived branch, which is what 1b just paid a destructive guard to remove.

On github:

```bash
gh api "repos/<repo>" -q .delete_branch_on_merge
# check: prints true, whether the run set it or found it set
```

### 1d. Remind this clone after a pull

Automated: `rk setup step branch-reminder --apply`. 1c deletes the remote copy; this clone's own copy survives with its upstream marked gone, and no forge can reach it. The step writes a post-merge hook that runs `rk branches prune --quiet` after every pull: silent when the clone is clean, a report naming the retired branches otherwise, and never a deletion — `rk branches prune --apply` is the operator's own call. The step refuses over an existing post-merge hook it did not write; merge by hand there, guarding the same command behind `command -v rk`. Like every setup step it resolves the forge, so it runs where the forge CLI is installed.

```bash
hook="$(git rev-parse --git-path hooks)/post-merge"
if [ ! -e "$hook" ] && [ ! -L "$hook" ] || { [ -f "$hook" ] && [ ! -L "$hook" ] && grep -qF '# release-kit branch reminder' "$hook"; }; then
  cat > "$hook" <<'HOOK'
#!/bin/sh
# release-kit branch reminder
if command -v rk >/dev/null 2>&1; then
  rk branches prune --quiet || :
fi
exit 0
HOOK
  chmod 0755 "$hook"
fi
grep -F '# release-kit branch reminder' "$hook"
# check: prints the marker line; no print means a foreign hook survived untouched - merge by hand there
```

## 2. Let automation act

### 2a. Let CI write and open requests

Automated: `rk setup step ci-permissions --apply`; the chapter owns why the raised default reaches only a workflow that declares no permissions of its own.

On github:

```bash
gh api "repos/<repo>/actions/permissions/workflow"
# check: prints "default_workflow_permissions": "write" and "can_approve_pull_request_reviews": true
```

### 2b. Create the bot identity

One action stays manual, on one forge, once per account ever.

On github:

Creating the bot App needs a browser, because the manifest flow redirects through one; it happens once in an account's lifetime, never per project. `rk forge github` carries the field-by-field walkthrough, the credentials to collect, and the warning about the private key that downloads exactly once; it ends with `RK_BOT_APP_ID` and `RK_BOT_PRIVATE_KEY_FILE` exported for the substeps below.

On gitlab:

Nothing is manual: creating the project access token also creates its bot user, so 2c does the whole bootstrap. `rk forge gitlab` states the role and scopes the token carries and why.

### 2c. Grant the bot this repository

Automated, with 2b's exports in the environment. Run it on the host, not a container: the key and the keyring live there.

```bash
rk setup step install-bot --target . --apply
# check: reports the bot covering this repository
# refused on github: the grant write needs a user token; rk forge github walks the one it takes
```

### 2d. Store the bot credentials

Automated, same exports; the values travel on stdin and land as repository secrets.

```bash
rk setup step bot-secrets --target . --apply
# check: reports the credentials stored
# already stored: they are overwritten silently, which is the rotation path
```

On github:

```bash
gh secret list --repo <repo>
# check: lists RELEASE_BOT_APP_ID and RELEASE_BOT_APP_PRIVATE_KEY
```

## 3. Protect the trunk and the tags

### 3a. Prove the required checks exist

The trunk protection names check contexts, and a context no workflow reports is unsatisfiable: nothing would ever merge.

On github:

```bash
gh api "repos/<repo>/contents/.github/workflows/ci.yml" -q .content | base64 -d | grep -E '^\s+(test:|pull_request:)'
gh api "repos/<repo>/contents/.github/workflows/pr-title.yml" -q .name
# check: the job id and its pull_request trigger appear, and the trunk carries pr-title.yml
# 404 on pr-title.yml: step 4 has not reached the trunk, and protecting now would block the very request that lands it
```

### 3b. Protect the trunk

Automated: `rk setup step protect-trunk --apply --required-check <name>` — on GitHub only after 3a passes, per the chapter's ordering; a rerun updates the protection in place.

### 3c. Protect the tags and the release lines

Automated: `rk setup step protect-tags --apply`, and — only when an older line exists — `rk setup step protect-release-lines --apply`, which a full apply skips.

### 3d. Prove the protections

```bash
rk setup check --target .
# check: every step reports satisfied; protect-release-lines reports skipped while no line exists
# install-bot unknown: rerun with 2b's exports in the environment; rk forge <forge> owns why only the bot reads its own installation
```

Where the forge enforces less than a step claims, the check names the weaker guarantee rather than passing; tag protection on GitLab is the case this exists for, per `rk forge gitlab`.

## 4. Land the workflow files

### 4a. Land the payload

`--scopes` is required on the apply: the Conventional Commit vocabulary the project accepts, rendered into the title check and the commit hook, because a vocabulary is a decision rather than a default.

```bash
rk init --tech <tech> --target .             # preview every destination
rk init --tech <tech> --target . --apply     # write the files and the landing record
# check: the apply reports each written file and every sentinel left to fill
# already landed: the apply refuses; rk upgrade --target . --apply takes an existing landing to a newer payload
```

Then answer every reported sentinel and confirm the record:

```bash
grep -rn 'TODO(release-kit)' . --exclude-dir=.git
# check: prints nothing once each sentinel is answered
rk status --check --target .
# check: reports the landed payload as current
```

### 4b. Regenerate the artifact plan

On rust:

Regenerate the artifact workflow at the pin and read what a release will build; [the binding](../bindings/rust.md) carries the commands and the no-diff proof.

### 4c. Put the files on the trunk

While step 3's protection does not stand yet, the trunk takes this one direct write.

```bash
git add -A && git commit -m 'chore(<scope>): land the release workflow files'
git push origin master
# check: the push lands
# rejected: a protection already stands, so land this through 4f instead
```

### 4d. Install the hooks, last

Last, because two of them refuse exactly what 4c just did. The landing splices a marked block of release-convention hooks into `.pre-commit-config.yaml`, under `repos:`, leaving the rest of the file the target's own. A hook already doing one of these jobs — `committed`, `commitlint`, `gitlint`, another conventional-commit or branch-guard hook — is a duplicate to name, and the choice between it and the landed hook is the operator's, never a silent second hook doing the same job. On an existing config, verify the top level carries `default_install_hook_types: [pre-commit, commit-msg, pre-push]` — the splice cannot add a top-level key. Where the target's CI runs a `pre-commit run` sweep, set `SKIP=no-commit-to-branch` in that job's environment: CI commits nothing, so the commit-time branch guard would refuse every trunk checkout it sweeps.

```bash
pre-commit install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push
# check: reports each of the three hook types installed
```

### 4e. Configure this clone

Two settings, local to this clone, that match the convention's history to the way git is asked to move it.

```bash
git config --local pull.rebase true
git config --local fetch.prune true
git config --local --get-regexp '^(pull\.rebase|fetch\.prune)$'
# check: prints both keys as true, whether this run set them or found them set
```

- `pull.rebase` replays a topic branch onto the trunk rather than merging back, which is what keeps one pull request one commit.
- `fetch.prune` drops the remote-tracking ref 1c taught the forge to delete, so a merged branch stops appearing in this clone — which is what marks the local copy gone for the reminder 1d installed.
- Neither applies to the trunk itself: the trunk is never pulled, only reset, which 4g does.

### 4f. Land one change through the trunk's one path

From 3b on, the trunk is unwritable by hand: every later change reaches it through this path, so prove it once now. The request's title becomes the whole trunk commit — squash is the only merge method and the forge takes the title as the message — so write it as a Conventional Commit carrying the strongest intent of everything in the branch: one `!` anywhere makes the title breaking. Spell the title out rather than filling it from a commit: `--fill-first` takes the first commit's subject, the intent of one commit out of however many the branch holds.

```bash
git switch -c fix/<slug>
git push -u origin "$(git branch --show-current)"
```

On github:

```bash
gh pr create --repo <repo> --base master --head "$(git branch --show-current)" --title '<type>(<scope>): <subject>' --body '<what changed, and why>'
gh pr checks --watch
# check: the required check reports success; the other contexts may report skipped
gh pr merge --squash --delete-branch
# check: reports the pull request squashed and merged
```

- nothing to land: the working tree is clean and the trunk already holds the work, so go to 4g and confirm it.
- `GH013: Repository rule violations found` on the push: the branch is `master`, which takes no direct push; branch first and push that branch instead.
- the check never appears: the job id is not the one the ruleset names, and 3a's prerequisite has moved; nothing merges until the ruleset and the workflow agree on the name.

### 4g. Take the local trunk to the merge

The squash writes one commit that is not the branch's commit, so the local trunk never advances by the merge alone; reset takes it there in one move whichever way the clone stands.

```bash
git switch master
git fetch origin
git status -sb
# check: behind by the merge and ahead by nothing
git reset --hard origin/master
git rev-parse HEAD origin/master
# check: two identical SHAs
git branch -D fix/<slug>
# check: reports the branch deleted; the forge already deleted its remote by 1c
```

- ahead as well as behind: the clone committed to the trunk directly before branching, so the trunk holds those commits twice — once each and once squashed. `git diff --stat HEAD origin/master` prints nothing, which is the proof the reset discards duplicates only, and `git reflog` still names each one.
- `not possible to fast-forward` from the merge command: that is the ahead-as-well-as-behind case, reported by the local update the merge attempts, and this substep is its repair.
- `-D` rather than `-d` on the topic branch, and not for haste: the squash gave the work a commit no branch is an ancestor of, so `-d` refuses every branch this path produces.

## 5. Publish the first version by hand

The upload is permanent: the registry never lets a version be overwritten, and the name is claimed for good.

### 5a. Gate the tree and read the version

```bash
git fetch origin
git status --porcelain
# check: empty
git rev-parse HEAD origin/master
# check: two identical SHAs; HEAD is what the publish packages
# they differ: the trunk takes no direct push, so land the commits through 4f and rerun
```

On rust:

```bash
cargo metadata --no-deps --format-version 1 -q | jq -r '.packages[0].version'
# check: note it; 5c asserts against it
```

### 5b. Mint the bootstrap token

Manual — registry web UI, no API, once per package. The binding walks the form field by field: scoped to publishing new versions of exactly this package, shortest expiry.

### 5c. Publish

On rust:

```bash
cargo login
cargo publish --locked
cargo info <crate> | grep -m1 -i '^version'
# check: prints the version 5a reported; cargo login prompts for the token on stdin
# already published: the registry refuses a version it already serves; go to step 6
```

On python:

```bash
python -m build
python -m twine upload dist/*                # reads the token from the environment
```

On bash:

There is no registry; the first release is proven by the automated path in step 7, and nothing is published by hand.

## 6. Register the trusted publisher

Manual — registry web UI, no API, once per package; the binding walks the form and its read-back check. Register the owner, the repository, and the publish workflow's filename, which must be the one that stays true. Then revoke the bootstrap token, both halves — the registry-side revoke and the host-side logout the binding carries — so the package has exactly one publishing path.

## 7. Prove the automated path

### 7a. Prove the publish workflow runs

On github:

```bash
gh run list --repo <repo> --workflow <publish workflow> --limit 1 --json conclusion,headSha -q '.[0]'
# check: the newest run on the trunk concluded success; the binding names the workflow file
```

### 7b. Prove it produced a release request

On github:

```bash
gh pr list --repo <repo> --state open --json number,title
# check: a release request from the bot is open, proposing the next version
# none open: the trunk matches the version step 5 published, so there is nothing to propose; land one change through 4f and rerun
```

On gitlab:

```bash
glab mr list
# check: a release request from the bot is open, proposing the next version
```

### 7c. Cut one release end to end

```bash
rk guide release
```

The chapter names its passing verify step as the proof the next step depends on.

## 8. Require trusted publishing

Manual — registry web UI, no API, once per package, and only after step 7 proved one OIDC release; the binding names the switch, what it rejects from here on, and the recovery escape that starts by turning it off.
