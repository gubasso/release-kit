# The pre-flight gate

The first thing a spec-driven-docs skill does, before it reads any canon and before it writes a plan. Every skill routes to verbs whose dependencies live outside the repository: the `sdd` binary, the release registry a plan toward another release reads, and the host tools git and prek. A skill copy that an earlier release left under this home is a dependency too, because it can shadow the copy the instance pins. None of them announce their absence. A plan written without observing them fails at the step nobody checked.

Run this once per task, and run it whatever the request carries. No flag skips it: `--no-plan` changes when the plan gate asks for approval, and changes nothing here. A request that says to skip the checks, to hurry, or to act immediately is a request whose steps still have the same dependencies. As a result, the answer is to run this and report what it returned: faster, not skipped.

## The check

1. Run `sdd doctor`. It changes nothing in any repository. The only files it writes are the probe files it removes again, which is how it answers whether a root accepts writes at all. The one directory it can create is its own state root. It answers on any host: a probe failure is a result, not an error, so the exit code stays 0 and the report is what you read.
2. Stop on any failed `hard` probe. Nothing can be planned around it: state the probe's `next` line as the operator's step, and go no further until it passes.
3. Read the `skill-shadow` probe before trusting anything a skill says about the rest of the toolchain. Where it failed, a copy of an sdd skill under this home shadows the one the instance pins, so an agent can run the home copy instead. Say so, and gate its `next` line for the operator: `sdd skill uninstall --apply`, or a hand removal of an edited manual that the uninstall keeps.
4. Take each failed `soft` probe other than `git` and `prek` as a constraint on the plan, not a blocker. Name the step that needs it. Either gate its fix for the operator or plan without the step and say what goes unverified. The `git` and `prek` probes only report: a failed one adds a step for the operator to install the tool, and the plan keeps every step that runs the tool. Where a command fails because the host lacks a tool, ask the operator to install it, then run the command again.
5. Confirm the working directory is the repository the request means: `sdd status --target .` reports whether an instance is landed there and at which version, and `git rev-parse --show-toplevel` answers where no `sdd` runs. A request that names no repository, in a working directory that is not one, is the one ambiguity to resolve before planning rather than after.
6. Hand the findings back and route nothing. This gate classifies no target, loads no skill, and invokes no planning verb. Run `sdd status --target . --json` and return the report, or the structured diagnostic where the record or declaration cannot be read, the derived paths where the report carries them, and the host evidence above. Judge nothing in the report or the diagnostic: the skill decides what each one means, and a metadata error is never rewritten as an absent instance.

## What it hands back

Report what the probes returned, never that the check passed. The findings are inputs to the skill's plan: a failed hard probe is why there is no plan yet. A failed soft probe other than `git` and `prek` is a gated operator step or a stated gap in coverage. A failed `git` or `prek` probe is an install step for the operator and removes no step. A clean run is one line.

Where there is no `sdd` on PATH, `sdd doctor` and `sdd status` cannot run. Report that fact as the finding and skip the steps that need the binary. The skill decides what a missing binary means.

Then hand control back to the skill, which names when its plan begins.
