# Seeded by release-kit, and landed only where the target has no flake:
# an existing flake is never overwritten, and this project owns and tunes
# this one from the first landing.
#
# Deliberately minimal: one input and no toolchain overlay, so the seed
# imposes nothing. The package expression it calls is nix/package.nix;
# add toolchain machinery, more checks, or a richer devshell as the
# project needs them.
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = eachSystem (pkgs: rec {
        release = pkgs.callPackage ./nix/package.nix { };
        default = release;
      });

      checks = eachSystem (pkgs: rec {
        # nix flake check builds only the checks output; the package
        # itself is the first check, and the smoke run proves the built
        # binary answers at all.
        package = pkgs.callPackage ./nix/package.nix { };
        smoke = pkgs.runCommand "flake-smoke" { } ''
          ${pkgs.lib.getExe package} --version
          touch $out
        '';
      });

      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ (pkgs.callPackage ./nix/package.nix { }) ];
        };
      });
    };
}
