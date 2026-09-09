# Template — Known-issue record

Copy the block below to `<root>/reference/known-issues/KI-<slug>.md`. The filename is the case id, and it is what a suppression cites from source. A record opens at `state: investigating` and `filing: gathering`, and it is deleted, not archived, when its retire condition is met.

```markdown
---
upstream: <the issue URL, or the tracker where the case will be filed>
affects: <the component this project cannot fix>
state: <investigating | mitigated | masked | monitoring>
filing: <gathering | ready | filed | deferred>
workaround: <what this project does instead>
retire_when: <the condition that removes a mask; omit for a mitigation>
---

# <One line naming the defect, not the symptom>

## Symptom

<What a reader meets first: the failing command, the wrong output, the log line.>

## How it works

<Name the two or three moving parts. Run one concrete case. Show what each step leaves behind — the input, the command that acts on it, the state after it. Close with the cause in one line, the reading a triager reaches for first and why it is wrong, and the smallest fix.>

## Signal

<The text that identifies a recurrence, so the next reader recognizes it.>

## Workaround

<What the tree carries now, and where it lives.>

## Report

<Only where filing is `filed`: the body as it was filed, in the tracker's own markup. A Bugzilla body sits in a fence and stays at or below 79 columns.>
```

Checks before committing:

- The filename is `KI-<slug>.md`, with a slug that does not open with a digit.
- The record states exactly one `state:` and exactly one `filing:`, each from its vocabulary.
- A masked record carries a retire condition, and a mitigated one carries none.
- The mechanism is a run a reader can follow, not a restatement of the defect.
- A filed record names its upstream issue and carries the body it was filed with.
