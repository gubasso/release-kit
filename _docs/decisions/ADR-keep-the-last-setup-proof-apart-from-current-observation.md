# Keep the last setup proof apart from current observation

## Context and Problem Statement

`rk setup check` reads the forge now. The `install-bot` step reads the GitHub App's own installation, which only a runtime holding the App's private key can do. An isolated agent runtime holds no such key by design, so its check reported the step unknown and prescribed an apply, and agents read that as a broken setup and asked the operator to expose the key or redo a setup a credential-capable run had already proven. Nothing portable recorded that proof.

## Considered Options

- `a committed proof of the last complete observation, judged offline, beside a check that reads an unreachable key as unknown` — chosen.
- `a proof that expires after a fixed time` — rejected: time alone invalidates no forge fact, and a recent proof can already be wrong after a remote edit.
- `a signed proof` — rejected: git already attributes the bytes, and no command trusts the proof to authorize a mutation.
- `a proof imported from a green release run` — rejected: a workflow exercises some permissions and observes no setup step, so it would prove less than it claims.
- `a check that passes on a compatible proof` — rejected: the check promises current observation, and a proof would let a remote edit read clean.

## Decision Outcome

Chosen option: `a committed proof of the last complete observation, judged offline, beside a check that reads an unreachable key as unknown` — the proof answers what was last proven and the check answers what holds now, so an unknown step no longer reads as a defect and a step found wrong still fails.

Enforced by `setup-proof:a-checkpoint-records-only-a-complete-observation`, `setup-proof:the-status-reads-the-proof-offline`, `setup-proof:an-unavailable-credential-is-an-observation-boundary`, and `setup-proof:current-evidence-overrides-the-proof`.

## Consequences

- Good: an agent without the key reports an unverified step as unverified, and asks for no secret.
- Good: a stale proof names the setup answer that moved.
- Bad: every consumer commits one more file, and refreshes it after a setup change.

## Status

Implemented; `src/setup/report.rs`, `src/setup/proof.rs`, `src/setup/secrets.rs`, `src/commands/setup.rs`, `blocks/agents-block.md.in`, and `guidance/0.8.7.md` enact it.
