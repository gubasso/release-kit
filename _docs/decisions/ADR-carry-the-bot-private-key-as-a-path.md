# Carry the bot private key as a path

## Context and Problem Statement

`bot-secrets` stored a GitHub App private key the operator exported as `RK_BOT_PRIVATE_KEY="$(cat key.pem)"`, which satisfied `forge-setup:a-secret-never-reaches-argv`, whose weaker half is the environment: the block is readable at `/proc/<pid>/environ`, an `export` hands the key to every later child of that shell, a core file keeps it, and a pasted key lands in shell history. An App private key is the worst credential to hold so: it downloads once, and only a browser rotates it.

## Considered Options

- `The path is named to rk; rk reads the file and writes the step's stdin` — chosen.
- `The path is forwarded to the step, which redirects the file into the CLI` — rejected: `rk` reads the file anyway, so a redirect reopens the path and stores bytes it never checked.
- `Keep the contents variable, alone or beside the path` — rejected: not the operator's risk to carry, and it honors the stale export.

## Decision Outcome

`RK_BOT_PRIVATE_KEY_FILE` names the `.pem`: the environment carries a path, never key material, and the path reaches no child either. `RK_BOT_PRIVATE_KEY` is refused wherever a run opens, naming its replacement.

`rk` opens the file once and that handle does everything: it refuses a wrong kind, mode, size, or encoding; it is the needle behind the journal's `redacted` claim; and it is the bytes written to the step's stdin, the way both forge CLIs document taking a secret. What was checked is what is stored.

`RK_BOT_APP_ID` and `RK_BOT_TOKEN` stay values: an identifier is public, a token short-lived and rotated by a command.

Enforced by `forge-setup:key-material-never-reaches-the-environment`.

## Consequences

- Good: no environment block holds key material, so no descendant inherits it and no `/proc` reader finds it; a wrong kind, mode, size, label, or encoding refuses before any forge call.
- Bad: `rk` holds the bytes while it runs, so a dump taken then carries them; scrubbing on drop bounds that window without closing it.
- Bad: the `.pem` must stay at mode `600` outside the repository.

## Status

Implemented — `src/setup/secrets.rs`, `setup/github/bot-secrets`.
