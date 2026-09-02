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
- Publishing needs a verified email on the account: an unverified one fails at the upload, after the token is minted.

### The artifact workflow pin

`versions.toml` owns the cargo-dist pin and `dist-workspace.toml` carries it. The nix devshell does not carry `dist`; install it at the pin with the installer the workflow itself uses, then prove the committed workflow is what that pin produces and read what a release will build.

```bash
PIN="$(grep -m1 '^cargo-dist-version' dist-workspace.toml | cut -d'"' -f2)"
curl --proto '=https' --tlsv1.2 -LsSf \
  "https://github.com/axodotdev/cargo-dist/releases/download/v$PIN/cargo-dist-installer.sh" | sh
dist --version
# check: prints the pin
dist generate
git diff --stat .github/workflows/release.yml
# check: no diff
dist plan
# check: prints the artifact list for every target in dist-workspace.toml
```

### The bootstrap token

The first publish is manual, and the registry exposes token creation in the browser only: the New API Token form at `crates.io/settings/tokens`. Mint the narrowest token that can do the job.

1. Click New Token.
2. Name: the crate and the job, `<crate> bootstrap publish`.
   - the field grants nothing; it is the label the token list shows
3. Expiration: pick 7 days from the dropdown.
   - it defaults to 90 days, and 7 is the shortest preset; the token has one job
   - the line beside the dropdown reads "The token will expire on" the date seven days out
4. Scopes: check `publish-new`, Publish new crates. Leave the other four unchecked.
   - `publish-update`, `yank`, `change-owners`, and `trusted-publishing` are jobs this token never does; the trusted publisher is registered in the crate's own settings, with no token
5. Crates: click Add pattern, then enter the crate's name.
   - a pattern also matches crates published after the token is created, so the unclaimed name binds
   - an empty list reads Unrestricted, which is wider than the job
6. Click Generate Token.
7. Copy the value from the new row.
   - the copy icon renders only where the browser exposes a clipboard; without one, select the shown value and copy it by hand
   - the value is shown this once; a token left uncopied is revoked and reminted, never guessed
   - check: the row reads Scopes: publish-new, Crates: the crate's name, and Expires in 7 days

`cargo login` then takes the value on stdin, and `cargo publish --locked` spends it.

### The trusted publisher

crates.io exposes this in the browser only, once per package.

1. Open `crates.io/crates/<crate>/settings`.
2. Under Trusted Publishing, choose Add, then GitHub.
3. Fill the form.
   - Repository owner: the account or organization
   - Repository name: the repository
   - Workflow filename: `release-plz.yml`
   - Environment: leave empty
4. Choose Add.
5. Read the Trusted Publishing table back on that page.
   - check: it lists the owner, the repository, and `release-plz.yml`

The filename is the invariant: `release-plz.yml` publishes, `release.yml` builds installers and is never registered. The workflow needs nothing added — `release-plz.yml` already carries `id-token: write` on its release job and sets no `CARGO_REGISTRY_TOKEN`, which is the trusted-publishing form release-plz documents; `rust-lang/crates-io-auth-action` belongs to hand-written publish workflows, not to this one.

Enforcement is the separate "Require trusted publishing for all new versions" checkbox on the same settings page, enabled only after one proven OIDC release; reload the page and the setting reads as enabled. From there every token publish is rejected, and the hand-publish escape in [recovery](../method/04-recovery.md) starts by turning it off.

### Revoking the bootstrap token

Two halves, and the second is the one people skip. Server side: open `crates.io/settings/tokens`, then click Revoke next to the bootstrap token. Host side: clear the local copy `cargo login` wrote.

```bash
cargo logout
grep -c crates-io "${CARGO_HOME:-$HOME/.cargo}/credentials.toml" 2>/dev/null || echo 0
# check: prints 0, or the file is gone
# already revoked: cargo logout says there is nothing to remove, and the count is still 0
```

The package then has exactly one publishing path.

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
