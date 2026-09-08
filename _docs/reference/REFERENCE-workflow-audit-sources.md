# Workflow Audit Sources

External sources behind the workflow audit this repository runs over its own `.github/workflows/` and over the payload in `snippets/`: where each tool reads its configuration, what a suppression means to it, and how comparable projects record the findings they accept. Each entry states what the source says and what this repository does with it.

Verified against the listed sources on 2026-09-08.

## zizmor, on where it reads its configuration

zizmor discovers its configuration by walking up from the input: `.github/zizmor.yml`, `.github/zizmor.yaml`, then `zizmor.yml` and `zizmor.yaml` at the repository root. `--config <FILE>` names one file across every input group, and `--no-config` disables configuration loading entirely. Passing `.github/workflows/` is a documented special case: discovery starts two parents above the given directory, so a `zizmor.yml` config is never confused with a workflow of that name.

- <https://docs.zizmor.sh/configuration/>
- <https://docs.zizmor.sh/usage/>

Bearing: this repository keeps `zizmor.yml` at the root, beside `_typos.toml`, `dprint.json`, `lychee.toml`, and `deny.toml`, and names it with `--config` in the hook rather than relying on discovery. Both positions are documented and both are in use upstream, so the root is a choice this repository makes for consistency with its other tool configurations, not a vendor requirement.

## zizmor, on what an ignore matches

Each member of `rules.<audit>.ignore` is a workflow rule written as `filename.yml`, optionally with `:line` and `:column`. The filename is the base name of the workflow. Observed on 2026-09-08 with zizmor 1.30.0: a path-qualified entry such as `.github/workflows/release.yml` matches nothing and silently suppresses nothing, and a base-name entry suppresses the audit in every input sharing that base name.

- <https://docs.zizmor.sh/configuration/>

Bearing: this repository ships a `release.yml` of its own in `snippets/bash/github/`, so an ignore for the generated `release.yml` would reach the payload too. The payload hook runs `--no-config` for that reason.

## zizmor, on inline suppression

A finding is suppressed inline with `# zizmor: ignore[rulename]`, several audits separated by commas, and free text allowed after the bracket. The comment must sit inside a span the finding identifies and must be a real YAML comment, so it cannot go inside a block literal. Observed on 2026-09-08: a comment placed on the line above a step's `- uses:` line does not suppress a finding anchored to that step; a comment inside the step's own `with:` block does.

- <https://docs.zizmor.sh/usage/>

Bearing: every accepted finding in `snippets/` is an inline comment in the snippet, so it renders into each target, carries its reasoning to the reader who meets it there, and drifts no rendered file.

A payload comment states its reason and names no case, which is the permanent-exception half of `spec-to-code:a-suppression-names-its-case`. `pull_request_target` is the trigger that makes the title gate unforgeable, and the bash release request's checkout persists the credential its own push uses; both are constructs this project chose and keeps, so no record could carry a retirement condition anyone can meet. The suppression over `.github/workflows/release.yml` is the other half, a mask over an external defect, and it names its record.

## Comparable projects, on recording an accepted finding

Every project read here uses the same shape: per-audit `ignore` lists keyed by base filename, each entry carrying a comment that gives the reason. None suppresses a generated file inline. The location splits: two keep the file at the repository root and three keep it under `.github/`, so both discovery positions are in real use and neither is the convention.

- <https://github.com/n0-computer/iroh/blob/main/zizmor.yml> — root. Ignores `github-app` for one workflow, with three lines explaining that the token itself is least-privilege and that the install scope is what remains flagged.
- <https://github.com/prefix-dev/rattler-build/blob/main/zizmor.yml> — root. Ignores `artipacked` for the workflows that need git credentials to push.
- <https://github.com/prefix-dev/pixi/blob/main/.github/zizmor.yml> — under `.github/`. Ignores `template-injection` for `release.yml`, reasoning that the release matrix values are defined in-workflow rather than from untrusted input, and ignores `artipacked` for the workflows whose checkouts push.
- <https://github.com/astral-sh/ruff/blob/main/.github/zizmor.yml> — under `.github/`. Ignores `secrets-outside-env` for six workflows, with a TODO naming the exit.
- <https://github.com/astral-sh/uv/blob/main/.github/zizmor.yml> — under `.github/`. Disables `secrets-inherit` outright, with the reason recorded above it.

Bearing: this repository follows the same shape and adds the exit its own convention asks for, a `KI-` record whose `retire_when` names the upstream release that removes the entry.

## release-plz, on scoping the App token it authenticates with

release-plz's own release workflow mints its App token with `permission-contents: write` on the job that runs `command: release`, carrying the comment that the pull-requests permission is not needed for that command, and with `permission-contents: write` plus `permission-pull-requests: write` on the job that runs `command: release-pr`. The documentation's GitHub App permission list, Contents and Pull requests both read and write, covers both commands together.

- <https://github.com/release-plz/release-plz/blob/main/.github/workflows/release-plz.yml>
- <https://release-plz.dev/docs/github/token>

Bearing: the payload's rust binding mints the same two scopes on the same two jobs.

## actions/create-github-app-token, on the default scope

The action takes a `permission-<permission name>` input per permission. Without one, the documentation states that the token inherits all of the installation's permissions, and it recommends listing the required permissions explicitly.

- <https://github.com/actions/create-github-app-token>

Bearing: every mint in the payload names its scope, in all three technology bindings.

## cargo-dist, on the workflow it generates

The generated `release.yml` sets `contents: write` at workflow level and interpolates expressions into `run:` bodies. Both are open upstream. dist 0.32.0 is the latest release and the one this repository pins, so no upgrade removes them today.

- <https://github.com/axodotdev/cargo-dist/issues/2320> — open, 2026-03-04, on the interpolations, with a candidate fix linked.
- <https://github.com/axodotdev/cargo-dist/issues/62> — open, 2023-01-31, on the shell quoting.
- <https://github.com/axodotdev/cargo-dist/issues/2133> — closed completed 2025-10-22; moved `attestations` and `id-token` to the jobs and left `contents: write` at workflow level.

Bearing: `_docs/reference/known-issues/KI-dist-generates-an-unhardened-workflow.md` records the acceptance and its retirement condition.
