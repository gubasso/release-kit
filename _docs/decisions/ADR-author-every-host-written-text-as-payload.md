# Author every host-written text as payload

## Context and Problem Statement

The binary writes whole human-faced texts outside `snippets/`: the routing and hook blocks it splices into a target, the worktree guard entry, and the post-merge reminder body it installs on a host. All lived as Rust string literals in `src/landing.rs` and `src/setup/branch_reminder.rs`, so the artifacts a reader most needs to review as prose were the ones not authored as files. Where do these texts live so authored bytes and written bytes cannot drift?

## Considered Options

- `a blocks/ payload root of authored template files` — chosen.
- `keep the texts as source literals` — rejected: a literal hides prose from formatters, linters, and reviewers, and grows the sources with content that is not code.
- `fold them into snippets/` — rejected: `snippets/` is scoped by `(technology, forge)` and landed whole by `rk init`; these texts are spliced or host-written and belong to no pair.
- `one file per rendered variant` — rejected: the variants share every byte but one line, and two authored copies of the shared bytes is the drift the move exists to end.

## Decision Outcome

Chosen option: `a blocks/ payload root of authored template files` — the tenth entry in `src/payload_roots.rs`, embedded like every root, with `.in` suffixes keeping the token-bearing fragments away from formatters that would fold or reject them. The `.in` readers strip the one hook-enforced final newline; the hook body ships its newline and is written verbatim; tokens, markers, and the branch grammar stay code. Enforced by `distribution:a-human-faced-artifact-is-authored-text`.

This supersedes earlier records' location claims: the blocks and the reminder body are authored under `blocks/`, and `src/landing.rs` and `src/setup/branch_reminder.rs` hold only the rendering.

## Consequences

- Good: every human-faced text is reviewable as text, the source scan refuses a new literal, and byte-equality tests pin the round trip so no landed target reads as drift.
- Bad: the template's token lines are byte-fragile — one sits mid-line before YAML indentation — so the equality tests are load-bearing, not decorative.

## Status

Implemented — `blocks/` is embedded; `rk payload` lists it.
