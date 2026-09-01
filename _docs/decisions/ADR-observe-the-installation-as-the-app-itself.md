# Observe the installation as the App itself

## Context and Problem Statement

`install-bot` observed through `GET /user/installations`, which GitHub serves only to App user access tokens — a class no CLI login and no personal access token produces — so the step 403'd for every operator, and its remediation named a classic token that cannot fix it. The installation-reading endpoints that answer are JWT-only, and the grant write takes a classic personal access token alone: no single credential reads and writes an installation. `REFERENCE-forge-setup-sources.md` records the verification.

## Considered Options

- `Sign the App JWT in rk and observe GET /repos/{owner}/{repo}/installation` — chosen.
- `Keep a user-token observation` — rejected: no reachable endpoint answers one.
- `Sign with a Rust crate` — rejected: `deny.toml` bans `openssl-sys`, the license allowlist excludes ISC (`ring`, `aws-lc-rs`), and `rsa` carries the unfixed RUSTSEC-2023-0071.
- `Carry the JWT through gh` — rejected: a JWT travels only as `Authorization: Bearer`, and gh sends the `token` scheme (cli/cli#12828).

## Decision Outcome

`rk` builds the RS256 signing input, has the OpenSSL CLI sign it — key bytes on standard input, no child told the path — and carries the token to the forge through `curl`, the header on standard input via `-H @-`. Both spawns bypass the journaling executor: the signer's output is the token's third segment, and a verbose curl would echo the header. The token is minted at most once per run — the key read once, its bytes and the token redaction needles from that moment — and observation, readback, and the grant's id discovery reuse it on the two exports step 7 already needs. `RK_BOT_INSTALLATION` became rk-derived from the account's own installation endpoint; the classic token's only job left is the grant write.

## Consequences

- Good: one credential pair serves observation, check, and `bot-secrets`; the dead listing, the operator-set id, and the classic token's every job but the grant write are gone.
- Bad: the host needs `openssl` and `curl`, probed by `rk doctor`, and two spawns replace one forge call per run.

## Status

Implemented — `src/setup/app_jwt.rs`, `setup/github/install-bot`.
