# Find the bot key at a default path

## Context and Problem Statement

`rk` read the path of the release bot App key from `RK_BOT_PRIVATE_KEY_FILE` alone, so each project exported the same machine fact. Where the process could not read the key file, `rk setup check` exited 1 with no clear remedy.

## Considered Options

- `A default path named by the App ID, with two overrides` — chosen.
- `A default path named by the repository owner` — rejected: one App can serve the repositories of two owners.
- `The key path in the committed .release-kit/config.toml` — rejected: a path on one machine is a coordinate that no other machine shares.
- `A setup step that imports the key` — rejected: the import takes a path and changes nothing on a forge, so it is the verb `rk setup import-key`.
- `A recorded key-rotation date` — rejected: the date is a fact of the machine and of the App, not of a project. A tool that manages the file can rewrite its file time at any time.
- `An unreadable key as a gap that fails the check` — rejected: it is an unmet requirement with a known remedy, and it proves nothing wrong on the forge.

## Decision Outcome

Chosen option: `a default path named by the App ID`, because one key signs for exactly one App, and one machine then serves every project that shares the App. The path is `${XDG_CONFIG_HOME:-$HOME/.config}/release-kit/bot/github/<app-id>.pem`. The variable wins for one run, then the user configuration entry, then the default. A key file that the process cannot read reads `unreadable`, and a check with no other gap exits 0.

Enforced by `bot-key:the-key-has-one-default-path`, `bot-key:three-sources-name-the-key-in-a-fixed-order`, `bot-key:the-import-checks-and-writes-the-key-atomically`, and `bot-key:an-unreadable-key-is-an-unmet-requirement`.

## Consequences

- Good: a project that commits `setup.bot.app_id` needs no export to set up its bot.
- Good: a key that another tool manages stays where it is, and the import refuses to replace its link.
- Bad: the default path is a contract. A change to it moves every key that sits there.

## Status

Accepted.
