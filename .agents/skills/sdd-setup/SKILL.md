---
name: sdd-setup
description: Lands and operates a spec-driven-docs instance in a project through the sdd CLI. Use when asked to install or set up spec-driven docs, detect whether a repository has an instance, verify or upgrade an installed instance, splice the documentation section into AGENTS.md, or diagnose sdd verify failures. Triggers include spec-driven-docs, sdd init, sdd status, sdd verify, sdd upgrade, and docs governance instance.
license: CC-BY-4.0
compatibility: Requires the sdd binary on PATH; install with cargo install spec-driven-docs or cargo binstall spec-driven-docs. Requires pre-commit, which runs the delivered gates the install wires into .pre-commit-config.yaml.
---

# sdd-setup

Operate a project's spec-driven-docs instance through the `sdd` CLI. The CLI is the whole interface: every binding rule is readable with `sdd spec <name>` and every method chapter with `sdd method <chapter>`.

## Detect

```bash
sdd status --target . --json
```

`"instance": false` means the repository has no instance; any other result reports the installed profile, version alignment, and drift counts. `sdd status` exits 0 whether or not an instance exists.

## Land an instance

1. Choose the profile: `codebase` keeps records under `docs/`; `knowledge-base` keeps them under `_docs/`.
2. Preview: `sdd init --target "$PWD" --profile codebase`. A non-empty target defaults to a dry run and lists every destination.
3. Review the listed paths, then apply: `sdd init --target "$PWD" --profile codebase --apply`.
4. Confirm: `sdd verify --target "$PWD"` prints `OK spec-driven-docs <version>`.
5. Declare the plan zone: in the landed `SPEC-spec-to-code.md`, retarget the verification command of `spec-to-code:a-spec-change-is-typed` at the directory the project's planning tool writes its work records to. Remove the requirement instead when the project keeps no plan zone. This is the only seeded value that is not portable, because the planning tool owns the record and this framework names none.

The install seeds specs and templates the project owns from then on (adopted), lands byte-exact configurations and agent skills the canon owns (managed), and splices one marked block into `.pre-commit-config.yaml`. It touches nothing outside its destinations and the markers.

## Wire agent context

Add the documentation section to the project's `AGENTS.md`, creating the file when absent:

```markdown
## Documentation

- Load the affected specs before editing governed content.
- Treat decision records as immutable rationale and load them only when asked why.
- Run `sdd verify` before handoff.
- Keep adopted specs and local integration instance-owned.
```

## Verify

```bash
sdd verify --target .
```

`FAIL` lines mean managed drift or a broken record and exit 1; `DRIFT` lines mean adopted edits awaiting reconciliation and exit 0. A failure message cites a `domain:rule` ID; read the binding sentence with `sdd spec <domain>`.

## Upgrade

```bash
sdd upgrade --target . --dry-run
sdd upgrade --target .
```

Install the newer `sdd` first. A locally edited managed file aborts the whole upgrade with every conflict listed in one run; revert or reconcile, then re-run. Adopted files and content outside the markers survive upgrades.

An adopted seed the canon stops shipping is left in place, because the project owns it from the moment it lands. An upgrade that stops seeding one names it in the release notes; delete the file once nothing local cites its rules. `SPEC-distribution.md` is the first: it states the installer's obligations, which no project can meet or check, and `SPEC-instance.md` now carries what a project owes its own installation.

## Defaults

- One instance per repository, at the repository root.
- Prefer `sdd status --json` for machine decisions; stdout is one JSON object.
- Leave managed files alone: `.spec-driven-docs/` and the installed skill files under `.claude/skills/` and `.agents/skills/` belong to the canon, and `sdd verify` fails on any edit.
