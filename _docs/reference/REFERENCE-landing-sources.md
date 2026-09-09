# Landing Sources

External sources behind `SPEC-landing.md`: how comparable tools record what they generated into a project, how they judge whether it is still theirs, what each one does when it is not, and what a forge's own configuration language allows a landed file to leave to the target. Each entry states what the source says and which rule it bears on.

Verified against the listed sources on 2026-08-28 and re-checked on 2026-08-29; the arming entries verified on 2026-09-03; the release-marker entries verified on 2026-09-05; the run-mode entry verified on 2026-09-07; the GitLab composition entries verified on 2026-09-08; the writer-version entries verified on 2026-09-09.

## cargo-dist, on generated files that refuse to drift

`dist init` is designed to be rerun repeatedly, preserving settings while handling updates and migrations. `dist generate --check` errors if generating would change the file's contents, ignoring newline style, and most commands run that check on startup, so an out-of-date or hand-edited workflow file is an error rather than a surprise. `allow-dirty = ["ci"]` is the documented escape for someone who genuinely must hand-edit.

- <https://github.com/axodotdev/cargo-dist>
- <https://axodotdev.github.io/cargo-dist/>
- <https://github.com/axodotdev/cargo-dist/blob/main/CHANGELOG.md>

Bearing: `landing:a-rendered-file-is-reproducible` and `landing:an-upgrade-refuses-on-owned-drift`. This is also the exemplar release-kit already teaches its own users about in `bindings/rust.md`, so adopting the same shape for its own landed files is consistent with what it asks of them. The general stance is the load-bearing part: a tool that generates a file should be able to say whether that file is still what it generated, which is what the recorded digests are for.

## cargo-dist, on the run mode, and the projects that skip it

`pr-run-mode` selects what the generated GitHub workflow does on a pull request: `plan`, the default, runs the plan job and the reference calls it recommended; `upload` also builds and uploads the artifacts; `skip` leaves the `pull_request` trigger out of the generated file entirely. GitHub Actions resolves a job's `needs` inside one workflow file, and a required status check names a job rather than a workflow, so a job in the generated file can be in no other file's gate. astral-sh/uv sets `pr-run-mode = "skip"` and carries a hand-written `dist plan` job in its own gated CI graph; astral-sh/ruff carries a `cargo-publish-dry-run` job in its CI workflow for the same reason. rust-lang/cargo and rust-lang/rust-analyzer both end their CI graph in an aggregate job named `conclusion`, which reads `toJson(needs)` with jq and fails on any result other than success.

- <https://axodotdev.github.io/cargo-dist/book/reference/config.html>
- <https://docs.github.com/en/actions/reference/workflows-and-actions/contexts>
- <https://github.com/astral-sh/uv/blob/main/dist-workspace.toml>
- <https://github.com/astral-sh/uv/blob/main/.github/workflows/check-release.yml>
- <https://github.com/astral-sh/ruff/blob/main/.github/workflows/ci.yaml>
- <https://github.com/rust-lang/cargo/blob/master/.github/workflows/main.yml>

Bearing: `landing:a-seeded-file-still-carries-the-invariants`, for the `pr-run-mode-not-skip` and `workflow-runs-on-a-request` codes, and the jobs `bindings/rust.md` serves for the project's own gate.

## projen, on markers, anti-tamper, and the sweep

Generated files carry a magic marker, and any file carrying it is cleaned up automatically once it is no longer generated. Most generated files are marked read-only, and an anti-tamper check runs in CI to confirm they were not modified during a build.

- <https://github.com/projen/projen>
- <https://projen.io/docs/introduction/the-projen-workflow/>

Bearing: the sweep-on-drop behaviour release-kit already implements for skills as `distribution:an-install-sweeps-what-the-payload-dropped`, arrived at independently, and the argument that a landing record is what would make the same sweep possible for landed files.

The read-only-file approach is the half not worth copying. Landed files carry sentinels the operator must fill, so they are explicitly not read-only — which is also why `landing:a-dropped-file-stays` is the opposite choice from projen's, and deliberately so.

## copier and cruft, on the receipt living in the generated project

copier writes an answers file into the generated project holding both the answers and the template version that produced it. Its update regenerates from the current template using those answers, diffs that against the actual project to extract local changes, applies migrations, then re-applies the local modifications, leaving conflict markers where both sides moved. cruft writes a record whose two load-bearing fields are the template and the commit.

- <https://copier.readthedocs.io/en/stable/updating/>
- <https://cruft.github.io/cruft/>

Bearing: `landing:a-landing-leaves-a-record` and `landing:a-record-states-its-schema`. The shape of what this family records is unanimous and worth naming: an opaque identity, a commit or a tag, never a per-file version.

Warning worth carrying: copier documents that hand-editing the answers file tricks the tool into believing the wrong version generated the project. A record needs the same warning and, better, a way to detect the tampering rather than only forbidding it, which is what the per-file digests give `rk status --check`.

## pre-commit, on immutable pins and an explicit update verb

The revision field must point at a fixed tag; mutable references are unsupported and are never updated after first install. The autoupdate verb is the explicit action that rewrites the configuration to the latest released versions and converts a mutable revision to an immutable one. Versions of additional dependencies must still be updated by hand.

- <https://github.com/pre-commit/pre-commit/issues/1354>
- <https://github.com/pre-commit/pre-commit/issues/3521>
- <https://docs.renovatebot.com/modules/manager/pre-commit/>

Bearing: the discipline `versions.toml` already applies, and the argument that a landing record holds an immutable identity plus an explicit update command — `rk upgrade` — rather than a floating reference that quietly re-resolves.

## Terraform, on a committed lock file of versions and checksums

Project configuration records the exact selected versions and their checksums in a lock file that is committed and reviewed like any other change. The shared cache is an optimization, not the record.

- <https://developer.hashicorp.com/terraform/language/files/dependency-lock>
- <https://developer.hashicorp.com/terraform/cli/commands/providers/lock>

Bearing: the digest-bearing record, and the separation between a materialization cache, which is regenerable and authoritative for nothing, and a record that must be believed. It is also the precedent for committing the record rather than treating it as local state.

## Adoption, and why none of it is blind

Import brings pre-existing infrastructure under management by recording it rather than recreating it. A provider option exists specifically to take ownership of a resource that already exists, and a Helm proposal covers the same ground through server-side apply. No member of this family adopts by overwriting what it finds.

- <https://helm.sh/community/hips/hip-0023/>
- <https://developer.hashicorp.com/terraform/cli/commands/plan>

Bearing: `landing:an-adoption-writes-the-record-and-nothing-else`. Adoption records both digests for a file the target may edit, refuses outright on a mismatch in a file the payload owns, and changes no target file — which is the strictest reading of this family's shared rule rather than a departure from it.

## pre-commit, on the stages and environment the landed hooks lean on

Verified 2026-09-01. A hook declares its `stages`, and a stage's hooks run only where that hook type is installed — `pre-commit install --hook-type commit-msg --hook-type pre-push` — which `default_install_hook_types` makes the default for a repository. Pre-push hooks receive `PRE_COMMIT_REMOTE_BRANCH` carrying the full remote ref being pushed, so a local hook can refuse a push to `refs/heads/master` or a `refs/tags/v*` tag; pushes that delete a ref intentionally skip the hooks (pre-commit issue 3050), and git tells a pre-push hook nothing about `--force`, so a force-push has no local mirror. `pre-commit/pre-commit-hooks` ships `no-commit-to-branch`, protecting `main` and `master` by default with `--branch` and `--pattern` overrides; it reads the current branch rather than a commit event, so a `pre-commit run` sweep over a checked-out protected branch fails the same way a commit would. The `SKIP` environment variable, a comma-separated list of hook ids, is pre-commit's documented way to skip named hooks for one invocation and reports them as skipped rather than silently omitting them, which is how a CI sweep keeps the guard out of a context that commits nothing.

`compilerla/conventional-pre-commit` checks a commit message against Conventional Commits at the `commit-msg` stage, with `--strict`, `--force-scope`, and a comma-delimited `--scopes` list; `crate-ci/committed` offers `allowed_scopes` but no option to require a scope, which is what decided between them.

Verified 2026-09-08, by reading `conventional_pre_commit/format.py` in the hook repository the landed block pins. The `r_scope` property builds its pattern two ways. Named scopes produce an alternation of exactly those words. No named scopes, with `--force-scope`, produce `(\([\w :,\-/.#]+\))`: a scope stays mandatory, and any token of word characters, spaces, and the delimiters `: , - / . #` passes. The tool takes no scope pattern of its own, so the landed hook holds that a scope is present, and the shape is held beside it — by the title check on the forge, and by `rk message --check` at the desk.

- <https://pre-commit.com/#pre-commit-configyaml---top-level>
- <https://pre-commit.com/#pre-push>
- <https://pre-commit.com/#temporarily-disabling-hooks>
- <https://github.com/pre-commit/pre-commit/issues/3050>
- <https://github.com/pre-commit/pre-commit-hooks>
- <https://github.com/compilerla/conventional-pre-commit>
- <https://github.com/crate-ci/committed/blob/master/docs/reference.md>
- <https://git-scm.com/docs/githooks>

Bearing: `landing:a-landed-hook-serves-the-release-convention-alone`. Every mirror the block carries rests on a documented mechanism above, the two honest limits — `--no-verify` and the invisible force-push — are stated by the same sources, and the two third-party hooks are pinned in `versions.toml` like every other snippet pin.

## Arming the release request

Three upstream facts carry the arming steps the landed release workflows render. GitHub states that with the exception of `workflow_dispatch` and `repository_dispatch`, events triggered by `GITHUB_TOKEN` do not create workflow runs at all, so an arm made with the default token merges a bump that starts no publish. The release-plz action at the pinned commit declares a `pr` output — the release request it opened or refreshed, a JSON object carrying `number`, `head_branch`, `base_branch`, and `html_url` — and release-plz refreshes by force-push on GitHub and by closing the outdated request and opening a fresh one on GitLab, which is why an arm is re-applied on every run. The release-please action sets its outputs dynamically rather than in `action.yml`: `prs_created` is true if any pull request was created or updated, and `pr` is a JSON string of the PullRequest object, unset when none exists.

- <https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows>
- <https://release-plz.dev/docs/github/output>
- <https://github.com/release-plz/action/blob/2eb1d8bcb770b4c48ccfaad919734b38b51958c9/action.yml>
- <https://github.com/googleapis/release-please-action/blob/45996ed1f6d02564a971a2fa1b5860e934307cf7/README.md>

Bearing: `landing:the-arming-identity-is-the-bot`, both scenarios, and the arming steps in every landed release workflow. The default-token fact is the single most load-bearing citation in the arming design: it is why the arm sits in the job that already mints the bot token.

## The Nix capability's destinations

The Nix-owned destinations the payload names, verified against the Nix reference documentation on 2026-09-04. The flake file must be named `flake.nix` and live in the repository's root directory: the reference manual's flake description states that a flake is a filesystem tree whose root directory contains `flake.nix`, and the `nix flake` reference documents resolution of a `github:`/`git+https:` reference to the flake file at the tree's root. `flake.lock` is written beside it by the lock machinery, in the same root, and is maintained by Nix's own commands after landing — which is why it lands as a `state` file. The `nix/` subdirectory for auxiliary expressions is a placement release-kit chooses for its own seed, not a Nix requirement: the seed's `flake.nix` names the path explicitly, so a target may move it and adjust the call.

- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake>
- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake-lock>

Bearing: `landing:the-flake-pair-lands-all-or-nothing`, and `placement:a-third-party-destination-names-its-source` for `flake.nix` and `flake.lock`.

## GitLab, on why an include cannot isolate a target's jobs

Included configuration is merged by a deep merge, at any depth: "Included files are read in the order defined in the configuration file, and the included configuration is merged together in the same order", and "after all configuration added with `include` is merged together, the main configuration is merged with the included configuration". The main file wins a key both files declare, so the included file contributes every key the main file omits, whatever the include order. A file the target owned as an include would therefore add `allow_failure`, `needs`, `retry`, `tags`, or `artifacts` to a job release-kit owns, and would add `default`, `variables`, and `workflow` subkeys globally. No include order prevents it.

- <https://docs.gitlab.com/ci/yaml/includes/>

Bearing: `landing:the-flake-pair-lands-all-or-nothing`, and the `project-jobs` bridge in both rendered GitLab parents. This is the fact that rules out the simpler shape and leaves a child pipeline as the only isolating one.

## GitLab, on the child pipeline that carries the target's jobs

A downstream pipeline is a separate configuration, so a child's globals, `default` block, and job names never merge into the parent. `trigger:strategy` forces the trigger job to wait for the downstream pipeline before it is marked success, against a default that marks the trigger job success as soon as the downstream pipeline is created. Of its two values, `mirror` "mirrors the status of the downstream pipeline exactly" and the reference introduced it in GitLab 18.2; `depend` is documented as "not recommended, use `mirror` instead", and its status can read running while the downstream waits on a manual job. Two documented shapes of a child job cannot gate: an optional manual job "does not affect the status of the downstream pipeline or the upstream trigger job", and a failed job under `allow_failure: true` leaves the downstream pipeline successful. A rule combining `if:` and `exists:` matches only when both match — "the rule evaluates to true only when all included keywords evaluate to true" — and `exists:` paths are relative to the project directory. Inside the child, `CI_PIPELINE_SOURCE` reads `parent_pipeline`.

- <https://docs.gitlab.com/ci/yaml/#triggerstrategy>
- <https://docs.gitlab.com/ci/yaml/#rulesexists>
- <https://docs.gitlab.com/ci/pipelines/downstream_pipelines/>
- <https://docs.gitlab.com/ci/jobs/job_rules/#complex-rules>
- <https://docs.gitlab.com/ci/variables/predefined_variables/>

Bearing: the `project-jobs` bridge, the GitLab 18.2 version floor `rk setup step forge-version` enforces, and the gating caveats `forges/gitlab.md` states. The 18.2 date is what makes the floor a requirement rather than a preference.

## GitLab, on the pipeline a merge request did not get

"If a pipeline contains only jobs in the `.pre` or `.post` stages, it does not run." On a merge-request event the landed release jobs' rules never match, so a title gate at `stage: .pre` was the only job that survived, and the forge created no pipeline at all. The project setting `only_allow_merge_if_pipeline_succeeds` then blocked every merge for want of the pipeline it waits on. `.pre` and `.post` need no entry in `stages:`; every other stage does, and arrays are not deep-merged, so a target cannot contribute a stage from a file of its own.

- <https://docs.gitlab.com/ci/yaml/#stage-pre>
- <https://docs.gitlab.com/ci/yaml/#stages>

Bearing: the `test` stage both rendered GitLab parents declare last, and the `stage: test` the landed `mr-title` job carries. This is the defect the bridge would have inherited, since the bridge sits in an ordinary stage too.

## The writer's version, and why no comparable tool prompts from it

Verified 2026-09-09. Across this family a record carries up to three separable facts, and the version of the executable that wrote it is never the one that answers "is there something to take". Terraform state is the only member carrying all three at once and separates them explicitly: `version` is the state format version and is 4 for every Terraform 1.x, `terraform_version` records the Terraform that wrote the snapshot, and `serial` is a monotonic counter incremented on each modification, with `lineage` a UUID fixed at creation because serials compare only within one lineage. Terraform refuses a state written by a newer Terraform, the same conservative direction as `landing:a-target-is-never-downgraded`, and it prompts nothing merely because the running binary is newer than `terraform_version`. `Cargo.lock` carries a top-level `version` that is the lockfile format version alone — cargo writes no cargo version into it — and an older cargo hard-errors on a format it does not understand rather than warning. copier's answers file records `_commit`, the template version, and cruft's record carries `template` and `commit`; neither writes the scaffolding executable's own version at all. cargo-dist's `cargo-dist-version` is the apparent counterexample and is materially different: it is a pin that the generated CI fetches and runs, a human edits it deliberately when upgrading, and drift is caught separately by `dist generate --check` comparing regenerated content, which is why bumping the pin without regenerating is the documented way to trip that check.

- <https://developer.hashicorp.com/terraform/language/state/backends>
- <https://developer.hashicorp.com/terraform/cli/commands/state/push>
- <https://github.com/hashicorp/terraform/pull/7109>
- <https://doc.rust-lang.org/stable/nightly-rustc/cargo/core/resolver/enum.ResolveVersion.html>
- <https://github.com/rust-lang/cargo/issues/10046>
- <https://copier.readthedocs.io/en/stable/configuring/>
- <https://cruft.github.io/cruft/>
- <https://axodotdev.github.io/cargo-dist/book/workspaces/simple-guide.html>

Bearing: `landing:status-judges-only-under-check`, for the pending count the upgrade prompt reads and for the recorded `rk_version` prompting nothing on its own. The distinction the family draws is between a stamp and a pin: `rk_version` is written from `CARGO_PKG_VERSION` at every landing, adoption, and upgrade, so it is a stamp, and a stamp of a release-automation tool necessarily lags its own crate version, because the version is derived from the commit that already carries the record. `cargo-dist-version` never drifts that way only because nothing writes it automatically.

## release-plz, on what a release request can and cannot carry

Verified 2026-09-09. The configuration reference documents `[workspace]`, `[[package]]`, and `[changelog]`, and no command hook of any kind: there is no facility for running a command while the release request is built, and so none for adding a generated file to it. `allow_dirty` permits pre-existing changes during an update, which is not a generation step. A surrounding workflow can read the action's `pr` output, check out its head, commit, and push, but release-plz refreshes that branch by force-push, and in this project's landed workflow the same job arms the request for auto-merge immediately afterwards.

- <https://release-plz.dev/docs/config>
- <https://release-plz.dev/docs/github/output>

Bearing: `landing:status-judges-only-under-check`, as the option not taken. Keeping the recorded version equal to the crate version would take custom orchestration racing the force-push and the arm, to hold a fact that answers no question the report asks.

## semantic-release and GoReleaser, on where their configuration lives

semantic-release reads its configuration from `.releaserc` with no extension or with `.yaml`, `.yml`, `.json`, `.js`, `.cjs`, or `.mjs`, from `release.config.js`, `release.config.cjs`, or `release.config.mjs`, or from a `release` key in `package.json`. GoReleaser looks for `.config/goreleaser.yml`, `.config/goreleaser.yaml`, `.goreleaser.yml`, `.goreleaser.yaml`, `goreleaser.yml`, and `goreleaser.yaml`, in that order.

- <https://semantic-release.gitbook.io/semantic-release/usage/configuration>
- <https://goreleaser.com/customization/>

Bearing: `landing:a-landing-classifies-its-target-first`. The marker catalog `rk assess` reads, `RELEASE_MARKERS` in `src/assess.rs`, carries every documented name, and `package.json` counts only with the `release` key, because an ordinary Node manifest is not a release mechanism.
