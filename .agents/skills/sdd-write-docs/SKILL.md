---
name: sdd-write-docs
description: Authors documentation governed by spec-driven-docs, covering current specs, immutable decision records, agent digests, comparison documents, known-issue records, and step-by-step guides, and fixes a document a documentation gate fails. Use when writing or editing docs in a repository that carries a spec-driven-docs instance, when a hook documentation gate or sdd verify fails citing a domain:rule ID, or when a task mentions SPEC files, ADRs, EARS requirements, sdd method, sdd spec, or sdd template.
license: CC-BY-4.0
compatibility: Requires the sdd binary on PATH and a landed spec-driven-docs instance; the sdd-setup skill provisions the binary and lands the instance.
---

# sdd-write-docs

Write, edit, and fix the documents of a spec-driven-docs instance by the rules the instance adopted. The request picks the task, the adopted specs bind every line, and the delivered gates prove the result.

## Pre-flight

1. Before any other action, read `references/pre-flight-gate.md` and run it whatever the request carries. No flag skips it.
2. Hand the task to the `sdd-setup` skill on any row below, state the evidence, and write nothing, because only that skill provisions `sdd` and lands an instance.

| Finding                           | Evidence                                      |
| --------------------------------- | --------------------------------------------- |
| No `sdd` on PATH                  | `command -v sdd` finds nothing                |
| No instance at the target         | `instance` reads `false` in the status report |
| An instance newer than the binary | `alignment` reads `instance-newer`            |

1. Where the pre-flight returns a diagnostic instead of a status report, write one plan that carries the diagnostic and blocks on it.

## General rules

1. Read a rule from the adopted `SPEC-<domain>.md` in the directory `paths.active.destinations.specs` names, because the adopted text is what the instance owns. Read `sdd spec <domain>` only for a domain the instance has not adopted.
2. Take every path from the `paths` section of `sdd status --target . --json`. Never spell a path by hand.
3. Route, never restate: find a topic with `sdd docs`, then `sdd docs <topic>`, a chapter with `sdd method <chapter>`, a template with `sdd template <name>`, and the known-issue cases with `sdd ki list`. Never copy a chapter or a spec into a document or a plan.
4. Before you author or edit prose, read the writing source the instance selects, which the managed documentation block in the root agent digest names: `sdd method writing-style` unless the declaration selects another.
5. Never widen a budget or remove a gate to admit a document, because a gate that bends to the document no longer judges it.

## Gate

Read the task from the request.

| Request                                                      | Tasks                  |
| ------------------------------------------------------------ | ---------------------- |
| Write a new document, or edit one                            | `write`                |
| A gate or `sdd verify` failure that cites a `domain:rule` ID | `triage`, then `write` |

- Ask where the request names no document and no failure: state what the target holds, and wait.
- Hand a failure that cites no document, such as a managed file or a marker block, to the `sdd-setup` skill, whose `repair` task owns it.

After the pre-flight, read `references/plan-gate.md` and hold it for the whole task.

## Tasks

### Triage

1. Read the rule the failure cites, by general rule 1.
2. Name the sentence of the rule the document breaks, and the document and line the failure names.
3. Fix that document with the `write` task, to satisfy the cited sentence, then run the failing command again.

### Write

1. Load the adopted spec for each domain the change touches: `SPEC-docs-format.md` always, `SPEC-docs-foundations.md` for a new document's name and home, `SPEC-docs-specs.md` for a `SPEC-*.md` file, `SPEC-decision-records.md` for a decision record, `SPEC-comparison-docs.md` for a comparison, `SPEC-known-issues.md` for a known-issue record, and `SPEC-guides.md` for a step-by-step guide.
2. Start a new document from its template: `sdd template spec`, `sdd template adr`, `sdd template comparison`, `sdd template agents-digest`, `sdd template guide`, or `sdd template known-issue`.
3. Name a new document by a kebab-case slug drawn from its subject, never by a number or a position in a sequence, because two branches can claim one number and not one subject. Where the subject carries a number, put the word first: `roadmap-2026.md`, never `2026-roadmap.md`.
4. Where a new document carries no kind prefix, add it to the `README.md` of its directory, and create that file where the directory has none, because no gate judges an unprefixed name.
5. Write each requirement with its five parts: a title, a `domain:rule` ID, one EARS statement, a `GIVEN`/`WHEN`/`THEN` scenario, and a `Verify:` line that names a live command.

## Post-flight

1. Run `sdd verify --target .`, then `prek run --all-files`. Where the second command fails because the host lacks prek, ask the operator to install it, then run the command again. Where either fails citing a rule inside a document, run the `triage` task again. Where it fails on a managed file, keep the change and hand that failure to the `sdd-setup` skill.
2. Report from observation, in this template.

```text
Result: <the exact sdd verify line, or the first failing line>
Documents:
  - <path>: <written | edited | fixed>; rules: <the domain:rule IDs it answers>
Still the operator's:
  - <exact command>: <what it changes>
Next: <one action>
```
