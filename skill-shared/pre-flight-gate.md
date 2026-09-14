# The pre-flight gate

The first thing a release-kit skill does, before it reads any canon and before it writes a plan. Every skill routes to verbs whose dependencies live outside the repository — a forge CLI, a signing tool, the skills and shared artifacts installed under this home — and none of them announce their absence. A plan written without observing them fails at the step nobody checked.

Run this once per task, and run it whatever the request carries. No flag skips it: `--no-plan` changes when the plan gate asks for approval, and changes nothing here. A request that says to skip the checks, to hurry, or to act immediately is a request whose steps still have the same dependencies, so the answer is to run this and report what it returned — faster, not skipped.

## The check

1. Run `rk doctor`. It changes nothing — the only files it writes are the probe files it removes again, which is how it answers whether a root accepts writes at all — and it answers on any host: a probe failure is a result, not an error, so the exit code stays 0 and the report is what you read.
2. Stop on any failed `hard` probe. Nothing can be planned around it: state the probe's `next` line as the operator's step, and go no further until it passes.
3. Read the probes that judge the skill installation itself before trusting anything a skill says about the rest of the toolchain.
   - `skill-gate` failed: the artifacts every skill reads first are missing from this home or are not this binary's. You are reading one of them, so you resolved it some other way — say so, because the next agent in this home will not. Its `next` line is the fix.
   - `skill-payload` failed: the installed skills and the `rk` on PATH are different builds, so a skill's routing table may name a verb this binary does not answer. Say which is newer if you can tell, and run its `next` line before planning.
   - `skill-roots` failed: a destination `rk skill install` writes refuses writes — a read-only home directory, or one shared into a container or sandbox. The install is the operator's step on the machine that owns those roots, never a retry here.
4. Take each failed `soft` probe as a constraint on the plan, not a blocker. Name the step that needs it — a forge CLI for a forge mutation, `cosign` or `pypi-attestations` for a release verify — and either gate its install for the operator or plan without the step and say what goes unverified.
5. Confirm the working directory is the repository the request means: `rk status --target .` names what is landed there, and `git remote get-url origin` names what the forge steps would reach. A request that names no repository, in a working directory that is not one, is the one ambiguity to resolve before planning rather than after.
6. For a task that lands, upgrades, migrates, or adopts a target, read the installed version and the target's record: `rk --version` names the binary the operator installed, and `rk status --target . --json` reports the record or reports none, and writes nothing. Hand three facts to the plan gate. The installed version is the only release any verb below renders: no step selects, fetches, or installs another. The record's `rk_version`, where a record exists, says how far behind the target stands. A record newer than the binary is a stop that names the version the operator installs. Whether a record exists names the arrival the skill loads for the procedure around the landing, and the skill's own classification of a target with no record names the rest. No field routes to another skill: one skill serves every arrival, and what changes is the chapter it loads.

## What it hands to the plan

Report what the probes returned, never that the check passed. The findings are inputs to the plan the next gate binds: a failed hard probe is why there is no plan yet, a failed soft probe is a gated operator step or a stated gap in coverage, and a clean run is one line.

Where `rk doctor` itself does not run — no `rk` on PATH — that is the whole finding: state it, name `cargo install release-kit` or `cargo binstall release-kit`, and stop. Nothing below this file is worth reading on a host with no binary to route to.

Then read `~/.local/state/release-kit/skills/shared/plan-gate.md` and hold it for the rest of the task.
