# Carry every skill file in its package

## Context and Problem Statement

The shared gates installed once under `~/.local/state/release-kit/skills/shared/`, and every skill named them by that absolute path. A skill then read outside its own directory: a sandboxed agent can prompt on that read, and an agent with no home access cannot take it. The installer also wrote `SKILL.md` alone, so a skill could not ship a reference file of its own.

## Considered Options

- `copy the shared gates into each package, beside the skill's own references` — chosen.
- `keep one shared root and name it by absolute path` — rejected: the format defines no location two skills share, and a read outside the skill directory is a hop it never promised.
- `link each package to one canonical copy` — rejected: the installers that link record links that never reach an agent root and copies that turn back into links, and `distribution:a-skill-destination-is-a-regular-file` already requires a regular file.

## Decision Outcome

Chosen option: `copy the shared gates into each package, beside the skill's own references`. `rk skill install` writes `SKILL.md`, the skill's own `references/`, and both gates under `references/`, per agent root, and the record vouches for each file. A skill names each gate as `references/<file>`. A skill file that takes a gate's name fails the build. The first apply removes the old shared root where the record vouches for it.

The [earlier record](./ADR-share-skill-artifacts-outside-the-skill-directory.md) rejected copies because an author would keep one per skill. Copies an installer writes from one embedded source keep one authored file, so a fix still lands once.

Checked against the Agent Skills specification, the Atlassian TWG CLI, `gh skill install`, and the Vercel `skills` CLI, in `../reference/REFERENCE-skill-format-sources.md`.

Enforced by `distribution:shared-skill-artifacts-have-one-home` and `skill-format:shared-content-is-authored-once`.

## Consequences

- Good: each installed skill reads only inside its own directory, on every agent.
- Bad: a home holds eight gate files, two per package under each of two agent roots, where it held two.

## Status

Implemented: `src/skills.rs`, `src/skills/installer.rs`, `src/probes.rs`, `src/stage.rs`.
