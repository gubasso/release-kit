# Seeded by release-kit: a starting point this project owns and tunes;
# release-kit reports drift here and never rewrites it.
#
# Supported shape: one crate with a [package] table — an implicit
# src/main.rs binary or an explicit [[bin]] entry — building with the
# committed Cargo.lock. A workspace root fails by name below rather than
# throwing on a missing attribute: point the importTOML call at the member
# crate's Cargo.toml and set mainProgram yourself.
{ lib, rustPlatform }:

let
  cargoToml = lib.importTOML ../Cargo.toml;
  package =
    cargoToml.package
      or (throw "nix/package.nix: Cargo.toml has no [package] table; this seed does not support a workspace root");
in
rustPlatform.buildRustPackage {
  pname = package.name;
  version = package.version;

  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;

  meta =
    {
      # The first [[bin]] name where one is declared, else the package
      # name — the implicit src/main.rs binary. nix run resolves the
      # binary through this attribute.
      mainProgram = if cargoToml ? bin then (lib.head cargoToml.bin).name else package.name;
      # TODO(release-kit): match Cargo.toml's license expression; Nix
      # cannot derive it from the SPDX string in pure evaluation.
      license = lib.licenses.mit;
    }
    // lib.optionalAttrs (package ? description) { inherit (package) description; }
    // lib.optionalAttrs (package ? homepage) { inherit (package) homepage; };
}
