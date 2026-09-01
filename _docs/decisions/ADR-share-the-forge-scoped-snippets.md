# Share the forge-scoped snippets

## Context and Problem Statement

The title checks are forge-scoped and technology-independent: `pr-title.yml` is identical for every GitHub pair, `mr-title.yml` for every GitLab pair. `snippets/` is scoped by `(technology, forge)` pair, so the natural landing spot was five copies of two files, and the one-owner rule in `AGENTS.md` forbids exactly that.

## Considered Options

- `A _shared/<forge> zone composed into every pair` — chosen.
- `A copy per pair` — rejected: five owners for one fact, and a title-regex fix becomes five edits with a test to keep them identical.
- `A pseudo-technology named shared` — rejected: `rk init --tech shared` would then be a selectable landing that lands half a convention.

## Decision Outcome

Chosen option: `the shared zone`. `snippets/_shared/<forge>` holds the forge's technology-independent files, and `pair_files` composes it with the selected pair, shared files first. The underscore keeps it out of the technology namespace: the bindings listing never names it, and selecting it as a tech refuses as unknown. A destination the shared zone and a pair both ship refuses as a payload defect rather than either zone silently winning, so ownership stays unambiguous per `landing:the-shared-zone-composes-into-every-pair`.

The kind table closes over the shared files like every snippet, so a shared file without a declared kind fails the same test a pair file does.

## Consequences

- Good: one owner per shared fact; a title-check fix is one edit landing everywhere.
- Bad: a pair's landed file list is no longer readable from its own directory alone; `rk snippet --list` and the projection stay the honest listing.

## Status

Implemented — `src/landing.rs` composes the zone; `snippets/_shared/` holds the two checks.
