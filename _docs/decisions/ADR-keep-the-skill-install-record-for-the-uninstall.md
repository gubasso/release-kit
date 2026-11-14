# Keep the skill install record for the uninstall

## Context and Problem Statement

Releases before 0.10.0 installed `rk-setup` and `rk-release` under `$HOME/.claude/skills` and `$HOME/.agents/skills`, and recorded each written file with its digest in `$HOME/.local/state/release-kit/skills.sha256`. Since 0.10.0 the skills land in each project, and no rk verb writes a skill under the home. The home copies stay behind, and Claude Code takes a home copy over the project's. Something must remove them without removing a file the user wrote or edited.

## Considered Options

- `Keep the record, read only by the uninstall` — chosen.
- `Drop the record with the install` — rejected: without it, an old release's copy and a user's edit look alike, so the uninstall either refuses on every old copy or removes edits.
- `Remove the whole package folder` — rejected: it deletes files the user added beside a skill.

## Decision Outcome

Chosen option: `keep the record, read only by the uninstall`. `rk skill uninstall --apply` removes a home file only where its bytes match the embedded package or the digest the record holds for it, and keeps any folder that still holds another file. No verb adds an entry to the record any more.

Enforced by `distribution:skill-uninstall-removes-only-what-it-wrote`.

## Consequences

- Good: one run per host clears the copies of every earlier release, and a user's edit or note stays.
- Bad: rk still reads state outside every repository it serves, until the last host has run the uninstall.

## Status

Accepted
