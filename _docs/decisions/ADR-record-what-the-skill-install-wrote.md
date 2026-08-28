# Record what the skill install wrote

## Context and Problem Statement

`rk skill install` writes under the user's home, where nothing records those files. Its only reference is the payload it currently carries, which makes a copy an older release wrote indistinguishable from a file the user edited. Every release touching a skill would then refuse on destinations nobody touched, breaking `just install`.

## Considered Options

- `Record what a successful apply wrote` — chosen.
- `Pass --force in the install recipe` — rejected: it removes the protection while leaving the appearance of it.
- `Compile every released payload into the binary` — rejected: it would carry every historical skill forever.
- `Compare against the last released tag over the network` — rejected: it gives up the offline install embedding bought.

## Decision Outcome

Chosen option: `record what a successful apply wrote`. `$HOME/.local/state/release-kit/skills.sha256` maps each destination to the digest written there, and an apply refuses only on bytes matching neither the payload nor that record. `--force` still overrides.

The record is state, not a manifest. Nothing verifies against it, and every unreadable shape resolves to an empty record, so a lost one costs only the benefit of the doubt. It is home-relative rather than XDG-relative because the roots it speaks for are `$HOME/.claude` and `$HOME/.agents`, which no XDG variable moves.

It also names what the payload cannot: a renamed skill leaves a file an agent keeps offering, and only the record marks that leftover as ours to take back.

It also backs up before the first write: the loop crosses two roots, and a failure on the second left the first upgraded.

Enforced by `distribution:a-stale-skill-is-not-a-conflict`, `distribution:an-install-sweeps-what-the-payload-dropped`, and `distribution:a-skill-install-restores-on-failure`.

## Consequences

- Good: `just install` is idempotent across releases, and a refusal is now true.
- Good: a genuine edit still refuses, because the record vouches for bytes rather than paths.
- Bad: the tool keeps state outside every repository it serves.
- Bad: a home installed before this change asks for `--force` once.

## Status

Implemented — `src/skills/record.rs` holds the record; `src/skills/installer.rs` plans, sweeps, and restores from it.
