{
  description = "release-kit development shell and package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      # The support claim, not a convenience: every system named here is one
      # CI natively builds and smokes, and the list grows together with the
      # CI matrix (packaging:an-advertised-system-is-a-proven-system).
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            }
          )
        );
    in
    {
      packages = eachSystem (pkgs: rec {
        release-kit = pkgs.callPackage ./nix/package.nix { };
        default = release-kit;
      });

      devShells = eachSystem (
        pkgs:
        let
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in
        {
          # What belongs here: a tool this project pins, and a runtime pre-commit
          # needs to build a hook environment. What does not: the host baseline.
          # git is assumed present, because a pre-commit hook has no meaning
          # without it; the Rust toolchain comes from rust-toolchain.toml through
          # the overlay so CI and local development share one compiler. The
          # installed package owns its own runtime closure in nix/package.nix.
          default = pkgs.mkShell {
            packages = [
              toolchain
              pkgs.cargo-nextest
              pkgs.cargo-deny
              pkgs.just
              pkgs.pre-commit
              pkgs.dprint
              pkgs.editorconfig-checker
              pkgs.nodejs
              pkgs.ripgrep
              pkgs.python3Packages.md-toc
              pkgs.typos
              pkgs.markdownlint-cli2
              pkgs.lychee
              pkgs.ripsecrets
              pkgs.shellcheck
              pkgs.shfmt
              pkgs.jq
            ];
          };
        }
      );

      checks = eachSystem (
        pkgs:
        let
          pkg = self.packages.${pkgs.stdenv.hostPlatform.system}.release-kit;
        in
        {
          # nix flake check builds only the checks output; packages are merely
          # evaluated, so the package itself is the first check.
          package = pkg;
          # The payload canary: a source-filtering regression that survives
          # the build fails here, offline.
          smoke = pkgs.runCommand "release-kit-smoke" { } ''
            ${pkgs.lib.getExe pkg} --version
            ${pkgs.lib.getExe pkg} method --list > /dev/null
            touch $out
          '';
        }
      );

      formatter = eachSystem (pkgs: pkgs.nixfmt-rfc-style);
    };
}
