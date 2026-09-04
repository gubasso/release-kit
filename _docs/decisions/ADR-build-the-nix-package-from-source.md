# Build the Nix package from source with plain rustPlatform

## Context and Problem Statement

The flake exposes an installable `rk` beside the devshell. Three build strategies compete: compile the crate from the repository, repackage the prebuilt dist archives, or reuse the rust-overlay toolchain the devshell pins. The choice decides how many hashes and version strings the tree carries and what a consumer's `nix build` actually proves.

## Considered Options

- `rustPlatform.buildRustPackage` from source with `cargoLock.lockFile` — chosen.
- Repackaging the dist release archives — rejected: the archives are `-gnu`-linked and need patchelf on NixOS, and every target reinstates per-target hash maintenance; a binary cache is the later answer to compile time.
- The rust-overlay toolchain inside the package — rejected: the pinned nixpkgs rustc already clears `rust-version` and edition 2024, and keeping the overlay out keeps `nix/package.nix` callPackage-able without it.
- `crane` or `naersk` — rejected: building within the project with a committed lockfile is the documented case for `cargoLock`, and an extra input buys nothing here.

## Decision Outcome

Chosen option: `rustPlatform.buildRustPackage` from source — it leaves zero hashes in the tree, reads name and version from `Cargo.toml` alone, and makes a consumer's build prove the same source the tag names.

Enforced by `packaging:the-version-has-one-owner` and `packaging:the-package-source-carries-every-root`.

## Consequences

- Good: no `cargoHash`, no restated version, and the derivation stays a portable callPackage unit.
- Bad: a consumer compiles the crate; compile time is answered later by a binary cache, not by repackaging.

## Status

Implemented: `nix/package.nix`, wired in `flake.nix`.
