# Pin every action by commit

## Context and Problem Statement

Every action in the payload was referenced by a movable ref — seven tags and branches in `versions.toml`, plus `dtolnay/rust-toolchain@stable` declared nowhere — `actions/attest`, the step minting the bash pair's signature, among them. In March 2025 tj-actions/changed-files was compromised by pushing malicious code to existing tags; every repository referencing it by tag ran that code and leaked CI secrets. GitHub documents the full commit SHA as the immutable reference.

## Considered Options

- `Pin by commit SHA, readable ref as a comment, both in the registry` — chosen; the form Renovate's `helpers:pinGitHubActionDigests` produces.
- `Bare SHA with no comment` — rejected: unreviewable, and nothing states what the pin came from.
- `Keep tags and trust the marketplace` — rejected by the incident above.
- `Treat a moved discovery ref as a security event` — rejected: those refs exist to move, the tool cannot distinguish an update from an attack, and the pinned commit already prevents movement from changing what executes.

## Decision Outcome

Chosen option: two fields with two jobs. The registry's `commit` is the immutable execution reference; the tag or branch in `action` stays as the discovery ref, classified by `ref_class`, and `rk versions --check` resolves it upstream, reporting `ref-unmoved`, `ref-moved`, `ref-unreachable` or `ref-unparsable` in schema `rk.versions-check/2`. cargo-dist's injected actions — the attest step included — are pinned through `[dist.github-action-commits]` in the seed and this repository alike, and the seeded-file invariant guard holds a landed target's table to the seed's. Boundary: the GitLab payload fetches no CI code — no catalog component, held by test — and container images stay pinned by version tag; they are the execution environment, not fetched steps.

## Consequences

- Good: a moved tag cannot change what any payload workflow executes, the signing step included.
- Good: every pin has one declared owner and a resolvable freshness signal.
- Bad: a pin must be refreshed deliberately; `rk versions --check` is what keeps that cost visible.

## Status

Accepted.
