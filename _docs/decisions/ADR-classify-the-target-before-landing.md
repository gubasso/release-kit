# Classify the target before landing

## Context and Problem Statement

The setup and migration skills routed on one signal: whether the target carried a landing record. A repository already releasing through another tool, or through a gated two-branch flow, read as a fresh start, so a setup could land the payload beside the old mechanism and leave two release paths. The migration procedure also had no owner a skill may route to, as `distribution:a-skill-routes-and-never-restates` requires.

## Considered Options

- `a read-only classification verb, and a migration chapter paired with its runbook` — chosen.
- `agent judgment in skill prose` — rejected: "some release setup" is a feel, not a rule, and no test can hold it.
- `a chapter with no runbook` — rejected: a lone chapter puts the commands back in the skill.
- `a binary-owned migration state machine` — deferred: revisit if an interrupted migration loses state in practice, or a second consumer needs the inventory machine-readable.

## Decision Outcome

Chosen option: `a read-only classification verb, and a migration chapter paired with its runbook`. `rk assess` gathers the evidence — the record, the technology and forge, other tools' release markers, the payload destinations present, the tags, the long-lived branches — and computes one verdict by an explicit, unit-tested rule: `brownfield` on any release mechanism, `greenfield` on nothing, `needs-decision` on release activity no mechanism explains. A recorded target routes by its status report whatever the verdict says. The skills and the pre-flight gate route by the verdict; the chapter and runbook own the procedure.

Enforced by `landing:a-landing-classifies-its-target-first` and `distribution:a-runbook-renders-the-spine`.

## Consequences

- Good: a target running another release tool stops reading as greenfield, so a setup cannot start a second release path.
- Good: the verdict is evidence a plan can cite and a test can hold, and the migration skill routes and carries judgment only.
- Bad: the recognized markers and branch names are a fixed list; an unrecognized mechanism costs the operator a question.

## Status

Implemented; `src/assess.rs`, `method/10-migration.md`, and `runbooks/migration.md` enact it.
