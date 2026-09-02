# Provenance Sources

External sources behind the provenance answers in `bindings/rust.md` and `bindings/bash.md`: what each channel's attestation machinery actually does, where its defaults fall short of the invariant, and what a repository must be for the machinery to work. Each entry states what the source says and what it bears on.

Verified against the listed sources on 2026-09-02.

## cargo-dist, on the attestation phase and its default

The configuration reference documents `github-attestations` (default `false`), `github-attestations-phase` with the values `announce`, `host`, and `build-local-artifacts` (the default), `github-attestations-filters` for the `host` phase (default `["*"]`), and `github-release` with the values `auto`, `host`, and `announce` (default `auto`), which "controls which stage of the release process the GitHub Release will be created in".

In the v0.32.0 templates, the default phase's attest step lives in `templates/ci/github/release.yml.j2` inside the `build-local-artifacts` matrix job with `subject-path: "target/distrib/*${{ join(matrix.targets, ', ') }}*"` — the per-platform archives only. In the `host` phase the attest step moves to `templates/ci/github/partials/publish_github.yml.j2`, after every asset is downloaded and merged into `artifacts/` and before `Create GitHub Release`, globbing `artifacts/{filter}` per filter entry.

- <https://axodotdev.github.io/cargo-dist/book/reference/config.html>
- <https://github.com/axodotdev/cargo-dist/blob/v0.32.0/cargo-dist/templates/ci/github/release.yml.j2>
- <https://github.com/axodotdev/cargo-dist/blob/v0.32.0/cargo-dist/templates/ci/github/partials/publish_github.yml.j2>

Bearing: the `github-attestations-phase = "host"` and `github-release = "host"` pairing in `snippets/rust/github/dist-workspace.toml`, and the regenerated `.github/workflows/release.yml`, whose diff confirms the template behaviour: the attest step lands in the `host` job with `subject-path: artifacts/*`, before `Create GitHub Release`.

## GitHub, on who may mint artifact attestations

Public repositories write attestations to the Sigstore Public Good Instance; private repositories on GitHub Enterprise plans write them to GitHub's own private Sigstore instance. A repository outside both sets has no attestation store to write to.

- <https://docs.github.com/en/actions/concepts/security/artifact-attestations>
- <https://github.blog/news-insights/product-news/introducing-artifact-attestations-now-in-public-beta/>

Bearing: the availability sentence in `bindings/rust.md`, and the choice to treat attestation as the floor for every public release-kit target.

## GitLab, on the runner's provenance statement

Setting `RUNNER_GENERATE_ARTIFACTS_METADATA: "true"` makes the runner emit artifact provenance metadata beside the uploaded artifacts, as an in-toto v0.1 Statement carrying a SLSA 1.0 Provenance predicate (`predicateType: https://slsa.dev/provenance/v1`), in a file named `{ARTIFACT_NAME}-metadata.json` — `artifacts-metadata.json` when the artifact name is default.

- <https://docs.gitlab.com/ci/runners/configure_runners/>

Bearing: the `tag-and-build` job in `snippets/bash/gitlab/.gitlab-ci.yml`, and the falseness of the retired claim that the channel offers no provenance.

## GitLab, on the SLSA component and the exact cosign forms

The official SLSA component's `provenance-signer.yml` template extracts the statement's `.predicate` with jq and signs it with `cosign attest-blob --predicate … --type slsaprovenance1 --bundle …`, then self-verifies with `cosign verify-blob-attestation --type slsaprovenance1 --bundle … --certificate-oidc-issuer …` — confirming both command forms, where GitLab's `signing_examples` page shows `sign-blob` and `verify-blob` alone. The component's job runs downstream of the build with an `id_tokens` token whose audience is `sigstore`, and reads the metadata file from the build job's artifacts. Two of its choices stay with the component: `image: alpine:latest` and `apk add --update cosign` are floating references, and this repository pins every tool instead.

- <https://gitlab.com/components/slsa> — templates read at tag 0.1.1
- <https://docs.gitlab.com/ci/yaml/signing_examples/>

Bearing: the `provenance` job's command shape, and the choice to sign the statement rather than the blob.

## GitLab, on the keyless-signing boundary

The prerequisites for keyless signing state "You must be using GitLab.com", require Cosign 2.0.1 or later, and require the `id_tokens` portion of the CI/CD configuration to be located in the project being built and signed — AutoDevOps, CI files included from other repositories, and child pipelines are not supported. The ID token's `aud` claim is `sigstore`, and cosign picks the token up from the `SIGSTORE_ID_TOKEN` variable automatically.

- <https://docs.gitlab.com/ci/yaml/signing_examples/>

Bearing: the GitLab.com guard in the `provenance` job, and the boundary statements in `bindings/bash.md` and `forges/gitlab.md`.

## Sigstore, on installing cosign and cosign v3's bundle default

Sigstore's installation guidance has release binaries verified against their published digests before use. In cosign v3 the Sigstore bundle became the output format for blob signing — the `--bundle` flag moved from optional to required — and `cosign verify-blob-attestation` takes the bundle with `--certificate-identity` and `--certificate-oidc-issuer` for keyless flows, confirmed against the v3.1.3 binary's own `--help` output; the v2-era `--new-bundle-format` opt-in flag is gone.

- <https://docs.sigstore.dev/cosign/system_config/installation/>
- <https://github.com/sigstore/cosign/releases/tag/v3.0.1>

Bearing: the pinned, digest-verified cosign download in the `provenance` job, and the command forms differing from the component's v2-era ones.
