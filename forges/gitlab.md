# GitLab

How this forge answers the method's fifth axis. The CLI is `glab`, and `rk setup` runs the `setup/gitlab/` tree against it. The project path may nest deeper than two levels; every API call URL-encodes it.

## Answers

- The release request is a merge request against the trunk. Force-push refresh is unavailable here, so the bot keeps a request current by closing the open one and opening a fresh one when new work lands — and a correction on the closed request goes with it. The changelog window in the operate chapter is therefore narrower on this forge: correct and merge, with nothing landing in between.
- The gate is the release request's own merge, enforced by the project setting `only_allow_merge_if_pipeline_succeeds`. That is real enforcement — a Maintainer cannot merge past a failing pipeline — with a different shape: it is project-wide rather than branch-targeted, it requires the whole pipeline rather than a named job, and it blocks a merge when there is no pipeline at all. There is no check name to register and nothing for one to point at.
- Linear trunk history is the project setting `merge_method=ff` plus squash on every merge request: the forge that can fast-forward, does, so the trunk takes no merge commits.
- Protections are protected branches and protected tags. A protected branch updates in place through `PATCH /projects/:id/protected_branches/:name`; protected tags expose no update endpoint, so a change is delete-then-create and a rerun is briefly not atomic.
- The issue link is a name match: a branch named `<issue>-<slug>` — the shape the forge mints from an issue — cross-links, and the merge request opened from it carries `Closes #<issue>` by default. `glab mr create --related-issue <issue> --create-source-branch` creates the branch and that merge request in one move.
- The bot identity is a project access token: creating the token also creates its bot user, in one API call. A push authenticated with the default CI job token starts no pipeline, which is why the token exists at all.

## Bootstrap

Nothing here is manual. `rk setup step install-bot --target . --apply` is the whole bootstrap: `POST /projects/:id/access_tokens` creates the token and its bot user together, with the `api` and `write_repository` scopes at Maintainer access, expiring roughly a year out.

The create response is the only time the forge shows the token's value, so the same step immediately stores it as the masked CI variable `RELEASE_BOT_TOKEN` rather than printing it and asking an operator to copy it; the value never appears in any output or record.

Rotation is two commands: create a replacement token through the same step once the old one nears expiry, or export a value as `RK_BOT_TOKEN` and run `rk setup step bot-secrets --target . --apply` to overwrite the stored variable, the value travelling on standard input.

## Mapping

| Purpose                              | Command                                                                |
| ------------------------------------ | ---------------------------------------------------------------------- |
| Raw API                              | `glab api`                                                             |
| Set the default branch               | `glab api -X PUT projects/:id` with `default_branch`                   |
| Delete a branch when its merge lands | `glab api -X PUT projects/:id` with `remove_source_branch_after_merge` |
| Store a secret                       | `glab variable set NAME --masked` with the value on stdin              |
| List open release requests           | `glab mr list --target-branch <branch>`                                |
| Merge the release request            | `glab mr merge --squash --remove-source-branch`                        |
| Wait on checks                       | `glab ci status --wait`                                                |
| Wait on a build                      | `glab ci status --wait`                                                |
| Create the branch for an issue       | `glab mr create --related-issue <issue> --create-source-branch`        |
| Protect a branch                     | `POST /projects/:id/protected_branches`, `PATCH` to update             |
| Require the trunk's checks           | `PUT /projects/:id` with `only_allow_merge_if_pipeline_succeeds`       |
| Set the merge method                 | `PUT /projects/:id` with `merge_method=ff`                             |
| Protect tags                         | `POST /projects/:id/protected_tags`; no `PATCH`, so delete-then-create |
| Grant the bot access to a project    | `POST /projects/:id/access_tokens`                                     |
| Find the bot identity                | `GET /projects/:id/access_tokens`                                      |

## Limitations

- Tag immutability is weaker than the method asks. Protected tags stop git clients and non-privileged users, but a project Owner or Maintainer can still delete a protected tag through the UI or the API: protection against accident, not against authority. `rk setup check` reports that weaker guarantee by name rather than a pass, and the invariant survives as an invariant of the method, held by convention where the forge stops.
- The gate names no check. Per-check enforcement exists only as external status checks in the Ultimate tier, so `--required-check` is a usage error on this forge rather than a value silently discarded.
- Registry trusted publishing that supports this forge covers GitLab.com only, in public beta; a self-hosted instance cannot satisfy the OIDC invariant and falls back to a long-lived token, which is what the invariant exists to remove. `rk setup` reports this at the first step rather than letting it surface when the trusted publisher will not register.
- The `(rust, gitlab)` pair has no artifact builder, so its release page carries no installers; [the Rust binding](../bindings/rust.md) carries that fact, because it is a property of the pair.
- This tree is younger than the GitHub one. Every step is tested against a mocked CLI and every command below is taken from this forge's own documentation, but the tree has not yet run against a live project, and two behaviours in particular are read rather than observed: that a new push closes and replaces an open release request instead of force-pushing it, and that `only_allow_merge_if_pipeline_succeeds` blocks a Maintainer's merge rather than only a Developer's. Treat a first setup here as the proving ground it is, and read `rk setup check` rather than assuming the step did what it claims.
