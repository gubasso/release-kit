# A release tool for a uv project and the alternatives

Every verdict below is a scenario that was run, not a feature claim. A capability name links to the scenario. A verdict links to what that tool did when the scenario was run against it. A behavior that needs a live forge was not run here: its cell reads untested and links the source that states it.

Legend: ✅ yes, works on the default path. ⚠️ partial, needs configuration or has a stated limit. ❌ no, was run and failed. ➖ n/a, meaningless for this subject. 🧪 unstable, experimental path. ❓ untested, not yet run.

## The version file and the lock

This table settles which tool bumps `pyproject.toml` and leaves `uv.lock` consistent with it, so `uv sync --locked` passes on the commit it writes.

Verified: 2026-11-12 against `uv version` 0.13.7, commitizen 4.19.3, python-semantic-release 10.7.0, release-please 17.11.2.

| capability                                                 | `uv version`                     | commitizen                       | python-semantic-release                          | release-please                            |
| ---------------------------------------------------------- | -------------------------------- | -------------------------------- | ------------------------------------------------ | ----------------------------------------- |
| [The bumped commit passes `uv sync --locked`](#scenario-a) | [✅ yes](#scenario-a-uv-version) | [✅ yes](#scenario-a-commitizen) | [⚠️ partial](#scenario-a-python-semantic-release) | [❓ untested](#scenario-a-release-please) |

## The next version

This table settles which tool computes the next version from Conventional Commits the way release-kit's convention does, below and above `1.0.0`.

Verified: 2026-11-12 against git-cliff 2.14.2, commitizen 4.19.3, python-semantic-release 10.7.0.

| capability                                            | git-cliff                       | commitizen                          | python-semantic-release                          |
| ----------------------------------------------------- | ------------------------------- | ----------------------------------- | ------------------------------------------------ |
| [A version above `1.0.0` follows semver](#scenario-b) | [✅ yes](#scenario-b-git-cliff) | [✅ yes](#scenario-b-commitizen)    | [✅ yes](#scenario-b-python-semantic-release)    |
| [A version below `1.0.0` stays below it](#scenario-b) | [✅ yes](#scenario-b-git-cliff) | [⚠️ partial](#scenario-b-commitizen) | [⚠️ partial](#scenario-b-python-semantic-release) |

## The release model

This table settles which tool keeps one release request, as `ADR-gate-the-release-on-one-pull-request.md` requires, and on which forge.

Verified: 2026-11-12 against release-please 17.11.2, releaser-pleaser 0.9.0, python-semantic-release 10.7.0, knope 0.23.2, release-plz 0.4.3.

| capability                               | release-please                                                                                                   | releaser-pleaser                                                                           | python-semantic-release                                                                                                                                        | knope                                                                                                        | release-plz                                                                                                                                                    |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Keeps one release request](#scenario-c) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#release-please-on-the-python-strategy-and-uvlock) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#releaser-pleaser-on-gitlab) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#python-semantic-release-commitizen-and-semantic-release-on-the-release-model-and-the-uv-relock) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#knope-and-cocogitto-on-the-forges-they-reach) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#release-plz-on-the-request-refresh-the-release-rule-the-lock-update-and-git-cliff-as-a-library) |
| [Reaches GitLab](#scenario-c)            | [❓ untested](../reference/REFERENCE-release-engine-sources.md#release-please-on-the-python-strategy-and-uvlock) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#releaser-pleaser-on-gitlab) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#python-semantic-release-commitizen-and-semantic-release-on-the-release-model-and-the-uv-relock) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#knope-and-cocogitto-on-the-forges-they-reach) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#release-plz-on-the-request-refresh-the-release-rule-the-lock-update-and-git-cliff-as-a-library) |

## The release model, the remaining tools

This table completes the release model for the surveyed tools that the table above leaves out: two that release on push, and two that keep no request because they are not release tools.

Verified: 2026-11-12 against commitizen 4.19.3, semantic-release 25, `uv version` 0.13.7, git-cliff 2.14.2.

| capability                               | commitizen                                                                                                                                                     | semantic-release                                                                                                                                               | `uv version` | git-cliff |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ | --------- |
| [Keeps one release request](#scenario-c) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#python-semantic-release-commitizen-and-semantic-release-on-the-release-model-and-the-uv-relock) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#python-semantic-release-commitizen-and-semantic-release-on-the-release-model-and-the-uv-relock) | ➖ n/a       | ➖ n/a    |
| [Reaches GitLab](#scenario-c)            | [❓ untested](../reference/REFERENCE-release-engine-sources.md#python-semantic-release-commitizen-and-semantic-release-on-the-release-model-and-the-uv-relock) | [❓ untested](../reference/REFERENCE-release-engine-sources.md#python-semantic-release-commitizen-and-semantic-release-on-the-release-model-and-the-uv-relock) | ➖ n/a       | ➖ n/a    |

## The relock without an index

This table settles whether `uv version` can rewrite the lock when the package index cannot be reached, which is the failure mode of a release job whose network fails.

Verified: 2026-11-12 against `uv version` 0.13.7.

| capability                                                   | `uv version`                        |
| ------------------------------------------------------------ | ----------------------------------- |
| [Relocks a project with no registry dependency](#scenario-d) | [✅ yes](#scenario-d-uv-version)    |
| [Relocks a project with a registry dependency](#scenario-d)  | [⚠️ partial](#scenario-d-uv-version) |

## Scenario A

Record whether the commit a tool writes for the next version carries a `uv.lock` that agrees with `pyproject.toml`.

1. Create a uv project with the `uv_build` backend, a static version `0.1.0`, `uv.lock`, and two Conventional Commits, `feat: first` and `fix: a bug`.
2. Let the tool bump the version, with no push.
3. Run `uv sync --locked` on the commit the tool wrote.

```bash
uv sync --locked
```

1. Record the exit code and the project's `version` line in `uv.lock`.

The fixture, in a scratch directory shown as `$SCRATCH`, one copy per subject. It was rebuilt on 2026-11-12 with every command recorded, and scenario A's `uv version` result checked again on it:

```text
$ uv --version
uv 0.13.7 (x86_64-unknown-linux-gnu)
$ uv init --package --build-backend uv --name demo-pkg .     # uv_build, version = "0.1.0"
$ uv lock                                                    # uv.lock: demo-pkg version = "0.1.0"
$ git add -A && git commit -m "feat: first"
$ echo '# a fix' >> src/demo_pkg/__init__.py && git commit -am "fix: a bug"
$ git log --oneline
fix: a bug
feat: first
$ uv version 0.2.0 && uv sync --locked
demo-pkg 0.1.0 => 0.2.0
exit 0
```

The control, a hand edit with no tool:

```text
$ sed -i 's/^version = "0.1.0"/version = "0.2.0"/' pyproject.toml && uv sync --locked
error: The lockfile at `uv.lock` needs to be updated, but `--locked` was provided.
exit 1; uv.lock demo-pkg version = "0.1.0"
```

### Scenario A, `uv version`

uv 0.13.7, run on 2026-11-12. `uv version --bump minor` and `uv version 0.2.0` each wrote `0.2.0` into `pyproject.toml` and relocked, so `uv.lock` read `0.2.0` and `uv sync --locked` exited 0. A hand edit of `pyproject.toml`, as a control, left the lock at `0.1.0`, and `uv sync --locked` exited 1. `uv version` writes no commit and no tag; a caller commits both files.

```text
$ uv version --bump minor
demo-pkg 0.1.0 => 0.2.0
exit 0; pyproject.toml version = "0.2.0"; uv.lock version = "0.2.0"
$ uv sync --locked
exit 0; git status: M pyproject.toml, M uv.lock
$ uv version 0.2.0        # on a fresh copy
demo-pkg 0.1.0 => 0.2.0
exit 0; uv sync --locked exit 0
```

### Scenario A, commitizen

commitizen 4.19.3 with `version_provider = "uv"`, run on 2026-11-12. `cz bump --yes` rewrote `pyproject.toml` and `uv.lock`, committed both, and tagged `v0.2.0`, and `uv sync --locked` exited 0. Its default commit message, `bump: version 0.1.0 → 0.2.0`, is not a Conventional Commit, so a commit-msg gate refuses it until `bump_message` names one.

```toml
[tool.commitizen]
name = "cz_conventional_commits"
version_provider = "uv"
tag_format = "v$version"
```

```text
$ uvx --from commitizen==4.19.3 cz bump --yes
bump: version 0.1.0 → 0.2.0
tag to create: v0.2.0
[master 7828710] bump: version 0.1.0 → 0.2.0
 2 files changed, 2 insertions(+), 2 deletions(-)      # pyproject.toml, uv.lock
exit 0; tag v0.2.0; working tree clean
$ uv sync --locked
exit 0; uv.lock version = "0.2.0"
```

With a commit-msg gate that requires a Conventional Commit, the same command wrote both files and then failed at the commit (exit 6), leaving no commit and no tag.

### Scenario A, python-semantic-release

python-semantic-release 10.7.0, run on 2026-11-12, with `version_toml = ["pyproject.toml:project.version"]` and a remote named `origin`, which the command requires. With no `build_command`, its release commit and tag kept `uv.lock` at the old version, and `uv sync --locked` exited 1. With `build_command` relocking alone, the relock stayed out of the release commit, and the tagged commit still failed. Only the build command of its uv guide, which relocks and runs `git add uv.lock`, put the lock into the release commit, and its default commit message is the bare version, not a Conventional Commit.

```toml
[tool.semantic_release]
version_toml = ["pyproject.toml:project.version"]
# variant 2:
build_command = 'uv lock --upgrade-package "$PACKAGE_NAME"'
# variant 3, the uv guide:
build_command = '''
    uv lock --upgrade-package "$PACKAGE_NAME"
    git add uv.lock
    uv build
'''
```

```text
$ uvx --from python-semantic-release==10.7.0 semantic-release -v version --no-push --no-vcs-release
::ERROR:: cmdline: git remote get-url origin     # no remote: exit 1
# after git remote add origin https://github.com/example/demo-pkg.git (never contacted)
The next version is: 1.0.0!
No build command specified, skipping
exit 0; commit "1.0.0" holds CHANGELOG.md and pyproject.toml; tag v1.0.0
$ uv sync --locked                                # variant 1
exit 1; uv.lock version = "0.1.0"
# variant 2: exit 0; git status M uv.lock; git show v1.0.0:uv.lock reads 0.1.0
$ git archive v1.0.0 | tar -x -C tagged && cd tagged && uv sync --locked
exit 1
# variant 3: the release commit holds uv.lock at 1.0.0; uv sync --locked at the tag exit 0
```

### Scenario A, release-please

release-please 17.11.2, run on 2026-11-12. `release-pr --dry-run --release-type python` refused without `--repo-url`, and with one, even under `--local`, it asked the GitHub API for the repository's default branch and failed with 404. It has no offline path, so the scenario could not run without a real GitHub repository. The [reference file](../reference/REFERENCE-release-engine-sources.md#release-please-on-the-python-strategy-and-uvlock) records that its python updater does not touch `uv.lock`.

```text
$ npx -y release-please@17.11.2 release-pr --dry-run --release-type python
Missing required argument: repo-url                       exit 1
$ npx -y release-please@17.11.2 release-pr --dry-run --release-type python --repo-url=example/demo-pkg --local --local-path=.
RequestError [HttpError]: Not Found - https://docs.github.com/rest/repos/repos#get-a-repository
    at async GitHubApi.defaultBranch
exit 1; pyproject.toml untouched
```

Not run: the python updater on a real repository.

## Scenario B

Record the next version each tool computes, without writing, from these starting states: `0.1.0` tagged `v0.1.0` with two `feat:` commits; `1.0.0` tagged `v1.0.0` with two `feat:` commits; `1.0.0` with a `feat:` and a `feat!:` commit; `0.1.0` with a `feat:` and a `feat!:` commit.

1. Reach each starting state in its own copy of the scenario A project.
2. Ask the tool for the next version.

```bash
git cliff --bumped-version
```

1. Record the version printed, and confirm the tree and the tags did not change.

Each subject ran in its own copy of each state, and every run exited 0 with `git status` clean and the tags unchanged:

| state                             | git-cliff 2.14.2 | commitizen 4.19.3 | commitizen, `major_version_zero = true` | python-semantic-release 10.7.0 | python-semantic-release, `allow_zero_version = true`, `major_on_zero = false` |
| --------------------------------- | ---------------- | ----------------- | --------------------------------------- | ------------------------------ | ----------------------------------------------------------------------------- |
| `0.1.0` and two `feat:`           | `v0.1.1`         | `0.2.0`           | `0.2.0`                                 | `1.0.0`                        | `0.2.0`                                                                       |
| `1.0.0` and two `feat:`           | `v1.1.0`         | `1.1.0`           | ➖ n/a                                  | `1.1.0`                        | ➖ n/a                                                                        |
| `1.0.0`, a `feat:` and a `feat!:` | `v2.0.0`         | `2.0.0`           | ➖ n/a                                  | `2.0.0`                        | ➖ n/a                                                                        |
| `0.1.0`, a `feat:` and a `feat!:` | `v0.2.0`         | `1.0.0`           | `0.2.0`                                 | `1.0.0`                        | `0.2.0`                                                                       |

### Scenario B, git-cliff

git-cliff 2.14.2 with release-kit's landed `cliff.toml`, run on 2026-11-12. It printed `v0.1.1`, `v1.1.0`, `v2.0.0`, and `v0.2.0` for the four states: below `1.0.0` a feature bumps the patch and a breaking change the minor, and above it a feature bumps the minor and a breaking change the major.

```text
$ uvx git-cliff==2.14.2 --bumped-version      # cliff.toml copied unedited from snippets/bash/github/cliff.toml
v0.1.1
```

### Scenario B, commitizen

commitizen 4.19.3, `cz bump --get-next`, run on 2026-11-12. Above `1.0.0` it gave `1.1.0` and `2.0.0`. Below it, a feature gave `0.2.0` and a breaking change `1.0.0`, unless `major_version_zero = true`, which gives `0.2.0` for both; no setting gave a patch for a feature below `1.0.0`.

```text
$ uvx --from commitizen==4.19.3 cz bump --get-next
0.2.0
```

### Scenario B, python-semantic-release

python-semantic-release 10.7.0, `semantic-release version --print`, run on 2026-11-12. Above `1.0.0` it gave `1.1.0` and `2.0.0`. Below it, the default `allow_zero_version = false` jumped to `1.0.0` for both states; with `allow_zero_version = true` and `major_on_zero = false`, both gave `0.2.0`.

```text
$ uvx --from python-semantic-release==10.7.0 semantic-release version --print
WARNING Token value is missing!
1.0.0
```

## Scenario C

Record whether a tool keeps one open release request that a merge releases, or releases on every push, and which forges it reaches. This needs a live forge repository, so it was not run here: each cell links the source section that states the behavior.

1. Push a `feat:` commit to the trunk of a repository the tool is configured for.
2. Observe whether a request opens, and whether a second push refreshes it in place.

```bash
git push origin master
```

1. Record the request, or the release, the push produced.

## Scenario D

Record whether `uv version` rewrites the lock when the package index is unreachable, for a project with no dependency and one with a dependency from the index.

1. Take the scenario A project, and a copy with `uv add idna` run while online.
2. Make the index unreachable: once with `UV_INDEX_URL` naming a closed port, once with `UV_OFFLINE=1`, once behind a dead proxy, each with a warm and a cold uv cache.
3. Bump the version.

```bash
uv version 0.2.0 --no-sync
```

1. Record the exit code and the lock's version line.

### Scenario D, `uv version`

uv 0.13.7, run on 2026-11-12. The project with no dependency relocked in every case, with or without `--no-sync`. The project with `idna` relocked only offline from a warm cache: with a cold cache it exited 1 under `UV_OFFLINE=1` and 2 behind the dead proxy, and a changed `UV_INDEX_URL` exited 2 in every case. A failed run wrote nothing, so neither file moved. The relock re-resolves, so it needs the index metadata of every registry dependency, and `--no-sync` does not remove that need.

Each cell is a fresh copy; `base` has no dependency, and `base-idna` ran `uv add idna` while online (idna 3.20 from pypi.org). The network blocks are `UV_INDEX_URL=http://127.0.0.1:9/simple`, `UV_OFFLINE=1`, and a proxy on a closed port.

| project     | network block              | cache        | `uv version 0.2.0`              | `uv version 0.2.0 --no-sync`    |
| ----------- | -------------------------- | ------------ | ------------------------------- | ------------------------------- |
| `base`      | every block                | warm or cold | exit 0, lock `0.2.0`            | exit 0, lock `0.2.0`            |
| `base-idna` | `UV_INDEX_URL` closed port | warm or cold | exit 2, nothing written         | exit 2, nothing written         |
| `base-idna` | `UV_OFFLINE=1`             | warm         | exit 0, lock `0.2.0`, idna 3.20 | exit 0, lock `0.2.0`, idna 3.20 |
| `base-idna` | `UV_OFFLINE=1`             | cold         | exit 1, nothing written         | exit 1, nothing written         |
| `base-idna` | closed proxy               | warm         | exit 0, lock `0.2.0`, idna 3.20 | exit 0, lock `0.2.0`, idna 3.20 |
| `base-idna` | closed proxy               | cold         | exit 2, nothing written         | exit 2, nothing written         |

```text
$ UV_INDEX_URL=http://127.0.0.1:9/simple uv version 0.2.0 --no-sync     # base-idna
error: Request failed after 3 retries
  cause: Failed to fetch: http://127.0.0.1:9/simple/idna/
exit 2
$ UV_OFFLINE=1 uv version 0.2.0 --no-sync                              # base-idna, cold cache
  cause: Because idna was not found in the cache and your project depends on idna>=3.20, we can conclude that your project's requirements are unsatisfiable.
exit 1
$ UV_OFFLINE=1 uv version 0.2.0 --no-sync                              # base-idna, warm cache
demo-pkg 0.1.0 => 0.2.0
exit 0
```

## Re-verification

Re-run every scenario when a pinned version in `versions.toml` moves, and at the cadence `_docs/reference/tracking.yaml` records for release-please's `uv.lock` support, uv's dynamic version, and releaser-pleaser on GitLab. Re-running is what a refresh is; editing version numbers without re-running produces a false date.
