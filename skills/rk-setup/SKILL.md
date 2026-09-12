---
name: rk-setup
description: Lands the release-kit workflow in a project through the rk CLI. Use when asked to set up a release workflow, release automation, trusted publishing, release-plz, release-please, git-cliff, changelog automation, or trunk-based release automation, or to adapt the release-kit convention to a project's technology. Triggers include vulnerability reporting, SECURITY.md, release-kit, rk init, release setup, and release workflow setup.
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
| The verdict on a target with no record     | `rk assess --target .`     |
| Judge a message against the content guards | `rk message --check`       |
| The merged branches this clone still holds | `rk branches prune`        |
| The worktree lifecycle, as commands        | `rk guide worktree`        |
| Start work an issue names                  | `rk issue start <issue>`   |
| The issue-to-branch procedure              | `rk guide issue`           |
| The release line's whole life, as commands | `rk guide release-lines`   |
| The line lifecycle verbs                   | `rk lines --help`          |
| The working-copy forms and the mode        | `rk method worktrees`      |
| What a project's devshell wiring carries   | `rk self-depend status`    |
| The flake fragments and the seed pair      | `rk self-depend add`       |
| The predecessor bump mechanism's removal   | `rk self-depend clean`     |
| The pin moved to the latest release        | `rk self-depend sync`      |
| Another project as a dependency            | `rk depend assess`         |

## Installation scope

Skills and the agent setup install at user scope only — `rk skill install --apply`, once per user; no system mode exists, by decision. The setup runbook's prerequisites own the step and the two roots. Where a file lands for a third-party application is the target project's own decision, made against that application's documentation with a dated citation — never generalized from another application.

## Start work an issue names

A request naming an issue — an issue URL, or "fix", "address", "implement", or "work on" plus an issue — starts from `rk issue start <issue>`, and the plan's first step is that command. The forge names the branch and rk seats it the way the project's recorded workflow mode says. Three rules bind this.

- Never write a predicted branch name into the plan. The name is whatever the forge mints, and stating a guess is the mistake this verb exists to prevent. Write "the branch the forge mints for issue <n>".
- The preview is what the plan presents for approval, and the apply is what execution runs. That is the plan gate's own two-phase shape, so no new gate appears here.
- Minting a branch at the forge is a forge action, so it happens only where the operator's request named starting work on that issue.

`rk guide issue` renders the whole procedure, and `rk method model` owns why the name comes from the forge.

## Land the workflow

1. Detect the technology: `Cargo.toml` means rust, `pyproject.toml` means python, a `VERSION` file or a plain script tree means bash. When none of the bindings fit, stop and say so; the method still applies, the files do not. In the same pass run `rk self-depend status --target .` and read whether the project carries a flake, an `.envrc`, the release-kit input, and any leftover of a predecessor bump mechanism — the `leftovers` list, reported whatever the state is.
2. Read the spine and the binding before touching anything: `rk method model`, `rk method invariants`, and `rk binding <tech>`.
3. Check freshness with `rk versions --check`, a verb that fetches: it compares each version pin against its source and resolves each action pin's discovery ref against the pinned execution commit. `update-available` and `ref-moved` are updates to review — read the release notes for what moved — never incidents; `no-version-source` marks a pin whose freshness signal is its ref alone. Prefer the latest version when landing; where the landed file then diverges from the snippet, say what moved and why.
4. Check for an existing landing first: `rk status --target .`. A target already carrying `.release-kit/manifest.json` takes `rk upgrade`, not a second landing, and `rk init --apply` refuses over one. A target with no record routes by the pre-flight's verdict: `greenfield` lands here, `brownfield` belongs to the rk-migrate skill because a payload landed beside another release mechanism is a second release path, and `needs-decision` is the operator's answer first.
5. Decide the workflow mode, the release style, the Nix opt-in, and the development environment in the same planning turn, with `AskUserQuestion` beside the binding and forge choices; the answers flow into the planned `rk init` flags.
   - The workflow mode: `--workflow worktree` (the default, recommended — every code-changing branch in a linked worktree, the main checkout commits nothing) or `--workflow branches` (branches worked in the main checkout, worktrees optional beside them). `rk method worktrees` owns the trade.
   - The release style, asked as a real question with the trade stated, because it changes what a green trunk does. `--style trunk` (the default) arms the bot's release request from the moment it opens: every release ships itself the instant every required check passes — continuous release, the checks inherent to the arm and the release decision made once here rather than per release. A release is held by disarming the request before its last check goes green, one held too late is withdrawn rather than abandoned, and the changelog's quality lives in the squash titles and bodies the landed gates judge. `--style lines` leaves every request for a human's merge, which is what a project needs when users cannot be rolled forward — pinned self-hosted versions, a support contract on an old line, a sign-off gate before a ship — at the cost of a duplicated pipeline per live line. `rk method model` carries the five-question table, and `rk method release-lines` owns the second style's whole life; present both options with those consequences and take the operator's answer before planning the landing.
   - The Nix opt-in: `--nix` lands a seeded package expression and a seed flake pair where the target has none — off by default, and the binding owns the matrix and its degradation. It lands no CI file: the build proof is a job the binding serves for the project's own gated pipeline — on github for the gated workflow, so propose that job with the gate in step 10, and on gitlab for `.gitlab/ci/project.yml`. Where the binding serves no job for the pair, say that the capability ships the smaller product rather than implying a proof. A target with its own flake keeps it; `rk init` reports what it withholds and why.
   - The development environment: how the project obtains `rk`. Three answers: replace what the project carries with the devshell pin (recommended — `rk` from the project's own flake, pinned at a release tag, moved forward once a day from `.envrc`), wire the devshell pin beside what is there, or leave the host install alone. Where step 1's status reported leftovers, offer the replacement alone: two bump mechanisms over the same two files fight or silently undo each other, and `rk guide setup` owns why. Where the operator chooses the replacement, the plan names four steps in order — `rk self-depend clean --apply`, `rk self-depend add` (an owned flake takes the printed fragments by hand, in the order the `--json` report lists them, and `rk init --nix` goes first where the landed packaging capability is also wanted), the fragment application, and the `.envrc` line — plus, file by file, every `manual` entry the clean reported, because those are the edits the operator or the agent still makes by hand. The proof is `rk self-depend status --json` reporting an empty `leftovers` list and `ready` before the setup is called done.
6. Reconcile the commit hooks before applying, as `rk guide setup` step 4 directs — the duplicate-hook names, the top-level install types, the CI sweep skip (the pair `no-commit-to-branch,rk-worktree-location` on a worktree-mode target). The choice between an existing hook and the landed one is the operator's, never a silent second hook doing the same job; check the landed hooks' pins against `rk versions` per step 3, research whether a better current tool exists, and say so when one does.
7. Preview, then land: `rk init --tech <tech> --target .` lists every destination without writing; `--apply` writes the files, splices the routing block into `AGENTS.md` and the hook block — the commit-shape hooks and the `rk-message` content guard — into `.pre-commit-config.yaml`, and writes the landing record last. The forge and repository come from the git remote; pass `--forge` and `--repo` where no remote decides them. The lander refuses when a file release-kit owns holds different content; a differing seeded file is the target's own and is kept.
8. Fill the sentinels. The repository owner and the scope shape are substituted at landing, so apply reports only the judgment markers — each a `TODO(release-kit)` line in a seeded file. Resolve each one from the project; `rk status --check` exits nonzero while one remains.
9. Walk the repository-side setup by the guide's numbers: `rk guide setup` steps 1 to 3 for the forge-side commands — step 3e's auto-merge switch is what the trunk style's standing arm needs, step 3g owns vulnerability reporting, and step 4a owns policy landing and its checks — and steps 5 to 8 for the registry-side actions, with `rk method setup` for each step's reasoning. The bot-identity walkthrough is `rk forge <name>`. Before step 3c on GitHub, read the jobs the project's CI workflow runs on a pull request: the protection requires one named check beside `pr-title`, and the gate's `needs` is the voting list, so where more than one job reports, propose the gate job step 1 shows — needing every job whose failure must hold the merge, `if: always()` or `if: ${{ !cancelled() }}`, failing on any result other than success — and take the operator's answer before choosing `--required-check` and before deciding which jobs go in that list; `rk setup check --required-check <name>` judges the gate's shape alone and says nothing about the jobs the gate does not name. On rust and github the gate also needs a job carrying the binding's release proofs, and one carrying the Nix build where the target opted in, because the payload lands no workflow that reports on a request beside the title check; propose those jobs with the gate and take the operator's answer for each. On gitlab no check is named at all, and a job the project runs of its own goes in `.gitlab/ci/project.yml`, which the landed pipeline triggers as a child pipeline: `rk guide setup` step 3a carries the steps and the proof, and `rk forge gitlab` carries the rules that file follows. That pipeline needs GitLab 18.2 or newer, which step 3b reads and refuses below, before the protection is installed; report the refusal to the operator rather than working around it.

After trunk protection is applied, follow `rk guide setup` step 3f to read back the freshness requirement; `rk forge <name>` names the enforcing setting for each forge.

When `package-check` reports a limitation, read it out rather than treating the step as done: `rk method setup` owns why the gate asks whether the artifact carries the reporting policy, and `rk binding <tech>` names the inspection that answers it for this target. Do not restate either; route the operator to them and carry the limitation into the release plan as a manual check.

## Verify

The landed files hold the invariants of `rk method invariants`: exactly one workflow carries the OIDC permission and its filename is the one registered; the version file leads and no tag is hand-authored; the trunk is written through squash-merged pull requests only; every artifact a consumer downloads is attested by the run that built it. The proof of the whole setup is one release cut end to end with `rk method operate`.

## Defaults

- Never run the setup steps out of order; each one names what the next depends on.
- Never edit a generated artifact workflow by hand; change its configuration and regenerate, as the binding directs.
- Never propose a second request-reporting workflow as a gated check; `needs` resolves inside one file, so the gate reaches a job only in its own.
- Never answer provenance with a signing scheme of your own; take what the channel offers by default, and where it offers nothing, say so rather than implying otherwise.
- A greenfield project that already carries part of the setup gets the same sequence, skipping only what is verifiably done; a brownfield one is the rk-migrate skill's, through `rk guide migration`.
