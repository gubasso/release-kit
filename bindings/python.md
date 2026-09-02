# Python binding

| Axis                | Answer                                                |
| ------------------- | ----------------------------------------------------- |
| Version file        | `pyproject.toml`, `[project] version`                 |
| Release-request bot | release-please                                        |
| Registry and auth   | PyPI, trusted publishing over OIDC                    |
| Artifact builder    | none separate; the wheels and sdist are the artifacts |

`rk init --tech python` lands `release-please-config.json`, `.release-please-manifest.json`, and the publish workflow `.github/workflows/release-please.yml`.

The release-request answer holds on GitHub only: release-please speaks the GitHub API and no other forge's, so `(python, gitlab)` has no release-request bot and `rk init` lands nothing for that pair — stated here as a smaller product rather than smoothed over, the same shape as the Go column's missing bot.

## The workflows

`release-please.yml` is the publish workflow: the filename registered at PyPI, and the only workflow with `id-token: write`. release-please maintains the release request with the `python` release type, which bumps `[project] version` in `pyproject.toml`; the publish job builds the distributions with `python -m build` and uploads them through `pypa/gh-action-pypi-publish`.

The publish job runs in a GitHub environment, because PyPI's trusted-publisher registration takes an optional environment name and environment protection rules are the PyPI-side hardening lever. The registration is owner, repository, workflow filename, and that environment.

## Setup specifics

- Step 0 is `python -m build` plus `twine check dist/*`, which catches metadata rejects without credentials.
- The bootstrap publish is `twine upload` with a project-scoped API token, revoked after the trusted publisher is registered. A brand-new project name can also be registered on PyPI as a pending publisher before the first upload, which skips the token entirely; take that path when the name is not yet claimed.
- PyPI trusted publishing is the direct analogue of crates.io's, so the setup sequence maps one to one.

## Tool boundary

release-please knows the `python` release type and deterministically bumps `pyproject.toml`, which is what qualifies it under [the invariants](../method/01-invariants.md). python-semantic-release is a push model that versions and publishes on the push itself, with no release-request gate, so it does not fit this convention.

## Provenance

- PyPI carries it, by default and with no configuration to write. From 1.11.0 onward `pypa/gh-action-pypi-publish` generates and uploads PEP 740 attestations for every project publishing over trusted publishing, so the pinned action already does this and the binding adds nothing.
- The attestation is Sigstore-signed and the index serves it beside the distribution, so a consumer verifies against PyPI itself rather than against the forge that built the file.
- The release verify step proves it with `pypi-attestations verify pypi --repository https://github.com/<owner>/<repo> pypi:<distribution filename>`, once per sdist and wheel; obtain the tool reproducibly at its `versions.toml` pin, `pipx install pypi-attestations==<version>`.
- No installer checks it today: `pip install` does not verify attestations. The attestation is published evidence, not an install-time gate.

## Recovery specifics

- Withdraw by yanking the release on PyPI; a yanked version stops resolving for new installs while pinned installs keep working.
- The hand-publish path is `twine upload` with a token; where the project enforces trusted publishing or the environment gates the upload, loosen that switch first, publish, revoke, re-tighten.
