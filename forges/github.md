# GitHub

How this forge answers the method's fifth axis. The CLI is `gh`, and `rk setup` runs the `setup/github/` tree against it.

## Answers

- The release request is a pull request against the trunk — the default branch, which the bot targets with no configuration. While only its own commits sit on the request's branch the bot refreshes it by force-push; once a human commit lands there, the next refresh closes the request and opens a fresh one, taking the commit with it, which is why a changelog correction is the last act before merging.
- The gate is the release request's own merge, enforced by the trunk's ruleset: no direct push, squash as the only merge method, and two required status checks matched by name — the CI job the project's own workflow reports, and the landed `pr-title` check. The squash message is the request's title because `protect-trunk` sets `squash_merge_commit_title=PR_TITLE`; unset, a one-commit request offers that commit's own subject, which is how a `wip` subject reaches the trunk.
- Protections are rulesets, one per target: the trunk, the `v*` tags, and — where a project keeps older lines — the `release/*` branches. A ruleset has separate create and update endpoints, so a rerun resolves the ruleset id and updates in place.
- The issue link is an explicit relation, not a name match: `gh issue develop <issue> --checkout` generates the branch from the issue — named `<issue>-<slug>` — and records the link, so the pull request from that branch attaches to the issue and merging it closes the issue. A branch named by hand links only through a closing keyword, `Closes #<issue>` in the pull request body.
- The bot identity is a GitHub App installed on the repository. Its token is what makes a tag push start workflows; a tag pushed with the default CI token starts nothing.

## Bootstrap

Creating the App is the one action no command performs, because the manifest flow needs a browser redirect. It happens once in an account's lifetime, not once per project: adding a project to an existing installation is `rk setup step install-bot`.

### Check whether the App exists

1. Open the account's App list.
   - Personal account: `github.com/settings/apps`
   - Organization: `github.com/organizations/<owner>/settings/apps`
2. Look for the bot App by name.
   - Listed: skip to installing it on the repository.
   - Not listed: register it first.

### Register it

Walk the form at `github.com/settings/apps/new` top to bottom; each item below is one of its sections.

1. Top of the form
   - GitHub App name: yours to choose, globally unique across GitHub, at most 34 characters
   - Description: optional, what the bot does
   - Homepage URL: any URL; the form requires one and the bot ignores it
2. Identifying and authorizing users
   - Callback URL: empty
   - Expire user authorization tokens: leave checked, the App mints no user tokens
   - Request user authorization (OAuth) during installation: unchecked
   - Enable Device Flow: unchecked
3. Post installation
   - Setup URL: empty
   - Redirect on update: unchecked
4. Webhook
   - Active: uncheck it
   - Webhook URL, Webhook secret, SSL verification: leave them, all three disable once Active is cleared
5. Repository permissions
   - Contents: Read and write
   - Pull requests: Read and write
   - Metadata: Read-only, set by GitHub and not removable
   - Every other one: No access
   - Anything less than the two writes and the bot cannot push a release branch or open a request; the failure shows up weeks later as a workflow that silently never ran.
6. Organization permissions, Account permissions
   - Every one: No access
7. Subscribe to events
   - Nothing checked
8. Where can this GitHub App be installed?
   - Only on this account
9. Click Create GitHub App.

What it does not need, against the guesses that cost a rerun:

- Issues: release-plz opens pull requests, never issues.
- Actions, Workflows: `Workflows: write` is needed only to write files under `.github/workflows/`, and the release request never touches them.
- Administration: release-plz asks for it only where a tag protection blocks tag creation. The convention's tag ruleset restricts `deletion` and `update` on `v*` and leaves creation open, so `Contents: write` carries the tag push.

### Collect the credentials

On the App's own settings page, which the registration lands on:

1. About section, copy the App ID.
   - the numeric id, not the Client ID: the landed `release-plz.yml` reads the pair through `actions/create-github-app-token@v3`, which prefers `client-id` and still accepts `app-id`, and this convention stays on the App ID
2. Private keys section, click Generate a private key.
   - a `.pem` downloads at once; GitHub keeps only the public half and never shows the file again, so store it before leaving the page
3. Move the file outside every repository, then `chmod 600` it; `bot-secrets` refuses a group-readable key.
4. Export the pair for `rk setup`: `RK_BOT_APP_ID` holds the App ID, `RK_BOT_PRIVATE_KEY_FILE` holds the path to the `.pem`, never its contents. `install-bot` and `bot-secrets` read the pair; nothing else identifies the App.

### Install it on this repository

1. Left sidebar of the App's settings page, click Install App.
2. Click Install next to the account.
3. Choose Only select repositories; the selected set can be edited later, and `rk setup step install-bot` adds a project through the API.
4. Select the repository in the picker.
5. Click Install.
6. Open `github.com/settings/installations`.
   - check: it lists the App, with the repository under Repository access

From here every remaining action is a command: `rk setup step install-bot --target . --apply` grants the App this project, and `rk setup step bot-secrets --target . --apply` stores the credentials as repository secrets. Both values travel on standard input, so no environment `rk` builds and no process argument list ever holds either. `rk` checks the path, the mode, and the PEM encoding first and refuses anything that is not a PEM private key before calling the forge, then sends the very bytes it checked. Whether the encoded key is one GitHub accepts stays GitHub's answer.

`install-bot` observes and verifies as the App itself: GitHub serves the installation-reading endpoints to App credentials only — an RS256 JWT signed with the App's private key, which no personal access token of any class substitutes for — so `rk` builds that token from the two exports, has the OpenSSL CLI sign it with the key bytes on standard input, and carries it to the forge through `curl` in a header read from standard input, never an argument list. `rk setup check` reports this step `unknown` until it runs with the App exports; no user token settles it. Run it on the host: the key lives there, and a keyring lookup needs a session bus a container does not have.

### Grant a later repository

One caveat remains, on the write alone: adding a repository an existing installation does not yet cover is documented to work only for a classic personal access token with `repo` scope, which GitHub mints through its web settings only, under Tokens (classic). The by-hand form is one click, per project: open `github.com/settings/installations`, Configure next to the App, and add the repository under Only select repositories. By command, mint the token, run the grant with it beside the two App exports, and delete it.

1. Open `github.com/settings/tokens`.
   - the left sidebar lands on Personal access tokens, Tokens (classic)
2. Click Generate new token, then Generate new token (classic), the entry labelled For general use.
   - the other entry, Fine-grained, repo-scoped, cannot grant an installation
3. Confirm your password or passkey if GitHub asks; creating a token is a sudo-mode action.
4. Note: `<repo> install-bot grant`.
   - the field grants nothing; it is the label the token list shows
5. Expiration: 7 days.
   - the token has one job and is deleted after it; No expiration is revoked only after a year unused
6. Select scopes: check `repo`, Full control of private repositories. Leave every other box unchecked.
   - it also checks `repo:status`, `repo_deployment`, `public_repo`, `repo:invite`, and `security_events`; the five are not separable
   - no other scope is reachable from this step: it reads one repository id, adds it to the installation, and lists the installation back
   - a token with no scope reaches public information only, and the grant fails
7. Click Generate token, then copy the value; it is shown this once.
8. Where the owner is an organization using SAML single sign-on, click Configure SSO next to the token, then Authorize.
9. Store it in the OS keyring, so the value reaches `rk` as an environment assignment and never a process argument, per `forge-setup:a-secret-never-reaches-argv`.

   ```bash
   secret-tool store --label='GitHub classic PAT (repo scope)' service github account gh-classic-pat
   # it prompts "Password:" for the value to store; paste the ghp_ token there
   secret-tool lookup service github account gh-classic-pat | cut -c1-4
   # check: prints ghp_, so a mistyped store surfaces here and not four commands later
   GH_TOKEN="$(secret-tool lookup service github account gh-classic-pat)" gh api user -q .login
   # check: prints the account's login; GH_TOKEN overrides the gh login, so a wrong value 401s here
   ```

10. Run the grant with the token beside the two App exports; `rk` reads the installation id as the App and the grant rides the token.

    ```bash
    GH_TOKEN="$(secret-tool lookup service github account gh-classic-pat)" rk setup step install-bot --target . --apply
    # check: reports the installation id covering the repository
    ```

11. Delete the token at `github.com/settings/tokens`; it has no further job.

## Mapping

| Purpose                              | Command                                                                           |
| ------------------------------------ | --------------------------------------------------------------------------------- |
| Raw API                              | `gh api`                                                                          |
| Set the default branch               | `gh repo edit --default-branch`                                                   |
| Delete a branch when its merge lands | `gh api -X PATCH /repos/{owner}/{repo}` with `delete_branch_on_merge`             |
| Store a secret                       | `gh secret set NAME` with the value on stdin                                      |
| List open release requests           | `gh pr list --base <branch> --state open`                                         |
| Merge the release request            | `gh pr merge --squash --delete-branch`                                            |
| Wait on checks                       | `gh pr checks --watch`                                                            |
| Create the branch for an issue       | `gh issue develop <issue> --checkout`                                             |
| List the branches an issue links     | `gh issue develop --list <issue>`                                                 |
| Wait on a build                      | `gh run watch --exit-status`                                                      |
| Find the merged request for a commit | `gh api /repos/{owner}/{repo}/commits/{sha}/pulls`                                |
| Protect a branch                     | rulesets: `POST` or `PUT /repos/{owner}/{repo}/rulesets`                          |
| Require the trunk's checks           | a ruleset rule naming a status-check context                                      |
| Restrict the merge method            | a ruleset `pull_request` rule listing `allowed_merge_methods`                     |
| Make the title the squash message    | `gh api -X PATCH /repos/{owner}/{repo}` with `squash_merge_commit_title=PR_TITLE` |
| Protect tags                         | a ruleset targeting `v*`                                                          |
| Grant the bot access to a project    | `PUT /user/installations/{id}/repositories/{id}`                                  |
| Find the bot identity                | `GET /repos/{owner}/{repo}/installation`, as the App                              |

## Limitations

- Rulesets on a private repository require a paid plan; on a free private repository the protections cannot be applied and `rk setup check` reports each as unsatisfied rather than pretending.
- The App's installation endpoints need App JWT authentication and refuse every user token, and the one write that adds a repository takes a classic personal access token alone; no single credential both reads and writes an installation, which is why `install-bot` observes as the App and grants as the user.
- Pull requests offer no fast-forward merge. Linear trunk history is reached through squash instead: one pull request becomes one commit, which is the shape the method wants anyway.
