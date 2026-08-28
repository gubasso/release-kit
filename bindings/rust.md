# Rust binding

| Axis                | Answer                                  |
| ------------------- | --------------------------------------- |
| Version file        | `Cargo.toml`                            |
| Release-request bot | release-plz                             |
| Registry and auth   | crates.io, trusted publishing over OIDC |
| Artifact builder    | cargo-dist                              |

`rk init --tech rust` lands `release-plz.toml`, `dist-workspace.toml`, and the publish workflow `.github/workflows/release-plz.yml`.

## The workflows

`release-plz.yml` is the publish workflow: it is the filename registered at crates.io, and the only workflow declaring `id-token: write`. It carries three jobs — the release-request maintainer on `develop`, the gate opener on the version-bump push, and the tag-and-publish half on `master`.

`release.yml` is the artifact workflow and cargo-dist generates it. Never edit it by hand: a hand edit is silently reverted at the next `dist generate`. Change `dist-workspace.toml` and regenerate instead, and bump the `cargo-dist-version` pin there deliberately — regenerate and read the diff. It is never registered at crates.io.

The tag push retriggers `release.yml` only because the publish jobs authenticate with a GitHub App token; a tag pushed with `GITHUB_TOKEN` starts no workflow.

## Setup specifics

- Step 0 is `cargo publish --dry-run` plus reading `cargo package --list`. crates.io hard-rejects a publish with no `description` and rejects a `categories` value that is not a canonical slug; both surface here without credentials.
- The `.crate` hard limit is 10 MB. For a binary crate no consumer reads any file in the tarball beyond build inputs, so `exclude` in `Cargo.toml` keeps it lean. `exclude` is the safer default over `include`: `include` is an allowlist that drops `README.md` and a plain `LICENSE` unless each is listed.
- Token scopes for the bootstrap publish: crates.io offers `publish-new`, `publish-update`, `yank`, `change-owners`, and `legacy`. The bootstrap token is `publish-new`, exact crate name, shortest expiry, revoked after the trusted publisher is registered.
- The trusted publisher is owner, repository, and the workflow filename `release-plz.yml`. Enforcement is the separate "Require trusted publishing for all new versions" switch, enabled only after one proven OIDC release.

## Operate specifics

- `release_always = true` in `release-plz.toml`, because the gate's head branch is `release/v<version>`, not a `release-plz-*` branch, so the branch heuristic behind `release_always = false` would never fire. Releasing always is safe: the command no-ops when the registry already serves the version.
- `git_release_enable = false`: cargo-dist owns the GitHub release, because it is the half holding the installers. Both creating it leaves dist failing on an existing tag name and every release page empty. The tag stays release-plz's.
- `semver_check = false` for a binary-only crate, or one whose lib target exists only for its own tests; cargo-semver-checks gates the bump only when external consumers hold the API.
- `cargo binstall <crate>` resolves cargo-dist's artifacts from the first release with no configuration.

## Recovery specifics

- Withdraw with `cargo yank --version <v>`; reverse with `cargo yank --version <v> --undo`. Yank stops new resolution and breaks no existing lockfile.
- The hand-publish path is `cargo publish` with a token, and it fails against enforcement with a message that points at `cargo login`, not at the switch; turn enforcement off first, as [recovery](../method/04-recovery.md) orders.
