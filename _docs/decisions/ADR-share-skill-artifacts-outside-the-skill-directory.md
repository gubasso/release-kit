# Share skill artifacts outside the skill directory

## Context and Problem Statement

Every skill drives destructive operations, so each needs the same plan gate, and this repository keeps each durable fact in one owner. The Agent Skills specification makes the skill directory the unit of distribution and defines no location two skills share: discovery scans a skills root for subdirectories holding a `SKILL.md`, so a `_shared/` beside them is ignored. The installer writes two roots, `~/.claude/skills` and `~/.agents/skills`, so no relative path names one file from both.

## Considered Options

- `One copy under the rk-owned state root` — chosen.
- `A copy bundled into every skill directory, as references/plan-gate.md` — rejected: the spec's own idiom, and it survives packaging, but it makes the gate six files across two roots. These skills declare `Requires the rk binary on PATH`, so they never travel as an upload; the property bundling buys is unusable here.
- `Inline the gate in each SKILL.md` — rejected: cheapest, but it puts one block in three authored files.
- `Ship the skills as a Claude Code plugin` — rejected: a real namespace, but it renames every command and abandons `~/.agents/skills`, the interoperability the portable format exists to hold.

## Decision Outcome

The shared artifacts are authored under one payload root, `skill-shared/`, and installed once to `~/.local/state/release-kit/skills/shared/`. Every skill names that absolute path, so one authored line serves both agent families. It is home-relative, not `XDG_STATE_HOME`-relative, for the reason the record already gives: the skills reading it live under `$HOME/.claude` and `$HOME/.agents`, which no XDG variable moves. An install writes it whichever agent it was asked for; an uninstall keeps it while any root still holds a skill that reads it.

Enforced by `distribution:shared-skill-artifacts-have-one-home`.

## Consequences

- Good: the gate is one file to correct, and install, uninstall, and sweep own it as they own a `SKILL.md`.
- Bad: it sits outside the skill directory, so an agent allowlisting skill directories may prompt once to read it, and a skill copied out of this installation loses its gate.

## Status

Implemented — `skill-shared/`, `src/skills/installer.rs`.
