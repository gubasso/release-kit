# GitHub

How this forge answers the method's fifth axis. The CLI is `gh`, and `rk setup` runs the `setup/github/` tree against it.

## Answers

- The release request is a pull request against the trunk — the default branch, which the bot targets with no configuration. While only its own commits sit on the request's branch the bot refreshes it by force-push; once a human commit lands there, the next refresh closes the request and opens a fresh one, taking the commit with it, which is why a changelog correction is the last act before merging.
- The gate is the release request's own merge, enforced by the trunk's ruleset: no direct push, squash as the only merge method, and a required status check matched by name against the CI job the project's own workflow reports.
- Protections are rulesets, one per target: the trunk, the `v*` tags, and — where a project keeps older lines — the `release/*` branches. A ruleset has separate create and update endpoints, so a rerun resolves the ruleset id and updates in place.
- The issue link is an explicit relation, not a name match: `gh issue develop <issue> --checkout` generates the branch from the issue — named `<issue>-<slug>` — and records the link, so the pull request from that branch attaches to the issue and merging it closes the issue. A branch named by hand links only through a closing keyword, `Closes #<issue>` in the pull request body.
- The bot identity is a GitHub App installed on the repository. Its token is what makes a tag push start workflows; a tag pushed with the default CI token starts nothing.

## Bootstrap

Creating the App is the one action no command performs, because the manifest flow needs a browser redirect. It happens once in an account's lifetime, not once per project: adding a project to an existing installation is `rk setup step install-bot`.

1. Open the account's developer settings, GitHub Apps, and check whether the bot App already exists. If it does, skip to step 5.
2. Create a new GitHub App. The name is yours to choose; the homepage URL is unused; clear the webhook's Active checkbox.
3. Set the repository permissions: Contents read and write, Pull requests read and write, Metadata read-only. Anything less and the bot cannot push a release branch or open a request; the failure shows up weeks later as a workflow that silently never ran.
4. Create the App, then install it on the account. Choose "Only select repositories" unless every repository should carry it; the selected set can be edited later, and `rk setup step install-bot` adds a project through the API.
5. Collect the credentials from the App's settings page: the App ID from the About section, and a generated private key. The `.pem` downloads exactly once and is never shown again; store it before leaving the page, outside any repository, and `chmod 600` it.
6. Export the credentials for `rk setup`: `RK_BOT_APP_ID` holds the App ID, `RK_BOT_PRIVATE_KEY_FILE` holds the path to the `.pem`, never its contents. Where the account has more than one App installation, `RK_BOT_INSTALLATION` names the installation id `install-bot` should use.

From here every remaining action is a command: `rk setup step install-bot --target . --apply` grants the App this project, and `rk setup step bot-secrets --target . --apply` stores the credentials as repository secrets. Both values travel on standard input, so no environment `rk` builds and no process argument list ever holds either. `rk` checks the path, the mode, and the PEM encoding first and refuses anything that is not a PEM private key before calling the forge, then sends the very bytes it checked. Whether the encoded key is one GitHub accepts stays GitHub's answer.

One caveat holds for `install-bot`: the grant endpoint is documented to work only for a classic personal access token with `repo` scope, and the listing it observes through refuses the OAuth token `gh auth login` normally mints. Run that one step from a host shell with the classic token stored in the OS keyring and read back per run, `GH_TOKEN="$(secret-tool lookup service github account gh-classic-pat)" rk setup step install-bot --target . --apply`, so the value reaches the environment and never a process argument — or grant the project by hand at the installation's settings page, one click, per project, under `github.com/settings/installations`. The check reads the same listing, so `rk setup check` reports this step `unknown` rather than satisfied until it too runs with such a token.

## Mapping

| Purpose                           | Command                                                       |
| --------------------------------- | ------------------------------------------------------------- |
| Raw API                           | `gh api`                                                      |
| Set the default branch            | `gh repo edit --default-branch`                               |
| Store a secret                    | `gh secret set NAME` with the value on stdin                  |
| List open release requests        | `gh pr list --base <branch> --state open`                     |
| Merge the release request         | `gh pr merge --squash --delete-branch`                        |
| Wait on checks                    | `gh pr checks --watch`                                        |
| Create the branch for an issue    | `gh issue develop <issue> --checkout`                         |
| List the branches an issue links  | `gh issue develop --list <issue>`                             |
| Wait on a build                   | `gh run watch --exit-status`                                  |
| Protect a branch                  | rulesets: `POST` or `PUT /repos/{owner}/{repo}/rulesets`      |
| Require the trunk's checks        | a ruleset rule naming a status-check context                  |
| Restrict the merge method         | a ruleset `pull_request` rule listing `allowed_merge_methods` |
| Protect tags                      | a ruleset targeting `v*`                                      |
| Grant the bot access to a project | `PUT /user/installations/{id}/repositories/{id}`              |
| Find the bot identity             | `GET /user/installations`                                     |

## Limitations

- Rulesets on a private repository require a paid plan; on a free private repository the protections cannot be applied and `rk setup check` reports each as unsatisfied rather than pretending.
- The App's own REST endpoints need App JWT authentication, so a user token cannot query an App directly; `install-bot` and its check work through `GET /user/installations` instead, which sees only installations the authenticated user can reach.
- Pull requests offer no fast-forward merge. Linear trunk history is reached through squash instead: one pull request becomes one commit, which is the shape the method wants anyway.
