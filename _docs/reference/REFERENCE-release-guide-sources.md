# Release Guide Sources

External sources behind [setup.md](../guides/release/setup.md): what the registry's token form carries, what its scopes and crate patterns mean, and where cargo keeps the credential the bootstrap publish uses. Each entry states what the source says and which step it bears on.

Verified against the listed sources on 2026-09-01, the version-derivation entry included.

## The Cargo book, on version numbers below 1.0.0

"This guide uses the terms \"major\" and \"minor\" assuming this relates to a \"1.0.0\" release or later. Initial development releases starting with \"0.y.z\" can treat changes in \"y\" as a major release, and \"z\" as a minor release. \"0.0.z\" releases are always major changes. This is because Cargo uses the convention that only changes in the left-most non-zero component are considered incompatible." The specification the convention rests on says of the same range: "Major version zero (0.y.z) is for initial development. Anything MAY change at any time."

- <https://doc.rust-lang.org/cargo/reference/semver.html>
- <https://semver.org/>

Bearing: release.md step 1, the intent a squash title carries. Below 1.0 a breaking change moves the minor and a feature moves the patch, so a `feat` proposing a patch bump is the convention working rather than a misread commit.

## RFC 2947, on token scopes and crate patterns

The RFC defines the original endpoint-scope model: `publish-new` allows publishing new crates, `publish-update` allows new versions of existing crates, `yank` allows yanking and unyanking, `change-owners` allows inviting and removing owners, and `legacy` grants everything but token creation. The current form keeps the first four as checkboxes, adds `trusted-publishing`, and exposes no `legacy` box; the frontend source below is the authority on the current set. A crate scope restricts the token to name patterns — an exact name, or a prefix ending in `*` — and a pattern allows interacting with matching crates published after token creation, which is what lets the bootstrap token pin a name that does not exist yet.

- <https://rust-lang.github.io/rfcs/2947-crates-io-token-scopes.html>

Bearing: setup.md step 15.3, the scope choice and the crate pattern. The pattern binding future crates is why the pattern is `$CRATE` rather than empty: an unrestricted token is wider than the one job it has.

## The crates.io token form, from its own source

The New API Token page (`crates.io/settings/tokens`, New Token) carries four fields. Name is required text. Expiration is a dropdown of No expiration, 7, 30, 60, 90, and 365 days, plus a custom date, defaulting to 90; 7 days is the shortest preset. Scopes are five checkboxes — `change-owners`, `publish-new`, `publish-update`, `trusted-publishing`, `yank` — of which at least one must be checked; `trusted-publishing` manages trusted-publishing configurations and is distinct from the crate-settings flow step 16 walks. Crates is a pattern list whose empty state reads Unrestricted; Add pattern appends an entry, and a pattern is `*`, or an identifier of ASCII alphanumerics, `_`, and `-` not opening with `_` or `-`, optionally ending in `*`. Generate Token returns to the token list, which shows the value once beside the explainer "Make sure to copy your API token now. You won't be able to see it again!", and a token row lists its scopes. The copy icon renders only where the browser exposes a clipboard, reports "Copied to clipboard!" on success and "Copy to clipboard failed!" on failure, and the shown value is selectable for a hand copy. A token row lists the name, Scopes, Crates, and an Expires distance.

- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/routes/settings/tokens/new/+page.svelte>
- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/lib/utils/token-scopes.ts>
- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/routes/settings/tokens/+page.svelte>
- <https://github.com/rust-lang/crates.io/blob/main/svelte/src/lib/components/CopyButton.svelte>
- <https://blog.rust-lang.org/2023/06/23/improved-api-tokens-for-crates-io/>

Bearing: setup.md step 15.3, every field the browser flow enumerates. The frontend source is the authority the deployed form is built from; the blog post introduced the scoped-token UI and corroborates the field set.

## The Cargo book, on the token's life around the publish

Publishing requires a crates.io account with a verified email. `cargo login` prompts for the token on standard input and stores it in `$CARGO_HOME/credentials.toml`; the book marks the token a secret to revoke immediately if it leaks, and `cargo logout` removes the stored copy.

- <https://doc.rust-lang.org/cargo/reference/publishing.html>

Bearing: setup.md step 0's verified-email prerequisite, step 15.4's `cargo login` on stdin, and step 17's host-side revocation of the local copy.
