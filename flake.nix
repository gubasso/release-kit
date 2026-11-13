{
  description = "release-kit development shell and package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # This repository's docs gate, taken as a dev dependency through a flake
    # input. The tag in this URL is the version and flake.lock is
    # the content pin; nothing else in this repository names an sdd version.
    # The predecessor was a revision installed by a CI step, because the last
    # release then carried neither a Nix package nor the gates this instance
    # runs. A released tag carries both now, so the pin is a tag and the CI
    # step is gone. Freshness is the manager's own verb: nix flake update
    # spec-driven-docs, with the URL's tag moved to match, followed by sdd
    # upgrade where the release moves the canon.
    spec-driven-docs = {
      url = "github:gubasso/spec-driven-docs/v0.17.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # The store guard of the canon (exobrain): `exobrain store-guard --daily`
    # in .envrc warns about working-tree copies in the Nix store.
    exobrain-tech = {
      url = "github:gubasso/exobrain-tech/v1.0.0";
      inputs.nixpkgs.follows = "nixpkgs";
      # Its rk and sdd serve only its own devShell, so it takes these: the
      # empty string is the root flake, this repository.
      inputs.release-kit.follows = "";
      inputs.spec-driven-docs.follows = "spec-driven-docs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      spec-driven-docs,
      exobrain-tech,
    }:
    let
      # The support claim, not a convenience: the one system named here is
      # the one CI natively builds and smokes, and the only platform this
      # project supports (packaging:an-advertised-system-is-a-proven-system).
      systems = [
        "x86_64-linux"
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
          # The release jobs run this release-plz, so it stays at or above the
          # registry's pin in versions.toml: git-only mode packages no earlier
          # release from 0.3.168 on. nixpkgs carries 0.4.1, so the flake builds
          # the pinned release over the nixpkgs package. Drop the override once
          # nixpkgs carries the pin.
          release-plz = pkgs.release-plz.overrideAttrs (
            finalAttrs: prev: {
              version = "0.4.3";
              src = pkgs.fetchFromGitHub {
                owner = "release-plz";
                repo = "release-plz";
                rev = "release-plz-v${finalAttrs.version}";
                hash = "sha256-lUyhbf4KD9QVUBebgcrfTWkHV4PBxvNZMQlObCU8jwM=";
              };
              cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
                inherit (finalAttrs) pname version src;
                hash = "sha256-wYDAx23ayZCHFTTB3bLHt7ar6FOh+9kIR0WexbvGeyc=";
              };
              buildInputs = prev.buildInputs ++ [
                pkgs.aws-lc
                pkgs.libgit2
                pkgs.sqlite
                pkgs.zstd
              ];
            }
          );
        in
        {
          # What belongs here: a tool this project pins that no more specific
          # owner declares, and a runtime the hook runner needs to build or run
          # a hook. A hook tool that the runner can build (typos, zizmor,
          # actionlint, md-toc, ...) is pinned in .pre-commit-config.yaml, and a
          # language library never enters this shell. What does not: the host
          # baseline. git and the hook runner, prek, come from the host:
          # .pre-commit-config.yaml declares the oldest prek it accepts, and
          # ci.yml installs that version. The Rust toolchain comes from
          # rust-toolchain.toml through the overlay so CI and local development
          # share one compiler. The installed package owns its own runtime
          # closure in nix/package.nix.
          #
          # rk is absent on purpose, and it is the one tool this shell may not
          # carry. A binary embeds the sources it was compiled from, so an rk
          # built when the lock last moved judges the current snippets against a
          # stale copy of them and reports drift that is not there. This
          # repository's rk is the one it just built: the justfile puts
          # target/debug on PATH for the sweep, and .envrc does the same for an
          # interactive shell.
          default = pkgs.mkShell {
            packages = [
              toolchain
              spec-driven-docs.packages.${pkgs.stdenv.hostPlatform.system}.default
              # exobrain, the store guard that .envrc runs once a day.
              exobrain-tech.packages.${pkgs.stdenv.hostPlatform.system}.default
              pkgs.cargo-nextest
              pkgs.cargo-deny
              pkgs.just
              pkgs.dprint
              # The markdownlint-cli2 hook builds on this nodejs
              # (default_language_version in .pre-commit-config.yaml).
              pkgs.nodejs
              # The Verify lines of the specs that sdd ships call rg.
              pkgs.ripgrep
              pkgs.lychee
              # actionlint, a hook that the hook runner builds, runs shellcheck from
              # PATH on the `run:` scripts of the workflows it reads.
              pkgs.shellcheck
              pkgs.jq
              # The changelog test renders the landed cliff.toml seeds through
              # the git-cliff the release pipelines install: the same release
              # tarball at the pin versions.toml names, not the nixpkgs build,
              # which moves with the lock. The rk release engine refuses any
              # other version.
              (pkgs.stdenvNoCC.mkDerivation rec {
                pname = "git-cliff";
                version = "2.14.2";
                src = pkgs.fetchurl {
                  url = "https://github.com/orhun/git-cliff/releases/download/v${version}/git-cliff-${version}-x86_64-unknown-linux-musl.tar.gz";
                  hash = "sha256-+iHi/HAEb8/Q5D5eRhOHOreZr/PZ3IVZwA0xsnDkF9A=";
                };
                installPhase = ''
                  install -Dm755 git-cliff $out/bin/git-cliff
                '';
              })
              # The release jobs run release-plz from this shell, as
              # .release-kit/config.toml records, so CI installs no release
              # tool of its own.
              release-plz
              # dist, the latest cargo-dist release. Its callers: the dist-plan
              # and dist-build jobs of ci.yml, and the dist hook of
              # .pre-commit-config.yaml. dist-workspace.toml pins the same
              # version, and a CI step fails when the two differ. Should
              # nixpkgs lag the latest release, build it here over the nixpkgs
              # package, the way release-plz is built above.
              pkgs.cargo-dist
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
          # The served-content canary plus the wrapper proof, offline. HOME is set
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
