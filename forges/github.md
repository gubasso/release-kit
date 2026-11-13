# GitHub

This page states how this forge answers the method's fifth axis. The CLI is `gh`, and `rk setup` runs the `setup/github/` tree against it.

## Answers

Vulnerability reports use GitHub's private reporting channel on public repositories. The required `private-vulnerability-reporting` setup step enables the channel and reads it back. The step skips private repositories as inapplicable. The `SECURITY.md` that `rk init` lands explains the channel and its eligibility. The target is ready only after both the file and the forge setup are complete. Where the form is unavailable, the policy names a fallback contact, which `security.contact` in `.release-kit/config.toml` supplies. An empty value keeps this forge's own wording, an existing private conversation with a maintainer. `security.response` states the acknowledgment window that the same policy promises. `security.scope` states what the project does not own and where a reporter goes instead. The policy puts that statement in one paragraph after the reporting channel. An empty value states nothing.

An OpenSSF Scorecard run reads this convention's own decisions. A landed target asks about the two checks below. Scorecard is the published score of the OpenSSF over the supply-chain practices of a project. `_docs/reference/REFERENCE-forge-setup-sources.md` carries the dated citations behind every number here.

Branch-Protection scores in cumulative tiers, so an unsatisfied tier forfeits every tier above it. A run from a workflow token treats each requirement that the check marks "for administrators" as met. The landed safety ruleset refuses deletion and force-push, which is tier one and three points. The trunk ruleset requires two status checks, which is tier three. Between them sits the one tier-two requirement that this convention leaves to the target: at least one required reviewer. `protection.required_approving_review_count` floors at zero. A target that keeps the default reads three out of ten rather than eight, and it is not misconfigured. One required reviewer reaches eight. Ten needs two reviewers and a code owner review, which a solo-maintained project cannot honestly require. The empty `bypass_actors` list of forge integration satisfies the tier-five requirement, no excluded administrator. Local integration trades that tier away on purpose. Its sole bypass, the repository administrator, admits the direct trunk push and leaves integrations governed.

Pinned-Dependencies cannot reach ten while cargo-dist generates `.github/workflows/release.yml`. Scorecard counts a download piped into a shell as one unpinned dependency. The generated file installs cargo-dist and rustup that way. `_docs/reference/known-issues/KI-dist-generates-an-unhardened-workflow.md` owns that file. The same generation is why a Scorecard run has no inline suppression either. This convention pins every action by commit, per `_docs/decisions/ADR-pin-every-action-by-commit.md`. So the actions half of the check costs a landed target nothing.

A workflow that publishes its Scorecard result carries restrictions of its own. Where one is broken, the publishing API rejects the result and fails the run. These are the restrictions:

- The workflow declares none of these:
  - a top-level `env`
  - `defaults`
  - a write permission at workflow level
- Only the job that runs the action carries `id-token: write`.
- That job declares none of these:
  - `env`
  - `defaults`
  - a container
  - a service
- That job runs on a hosted Ubuntu runner.
- That job carries steps from the approved list of the action only.

A target that lands such a workflow reports on no pull request, so the workflow cannot reach the required gate.

`rk init --code-scanning codeql` lands `.github/workflows/code-scanning-codeql.yml`. `--code-scanning semgrep` lands `.github/workflows/code-scanning-semgrep.yml` beside it as the alternative. One provider lands one destination, so a switch retires the other. The landing removes the other file where nobody edited it, and leaves an edited one as the target's own. Both are `rendered`. Neither triggers on a pull request. That is the rule this forge states above, not a limitation of the workflow. `needs` resolves inside one workflow file, so a second workflow that reports on a request holds no merge, whatever it proves. Each workflow uploads its SARIF to the code scanning of this repository. So the job takes `security-events: write`, and the workflow level takes nothing. The job also takes `actions: read`, because the action reads the workflow run that it reports against. Only a private repository needs that permission, and without it the upload fails there. The landing does not know whether the target is private, and read access to its own runs costs a public target nothing. A private target that fetches private CodeQL packs adds `packages: read` beside it. The default query packs need none. Neither workflow adds a setup step. A scanner reads one language, so both workflows ship in the binding's own pair, not in the shared zone of this forge. A binding that ships none refuses the flag by name. `codeql` carries a licence condition that the binding owns. `semgrep` carries none. [The Rust binding](../bindings/rust.md) states which field the licence is read from and which three providers were refused.

`rk init --scorecard` lands that workflow, and the landing records the answer as `capabilities.scorecard` in `.release-kit/config.toml`. The target gets `.github/workflows/scorecard.yml`, a `rendered` file that release-kit owns. The workflow runs on a push to the recorded trunk and once a week. It computes the score and publishes it to the public Scorecard API, and that publication makes the badge and the site entry resolve. It uploads no SARIF, so it needs neither code scanning nor `security-events: write`. It adds no setup step, because a run needs no forge setting on a public repository. The capability is off by default. A run posts to a public API and reads repository metadata, which is a choice and not a guard. `rk upgrade --scorecard off` drops the destination from the receipt and removes the file where nobody edited it. `ossf/scorecard-action` publishes no moving major tag. So `versions.toml` pins the exact release tag, and only a retag moves the discovery ref.

- The release request is a pull request against the trunk. The trunk is the default branch, which the bot targets with no configuration. On the rust pair, release-plz refreshes the request by force-push while only its own commits sit on the request's branch. Once a human commit lands there, the next refresh closes the request, opens a fresh one, and takes the commit with it. On the bash and python pairs, the rk engine keeps one request per base on `rk-release/<base>`. The engine refreshes that request in place by force-push on every run. So the request keeps its number, and the next refresh drops a commit that a person pushed there. Either way, a changelog correction is the last act before the merge.
- The gate is the merge of the release request itself, and the trunk's ruleset enforces it. The ruleset allows squash as the only merge method and requires two status checks, matched by name. One is the one job that the project's own workflow reports as its gate. The other is the landed `pr-title` check. Under forge integration, no direct push bypass exists. Under local integration, only the repository-administrator role bypasses those rules, for the deliberate push. The release App stays governed, and the deletion and force-push rules sit in a second ruleset that nobody bypasses. The request must also carry the trunk's tip. `strict_required_status_checks_policy=true` enforces freshness at merge time in both modes, and `rk setup check` faults a false or absent policy. `needs` names job ids inside one workflow file, so that gate reaches no job in another file. A second workflow that reports on a request gates nothing, whatever it proves. That is why the landed workflows report on a request from the title check alone. It is also why the landing configures a generated artifact workflow to report on none.
- `gate.needs` is the voting list, by convention and not by proof. A job that must block a merge belongs in the gate's own workflow file and in that list. A job that runs on a pull request and does not appear there does not hold the merge. This matters more here than elsewhere, because the operator who merges the release request reads the named check alone, not the check list. `rk setup check --required-check <name>` judges the shape of the gate. The gate must report in this shape:

  - uniquely
  - under a condition that survives a failed dependency
  - with a literal `needs`
  - on a trigger that reaches the request

  The check says nothing about the jobs that the gate does not name. Whether a job is meant to block is intent, and no file states it. The squash message is the request's title and body, because `protect-trunk` sets `squash_merge_commit_title=PR_TITLE` and `squash_merge_commit_message=PR_BODY`. When they are unset, a one-commit request offers that commit's own subject, and that is how a `wip` subject reaches the trunk. The description becomes the body of the trunk commit, so the `pr-title` workflow also greps it against the content guards. The landed `rk-message` hook is the stronger check, because it reads the ignores. The observation faults a message source that is not `PR_BODY`.
- This convention refuses a merge queue and uses the aggregate gate instead. A queued repository holds every request until a check that triggers on `merge_group` reports. The landing ships no workflow that does. So the queue waits on a check that never comes, and drops the request when its CI timeout expires. `rk setup check` faults a queue on the trunk by name and states the remedy: `rk setup step protect-trunk --apply` rewrites the ruleset without it. A project that wants a queue wants a different convention, not a different flag.
- Protections are rulesets. A ruleset has separate create and update endpoints, so a rerun resolves the ruleset id and updates in place. These are the rulesets:

  - two that cover the trunk
  - one that covers the `v*` tags
  - one that covers the `release/*` branches, where a project keeps older lines
- The trunk takes two, because a bypass actor is a ruleset field and excuses every rule in that ruleset. The safety ruleset carries deletion and force-push protection and names nobody. The trunk ruleset carries the request rule and the required checks, and names whoever the integration mode excuses. `protect-trunk` writes the safety ruleset first, so no rerun leaves a window with neither. It writes that empty bypass itself and reads no key for it, so no configuration answer can widen who can rewrite the trunk.
- The name of a ruleset is the project's own. These keys name the four rulesets:

  - `protection.trunk_ruleset`
  - `protection.safety_ruleset`
  - `protection.tag_ruleset`
  - `protection.lines_ruleset`

  The setup and its check read the same value, so they cannot disagree about the name of a ruleset. A target that names no trunk ruleset gets `<trunk>-protection`, which is the name the step built before the key existed. A target that names no safety ruleset gets `<trunk>-safety`. A rename after the ruleset exists leaves the old one standing. The forge resolves by name, so the setup creates a second ruleset and does not rename the first. The operator deletes the old one.
- The protection's policy has floors and is not free. Each of these carries a minimum that the method states:

  - `protection.tag_pattern`
  - `bypass_actors`
  - `allowed_merge_methods`
  - `strict_required_status_checks`
  - `owned_trunk_rules`
  - the two squash sources

  The recorded merge form sets the floor of the merge method and the squash sources, and under `rebase` no squash source has a floor. The refusal of a weaker value names the key and the floor. It also names `rk method invariants`. The bypass floor depends on the mode: empty under forge integration, and exactly `repository-admin` under local integration. The review block has permissive floors. A project can require approvals and can dismiss stale reviews, and the floor says which direction is safe. `rk` judges every value before a step runs, so a step substitutes and never decides.
- `protection.title_check` names the context that the trunk protection requires for the landed title job. The job's own name inside `pr-title.yml` stays a constant of the rendered file. A target that renames the key and not the job requires a check that nothing reports, and the merge hangs. `rk setup check` reports that as a fault and does not pass it.
- Under `git.merge_form = "rebase"`, the trunk ruleset allows the rebase merge alone, and the safety ruleset adds `required_linear_history`. The required context is the landed `pr-commits` job in place of `pr-title`. Every commit of the request reaches the trunk with its own message. So that job reads the request's commits through the API on each push, and holds each one to the scoped convention and the content guards. The forge lists at most 250 commits of a request. The job fails a longer one and does not pass commits it never read. No squash source applies, so `protect-trunk` leaves both alone, and the observation reads neither. The repository's own `allow_rebase_merge` switch is separate from the ruleset. A repository that disallows the one method the ruleset allows can merge nothing. So `protect-trunk` turns that switch on and touches no other merge switch. Both checks fault the switch where it reads off or missing. They also fault it where it is unreadable. A rebase merge writes the commits again with new SHAs, so the merged request stays the proof that retires the branch.
- The issue link is an explicit relation, not a name match. `gh issue develop <issue> --checkout` generates the branch from the issue, named `<issue>-<slug>`, and records the link. The pull request from that branch attaches to the issue, and its merge closes the issue. A branch named by hand links only through a closing keyword, `Closes #<issue>` in the pull request body. The name comes from the server. The mutation behind the command takes an optional branch name that defaults to the issue number and title. So a mint that passes no name asks the forge to choose. `rk issue start` mints that way and then reads `Issue.linkedBranches` back for the name. That read also makes a second run adopt the branch and not mint it again.
- The bot identity is a GitHub App installed on the repository. Its token makes the pushes of the bot start workflows. A push with the default CI token starts no workflow, so a tag that it pushes starts no artifact build. A release request that the default token opens or refreshes creates its workflow runs in an approval-required state. A person with write access must select Approve workflows to run on the request, after every refresh. So the request reports no check until a person acts, and `rk release merge` refuses it until then. Under the App's token, the runs start with no approval. The rk engine refuses the default token before it pushes. GitHub's documentation, read on 2026-11-13, states this rule, and `_docs/reference/REFERENCE-forge-setup-sources.md` carries the source.

## Bootstrap

The creation of the App is the one action that no command performs, because the manifest flow needs a browser redirect. It happens once in the lifetime of an account, not once per project. `rk setup step install-bot` adds a project to an existing installation.

### Check whether the App exists

1. Open the account's App list.
   - Personal account: `github.com/settings/apps`
   - Organization: `github.com/organizations/<owner>/settings/apps`
2. Look for the bot App by name.
   - Listed: skip to installing it on the repository.
   - Not listed: register it first.

### Register it

Walk the form at `github.com/settings/apps/new` from top to bottom. Each item below is one of its sections.

1. Top of the form
   - GitHub App name: yours to choose, globally unique across GitHub, at most 34 characters
   - Description: optional, what the bot does
   - Homepage URL: any URL. The form requires one, and the bot ignores it
2. Identifying and authorizing users
   - Callback URL: empty
   - Expire user authorization tokens: leave it checked. The App mints no user tokens
   - Request user authorization (OAuth) during installation: unchecked
   - Enable Device Flow: unchecked
3. Post installation
   - Setup URL: empty
   - Redirect on update: unchecked
4. Webhook
   - Active: uncheck it. The three fields below disable once you clear it
   - Webhook URL: leave it
   - Webhook secret: leave it
   - SSL verification: leave it
5. Repository permissions
   - Contents: Read and write
   - Pull requests: Read and write
   - Checks: Read-only
   - Metadata: Read-only, set by GitHub and not removable
   - Every other one: No access
   - With less than the two writes, the bot cannot push a release branch or open a request. The failure shows up weeks later as a workflow that silently never ran.
   - The merge job needs Checks. `rk release merge` dispatches that job, and the job reads the required check run for the request's head before it merges. `rk setup check` asks an automatic release for it.
6. Organization permissions, Account permissions
   - Every one: No access
7. Subscribe to events
   - Nothing checked
8. Where can this GitHub App be installed?
   - Only on this account
9. Click Create GitHub App.

The App does not need the permissions below. Each one is a guess that costs a rerun:

- Issues: release-plz opens pull requests, never issues.
- Actions, Workflows: `Workflows: write` serves only a write of files under `.github/workflows/`, and the release request never touches them.
- Administration: release-plz asks for it only where a tag protection blocks tag creation. The convention's tag ruleset restricts `deletion` and `update` on `v*` and leaves creation open, so `Contents: write` carries the tag push. A target that adds a `creation` rule grants Administration here and widens the mint below to match.

The grant above is the ceiling. Each landed workflow mints a token narrower than it, per job, through the `permission-*` inputs of `actions/create-github-app-token`. Without one, the token inherits every permission that the installation holds. The half that opens the release request takes `permission-contents: write` and `permission-pull-requests: write`. The half that tags and publishes opens no request and takes `permission-contents: write` alone. The merge job that `rk release merge` dispatches adds `permission-checks: read` to the first pair, because it reads the required check before it merges. No other job arms or merges the request. release-plz scopes its own release workflow the same way.

### Collect the credentials

On the App's own settings page, which the registration lands on:

1. In the About section, copy the App ID.
   - Copy the numeric id, not the Client ID. The landed workflows pass it through the `client-id` input of `actions/create-github-app-token@v3`, which hands the value to GitHub as the JWT issuer. GitHub accepts the numeric App ID there. The secret keeps its name `RELEASE_BOT_APP_ID`, and a Client ID stored in it works too. `rk setup` reads the numeric App ID to sign its own token and refuses any other value. So this walkthrough copies the App ID.
2. In the Private keys section, click Generate a private key.
   - A `.pem` downloads at once. GitHub keeps only the public half and never shows the file again, so store it before you leave the page
3. Move the file outside every repository. Then run `chmod 600` on it, because `bot-secrets` refuses a group-readable key.
4. Give `rk setup` the pair. The App ID is a public identifier, so `setup.bot.app_id` in `.release-kit/config.toml` holds it once, and the export is unnecessary. `RK_BOT_APP_ID` still wins where it is set, for a run against another App. `RK_BOT_PRIVATE_KEY_FILE` holds the path to the `.pem`. Commit neither that variable nor the key itself. `install-bot` and `bot-secrets` read the pair, and nothing else identifies the App.

### Install it on this repository

1. In the left sidebar of the App's settings page, click Install App.
2. Click Install next to the account.
3. Choose Only select repositories. You can edit the selected set later, and `rk setup step install-bot` adds a project through the API.
4. Select the repository in the picker.
5. Click Install.
6. Open `github.com/settings/installations`.
   - check: it lists the App, with the repository under Repository access

From here, every remaining action is a command. `rk setup step install-bot --target . --apply` grants the App this project. `rk setup step bot-secrets --target . --apply` stores the credentials as repository secrets. Both values travel on standard input, so no environment that `rk` builds and no process argument list ever holds either. `rk` checks the path and the mode first. It checks the PEM encoding first too. It refuses anything that is not a PEM private key before it calls the forge, and then sends the same bytes that it checked. GitHub alone answers whether it accepts the encoded key.

`install-bot` observes and verifies as the App itself. GitHub serves the endpoints that read an installation to App credentials only. Such a credential is an RS256 JWT signed with the App's private key, and no personal access token of any class substitutes for it. So `rk` builds that token from the two exports and signs it in-process with the key bytes. It carries the token to the forge in a header of its own request. So no child process and no argument list holds the key or the token. `rk setup check` reports this step `unknown` until the step runs with the App exports, and no user token settles it. Run it on the host. The key lives there, and a keyring lookup needs a session bus that a container does not have.

### Grant a later repository

One caveat remains, on the write alone. GitHub documents that only a classic personal access token with `repo` scope can add a repository that an existing installation does not yet cover. GitHub mints that token through its web settings only, under Tokens (classic). By hand, the grant is one click per project. Open `github.com/settings/installations`. Click Configure next to the App. Add the repository under Only select repositories. By command, the steps below do three things:

- They mint the token.
- They run the grant with it beside the two App exports.
- They delete it.

1. Open `github.com/settings/tokens`.
   - the left sidebar lands on Personal access tokens, Tokens (classic)
2. Click Generate new token. Then click Generate new token (classic), the entry labelled For general use.
   - the other entry, Fine-grained, repo-scoped, cannot grant an installation
3. If GitHub asks, confirm your password or passkey. The creation of a token is a sudo-mode action.
4. Set Note to `<repo> install-bot grant`.
   - the field grants nothing. It is the label that the token list shows
5. Set Expiration to 7 days.
   - the token has one job, and you delete it after that job. GitHub revokes a token with No expiration only after a year unused
6. Under Select scopes, check `repo`, Full control of private repositories. Leave every other box unchecked.
   - it also checks five scopes that are not separable:
     - `repo:status`
     - `repo_deployment`
     - `public_repo`
     - `repo:invite`
     - `security_events`
   - no other scope is reachable from this step. The step reads one repository id, adds it to the installation, and lists the installation back
   - a token with no scope reaches public information only, and the grant fails
7. Click Generate token. Copy the value, because GitHub shows it this once.
8. If the owner is an organization that uses SAML single sign-on, click Configure SSO next to the token. Then click Authorize.
9. Store it in the OS keyring. The value then reaches `rk` as an environment assignment and never as a process argument, per `forge-setup:a-secret-never-reaches-argv`.

   ```bash
   secret-tool store --label='GitHub classic PAT (repo scope)' service github account gh-classic-pat
   # it prompts "Password:" for the value to store; paste the ghp_ token there
   secret-tool lookup service github account gh-classic-pat | cut -c1-4
   # check: prints ghp_, so a mistyped store surfaces here and not four commands later
   GH_TOKEN="$(secret-tool lookup service github account gh-classic-pat)" gh api user -q .login
   # check: prints the account's login; GH_TOKEN overrides the gh login, so a wrong value 401s here
   ```

10. Run the grant with the token beside the two App exports. `rk` reads the installation id as the App, and the grant uses the token.

    ```bash
    GH_TOKEN="$(secret-tool lookup service github account gh-classic-pat)" rk setup step install-bot --target . --apply
    # check: reports the installation id covering the repository
    ```

11. Delete the token at `github.com/settings/tokens`. It has no further job.

## Mapping

| Purpose                              | Command                                                                                                                                                               |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Raw API                              | `gh api`                                                                                                                                                              |
| Set the default branch               | `gh repo edit --default-branch`                                                                                                                                       |
| Delete a branch when its merge lands | `gh api -X PATCH /repos/{owner}/{repo}` with `delete_branch_on_merge`                                                                                                 |
| Store a secret                       | `gh secret set NAME` with the value on stdin                                                                                                                          |
| List open release requests           | `gh pr list --base <branch> --state open`                                                                                                                             |
| Merge the trunk's release request    | `rk release merge --target . --apply`                                                                                                                                 |
| Merge a release line's request       | `gh pr merge --squash --delete-branch`                                                                                                                                |
| Wait on checks                       | `gh pr checks --watch`                                                                                                                                                |
| Create the branch for an issue       | `gh issue develop <issue>`; `--checkout` seats it in the main checkout                                                                                                |
| List the branches an issue links     | `gh issue develop --list <issue>`                                                                                                                                     |
| Wait on a build                      | `gh run watch --exit-status`                                                                                                                                          |
| Tell a queued job from a stuck one   | `gh run view <id> --json jobs`, then `gh api repos/<repo>/actions/runners`; evidence only: a self-hosted job needs one online runner carrying every label it requests |
| Find the merged request for a commit | `gh api /repos/{owner}/{repo}/commits/{sha}/pulls`                                                                                                                    |
| Protect a branch                     | rulesets: `POST` or `PUT /repos/{owner}/{repo}/rulesets`                                                                                                              |
| Require the trunk's checks           | a ruleset rule naming a status-check context                                                                                                                          |
| Restrict the merge method            | a ruleset `pull_request` rule listing `allowed_merge_methods`                                                                                                         |
| Make the title the squash message    | `gh api -X PATCH /repos/{owner}/{repo}` with `squash_merge_commit_title=PR_TITLE`                                                                                     |
| Protect tags                         | a ruleset targeting `v*`                                                                                                                                              |
| Grant the bot access to a project    | `PUT /user/installations/{id}/repositories/{id}`                                                                                                                      |
| Find the bot identity                | `GET /repos/{owner}/{repo}/installation`, as the App                                                                                                                  |

## Limitations

- Rulesets on a private repository require a paid plan. On a free private repository, the protections cannot apply, and `rk setup check` reports each as unsatisfied and does not pretend otherwise.
- The installation endpoints of the App need App JWT authentication and refuse every user token. The one write that adds a repository takes a classic personal access token alone. No single credential both reads and writes an installation, which is why `install-bot` observes as the App and grants as the user.
- Pull requests offer no fast-forward merge. The trunk reaches linear history through squash instead: one pull request becomes one commit, which is the shape the method wants anyway.
