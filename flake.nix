{
  description = "release-kit development shell and package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # This repository's docs gate, taken as a dev dependency the way rk depend
    # serves the fragment. The tag in this URL is the version and flake.lock is
    # the content pin; nothing else in this repository names an sdd version.
    # The predecessor was a revision installed by a CI step, because the last
    # release then carried neither a Nix package nor the gates this instance
    # runs. A released tag carries both now, so the pin is a tag and the CI
    # step is gone. Freshness is the manager's own verb: nix flake update
    # spec-driven-docs, with the URL's tag moved to match, followed by sdd
    # upgrade where the release moves the canon.
    spec-driven-docs = {
      url = "github:gubasso/spec-driven-docs/v0.4.5";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      spec-driven-docs,
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
          #
          # rk is absent on purpose, and it is the one tool this shell may not
          # carry. A binary embeds the payload it was compiled from, so an rk
          # built when the lock last moved judges the current snippets against a
          # stale copy of them and reports drift that is not there. This
          # repository's rk is the one it just built: the justfile puts
          # target/debug on PATH for the sweep, and .envrc does the same for an
          # interactive shell.
          default = pkgs.mkShell {
            packages = [
              toolchain
              spec-driven-docs.packages.${pkgs.stdenv.hostPlatform.system}.default
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
              # The payload is GitHub Actions workflows, so the workflows are
              # what this project lints. zizmor audits them for security and
              # actionlint for correctness, both over snippets/ and over this
              # repository's own .github/workflows/.
              pkgs.zizmor
              pkgs.actionlint
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
          # The payload canary plus the wrapper proof, offline. HOME is set
          # because the state-root probe is Hard by design with no home; only
          # PATH is cleared, so a failure here means the wrapper lost a tool,
          # not that the sandbox lost a variable. The greps hold the git
          # probe's own line, never doctor's exit code, so an unrelated soft
          # probe cannot flip the result.
          smoke = pkgs.runCommand "release-kit-smoke" { } ''
            export HOME=$TMPDIR
            PATH= ${pkgs.lib.getExe pkg} --version
            PATH= ${pkgs.lib.getExe pkg} method --list > /dev/null
            # the wrapper's PATH suffix supplies git and sh
            PATH= ${pkgs.lib.getExe pkg} doctor | grep -E '^  ok +git: '
            PATH= ${pkgs.lib.getExe pkg} doctor | grep -E '^  ok +sh: '
            # an operator's own override wins over the wrapped git
            PATH= RK_GIT_BIN=/nonexistent/git ${pkgs.lib.getExe pkg} doctor \
              | grep -E '^  failed +git: '
            touch $out
          '';
        }
      );

      formatter = eachSystem (pkgs: pkgs.nixfmt-rfc-style);
    };
}
