# Template — Spec

Copy the block below to `<root>/specs/SPEC-<domain>.md`. One spec per domain, at or below 300 lines, with a `<!--TOC-->` marker added above 100 lines. The shape is fixed and gated.

```markdown
# <Domain> Specification

## Purpose

<One paragraph: what this domain covers, and where its boundary runs against neighbouring domains. No rules here.>

## Requirements

### `<domain-slug>:<rule-slug>` — <Behavior title, as a statement>

<One sentence. One of: `The <subject> MUST <response>.` | `While <precondition>, the <subject> MUST
<response>.` | `When <trigger>, the <subject> MUST <response>.` | `Where <feature is included>, the
<subject> MUST <response>.` | `If <trigger>, then the <subject> MUST <response>.`>

#### Scenario: <The case that would be argued about>

- GIVEN <initial state>
- WHEN <action>
- THEN <expected outcome>

Verify: `<command that exits non-zero on violation>`

### `<domain-slug>:<next-rule-slug>` — <next, most consequential first>

...
```

Checks before committing:

- The most consequential requirement is first.
- No narrative sits between two requirements.
- Every requirement has all five parts.
- Prohibitions are at or below five, each paired with the action that replaces it.
- Every rule ID is unique across the project.
