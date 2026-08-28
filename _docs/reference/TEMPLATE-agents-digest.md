# Template — Author-instructions digest

Copy the block below to `<directory>/AGENTS.md`. A digest routes; it never owns a rule. At or below 100 lines at the repository root and 150 lines in a subtree.

Where a tool reads only its own filename, add a one-line `<directory>/CLAUDE.md` containing `@AGENTS.md` beside it. The import path resolves relative to the importing file, so the sibling is `@AGENTS.md`, not a repo-root path.

```markdown
# AGENTS

## Scope

<One or two sentences: what this directory covers, and what is explicitly outside it.>

## How to use this

Load this file, find the owning document below, then read that document and the sources it names.

## Where the rules live

| Question the agent arrives with | Owning document                       |
| ------------------------------- | ------------------------------------- |
| <question>                      | `<root>/specs/SPEC-<domain>.md`       |
| <question>                      | `<root>/specs/SPEC-<other-domain>.md` |

## Non-negotiables

- <Rule that binds every file here, one imperative sentence.>
- <At most a handful. Everything else is a requirement in a spec.>

## Maintenance

- Regenerate when the rules this routes to change.
- This digest is a router, never a rules home; the owning spec wins on disagreement.
```
