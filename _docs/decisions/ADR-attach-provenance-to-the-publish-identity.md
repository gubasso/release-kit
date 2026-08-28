# Attach provenance to the publish identity

## Context and Problem Statement

The invariants move trust from people to automation, then stop at attaching artifacts. A consumer has checksums, which sit on the same mutable release page as the file they describe: they answer integrity, never origin. The ecosystem split that question in two. Trusted publishing settles who may publish; a build attestation settles what was built. The canon named only the first.

## Considered Options

- `A property of spine stage 5, each binding stating its channel's answer` — chosen.
- `A fifth diff-surface axis` — rejected: provenance is not a binding's choice, it is what the chosen channel offers, so the row would duplicate the registry and artifact-builder ones.
- `Mandate SLSA Build Level 3` — rejected: it needs build instructions in a reusable workflow outside the repository, reshaping every binding for a level most adopters do not need. Build Level 2 is the floor stated instead.
- `A project-owned scheme with a maintained key` — rejected: key custody and rotation to buy what the channel gives free, and no default installer checks that either.
- `Leave it to the adopting project` — rejected: uv 0.11.9 shipped unattested because a registry timeout sent a maintainer down the hand-publish path this canon documents, and verifying consumers refused it.

## Decision Outcome

Chosen option: `a property of stage 5`. A seventh invariant states that the identity authorizing the publish signs the artifacts, in the run that built them, and that a binding takes what its channel offers by default. Rust turns on `github-attestations` because crates.io stores no provenance; Python gets it free from the pinned publish action; Bash attests the tarball with `actions/attest`. Recovery names the hand-publish gap and routes it back through a workflow re-run.

## Consequences

- Good: the release page stops being the only witness to its own contents.
- Good: no key material, no rotation, no ceremony to forget.
- Bad: verification is one-sided today; no default install path checks an attestation, so the canon produces evidence ahead of its consumers.

## Status

Accepted.
