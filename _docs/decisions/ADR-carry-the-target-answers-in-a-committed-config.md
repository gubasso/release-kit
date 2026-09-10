# Carry the target answers in a committed config

## Context and Problem Statement

A machine-written landing manifest records resolved values, while setup facts were repeated as flags. A project needed one editable, shared answer without making later edits invalidate the bytes an earlier landing recorded.

[Keep two records with different failure modes](./ADR-keep-two-records-with-different-failure-modes.md) established that a file's reader earns its failure mode from its job. A third target file needs the same deliberate distinction.

## Considered Options

- `Committed config as input, manifest as record` — chosen.
- `Hand-edit the manifest` — rejected: an input edit would invalidate the record's reproduction of landed bytes.
- `Clone-local configuration` — rejected: clones could disagree about shared workflow behavior.
- `Serialize the whole config on flag write-back` — rejected: it drops the operator's comments.

## Decision Outcome

Chosen option: `committed config as input, manifest as record` — landing resolves the answers and records them; comparisons keep reading the record alone.

The manifest is machine-written, so an unknown field is version skew and dropping it is correct; the config is hand-written, so an unknown field is a typo, and dropping it leaves the operator believing a setting is on. That asymmetry is the decision, not an inconsistency. Absence remains compatible; malformed content, unknown schemas, unknown keys and weakened policy refuse.

This supersedes the rejected configuration option in [Make the release style a landing parameter](./ADR-make-the-release-style-a-landing-parameter.md) only for committed configuration: a shared file has none of the clone-local toggle's divergent truths. Style remains a recorded landing parameter.

The authored template supplies comments for a new file. `toml_edit` changes a landing key while retaining existing comments and table ordering, held by `rewrite_key_preserves_comments`.

Enforced by `target-config:the-config-is-input-and-the-record-is-the-record`, `target-config:an-unknown-key-refuses`, and `target-config:a-flag-overrides-and-a-landing-writes-back`.

## Consequences

- Good: one reviewable answer shared by clones; reproducible comparisons survive input edits.
- Bad: two target files express values at different moments, so status must explain an untaken edit.

## Status

Accepted.
