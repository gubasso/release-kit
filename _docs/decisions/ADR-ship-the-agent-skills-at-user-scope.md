# Ship the agent skills at user scope

## Context and Problem Statement

Coding agents discover operating knowledge through skill files in agent-specific directories. A project needs every agent — Claude Code, Codex, Gemini CLI, Copilot — to run a release the canonical way, offline. Where those files live decides whether that works.

## Considered Options

- `Embed the skills in the binary and install them at user scope` — chosen.
- `Land the skills into each target through rk init` — rejected: an agent resolves a skill by name across scopes, so a project copy beside a home copy offers two entries under one name.
- `Per-agent skill variants with a render step` — rejected: two renders of one skill drift.
- `Publish the skills as a separate download` — rejected: it reintroduces the network dependency embedding removed.
- `Claude-specific overlay fields on the shared body` — deferred: revisit if an agent requires a field the portable Agent Skills format cannot carry.

## Decision Outcome

Chosen option: `embed the skills in the binary and install them at user scope`. The Agent Skills format is the portable intersection every listed agent reads, and `include_dir!` keeps the shipped bytes identical to the authored `skills/` tree. One installed binary already serves every repository, so the skills routing into it belong at the same scope: `rk skill install` writes `~/.claude/skills/` and `~/.agents/skills/`, and `rk init` lands none.

The skills carry no canon: they route to `rk method` and `rk binding`, so an agent reads the version-locked canon from the binary it invokes.

Enforced by `distribution:skills-are-part-of-the-payload`, `distribution:a-skill-has-one-owner`, and `distribution:a-skill-obeys-the-portable-format`.

## Consequences

- Good: one authored file serves every agent byte-identically, and a skill name resolves to one file.
- Good: the skills upgrade with the binary, so they cannot describe a CLI they no longer match.
- Bad: no skill may use a vendor-only field.
- Bad: a repository cannot pin the skill text to a canon version of its own.

## Status

Implemented — `src/skills.rs` embeds the payload and `rk init` projects no skill.
