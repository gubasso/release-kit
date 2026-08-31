# Forge Setup Sources

External sources behind `SPEC-forge-setup.md`: what each forge's API actually offers, which setup actions are scriptable at all, and how an embedded script is executed safely. Each entry states what the source says and which rule or file it bears on.

Verified against the listed sources on 2026-08-28 and re-checked on 2026-08-29. A source marked corroborating was reported by a parallel review and not independently fetched. Forge APIs move; re-check an entry before trusting it to design something new.

## GitHub rulesets

Rulesets are managed at `/repos/{owner}/{repo}/rulesets`, with separate create and update endpoints, and require repository Administration write permission. A ruleset is a named collection of rules applying to branches or tags with pattern targeting, and its rule requirements include status checks, signed commits, and blocking force pushes.

The `gh ruleset` subcommands are `check`, `list`, and `view`, all read-only, so a ruleset write goes through `gh api` while a read may use the friendlier verbs.

- <https://docs.github.com/en/rest/repos/rules>
- <https://cli.github.com/manual/gh_ruleset>
- <https://github.blog/news-insights/product-news/github-repository-rules-are-now-generally-available/>
- <https://github.com/orgs/community/discussions/139808>

Bearing: `forge-setup:a-step-is-idempotent`. Separate create and update endpoints are why a step resolves the ruleset by name and then chooses between them, rather than blindly creating.

## GITHUB_TOKEN and recursive workflow triggering

Events triggered by the default `GITHUB_TOKEN` do not start new workflow runs.

- <https://docs.github.com/en/actions/concepts/security/github_token>

Bearing: this is the whole reason a bot identity exists. A tag pushed under the default token would never retrigger the artifact workflow, which is what `method/01-invariants.md` states as an invariant rather than a preference.

## GitHub App registration and installation

The App manifest flow, `POST /app-manifests/{code}/conversions`, is the only programmatic registration path, and it works by redirecting a person to the forge and exchanging a temporary code afterwards. It returns the app id, the private key, and the webhook secret, and it still requires a browser. There is no API to create an App without that redirect.

Adding a repository to an existing installation is an API call: `PUT /user/installations/{installation_id}/repositories/{repository_id}`, documented as working only for classic personal access tokens carrying `repo` scope, and requiring admin on the repository. `DELETE` on the same path removes it, returning 422 where removal would leave the installation with none. `GET /user/installations` lists the installations the authenticated user can reach, which is how a tool discovers the installation id.

- <https://docs.github.com/en/apps/sharing-github-apps/registering-a-github-app-from-a-manifest>
- <https://docs.github.com/en/rest/apps/apps>
- <https://docs.github.com/en/rest/apps/installations>
- <https://github.com/github/rest-api-description>

Bearing: creating the bot identity is manual once per account, ever; granting it a project is a command. The token-class restriction is stated on the grant and not on the listing, which is why `forges/github.md` and the install-bot step name the classic personal access token explicitly and why a 403 mentioning a GitHub App is reported as the wrong token class rather than as missing authentication. Whether the listing accepts a classic token is unverified — the description is silent, which is not the same as a refusal.

## GitLab protected branches, protected tags, and project access tokens

Protected branches expose `GET`, `POST`, `PATCH /projects/:id/protected_branches/:name`, and `DELETE`. Protected tags expose `GET`, `POST`, and `DELETE`, with no `PATCH` or `PUT`.

`POST /projects/:id/access_tokens` creates a project access token and its bot user in one call. Bot users are service accounts and consume no licensed seat. The maximum role settable depends on the caller's own role.

- <https://docs.gitlab.com/api/protected_branches/>
- <https://docs.gitlab.com/api/protected_tags/>
- <https://docs.gitlab.com/api/project_access_tokens/>
- <https://docs.gitlab.com/user/project/settings/project_access_tokens/>
- <https://docs.gitlab.com/security/tokens/>

Bearing: a protected branch updates in place, so its step is atomic on rerun; a protected tag has no update endpoint, so its step is delete-then-create and briefly not atomic, which `forges/gitlab.md` states rather than smooths over. The single-call bootstrap is the asymmetry with GitHub worth stating in both forge documents: the GitLab bot is fully scriptable per project, and the GitHub one is not.

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

| Action                                 | Scriptable               | Frequency        |
| -------------------------------------- | ------------------------ | ---------------- |
| Create the bot identity on GitHub      | no, browser redirect     | once per account |
| Add a project to a GitHub installation | yes                      | per project      |
| Create the bot identity on GitLab      | yes                      | per project      |
| Store credentials on the project       | yes                      | per project      |
| Create the bootstrap registry token    | no, web UI               | once per package |
| First publish                          | yes                      | once per package |
| Register the trusted publisher         | no, web UI, no API found | once per package |
| Revoke the bootstrap token             | no, web UI               | once per package |
| Enable publishing enforcement          | no, web UI, no API found | once per package |

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
