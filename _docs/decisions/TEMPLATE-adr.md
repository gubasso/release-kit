# Template — Decision record

Copy the block below to `<root>/decisions/ADR-<slug>.md`. A project keeping its own copy names it `TEMPLATE-adr.md`, so the prefix distinguishes the seed from the records it seeds and no template reads as a decision the project made. A filled body is at or below 350 words.

```markdown
# <Short title naming the choice, not the task>

## Context and Problem Statement

<Two or three sentences. What problem, and why it matters.>

## Considered Options

- `<option 1>` — chosen.
- `<option 2>` — rejected: <one sentence: why it lost>.
- `<option 3>` — deferred: revisit if <condition that would reopen it>.

## Decision Outcome

Chosen option: `<option 1>` — <one sentence: why>.

<If this decision binds behavior, name the requirement that enforces it: Enforced by `<domain-slug>:<rule-slug>`.>

## Consequences

- Good: <trade-off gained>
- Bad: <trade-off accepted>

## Status

<Proposed | Accepted | Implemented | Deprecated | Superseded | Rejected>

<If Implemented: link what enacts it. If Superseded: link the successor. If Deprecated: say why it
stopped applying.>
```

Checks before committing:

- The filename carries no digit and will never be renamed.
- The title names the choice, not the implementation task.
- Every considered option carries a disposition, and every deferral names its reopening condition.
- An enforceable consequence is stated as a requirement in a spec, cited above.
