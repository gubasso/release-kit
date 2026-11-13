# Release Engine Sources

External sources behind the release engine `rk release` runs for the bash and python pairs: what each upstream release tool does with a uv project and on which forge, what uv, git-cliff, and the publishing actions do, and what GitLab's release service does. Each section states what the source says on the date given, and where it bears. The scenarios that ran are in [the comparison of release tools for a uv project](../comparison-docs/COMPARISON-release-tools-for-a-uv-project.md).

The research read these sources on 2026-11-11; each section names the date it was re-read.

## release-please, on the python strategy and uv.lock

Verified: 2026-11-12

The latest release is `v17.11.2`, published 2026-08-24. The python strategy reads `pyproject.toml` and writes its version through the `PyProjectToml` updater, which rewrites `project.version` or, failing that, `tool.poetry.version`. When the version is dynamic, the updater logs "dynamic version found in 'pyproject.toml'. Skipping update." and leaves the file unchanged. The strategy's update list (`src/strategies/python.ts`) names `CHANGELOG.md`, `setup.cfg`, `setup.py`, `pyproject.toml`, and the package's version files. It names no `uv.lock`.

Issue 2561, "Support `uv.lock` files out-of-the-box.", is open. It opened on 2025-06-26 and states that release-please "already understands and correctly manages the `pyproject.toml` file used by `uv`, but not the `uv.lock` lockfile, which also contains the package version." It records the workaround as an `extra-files` entry of type `toml` on path `uv.lock` with the jsonpath `$.package[?(@.name.value=='my-package')].version`.

Issue 2455 is open. It opened on 2024-12-18 and reports that a jsonpath filter on `uv.lock` matched nothing: "No entries modified in $.package[?(@.name==="app")].version". The comment of 2025-02-07 that issue 2561 links finds the cause: release-please parses TOML into objects that carry `start`, `end`, and `value`, so a filter has to compare `@.name.value`, not `@.name`. Another comment in the same thread relocks instead. After release-please opens its request, a workflow checks out the release branch, runs `uv lock`, and pushes a commit "ci: update uv.lock".

Pull request 2693, "feat: support uv.lock files in the Python strategy", is open and not merged. It opened on 2026-03-02 and was last updated on 2026-08-26. It proposes a `UvLock` updater that rewrites the project's `[[package]]` entry in place and gates it on a known project name. The project name is matched with PEP 503 normalization, and the updater does not create the file (`createIfMissing: false`).

- https://github.com/googleapis/release-please/issues/2561
- https://github.com/googleapis/release-please/issues/2455
- https://github.com/googleapis/release-please/issues/2455#issuecomment-2643132770
- https://github.com/googleapis/release-please/pull/2693
- https://github.com/googleapis/release-please/blob/main/src/updaters/python/pyproject-toml.ts
- https://github.com/googleapis/release-please/blob/main/src/strategies/python.ts
- https://github.com/googleapis/release-please/releases/tag/v17.11.2

Bearing: a release-please pair for uv cannot rely on the python strategy to keep `uv.lock` in step. It needs either the `@.name.value` jsonpath `extra-files` entry, which edits the lock text, or a relock commit on the release branch, and a dynamic version is skipped silently.

## releaser-pleaser, on GitLab

Verified: 2026-11-12

releaser-pleaser is a Go tool by Julian Tölle (`apricote`) that "streamlines the release process through GitHub Actions or GitLab CI". Its README names "Support for multiple forges (both GitHub and GitLab)" as a key difference from release-please. The GitHub Action also takes `forge: forgejo`. The latest release is `v0.9.0`, published 2026-07-05. The repository has 58 stars and is GPL-3.0. Its human contributors are `apricote` with 139 commits, `jooola` with 2, and six others with one each. A Renovate bot carries 329 commits.

The GitLab tutorial states: "releaser-pleaser requires Fast-forward merges and squashing. With other merge options it can not reliably find the right merge request for every commit on main." It directs the operator to set the merge method to "Fast-forward merge" and "Squash commits when merging" to "Require". The job needs a project access token with role Maintainer and scopes `api`, `read_repository`, and `write_repository`, because "The default GITLAB_TOKEN available in CI jobs does not have enough permissions". The tool is published as a GitLab CI/CD component, `$CI_SERVER_FQDN/apricote/releaser-pleaser/run`. When the release merge request merges, releaser-pleaser "automatically creates a Git tag and GitLab Release". The component does not support floating tags, so a pipeline pins a full version or a commit SHA.

The updaters are `changelog`, enabled by default; `generic`, enabled by default, which rewrites any line that carries the marker `x-releaser-pleaser-version` in the files listed in `extra-files`; and `packagejson`, disabled by default. No updater handles `pyproject.toml` or `uv.lock`. The CLI `run` command takes only `--forge`, `--branch`, `--owner`, `--repo`, `--extra-files`, `--updaters`, and `--api-url`, and offers no command hook. Issue 160, "feat: support user-hooks for additional file changes", is open since 2025-03-18. Its author notes that "on gitlab-ci we are running in a single binary container that does not have access to any other runtimes/programs". Issue 463, open since 2026-08-01, reports that GitLab's automatic rebase before merge (GitLab 19.2) makes releaser-pleaser "tag a commit that is not on the target branch" when squashing is not required.

- https://github.com/apricote/releaser-pleaser
- https://apricote.github.io/releaser-pleaser/tutorials/gitlab.html
- https://github.com/apricote/releaser-pleaser/blob/main/docs/reference/updaters.md
- https://github.com/apricote/releaser-pleaser/blob/main/docs/reference/gitlab-cicd-component.md
- https://github.com/apricote/releaser-pleaser/blob/main/docs/guides/updating-arbitrary-files.md
- https://github.com/apricote/releaser-pleaser/blob/main/docs/reference/github-action.md
- https://github.com/apricote/releaser-pleaser/blob/main/cmd/rp/cmd/run.go
- https://github.com/apricote/releaser-pleaser/issues/160
- https://github.com/apricote/releaser-pleaser/issues/463
- https://github.com/apricote/releaser-pleaser/releases/tag/v0.9.0

Bearing: releaser-pleaser gives a GitLab release merge request only under fast-forward and required squash. Its container cannot run `uv lock`, so a uv pair on it edits the lock version by a marker line or relocks in a separate job.

## python-semantic-release, commitizen, and semantic-release, on the release model and the uv relock

Verified: 2026-11-12

All three release on push. None of them opens a release request.

python-semantic-release's latest release is `v10.7.0`, published 2026-09-22. Its uv guide states that after it rewrites the version, "this action will cause uv to fail on the next execution because the lock file will be out of sync". It gives two remedies. One puts `uv lock --upgrade-package "$PACKAGE_NAME"` and `git add uv.lock` into `build_command`. The other runs PSR twice: `semantic-release -v version --skip-build --no-commit --no-tag --no-changelog`, then `uv lock --upgrade-package <your-package-name>` and `git add uv.lock`, then `semantic-release -v version`. The guide states that "The intent of the lock upgrade-package call is ONLY to update the version of your project within the lock file". Its PSR GitHub Action "runs in a Docker environment does not include uv by default". Its example workflow triggers `on: push: branches: - main`.

commitizen's latest release is `v4.19.3`, published 2026-10-24. Its `uv` version provider "Manages version in both pyproject.toml (project.version) and uv.lock (package.version for the matching package name)". The docs add that "Even though uv follows PEP 621 format, pep621 does not manage the version in uv.lock". The provider's source (`commitizen/providers/uv_provider.py`) edits the `version` of the `[[package]]` entry whose name matches the canonicalized project name, through `tomlkit`. It does not run uv. The GitLab CI tutorial's goal is to "Bump a new version every time that a change occurs on the master branch". Its `auto-bump` job runs `cz bump --yes`, then `git push origin master:$CI_COMMIT_REF_NAME` and `git push origin $TAG`, and its commit must carry `[skip-ci]` "otherwise, the process will generate a loop".

semantic-release's latest release is `v25.0.9`, published 2026-08-05. Its README states that it "is meant to be executed on the CI environment after every successful build on the release branch". semantic-release-uv (`Deltamir/semantic-release-uv`, npm `@open_resources/semantic-release-uv`, latest `1.5.1` published 2025-12-05, one star) runs, in its `prepare` step, a rewrite of `project.version` in `pyproject.toml`, then `uv lock`, then `uv build`. Its `publish` step runs `uv publish` with `PYPI_TOKEN`, which its `verifyConditions` step requires. The README says to commit `pyproject.toml` and `uv.lock` with `@semantic-release/git`. Its plain `uv lock` call does not restrict the relock to the project's own entry, a reading of `src/prepare.ts`.

- https://python-semantic-release.readthedocs.io/en/stable/configuration/configuration-guides/uv_integration.html
- https://commitizen-tools.github.io/commitizen/config/version_provider/
- https://commitizen-tools.github.io/commitizen/tutorials/gitlab_ci/
- https://github.com/commitizen-tools/commitizen/blob/master/commitizen/providers/uv_provider.py
- https://github.com/semantic-release/semantic-release
- https://github.com/Deltamir/semantic-release-uv
- https://github.com/Deltamir/semantic-release-uv/blob/main/src/prepare.ts
- https://registry.npmjs.org/@open_resources%2fsemantic-release-uv

Bearing: these tools show the two relock shapes a uv engine chooses between. One is a text edit of the project's lock entry (commitizen's `uv` provider). The other is a scoped `uv lock --upgrade-package <name>` after the version rewrite (PSR). All three fit a release-on-push model, not a reviewed release request.

## knope and cocogitto, on the forges they reach

Verified: 2026-11-12

knope's latest release is `knope/v0.23.2`, published 2026-10-22. Its forge reference states: "Currently, Knope can interact with the following forges: Gitea, GitHub". It does not list GitLab. Issue 1974, "gitlab support", is open since 2026-07-23. The maintainer `dbanty` answered on 2026-07-25: "Not planned yet, but if enough people ask for it (or someone wants to contribute it!) then we shall have it." A user's reply shows a GitLab release through `Command` steps that run `git push`, `git push --tags -o ci.skip`, and `glab release create`. Issue 1995, "Feature request: support `uv.lock`", is open since 2026-07-30. Its whole body is "...for Astral's `uv` Python tool". It is a uv.lock request, not a GitLab one. knope's versioned-file sources include `pyproject.rs`, `cargo_lock.rs`, `deno_lock.rs`, and `package_lock_json.rs`, and no uv lock file.

cocogitto's latest release is `7.0.0`, published 2026-03-04. Its documentation website (the `website/` tree, served at https://docs.cocogitto.io) has CI/CD pages for a GitHub Action, a GitHub bot, and a Docker image, and no GitLab page. The changelog guide's `remote` template takes `--remote github.com --owner … --repository …` to build links. Whether the template renders GitLab-shaped links for a GitLab host is unverified, because the page shows only a GitHub example. cocogitto runs `pre_bump_hooks` before the version commit and `post_bump_hooks` after the version is created. The bump guide states "There is no rollback procedure for post-bump hook".

- https://knope.tech/reference/concepts/forge/
- https://github.com/knope-dev/knope/issues/1995
- https://github.com/knope-dev/knope/issues/1974
- https://github.com/knope-dev/knope/releases/tag/knope/v0.23.2
- https://docs.cocogitto.io/
- https://github.com/cocogitto/cocogitto/tree/main/website/ci_cd
- https://github.com/cocogitto/cocogitto/blob/main/website/guide/changelog.md
- https://github.com/cocogitto/cocogitto/blob/main/website/guide/bump.md

Bearing: neither tool creates a GitLab release or a release merge request natively. A GitLab pair on either runs `glab` or the API from a command step, and a `pre_bump_hooks` entry is where cocogitto would run `uv lock`.

## release-plz, on the request refresh, the release rule, the lock update, and git-cliff as a library

Verified: 2026-11-12

The latest release is `release-plz-v0.4.3`, published 2026-11-02.

Request refresh. When a release PR is already open and holds only bot commits, release-plz does this: "The update mechanism is simple: overwrite everything and force-push. 💥". The docs mark that path "(Not available on Gitea and GitLab)". Otherwise, "release-plz closes the old PR and opens a new one". It also closes the release PR "when it cannot update it (for example, the force-push fails due to merge conflicts)".

Release rule. `release_package_if_needed` in `crates/release_plz_core/src/command/release.rs` (read at commit `11e67b5fe58ec01399f55be1c576f63c25e4e6bf`) first skips a package whose tag exists: "Already published - Tag {} already exists". When publishing is enabled (`config.publish.enabled && !config.git_only`), it asks each registry index whether the version is published and skips it if so. Under `git_only` it skips registry checks entirely, with the comment "When publishing is disabled (e.g., git_only mode), skip registry checks entirely and only perform git tag/release operations." The config doc reads "Whether the package versions are tracked with git tags instead of a cargo registry." `should_release` releases only when the current commit comes from a release PR, unless `release_always` is set. The `why` page states that release-plz "looks at the cargo registry if the relative git tag doesn't exist". On GitLab, `release-plz release --forge gitlab` needs a token with role Maintainer and scopes `api` and `write_repository`.

Lock update. `update_cargo_lock` runs `cargo update --workspace`, and runs plain `cargo update` only when dependency updates are enabled. The update docs list "Updates all dependencies by running cargo update (disabled by default)."

git-cliff as a library. The workspace `Cargo.toml` declares `git-cliff-core = { version = "2.14.2", default-features = false }`, and `release_plz_core` depends on it.

- https://release-plz.dev/docs/usage/release
- https://release-plz.dev/docs/usage/release-pr
- https://release-plz.dev/docs/usage/update
- https://release-plz.dev/docs/why
- https://github.com/release-plz/release-plz/blob/main/crates/release_plz_core/src/command/release.rs
- https://github.com/release-plz/release-plz/blob/main/crates/release_plz_core/src/command/update/mod.rs
- https://github.com/release-plz/release-plz/blob/main/Cargo.toml

Bearing: release-plz is the model a uv engine copies. A tag that exists means released. A registry decides only when publishing is on. The lock moves only for the workspace's own packages. git-cliff runs in-process. Its GitLab release request is recreated, not force-pushed.

## uv, on `uv version`, `uv build`, `uv publish`, `SOURCE_DATE_EPOCH`, and the `uv_build` refusal of a dynamic version

Verified: 2026-11-12

The latest uv release is `0.13.7`, published 2026-11-03. The package guide states: "By default, when `uv version` modifies the project it will perform a lock and sync. To prevent locking and syncing, use `--frozen`. To prevent only syncing, use `--no-sync`." The CLI reference describes `uv version --frozen` as "Update the version without re-locking the project", adding "The project environment will not be synced". It describes `--no-sync` as "Avoid syncing the virtual environment after re-locking the project". `--bump` accepts `major`, `minor`, `patch`, `stable`, `alpha`, `beta`, `rc`, `post`, and `dev`. It can be passed more than once, so `uv version --bump patch --bump beta` gives `1.3.0 => 1.3.1b1`.

`uv build` builds an sdist and a wheel into `dist/`. The guide recommends `uv build --no-sources` before publishing, "to ensure that the package builds correctly when `tool.uv.sources` is disabled".

`uv publish --trusted-publishing` takes `automatic`, `always`, or `never`. The reference reads: "By default, uv checks for trusted publishing when running in a supported environment, but ignores it if it isn't configured. uv's supported environments for trusted publishing include GitHub Actions and GitLab CI/CD." `--no-attestations` reads: "By default, uv attempts to upload matching PEP 740 attestations with each distribution that is published." The reference describes uploading attestations that already exist. It does not say that uv signs them. After trusted publishing, "uv will attempt to invalidate the short-lived PyPI token after publishing, even if publishing fails". On a partial failure, "With PyPI, you can retry the exact same command; existing identical files will be ignored", and other registries need `--check-url`.

`SOURCE_DATE_EPOCH`: the uv docs pages read here do not mention it. Issue 7066, "uv build doesn't respect SOURCE_DATE_EPOCH", closed as not planned on 2024-09-05. Its author wrote: "`uv build` is not responsible for any of this." The build frontend leaves timestamps to the backend. In `crates/uv-build-backend/src/source_dist.rs`, the sdist tar headers set entry type, size, and mode and set no mtime. The `uv_build` backend source read here contains no `SOURCE_DATE_EPOCH`. Whether `uv_build` archives are byte-reproducible is unverified. Issue 13139, "Deterministically normalize wheel ZIP metadata", is open.

`uv_build` refuses a dynamic version. `crates/uv-build-backend/src/metadata.rs` declares `#[error("Dynamic metadata is not supported")] Dynamic` and returns it whenever `project.dynamic` is non-empty (read at commit `f45ea9bd5335a60832c10cfb359b8e58bc358810`). Issue 14037, "Support Git scm-based versioning in `uv` build backend", is open since 2025-06-14. A commenter wrote: "You can switch to uv and keep hatchling around as the build backend."

- https://docs.astral.sh/uv/guides/package/
- https://docs.astral.sh/uv/reference/cli/
- https://github.com/astral-sh/uv/blob/main/docs/guides/package.md
- https://github.com/astral-sh/uv/blob/main/crates/uv-build-backend/src/metadata.rs
- https://github.com/astral-sh/uv/blob/main/crates/uv-build-backend/src/source_dist.rs
- https://github.com/astral-sh/uv/issues/14037
- https://github.com/astral-sh/uv/issues/7066
- https://github.com/astral-sh/uv/issues/13139
- https://github.com/astral-sh/uv/releases/tag/0.13.7

The release `0.13.7` publishes `uv-installer.sh` beside its archives. The installer downloaded from the release on 2026-11-12 hashes to `d263ee68314cf5bea36b08e6bd9e2c4563240d7adb73756cf33af5c199b92db4`. It carries `APP_VERSION="0.13.7"` and a SHA-256 for each platform archive, which it checks after the download. It installs into the directory `UV_UNMANAGED_INSTALL` names, with no receipt and no change to the shell profile. The release names no checksum file for the installer itself.

- https://github.com/astral-sh/uv/releases/download/0.13.7/uv-installer.sh

Bearing: the engine sets the version with `uv version <v> --no-sync`, which rewrites `pyproject.toml` and relocks, or with `--frozen` plus a separate scoped relock. It requires a static `project.version` under `uv_build`. The GitLab `pypi` job publishes with `uv publish --trusted-publishing always`, and the engine cannot count on `uv` to sign attestations or to honor `SOURCE_DATE_EPOCH`. The GitLab python jobs fetch the installer of the uv they need, check it against the digest above where that uv is the pinned `0.13.7`, and run it into their tool directory.

## astral-sh/setup-uv, on the uv version it chooses

Verified: 2026-11-12

The latest release is `v10.3.3`, published 2026-10-28. Its tag points directly at commit `68559e99cc807c1eb7e9238fd43a15bba460eec2`. The README states: "If you do not specify a version, this action will look for a required-version in a `uv.toml` or `pyproject.toml` file in the repository root. If none is found, the latest version will be installed." The input reference reads: "`version-file` … (default: searches uv.toml then pyproject.toml)". It also accepts `.tool-versions`, `requirements.txt`, and `uv.lock` when named. The `version` input accepts `latest` and `latest-known`. The action resolves versions from the `astral-sh/versions` manifest and downloads uv from the uv GitHub releases. The README at the `v10.3.3` tag carries the same required-version text.

- https://github.com/astral-sh/setup-uv
- https://github.com/astral-sh/setup-uv/blob/v10.3.3/README.md
- https://github.com/astral-sh/setup-uv/releases/tag/v10.3.3

uv's configuration page, read on 2026-11-12, states that "`uv.toml` files take precedence over `pyproject.toml` files". Where both files sit in one directory, "configuration will be read from `uv.toml`, and the `[tool.uv]` section in the accompanying `pyproject.toml` will be ignored."

- https://docs.astral.sh/uv/concepts/configuration-files/

Bearing: where `uv.toml` exists, it alone answers which uv the project requires, and a `required-version` in the `[tool.uv]` table of `pyproject.toml` beside it is ignored. Where no `uv.toml` exists, `pyproject.toml` answers. A target whose answering file names an exact `required-version` gets that uv from `setup-uv` with no version input. Every other target passes the `versions.toml` pin as `version`. Without that input, a target with no pin would get the latest uv on every run, and a target whose `uv.toml` names no pin could get the ignored `pyproject.toml` pin, because `setup-uv` searches both files. The workflow sets `enable-cache: false`, because the release jobs hold a publishing identity and read no cache another workflow writes.

## git-cliff, on `--bumped-version`, `--bump`, `--unreleased --tag`, and the `[bump]` table

Verified: 2026-11-12

The latest release is `v2.14.2`, published 2026-09-18. The website banner still announces v2.14.0. `git cliff --bumped-version` "Prints bumped version for unreleased changes". `--bump` "Bumps the version for unreleased changes [default: auto] [possible values: auto, major, minor, patch]". The bump page states that "fix:" increments PATCH, "feat:" increments MINOR, and a breaking change increments MAJOR, and that "The next version is checked against the regex value set by tag_pattern". `--tag <TAG>` "Sets the tag for the latest version". The examples show `git cliff --unreleased --tag 1.0.0 --prepend CHANGELOG.md`, which "generates a changelog for the tag '1.0.0' without creating the tag itself".

The `[bump]` table holds `features_always_bump_minor`, `breaking_always_bump_major`, `initial_tag`, `custom_major_increment_regex`, `custom_minor_increment_regex`, `no_increment_regex`, and `bump_type`. With `features_always_bump_minor = false`, a feature gives "A patch version update if the major version is 0". With `breaking_always_bump_major = false`, a breaking change gives "A minor version update if the major version is 0".

- https://git-cliff.org/docs/usage/bump-version/
- https://git-cliff.org/docs/configuration/bump/
- https://git-cliff.org/docs/usage/args/
- https://git-cliff.org/docs/usage/examples/
- https://github.com/orhun/git-cliff/releases/tag/v2.14.2

Bearing: a bash or uv pair can derive the next version with `git cliff --bumped-version` and write the section with `git cliff --unreleased --tag <v> --prepend CHANGELOG.md`. The `[bump]` table sets the 0.x rules, so they match the Rust pairs.

## pypa/gh-action-pypi-publish, on the files it uploads

Verified: 2026-11-12

The latest release is `v1.14.3`, published 2026-10-20. The tag points directly at commit `aef6c9b6e301dc803e1bc9133e9ce67ae9b0906b`. `action.yml` sets `packages_dir` to `default: dist`, and `twine-upload.sh` reads `packages-dir` with `packages_dir` as a fallback. The script fails only when the directory holds neither a `*.tar.gz` nor a `*.whl`. It then checks and uploads the whole directory with a glob:

```sh
twine check ${INPUT_PACKAGES_DIR%%/}/*
```

```sh
python /app/attestations.py "${INPUT_PACKAGES_DIR%%/}"
```

```sh
exec twine upload ${TWINE_EXTRA_ARGS} ${INPUT_PACKAGES_DIR%%/}/*
```

Attestations are on unless the input is `false`. The script turns them off with a warning when trusted publishing is not in use, or when the repository URL is not PyPI or TestPyPI.

- https://github.com/pypa/gh-action-pypi-publish/blob/v1.14.3/twine-upload.sh
- https://github.com/pypa/gh-action-pypi-publish/blob/v1.14.3/action.yml
- https://github.com/pypa/gh-action-pypi-publish/releases/tag/v1.14.3

Bearing: the publish job must hand the action a `dist/` that holds only this release's sdist and wheel. Any other file in it is checked, attested, and uploaded.

## GitLab, on native release support

Verified: 2026-11-12

The `release:` CI keyword "uses the `glab` CLI and creates the release with `glab release create`". It requires that "The release job must have access to the `glab` CLI, which must be in the `$PATH`". It authenticates with `CI_JOB_TOKEN` by default and prefers `GITLAB_TOKEN`, `GITLAB_ACCESS_TOKEN`, or `OAUTH_TOKEN` when set. "If the release already exists, it is not updated and the job with the `release` keyword fails." Under `release:tag_name`: "If the tag does not exist in the project yet, it is created at the same time as the release". The `ref` key of the `release:` keyword names the commit, tag, or branch for a tag that does not exist yet. `release:assets:links` takes `name` and `url` pairs.

The changelog API, `GET /projects/:id/repository/changelog`, generates notes "without committing them to a changelog file". A `.txt` suffix returns plain Markdown (GitLab 19.1). It selects commits by Git trailer: "`trailer` … Defaults to `Changelog`. Case-sensitive". It does not select commits by Conventional Commit type. When `from` is unset, GitLab picks "the last stable version tag that precedes your specified version", recognizing `X.Y.Z` or `vX.Y.Z`. "Pre-release tags like `v1.0.0-pre1` are ignored."

The release create API takes `assets:links` as an array of `name`, `url`, optional `direct_asset_path`, and `link_type` (`other`, `runbook`, `image`, `package`). A link is a URL, so the file itself has to be stored elsewhere first.

`Releases::CreateService#execute` (file last changed at commit `07fffa9f23fdf6193dcebaf10ba36c3ba77d12a6`, 2024-10-29) calls `tag = ensure_tag` before `create_release`. `ensure_tag` is `existing_tag || create_tag`, and `create_tag` runs `Tags::CreateService`. `create_release` then calls `release.save!`, and its `rescue StandardError => e` returns `error(e.message, 400)` with no tag deletion. So a release that fails to save leaves the new tag in place. This is a reading of the code, not documented behavior.

Issue 387240, "Release Please Integration - automated release merge request", asked GitLab for a release-please-style release merge request. It is closed (closed 2024-07-31) with 81 upvotes. Why it closed is unverified, because its notes need a signed-in API call. GitLab ships no release merge request feature in the pages read here.

- https://docs.gitlab.com/ci/yaml/#release
- https://gitlab.com/gitlab-org/gitlab/-/blob/master/doc/ci/yaml/_index.md
- https://docs.gitlab.com/api/repositories/#generate-changelog-data
- https://gitlab.com/gitlab-org/gitlab/-/blob/master/doc/api/repositories.md
- https://docs.gitlab.com/api/releases/
- https://gitlab.com/gitlab-org/gitlab/-/blob/master/doc/api/releases/_index.md
- https://github.com/gitlabhq/gitlabhq/blob/master/app/services/releases/create_service.rb
- https://gitlab.com/gitlab-org/gitlab/-/issues/387240 (now served as https://gitlab.com/gitlab-org/gitlab/-/work_items/387240)

Bearing: on GitLab the engine owns the release request itself, because the forge keeps none. It takes notes from git-cliff, not from the trailer-based changelog API. One release create with `assets[links]` makes the tag, after the assets are stored; because the service keeps a tag whose release save failed, a rerun finds that tag at its own commit and creates the release on it.

## The survey of uv projects

Verified: 2026-11-12

- Astral's uv runs a `workflow_dispatch` "Prepare release" job. The job runs `scripts/release.sh`: `rooster release` for the changelog, `cargo update -p uv`, then `uv lock --no-locked`, then a commit on `release/<version>`. A Codex step rewrites the changelog, and the job force-pushes and opens or updates a PR. https://github.com/astral-sh/uv/blob/main/.github/workflows/release-prepare.yml and https://github.com/astral-sh/uv/blob/main/scripts/release.sh
- FastAPI runs `prepare-release.yml` on `workflow_dispatch`. It bumps `fastapi/__init__.py` (a `pdm.backend` dynamic version) and the release notes with `scripts/prepare_release.py`, then opens a PR. Merging a PR labeled `release` drafts a GitHub release (`create-draft-release.yml`). Publishing the release runs `uv build` and `uv publish` (`publish.yml`). https://github.com/fastapi/fastapi/blob/master/.github/workflows/prepare-release.yml
- pydantic's maintainer runs `uv run release/prepare.py`, which updates `version.py` and runs `uv lock -P pydantic`. Then `release/push.py` opens a labeled PR and a draft GitHub release, and publishing the release starts the build. https://github.com/pydantic/pydantic/blob/main/release/README.md
- openai-agents-python uses release-please with `release-type: python` and the `extra-files` workaround `$.package[?(@.name.value=='openai-agents')].version` on `uv.lock`. https://github.com/openai/openai-agents-python/blob/main/release-please-config.json
- CrowdStrike falcon-mcp uses release-please action v3.7.13 with `release-type: python`. A later step checks out the release PR branch, runs `uv lock`, and pushes "chore: update uv.lock". https://github.com/CrowdStrike/falcon-mcp/blob/main/.github/workflows/release-please.yml
- LangChain deepagents uses release-please with one python package per `libs/*` directory and `separate-pull-requests`. An "Update lockfiles" job runs `uv lock --directory "$dir"` on the release branch and commits "chore: update lockfiles", because "release-please updates pyproject.toml versions but doesn't regenerate lockfiles". https://github.com/langchain-ai/deepagents/blob/main/.github/workflows/release-please.yml
- marge-bot on GitLab uses semantic-release with `@semantic-release/gitlab`. `semantic-release-replace-plugin` rewrites `version` in `pyproject.toml` and in the `marge-bot` entry of `uv.lock` by regex, and `@semantic-release/git` commits both. https://gitlab.com/marge-org/marge-bot/-/blob/main/.releaserc.yaml
- typed-settings on GitLab takes its version from git tags (`hatch-vcs`, `dynamic = ["version"]`). Its tag-pipeline `release` job runs `uv publish dist/*` with a `PYPI_ID_TOKEN` id token of audience `pypi`. https://gitlab.com/sscherfke/typed-settings/-/blob/main/.gitlab-ci.yml
- periodiq on GitLab uses `uv_build` with a static `version`. Its tag-pipeline `publish` job runs `uv build` and `uv publish --trusted-publishing always` with a `PYPI_ID_TOKEN` of audience `pypi`. https://gitlab.com/bersace/periodiq/-/blob/master/.gitlab-ci.yml
- The Cambridge UIS DevOps guidebook runs release-it from a shared GitLab CI template. The job releases on each merge to the default branch by default. With `USE_MERGE_REQUEST_RELEASE_FLOW` set, an `update-release-merge-request` job keeps a release merge request current instead. `@release-it/bumper` writes `project.version` in `pyproject.toml`. The pages read name no `uv.lock` handling. https://guidebook.devops.uis.cam.ac.uk/explanations/git-gitlab/gitlab-release-automation/ and https://guidebook.devops.uis.cam.ac.uk/howtos/git-gitlab/enable-automated-gitlab-releases/

The guidebook has no page titled "release-it"; the two URLs above are the release-it pages its sitemap lists.

Bearing: the sources show four shapes. Astral's uv, pydantic, openai-agents-python, falcon-mcp, deepagents, and marge-bot keep `uv.lock` in step through a step or a setting they wrote themselves. FastAPI and typed-settings take a dynamic version and edit no lock. typed-settings and periodiq publish from a tag pipeline with an `id_tokens` trusted publisher. marge-bot, and the UIS template by default, release on a push to the default branch. No surveyed project gets a lock-aware release request from an upstream tool alone, which is the gap the engine fills. When the engine releases, and which step makes the tag, is the engine's own decision; this survey decides neither.

## PyPI and PEP 440, on prerelease spelling, trusted publishing, and attestations

Verified: 2026-11-12

PEP 440 is now "a historical document. The up-to-date, canonical spec, Version specifiers, is maintained on the PyPA specs page." Its canonical scheme is `[N!]N(.N)*[{a|b|rc}N][.postN][.devN]`, so `1.4.3rc2` is the canonical spelling. Under "Pre-release separators", "The normal form for this is without a separator", and "a separator … between the pre-release signifier and the numeral" is also allowed, as in `1.0a.1` normalized to `1.0a1`. By those two rules a SemVer-style `1.4.3-rc.2` normalizes to `1.4.3rc2`; this applies the stated rules and is not a worked example in the PEP. "Pre-release spelling" maps `c`, `pre`, and `preview` to `rc`. Under case normalization, `1.1RC1` normalizes to `1.1rc1`.

PyPI's trusted-publisher pages cover GitHub Actions, Google Cloud, ActiveState, and GitLab CI/CD. A minted token "is only valid for 15 minutes from time of creation". For GitLab, the documented job requests `id_tokens: PYPI_ID_TOKEN: aud: pypi`, and "Twine is called to upload the package with no token specified".

PyPI accepts attestations signed by the Trusted Publisher identities "GitHub Actions, GitLab CI/CD, Google Cloud". It takes at most two per file, one SLSA Provenance and one PyPI Publish. On GitHub, `pypa/gh-action-pypi-publish` generates and uploads them "automatically by default". On GitLab, the documented client is `pypi-attestations`. A job with `id_tokens: SIGSTORE_ID_TOKEN: aud: sigstore` runs `python -m pypi_attestations sign python_pkg/dist/*`, and the publish job runs `twine upload --attestations python_pkg/dist/*`. So `pypi-attestations sign` with `twine upload --attestations` is documented. The page names no uv signing path. uv documents only uploading matching attestations.

- https://peps.python.org/pep-0440/
- https://docs.pypi.org/trusted-publishers/
- https://docs.pypi.org/trusted-publishers/using-a-publisher/
- https://docs.pypi.org/attestations/
- https://docs.pypi.org/attestations/producing-attestations/

Bearing: the engine spells Python prereleases `X.Y.ZrcN` and maps SemVer `-rc.N` to that form. It publishes through trusted publishing on both forges. On GitLab, attestations need a separate `pypi-attestations sign` job before the upload, because neither `uv publish` nor GitLab signs them.

## PyPI from GitLab CI/CD, on the upload and its PEP 740 attestations

Verified: 2026-11-12

PyPI's page on adding a trusted publisher states for GitLab CI/CD: "Currently, only projects hosted on https://gitlab.com are supported. Self-managed instances are not supported." The form takes the namespace, the project name, the file path of the top-level pipeline definition, and an optional environment.

PyPI's page on producing attestations documents GitLab CI/CD in three jobs. A `generate-pypi-attestations` job requests `id_tokens: SIGSTORE_ID_TOKEN: aud: sigstore`, installs `pypi-attestations`, and runs `python -m pypi_attestations sign python_pkg/dist/*`. The publish job requests `PYPI_ID_TOKEN` with audience `pypi`, or `testpypi` for TestPyPI, and runs `twine upload --attestations python_pkg/dist/*`. The page names no uv path.

uv's CLI reference states that `uv publish` "Selects only wheels and source distributions and their attestations, while ignoring other files", and that `--no-attestations` turns off what it does "By default", which is to "upload matching PEP 740 attestations with each distribution that is published". Its trusted-publishing environments "include GitHub Actions and GitLab CI/CD". `--check-url` makes a retry skip a file the index already holds with the same hash. uv's packaging guide states: "uv publish does not currently generate attestations; attestations must be created separately before publishing", and shows a `dist/` in which `hello_world-1.0.0.tar.gz.publish.attestation` sits beside `hello_world-1.0.0.tar.gz`. In uv's source, trusted publishing asks `https://pypi.org/_/oidc/audience` for the audience and takes the token through the `ambient-id` crate. That crate's GitLab detector, read at version 0.0.11, runs where `GITLAB_CI` is `true` and reads the variable `<AUDIENCE>_ID_TOKEN`, so the audience `pypi` reads `PYPI_ID_TOKEN`.

`pypi-attestations` 0.0.31 was published on 2026-11-02. Its README shows `pypi-attestations sign` writing `<file>.publish.attestation` beside each input, and states that the CLI "is intended primarily for experimentation, and is not considered a stable interface for generating or verifying attestations".

- https://docs.pypi.org/trusted-publishers/adding-a-publisher/
- https://docs.pypi.org/trusted-publishers/using-a-publisher/
- https://docs.pypi.org/attestations/producing-attestations/
- https://docs.astral.sh/uv/reference/cli/#uv-publish
- https://docs.astral.sh/uv/guides/package/
- https://github.com/astral-sh/uv/blob/main/crates/uv-publish/src/trusted_publishing.rs
- https://docs.rs/ambient-id/0.0.11/src/ambient_id/gitlab.rs.html
- https://github.com/pypi/pypi-attestations
- https://pypi.org/project/pypi-attestations/0.0.31/

Bearing: the documented signer is `pypi-attestations sign` under a Sigstore ID token, and uv documents uploading the `.publish.attestation` files it finds beside each distribution, so `uv publish` takes the place of `twine upload --attestations`. The GitLab `pypi` job requests both ID tokens, signs the sdist and the wheel with `pypi-attestations` at its pin, and runs `uv publish --trusted-publishing always --check-url https://pypi.org/simple/`. It signs in the upload job rather than in a job of its own. PyPI's GitLab publisher names the project and the pipeline file, not a job, so a reading of the form finds no reason for a separate job. The job runs on GitLab.com alone and states the limitation elsewhere. Pending: a live run against TestPyPI from a GitLab.com project has not taken place, so what PyPI shows for these attestations is not yet observed.

## GitHub REST, on draft releases, the tag a publish creates, and the request a commit merges

Verified: 2026-11-12

The create-a-release endpoint documents `draft` as "`true` to create a draft (unpublished) release, `false` to create a published one", and `target_commitish` as "Specifies the commitish value that determines where the Git tag is created from. Can be any branch or commit SHA. Unused if the Git tag already exists." The update endpoint documents `draft` as "`true` makes the release a draft, and `false` publishes the release", and `make_latest` with "Drafts and prereleases cannot be set as latest." The get-by-tag endpoint reads "Get a published release with the specified tag", so a draft answers 404 there and appears only in the release list, which "Only users with push access will receive listings for draft releases". The REST pages read here do not state in words that a draft creates no tag and that its publish does. The `gh release create` help text states the sequence: "When you attach assets, `gh release create` creates a draft release, uploads each asset, and then publishes the release. These steps use separate API requests and do not form a single atomic operation." The immutable-releases article recommends the same order, "Create the release as a draft", "Attach all associated assets to the draft release", "Publish the draft release", after which "its associated Git tag is locked to a specific commit".

The list-pull-requests-associated-with-a-commit endpoint, `GET /repos/{owner}/{repo}/commits/{commit_sha}/pulls`, "Lists the merged pull request that introduced the commit to the repository". It answers `pull-request-simple` objects, whose schema in the published OpenAPI description carries `merged_at`, `head.ref`, `base.ref`, and `merge_commit_sha`. The fine-grained permission table lists that endpoint under "Pull requests" at `read`, and lists `POST /repos/{owner}/{repo}/releases` and `PATCH /repos/{owner}/{repo}/releases/{release_id}` under "Contents" at `write`.

- https://docs.github.com/en/rest/releases/releases
- https://docs.github.com/en/rest/commits/commits#list-pull-requests-associated-with-a-commit
- https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens
- https://github.com/github/rest-api-description/blob/main/descriptions/api.github.com/api.github.com.json
- https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
- https://github.com/cli/cli/blob/trunk/pkg/cmd/release/create/create.go

Bearing: the publish creates a draft at the commit, uploads every asset, and publishes the draft with `target_commitish` set to the commit, so the tag is created last. Should a draft ever create its tag, the tag names this commit and the guard resumes it. The build establishes a release trigger from the commit's merged request, keeping one whose `head.ref` is `rk-release/<base>` and whose `merge_commit_sha` is this commit, so the release job's App token carries `pull-requests: read` beside `contents: write`.

## GitLab REST, on the merge request a commit introduced

Verified: 2026-11-12

`GET /projects/:id/repository/commits/:sha/merge_requests` "Returns information about the merge request that originally introduced a specific commit." Its example answer carries `state`, `source_branch`, `target_branch`, `sha`, `merge_commit_sha`, and `squash_commit_sha`, the last two `null` in the example.

- https://docs.gitlab.com/api/commits/

Bearing: the build keeps a merged request whose `source_branch` is `rk-release/<base>` and whose `merge_commit_sha` or `squash_commit_sha` is this commit. A fast-forward merge that squashes nothing writes neither field, so the build also accepts a request whose `merge_commit_sha` is `null` and whose `sha`, the head the forge merged, is this commit. That case is a reading of the fields, not documented behavior.

## glab, git-cliff, and the Debian image, on the pins the GitLab rk jobs install

Verified: 2026-11-12

glab's latest release is `v1.130.0`, released 2026-11-10, as the GitLab releases permalink of `gitlab-org/cli` answers. The release links `checksums.txt`, which names `ce06313b2075af25180cd195ab3496d42bc5857c85a1c12ba996838dfe9d822c` for `glab_1.130.0_linux_amd64.tar.gz`. The archive downloaded from the release's asset link hashes to the same value, and it carries the binary at `bin/glab`.

glab's configuration page lists `GITLAB_HOST` as the "Default GitLab hostname to use", and states that "A host from an environment variable, such as `GITLAB_HOST`, always wins". For `GITLAB_TOKEN` it states: "A token set through an environment variable is sent only to the host named by `GITLAB_HOST` (or the `host` configuration key) or `GITLAB_REPO`". `GLAB_ENABLE_CI_AUTOLOGIN` is `false` by default, and when it is set glab signs in with `CI_JOB_TOKEN`.

git-cliff `v2.14.2` publishes `git-cliff-2.14.2-x86_64-unknown-linux-musl.tar.gz` with a `.sha512` file beside it. The downloaded archive matches that SHA-512, and its SHA-256 is `fa21e2fc70046fcfd0e43e5e4613873ab799aff3d9dc8559c00d31b270e417d0`. The archive carries the binary at `git-cliff-2.14.2/git-cliff`.

On Docker Hub, the tags `debian:13.7-slim`, `debian:13-slim`, and `debian:trixie-slim` name the image index `sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2`, as the registry's `Docker-Content-Digest` header answers. `13.7-slim` is the newest point-release slim tag. The tag list answers an object whose `results` carry each tag's `name`, and a `next` link while more pages remain. Filtered by `name=.`, the list holds only the numbered tags, 149 of them on 2026-11-12, in two pages of 100. Unfiltered, the slim tags alone count 1319, and Docker Hub refuses an anonymous read past page 10.

- https://gitlab.com/gitlab-org/cli/-/releases/v1.130.0
- https://gitlab.com/api/v4/projects/gitlab-org%2Fcli/releases/permalink/latest
- https://gitlab.com/gitlab-org/cli/-/blob/v1.130.0/docs/source/configuration.md
- https://github.com/orhun/git-cliff/releases/tag/v2.14.2
- https://hub.docker.com/_/debian
- https://hub.docker.com/v2/namespaces/library/repositories/debian/tags?page_size=100&name=.

Bearing: the GitLab rk jobs pin glab and git-cliff by version and SHA-256, and check each archive before they unpack it. They set `GITLAB_HOST` to the instance, because glab sends `GITLAB_TOKEN` to no other host, and they leave CI auto-login off, because it would sign in with the job token. They run on the Debian image pinned by digest, because the `rk` archive targets glibc. `rk versions --check` reads every page of the numbered tags and takes the greatest.

## Cargo, on the include and exclude fields

Verified: 2026-11-12

The Cargo book's manifest reference states that `exclude` and `include` "can be used to explicitly specify which files are included when packaging a project to be published". The patterns "should be gitignore-style patterns":

- `foo` matches a file or a directory of that name anywhere in the package, the same as `**/foo`, and `/foo` matches it only at the root.
- `foo/` matches a directory, `*` and `?` match within one segment, and `[]` matches a range of characters.
- `**/` matches in any directory, `/**` matches everything inside, and `/**/` matches zero or more directories.
- A `!` prefix negates a pattern.

"The options are mutually exclusive; setting `include` will override an `exclude`. If you need to have exclusions to a set of `include` files, use the `!` operator." With neither field, the package holds every file from its root, without the files gitignore rules ignore. `Cargo.toml` "is always included, it does not need to be listed in `include`". The reference names `cargo package --list` as the way to verify which files a package holds. Cargo builds both lists with the gitignore matcher of the `ignore` crate, `GitignoreBuilder`, in `src/sources/path.rs`, and judges each file and every directory above it.

- https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields
- https://doc.rust-lang.org/cargo/commands/cargo-package.html
- https://github.com/rust-lang/cargo/blob/master/src/sources/path.rs
- https://docs.rs/ignore/latest/ignore/gitignore/struct.GitignoreBuilder.html

Bearing: `release-engine:the-bash-release-packs-its-declaration` takes these semantics for `profile.release.build.include` and `exclude`, through the same matcher. `VERSION` stands where Cargo's `Cargo.toml` stands, and the files `rk` lands stay out unless `include` names one, which is the one addition.

## release-plz, git-cliff, uv, and git archive, on the files a release packs

Verified: 2026-11-12

release-plz `release-plz-v0.4.3` decides per package in `crates/release_plz_core/src/command/update/updater.rs`, `get_package_diff`. It walks the commits since the release. At each one, `are_packages_equal_cached` (lines 766 to 799) compares the packaged files with the released package, and an equal snapshot ends the walk on that lineage. It counts a commit only where `are_changed_files_in_package` returns `!package_files.is_disjoint(&changed_files)` (lines 800 to 811 and 950 to 970). The comment there reads "only count commits that touch files Cargo would package for this package." The package list is `cargo package --list` (`get_cargo_package_files`, `package_compare.rs` line 147), and the comparison skips generated files and `Cargo.lock` (lines 103 to 132). The next version comes from the messages of the counted commits alone. The usage page states only that release-plz downloads the published packages and compares them with the local ones.

Measured in throwaway repositories with git-cliff 2.14.2, uv 0.13.7, and the landed `cliff.toml`, one commit after `v0.1.0`:

| Commit                                                               | `--bumped-version`           |
| -------------------------------------------------------------------- | ---------------------------- |
| `build:`, `ci:`, `docs:`, `chore(deps):`, `test:`, `style:`, `feat:` | `v0.1.1`                     |
| `feat!:`                                                             | `v0.2.0`                     |
| none, or `chore(release):` alone                                     | `v0.1.0`, nothing to release |

- With the packed files `src/tool.sh`, `README.md`, and `VERSION` as `--include-path`, a `fix:` on `.gitlab-ci.yml`, a `docs:` on `docs/`, and a `build:` on `Makefile` give `v0.1.0`. Without the paths they give `v0.1.1`. A `fix:` on `src/tool.sh` gives `v0.1.1`.
- `--unreleased --tag v0.1.1` with the same paths writes the packed commits alone.
- A path filter can hide a release. After `v0.1.1` sits on a `chore(release)` commit that changes `VERSION` alone, a `fix:` on a packed file gives `v0.1.1` with the packed files as paths, and `v0.1.2` with `VERSION` added or with no path. git-cliff filters the commits by path before it matches the tags to them (`git-cliff/src/lib.rs` lines 332 to 370), so a tag on a filtered commit bounds no release. `--unreleased --context` with the same paths still names `v0.1.1` as the previous release. A range `v0.1.1..HEAD` gives `v0.1.2` too, but over a past range git-cliff took the newest tag of the repository as the base.
- After a commit that reverts both packed commits, git-cliff still gives `v0.1.1`, while `git diff --quiet v0.1.0 HEAD -- <paths>` finds no change.
- With no tag and a `0.0.0` baseline, git-cliff proposes the `initial_tag`, `v0.1.0`, whatever the paths.
- With 5000 tracked files passed as 10000 `--include-path` arguments, 203893 bytes against an `ARG_MAX` of 2097152, `--bumped-version` answered `v0.1.1` in 0.36 s and the changelog wrote in 0.34 s. The same paths took `git diff --quiet` 0.17 s.
- `uv build --list` names each packed file with its source path, but only with the `uv_build` backend. With hatchling it fails with "Can only use `--list` with a compatible uv build backend", and it writes `dist/` anyway.
- `uv build --sdist --out-dir <dir>` works with every backend and took 0.37 s on a fresh hatchling project. Its members sit under `<name>-<version>/`, and `PKG-INFO`, and `pyproject.toml.orig` under `uv_build`, are generated.
- The default hatchling sdist packs every tracked file, `.gitlab-ci.yml`, `.github/workflows/release.yml`, `.release-kit/config.toml`, `cliff.toml`, `uv.lock`, and `.python-version` included. The default `uv_build` sdist packs `pyproject.toml`, `README.md`, and `src/`.
- A `git archive` tarball packs every tracked file without the `export-ignore` attribute, under one top directory.

- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/command/update/updater.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/package_compare.rs
- https://release-plz.dev/docs/usage/update
- https://github.com/orhun/git-cliff/blob/v2.14.2/git-cliff/src/lib.rs
- https://git-scm.com/docs/git-archive

Bearing: the engine opens a request only where a packed file differs from the release tag, and passes the packed files to git-cliff as paths, with the version file, which every release commit changes, so the last release still bounds the history. The path filter alone misses a revert, so the equality check stands beside it. A hatchling project packs the files `rk` lands, so the engine drops them, and an operator keeps any other python file out through the backend's own exclude. A bash release no longer builds through `git archive`, so `export-ignore` plays no part in it.

## release-plz, on the packages it releases, the lock check, and `release_commits`

Verified: 2026-11-12

Read at tag `release-plz-v0.4.3`. Paths are below `crates/`.

- Configuration files. Without `--config`, release-plz reads the first of `release-plz.toml`, `.release-plz.toml`, and `.config/release-plz.toml` (`release_plz/src/args/config_path.rs` lines 14 to 18).
- The `release` key. `Project::new` keeps a workspace member only where `get_release_metadata` answers (`release_plz_core/src/project.rs` lines 64 to 76), and that answer exists only where the package's `release` is true (`release_plz_core/src/next_ver.rs` lines 55 to 63). An unset `release` is true: `release: config.release != Some(false)` (`release_plz/src/config.rs` line 502). A `[[package]]` entry's key overrides the `[workspace]` key of the same name (`PackageConfig::merge`, lines 524 to 560).
- Cargo's `publish` field and `git_only`. A package "can be published" where `publish` is unset and the package is not example-only, or where it names at least one registry: `publish = false` and `publish = []` read as an empty list (`next_ver.rs` lines 449 to 465). A member takes part where it can be published, or where it is `git_only` and is not an example-only package that leaves `publish` unset (`takes_part_in_release`, lines 467 to 485). `packages_to_process` filters the members by that rule (`release_plz_core/src/command/update/updater.rs` lines 376 to 382), and `git_only` is read per package (`release_plz_core/src/command/update/update_request.rs` lines 258 to 261).
- release-plz's own `publish` key selects nothing. It only decides whether `cargo publish` runs: `config.publish.enabled && !config.git_only` (`release_plz_core/src/command/release.rs` lines 154 to 157). `git_only = true` with `publish = true` is refused as mutually exclusive (`release_plz/src/config.rs` lines 164 to 172).
- The lock. For a package with a binary target, release-plz adds the synthetic commit `chore: update Cargo.lock dependencies` to the package's diff where the lock changed by meaning (`updater.rs` lines 864 to 876, `contains_executable` at line 1022). The meaning counts only a version change of a dependency the released lock already had (`release_plz_core/src/lock_compare.rs` lines 72 to 74 and 279 to 300). The real commit that changed `Cargo.lock` is not in the diff, because the package comparison skips the lock.
- `release_commits`. The regex filters a package whose diff holds no matching commit message (`updater.rs` lines 120 to 127, `release_plz_core/src/diff.rs` lines 117 to 120). A lock-only change carries only the synthetic `chore:` message there.

- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz/src/args/config_path.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/project.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/next_ver.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz/src/config.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/command/update/updater.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/command/update/update_request.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/command/release.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/lock_compare.rs
- https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/diff.rs

Bearing: `release-engine:a-rust-target-releases-one-package` selects the member release-plz releases by the same three facts, so an `xtask` helper with `publish = false` beside a registry crate is skipped and a single `git_only` crate with `publish = false` is judged. A `release_commits` regex would read only the synthetic `chore:` line for a lock-only security update and never release it, so rk reads the real commit from git and release-plz takes no such key.
