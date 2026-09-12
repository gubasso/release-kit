# Packaging Sources

External sources behind the consumer-pin rules of `SPEC-packaging.md` and the artifact-reach half of `SPEC-forge-setup.md`: what the Nix CLI, direnv, the forge, and each binding's packaging tool promise, and which rule each promise bears on.

Verified against the listed sources on 2026-09-04, and the packaging-tool sections on 2026-09-11.

## Nix, on updating one input and on the result symlink

`nix flake update` takes a list of input names as its positional arguments; by default all inputs are updated, a lock file that does not exist yet is created, and inputs not yet in the lock file are added. `nix build --no-link` does not create symlinks to the build results, whose default prefix under `--out-link` is `result`.

- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake-update>
- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-build>

Bearing: `packaging:the-consumer-pin-has-two-facts-and-one-mover` — the sync refreshes the one node by name, and a lock a seed did not write is created by the same call — and `packaging:a-pin-bump-is-all-or-nothing`, whose fence builds with `--no-link` so a directory entry drops no `result` symlink into the tree.

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

## Cargo, on package selection and the file listing

`cargo metadata` reports `workspace_default_members`, the package ids a bare command operates on, beside a `packages` array whose entries carry `id` and `manifest_path`; `--no-deps` limits the output to workspace members and `--format-version 1` fixes the shape. `cargo package --list` prints the files that would be included in the generated `.crate`, one per line; `--manifest-path` selects the manifest, and `--allow-dirty` permits an uncommitted working tree. Which files land is governed by `[package].include` and `[package].exclude`, which the manifest reference documents together with the ignore rules Cargo applies, and that reference names `cargo package --list` as the way to see the result. A package root bounds the files a package can carry, so a nested workspace member cannot include a file above it.

- <https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>
- <https://doc.rust-lang.org/cargo/commands/cargo-package.html>
- <https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields>

Bearing: `forge-setup:a-package-check-states-policy-reach` — the metadata call decides whether exactly one default package is rooted at the target, which is the one shape whose listing is unambiguous, and the listing then answers policy inclusion by exact path.

## Python packaging, on the sdist and the wheel

A PEP 517 project names its own build backend in `pyproject.toml`, and the backend decides what each distribution contains; `python -m build` produces an sdist and then a wheel from that sdist by default. The packaging guide states that the sdist and the wheel are different distribution formats with different contents, and each backend documents its own inclusion configuration, so there is no one command that lists both across backends.

- <https://peps.python.org/pep-0517/>
- <https://build.pypa.io/en/stable/>
- <https://packaging.python.org/en/latest/discussions/package-formats/>

Bearing: `forge-setup:a-package-check-states-policy-reach` — the binding has no deterministic listing command here, so the check keeps its packaging result and names the inclusion unproved.

## Git, on archive and export-ignore

`git archive` creates an archive of the named tree, and paths carrying the `export-ignore` attribute are not added to it.

- <https://git-scm.com/docs/git-archive>
- <https://git-scm.com/docs/gitattributes#_creating_an_archive>

Bearing: `forge-setup:a-package-check-states-policy-reach` — a tracked file can be absent from the Bash binding's tarball, so the check names the tarball as uninspected instead of implying the policy travels.
