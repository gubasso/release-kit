# The pre-flight gate

The first thing a release-kit skill does, before it reads any canon and before it writes a plan. Every skill routes to verbs whose dependencies live outside the repository: a forge CLI, a signing tool, the skill packages installed under this home. None of them announce their absence, and a plan written without observing them fails at the step nobody checked.

Run this once per run, and run it whatever the request carries. No flag skips it. A request that says to skip the checks, to hurry, or to act immediately still has the same dependencies, so run this and report what it returned: faster, not skipped.

## The check

1. Run `command -v rk`. Where it finds nothing, report that there is no `rk` on PATH, skip the steps that need the binary, and go to the hand-back below.
2. Run `rk doctor`. It changes nothing in any repository: the only files it writes are the probe files it removes again, which is how it answers whether a root accepts writes. A probe failure is a result, not an error, so the exit code stays 0 and the report is what you read.
3. Stop on any failed `hard` probe. Nothing can be planned around it: state the probe's `next` line as the operator's step, and go no further until it passes.
4. Read the probes that judge the skill installation before trusting anything a skill says about the rest of the toolchain.
   - `skill-gate` failed: a package in this home lacks a gate, or carries one that is not this binary's. You are reading a gate, so you resolved it some other way. Say so, because the next agent in this home will not. Its `next` line is the fix.
   - `skill-sources` failed: the installed skills and the `rk` on PATH are different builds, so a skill can name a verb this binary does not answer. Say which is newer where you can tell, and run its `next` line before planning.
   - `skill-roots` failed: a destination `rk skill install` writes refuses writes, such as a read-only home or one shared into a container. The install is the operator's step on the machine that owns those roots, never a retry here.
5. Take each failed `soft` probe as a constraint on the plan, not a stop. Name the step that needs it, such as a forge CLI for a forge mutation, or `cosign` or `pypi-attestations` for a release verify. Gate its install for the operator, or plan without the step and say what goes unverified.
6. Confirm the working directory is the repository the request means: `git remote get-url origin` names what the forge steps reach. A request that names no repository, in a working directory that is not one, is the one ambiguity to resolve before any plan.
7. Run `rk status --target . --json` and hold its report. Where the record or the configuration does not read, the command exits with a structured diagnostic: hold that instead, because an unreadable record is never an absent one. Judge nothing in either. No field routes to another skill.

## The hand-back

Report what the probes returned, never that the check passed. A failed hard probe is why there is no plan yet, a failed soft probe is a gated operator step or a stated gap in coverage, and a clean run is one line.

Hand the report, or the diagnostic, back to the skill. The skill decides what each finding means, a missing binary included, and names when its plan begins.
