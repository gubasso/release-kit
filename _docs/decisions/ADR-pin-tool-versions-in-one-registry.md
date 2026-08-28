# Pin tool versions in one registry

## Context and Problem Statement

The landed files pin tool versions: workflow action references, a cargo-dist version, a git-cliff release. Pins age, and a canon that lands stale pins exports its staleness to every adopting project. Scattering the pins across snippets makes checking them a scavenger hunt.

## Considered Options

- `One registry file naming every pinned tool, its version, and the URL a freshness check queries` — chosen.
- `Pin nothing and always resolve latest at land time` — rejected: an unpinned workflow is not reproducible, and a registry outage or a breaking release lands silently.
- `An rk subcommand that queries upstream and reports stale pins` — deferred: revisit when the registry has aged enough that hand-checking it costs more than building the command.
- `A scheduled CI job that opens an issue on drift` — deferred: revisit alongside the subcommand it would run.

## Decision Outcome

Chosen option: `one registry file` — `versions.toml` at the repository root, embedded in the binary and printed by `rk versions`. Each entry carries the pinned version, the workflow reference where one exists, the bindings that use it, the URL a check queries, and the date of the last check. The rk-setup skill reads the registry at land time, compares each relevant entry upstream, and prefers the latest version, so an adopting project gets a best-effort-fresh landing even between canon releases.

## Consequences

- Good: one file answers what the canon pins and how old the answer is.
- Good: the land-time check moves freshness to the moment it matters, without a network dependency in the binary.
- Bad: the registry and the snippets state each pin twice, and nothing yet holds them equal.

## Status

Accepted.
