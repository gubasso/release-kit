# Task: configure

Change what a record selects, on a record at this binary's version: the Git workflow answers, the integration mode, the release style, and each opt-in capability. A changed answer is a landing at the same version with that answer as a flag. `rk upgrade` writes the answer back into `.release-kit/config.toml` with its comments kept, so never edit the configuration by hand.

## Pre-flight

1. Run `rk status --target . --json` for the current answers: the profile, the Git workflow, and the capabilities.
2. Run `rk profile --target . --json` for what the catalog can select here. Never offer a change the catalog reports unavailable.
3. State the consequence of each requested change in one line:
   - the checkout mode: branches open across the change follow the transition in `rk method worktrees`;
   - the integration mode: moving between `local` and `forge` rewrites the hook block and the routing block, and changes who integrates;
   - the release style: `trunk` arms the release request, and `lines` waits for a person, so an open request changes behavior;
   - a capability turned on lands its files, and turned off leaves them as released files whose removal is asked. The Scorecard workflow posts to a public API, and `codeql` reads the declared licence;
   - the workflow guards: on lands the agent and commit policy lines, the glossary, the workflow hooks, and two setup steps, and off retires them the same way;
   - the required check, the required workflow, and the trunk name.

## Execution

Where the gate composes this task after a landing task, add its answers to that task's Stage flags and land once. Otherwise fill the pipeline:

- Stage: `rk stage --target . --json`, with only the changed answers as flags.
- Land: `rk upgrade --stage <root> --target .`, then the same with `--apply` as a gated step.

## Post-flight

1. The Prove step passed, and `rk status --target . --json` reports `current` with the new answers.
2. Regenerate what a changed configuration generates, as a gated step.
3. Name the follow-ups the change causes under "Still the operator's": an armed or disarmed release request goes to the `rk-release` skill, and branches open across a checkout-mode change follow `rk guide worktree`.
4. Remove each released file the operator chose to remove, as a gated step after a commit.
