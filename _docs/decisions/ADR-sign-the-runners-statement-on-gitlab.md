# Sign the runner's statement on GitLab

## Context and Problem Statement

The `(bash, gitlab)` pair shipped claiming the channel offers no provenance attestation, leaving a checksum as the whole verification surface. GitLab documents a runner-generated SLSA v1 provenance statement, enabled with `RUNNER_GENERATE_ARTIFACTS_METADATA`, and publishes an official SLSA CI/CD component that signs it keylessly. Invariant 7's escape hatch was invoked on a falsehood, in the one pair with no registry behind it, whose release page is the entire distribution surface and mutable.

## Considered Options

- `Sign the runner's SLSA statement with cosign attest-blob` — chosen.
- `Sign the tarball with cosign sign-blob` — rejected: a signature over a digest carries no SLSA predicate, source commit or builder identity — it proves nothing a consumer can act on.
- `Leave checksums as the whole surface` — rejected: the premise is false, and whatever can replace the tarball can replace the `.sha256` beside it.
- `Ship the unsigned runner metadata alone` — rejected: SLSA Build Level 1 only; anyone who can swap the tarball can swap the metadata.

## Decision Outcome

Chosen option: sign the statement. The build job exports the tarball with runner metadata enabled; an isolated job signs the statement's predicate with `cosign attest-blob --type slsaprovenance1`, keylessly, against its own OIDC identity — no standing private key is managed, so this stays inside the one signing scheme the channel offers. The bundle publishes beside the tarball; `cosign verify-blob-attestation` verifies it.

The boundary is stated: Sigstore's public instance trusts `gitlab.com` and no self-managed instance, and `id_tokens` must live in the signed project. The pipeline degrades honestly elsewhere — it says why and releases without provenance instead of failing compilation.

## Consequences

- Good: the registry-less pair, which needed the attestation most, stops being the unsigned one.
- Good: the canon stops teaching a falsehood about the ecosystem.
- Bad: self-managed GitLab keeps checksums alone, now as a stated boundary.
- Bad: cosign becomes a pinned dependency with a digest to refresh deliberately.

## Status

Accepted.
