# Skill Scopes Sources

External sources behind the user-scope decision in `SPEC-distribution.md` and the placement rule in `SPEC-placement.md`: where each supported coding agent reads skills, what each vendor documents about a system scope, and how same-name collisions resolve. Decision records state what was chosen; this file states what the choice was checked against.

Verified against the listed sources on 2026-09-03.

## The discovery matrix

| Agent       | User scope                                                                              | System scope                                                                                                                                       | Same-name collision                                              |
| ----------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Claude Code | `~/.claude/skills/<name>/SKILL.md`; reads no `.agents/skills` at any scope              | `/etc/claude-code/.claude/skills/` on Linux; `/Library/Application Support/ClaudeCode/.claude/skills/` on macOS; a `Program Files` path on Windows | Shadows: enterprise overrides personal overrides project         |
| Codex CLI   | `~/.agents/skills/`                                                                     | `/etc/codex/skills`                                                                                                                                | Duplicates: two skills sharing one name both appear in selectors |
| Gemini CLI  | `~/.gemini/skills/`, or the `~/.agents/skills/` alias taking precedence within the tier | None documented                                                                                                                                    | Tiered precedence                                                |
| Copilot CLI | `~/.copilot/skills/` or `~/.agents/skills/`                                             | None documented                                                                                                                                    | Not documented                                                   |

What the matrix establishes: user scope has a cross-tool convention, and the two roots the code declares in `src/skills.rs` — `.claude/skills` and `.agents/skills` — are minimal rather than arbitrary: `.agents/skills` covers Codex CLI, Gemini CLI, and Copilot CLI, and Claude Code alone needs `.claude/skills`. System scope has no convention: each vendor differs or publishes nothing, and the collision semantics differ too — Codex duplicates where Claude Code shadows — so a system-scope design would need exclusivity rules written per agent.

## Sources

- Claude Code skills and managed settings: <https://code.claude.com/docs/en/skills> and <https://code.claude.com/docs/en/managed-settings> — the `~/.claude/skills` user root, the enterprise root per platform, and the shadowing order.
- Codex CLI skills: <https://learn.chatgpt.com/docs/build-skills> — `~/.agents/skills/`, `/etc/codex/skills`, and the duplicate behavior on same-name skills.
- Gemini CLI skills: <https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/skills.md> — `~/.gemini/skills/` with the `~/.agents/skills/` alias taking precedence within a tier; no system path documented.
- Copilot CLI skills: <https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-skills> — `~/.copilot/skills/` and `~/.agents/skills/`, the latter landed via github/copilot-cli#2230 (closed completed 2026-03-23). The `skillDirectories` setting and `COPILOT_SKILLS_DIRS` variable are user-scope configuration, not package-ownable paths.
