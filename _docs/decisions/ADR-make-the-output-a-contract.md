# Make the output a contract

## Context and Problem Statement

The product's own premise is that coding agents drive the CLI as the deterministic layer, and an agent driving it scraped prose: handlers printed free-form lines, no verb had a machine form, and a wording improvement was a breaking change nobody noticed. An output contract that exists for one verb and not its siblings is two personalities in one binary.

## Considered Options

- `One CLI-wide contract, landed before the setup verbs exist` — chosen.
- `Design output per verb as each lands` — rejected: the first verbs never converge, and every later one copies whichever it saw first.
- `Retrofit a contract after the setup verbs ship` — rejected: retrofitting under a shipped verb changes output agents already parse, which is the breakage the contract exists to prevent.

## Decision Outcome

Chosen option: `one CLI-wide contract, landed first`. Its parts: stdout carries the result and only the result, stderr everything else, in both modes; `--json` is declared per command; long-running verbs will emit NDJSON events opening with a schema event; every failure carries a `reason` from one closed, append-only vocabulary beside the exit-code matrix; a diagnostic is a value with named parts rendered at the boundary; a success ends with a `Next:` block; every schema is versioned and snapshot-tested from day one. `rk init` and `rk skill` moved onto the boundary in the same change, human lines unchanged, and no handler prints directly — a source-scan test holds that. The landing writer became temp-plus-rename in passing, and one probe catalog serves `rk doctor` and every future prerequisite guard.

Enforced by `distribution:machine-output-declares-its-schema`.

## Consequences

- Good: an agent branches on `reason` and schema fields instead of scraping prose, and a wording change is no longer an interface change.
- Good: every future verb inherits the boundary instead of choosing its own output shape.
- Bad: every new machine field is a commitment; removing or renaming one requires a schema-version bump.

## Status

Implemented — `src/output.rs` is the boundary, `src/diagnostic.rs` the vocabulary, and the snapshot tests hold every schema.
