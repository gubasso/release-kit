# Task: provision

Bring `sdd` to this host at the version the repository records, through the manager the project runs. The task observes the project's manager files and records no host decision. The gate picks it where the pre-flight finds no `sdd`, an instance newer than the binary, or a first landing or a migration where no manager file of the project names `spec-driven-docs`.

## Pre-flight

1. Run `command -v nix mise devbox direnv` to learn which managers and loaders the host carries.
2. Read the manager files at the repository root, `flake.nix`, `mise.toml`, and `devbox.json`, and hold each one that names `spec-driven-docs`.

## Step A: how the project obtains sdd

1. Where no `sdd` runs yet, get one for this session first, as a gated step on the operator's host: enter the manager that already pins it (`nix develop`, `mise install`, or `devbox shell`), or else run `cargo binstall spec-driven-docs`.
2. Ask the operator for the method: which manager the project runs. The default is the manager the repository already carries. Point to the Install section of the sdd `README.md`, which the crate page at <https://crates.io/crates/spec-driven-docs> shows. No `sdd` verb installs or moves `sdd`. State the cost of each option:
   - a Nix flake devshell, mise, or devbox: the manager file and its lock record the version for every contributor;
   - asdf: no asdf plugin for sdd exists, so an asdf user installs the binary from the GitHub release, as a host install;
   - a native host install: no file records the version, so each host installs `sdd` itself.
3. For a manager, placing the install lines of that section in the manager file is a gated edit you make. A host answer records nothing, because no file records a host install.
4. Make the binary run in this session, as a gated step on the operator's host: `nix develop`, `mise install`, `devbox shell`, or `cargo binstall spec-driven-docs` for a host install.

## Step B: how the dev environment loads

1. Ask whether the environment loads through direnv, another loader, or none.
2. With direnv, the loader line the manager needs goes into `.envrc` as a gated edit.

## Post-flight

1. In the dev environment, `sdd --version` names the version the manager file records. On a native host install, it names a release no older than the instance record.
2. Run the general pre-flight again, now with the binary.
