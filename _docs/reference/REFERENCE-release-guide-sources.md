# Release Guide Sources

External sources behind the registry walkthroughs in `bindings/rust.md` and the shipped runbooks: what the registry's token form carries, what its scopes and crate patterns mean, and where cargo keeps the credential the bootstrap publish uses. Each entry states what the source says and where it bears.

Verified against the listed sources on 2026-09-01, the version-derivation entry included; the auto-merge verbs verified on 2026-09-03.

## The Cargo book, on version numbers below 1.0.0

"This guide uses the terms \"major\" and \"minor\" assuming this relates to a \"1.0.0\" release or later. Initial development releases starting with \"0.y.z\" can treat changes in \"y\" as a major release, and \"z\" as a minor release. \"0.0.z\" releases are always major changes. This is because Cargo uses the convention that only changes in the left-most non-zero component are considered incompatible." The specification the convention rests on says of the same range: "Major version zero (0.y.z) is for initial development. Anything MAY change at any time."

- <https://doc.rust-lang.org/cargo/reference/semver.html>
- <https://semver.org/>

Bearing: the release runbook's step 1 and the overlay's semver note, the intent a squash title carries. Below 1.0 a breaking change moves the minor and a feature moves the patch, so a `feat` proposing a patch bump is the convention working rather than a misread commit.

## RFC 2947, on token scopes and crate patterns

The RFC defines the original endpoint-scope model: `publish-new` allows publishing new crates, `publish-update` allows new versions of existing crates, `yank` allows yanking and unyanking, `change-owners` allows inviting and removing owners, and `legacy` grants everything but token creation. The current form keeps the first four as checkboxes, adds `trusted-publishing`, and exposes no `legacy` box; the frontend source below is the authority on the current set. A crate scope restricts the token to name patterns — an exact name, or a prefix ending in `*` — and a pattern allows interacting with matching crates published after token creation, which is what lets the bootstrap token pin a name that does not exist yet.

- <https://rust-lang.github.io/rfcs/2947-crates-io-token-scopes.html>

Bearing: the bootstrap-token walkthrough in `bindings/rust.md`, the scope choice and the crate pattern. The pattern binding future crates is why the pattern is the crate's name rather than empty: an unrestricted token is wider than the one job it has.

## The crates.io token form, from its own source

The New API Token page (`crates.io/settings/tokens`, New Token) carries four fields. Name is required text. Expiration is a dropdown of No expiration, 7, 30, 60, 90, and 365 days, plus a custom date, defaulting to 90; 7 days is the shortest preset. Scopes are five checkboxes — `change-owners`, `publish-new`, `publish-update`, `trusted-publishing`, `yank` — of which at least one must be checked; `trusted-publishing` manages trusted-publishing configurations and is distinct from the crate-settings flow the binding's trusted-publisher walkthrough covers. Crates is a pattern list whose empty state reads Unrestricted; Add pattern appends an entry, and a pattern is `*`, or an identifier of ASCII alphanumerics, `_`, and `-` not opening with `_` or `-`, optionally ending in `*`. Generate Token returns to the token list, which shows the value once beside the explainer "Make sure to copy your API token now. You won't be able to see it again!", and a token row lists its scopes. The copy icon renders only where the browser exposes a clipboard, reports "Copied to clipboard!" on success and "Copy to clipboard failed!" on failure, and the shown value is selectable for a hand copy. A token row lists the name, Scopes, Crates, and an Expires distance.

- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/routes/settings/tokens/new/+page.svelte>
- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/lib/utils/token-scopes.ts>
- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/routes/settings/tokens/+page.svelte>
- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/lib/components/CopyButton.svelte>
- <https://blog.rust-lang.org/2023/06/23/improved-api-tokens-for-crates-io/>

Bearing: the bootstrap-token walkthrough in `bindings/rust.md`, every field the browser flow enumerates. The frontend source is the authority the deployed form is built from; the blog post introduced the scoped-token UI and corroborates the field set.

## The Cargo book, on the token's life around the publish

Publishing requires a crates.io account with a verified email. `cargo login` prompts for the token on standard input and stores it in `$CARGO_HOME/credentials.toml`; the book marks the token a secret to revoke immediately if it leaks, and `cargo logout` removes the stored copy.

- <https://doc.rust-lang.org/cargo/reference/publishing.html>

Bearing: the setup runbook's verified-email prerequisite, `cargo login` on stdin in the binding's bootstrap-token walkthrough, and the host-side half of its token revocation.

## The auto-merge verbs on the release request

`gh pr merge --auto` enables auto-merge so the request merges once its requirements are met, and `--disable-auto` disarms a pending one; `--match-head-commit <SHA>` merges only while the head still names that commit. `glab mr merge` sets auto-merge by default while a pipeline is running, with `--auto-merge=false` merging immediately and `--sha` pinning the reviewed head. The GitLab API merges with `PUT /projects/:id/merge_requests/:merge_request_iid/merge` carrying `merge_when_pipeline_succeeds=true` — the parameter name predates the feature's auto-merge label — and cancels a pending auto-merge with `POST /projects/:id/merge_requests/:merge_request_iid/cancel_merge_when_pipeline_succeeds`, which is the disarm on a forge whose CLI carries no cancel verb.

- <https://cli.github.com/manual/gh_pr_merge>
- <https://gitlab.com/gitlab-org/cli/-/blob/main/docs/source/mr/merge.md>
- <https://docs.gitlab.com/api/merge_requests/>

Bearing: the release runbook's hold substep and its armed merge step, and the arming steps the landed workflows carry. The cancel endpoint keeping the old name is why the runbook's GitLab disarm goes through `glab api` rather than a dedicated verb.

## The release create that makes the tag, on both forges

Checked on 2026-11-12.

- GitLab's `POST /projects/:id/releases`: where the tag named by `tag_name` does not exist, the release is created from `ref`, a commit SHA, a tag, or a branch, and tagged with `tag_name`; `ref` is required in that case. The page does not say what `ref` does for a tag that exists. `assets:links` is an array of hashes with `name` and `url`, sent as JSON. The release read answers a `commit` object with its `id`.
- GitLab's `Releases::CreateService` creates the tag before it saves the release and does not remove the tag when the save fails.
- `gh release create` with assets creates a draft release, uploads each asset, and then publishes it, in separate API requests that do not form one atomic operation. A tag that does not exist is created at `--target`, the default branch where none is given.

- <https://docs.gitlab.com/api/releases/>
- <https://github.com/gitlabhq/gitlabhq/blob/master/app/services/releases/create_service.rb>
- <https://cli.github.com/manual/gh_release_create>
- <https://cli.github.com/manual/gh_release_view>

Bearing: the `attach` job of `snippets/bash/gitlab/.gitlab-ci.yml` and the `attach` step of `snippets/bash/github/.github/workflows/release.yml` create the tag with the release, last. A tag at another commit stops each one, because GitLab does not document `ref` for an existing tag and the method never moves a protected tag. A GitLab release save that fails leaves the tag at the pipeline's own commit, and a rerun creates the release on it. A GitHub draft left by a failed upload is finished with `gh release upload --clobber` and `gh release edit --draft=false`.

## Queued or stuck, and the retried job

Checked on 2026-11-12 against GitLab's source; the retry behaviour was observed on a live retry on 2026-09-14.

- The GraphQL field `Project.jobs` takes `statuses: [CiJobStatus]`, and `CiJob.stuck` is a non-null Boolean, "Indicates the job is stuck". GitLab computes it as a pending job with no runner online, so a query for `PENDING` jobs reads it. The REST job fields carry no such field, and `DetailedStatus` has no `illustration` field, so `stuck` is the one to read.
- A GitHub job on self-hosted labels needs one online runner that carries every label it requests, and fails after 24 hours in the queue.
- A retried GitLab job uses the pipeline's original configuration, so a retry recovers a transient failure and never a defect a later commit fixed. A live retry's trace repeated the pre-fix failure.

- <https://docs.gitlab.com/api/graphql/reference/>
- <https://github.com/gitlabhq/gitlabhq/blob/master/app/graphql/resolvers/project_jobs_resolver.rb>
- <https://github.com/gitlabhq/gitlabhq/blob/master/app/graphql/types/ci/job_type.rb>
- <https://github.com/gitlabhq/gitlabhq/blob/master/app/models/ci/build.rb>
- <https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/use-in-a-workflow>
- <https://docs.github.com/en/actions/reference/runners/self-hosted-runners>
- <https://docs.gitlab.com/ci/jobs/>

Bearing: step 5 of `runbooks/release.md` and step 3a of `runbooks/setup.md` read `stuck` for a pending GitLab job and the runner labels for a queued GitHub job, and the recovery chapter's un-bump rests on the retry running its own commit's configuration.
