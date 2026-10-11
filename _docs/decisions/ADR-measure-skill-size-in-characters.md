# Measure skill size in characters

## Context and Problem Statement

A skill's body was capped at 150 lines. A line counts the same whether it holds three words or three hundred, so the cap pushed authors toward long lines rather than short skills, and `rk-setup` reached the cap while holding 22,744 characters. The budget that matters is the context a host keeps: the first 5,000 tokens of a skill after compaction, and a body under 5,000 tokens by the format's own recommendation.

## Considered Options

- `Count Unicode characters: a body at most 12,000, a reference file at most 32,000, and a reference file over 8,000 opening with its sections` — chosen.
- `Keep the line cap` — rejected: on a corpus of 145 Markdown files, lines predicted the token count with a 12% median error and a 126% worst case.
- `Count words` — rejected: words erred 9% at the median, against 4% for characters.
- `Count tokens` — rejected: Claude's tokenizer is not public, and two public tokenizers disagree by 20% to 50% on one text.

## Decision Outcome

Chosen option: `characters`, counted as `chars().count()` over everything after the frontmatter's closing line. 12,000 characters is about 4,800 tokens at the conservative 2.5 characters per token the vendor states for its current tokenizer, inside the 5,000-token budget. 32,000 follows the 32 KiB limit a vendor sets for an instruction file. 8,000 is the target a rewrite aims for and fails no test. A long reference file opens with its sections because an agent previews only its head. Sources, in `../reference/REFERENCE-skill-format-sources.md`: the Agent Skills specification, the Claude Code skills page, the Claude models overview, and the Codex AGENTS.md page.

Enforced by `skill-format:size-is-counted-in-characters`.

## Consequences

- Good: the cap tracks the budget it protects, and a long line costs what it weighs.
- Good: a skill that outgrows its body moves knowledge into a reference file, which the format already loads on demand.
- Bad: a character count still only approximates tokens, and a table the formatter pads costs characters it does not cost meaning.

## Status

Accepted.
