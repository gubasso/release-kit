# Setup runbook

This runbook gives the steps of [setup](../method/02-setup.md) as commands. Run them once per repository, in the order of the chapter. The chapter owns the why of each step, and this page owns its how. A substep carries its command and the check that the command prints. Where `rk setup` executes a step, the `Automated:` line names the verb. `rk setup script <name>` prints the exact calls that the verb runs, which is its form by hand. `rk guide setup` fills in these values where detection resolves them:

- the project path
- the forge
- the technology

The commands are the operator's to run. An agent serves a runbook and states the command. It runs a command only where the operator's request named that step.

## Prerequisites

Every step below assumes these prerequisites. Each one is a probe, not a change.

- `rk` on `PATH`: `rk --version`. The tool manager that the project runs installs `rk` and moves its pin, by the commands of the Install section of the release-kit README. The crate's page, <https://crates.io/crates/release-kit>, carries that README. No `rk` verb installs or moves `rk`.
- No user-scope copy of the skills: `rk doctor` reports `skill-shadow` ok. The landing writes the skills of this binary into the project, under two roots:
  - `.claude/skills`, which Claude Code reads
  - `.agents/skills`, which these agents read:
    - Codex
    - Gemini CLI
    - Copilot CLI

  Before the landing, `rk skill show rk-setup` prints the setup skill, and `rk skill show rk-setup --file <path>` prints each file it names. A copy that an earlier release installed under the home shadows the project's, and `rk skill uninstall --apply` is the repair.
- The forge CLI: `rk doctor` reports it ok. `rk` sends its own web requests and signs the App token itself, so it needs neither `curl` nor `openssl`.
- A hook manager that reads `.pre-commit-config.yaml`, such as prek or pre-commit, which the host installs: its own version call answers. Step 4d installs its hooks in the clone, and the `pre-merge-commit` gate of `rk integrate` is one of them.
- An OS keyring where the forge bootstrap needs one: `secret-tool --version`, on a host shell and never in a container.
- A clean trunk: `git status --porcelain` is empty, and `HEAD` equals the remote trunk.

Decide how the project obtains `rk` before the first landing closes, with an agent or without one. Two answers close it:

- A manager. Place the install lines of that manager in its file, and commit the file with its lock. The manager file and its lock record the version for every contributor. A pin move edits the version in the manager file by that manager's own commands, then lands the new release through [the landing runbook](./landing.md).
- A host install. No file records the version, so each host installs `rk` itself, and nothing lands in the repository.

This project decides where a file lands for a third-party application, such as an agent root or an editor directory. The project makes it against the current documentation of that application and records it with a dated citation. Release-kit states where its own artifacts go. It never decides such a placement for a project.

On github:

`gh auth status` names the repository's owner with `repo` scope.

On rust:

The registry account is signed in at crates.io with a verified email. `cargo info <crate>` answers 404 unless the account already owns the crate. [The binding](../bindings/rust.md) says why the email gates the first publish.

The target configuration answers which steps apply, before the operator does:

- A forge step applies where `profile.forge` names a forge that this release drives.
- The release half applies where `profile.release.mode` is `automatic`.
- The packaging gate applies where that release names a driver.
- The tag protection applies where `profile.release.mode` is not `none`.
- The reporting channel applies where `capabilities.reporting_policy` asks for the landed policy.

`rk profile --target .` reports those values. Every run prints the stance of the step before it acts.

A project that runs only part of what applies declares that before its first apply. `setup.excluded_steps` in `.release-kit/config.toml` names each step that the project does not run, with the reason. `rk setup --list` names the steps that it can hold. Then:

- A full run states each exclusion and runs nothing for it.
- `rk setup check --target .` reports each exclusion with its reason and judges the rest.
- `rk setup step <name> --apply` refuses a step named there until the entry goes.

An exclusion declared for a step that does not apply here reports as redundant. The setup reads the file with no landing, so a project that lands no file still declares its model.

The landing-side twin is `landing.accepted_drift`. A rendered file that the project must change carries an entry. The entry holds the reason and the SHA-256 of the accepted bytes. `rk status` then reports the edit with its reason and does not judge it, but a further edit still faults. Where a parameter reaches the change, set it instead. Then land again.

## 0. Gate the package metadata

Run this step before anything that needs credentials or cannot be undone. The binding names what the registry rejects here. A rust or python project whose `profile.release.registry` is `none` packages nothing, so the step reports not applicable there.

```bash
rk binding <tech>
# check: what the registry rejects, for the release driver this project runs
rk setup step package-check --target . --apply
# check: exits 0; the package is publishable with no token spent and ships none of release-kit's own files
```

## 1. Make the trunk the sole long-lived branch

The full apply covers this section and the next two in order:

```bash
rk setup --target .                                      # preview every step
rk setup --target . --apply                              # run them in order
rk setup check --target .                                # prove what was applied
```

On github:

`setup.required_check` in `.release-kit/config.toml` names the one CI job that the trunk protection requires beside the landed title check. `--required-check <name>` overrides it for one invocation. The protection refuses until one of the two answers it. A wrong or missing check name does not fail. It hangs the merge button, and nothing says why. `gh api repos/<repo>/commits/HEAD/check-runs` lists the names that the project's own workflow reports.

The `needs` of the gate is the voting list. Where the workflow declares more than one job, name a gate job. Make it need every job whose failure must hold the merge. The merge reads the named check alone, and nobody reads the rest of the list. A job left out of that list still reports, and it holds nothing. The gate also cannot need a job in a second workflow file, because `needs` names job ids inside one file. So every proof that a merge rests on belongs in the workflow of the gate. A workflow that the tooling of a technology generates is configured to report on no request at all.

The gate runs `if: always()` and fails on any result other than `success`. The forge reports a job that a failed dependency skipped as success. The gate names the job that failed, because a red merge is read in the log of the gate:

```yaml
gate:
  if: always()
  needs: [lint, test, build]
  runs-on: ubuntu-latest
  steps:
    - name: every job succeeded
      env:
        RESULTS: ${{ toJson(needs) }}
      run: |
        set -eu
        bad="$(printf '%s' "$RESULTS" |
          jq -r 'to_entries[] | select(.value.result != "success") | "\(.key) reported \(.value.result)"')"
        [ -z "$bad" ] || { echo "$bad"; exit 1; }
```

`rk setup check --required-check <name>` judges the shape of the gate. It faults `protect-trunk` on five things:

- No job reports the name.
- More than one job reports it.
- The condition is neither `always()` nor `${{ !cancelled() }}`.
- `needs` is not a literal list.
- The trigger filters the request away.

It says nothing about the jobs that the gate does not name. The author of this list decides which of those must vote. The binding owns what to write instead, and `rk status --check` names it by remediation for a generated workflow.

On gitlab:

No check is named here. The forge requires the whole pipeline through one project setting. So the setup refuses `--required-check` as a usage error and does not silently discard it. The project's own jobs go in `.gitlab/ci/project.yml`, which step 3a covers.

The substeps below are the same steps one at a time, for a partial run or a rerun.

### 1a. Make the trunk the default branch

Automated: `rk setup step default-branch --apply`. Where a repository has no such branch, the step first creates it at the tip of the current default.

On github:

```bash
gh repo view <repo> --json defaultBranchRef -q .defaultBranchRef.name
# check: prints master, whether the run set it or found it set
```

### 1b. Retire every other long-lived branch

Automated: `rk setup step single-trunk --apply`. This is the one destructive step. Its guard deletes a candidate only when it is an ancestor of the trunk. It fails closed on anything else.

On github:

```bash
gh api "repos/<repo>/git/ref/heads/<candidate>"
# check: 404s for each retired candidate; the script names the candidates it retires
```

### 1c. Delete every branch its merge retires

This step runs under the workflow guards alone. Without them, `rk setup` reports the step not applicable, and the project's own workflow decides. Automated: `rk setup step merge-cleanup --apply`. It is one switch on the repository, so it holds however anyone merges. A merged branch left behind becomes another long-lived branch. 1b used a destructive guard to remove exactly that.

On github:

```bash
gh api "repos/<repo>" -q .delete_branch_on_merge
# check: prints true, whether the run set it or found it set
```

### 1d. Remind this clone after a pull

This step runs under the workflow guards alone, as 1c does. Automated: `rk setup step branch-reminder --apply`. 1c deletes the remote copy. The copy in this clone survives with its upstream marked gone, and no forge can reach it. The step writes a post-merge hook that runs `rk branches prune --quiet` and `rk worktree prune --quiet` after every pull. The hook is silent when the clone is clean. Otherwise it prints a report that names the retired branches and worktrees. It never deletes anything, because the `--apply` forms are the operator's own call. Each call sits behind a capability probe on its own verb, `rk <verb> --help`. So an `rk` that is missing or too old for a verb keeps the hook silent. A genuine refusal still reaches the operator. The step refuses over an existing post-merge hook that it did not write. There, merge the hook by hand. Guard each call behind its probe, as below. The step is local work alone and calls no forge. So it runs whether or not the forge CLI is installed.

```bash
hook="$(git rev-parse --git-path hooks)/post-merge"
if [ ! -e "$hook" ] && [ ! -L "$hook" ] || { [ -f "$hook" ] && [ ! -L "$hook" ] && grep -qF '# release-kit branch reminder' "$hook"; }; then
  cat > "$hook" <<'HOOK'
#!/bin/sh
# release-kit branch reminder
if rk branches prune --help >/dev/null 2>&1; then
  rk branches prune --quiet || :
fi
if rk worktree prune --help >/dev/null 2>&1; then
  rk worktree prune --quiet || :
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

Automated: `rk setup step ci-permissions --apply`. The chapter owns why the raised default reaches only a workflow that declares no permissions of its own.

On github:

```bash
gh api "repos/<repo>/actions/permissions/workflow"
# check: prints "default_workflow_permissions": "write" and "can_approve_pull_request_reviews": true
```

On gitlab:

The step also keeps merge request pipelines away from the protected variables and runners, because such a pipeline runs the code of its source branch.

```bash
glab api "projects/:id" --jq '.protect_merge_request_pipelines'
# check: prints false
# prints null: the instance is older than GitLab 18.1, where a merge request pipeline never reads a protected variable; the step notes it and writes nothing
```

### 2b. Create the bot identity

One action stays manual, on one forge, once per account ever.

On github:

The bot App needs a browser to create, because the manifest flow redirects through one. It happens once in the lifetime of an account, never per project. `rk forge github` carries the walkthrough field by field and the credentials to collect. It also carries the warning about the private key, which downloads exactly once. Commit the numeric App ID as `setup.bot.app_id` in `.release-kit/config.toml`. Then import the downloaded key:

```bash
rk setup import-key --target . <downloaded.pem>
# check: prints the written path and the next command; the download is gone
# a key already stands there: the import refuses, and --replace is the rotation below
# refused naming RK_BOT_PRIVATE_KEY_FILE or the user configuration: that source names the key, so the import writes nothing
```

The key lands at `${XDG_CONFIG_HOME:-$HOME/.config}/release-kit/bot/github/<app-id>.pem`, with mode 0600 in a folder with mode 0700. A key that already stands at that path needs no import. Two overrides name another path, and the first match wins:

1. `RK_BOT_PRIVATE_KEY_FILE`, for one run against another file.
2. The user configuration file `${XDG_CONFIG_HOME:-$HOME/.config}/release-kit/config.toml`, table `[bot.github.private_key_files]`, one entry `"<app-id>" = "<absolute path>"` per App.

`RK_BOT_APP_ID` wins over `setup.bot.app_id` for a run against another App. The committed configuration never holds a key path.

On gitlab:

Nothing is manual: 2c does the whole bootstrap. It reads the tier of the namespace on GitLab.com and prints it as a `note:` line. Then it creates a project access token. On a Free group, it creates a group service account instead, which is a member of this project alone. Run it as a project Maintainer for a token. Run it as the top-level group Owner for a service account. `rk forge gitlab` states these facts:

- the two forms
- the role that each form needs
- the scopes that the token carries

### 2c. Grant the bot this repository

Automated. On github, the step reads the key at the path that 2b resolves. Run it where the key file is readable. Where the current process cannot read it, `rk setup check` reports this step and 2d as `unreadable`, and names the path and the remedy.

```bash
rk setup step install-bot --target . --apply
# check: reports the bot covering this repository with the permissions its release automation needs
# unsatisfied naming a permission: approve the App's updated permissions on the installation's own settings page; no token this run holds can grant them
# refused on github: the grant write needs a user token; rk forge github walks the one it takes
# refused on gitlab naming a personal namespace: a Free personal namespace has no bot identity; move the project under a group, or use a paid tier
```

### 2d. Store the bot credentials

Automated. On github, the step reads the same key as 2c. The values travel on stdin.

On github, the step stores both secrets in the release environment, `release` unless `protection.release_environment` names another. A python target that publishes also keeps them in `pypi`. The deployment policy of each environment admits these refs alone:

- the trunk
- the release lines
- the release request branches
- the release tags

The step removes the repository copies once the trunk and every live release line run a workflow that names the environment. Until then it keeps them, and `rk setup check` names each ref that still runs the old workflow. A private repository on the Free plan holds no environment secrets, so there the repository copies stay, and the check names that limitation.

On gitlab, the step stores `RELEASE_BOT_TOKEN` masked and protected, so only a pipeline on a protected branch or tag reads it. A token stored before the flag is protected in place, with its stored value, and nothing is minted.

```bash
rk setup step bot-secrets --target . --apply
# check: reports the credentials stored
# already stored: they are overwritten silently, which is the rotation path
```

On github:

```bash
gh secret list --repo <repo> --env release
# check: lists RELEASE_BOT_APP_ID and RELEASE_BOT_APP_PRIVATE_KEY
gh secret list --repo <repo>
# check: lists neither, once every live release ref runs the upgraded workflow
```

On gitlab:

```bash
glab api "projects/:id/variables/RELEASE_BOT_TOKEN" --jq '{masked, protected}'
# check: prints true for both; the value never reaches the output
```

## 3. Protect the trunk, the tags, and the release branches

### 3a. Prove the required checks exist

The trunk protection names check contexts. A context that no workflow reports is unsatisfiable, and nothing merges. The forge gate is the context that the protection requires beside the check of the project. It is `pr-title` under `git.merge_form = "squash"`, and `pr-commits` under `rebase`, which judges every commit of a request.

A forge gate lands only under forge integration or an automatic release. Under local integration with no automatic release, as `rk init --workflow-only` lands, no request reaches the trunk. The GitHub trunk ruleset requires no check, and the GitLab trunk requires no pipeline. This substep then has nothing to prove, so 3c takes no `--required-check`.

On github:

```bash
gh api "repos/<repo>/contents/.github/workflows/ci.yml" -q .content | base64 -d | grep -E '^\s+(<name>:|pull_request:|needs:)'
gh api "repos/<repo>/contents/.github/workflows/pr-title.yml" -q .name
# check: the job id and its pull_request trigger appear, the job's needs line names every job whose failure must hold the merge, and the trunk carries pr-title.yml
# a job named from a second workflow file is one the gate cannot need: rk status --check names the remedy for a generated artifact workflow
rk setup check --target . --required-check <name>
# check: protect-trunk reports ok once 3c has applied; a fault here names the gate shape to fix before the protection stands
# 404 on pr-title.yml: step 4 has not reached the trunk, and protecting now would block the very request that lands it
```

A project that keeps older lines reads the same file for `release/**`. A pull-request filter selects by base branch. So the required check reports on the request of a line only where the project's own workflow watches the lines too. [The release-lines runbook](./release-lines.md) step 2 owns that wiring. Do it before you cut the first line.

On gitlab:

There is no check to prove. The merge gate is the whole pipeline. The project's own jobs go in `.gitlab/ci/project.yml`, which the landed pipeline triggers as a child pipeline. Declare them after step 4 lands the pipeline:

```bash
mkdir -p .gitlab/ci
"$EDITOR" .gitlab/ci/project.yml
# declare the jobs, and the stages they use, inside this one file
git add .gitlab/ci/project.yml && git commit -m 'ci(project): declare this project'"'"'s own jobs'
```

Then open a merge request. Read its pipeline in this order:

- the head pipeline of the merge request
- the jobs of the parent
- the bridge and the child that it triggered

`MR` is the iid of the merge request, which `glab mr list` prints.

```bash
MR=<iid>
PIPELINE="$(glab ci get --merge-request "$MR" --output json --jq .id)"
test -n "$PIPELINE" && echo "$PIPELINE"
# check: prints the pipeline id of this merge request's head
# empty: the merge request has no pipeline, so the configuration did not compile or the parent's workflow rules matched nothing; read the forge's own error on the merge request
glab api --paginate "projects/:id/pipelines/$PIPELINE/jobs" --jq '.[].name'
# check: mr-title is listed; the parent's ordinary jobs are here and the bridge is not, because a trigger job is not an ordinary job
CHILD="$(glab api --paginate "projects/:id/pipelines/$PIPELINE/bridges" --jq '.[] | select(.name == "project-jobs") | .downstream_pipeline.id')"
test -n "$CHILD" && echo "$CHILD"
# check: prints the id of the child pipeline the bridge triggered
# empty: the file is absent, or its exists: guard did not match, so the bridge stayed out of the pipeline
glab api --paginate "projects/:id/pipelines/$CHILD/jobs" --jq '.[].name'
# check: the child pipeline holds the jobs .gitlab/ci/project.yml declares, and nothing this convention owns
```

Compare the wait of the merge request pipeline with the usual runner queue. If it waits longer, tell a queued job from a stuck one. Do it before you wait more.

A `pending` job can be queued or stuck, and the REST job fields read the same for both. One GraphQL query decides it, on `pending` jobs alone. A `created` job waits on an earlier stage or on a `needs:` job, not on a runner:

```bash
glab api graphql -f query='{ project(fullPath: "<repo>") { jobs(statuses: [PENDING]) { nodes { name status stuck } } } }'
# check: stuck false means the runner queue is slow and waiting is correct; stuck true means no runner will take this job, and the cause is runner configuration, not patience
```

On github, the same question has evidence and no verdict:

A queued job is evidence, not a verdict:

```bash
gh run view <id> --repo <repo> --json jobs -q '.jobs[] | select(.status == "queued") | .name'
gh api repos/<repo>/actions/runners -q '.runners[] | select(.status == "online") | {name, labels: [.labels[].name]}'
# check: a job on a hosted label waits for capacity; a job on self-hosted labels needs one online runner that carries every label it requests
# no such runner: the job stays queued until a matching runner comes online, and fails after 24 hours in the queue
```

Every list call paginates, because these endpoints answer twenty rows a page. Without it, a project with more jobs than that reads as complete. The trigger-job endpoint is `bridges` on an instance at the 18.2 floor. GitLab 19.2 renames it `trigger_jobs` and keeps `bridges` as a deprecated alias.

Prove the gate once. Make one child job fail on purpose. The parent pipeline must turn red, and the merge must refuse. A job meant to gate carries no `allow_failure: true` and no optional `when: manual`. Either one lets the child pipeline finish without the proof. [The forge document](../forges/gitlab.md) carries the rest of the rules that the file follows.

### 3b. Prove the forge meets the version floor

Automated: `rk setup step forge-version --apply`. It reads and writes nothing.

On github:

Satisfied without a call. github.com is a rolling service and declares no version floor.

On gitlab:

The step makes one `GET /version` call and compares the answer against the minimum that [the forge document](../forges/gitlab.md) names and explains. The step refuses an instance below it here, before 3c installs a merge check that its pipeline cannot feed. A version that the step cannot read blocks the same as one that is too old. Where the reading is unknown, run `glab auth login`. Then rerun the step.

```bash
rk setup step forge-version --target . --apply
# check: reports the instance's version and that it is at or above the floor
# unsatisfied: a self-managed instance is below 18.2 or reports a pre-release; upgrade it
#   (gitlab.com satisfies the floor, and its -pre suffix is the deployment's own)
# unknown: glab could not read the version; authenticate and rerun
```

### 3c. Protect the trunk

Automated: `rk setup step protect-trunk --apply --required-check <name>`. On GitHub, run it only after 3a passes, per the order of the chapter. On GitLab, run it only after 3b passes. A rerun updates the protection in place. The recorded merge form names the one merge method that the protection allows. Under `squash`, GitHub takes the squash merge with the title and body of the request as its message. Under that form, GitLab squashes every merge request. Under `rebase`, GitHub takes the rebase merge alone, with the linear-history rule in the ruleset that no actor bypasses. GitLab sets the `ff` method with squash off. After `rk upgrade --merge-form <form> --apply`, rerun this substep. The forge holds the settings of the old form until you do.

On github, the step installs three rulesets, in this order:

1. The safety ruleset, `<trunk>-safety`, refuses a deletion and a force-push to every actor.
2. The updates ruleset, `<trunk>-updates`, carries the update rule alone. Only the repository administrator and the release App bypass it, so no other identity with write access updates the trunk.
3. The trunk ruleset, `<trunk>-protection`, requires the request and the checks.

The release App merges an ordered release request with an explicit bypass of the update rule alone. The request rule and the required checks still bind it. `setup.bot.app_id` or `RK_BOT_APP_ID` names the App. Without one, the updates ruleset names the administrator alone, and the App cannot merge the release request.

On gitlab, the trunk takes a push at No one and a merge at Maintainer. A Developer neither pushes the trunk nor merges into it. A recorded `protection.gitlab.merge_access_level` below 40 refuses before any forge call.

### 3d. Protect the tags and the release branches

Automated: `rk setup step protect-tags --apply`, then `rk setup step protect-release-branches --apply`. Only when an older line exists, also `rk setup step protect-release-lines --apply`.

On github:

- `protect-tags` installs the immutable `release-tags`, which refuses an update or a deletion of a release tag to every actor.
- `protect-tags` installs `release-tag-creation` beside it. Only the administrator and the release App create a release tag. On rust, it covers every tag that carries an `x.y.z` version with `refs/tags/**/*[0-9].[0-9]*.[0-9]*`, because the generated artifact workflow starts on such a tag, `x/1.2.3` included. A tag with no such version stays open.
- `protect-release-branches` installs the ruleset `release-branches` over the release lines, `release/**/*` by default, and over `release-plz-*`, `release-plz-*/**/*`, and `rk-release/**/*`. The `**/*` form covers a nested name such as `release/a/b`, which a bare `**` misses. Only the administrator and the release App create or update such a branch. So no other identity pushes a commit onto an open release request.

On gitlab:

- `protect-tags` protects the tag name of `protection.tag_pattern`, `v*` or `*`, with one create grant at Maintainer.
- `protect-release-branches` protects the release lines, `release-plz-*`, and `rk-release/*`, each at Maintainer for a push and a merge, with force pushes off.

Under `release.mode = "none"`, the release mints no tag. So these three things follow:

- `protect-tags` and `protect-release-branches` report not applicable.
- A run that names either by hand refuses.
- 3f requires no tag protection.

A full apply skips the release-lines step until `setup.release_lines` is true in `.release-kit/config.toml`. A project that keeps lines sets it once.

### 3e. Leave every release request to the operator's order

Automated: `rk setup step merge-on-order --apply`. No landed job arms or merges a release request, so a release waits for the operator's merge. The step disarms each open release request that an rk before 0.9.15 armed. It leaves every other request as it is.

On github:

```bash
gh pr list --repo <repo> --state open --json number,headRefName,autoMergeRequest
# check: no release request carries an autoMergeRequest; the repository's allow_auto_merge switch stays as the project set it
```

On gitlab:

The step also asserts the pipeline requirement, which holds a person's merge behind a passing pipeline.

```bash
glab mr list
# check: no release request merges when its pipeline succeeds
```

### 3f. Prove the protections

```bash
rk setup check --target .
# check: the protection steps report satisfied — protect-trunk reads back strict_required_status_checks_policy=true with required checks on GitHub, or merge_method=ff on GitLab; merge-on-order reports no armed release request, and protect-release-lines is skipped while no line exists
# private-vulnerability-reporting unsatisfied or unknown: continue at 3g before calling setup ready
# protect-trunk unsatisfied: return to 3c; a loose or absent strict policy faults even when checks are required
# protections-check unsatisfied naming a ruleset, a pattern, or a bypass actor: rerun the substep that the remedy names
# bot-secrets unsatisfied naming refs: those refs still run a workflow that names no environment; bring the upgraded workflow there, then rerun 2d
# install-bot and bot-secrets unreadable: this process cannot read the key file; the detail names the path and the remedy, so run the check where the key file is readable, or read rk setup status
# install-bot unknown: this runtime cannot read the App's installation, which proves nothing wrong. Read rk setup status; rk forge <forge> owns why only the bot reads its own installation
```

Where the forge enforces less than a step claims, the check names the weaker guarantee and does not pass. Tag protection on GitLab is the case for this, per `rk forge gitlab`.

### 3g. Provide a private vulnerability reporting channel

1. Run `rk setup step private-vulnerability-reporting --target . --apply` (check: public GitHub reports applied after the step enables the switch and reads it back. A rerun reports satisfied. Private GitHub reports skipped and does not call the public-only endpoint. GitLab reports satisfied with its confidential-issue limitation).
2. Inspect the result of the step (check: GitLab states that confidentiality is a per-report choice subject to reporter access. It is not a project-level switch, and not proof that every external reporter can submit). If authentication or connectivity is unknown, repair that access. Then rerun 3g. If GitLab intake is disabled or restricted, have a Maintainer or Owner decide whether to allow intake. Do this before you change access. Then use these controls:
   - Open project Settings > General. Expand Visibility, project features, permissions.
   - Enable Work items (Issues on the supported older interface).
   - Set its access to Everyone with access. Leave all other settings unchanged.
   - Select Save changes (check: the project readback shows `issues_access_level: "enabled"`. Otherwise, return to these controls).
   - Rerun 3g.
3. Run `rk setup check --target .` (check: this step reports satisfied for public GitHub and skipped for private GitHub. GitLab reports satisfied with the named limitation. For any other reporting result, return to 3g). The check of policy presence comes after its producer, step 4a. Setup writes no policy and creates no test report.

## 4. Land the workflow files

### 4a. Land the files

The apply names no scope vocabulary. The title check holds a scope to these characters:

- lowercase letters
- digits
- `_ . / -`

`--workflow-guards` adds the opt-in workflow guards:

- the agent and commit policy lines of the landed `AGENTS.md` block
- `GLOSSARY.md`
- these hooks:
  - the commit-message hook
  - the branch-name hook
  - the worktree hook
  - the trunk hook
- steps 1c and 1d

A fresh landing takes none of them, because they shape how the project works, not what its release needs. `rk upgrade --workflow-guards on` or `off` changes the answer later.

The landing does not ask the checkout mode. `git.checkout_mode` keeps its default, `linked-worktree`. It sets where the helper verbs open a topic branch, and under the workflow guards, whether the location hook lands.

`--integration` chooses which authority moves an implementation onto the trunk. It defaults to `local`: the checkout squashes and records it through `rk integrate`, with no forge round trip. `--integration forge` keeps a pull request or merge request as the integration. A project with more than one writer and a review requirement wants that. Ask for this answer. Do not assume it. A project that never answered has an answer that it did not choose. The release request integrates at the forge in both modes.

`--merge-form` chooses how the commits of a branch reach the trunk. It defaults to `squash`, one trunk commit per implementation. `--merge-form rebase` keeps every commit of the branch, each with its own message. Where a forge gate lands, it lands the commit gate in place of the title check. [The integration runbook](./integration.md) carries the choice.

`--workflow-only` lands the Git workflow with no release. It sets these answers:

- the release mode `none`
- local integration
- the `linked-worktree` checkout mode
- the workflow guards

[The landing runbook](./landing.md) step 4a names what it lands and what it omits. A project that releases nothing then skips steps 0, 2, 5, 6, and 8. Of step 7, it keeps 7a and 7e.

`--release-style` chooses the release style of an automatic release and defaults to `trunk`: the project ships from the trunk alone. `--release-style lines` is for a project that keeps older lines and validates a candidate by hand. In neither style does a request merge itself. A release waits for the operator's merge. To check the answers, run `rk status`. It prints these values:

- the checkout mode
- the integration mode
- the style

The landed block holds release-kit's own hooks alone. The project owns everything in `.pre-commit-config.yaml` outside the markers. There it decides what goes where:

- each check, on the earliest native stage that can run it
- the suite that must pass before an integration, tagged with the `pre-merge-commit` stage
- the project's own continuous integration, which runs those same checks

Release-kit asks git to run the `pre-merge-commit` hook of the clone. It names no hook runner and reads no hook identifier, so the assignment stays with the project. [Integration](../method/12-integration.md) owns the rule.

Each landing goes through five parts:

- a stage
- a comparison
- a preparation
- a render
- a proof

The first three parts are [the landing runbook](./landing.md) steps 1 to 3. They come before the front below, which is that runbook's step 4. Then its step 5 and the close after it carry the rest:

- the proof
- the checks
- the cleanup

```bash
rk profile --target .                        # what the target resolves to, and what the catalog selects
rk init --stage <stage> --target .           # preview every destination, with the answers the stage resolved
rk init --stage <stage> --target . --apply   # write the files and the landing record
# check: the apply reports each written file, including SECURITY.md, and every sentinel left to fill
# already landed: the apply refuses; rk upgrade --target . --apply takes an existing landing to the installed binary's projection
# a record from before the style parameter: the upgrade refuses until --release-style names one
```

After landing, read `SECURITY.md`. Its project path must name this target. Its instructions must describe the intended private reporting channel. If they do not, return to 4a with the correct repository parameter. Existing landings receive the policy through `rk upgrade`.

Release-kit owns this rendered policy. An upgrade refuses a hand edit until the operator names it with `--replace`. Then the upgrade saves a copy and replaces the file. So the three facts that only this project knows are landing parameters instead:

- Set `security.contact` in `.release-kit/config.toml` to one line that names who receives a report when the forge channel is unavailable. Leave it empty to keep the wording of the forge. The line can name one of these:
  - an address
  - a URL
  - a person
  - a team
- Set `security.response` to one of these values. A window promises that the maintainers acknowledge a report within it. The value promises acknowledgment alone, never remediation or disclosure.
  - `best-effort`
  - `1 day`
  - `<n> days`
  - `1 business day`
  - `<n> business days`
- Set `security.scope` to one paragraph about what this project does not own. Examples are a wrapped tool or a vendored tree. In that paragraph, name where to report a flaw in it. Leave it empty to state nothing. The paragraph carries none of these:
  - a blank line
  - a heading
  - `<!--` or `-->`

All three are class P. `rk status` reports an edited key as pending configuration. The next `rk upgrade --apply` takes the key and records it (check: `rk status` lists the key under pending, and the applied policy states the new wording).

After step 4c publishes the policy on the target, check that the hosted file is visible. Check that the reporter can reach the intended private form. On GitLab, open the new issue form of this project on its own instance. Check that confidentiality is selected. Do not guess a host for the policy. On private GitHub, check that the stated existing private conversation is available instead. Submit no report for this check. If the channel is unavailable, return to 3g. If the hosted file is absent, return to 4c.

Then answer every reported sentinel:

```bash
grep -rn 'TODO(release-kit)' . --exclude-dir=.git
# check: prints nothing once each sentinel is answered
```

### 4b. Gate the artifact plan

On rust:

On github, first add the release proofs to the project's own gated workflow. Then regenerate the artifact workflow at the pin. Read what a release will build. Keep that order. The generated workflow stops reporting on a request as soon as it is regenerated, and the gated job replaces it. [The binding](../bindings/rust.md) carries the rest:

- the commands
- the job
- the no-diff proof

Where the target opts into Nix, its build proof is a job of the same workflow. The binding carries that job too. On gitlab, the pair builds no artifacts, so there is no plan to gate. The Nix build proof there is a job of `.gitlab/ci/project.yml`, which 3a covers. The binding serves that job too.

### 4c. Put the files on the trunk

First, write the setup proof that the landing closes on, from a host that holds the key of the bot. Then confirm the landing. Steps 0 to 3 ran, and 4b put the gated jobs in place. So every step that the check reads can hold:

```bash
rk setup check --target .
# check: every applicable step satisfied; a step this forge cannot run goes in setup.excluded_steps with its reason
rk setup checkpoint --target .
# check: wrote .release-kit/setup-proof.json
rk status --check --target .
# check: reports the landing as current, with the setup proof compatible
```

The landed files and `.release-kit/setup-proof.json` reach the trunk together, through the project's own workflow. That is a direct commit while no protection stands, or the usual change path of the project once one does. Release-kit prescribes neither.

```bash
git fetch origin
git log -1 --format='%h %s' origin/master -- .release-kit/manifest.json
# check: prints the commit that put the landed record on the trunk
```

### 4d. Install the hooks, last

Install the hooks last, because under the workflow guards two of them refuse a direct write to the trunk.

The landing splices a marked block of release-convention hooks into `.pre-commit-config.yaml`, under `repos:`. The rest of the file stays the target's own. The block holds:

- the tag guard and the status check, always
- under the workflow guards, the commit-shape hooks
- under the workflow guards, the `rk-message` content guard. It refuses a message that carries agent attribution or a reference to a git-ignored path. The request of the release bot is exempt by its title.

A hook that already does one of these jobs is a duplicate to name. Examples:

- `committed`
- `commitlint`
- `gitlint`
- another conventional-commit or branch-guard hook

The operator chooses between it and the landed hook. The choice is never a silent second hook that does the same job.

On an existing configuration, check that the top level carries `default_install_hook_types: [pre-commit, commit-msg, pre-push]`. Under local integration, add `pre-merge-commit` to it. The splice cannot add a top-level key.

The CI of a target can run the hooks with prek. If it does, run each stage with its group filter, as the comment of the block shows: `--no-group local-only` at the `pre-commit` and `pre-push` stages, and `--group ci-only` at the `manual` stage. A CI checkout is commonly detached on the main worktree, or on the trunk, and commits nothing. So the commit-time guards of the workflow guards refuse it, and the block puts them in the `local-only` group. Put each hook of the project that CI cannot run in the same group, with its reason in a comment above it. Leave the `ci-only` step out where no hook carries that group. Set no `SKIP` in a CI job. A check then runs on both sides of the boundary, or says why it cannot.

```bash
prek install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push --hook-type pre-merge-commit
# check: names each of the four hooks it installed; on git 2.54 or newer, git hook list pre-merge-commit names one too; another hook manager takes the same stages
```

## 5. Publish the first version by hand

The upload is permanent. The registry never lets anyone overwrite a version, and the name stays claimed for good. A rust or python project whose `profile.release.registry` is `none` skips steps 5, 6, and 8. Like bash, it publishes to no registry, and step 7 proves its first release. A python project whose name PyPI does not hold yet also skips step 5. Step 6 registers a pending trusted publisher, and the first engine release uploads with no token.

### 5a. Gate the tree and read the version

```bash
git fetch origin
git status --porcelain
# check: empty
git rev-parse HEAD origin/master
# check: two identical SHAs; HEAD is what the publish packages
# they differ: the trunk takes no direct push, so land the commits through the project's own change path and rerun
```

On rust:

```bash
cargo metadata --no-deps --format-version 1 -q | jq -r '.packages[0].version'
# check: note it; 5c asserts against it
```

On python:

```bash
uv version --short
# check: note it; 5c asserts against it
```

### 5b. Mint the bootstrap token

This step is manual, in the registry web UI, which has no API. Do it once per package. The binding walks the form field by field. Scope the token to publishing new versions of exactly this package, with the shortest expiry.

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
uv build
uv publish                                   # reads the token from UV_PUBLISH_TOKEN
curl -fsS -o /dev/null -w '%{http_code}\n' "https://pypi.org/pypi/<package>/<version>/json"
# check: prints 200 for the version 5a reported
# already published: the registry refuses a version it already serves; go to step 6
```

On bash, and for a rust or python project whose `profile.release.registry` is `none`:

There is no registry. The automated path in step 7 proves the first release, and nothing is published by hand.

## 6. Register the trusted publisher

Skip this step for a project that publishes to no registry. This step is manual, in the registry web UI, which has no API. Do it once per package. The binding walks the form and its read-back check. Register these three values:

- the owner
- the repository
- the filename of the publish workflow

The filename must be the one that stays true. Then revoke the bootstrap token, both halves: the registry-side revoke and the host-side logout that the binding carries. The package then has exactly one publishing path.

## 7. Prove the automated path

### 7a. Prove the protected path

From 3c on, the trunk takes a change only through its protected path. The recorded integration mode names the path:

- Under `local`, `rk integrate` squashes the branch in the checkout, or under `rebase` moves the trunk to the branch tip. A repository administrator then fast-forwards the trunk.
- Under `forge`, the forge merges a request whose required checks pass onto the trunk, in the recorded merge form.

Prove the path once with one real change, through the variant of [the integration runbook](./integration.md) for the recorded mode.

Under `squash`, the squash title becomes the subject of the trunk commit. Write it as a scoped Conventional Commit that carries the strongest intent of the branch. One `!` anywhere makes the title breaking. Under `rebase`, every commit message reaches the trunk as written, so each one carries that convention. Under that form, `rk integrate` takes no `-m`. Under `squash` on github, the description of the request becomes the body of the commit. So the description carries no agent attribution and no reference to an internal planning artifact. Under `rebase`, the body of each commit reaches the trunk, so the same holds for each commit message.

On local:

```bash
rk integrate <type>/<slug> -m '<type>(<scope>): <subject>' --apply
# check: the report names the trunk's new commit and records the integration
git push origin master
# check: a fast-forward, pushed as a repository administrator; never --force
```

On forge:

```bash
rk integrate <type>/<slug> --forge --apply
# check: the report names the request command for this forge; open the request with it
# check: the required check reports success, and the forge merges the request onto the trunk in the recorded merge form
```

- The check never appears. The job id differs from the one that the ruleset names, and the prerequisite of 3a moved. Nothing merges until the ruleset and the workflow agree on the name.

### 7b. Prove the publish workflow runs

On github:

```bash
gh run list --repo <repo> --workflow <publish workflow> --limit 1 --json conclusion,headSha -q '.[0]'
# check: the newest run on the trunk concluded success; the binding names the workflow file
```

### 7c. Prove it produced a release request

On github:

```bash
gh pr list --repo <repo> --state open --json number,title,headRefName
# check: a release request from the bot is open, proposing the next version, from release-plz-<suffix> on rust or rk-release/master on bash and python
# none open: the trunk matches the version step 5 published, so there is nothing to propose; land one change through 7a and rerun
```

On gitlab:

```bash
glab mr list
# check: a release request from the bot is open, proposing the next version, from release-plz-<suffix> on rust or rk-release/master on bash and python
```

### 7d. Cut one release end to end

```bash
rk guide release
```

The chapter names its passing verify step as the proof that the next step depends on.

### 7e. Refresh the setup proof

4c wrote the first proof. Read it again once the automated path is proven. If it reads anything but compatible, write it again from a host that holds the key of the bot:

```bash
rk setup status --target .
# check: setup proof: compatible; anything else runs the checkpoint below
rk setup checkpoint --target .
# check: wrote .release-kit/setup-proof.json; an unsatisfied or unknown step writes nothing, so return to the substep that owns it
```

Commit a refreshed `.release-kit/setup-proof.json` through the protected path of the trunk, as in 7a. The proof closes the setup. On a target with an automatic release, `rk status --check` counts an absent, stale, or invalid proof as a violation. A step that this forge cannot run goes in `setup.excluded_steps` with its reason, which keeps the proof complete. A runtime without the key reads `rk setup status` and does not repeat the check. After any change to the setup configuration, `rk setup status` names the changed field. Then run this substep again.

An upgrade or a new answer that moves the setup contract takes the same order. Land first. Then run `rk setup check --target .`. Then run `rk setup checkpoint --target .`. Then commit these together:

- the landed files
- the configuration
- the proof

## 8. Require trusted publishing

Skip this step for a project that publishes to no registry. This step is manual, in the registry web UI, which has no API. Do it once per package. Do it only after step 7 proved one OIDC release. The binding names the switch and what it rejects from here on. It also names the recovery escape, which starts when you turn the switch off.

## Rotate the bot key

Rotate the key of the GitHub release App once a year. Rotate it at once on any suspicion of a leak. These steps apply to a key that `rk` manages, a regular file at the default path or at an override. The App holds up to 25 keys, so the old key works until the last step.

1. Open the App's settings. Under Credentials, then Key pairs, click New key (check: a `.pem` downloads, once).
2. For a key at the default path, run `rk setup import-key --target . --replace <downloaded.pem>` (check: it prints the written path, and the download is gone).
3. For a key at an override, replace the file at that path with the download, with mode 0600 (check: `ls -l` on the path shows `-rw-------`).
4. Run `rk setup step bot-secrets --target . --apply` (check: it reports the credentials stored).
5. Run `rk setup check --target .` (check: `install-bot` and `bot-secrets` read satisfied).
6. On the App's settings, under Key pairs, delete the old key (check: the page lists the new key alone).

If the check at step 5 fails, keep the old key, and repair the new one first. `rk setup import-key --replace` refuses a default path that is a symbolic link, because such a key is managed elsewhere. Rotate that key where it is managed, then run steps 4 to 6.

`rk` records no rotation date. The date is a fact of the App and of the machine that holds the key, not of a project, and a tool that manages the file can rewrite its time at any moment. Keep the yearly date where the key is kept.
