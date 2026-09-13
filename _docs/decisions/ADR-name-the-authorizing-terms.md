# Name the authorizing terms

## Context and Problem Statement

`ADR-bound-the-agents-initiative.md` binds an agent in a target to the actions the operator's request names, and accepted one cost: an operator who wants a whole run asks for it per task. Nothing writes that expansion down, so the operator retypes the same list at the start of every task.

## Considered Options

- `A landed glossary with a release-kit block and a target-owned region` — chosen.
- `More rows in the routing block itself` — rejected: the block renders whole, so the operator gets no region of their own inside it.
- `A vendor import in AGENTS.md` — rejected: that syntax belongs to one vendor, which is why the same record refused a permission file.
- `A flag in the request, on the shape of --no-plan` — rejected: a flag reaches the skills alone, and the boundary binds every agent in the target.
- `Nothing, and the operator keeps retyping` — rejected: the retyping is the pressure the boundary fails under.

## Decision Outcome

Chosen option: `a landed glossary` — `GLOSSARY.md` at the target root, spliced between markers, with every line outside them the target's own. The name is the plain one, because the file is the target's vocabulary and one region of it is release-kit's.

A term is a hyphenated compound no ordinary sentence produces, carries no prefix, and expands to a named list of actions rather than to a category. `implement-and-request` stops before the merge, `implement-and-merge` stops before the release, and `full-implement` runs to a published version. Each contains the one above it. A request carrying a term names every action in that list, so the boundary holds and the operator wrote the expansion.

Enforced by `landing:a-block-destination-owns-its-marked-lines-alone`, `landing:the-routing-block-bounds-the-agents-initiative`, and `distribution:a-skill-plans-before-it-acts`.

## Consequences

- Good: the operator authorizes a whole run in one word, and keeps their own terms in the same file.
- Bad: every landed target gains a root file at its next upgrade, and a term expanding to more than the operator expected is a misreading no mechanism catches.

## Status

Proposed
