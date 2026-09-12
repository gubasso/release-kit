# Release Bundle Sources

External sources behind `SPEC-release-bundle.md`: how the crates venue lists a crate's versions and their checksums, where an archive is fetched from, and what the archive holds. Each entry states what the source says and which rule it bears on.

Verified against the listed sources on 2026-09-12.

## The sparse registry index, on the entry path and the checksum

The sparse protocol serves one file per crate over HTTP, at a path derived from the crate name: for a name of four or more characters, the first two characters, then the third and fourth, then the name, all lowercase. Each line of the file is one JSON object for one published version, carrying `name`, `vers`, `deps`, `cksum`, `features`, and `yanked`. `cksum` is the SHA-256 checksum of the `.crate` file, as a hex string. The registry's `config.json` names the download URL template, and crates.io's template resolves to `https://static.crates.io/crates/{crate}/{crate}-{version}.crate`.

- <https://doc.rust-lang.org/cargo/reference/registry-index.html>
- <https://doc.rust-lang.org/cargo/reference/registry-index.html#sparse-protocol>
- <https://doc.rust-lang.org/cargo/reference/registry-index.html#index-configuration>

Bearing: `release-bundle:a-fetched-bundle-is-verified-data` — the source reads the crate's one index file, takes `cksum` from the line naming the version, and compares the fetched archive's SHA-256 against it before anything is kept. `release-bundle:the-release-cache-is-content-addressed-and-bounded` — the same checksum names the cache directory, so a cached bundle is one the registry vouched for.

## The crate archive, on its layout

`cargo package` produces a gzip-compressed tar archive named `<name>-<version>.crate`, whose entries sit under one top-level directory `<name>-<version>/`. The archive carries the files the manifest's `include` and `exclude` fields admit, together with `Cargo.toml` at the package root. `cargo package --list` prints that file set.

- <https://doc.rust-lang.org/cargo/commands/cargo-package.html>
- <https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields>

Bearing: `release-bundle:a-fetched-bundle-is-verified-data` — the source unpacks the archive with `tar -xzf`, expects the `<name>-<version>/` directory, and reads the payload roots beneath it. `distribution:the-published-crate-carries-every-root` is what makes that archive a complete bundle, and its packaging test is what proves it before a release.

## The payload schema, on where a bundle declares it

A bundle carries no schema file. The number is the `PAYLOAD_SCHEMA` constant of the release the bundle is, declared once in a source file under `src/` as `const PAYLOAD_SCHEMA: u32 = <n>;`, with or without `pub`. Every release since the constant existed declares it in that spelling, and a test in this crate holds this release to it.

Bearing: `release-bundle:the-payload-schema-is-the-protocol-version` — the directory source reads the constant back out of the bundle's sources, so the protocol check reaches every published release without a format change.
