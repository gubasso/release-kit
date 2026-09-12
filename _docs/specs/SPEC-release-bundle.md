# Release Bundle Specification

## Purpose

Rules governing how the `rk` binary reads a release bundle. A release bundle is the payload of one release-kit release: every root `src/payload_roots.rs` declares, at that release's bytes. The engine reads a bundle through one seam, `ReleaseSource`, whether the bundle is the one compiled into the binary, one fetched from the crates venue, or a directory a test wrote. `SPEC-distribution.md` binds what the binary carries and writes outside a target. `SPEC-packaging.md` binds the venues `rk` publishes to. `SPEC-landing.md` binds what a landing writes into a target. This domain binds the boundary between an engine and any bundle it did not compile in. The external sources these rules were checked against are in `../reference/REFERENCE-release-bundle-sources.md`.

## Requirements

### `release-bundle:every-release-is-read-through-one-seam` — Every release is read through one seam

The projection, the block readers, and every landing verb MUST read a release bundle through `ReleaseSource` and MUST NOT reach an embedded global, because a planner written against the compiled-in payload can describe one release alone, and every question of the form "what does release X do here" would then need release X's binary.

#### Scenario: A front verb takes a shortcut to the embedded roots

- GIVEN a landing verb edited to read `embedded::SNIPPETS` or an `include_str!` of a block directly
- WHEN the test suite runs
- THEN the source scan fails naming the file, and the projection through the embedded source is proven byte-identical to the bytes on disk, so the seam costs nothing and skipping it buys nothing

Verify: `cargo nextest run -E 'test(every_front_verb_passes_a_release_source) or test(projection_through_the_embedded_source_is_byte_identical)'`

### `release-bundle:the-payload-schema-is-the-protocol-version` — The payload schema is the protocol version

The engine MUST read any bundle whose `payload_schema` is at or below its own, and MUST refuse a newer one before reading any of it, naming the bundle's release as the engine to install, because that number is the whole compatibility rule between an engine and a payload, and the one case where a newer binary must be obtained.

#### Scenario: A bundle from a later release declares a newer schema

- GIVEN a cached bundle whose sources declare a `PAYLOAD_SCHEMA` above this engine's
- WHEN `rk payload --release <version>` runs
- THEN it exits 73 with the reason `unsupported-schema` and an action naming that release, and no artifact of the bundle is served

Verify: `cargo nextest run -E 'test(an_engine_reads_a_bundle_at_or_below_its_schema) or test(an_engine_refuses_a_newer_schema_naming_the_engine_to_install) or test(payload_release_refuses_a_newer_schema_naming_the_engine_to_install)'`

### `release-bundle:a-fetched-bundle-is-verified-data` — A fetched bundle is verified data

When the crate source fetches an archive, it MUST verify the archive's SHA-256 against the checksum the registry index names before keeping anything, MUST refuse on a mismatch with nothing cached, and MUST treat the bundle as data: unpacked under one directory outside every target, never executed, never put on `PATH`, and never written into a repository.

#### Scenario: The archive and the index disagree

- GIVEN an index line naming one checksum and an archive that digests to another
- WHEN `rk payload --release <version>` runs
- THEN it exits 73 with the reason `bundle-unverified`, the cache holds no bundle and no version entry, and a landed target beside the run is byte-identical

Verify: `cargo nextest run -E 'test(the_crate_source_verifies_against_the_index_checksum) or test(the_crate_source_refuses_a_checksum_mismatch_and_caches_nothing) or test(the_crate_source_writes_under_the_state_directory_alone)'`

### `release-bundle:the-release-cache-is-content-addressed-and-bounded` — The release cache is content-addressed and bounded

The crate source MUST cache each verified bundle under the state root by the registry checksum, MUST serve a cached checksum and the exact version that named it with no network touch, and MUST keep at most the four newest fetched bundles, pruned after every fetch that adds one, because a content-addressed cache never serves a wrong byte and a cache nobody bounded grows forever.

#### Scenario: The same release is asked for twice

- GIVEN a bundle fetched and verified once
- WHEN `rk payload --release <version>` runs again with the network gone
- THEN it answers the same manifest, and the fetch log shows no new request

Verify: `cargo nextest run -E 'test(a_cached_digest_is_served_offline) or test(the_cache_keeps_the_newest_bundles)'`
