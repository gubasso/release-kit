# Packaging Sources

External sources behind the consumer-pin rules of `SPEC-packaging.md`: what the Nix CLI, direnv, and the forge promise, and which rule each promise bears on.

Verified against the listed sources on 2026-09-04.

## Nix, on updating one input and on the result symlink

`nix flake update` takes a list of input names as its positional arguments; by default all inputs are updated, a lock file that does not exist yet is created, and inputs not yet in the lock file are added. `nix build --no-link` does not create symlinks to the build results, whose default prefix under `--out-link` is `result`.

- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake-update>
- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-build>

Bearing: `packaging:the-consumer-pin-has-two-facts-and-one-mover` — the sync refreshes the one node by name, and a lock a seed did not write is created by the same call — and `packaging:a-devshell-bump-is-all-or-nothing`, whose fence builds with `--no-link` so a directory entry drops no `result` symlink into the tree.

## direnv, on `use flake` and the watched files

`use flake` loads the build environment of a derivation the way `nix develop` does, from the current directory's `flake.nix` devShell by default. The stdlib's `use_flake` calls `watch_file flake.nix` and `watch_file flake.lock`, and `watch_file` adds a file to direnv's watch list so a change reloads the environment on the next prompt.

- <https://direnv.net/man/direnv-stdlib.1.html>
- <https://github.com/direnv/direnv/blob/master/stdlib.sh>

Bearing: `packaging:the-unattended-caller-never-fails-the-shell` — the sync runs after `use flake`, with the pinned `rk` from the shell it is about to replace, and a moved pair reloads on its own because both files are watched.

## GitHub, on the latest release

The latest release is the most recent non-prerelease, non-draft release, sorted by the `created_at` attribute, and `/releases/latest` links to it.

- <https://docs.github.com/en/rest/releases/releases#get-the-latest-release>
- <https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases>

Bearing: `packaging:the-consumer-pin-has-two-facts-and-one-mover` — discovery reads the redirect of that page, which excludes prereleases, costs no API quota, and needs no token.
