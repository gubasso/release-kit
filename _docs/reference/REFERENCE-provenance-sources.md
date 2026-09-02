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
