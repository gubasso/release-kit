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
6. For a setup or migration task at a target with no landing record, classify it: `rk assess --target . --json` reads the evidence — the record, the technology and forge, other tools' release markers, the payload destinations already present, the tags, the long-lived branches — and answers `greenfield`, `brownfield`, or `needs-decision`. The verdict routes the plan: greenfield lands the workflow, brownfield loads the migration skill, and needs-decision is the operator's call, asked with the evidence in front of them. A target already carrying a record routes by its status report — upgrade, or the drift it names — not by classification, whose verdict describes every healthy landing as brownfield.

## What it hands to the plan

Report what the probes returned, never that the check passed. The findings are inputs to the plan the next gate binds: a failed hard probe is why there is no plan yet, a failed soft probe is a gated operator step or a stated gap in coverage, and a clean run is one line.

Where `rk doctor` itself does not run — no `rk` on PATH — that is the whole finding: state it, name `cargo install release-kit` or `cargo binstall release-kit`, and stop. Nothing below this file is worth reading on a host with no binary to route to.

Then read `~/.local/state/release-kit/skills/shared/plan-gate.md` and hold it for the rest of the task.
