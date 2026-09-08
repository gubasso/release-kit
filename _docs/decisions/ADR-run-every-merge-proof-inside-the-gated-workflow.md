# Run every merge proof inside the gated workflow

## Context and Problem Statement

The forge resolves a job's `needs` inside one workflow file, and a required status check names a job rather than a workflow. The trunk protection requires one project-owned context beside the landed title check, and the trunk style arms the merge when the request opens, so nobody reads the check list. Two landed artifacts reported a status no gate could hold: the generated artifact workflow under its default run mode, and the workflow the Nix opt-in landed. Each went red and shipped anyway.

## Considered Options

- Land no request-reporting workflow but the title check, and serve every other proof as a job of the project's own gated workflow — chosen.
- Require each second workflow's job as a further named context — rejected: a second name still holds no other job in that file.
- Call the generated workflow from the project's own as a reusable workflow — rejected: the generator reverts the call at the next generate.
- Teach a caller workflow over local reusable workflows — deferred: revisit if a gated file outgrows what one file reads well.
- Land a CI workflow carrying the jobs — rejected: the project's own CI is the target's to write.

## Decision Outcome

Chosen option: the seed sets `pr-run-mode = "skip"`, so the generated workflow is tag-only; the Nix capability lands its expression and its flake pair and no workflow; and the Rust binding serves both proof jobs for the gate. astral-sh/uv sets the same run mode and carries its own plan job the same way. `rk status --check` judges both ends of the generator, and `rk setup check` still names any job a gate leaves out.

Enforced by `landing:a-seeded-file-still-carries-the-invariants`, `landing:the-flake-pair-lands-all-or-nothing`, and `forge-setup:the-required-check-is-shaped-to-report`.

## Consequences

- Good: a broken release configuration holds the merge, because the required check needs the job that proves it.
- Bad: an operator owes the gate two jobs the distribution once wrote.

## Status

Implemented: `snippets/rust/github/dist-workspace.toml`, the `pr-run-mode-not-skip` and `workflow-runs-on-a-request` codes in `src/landing/invariants.rs`, and the served jobs in `bindings/rust.md`.
