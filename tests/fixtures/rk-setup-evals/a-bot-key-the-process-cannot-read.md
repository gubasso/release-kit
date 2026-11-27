# A bot key the process cannot read: every other step runs, the key steps wait

- case: a-bot-key-the-process-cannot-read
- classification: current
- intent: forge setup
- provision: no
- tasks: the setup tasks
- landing: no
- receipt: present
- history: useful
- legacy: none
- stage: none

## Situation

A GitHub target landed at the binary's version, with `setup.bot.app_id` committed. The operator asked to finish the forge setup. `rk setup check --target . --json` reports `install-bot` and `bot-secrets` as `unreadable`. Each detail names the key path, the reason, and the remedy: import the key with `rk setup import-key <file>`, or point `RK_BOT_PRIVATE_KEY_FILE` at it. `protect-release-branches` reads unsatisfied, and the check exits 0 with a summary line that names the unmet requirement.

## Gate

Present "State: `current`. Request: forge setup. Tasks: the setup tasks. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json` and `rk setup status --target . --json`.
2. `forge-settings` pre-flight: run `rk setup check --target . --json`. Read `install-bot` and `bot-secrets` as `unreadable`: an unmet requirement, not a step to run.
3. Plan every step that is not `satisfied` except the two key steps, in the order of `rk guide setup`.
4. Preview `rk setup step protect-release-branches --target .`. On the operator's approval, run it with `--apply`.
5. `forge-settings` post-flight: run `rk setup check --target . --json` again. Every step but the two key steps reads `satisfied`.
6. Skip `rk setup checkpoint`, because a checkpoint records only a complete observation.
7. Report: fill the template from observation. Under "Still the operator's", state that `install-bot` and `bot-secrets` read `unreadable`, and quote the requirement and the remedy as the JSON detail states them.

## Authority

- The request authorizes the forge steps it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Never ask for the key, its path, or its contents. Never run a key step to make the check pass.

## Documentation

- project documentation: nothing lands, so no candidate is staged.
- reference corpus: `rk forge github` and `rk guide setup` step 2b are read for the default path and the two overrides. They are not copied into the project.
