# Read every release through one seam

## Context and Problem Statement

The binary embeds every payload root at compile time, and the landing projection reads that payload alone. So only the 0.4.2 binary can say what release 0.4.2 lands, and a record proves a file diverged from its baseline without showing that baseline. Two questions are arguable: whether the engine runs the other release or reads it as data, and what the compatibility rule between an engine and a payload it did not compile in is.

## Considered Options

- A `ReleaseSource` trait with two methods, the manifest and a blob by digest, over the embedded roots, a fetched crate, or a directory — chosen.
- Running the candidate binary to describe itself — rejected: a fetched executable is a larger trust surface than an archive verified against the registry checksum.
- A new published bundle artifact — rejected: `distribution:the-published-crate-carries-every-root` already makes the crate a complete bundle.
- A schema file inside the bundle — rejected: every published release declares `PAYLOAD_SCHEMA` in its sources, and reading it back reaches every release.

## Decision Outcome

Chosen option: an engine that can only describe the payload compiled into it must be re-obtained to answer for another release, and a seam turns that executable into data the running engine reads. The projection and every landing verb take a source and name no embedded global. `payload_schema` is the protocol version: an engine reads any bundle at or below its own and refuses a newer one by naming the engine to install. The crate source verifies an archive against the index checksum before keeping it, caches by that checksum under the state root, serves a cached bundle offline, and keeps four.

Enforced by `release-bundle:every-release-is-read-through-one-seam`, `release-bundle:the-payload-schema-is-the-protocol-version`, `release-bundle:a-fetched-bundle-is-verified-data`, and `release-bundle:the-release-cache-is-content-addressed-and-bounded`.

## Consequences

- Good: a planner can describe any release it can read; a recorded release's baseline is bytes, not a digest; nothing fetched is executed.
- Bad: one indirection on every landing; a yanked or vanished release has no bundle, and a plan must say so.

## Status

Implemented: `src/release/`, `src/landing.rs`, `src/commands/payload.rs`.
