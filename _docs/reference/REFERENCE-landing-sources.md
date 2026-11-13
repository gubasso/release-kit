# Landing Sources

This file lists the external sources behind `SPEC-landing.md`. They show how comparable tools record what they generated into a project. They also show how each tool judges whether a file is still its own, and what it does when it is not. The sources also show what the configuration language of a forge allows a landed file to leave to the target. Each entry states what the source says and which rule it bears on.

Verified against the listed sources on 2026-08-28 and re-checked on 2026-08-29. Later entries carry their own dates:

- The arming entries were verified on 2026-09-03.
- The release-marker entries were verified on 2026-09-05.
- The run-mode entry was verified on 2026-09-07.
- The GitLab composition entries were verified on 2026-09-08.
- The writer-version entries were verified on 2026-09-09.

## cargo-dist, on generated files that refuse to drift

`dist init` is designed for repeated reruns: it preserves settings and handles updates and migrations. `dist generate --check` errors if a generation would change the contents of the file, and it ignores newline style. Most commands run that check on startup, so an out-of-date or hand-edited workflow file is an error and not a surprise. `allow-dirty = ["ci"]` is the documented escape for someone who genuinely must hand-edit.

- <https://github.com/axodotdev/cargo-dist>
- <https://axodotdev.github.io/cargo-dist/>
- <https://github.com/axodotdev/cargo-dist/blob/main/CHANGELOG.md>

Bearing: `landing:a-rendered-file-is-reproducible` and `landing:ownership-is-elementary`. Their replacement of a recorded generated file takes the other half of the same stance. A tool that generates a file can say whether the file is still its own. Here the receipt says it, and Git keeps the previous bytes. The release-kit tool already teaches this exemplar to its own users in `bindings/rust.md`. So the same shape for its own landed files is consistent with what it asks of them. The general stance carries the weight: a tool that generates a file needs a way to say whether that file is still what it generated. The recorded digests serve that purpose.

## cargo-dist, on the run mode, and the projects that skip it

`pr-run-mode` selects what the generated GitHub workflow does on a pull request. `plan`, the default, runs the plan job, and the reference calls it recommended. `upload` also builds and uploads the artifacts. `skip` leaves the `pull_request` trigger out of the generated file entirely. GitHub Actions resolves the `needs` of a job inside one workflow file, and a required status check names a job and not a workflow. So a job in the generated file can be in the gate of no other file. The project astral-sh/uv sets `pr-run-mode = "skip"` and carries a hand-written `dist plan` job in its own gated CI graph. The project astral-sh/ruff carries a `cargo-publish-dry-run` job in its CI workflow for the same reason. The projects rust-lang/cargo and rust-lang/rust-analyzer both end their CI graph in an aggregate job named `conclusion`. That job reads `toJson(needs)` with jq and fails on any result other than success.

- <https://axodotdev.github.io/cargo-dist/book/reference/config.html>
- <https://docs.github.com/en/actions/reference/workflows-and-actions/contexts>
- <https://github.com/astral-sh/uv/blob/main/dist-workspace.toml>
- <https://github.com/astral-sh/uv/blob/main/.github/workflows/check-release.yml>
- <https://github.com/astral-sh/ruff/blob/main/.github/workflows/ci.yaml>
- <https://github.com/rust-lang/cargo/blob/master/.github/workflows/main.yml>

Bearing: `landing:a-seeded-file-still-carries-the-invariants`, for the `pr-run-mode-not-skip` and `workflow-runs-on-a-request` codes, and the jobs `bindings/rust.md` serves for the project's own gate.

## projen, on markers, anti-tamper, and the sweep

Generated files carry a magic marker, and projen cleans up any file that carries it once the file is no longer generated. The projen tool marks most generated files read-only, and an anti-tamper check runs in CI. The check confirms that a build did not modify them.

- <https://github.com/projen/projen>
- <https://projen.io/docs/introduction/the-projen-workflow/>

Bearing: the sweep-on-drop behavior that release-kit already implements for skills as `distribution:an-install-sweeps-what-the-payload-dropped`, which release-kit arrived at independently. It also bears on the argument that a landing record is what makes the same sweep possible for landed files.

release-kit does not copy the read-only-file approach. Landed files carry sentinels that the operator must fill, so they are explicitly not read-only. That is also why `landing:a-dropped-file-stays` makes the opposite choice from projen's, on purpose.

## copier and cruft, on the receipt living in the generated project

copier writes an answers file into the generated project. The file holds both the answers and the template version that produced the project. Its update regenerates from the current template with those answers, and diffs that against the actual project to extract local changes. It then applies migrations and re-applies the local modifications. Where both sides moved, it leaves conflict markers. cruft writes a record whose two load-bearing fields are the template and the commit.

- <https://copier.readthedocs.io/en/stable/updating/>
- <https://cruft.github.io/cruft/>

Bearing: `landing:a-landing-leaves-a-record` and `landing:a-record-states-its-schema`. This family records a unanimous shape: an opaque identity, a commit or a tag, never a per-file version.

copier also documents a warning: a hand edit of the answers file tricks the tool into believing that the wrong version generated the project. A record needs the same warning. Better, it needs a way to detect the tampering and not only forbid it. The per-file digests give `rk status --check` that way.

## pre-commit, on immutable pins and an explicit update verb

The revision field must point at a fixed tag. Mutable references are unsupported, and pre-commit never updates them after the first install. The autoupdate verb is the explicit action that rewrites the configuration to the latest released versions. It also converts a mutable revision to an immutable one. An author still updates the versions of additional dependencies by hand.

- <https://github.com/pre-commit/pre-commit/issues/1354>
- <https://github.com/pre-commit/pre-commit/issues/3521>
- <https://docs.renovatebot.com/modules/manager/pre-commit/>

Bearing: the discipline that `versions.toml` already applies. It also bears on the argument that a landing record holds an immutable identity plus an explicit update command, `rk upgrade`. The same argument says that the record holds no floating reference that quietly re-resolves.

## Terraform, on a committed lock file of versions and checksums

Project configuration records the exact selected versions and their checksums in a lock file. The project commits and reviews that file like any other change. The shared cache is an optimization, not the record.

- <https://developer.hashicorp.com/terraform/language/files/dependency-lock>
- <https://developer.hashicorp.com/terraform/cli/commands/providers/lock>

Bearing: the digest-bearing record. It also bears on the separation between a materialization cache, which is regenerable and authoritative for nothing, and a record that a reader must believe. It is also the precedent for a committed record, not a record kept as local state.

## Adoption, and why none of it is blind

Import brings pre-existing infrastructure under management: it records the infrastructure and does not recreate it. A provider option exists to take ownership of a resource that already exists. A Helm proposal covers the same ground through server-side apply. No member of this family adopts by overwriting what it finds.

- <https://helm.sh/community/hips/hip-0023/>
- <https://developer.hashicorp.com/terraform/cli/commands/plan>

Bearing: `landing:an-adoption-writes-the-record-and-nothing-else`. Adoption records both digests for a file that the target can edit. It refuses outright on a mismatch in a file that the projection owns, and it changes no target file. That is the strictest reading of the shared rule of this family, not a departure from it.

## pre-commit, on the stages and environment the landed hooks lean on

Verified 2026-09-01. A hook declares its `stages`, and the hooks of a stage run only where that hook type is installed. `pre-commit install --hook-type commit-msg --hook-type pre-push` installs them, and `default_install_hook_types` makes that the default for a repository. Pre-push hooks receive `PRE_COMMIT_REMOTE_BRANCH`, which carries the full remote ref of the push. So a local hook can refuse a push to `refs/heads/master` or a `refs/tags/v*` tag. Pushes that delete a ref skip the hooks on purpose (pre-commit issue 3050). Git tells a pre-push hook nothing about `--force`, so a force-push has no local mirror. `pre-commit/pre-commit-hooks` ships `no-commit-to-branch`, which protects `main` and `master` by default, with `--branch` and `--pattern` overrides. It reads the current branch and not a commit event. So a `pre-commit run` sweep over a checked-out protected branch fails the same way a commit does. The `SKIP` environment variable is a comma-separated list of hook ids. It is pre-commit's documented way to skip named hooks for one invocation, and it reports them as skipped and does not silently omit them. That is how a CI sweep keeps the guard out of a context that commits nothing.

Verified 2026-11-12 against prek v0.5.7, which reads the same `.pre-commit-config.yaml`. `prek install` takes the same `--hook-type` flags, and falls back to `default_install_hook_types` when none is given (`crates/prek/src/cli/install.rs`, `get_hook_types`). Before a run, prek sets these variables (`crates/prek/src/cli/run/run.rs`):

- `PRE_COMMIT_FROM_REF`
- `PRE_COMMIT_TO_REF`
- `PRE_COMMIT_LOCAL_BRANCH`
- `PRE_COMMIT_REMOTE_BRANCH`
- `PRE_COMMIT_REMOTE_NAME`
- `PRE_COMMIT_REMOTE_URL`

The base it passes as `PRE_COMMIT_FROM_REF` matches the base of pre-commit in the case that matters here. Where the clone lacks the remote tip, both pass the parent of the first pushed commit that no remote-tracking ref holds. That parent is always an ancestor of the pushed tip (prek `crates/prek/src/cli/hook_impl.rs`, `pre_push_run_options`, and pre-commit 4.5.1 `pre_commit/commands/hook_impl.py`, `_pre_push_ns`). So a guard that judges only `PRE_COMMIT_FROM_REF` passes a force push that drops a commit the clone never fetched, under either runner. The landed `rk-no-force-push-to-trunk` answers that case: it reads the remote trunk with `git ls-remote`. The pre-commit runner passes the remote tip itself where the clone holds it, even for a rewind. The prek runner does so only where that tip is an ancestor of the pushed one. Sometimes the clone lacks the advertised remote tip, and no pushed commit is missing from the remote-tracking refs. In that case, both runners run no check for that ref. Git still runs the installed shim. So a pure rewind over an unfetched commit reaches the remote unchecked, and only a forge rule that refuses a non-fast-forward push holds it. Reproduced 2026-11-12 with this setup:

- prek 0.5.7
- pre-commit 4.5.1
- a bare remote
- two clones

The prek runner honors `SKIP` ([environment variables](https://prek.j178.dev/reference/environment-variables/)). Its YAML reader refuses a duplicate key, so a second `default_install_hook_types` beside the target's own stops every hook.

- <https://github.com/j178/prek/blob/v0.5.7/crates/prek/src/cli/install.rs>
- <https://github.com/j178/prek/blob/v0.5.7/crates/prek/src/cli/run/run.rs>
- <https://github.com/j178/prek/blob/v0.5.7/crates/prek/src/cli/hook_impl.rs>
- <https://github.com/pre-commit/pre-commit/blob/v4.5.1/pre_commit/commands/hook_impl.py>

`compilerla/conventional-pre-commit` checks a commit message against Conventional Commits at the `commit-msg` stage. It takes these options:

- `--strict`
- `--force-scope`
- a comma-delimited `--scopes` list

`crate-ci/committed` offers `allowed_scopes` but no option to require a scope, and that difference decided between them.

Verified 2026-09-08, by a read of `conventional_pre_commit/format.py` in the hook repository that the landed block pins. The `r_scope` property builds its pattern two ways. Named scopes produce an alternation of exactly those words. No named scopes, with `--force-scope`, produce `(\([\w :,\-/.#]+\))`. A scope stays mandatory, and any token of these characters passes:

- word characters
- spaces
- the delimiters `: , - / . #`

The tool takes no scope pattern of its own, so the landed hook holds that a scope is present. Two other checks hold the shape beside it: the title check on the forge, and `rk message --check` at the desk.

- <https://pre-commit.com/#pre-commit-configyaml---top-level>
- <https://pre-commit.com/#pre-push>
- <https://pre-commit.com/#temporarily-disabling-hooks>
- <https://github.com/pre-commit/pre-commit/issues/3050>
- <https://github.com/pre-commit/pre-commit-hooks>
- <https://github.com/compilerla/conventional-pre-commit>
- <https://github.com/crate-ci/committed/blob/master/docs/reference.md>
- <https://git-scm.com/docs/githooks>

Bearing: `landing:a-landed-hook-serves-the-release-convention-alone`. Every mirror that the block carries rests on a documented mechanism above. The same sources state the two honest limits, `--no-verify` and the invisible force-push. `versions.toml` pins the two third-party hooks like every other snippet pin.

## git and the hook managers, on the integration gate

Verified 2026-11-12 with these versions:

- git 2.56.0
- prek 0.5.7
- pre-commit 4.5.1

`git commit-tree` runs no hook, and a fast-forward runs only `post-merge`. `git merge` runs `pre-merge-commit` only before it writes a merge commit. So no hook fires during a local integration on a linear trunk. `git hook run <hook-name>` runs the hooks that git finds for that event, `core.hooksPath` included, and returns their exit status. Without `--ignore-missing`, it fails with `error: cannot find a hook named <hook-name>`. Git exports the worktree's own `GIT_DIR` to the hook. From git 2.54, the configuration can also define a hook, and `git hook list <hook-name>` names every hook that git runs. Both pre-commit and prek install a `pre-merge-commit` hook with `--hook-type pre-merge-commit`. Outside a merge, both run the hooks tagged with that stage on the staged files. The prek runner runs every `.pre-commit-config.yaml` below its root as a project of its own. It skips a directory that a `.prekignore` lists, in Git ignore syntax.

- <https://git-scm.com/docs/githooks#_pre_merge_commit>
- <https://git-scm.com/docs/git-hook>
- <https://github.com/git/git/blob/master/Documentation/RelNotes/2.54.0.adoc>
- <https://pre-commit.com/#pre-merge-commit>
- <https://prek.j178.dev/reference/workspace/>

## The bot identity of the release request

Three upstream facts carry the request jobs that the landed release workflows render. GitHub states that with the exception of `workflow_dispatch` and `repository_dispatch`, events triggered by `GITHUB_TOKEN` do not create workflow runs at all. So a bump pushed or merged with the default token starts no publish. The release-plz action at the pinned commit declares a `pr` output. The output is the release request that the action opened or refreshed. It is a JSON object that carries these fields:

- `number`
- `head_branch`
- `base_branch`
- `html_url`

The release-plz tool refreshes by force-push on GitHub. On GitLab, it closes the outdated request and opens a fresh one. That is why rk 0.9.14 and older re-applied its arm on every run. The release-please action sets its outputs dynamically and not in `action.yml`: `prs_created` is true if any pull request was created or updated, and `pr` is a JSON string of the PullRequest object, unset when none exists.

- <https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows>
- <https://release-plz.dev/docs/github/output>
- <https://github.com/release-plz/action/blob/66b324fd6c72d05d2a08568492ecdd62bfb8a62b/action.yml>
- <https://github.com/googleapis/release-please-action/blob/45996ed1f6d02564a971a2fa1b5860e934307cf7/README.md>

Bearing: `release-engine:the-request-pushes-as-the-bot`, and the bot token that every landed request job takes. The default-token fact is why no request job runs under the default CI token of the forge.

## Gating the release request

The forge behavior that carried the release gate of rk 0.9.14 and older under local integration was verified on 2026-09-16. GitHub's recursion guard is the first fact. Events raised by a job that runs under `GITHUB_TOKEN` create no workflow run. GitHub documents `workflow_run` as the trigger that observes the finish of another workflow. That is why a check-run event cannot carry the gate and this one can. The `workflow_run` trigger requires a `workflows` key that names a workflow by its `name`. So the waking answer is a workflow name and never a filename. The check-runs endpoint for a commit takes `check_name` and `filter=latest`. That is how the gate reads one current result for the exact head it judged.

The release-driver branch forms were verified against their pinned sources and a live Python request on 2026-09-21. Release-plz opens `release-plz-<...>`, and the bash pipeline writes `chore/release-v<version>` itself. The pinned release-please branch module builds `release-please--branches--<targetBranch>`, with an optional `--components--<component>` or `--groups--<group>` suffix. The live Python request carries the root form against `main`, which confirms the answer of the branch module outside its source.

The forge renders one App actor three ways, and that decides which answer the gate can read. The pulls endpoint states `user.login` as the App's slug with a `[bot]` suffix, and `user.type` as `Bot`. The forge CLI's own pull-request list renders the same actor as `app/<slug>`. GraphQL returns the bare slug under a `Bot` typename. So a filter written against either of the last two refuses every release. The pulls endpoint paginates with at most 100 items per page. `gh api --paginate` applies the `--jq` filter to each page and concatenates the survivors. So the shell counts them across the complete paginated answer, and merges only where exactly one survives. The one folding alternative is closed: the forge CLI refuses `--slurp` beside `--jq`, so a workflow that pairs them reads no request at all.

- <https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows>
- <https://docs.github.com/en/rest/checks/runs>
- <https://github.com/googleapis/release-please/blob/v17.11.2/src/util/branch-name.ts>
- <https://github.com/googleapis/python-genai/pull/2830>
- <https://docs.github.com/en/rest/pulls/pulls>
- <https://cli.github.com/manual/gh_api>
- <https://cli.github.com/manual/gh_pr_merge>

Bearing: `git:the-release-request-integrates-at-the-forge`, and the release gate that every landed GitHub release workflow renders under local integration. The branch forms are a structural second guard alone. The gate authenticates a request by three facts, and it reads each one back from the forge:

- its author
- its base
- its head repository

So a wrong branch form refuses a release and admits none.

## The Nix capability's destinations

This entry covers the Nix-owned destinations that the projection names, verified against the Nix reference documentation on 2026-09-04. The flake file must be named `flake.nix` and live in the root directory of the repository. The flake description of the reference manual states that a flake is a filesystem tree whose root directory contains `flake.nix`. The `nix flake` reference documents how a `github:`/`git+https:` reference resolves to the flake file at the root of the tree. The lock machinery writes `flake.lock` beside it, in the same root, and Nix's own commands maintain it after the landing. That is why it lands as a `state` file. The `nix/` subdirectory for auxiliary expressions is a placement that release-kit chooses for its own seed, not a Nix requirement. The seed's `flake.nix` names the path explicitly, so a target can move it and adjust the call.

- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake>
- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake-lock>

Bearing: `landing:the-flake-pair-lands-all-or-nothing`, and `placement:a-third-party-destination-names-its-source` for `flake.nix` and `flake.lock`.

## GitLab, on why an include cannot isolate a target's jobs

Included configuration merges by a deep merge, at any depth: "Included files are read in the order defined in the configuration file, and the included configuration is merged together in the same order". The reference also says that "after all configuration added with `include` is merged together, the main configuration is merged with the included configuration". The main file wins a key that both files declare. So the included file contributes every key that the main file omits, whatever the include order. A file that the target owns as an include therefore adds any of these keys to a job that release-kit owns:

- `allow_failure`
- `needs`
- `retry`
- `tags`
- `artifacts`

It also adds these subkeys globally:

- `default`
- `variables`
- `workflow`

No include order prevents it.

- <https://docs.gitlab.com/ci/yaml/includes/>

Bearing: `landing:the-flake-pair-lands-all-or-nothing`, and the `project-jobs` bridge in both rendered GitLab parents. This fact rules out the simpler shape and leaves a child pipeline as the only shape that isolates.

## GitLab, on the child pipeline that carries the target's jobs

A downstream pipeline is a separate configuration. So these parts of a child never merge into the parent:

- the globals
- the `default` block
- the job names

`trigger:strategy` forces the trigger job to wait for the downstream pipeline before the job is marked success. The default marks the trigger job success as soon as the downstream pipeline is created. Of its two values, `mirror` "mirrors the status of the downstream pipeline exactly", and the reference introduced it in GitLab 18.2. The reference documents `depend` as "not recommended, use `mirror` instead", and its status can read running while the downstream waits on a manual job. Two documented shapes of a child job cannot gate. An optional manual job "does not affect the status of the downstream pipeline or the upstream trigger job". A failed job under `allow_failure: true` leaves the downstream pipeline successful. A rule that combines `if:` and `exists:` matches only when both match: "the rule evaluates to true only when all included keywords evaluate to true". `exists:` paths are relative to the project directory. Inside the child, `CI_PIPELINE_SOURCE` reads `parent_pipeline`.

- <https://docs.gitlab.com/ci/yaml/#triggerstrategy>
- <https://docs.gitlab.com/ci/yaml/#rulesexists>
- <https://docs.gitlab.com/ci/pipelines/downstream_pipelines/>
- <https://docs.gitlab.com/ci/jobs/job_rules/#complex-rules>
- <https://docs.gitlab.com/ci/variables/predefined_variables/>

Bearing:

- the `project-jobs` bridge
- the GitLab 18.2 version floor that `rk setup step forge-version` enforces
- the gating caveats that `forges/gitlab.md` states

The 18.2 date makes the floor a requirement and not a preference.

## GitLab, on the pipeline a merge request did not get

"If a pipeline contains only jobs in the `.pre` or `.post` stages, it does not run." On a merge-request event, the rules of the landed release jobs never match. So a title gate at `stage: .pre` was the only job that survived, and the forge created no pipeline at all. The project setting `only_allow_merge_if_pipeline_succeeds` then blocked every merge, for want of the pipeline it waits on. `.pre` and `.post` need no entry in `stages:`, and every other stage does. Arrays are not deep-merged, so a target cannot contribute a stage from a file of its own.

- <https://docs.gitlab.com/ci/yaml/#stage-pre>
- <https://docs.gitlab.com/ci/yaml/#stages>

Bearing: the `test` stage that both rendered GitLab parents declare last, and the `stage: test` that the landed `mr-title` job carries. Without that stage, the bridge inherits this defect, because the bridge sits in an ordinary stage too.

## The writer's version, and why no comparable tool prompts from it

Verified 2026-09-09. Across this family, a record carries up to three separable facts. The version of the executable that wrote the record is never the fact that answers "is there something to take". Terraform state is the only member that carries all three at once, and it separates them explicitly. `version` is the state format version and is 4 for every Terraform 1.x. `terraform_version` records the Terraform that wrote the snapshot. `serial` is a monotonic counter that each modification increments, and `lineage` is a UUID fixed at creation, because serials compare only within one lineage. Terraform refuses a state written by a newer Terraform, the same conservative direction as `landing:a-target-is-never-downgraded`. It prompts nothing merely because the running binary is newer than `terraform_version`. `Cargo.lock` carries a top-level `version` that is the lockfile format version alone, and cargo writes no cargo version into it. An older cargo hard-errors on a format it does not understand and does not warn. The answers file of copier records `_commit`, the template version. The record of cruft carries `template` and `commit`. Neither writes the version of the scaffolding executable at all. cargo-dist's `cargo-dist-version` is the apparent counterexample, and it is materially different. It is a pin that the generated CI fetches and runs, and a human edits it on purpose for an upgrade. `dist generate --check` catches drift separately, because it compares regenerated content. That is why a bump of the pin without a regeneration is the documented way to trip that check.

- <https://developer.hashicorp.com/terraform/language/state/backends>
- <https://developer.hashicorp.com/terraform/cli/commands/state/push>
- <https://github.com/hashicorp/terraform/pull/7109>
- <https://doc.rust-lang.org/stable/nightly-rustc/cargo/resolver/enum.ResolveVersion.html>
- <https://github.com/rust-lang/cargo/issues/10046>
- <https://copier.readthedocs.io/en/stable/configuring/>
- <https://cruft.github.io/cruft/>
- <https://axodotdev.github.io/cargo-dist/book/workspaces/simple-guide.html>

Bearing: `landing:status-judges-only-under-check`, for the pending count that the upgrade prompt reads, and for the recorded `rk_version`, which prompts nothing on its own. The family draws a distinction between a stamp and a pin. Each of these writes `rk_version` from `CARGO_PKG_VERSION`, so the field is a stamp:

- a landing
- an adoption
- an upgrade

A stamp of a release-automation tool necessarily lags its own crate version, because the version comes from the commit that already carries the record. `cargo-dist-version` never drifts that way only because nothing writes it automatically.

## release-plz, on what a release request can and cannot carry

Verified 2026-09-09. The configuration reference documents these tables, and no command hook of any kind:

- `[workspace]`
- `[[package]]`
- `[changelog]`

It has no facility to run a command while the release request is built, and so none to add a generated file to it. `allow_dirty` permits pre-existing changes during an update, which is not a generation step. A surrounding workflow can take four steps on the request:

- read the `pr` output of the action
- check out its head
- commit
- push

But release-plz refreshes that branch by force-push. In the landed workflow of this project, the same job also arms the request for auto-merge immediately afterwards.

- <https://release-plz.dev/docs/config>
- <https://release-plz.dev/docs/github/output>

Bearing: `landing:status-judges-only-under-check`, as the option not taken. A recorded version equal to the crate version needs custom orchestration that races the force-push and the arm. It holds a fact that answers no question the report asks.

## semantic-release and GoReleaser, on where their configuration lives

The semantic-release tool reads its configuration from any of these:

- `.releaserc`, with no extension
- the same name with one of these extensions:
  - `.yaml`
  - `.yml`
  - `.json`
  - `.js`
  - `.cjs`
  - `.mjs`
- one of these configuration files:
  - `release.config.js`
  - `release.config.cjs`
  - `release.config.mjs`
- a `release` key in `package.json`

GoReleaser looks for these files, in this order:

- `.config/goreleaser.yml`
- `.config/goreleaser.yaml`
- `.goreleaser.yml`
- `.goreleaser.yaml`
- `goreleaser.yml`
- `goreleaser.yaml`

- <https://semantic-release.gitbook.io/semantic-release/usage/configuration>
- <https://goreleaser.com/customization/>

Bearing: `landing:a-landing-classifies-its-target-first`. The marker catalog that `rk assess` reads, `RELEASE_MARKERS` in `src/assess.rs`, carries every documented name. `package.json` counts only with the `release` key, because an ordinary Node manifest is not a release mechanism.

## SPDX, on the licence expression the code scanning condition reads

Checked 2026-09-15. The SPDX 2.3 license-expression annex settles four things the reader depends on:

- Identifiers: "License identifiers (including license exception identifiers) used in SPDX documents or source code files should be matched in a _case-insensitive_ manner."
- Operators: "License expression operators (`AND`, `OR` and `WITH`) should be matched in a _case-sensitive_ manner."
- The shape of `WITH`: "A valid `<license-expression>` is where the left operand is a `<simple-expression>` value and the right operand is a `<license-exception-id>` that represents the special exception terms."
- Precedence: "The default operator order of precedence of a `<license-expression>` is: `+ WITH AND OR`".

A lower-order operator applies before a higher-order one. Parentheses are available "when required to express an order of precedence that is different from the default order".

Precedence carries weight and is not decorative, because different rules judge `AND` and `OR`. `CC-BY-4.0 OR CC-BY-SA-4.0 AND MIT` groups as `CC-BY-4.0 OR (CC-BY-SA-4.0 AND MIT)`, and offers its reader a term over prose alone. A reader that folds it flat from the left passes it. The unit test carries that expression as its witness.

Cargo defines `package.license` as an SPDX expression and does not hold it to that casing. A crate that declares `license = "mit"` packages without complaint. A run of `cargo package` over a scratch crate checked this. So a real crate can carry any casing of an identifier, and a case-sensitive comparison refuses a licence that the reader recognizes.

The reader carries the exception register whole and does not sample it: 86 identifiers from the SPDX license-list-data repository. A subset refuses a legitimate crate, because an exception older than the newest of its family is still valid. `GPL-2.0-only WITH GCC-exception-2.0` names a real exception that a list with only `GCC-exception-3.1` rejects.

- <https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/>
- <https://spdx.org/licenses/exceptions-index.html>
- <https://github.com/spdx/license-list-data/blob/main/json/exceptions.json>
- <https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields>

Bearing: the licence arm of `landing:the-nix-capability-is-a-recorded-opt-in`. The reader answers three questions in one pass, and all three must hold:

- The expression is well formed.
- Every reader of it obtains an OSI-approved grant.
- Every licence identifier in it is one that this release recognizes.

An exception grants permission and does not withdraw it, so it changes no verdict. The reader still validates it, because a reader that cannot parse the expression did not read the licence.

## CodeQL, on the codebase its terms cover

Checked 2026-09-15. The CodeQL CLI terms define the codebase they cover in one sentence. An Open Source Codebase is "a codebase that is released under an OSI-approved License." Under that definition, the terms permit a holder to "Perform analysis on the Open Source Codebase". Where GitHub.com hosts that codebase, they also permit a holder to generate databases for these uses:

- automated analysis
- CI
- CD

They prohibit two things for a holder without a paid GitHub Advanced Security seat. That holder cannot generate "any CodeQL database for or during automated analysis, CI or CD" over anything else. That holder also cannot use the software "in connection with any codebase that is not an Open Source Codebase (e.g., code in a private repo in GitHub)."

The definition asks what the codebase is released under, not what every identifier beside it names. A project that releases its source under an OSI-approved licence and its prose under Creative Commons terms hands every reader the OSI-approved grant. So it is released under an OSI-approved licence, and the terms cover it. A project that offers a reader the choice between an OSI-approved licence and a term over prose alone is not released under an OSI-approved licence. Its reader can hold only the second.

- <https://github.com/github/codeql-cli-binaries/blob/main/LICENSE.md>
- <https://docs.github.com/en/code-security/codeql-cli/getting-started-with-the-codeql-cli/about-the-codeql-cli>

Bearing: the licence arm of `landing:the-nix-capability-is-a-recorded-opt-in`. The judgment is `grants` in `src/projection.rs`: a conjunction grants where any operand does, and a disjunction grants only where every arm does. The Creative Commons identifiers that the reader recognizes beside the OSI-approved ones are the attribution and share-alike families. Those families license material that is not code, and no conjunction lets a reader escape them. `CC0-1.0` is absent on purpose, because it dedicates code as readily as prose and belongs to a question that this list does not answer.
