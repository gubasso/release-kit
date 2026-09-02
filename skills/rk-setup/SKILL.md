---
name: rk-setup
description: Lands the release-kit workflow in a project through the rk CLI. Use when asked to set up a release workflow, release automation, trusted publishing, release-plz, release-please, git-cliff, changelog automation, or trunk-based release automation, or to adapt the release-kit convention to a project's technology. Triggers include release-kit, rk init, release setup, and release workflow setup.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Landing files into a target needs write access to that repository.
---

# rk-setup

Land the release-kit convention in a project. The CLI carries the whole canon: every method chapter is readable with `rk method <chapter>`, every technology binding with `rk binding <tech>`, and the deterministic files land with `rk init`.

## Before acting

Read `~/.local/state/release-kit/skills/shared/plan-gate.md` before the first action of a task, and hold it for the whole task. It binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The gate is why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, and the gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the approval turn only. Still state the ordered plan before acting, and still validate it as phase 2 directs.

## Route to the canon

| Need                            | Command                    |
| ------------------------------- | -------------------------- |
| List method chapters            | `rk method --list`         |
| Read a chapter                  | `rk method <chapter>`      |
| List bindings                   | `rk binding --list`        |
| Read a technology binding       | `rk binding <tech>`        |
| Read a forge's specifics        | `rk forge <name>`          |
| The setup recipe, as commands   | `rk guide setup`           |
| List the landable files         | `rk snippet --list`        |
| Print one landable file         | `rk snippet <tech>/<path>` |
| Print the pinned-tool registry  | `rk versions`              |
| List the executable setup steps | `rk setup --list`          |
| A landed target's own report    | `rk status --target .`     |

## Land the workflow

1. Detect the technology: `Cargo.toml` means rust, `pyproject.toml` means python, a `VERSION` file or a plain script tree means bash. When none of the bindings fit, stop and say so; the method still applies, the files do not.
2. Read the spine and the binding before touching anything: `rk method model`, `rk method invariants`, and `rk binding <tech>`.
3. Check freshness. `rk versions` prints each pinned tool with the URL its check queries. For every tool the chosen binding uses, fetch that URL, compare the latest version against the pin, and read the release notes when they differ. Prefer the latest version when landing; where the landed file then diverges from the snippet, say what moved and why.
4. Check for an existing landing first: `rk status --target .`. A target already carrying `.release-kit/manifest.json` takes `rk upgrade`, not a second landing, and `rk init --apply` refuses over one.
5. Decide the scopes. `rk init --apply` refuses without `--scopes`, the comma-separated Conventional Commit scopes the project accepts, rendered into the title check, the commit hook, and the routing block. Derive the list from the project's own structure — its modules, packages, or ownership zones — and from `git log --format=%s`, present it, and let the operator adjust before landing.
6. Reconcile the commit hooks. The landing splices a marked block of release-convention hooks into `.pre-commit-config.yaml`, under `repos:`, leaving the rest of the file the target's own. Before applying, read the target's existing config: a hook already doing one of these jobs — `committed`, `commitlint`, `gitlint`, another conventional-commit or branch-guard hook — is a duplicate to name, and the choice between it and the landed hook is the operator's, never a silent second hook doing the same job. Check the landed hooks' pins against `rk versions` per step 3, research whether a better current tool exists and say so when one does, and merge toward one lean config with one hook per job. On an existing config, verify the top level carries `default_install_hook_types: [pre-commit, commit-msg, pre-push]` — the splice cannot add a top-level key — and end the step with `pre-commit install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push` run or stated. Where the target's CI runs a `pre-commit run` sweep, set `SKIP=no-commit-to-branch` in that job's environment: CI commits nothing, so the commit-time branch guard would refuse every trunk checkout it sweeps.
7. Preview, then land: `rk init --tech <tech> --target .` lists every destination without writing; `--apply` writes the files, splices the routing block into `AGENTS.md` and the hook block into `.pre-commit-config.yaml`, and writes the landing record last. The forge and repository come from the git remote; pass `--forge` and `--repo` where no remote decides them. The lander refuses when a file release-kit owns holds different content; a differing seeded file is the target's own and is kept.
8. Fill the sentinels. The repository owner and the scopes are substituted at landing, so apply reports only the judgment markers — each a `TODO(release-kit)` line in a seeded file. Resolve each one from the project; `rk status --check` exits nonzero while one remains.
9. Walk the repository-side setup with `rk guide setup` for the commands and `rk method setup` for the reasoning. `rk setup --target .` previews the executable steps and `--apply` runs them in order — on GitHub with `--required-check <name>` — then `rk setup check --target .` proves what was applied. The bot-identity walkthrough is `rk forge <name>`; the remaining manual actions are all registry-side, and the runbook names each with its reason.

## Verify

The landed files hold the invariants of `rk method invariants`: exactly one workflow carries the OIDC permission and its filename is the one registered; the version file leads and no tag is hand-authored; the trunk is written through squash-merged pull requests only; every artifact a consumer downloads is attested by the run that built it. The proof of the whole setup is one release cut end to end with `rk method operate`.

## Defaults

- Never run the setup steps out of order; each one names what the next depends on.
- Never edit a generated artifact workflow by hand; change its configuration and regenerate, as the binding directs.
- Never answer provenance with a signing scheme of your own; take what the channel offers by default, and where it offers nothing, say so rather than implying otherwise.
- A project that already carries a partial setup gets the same sequence, skipping only what is verifiably done.
