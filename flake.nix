{
  description = "release-kit development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in {
        # What belongs here: a tool this project pins, and a runtime pre-commit
        # needs to build a hook environment. What does not: the host baseline.
        # git is assumed present, because a pre-commit hook has no meaning
        # without it; the Rust toolchain comes from rust-toolchain.toml through
        # the overlay so CI and local development share one compiler.
        devShells.default = pkgs.mkShell {
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
        checks.shell = pkgs.runCommand "release-kit-shell" { } ''
          test -x ${pkgs.bash}/bin/bash
          touch $out
        '';
        formatter = pkgs.nixfmt-rfc-style;
      });
}
