# Rust binding

| Axis                | Answer                                  |
| ------------------- | --------------------------------------- |
| Version file        | `Cargo.toml`                            |
| Release-request bot | release-plz                             |
| Registry and auth   | crates.io, trusted publishing over OIDC |
| Artifact builder    | cargo-dist                              |

`rk init --tech rust` lands `release-plz.toml`, `dist-workspace.toml`, and the publish workflow `.github/workflows/release-plz.yml`.

The artifact-builder answer holds on GitHub only. cargo-dist generates CI for GitHub Actions and for no other forge, so `(rust, gitlab)` has no artifact builder: the release page carries no installers, and [operate](../method/03-operate.md) step 5 has nothing to wait for. That is a smaller product, not a broken one — [the diff surface](../method/05-diff-surface.md) already contemplates an axis whose answer is nothing.

The registry-and-auth answer is narrower on GitLab too: crates.io trusted publishing covers GitHub Actions and GitLab.com only, the GitLab path in public beta, with no self-hosted support. A self-hosted GitLab therefore cannot satisfy the OIDC half of [the invariants](../method/01-invariants.md) and falls back to a long-lived token; `rk setup` reports that at its first step rather than letting it surface when the trusted publisher will not register.

## The workflows

`release-plz.yml` is the publish workflow: it is the filename registered at crates.io, and the only workflow declaring `id-token: write`. It carries two jobs — the release-request maintainer, which keeps the bump-and-changelog pull request open against the trunk, and the tag-and-publish half, which fires on the push that lands the bump.

`release.yml` is the artifact workflow and cargo-dist generates it. Never edit it by hand: a hand edit is silently reverted at the next `dist generate`. Change `dist-workspace.toml` and regenerate instead, and bump the `cargo-dist-version` pin there deliberately — regenerate and read the diff. It is never registered at crates.io.

The tag push retriggers `release.yml` only because the publish jobs authenticate with a GitHub App token; a tag pushed with `GITHUB_TOKEN` starts no workflow.

## Setup specifics

- Step 0 is `cargo publish --dry-run` plus reading `cargo package --list`. crates.io hard-rejects a publish with no `description` and rejects a `categories` value that is not a canonical slug; both surface here without credentials.
- The `.crate` hard limit is 10 MB. For a binary crate no consumer reads any file in the tarball beyond build inputs, so `exclude` in `Cargo.toml` keeps it lean. `exclude` is the safer default over `include`: `include` is an allowlist that drops `README.md` and a plain `LICENSE` unless each is listed.
- Token scopes for the bootstrap publish: crates.io offers `publish-new`, `publish-update`, `yank`, `change-owners`, and `legacy`. The bootstrap token is `publish-new`, exact crate name, shortest expiry, revoked after the trusted publisher is registered.
- The trusted publisher is owner, repository, and the workflow filename `release-plz.yml`. Enforcement is the separate "Require trusted publishing for all new versions" switch, enabled only after one proven OIDC release.

## Operate specifics

- `release_always = false` in `release-plz.toml`: the release half fires only on the merge of the bot's own request, which the branch heuristic recognizes by its `release-plz-*` head branch, so an ordinary work merge publishes nothing and the release decision stays on the one merge button.
- `git_release_enable = false`: cargo-dist owns the GitHub release, because it is the half holding the installers. Both creating it leaves dist failing on an existing tag name and every release page empty. The tag stays release-plz's.
- `semver_check = false` for a binary-only crate, or one whose lib target exists only for its own tests; cargo-semver-checks gates the bump only when external consumers hold the API.
- `cargo binstall <crate>` resolves cargo-dist's artifacts from the first release with no configuration.

## Provenance

- crates.io offers none. Trusted publishing authenticates the upload and stores no signature and no attestation, so a published crate gives a consumer nothing beyond the SHA-256 that `Cargo.lock` already records. Sigstore signing for crates.io remains a proposal, so this is the state to design around rather than wait out.
- The release artifacts carry it instead, which makes them the only verifiable half of a Rust release. `github-attestations = true` in `dist-workspace.toml` turns on GitHub Artifact Attestations; `github-attestations-phase` chooses where they are minted and defaults to `build-local-artifacts`, the phase that builds the binaries. Only public repositories, and private repositories of an Enterprise-plan organization, are supported.
- A consumer verifies with `gh attestation verify <file> --repo <owner>/<repo>`. Nothing in the default install path does this for them: `cargo binstall` supports minisign signatures only and does not check attestations, so the evidence is available on demand and enforced by no installer.

## Recovery specifics

- Withdraw with `cargo yank --version <v>`; reverse with `cargo yank --version <v> --undo`. Yank stops new resolution and breaks no existing lockfile.
- The hand-publish path is `cargo publish` with a token, and it fails against enforcement with a message that points at `cargo login`, not at the switch; turn enforcement off first, as [recovery](../method/04-recovery.md) orders.
