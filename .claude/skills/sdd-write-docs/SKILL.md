---
name: sdd-write-docs
description: Authors documentation governed by spec-driven-docs, covering current specs, immutable decision records, agent digests, comparison documents, and known-issue records. Use when writing or editing docs in a repository that carries a .spec-driven-docs/ directory, when a pre-commit documentation gate fails citing a domain:rule ID, or when a task mentions SPEC files, ADRs, EARS requirements, sdd method, sdd spec, or sdd template.
license: CC-BY-4.0
compatibility: Requires the sdd binary on PATH; install with cargo install spec-driven-docs or cargo binstall spec-driven-docs.
---

# sdd-write-docs

Author documentation by the spec-driven-docs method. The rules live in the instance's specs and in the binary; read them with the CLI instead of restating them, because the spec text is the owner.

## Route to the rules

| Need                                | Command                |
| ----------------------------------- | ---------------------- |
| List method chapters                | `sdd method --list`    |
| Read a chapter                      | `sdd method <chapter>` |
| List rule domains                   | `sdd spec --list`      |
| Read the binding rules for a domain | `sdd spec <domain>`    |
| List templates                      | `sdd template --list`  |
| Start a new document                | `sdd template <name>`  |

## Before writing

1. Run `sdd status --target . --json`; confirm the repository is an instance and note `docs_root`.
2. Load the spec for each domain the change touches: `sdd spec docs-format` always, `sdd spec docs-specs` for SPEC files, `sdd spec decision-records` for ADRs, `sdd spec comparison-docs` for comparisons, `sdd spec known-issues` for known-issue records.
3. Start from the matching template: `sdd template spec`, `sdd template adr`, `sdd template comparison`, or `sdd template agents-digest`.

## Format defaults

- One source line per paragraph or list item; no hard-wrapped prose.
- Headings, lists, tables, fenced code with a language, inline code, and links; no bold or italic text.
- State the present; decision records are the only history-bearing document class.
- A new requirement carries five parts: a title, a `domain:rule` ID, one EARS statement, a GIVEN/WHEN/THEN scenario, and a `Verify:` line naming a live command.

## Gate triage

A failing gate or `sdd verify` message cites a `domain:rule` ID. Read the rule with `sdd spec <domain>`, fix the document to satisfy the cited sentence, and re-run the failing command. A budget is never widened and a gate is never removed to admit a document.

## Handoff

```bash
sdd verify --target .
pre-commit run --all-files
```

Both pass before the change is done.
