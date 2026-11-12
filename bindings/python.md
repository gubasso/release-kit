# Python binding

| Axis                | Answer                                                                   |
| ------------------- | ------------------------------------------------------------------------ |
| Version file        | `pyproject.toml` `[project] version` and `uv.lock`, through `uv version` |
| Release-request bot | the rk engine, with git-cliff                                            |
| Registry and auth   | PyPI, trusted publishing over OIDC, or none                              |
| Artifact builder    | `uv build`, the sdist and the wheel                                      |

`rk init --technology python --release-driver python` lands `cliff.toml` and the release pipeline: `.github/workflows/release.yml` on GitHub, `.gitlab-ci.yml` on GitLab.

## The shape

The binding serves a uv project with a static version. `[project] version` in `pyproject.toml` is the committed source of truth, and `uv.lock` moves with it. A landing refuses before any write where the version is dynamic or absent, with a static `version` in `[project]` as the remedy, and where the root carries no `uv.lock`, with `uv lock` as the remedy.

The `rk` binary runs the release itself, as `rk release request`, `rk release build`, and `rk release publish`, the same engine the bash binding runs. `_docs/decisions/ADR-run-the-release-through-the-rk-engine.md` records why. The engine asks git-cliff for the next version, which git-cliff computes from Conventional Commits, and writes the changelog through git-cliff, at the version `versions.toml` pins. It writes every version through `uv version <version> --no-sync`, which relocks, so a release request and a candidate cut each carry `pyproject.toml` and `uv.lock` together, and `uv sync --locked` passes on both.

`profile.release.registry` answers where the release publishes: `pypi`, which an absent answer reads as, or `none`. Under `none` the tag and the release page with its attested distributions are the release, and the pipeline requests no registry credential.

## Versions and tags

A PEP 440 version maps to the semver-shaped tag every pair uses, per `release-engine:the-python-version-maps-to-the-tag`. `1.4.3` tags `v1.4.3`, and the candidate `1.4.3rc2` tags `v1.4.3-rc.2`. `rk lines mint <line> --apply` writes a candidate such as `1.4.3rc1` through `uv version`, and `rk lines mint <line> --promote` writes the stable `1.4.3` the same way. A candidate commit that changes only `pyproject.toml` and the project's own entry in `uv.lock` is a version change, so it never blocks a promotion.

## The workflows

Each pipeline runs `rk` in every release job and holds no release logic of its own. `rk` reads the trunk, the style, the integration, the line prefix, and the registry answer from the landing record.

- `release-request` runs `rk release request` on every push to the trunk and to a release line. It keeps one request per base on the branch `rk-release/<base>`, refreshes it in place, closes it when nothing remains to release, and arms it under the trunk style on the trunk. On GitHub under local integration the rendered `release-gate` job merges it instead.
- The build releases a final version only at the commit that merges the engine's own request, and a candidate only at the commit that writes it. It runs `uv build` into `dist/`, which then holds the sdist and the wheel alone. Where PyPI already serves a file of the version, the build compares the rebuild with the SHA-256 PyPI serves and stops on a difference before anything uploads.
- On GitHub, the `release` job runs `rk release build`, `actions/attest` over the sdist and the wheel, the PyPI upload, and `rk release publish`. The upload is `pypa/gh-action-pypi-publish` with `packages-dir: dist/` and `skip-existing: true`. Under `pypi` the job runs in the `pypi` environment.
- On GitLab, `build` runs `rk release build`, `provenance` signs the runner's statement over each distribution, `pypi` uploads them, and `publish` runs `rk release publish` after all three. The `pypi` job signs each distribution's PEP 740 attestation with `pypi-attestations sign` under a Sigstore ID token, then runs `uv publish --trusted-publishing always --check-url https://pypi.org/simple/` under a PyPI ID token, and `uv publish` uploads each attestation beside its file. Under `none` the pipeline carries no `pypi` job.
- `release-candidate` mints a candidate on a release line, on purpose: on GitHub a `workflow_dispatch` of `release.yml` with the inputs `line`, `expect`, and `promote`, and on GitLab a manual job on a pipeline of the line with the variables `LINE`, `EXPECT`, and `PROMOTE`. It runs `rk lines mint <line> --expect <sha> --apply`, which writes the candidate's version through `uv version`, commits `pyproject.toml` and `uv.lock` together, and pushes the line with the bot's token. That push releases the candidate, so the job itself publishes nothing and creates no tag. A candidate the line already carries with no tag yet reports `pending`, and the job writes and pushes nothing.
- On GitLab the job adds `--as-bot`, so `rk` reads the account the bot's token names and commits as it, with the email the account returns or its username's noreply address on the instance, as the release request does. Where the forge names no account, the job stops before it writes a file.
- On GitHub the candidate's page is a prerelease that is never marked latest. On GitLab the page carries no prerelease badge, so the first line of its description names the candidate, and the latest-release permalink follows the candidate until the next stable release.
- With `promote` set, the job runs `rk lines mint <line> --promote --expect <sha> --apply`, which writes the stable version of the newest candidate. That push releases nothing. The line's release request proposes the version with its changelog, and its merge releases it.
- Every GitLab release job holds a resource group keyed by its branch, so a release on one line never waits on another line's.
- Where the answer is `pypi`, the release job that the mint push starts uploads the candidate, such as `1.4.3rc2`, to PyPI, so the trusted publisher stays the one that names `release.yml`. A validator installs it with `pip install acme==1.4.3rc2`, with no `--pre`, because pip admits a prerelease that the specifier names. `pip install acme` still takes the stable version.
- Every job authenticates as the bot: on GitHub the release App's token reaches `rk` as `GH_TOKEN`, and on GitLab the job sets `GITLAB_TOKEN` from `RELEASE_BOT_TOKEN`. `rk` refuses the default CI token.
- One uv serves the project and its jobs: the exact `required-version` the project names, else the version `versions.toml` pins. Where `uv.toml` exists, it alone names that version, because uv then ignores `[tool.uv]` of `pyproject.toml`. Where it does not exist, `[tool.uv]` names it. On GitHub `astral-sh/setup-uv` installs it, and takes a version input only where the project names no exact version. On GitLab the jobs run on a Debian image pinned by digest that carries no uv, and install that version through the uv installer, which checks the archive it downloads. Where the version is the pinned one, the job also checks the installer against the SHA-256 `versions.toml` records.
- Where the target records no development environment, each job installs `rk` through the shell installer of the release that landed the pipeline, git-cliff at its pin, and uv. Where it records one, each job takes the recorded setup on GitHub or the recorded image and setup lines on GitLab, and runs `rk` and uv through the recorded prefix. The environment must then carry `rk`, git-cliff at its pin, and the uv the project requires. The GitLab jobs fetch glab and every other tool into `rk-tools/bin` under the temporary directory, as the bash binding states.

## Setup specifics

- The trusted publisher is the registry form. On GitHub it names the owner, the repository, the workflow `release.yml`, and the environment `pypi`. On GitLab it names the namespace and the project, and the top-level pipeline file `.gitlab-ci.yml`. A brand-new project name registers as a pending publisher before the first upload, which needs no token. Where the name is already claimed, publish once with a project-scoped API token and revoke it after the trusted publisher is registered.
- PyPI accepts a GitLab trusted publisher for a project on GitLab.com alone. On a self-managed instance the `pypi` job states that limitation and uploads nothing, and the distributions are uploaded by hand as the recovery section states.
- Step 0 is `uv build`, which `rk setup step package-check` runs and whose sdist and wheel it names. It does not prove that the distributions carry `SECURITY.md`. PEP 517 leaves the file set to the build backend, and an sdist and a wheel can carry different files. Inspect each before the first release: `tar -tzf dist/<name>-<version>.tar.gz` and `unzip -l dist/<name>-<version>-*.whl`, and add the policy through the backend's own include configuration where it is missing.
- Under `none` there is no trusted publisher, no bootstrap token, and no packaging gate.

## Provenance

- The release page carries the provenance of the forge: on GitHub an `actions/attest` attestation per distribution, and on GitLab a cosign bundle per distribution beside it, keyless and on GitLab.com alone, as the bash binding states for its tarball.
- PyPI carries PEP 740 attestations beside each file it serves. On GitHub the upload action generates them over trusted publishing. On GitLab the `pypi` job signs them with `pypi-attestations` and `uv publish` uploads them.
- The release verify step proves them with `pypi-attestations verify pypi --repository <the forge URL of the repository> pypi:<distribution filename>`, once per sdist and wheel. Obtain the tool reproducibly at its `versions.toml` pin, `uvx --from pypi-attestations==<version> pypi-attestations`.
- No installer checks it today: `pip install` does not verify attestations. The attestation is published evidence, not an install-time gate.

## Recovery specifics

- Withdraw by yanking the release on PyPI. A yanked version stops resolving for new installs while pinned installs keep working.
- A failed upload or a failed page reruns the release job on GitHub, or `pypi` and `publish` on GitLab, at the same commit. The rebuild must equal what PyPI already serves, the upload skips the files PyPI holds, and `rk release publish` finishes the page.
- The hand-publish path is `uv build`, then `uv publish` with a token. Where the project enforces trusted publishing or the environment gates the upload, loosen that switch first, publish, revoke, and re-tighten.
