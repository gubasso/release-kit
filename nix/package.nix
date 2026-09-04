{ lib, rustPlatform }:

let
  cargoToml = lib.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  # cleanSource, not a fileset: include_dir! embeds every root
  # src/payload_roots.rs declares, and a filter that omits one produces a
  # binary that builds and lies. The preBuild assertion below is what makes
  # a narrowed source fail by path.
  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;

  # tests/cli.rs drives real git and forge CLIs the sandbox lacks; just
  # check owns validation, and the flake's checks.smoke carries the
  # Nix-side signal.
  doCheck = false;

  # Every payload root, read from its one declaration rather than restated
  # here, plus the license files src/embedded.rs embeds from outside that
  # inventory. A root missing from the source closure fails by name, before
  # two smoke commands that would never notice.
  preBuild = ''
    roots=$(sed -n 's/^ *"\(.*\)",$/\1/p' src/payload_roots.rs)
    if [ -z "$roots" ]; then
      echo "no payload roots parsed from src/payload_roots.rs" >&2
      exit 1
    fi
    for path in $roots LICENSE LICENSE-MIT LICENSE-CC-BY-4.0; do
      if [ ! -e "$path" ]; then
        echo "payload root missing from the package source: $path" >&2
        exit 1
      fi
    done
  '';

  meta = {
    inherit (cargoToml.package) description homepage;
    license = with lib.licenses; [
      mit
      cc-by-40
    ];
    # Load-bearing: the package name and the binary name differ, and
    # nix run resolves the binary through this attribute.
    mainProgram = "rk";
  };
}
