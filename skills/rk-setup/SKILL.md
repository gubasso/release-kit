---
name: rk-setup
description: Lands the release-kit workflow in a project through the rk CLI. Use when asked to set up a release workflow, release automation, trusted publishing, release-plz, release-please, git-cliff, changelog automation, or trunk-based release automation, or to adapt the release-kit convention to a project's technology. Triggers include release-kit, rk init, release setup, and release workflow setup.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Landing files into a target needs write access to that repository.
---

# rk-setup

Land the release-kit convention in a project. The CLI carries the whole canon: every method chapter is readable with `rk method <chapter>`, every technology binding with `rk binding <tech>`, and the deterministic files land with `rk init`.

## Before acting

Read two shared files before the first action of a task, in this order, and hold both for the whole task.

1. `~/.local/state/release-kit/skills/shared/pre-flight-gate.md` — run it whatever the request carries. It checks this host with `rk doctor` and stops the task on what no plan can work around. No flag skips it.
2. `~/.local/state/release-kit/skills/shared/plan-gate.md` — it binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The two gates are why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, the pre-flight says whether this host can run it at all, and the plan gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the plan gate's approval turn only. Still run the pre-flight, still state the ordered plan before acting, and still validate it as phase 2 directs.

## Route to the canon

| Need                                       | Command                    |
| ------------------------------------------ | -------------------------- |
| Judge this host's readiness                | `rk doctor`                |
| List method chapters                       | `rk method --list`         |
| Read a chapter                             | `rk method <chapter>`      |
| List bindings                              | `rk binding --list`        |
| Read a technology binding                  | `rk binding <tech>`        |
| Read a forge's specifics                   | `rk forge <name>`          |
| The setup recipe, as commands              | `rk guide setup`           |
| List the landable files                    | `rk snippet --list`        |
| Print one landable file                    | `rk snippet <tech>/<path>` |
| Print the pinned-tool registry             | `rk versions`              |
| List the executable setup steps            | `rk setup --list`          |
| A landed target's own report               | `rk status --target .`     |
| Judge a message against the content guards | `rk message --check`       |
| The merged branches this clone still holds | `rk branches prune`        |
| The worktree lifecycle, as commands        | `rk guide worktree`        |
| The release line's whole life, as commands | `rk guide release-lines`   |
| The line lifecycle verbs                   | `rk lines --help`          |
| The working-copy forms and the mode        | `rk method worktrees`      |

## Land the workflow

1. Detect the technology: `Cargo.toml` means rust, `pyproject.toml` means python, a `VERSION` file or a plain script tree means bash. When none of the bindings fit, stop and say so; the method still applies, the files do not.
2. Read the spine and the binding before touching anything: `rk method model`, `rk method invariants`, and `rk binding <tech>`.
3. Check freshness with `rk versions --check`, the one verb that fetches: it compares each version pin against its source and resolves each action pin's discovery ref against the pinned execution commit. `update-available` and `ref-moved` are updates to review — read the release notes for what moved — never incidents; `no-version-source` marks a pin whose freshness signal is its ref alone. Prefer the latest version when landing; where the landed file then diverges from the snippet, say what moved and why.
4. Check for an existing landing first: `rk status --target .`. A target already carrying `.release-kit/manifest.json` takes `rk upgrade`, not a second landing, and `rk init --apply` refuses over one.
5. Decide the scopes. `rk init --apply` refuses without `--scopes`, the comma-separated Conventional Commit scopes the project accepts, rendered into the title check, the commit hook, and the routing block. Derive the list from the project's own structure — its modules, packages, or ownership zones — and from `git log --format=%s`, present it, and let the operator adjust before landing.
6. Decide the workflow mode and the release style in the same planning turn, with `AskUserQuestion` beside the binding and forge choices; both answers flow into the planned `rk init` flags.
   - The workflow mode: `--workflow worktree` (the default, recommended — every code-changing branch in a linked worktree, the main checkout commits nothing) or `--workflow branches` (branches worked in the main checkout, worktrees optional beside them). `rk method worktrees` owns the trade.
   - The release style, asked as a real question with the trade stated, because it changes what a green trunk does. `--style trunk` (the default) arms the bot's release request from the moment it opens: every release ships itself the instant every required check passes — continuous release, the checks inherent to the arm and the release decision made once here rather than per release. A release is held by disarming the request before its last check goes green, one held too late is withdrawn rather than abandoned, and the changelog's quality lives in the squash titles and bodies the landed gates judge. `--style lines` leaves every request for a human's merge, which is what a project needs when users cannot be rolled forward — pinned self-hosted versions, a support contract on an old line, a sign-off gate before a ship — at the cost of a duplicated pipeline per live line. `rk method model` carries the five-question table, and `rk method release-lines` owns the second style's whole life; present both options with those consequences and take the operator's answer before planning the landing.
7. Reconcile the commit hooks before applying, as `rk guide setup` step 4 directs — the duplicate-hook names, the top-level install types, the CI sweep skip (the pair `no-commit-to-branch,rk-worktree-location` on a worktree-mode target). The choice between an existing hook and the landed one is the operator's, never a silent second hook doing the same job; check the landed hooks' pins against `rk versions` per step 3, research whether a better current tool exists, and say so when one does.
8. Preview, then land: `rk init --tech <tech> --target .` lists every destination without writing; `--apply` writes the files, splices the routing block into `AGENTS.md` and the hook block — the commit-shape hooks and the `rk-message` content guard — into `.pre-commit-config.yaml`, and writes the landing record last. The forge and repository come from the git remote; pass `--forge` and `--repo` where no remote decides them. The lander refuses when a file release-kit owns holds different content; a differing seeded file is the target's own and is kept.
9. Fill the sentinels. The repository owner and the scopes are substituted at landing, so apply reports only the judgment markers — each a `TODO(release-kit)` line in a seeded file. Resolve each one from the project; `rk status --check` exits nonzero while one remains.
10. Walk the repository-side setup by the guide's numbers: `rk guide setup` steps 1 to 3 for the forge-side commands — step 3d's auto-merge switch is what the trunk style's standing arm needs — and steps 5 to 8 for the registry-side actions, with `rk method setup` for each step's reasoning. The bot-identity walkthrough is `rk forge <name>`.

## Verify

The landed files hold the invariants of `rk method invariants`: exactly one workflow carries the OIDC permission and its filename is the one registered; the version file leads and no tag is hand-authored; the trunk is written through squash-merged pull requests only; every artifact a consumer downloads is attested by the run that built it. The proof of the whole setup is one release cut end to end with `rk method operate`.

## Defaults

- Never run the setup steps out of order; each one names what the next depends on.
- Never edit a generated artifact workflow by hand; change its configuration and regenerate, as the binding directs.
- Never answer provenance with a signing scheme of your own; take what the channel offers by default, and where it offers nothing, say so rather than implying otherwise.
- A project that already carries a partial setup gets the same sequence, skipping only what is verifiably done.
