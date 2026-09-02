# Close the release on verified provenance

## Context and Problem Statement

The invariants demand a build-provenance attestation over every artifact a consumer downloads. The verify step checked none of it: operate step 6 listed the tag, the registry, the release page and an installed binary, and setup step 7 repeated the shorter form. Three releases were each verified correctly and each shipped unattested — nothing looked. The recovery table even routed a missing attestation through a workflow re-run, a path with nothing to trigger it. A stated invariant with no verification step is a comment, not a rule.

## Considered Options

- `Extend the verify step with a per-pair provenance check` — chosen.
- `One verifier per forge` — rejected: the verifier follows the channel that stores the attestation, not the forge alone. GitHub artifacts take `gh attestation verify`, a GitLab bundle takes `cosign verify-blob-attestation`, a PyPI distribution takes `pypi-attestations verify pypi` — and `(rust, gitlab)` declares no surface, where a forced check would fail wrongly.
- `Leave verification to the consumer` — rejected: a consumer cannot distinguish a missing attestation from an unattestable channel; the operator can, and the operator is the one who can still repair the release.

## Decision Outcome

Chosen option: extend the verify step. Operate step 6 names the check and bounds it to where the binding declares provenance; setup step 7's proof and the release guide's Done gate agree; the runbook renders the commands as pair-labelled variants, through a new `tech/forge` pair selector in `rk guide`, so each pair renders exactly its own verifier. The verifiers become pinned operator dependencies — cosign and pypi-attestations join `versions.toml`, and `rk doctor` probes both — so step 6 never surfaces a command the runbook never mentioned.

## Consequences

- Good: the defect class that shipped three unattested releases fails the release the day it recurs.
- Good: each pair is judged against its own honest surface.
- Bad: step 6 costs a download and two more installed tools.

## Status

Accepted.
