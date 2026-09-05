# Forge Setup Sources

External sources behind `SPEC-forge-setup.md`: what each forge's API actually offers, which setup actions are scriptable at all, and how an embedded script is executed safely. Each entry states what the source says and which rule or file it bears on.

Verified against the listed sources on 2026-08-28, re-checked on 2026-08-29, the GitHub App entries re-checked on 2026-08-31 and again on 2026-09-01 when the token-class findings below were also confirmed against a live account, and the merged-branch deletion and default-workflow-permissions entries verified on 2026-09-01, the auto-merge entries on 2026-09-03, and the protection-removal entries on 2026-09-05. A source marked corroborating was reported by a parallel review and not independently fetched. Forge APIs move; re-check an entry before trusting it to design something new.

## GitHub rulesets

Rulesets are managed at `/repos/{owner}/{repo}/rulesets`, with separate create and update endpoints, and require repository Administration write permission. A ruleset is a named collection of rules applying to branches or tags with pattern targeting, and its rule requirements include status checks, signed commits, and blocking force pushes.

The `gh ruleset` subcommands are `check`, `list`, and `view`, all read-only, so a ruleset write goes through `gh api` while a read may use the friendlier verbs.

- <https://docs.github.com/en/rest/repos/rules>
- <https://cli.github.com/manual/gh_ruleset>
- <https://github.blog/news-insights/product-news/github-repository-rules-are-now-generally-available/>
- <https://github.com/orgs/community/discussions/139808>

Bearing: `forge-setup:a-step-is-idempotent`. Separate create and update endpoints are why a step resolves the ruleset by name and then chooses between them, rather than blindly creating.

## Removing a protection from a retired branch

A ruleset is deleted with `DELETE /repos/{owner}/{repo}/rulesets/{ruleset_id}`, listed first by `GET /repos/{owner}/{repo}/rulesets`; a classic branch protection is read with `GET /repos/{owner}/{repo}/branches/{branch}/protection` and removed with `DELETE` on the same path, which returns 204 and removes every rule on that branch. On GitLab, `GET /projects/:id/protected_branches` lists the protected branches and `DELETE /projects/:id/protected_branches/:name` unprotects one, with `:id` the project's numeric id or its URL-encoded path. `glab api` replaces `:id` in the endpoint with the current directory's project, and `--method` or its `-X` shorthand overrides the request method.

- <https://docs.github.com/en/rest/repos/rules>
- <https://docs.github.com/en/rest/branches/branch-protection>
- <https://docs.gitlab.com/api/protected_branches/>
- <https://docs.gitlab.com/cli/api/>

Bearing: `runbooks/migration.md` steps 3a and 5a, the two gated removals a migration from the retired two-branch flow makes before the trunk can be fast-forwarded and before `single-trunk` can delete the integration branch. Both forms are read first and deleted second, because a repository may carry either a ruleset or a classic protection on the retired branch and the runbook must find whichever stands.

## GITHUB_TOKEN and recursive workflow triggering

Events triggered by the default `GITHUB_TOKEN` do not start new workflow runs.

- <https://docs.github.com/en/actions/concepts/security/github_token>

Bearing: this is the whole reason a bot identity exists. A tag pushed under the default token would never retrigger the artifact workflow, which is what `method/01-invariants.md` states as an invariant rather than a preference.

## Auto-merge on a pull request

GitHub gates the feature on the repository setting `allow_auto_merge`, readable and writable on `GET`/`PATCH /repos/{owner}/{repo}`; auto-merge is offered only on a request that cannot merge immediately — a branch protection with at least one unmet requirement — and is disabled when someone without write permission pushes to the head branch or the base branch changes. GitLab carries no project-level switch: auto-merge is generally available since 17.7, and its availability follows from pipelines running and the merge checks the project requires, of which `only_allow_merge_if_pipeline_succeeds` is the one the setup asserts.

- <https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/automatically-merging-a-pull-request>
- <https://docs.github.com/en/rest/repos/repos>
- <https://docs.gitlab.com/user/project/merge_requests/auto_merge/>

Bearing: `forge-setup:the-setup-permits-a-request-to-merge-itself` and the `auto-merge` step. The missing GitLab switch is why that forge's observation reports a limitation rather than a pass, and the unmet-requirement precondition is why the step needs no ordering against `protect-trunk`: a repository whose checks are all green simply merges immediately.

## Default workflow permissions for GITHUB_TOKEN

"By default, when you create a new repository in your personal account, `GITHUB_TOKEN` only has read access for the `contents` and `packages` scopes." The repository setting that changes it is documented as "Under 'Workflow permissions', choose whether you want the `GITHUB_TOKEN` to have read and write access for all permissions (the permissive setting), or just read access for the `contents` and `packages` permissions (the restricted setting)", beside the "Allow GitHub Actions to create and approve pull requests" setting, which "configure[s] whether `GITHUB_TOKEN` can create and approve pull requests".

`GET` and `PUT /repos/{owner}/{repo}/actions/permissions/workflow` carry both. `default_workflow_permissions` is a string of `read` or `write`, "The default workflow permissions granted to the GITHUB_TOKEN when running workflows", and `can_approve_pull_request_reviews` is a boolean, "Whether GitHub Actions can approve pull requests. Enabling this can be a security risk".

A workflow that declares the `permissions` key does not inherit that default. The permissions "are initially set to the default setting for the enterprise, organization, or repository", and "If you specify the access for any of these permissions, all of those that are not specified are set to `none`".

- <https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository>
- <https://docs.github.com/en/rest/actions/permissions>
- <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>

Bearing: `forge-setup:every-supported-forge-runs-every-step` and the `ci-permissions` step. Unnamed scopes falling to `none` is why every landed release workflow, each declaring `permissions:` per job, is untouched by this setting, and why the setup guide names `ci.yml` as the one workflow the default reaches.

## GitHub App registration and installation

The App manifest flow, `POST /app-manifests/{code}/conversions`, is the only programmatic registration path, and it works by redirecting a person to the forge and exchanging a temporary code afterwards. It returns the app id, the private key, and the webhook secret, and it still requires a browser. There is no API to create an App without that redirect.

Adding a repository to an existing installation is an API call: `PUT /user/installations/{installation_id}/repositories/{repository_id}`, documented as working only for classic personal access tokens carrying `repo` scope, and requiring admin on the repository. `DELETE` on the same path removes it, returning 422 where removal would leave the installation with none.

Reading an installation is a different credential class entirely. `GET /user/installations` is listed among the endpoints available to GitHub App user access tokens — the `ghu_` tokens only the App's own user-authorization flow mints — and a classic token with exactly `repo` scope gets 403 there, "You must authenticate with an access token authorized to a GitHub App", verified against a live account on 2026-09-01. The endpoints that do answer for the App are JWT-only: `GET /repos/{owner}/{repo}/installation` names the installation covering one repository, 404 when it does not, and `GET /users/{username}/installation` and `GET /orgs/{org}/installation` name the account-level installation directly, so discovering the grant's id needs no listing and no pagination. The JWT is RS256, signed with the App's private key, `iss` the App or client id, `iat` backdated sixty seconds against clock drift, `exp` at most ten minutes out, and it must travel as `Authorization: Bearer` — the `token` scheme is refused for JWTs. The gh CLI sends the `token` scheme and offers no Bearer form, so a JWT cannot ride `gh api`; GitHub's own documentation signs with the OpenSSL CLI in its shell example.

The classic token that grant requires is itself created in the account's web settings and nowhere else: token creation over the API ended with the OAuth Authorizations API, whose removal directs integrations to the web application flow instead. The `repo` scope it carries grants full read and write access to public and private repositories, including code, commit statuses, invitations, collaborators, deployment statuses, and webhooks. `GH_TOKEN` and `GITHUB_TOKEN`, in that order, take precedence over the credentials `gh auth login` stored, so a wrong value in the environment replaces a working login rather than falling back to it.

- <https://docs.github.com/en/apps/sharing-github-apps/registering-a-github-app-from-a-manifest>
- <https://docs.github.com/en/rest/apps/apps>
- <https://docs.github.com/en/rest/apps/installations>
- <https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-json-web-token-jwt-for-a-github-app>
- <https://docs.github.com/en/rest/authentication/authenticating-to-the-rest-api>
- <https://github.com/cli/cli/issues/12828>
- <https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens>
- <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps>
- <https://github.blog/changelog/2020-11-13-token-authentication-required-for-api-operations/>
- <https://cli.github.com/manual/gh_help_environment>
- <https://github.com/github/rest-api-description>

Bearing: creating the bot identity is manual once per account, ever; reading its installation is the App's own privilege; granting it a project is the one write left to a user credential. This split is why `install-bot` observes and verifies as the App — an OpenSSL-signed JWT carried by `curl`, since gh cannot speak Bearer — and grants through the documented PUT, with the installation id discovered as the App and handed to the script. An earlier revision of this entry called the listing's token acceptance unverified; the live 403 settled it, and it is the failure every operator hit while the step observed through `GET /user/installations`. That the classic token is browser-only is why the setup guide spends a sub-recipe on minting it, and the environment precedence is why that recipe proves the stored value before spending a forge call on it.

## GitLab protected branches, protected tags, and project access tokens

Protected branches expose `GET`, `POST`, `PATCH /projects/:id/protected_branches/:name`, and `DELETE`. Protected tags expose `GET`, `POST`, and `DELETE`, with no `PATCH` or `PUT`.

`POST /projects/:id/access_tokens` creates a project access token and its bot user in one call. Bot users are service accounts and consume no licensed seat. The maximum role settable depends on the caller's own role.

- <https://docs.gitlab.com/api/protected_branches/>
- <https://docs.gitlab.com/api/protected_tags/>
- <https://docs.gitlab.com/api/project_access_tokens/>
- <https://docs.gitlab.com/user/project/settings/project_access_tokens/>
- <https://docs.gitlab.com/security/tokens/>

Bearing: a protected branch updates in place, so its step is atomic on rerun; a protected tag has no update endpoint, so its step is delete-then-create and briefly not atomic, which `forges/gitlab.md` states rather than smooths over. The single-call bootstrap is the asymmetry with GitHub worth stating in both forge documents: the GitLab bot is fully scriptable per project, and the GitHub one is not.

## Deleting the merged branch

`PATCH /repos/{owner}/{repo}` takes `delete_branch_on_merge`, a boolean defaulting to false, documented as "Either true to allow automatically deleting head branches when pull requests are merged, or false to prevent automatic deletion". The same field is read back on `GET /repos/{owner}/{repo}`, so one endpoint serves the write and the observation.

GitLab spells the same setting `remove_source_branch_after_merge` on `PUT /projects/:id`, documented as "Whether the source branch is automatically removed after merge", and reads it back on `GET /projects/:id`.

- <https://docs.github.com/en/rest/repos/repos>
- <https://docs.gitlab.com/api/projects/>

Bearing: `forge-setup:every-supported-forge-runs-every-step` and the `merge-cleanup` step. Both forges settle this in project configuration rather than per merge, so the step is one write and one read on each, and neither depends on the merge verb the operator happens to use.

## Registry trusted publishing

A crate must be published manually once before a trusted publisher can be attached. The trusted publisher itself is configured in the registry web interface; no API endpoint for configuring it was found, so the OIDC token exchange during CI is the programmatic half and the configuration is not. Bootstrap token creation and the enforcement switch are likewise web-interface actions. Owners can enforce trusted publishing so that token publishing is disabled entirely. GitLab CI/CD is supported for the hosted instance only; a self-hosted instance cannot satisfy the OIDC invariant.

- <https://crates.io/docs/trusted-publishing>
- <https://rust-lang.github.io/rfcs/3691-trusted-publishing-cratesio.html>
- <https://blog.rust-lang.org/2026/01/21/crates-io-development-update>
- <https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07>
- <https://gitlab.com/gitlab-org/gitlab/-/work_items/572760>

Unverified: whether an undocumented or newly added API for trusted-publisher configuration exists. Searched and not found; treat as absent rather than proven absent.

Bearing: the ordering in `method/02-setup.md`, which publishes by hand before registering the publisher, and the self-hosted detection the setup reports at its first step rather than letting it surface when the registration fails.

## What is scriptable, and what is not

The classification the sources above produce, which is what `runbooks/setup.md` and `rk guide setup` route around.

| Action                                 | Scriptable                     | Frequency        |
| -------------------------------------- | ------------------------------ | ---------------- |
| Create the bot identity on GitHub      | no, browser redirect           | once per account |
| Add a project to a GitHub installation | yes, write needs a classic PAT | per project      |
| Create the bot identity on GitLab      | yes                            | per project      |
| Store credentials on the project       | yes                            | per project      |
| Create the bootstrap registry token    | no, web UI                     | once per package |
| First publish                          | yes                            | once per package |
| Register the trusted publisher         | no, web UI, no API found       | once per package |
| Revoke the bootstrap token             | no, web UI                     | once per package |
| Enable publishing enforcement          | no, web UI, no API found       | once per package |

## Secrets on standard input

`gh secret set` documents `-b/--body` as the value, reading from standard input if the flag is not specified; the documented examples use redirection with the flag omitted. There is no `-` sentinel, and `--body -` sets the literal value `-`. `glab variable set` likewise accepts the value from standard input, alongside `-v/--value`, `-m/--masked`, and `-p/--protected`.

Feeding a secret from a file is the documented form: `gh secret set --help` lists `gh secret set MYSECRET < myfile.txt` among its examples, and `glab variable set --help` lists both `glab variable set FROM_FILE < secret.txt` and `cat file.txt | glab variable set SERVER_TOKEN`. Neither offers a flag naming a file to read a secret from, so the value arrives on standard input whether a redirect, a pipe, or a writing parent supplies it. `rk` supplies it, because it has already read the file to validate it.

- <https://cli.github.com/manual/gh_secret_set>
- <https://docs.gitlab.com/cli/variable/set/>

Bearing: `forge-setup:a-secret-never-reaches-argv` and `forge-setup:key-material-never-reaches-the-environment`. The sentinel that does not exist is worth recording, because it is the transport an unchecked design would have used; so is the file-naming flag that does not exist, because its absence is why standard input is the only way in and why `rk` writes it rather than naming a path to something else.

## Reconcile inside the step, not a controller around it

A policy-as-code app for repository settings stores the desired state, compares it with the forge, calls the API only for real differences, and supports dry runs. It then needs a webhook service and a scheduled full resync, because webhook delivery is not guaranteed and manual drift happens.

- <https://github.com/github/safe-settings>
- <https://developer.hashicorp.com/terraform/cli/commands/plan>

Bearing: the step lifecycle — observe, compare, apply only where different, verify — is that loop without the controller. The hierarchy, the webhook service, and the scheduled daemon are what continuous multi-repository governance costs, and a one-time fixed-policy setup needs none of it. Terraform's partial-apply story, where there is no rollback and a rerun creates only what is missing, is the recovery model the setup runs on.

## Executing an embedded script

The kernel enforces `noexec` when a file is executed as an image; an interpreter handed a path only reads it, which is why at least one hardened operating system patches its shells to refuse scripts on such mounts.

Writing to a child's standard input without concurrently draining its piped output can deadlock once pipe buffers fill. A spawned command inherits the full parent environment unless it is cleared.

- <https://chromium.googlesource.com/chromiumos/docs/+/master/security/noexec_shell_scripts.md>
- <https://doc.rust-lang.org/std/process/struct.Command.html>
- <https://doc.rust-lang.org/std/process/struct.Stdio.html>

Breakage precedents on hardened hosts, where a mounted-noexec temporary directory stops a tool that materializes and runs a script: <https://github.com/nrwl/nx/issues/35570>, <https://gitlab.com/gitlab-org/gitlab-pages/-/issues/134>.

Bearing: `sh <path>` is the portable invocation form, with the caveat carried honestly — it is the standard behaviour rather than a guarantee, and a host that patches its shells is choosing to refuse. The concurrent drain and `env_clear` in the process adapter are the two failure modes the standard library documents rather than defects found by testing.

## Run records, and the bound they need

A package manager writes a per-run debug log, keeps at most a configured number with the oldest deleted first, and documents the benefit: run a command twice with different configuration and diff the logs. The complaint history asking to turn the files off is the other half of the lesson. A run framework stores each invocation under a run identifier with its output, return code, status, and ordered structured events.

Rebase earns `--continue`, `--skip`, and `--abort` because it holds an unfinished local transformation whose state cannot be reconstructed by examining the target.

- <https://docs.npmjs.com/cli/v11/using-npm/logging/>
- <https://github.com/npm/cli/issues/4206>
- <https://docs.ansible.com/projects/runner/en/2.2.1/intro/>
- <https://git-scm.com/docs/git-rebase>

Corroborating: the run framework's shape was reported by a parallel review and not independently fetched.

Bearing: the journal behind `rk runs`, including its retention bound and `rk runs prune` — per-run records are a shipped pattern and unbounded ones are a shipped complaint. The rebase entry is the argument against a resume verb: the forge is authoritative and reachable, so a failed setup is reconstructed by asking the forge, and the journal explains the past without deciding the future.

## The negative finding

No mainstream developer tool was found that embeds shell scripts in its binary and materializes them to execute. Repeated searches surfaced mechanism crates, self-extracting-archive techniques, and threat-intelligence writing about embedded payloads, and no peer.

Bearing: the execution model stands on this project's own constraints and claims no precedent. It is also one more reason the materialized copy is plain data invoked through an interpreter rather than an executable dropped to disk.

## The squash title source

Verified 2026-09-01. GitHub's repository update endpoint, `PATCH /repos/{owner}/{repo}`, takes `squash_merge_commit_title` with the values `PR_TITLE` and `COMMIT_OR_PR_TITLE`, the latter documented as the pull request's title only when the request holds more than one commit; the REST page states no default. The setting shipped in the 2022-05-11 changelog entry for customizable squash merge commit messages. Observed single-commit behaviour with the setting unset: the merge dialog offers that commit's own subject as the squash title, so a lone `wip` commit can become the trunk's message.

GitLab's project attribute `squash_commit_template` defaults to `%{title}`, the merge request's title, per the commit-templates documentation; the projects API takes it on `PUT /projects/{id}`. `merge_method=ff` and `squash_option=always` are the settings `protect-trunk` already asserts.

- <https://docs.github.com/en/rest/repos/repos#update-a-repository>
- <https://github.blog/changelog/2022-05-11-default-to-pr-titles-for-squash-merge-commit-messages/>
- <https://docs.gitlab.com/ee/api/projects.html>
- <https://docs.gitlab.com/ee/user/project/merge_requests/commit_templates.html>

Bearing: `forge-setup:the-setup-asserts-the-squash-title-source`. The stated no-default plus the observed single-commit fallback is why the setting is asserted rather than assumed, and why the assertion sits in `protect-trunk` beside the squash-only merge rule it completes.

## The squash message source

Verified 2026-09-02. The same repository update endpoint takes `squash_merge_commit_message` with the values `PR_BODY`, `COMMIT_MESSAGES`, and `BLANK`; `PR_BODY` is documented as the default message being the pull request's body. GitLab is structurally different: `squash_commit_template` defaults to `%{title}`, so the merge request's description never reaches the trunk commit and no body assertion or gate is needed there.

- <https://docs.github.com/en/rest/repos/repos#update-a-repository>
- <https://docs.gitlab.com/user/project/merge_requests/commit_templates/>

Bearing: `forge-setup:the-setup-asserts-the-squash-body-source`. `PR_BODY` making the request's description the trunk commit's body is why the description passes through the content gates and why the observation faults any other value.

## The title check beside the merge gate

Verified 2026-09-01. A required status check on GitHub matches by context name — for an Actions job, the job's name — so a workflow job named `pr-title` can be required by the trunk ruleset beside the project's own check. GitLab names no check: `only_allow_merge_if_pipeline_succeeds` requires the whole pipeline, so a title job in the merge request pipeline is blocking without registration, and it also gives every merge request the pipeline that setting waits on.

- <https://docs.github.com/en/rest/repos/rules>
- <https://docs.gitlab.com/ee/api/projects.html>

Bearing: the pair of required contexts `protect-trunk` writes, and why the GitLab half registers nothing.
