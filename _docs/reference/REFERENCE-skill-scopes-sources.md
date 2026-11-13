# Skill Scopes Sources

External sources behind the project-scope decision in `SPEC-distribution.md` and the placement rule in `SPEC-placement.md`: where each supported coding agent reads skills, what each vendor documents about each scope, and how same-name collisions resolve. Decision records state what was chosen. This file states what the choice was checked against.

Verified against the listed sources on 2026-11-13. The discovery matrix was first verified on 2026-09-03, and the project-scope facts below were added on 2026-11-13.

## The discovery matrix

| Agent       | User scope                                                                              | System scope                                                                                                                                       | Same-name collision                                              |
| ----------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Claude Code | `~/.claude/skills/<name>/SKILL.md`; reads no `.agents/skills` at any scope              | `/etc/claude-code/.claude/skills/` on Linux; `/Library/Application Support/ClaudeCode/.claude/skills/` on macOS; a `Program Files` path on Windows | Shadows: enterprise overrides personal overrides project         |
| Codex CLI   | `~/.agents/skills/`                                                                     | `/etc/codex/skills`                                                                                                                                | Duplicates: two skills sharing one name both appear in selectors |
| Gemini CLI  | `~/.gemini/skills/`, or the `~/.agents/skills/` alias taking precedence within the tier | None documented                                                                                                                                    | Tiered precedence                                                |
| Copilot CLI | `~/.copilot/skills/` or `~/.agents/skills/`                                             | None documented                                                                                                                                    | Not documented                                                   |

What the matrix establishes: each agent reads a project scope and a user scope, and the two roots that `src/skills.rs` declares, `.claude/skills` and `.agents/skills`, are minimal rather than arbitrary. `.agents/skills` covers Codex CLI, Gemini CLI, and Copilot CLI, and Claude Code alone needs `.claude/skills`. The landing writes both roots inside the target. System scope has no convention: each vendor differs or publishes nothing, and the collision semantics differ too, because Codex duplicates where Claude Code shadows.

## Project scope and collisions

Read on 2026-11-13.

- Claude Code loads a project skill from `.claude/skills/<skill-name>/SKILL.md`, and the page says to commit it "so your team gets it too". A collision resolves as "Enterprise over personal, and personal over project", so a copy under `~/.claude/skills/` wins over the project copy. Claude Code "picks up the change within the current session, without a restart" for a change under `~/.claude/skills/` or the project `.claude/skills/`. Source: <https://code.claude.com/docs/en/skills>.
- Codex "scans `.agents/skills` in every directory from your current working directory up to the repository root", and reads `$HOME/.agents/skills` at user scope. When two skills share a name, Codex keeps both, and both can appear in selectors. "If an update doesn't appear, restart Codex." Source: <https://learn.chatgpt.com/docs/build-skills>.
- Prisma ships its skills inside the package that the project installs. `prisma skills sync` writes one copy per agent harness, `.claude/skills` and `.agents/skills` among them. "The copies are ordinary files that git tracks: commit them so agents have the skills on a fresh clone." "A directory sync does not own, such as a hand-written skill whose name collides, is left intact and reported as refused." Sources: <https://www.prisma.io/docs/cli/skills>, <https://github.com/prisma/prisma-cli/blob/main/packages/cli/src/lib/skills/sync.ts>.
- `gh skill install` takes `--scope <string> (default "project")`, and project scope installs inside the current git repository. Source: <https://cli.github.com/manual/gh_skill_install>.
- cargo-dist refuses a binary whose version differs from `cargo-dist-version` with "You're running dist {running_version}, but 'cargo-dist-version = {config_version}' is set in your Cargo.toml", and its help reads "Rerun 'dist init' to update to this version." `dist init` lands and upgrades. Sources: <https://github.com/axodotdev/cargo-dist/blob/main/cargo-dist/src/errors.rs>, <https://axodotdev.github.io/cargo-dist/book/updating.html>.

Bearing: the landing writes both roots at project scope. A user-scope copy left on the host shadows the project copy in Claude Code and duplicates it in Codex, so `rk doctor` names it under `distribution:the-doctor-names-a-shadowing-user-copy`.

## Sources

- Claude Code skills and managed settings: <https://code.claude.com/docs/en/skills> and <https://code.claude.com/docs/en/managed-settings> — the `~/.claude/skills` user root, the enterprise root per platform, and the shadowing order.
- Codex CLI skills: <https://learn.chatgpt.com/docs/build-skills> — `~/.agents/skills/`, `/etc/codex/skills`, and the duplicate behavior on same-name skills.
- Gemini CLI skills: <https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/skills.md> — `~/.gemini/skills/` with the `~/.agents/skills/` alias taking precedence within a tier; no system path documented.
- Copilot CLI skills: <https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-skills> — `~/.copilot/skills/` and `~/.agents/skills/`, the latter landed via github/copilot-cli#2230 (closed completed 2026-03-23). The `skillDirectories` setting and `COPILOT_SKILLS_DIRS` variable are user-scope configuration, not package-ownable paths.
