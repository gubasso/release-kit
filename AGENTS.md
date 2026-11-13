# AGENTS

This repository is the canonical knowledge product for the release-kit workflow. `README.md` routes readers. This file routes agents to the rules that bind their work.

## Before acting

- Apply stated rules. Cite their `<domain>:<rule>` IDs in reports and failures. The Documentation block below carries which documents to load.
- Load `_docs/reference/REFERENCE-<domain>-sources.md` only to check a rule against the upstream documentation that it rests on. Also load it to re-verify a fact that a forge or a registry can move.
- Update the owning method chapter, binding, or skill in the same change as the behavior it describes.

## Ownership boundaries

- `method/` and `bindings/` are the canon prose. `snippets/` and `versions.toml` are the embedded sources that a landing renders from.
- `runbooks/`, `forges/`, and `setup/<forge>/` are host-side embedded sources. `rk guide` and `rk forge` serve them, `rk setup` executes them, and no target receives them. `SPEC-forge-setup.md` binds how the setup acts on a forge.
- The capability that lands each file scopes `snippets/`. The release automation sits at one `(driver, forge)` pair under `snippets/<driver>/<forge>/`, and the forge-scoped capabilities sit under `snippets/_shared/<forge>/`. `src/profile/catalog.rs` is the one owner of which file belongs to which capability. A pair can honestly land fewer files than another.
- `snippets/_shared/<forge>` holds the technology-independent files of a forge, and one capability that the catalog selects owns each file. It is not a technology. A destination that it shares with a pair is a projection defect.
- `blocks/` holds the whole texts that the binary writes outside `snippets/`: the spliced blocks and the host-side hook body. They are authored as files, so no human-faced artifact lives as a source literal.
- `src/issue.rs` owns the start of work from an issue. The pure half renders and judges, and the spawning half calls the forge CLI. The seating comes from `src/commands/worktree.rs`, so one derivation serves both verbs. `SPEC-issue-branch.md` binds it.
- Every landable file has a declared kind in `src/landing.rs`: `rendered`, `seeded`, or `state`. A landing writes `.release-kit/manifest.json` into the target, last. `SPEC-landing.md` binds the record and every verb that reads it. The target configuration is typed by domain. `src/profile.rs` resolves the project profile, the Git workflow, and the capability requests by one precedence. `SPEC-project-profile.md` and `SPEC-git.md` bind them, and `rk profile` reports what a target resolves to.
- `src/` is the distribution. The `rk` binary embeds every root in `src/distribution_roots.rs` through `src/embedded.rs`, with the licenses, at compile time. So canon and binary cannot drift. `src/projection.rs` is the pure projection that every landing verb and `rk stage` render from. This repository runs its previous release: the devshell carries the rk that `flake.nix` pins, which runs the hooks, the worktree verbs, the integration, and the own landing. The rk built from the tree runs only under test, by `target/debug/rk` or `cargo run`, under `distribution:this-repository-runs-its-previous-release`.
- `skills/` lands into each project with every landing, under `.claude/skills/` and `.agents/skills/`, at the rk that the project pins. No rk verb writes a skill under the home, because an agent resolves a skill by name across scopes, and a home copy shadows the project's. `rk skill uninstall` removes the copies that earlier releases installed there. `SPEC-distribution.md` binds both.
- Each project decides, case by case, where anything lands for a third-party application. It decides against that application's own documentation, with a dated citation in `_docs/reference/`. It never infers the place from a convention that this repository follows, and never generalizes from one application to another.
- `skill-shared/` is what every skill shares. It is authored once, and every package carries it as `references/<file>`. So a skill names a gate by a path relative to its own root, and a fix still lands in one file. The plan gate that every skill routes to lives there.
- `versions.toml` declares every pinned tool once. A snippet pin changes together with its registry entry. `guidance/` is the versioned reference material that the binary ships and `rk stage` copies into a stage's `reference/guidance`, and no target receives it. A release that changes a landed destination ships a guidance file or records its silence in `guidance/index.toml`. `guidance/README.md` carries the authoring rules.
- `_docs/` is this repository's own spec-driven-docs instance plus its decisions. It is instance-owned and never ships in the crate. `.spec-driven-docs/` belongs to the sdd canon.
- Keep each durable fact in one owner. Link to it elsewhere.
- `LICENSE` splits terms on the product boundary: CC BY 4.0 for the method, MIT for the distribution.

## Authoring

- The markdown shape, the size budgets, and the guide rules are adopted specs: `_docs/specs/SPEC-docs-format.md`, `_docs/specs/SPEC-docs-foundations.md`, and `_docs/specs/SPEC-guides.md`. Read them, not a restatement here.
- Runbooks and forge documents follow the guide rules instead of the chapter cap, and stay as lean as the procedure allows. Decision records are the only history-bearing document class.
- Keep exploratory material in `.draft/`. Promotion is a rewrite into the owning zone.

## Text that leaves this machine

- Text posted to a forge or a registry carries no operator particulars. Such text includes an issue, a pull request, a review comment, a commit message, and a release note. The particulars are a local filesystem path, a home directory, and a scratch, run, session, cache, or state directory. They also include a hostname and an account name that identifies the machine. The public guides already follow this rule, and it binds every forge-facing text the same way.
- A local research record is not a citation. Cite the public source that a reader can open, or restate the finding in the text itself. Keep the path to the local record out of the text entirely.
- Before posting or editing forge-facing text, scan the whole body for `/home/`, `/tmp/`, `/var/`, `.local/`, `.cache/`, and `~`. Stop on any match. The `no-personal-path` gate holds the files in this repository, and it never sees a body that goes straight to a forge. So this scan is the only check that such text gets.
- An edit alone does not repair a leak that already posted, because the forge keeps the edit history. Report it to the operator, who decides on deleting the revision.

## Executable artifacts

- Rust follows the exobrain CLI conventions: clap derive in `src/cli/`, one handler per subcommand in `src/commands/`, typed errors with a tested exit-code matrix in `src/error.rs`.
- Every handler emits through the output boundary in `src/output.rs`. Nothing in `commands/` prints directly, and every machine output carries a versioned, snapshot-tested schema per `distribution:machine-output-declares-its-schema`.
- Every subcommand lands with its integration tests in `tests/cli.rs`.
- Run `just check` before handoff. It runs the `manual` stage across all files. Every check on that stage also carries `pre-merge-commit`, the stage of the hook that `rk integrate` asks git to run on the staged squash. So the handoff gate holds every check of the integration gate. The hook runner is prek, which the host supplies and the devshell does not carry. `.pre-commit-config.yaml` declares the oldest prek it accepts, and the CI check job installs that version. Every check classified `both` is a hook at its declared stage: a costly one at pre-push and the rest at commit. So the push stage repeats no commit check. The manual stage holds every one of them too, and CI reaches each one through that stage. The `dist-plan` proofs are `both`, through the `dist-plan` hook. Two checks have another class, each with its reason where it is expressed. `rk-worktree-location` is `local-only` and skipped in the CI sweep, because CI checks out detached on the main worktree. The `dist-build` job is `ci-only` at the pull-request boundary. A full `--profile dist` build is too costly for every push, and a target that needs another runner can build only in CI. Outside that classification, `code-scanning-codeql.yml` and `scorecard.yml` are report-only, not checks. Each uploads its findings or its score to the forge and fails only on a tool error. `pr-title.yml` holds what the commit-msg hooks `conventional-pre-commit --strict --force-scope` and `rk-message` already hold. `release-plz.yml` and `release.yml` release and publish. So neither kind is a check.
- `Cargo.toml` is the release source of truth. Every commit message is a scoped Conventional Commit. release-plz derives the version, the changelog, and the tag from them. Never author a tag. This repository runs its own convention, `rk method operate`.
- Manage dependencies through cargo (`cargo add`, `cargo remove`, `cargo update`). Never hand-edit versions in `Cargo.toml`.

## Routing

- The method spine and recovery paths: `method/README.md`. Technology specifics: `bindings/README.md`.
- Forge specifics and the bot-identity walkthroughs: `forges/README.md`, served by `rk forge`.
- The how of each procedure, step by step: `runbooks/README.md`, served by `rk guide`. Its chapter owns the why of each step, and the pair states each procedure once.
- This repository's overlay over `rk guide setup` and `rk guide release`, with its coordinates, its deviations, and the proof transcript: `_docs/guides/release/README.md`. It names no account, repository, or crate, because the guides are public and carry no operator's particulars.
- The executable repository-side setup: `rk setup`, with `rk runs` over its journals.
- The branches that a squash merge retires in this clone: `rk branches prune`, a preview by default. The post-merge reminder lands with `rk setup step branch-reminder`, bound by `_docs/specs/SPEC-maintenance.md`.
- This repository works in linked worktrees. Every code-changing branch lives in its linked worktree, which `rk worktree add <branch>` creates or adopts beside the checkout. The main checkout commits nothing, and `rk worktree prune` retires a merged worktree. One branch, one writer. The lifecycle and the checkout mode: `rk worktree`, with `rk guide worktree` as the procedure.
- Work reaches `master` as one squash commit that `rk integrate <branch>` writes from a short-lived branch, once the clone's `pre-merge-commit` hook passes on the staged squash. Install that hook once per clone with `prek install --hook-type pre-merge-commit`. The branch name is `<type>/<slug>`, whose type matches the type of the squash title, or the forge-minted `<issue-id>-<slug>`. Nothing else commits on `master`. The push of the trunk is a separate operator action. `--forge` sends one execution through a pull request instead. The integration authority is the other Git workflow axis, `git.integration`, `local` or `forge`. `rk upgrade --integration <mode>` changes it, `rk guide integration` is the procedure, and `rk method integration` is its why. This repository records `local`.
- A request that names an issue starts from the forge's own branch. `rk issue start <issue>` mints it at the forge, seats it the way the recorded mode says, and links it to the issue. `rk guide issue` is the procedure, bound by `_docs/specs/SPEC-issue-branch.md`. Never invent a name for work an issue already names.
- A target that already releases somehow, and its verdict before anything lands: `rk assess`, with `rk guide migration` as the procedure and `rk method migration` as its why. The migration itself is `rk migrate`: an inventory, an archive into a workspace under the state root, the registry's marks, and a close, bound by `_docs/specs/SPEC-migration.md`. The stage that every landing write is studied from: `rk stage` writes this binary's candidate and its knowledge into a disposable directory. `rk stage compare` reads it against the target row by row, and `rk stage clean` removes exactly one stage that names itself, bound by `_docs/specs/SPEC-staging.md`. The write itself: `rk init`, `rk upgrade`, and `rk adopt` render afresh from the installed binary and the target. They take a stage's answers through `--stage` and print one ownership word per destination. Before the first write, they refuse every conflict that `--replace` does not name. `rk guide landing` is the procedure and `rk method landing` its why, bound by `_docs/specs/SPEC-landing.md`. Every command first checks the record against the binary, bound by `_docs/specs/SPEC-version-gate.md`.
- The release-line lifecycle and the release style: `rk lines`, with `rk guide release-lines` as the procedure, bound by `_docs/specs/SPEC-maintenance.md` and `_docs/specs/SPEC-landing.md`. Minting and promoting a release candidate: `rk lines mint`, bound by `_docs/specs/SPEC-release-candidates.md`. Where a landed target's release stands, its request, its last release, and every symptom: `rk release status`, and the operator's merge of a trunk release: `rk release merge`, both bound by `_docs/specs/SPEC-release-state.md`. The release engine a bash or python pair's CI runs, its request, its build, and its publish: `rk release request`, `rk release build`, `rk release eligible`, and `rk release publish`, bound by `_docs/specs/SPEC-release-engine.md`.
- What lands in a target: `snippets/`, served by `rk snippet --list`.
- The committed target answers: `.release-kit/config.toml`, the target configuration, which landing verbs read and which is written before the manifest. `_docs/specs/SPEC-target-config.md` binds the file, its floors, and informational pending input. `_docs/specs/SPEC-project-profile.md` binds the profile and the capability selection, and `_docs/specs/SPEC-git.md` binds the Git workflow. What a target resolves to, with the source of each value and the capabilities that the catalog selects: `rk profile`, which writes nothing. What a landed target reports about itself: `rk status`, with `--check` as the judging mode. `rk upgrade` takes it to the installed binary's projection, and `rk adopt` records a pre-record target.
- Pinned tools and freshness: `versions.toml`, served by `rk versions`. `rk versions --check`, `rk self-depend sync`, and `rk self-depend check` are the verbs that fetch, the check at most once a week per checkout.
- A consumer's `rk` from its own flake, pinned and kept fresh: `rk self-depend`, with `rk guide setup` carrying the procedure, bound by `_docs/specs/SPEC-packaging.md`.
- The host's state and the whole command surface: `rk doctor` and `rk usage`.
- What the binary carries and writes outside a target: `_docs/specs/SPEC-distribution.md`, served by `rk skill --help`.
- How a skill is packaged, loaded, and written: `_docs/specs/SPEC-skill-format.md`.

<!-- BEGIN release-kit -->

## Releases

- This repository runs the release-kit convention. `rk method invariants` states what must stay true. A release needs a commit typed `feat`, `fix`, or `perf`, or a breaking change, that changed a file the package ships. A dependency update that changes what ships is `fix(deps)`, and one for development only is `chore(deps-dev)` or `build(deps-dev)`. A security update is always `fix(deps)`, also where it changes only a shipped lock file. Its body names the advisory ID: `RUSTSEC-`, `GHSA-`, or `CVE-`.
- An agent here guides and never drives. It reads this convention and tells the operator which step comes next. It takes no git or forge action unless the operator's request named that action. The bounded actions include the following. Create, switch, or delete a branch. Mint a branch at the forge from an issue. Create or remove a worktree. Commit, push, or tag. Integrate a branch onto the trunk, locally or through a request. Open, update, or merge a pull request. A request to change code authorizes the file changes alone.
- The workflow terms live in `GLOSSARY.md` at this repository's root. The terms release-kit owns there are `implement-and-request`, `implement-and-merge`, and `full-implement`. A request that carries one of them names every action that term lists.
- The title of every commit that reaches the trunk MUST be a scoped Conventional Commit, because the version and the changelog come from it.
- The request's title becomes that commit's message, and its body carries the context and lands with it. The body names no internal planning artifact and carries no agent attribution. The landed rk-message hook, the forge's body check, and the observed body source hold that rule.
- Every commit follows the same scoped convention. The landed commit-msg hook requires a scope on every one, and the title check holds it to lowercase letters, digits, and `_ . / -`.
- The scope names the area you changed, and reads as `area/subarea` where that is clearer. Prefer a scope this repository already uses, which `git log --format=%s | sed -n 's/^[a-z]*(\([^)]*\)).*/\1/p' | sort -u` lists. Coin a new scope only where no existing one names the area.
- Never author a tag, and never hand-edit a generated artifact workflow.
- Run `rk status` before changing anything under `.github/workflows/` or `.gitlab-ci.yml`, or any file `.release-kit/manifest.json` names. Before reading a gap in `rk setup check` as a setup defect, run `rk setup status`. A credential this runtime cannot reach leaves a step unknown, which is an observation boundary and not drift. Never ask the operator to mount, expose, recreate, or rotate a secret only so an agent check can pass.
- The full method is `rk method --list`. The recovery paths are `rk method recovery`.

<!-- END release-kit -->

<!-- BEGIN spec-driven-docs docs -->
## Documentation

- Load the affected specs before editing governed content: `_docs/specs/SPEC-<domain>.md`. <!-- markdownlint-disable-line MD013 -->
- Treat decision records as immutable rationale and load them only when asked why. <!-- markdownlint-disable-line MD013 -->
- Read the writing style before you author or edit prose: `sdd method writing-style`. <!-- markdownlint-disable-line MD013 -->
- Write and edit step-by-step guides to the adopted guides spec, `_docs/specs/SPEC-guides.md`. <!-- markdownlint-disable-line MD013 -->
- Name a document by a slug drawn from its subject, never by a number. Where its directory holds documents with no kind prefix, give that directory a `README.md` saying what it holds and what each document covers. <!-- markdownlint-disable-line MD013 -->
- For a question about this method or about operating `sdd`, run `sdd docs` and read only the topic it names. <!-- markdownlint-disable-line MD013 -->
- Run `sdd verify` before handoff. <!-- markdownlint-disable-line MD013 -->
- Keep adopted specs, the tracking registry, and local integration instance-owned. <!-- markdownlint-disable-line MD013 -->
<!-- END spec-driven-docs docs -->
