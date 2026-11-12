# 05 — Diff surface

Five axes vary between projects. Four are technology answers, and a binding is those four plus the runbook lines and files that wire them. The fifth is the forge, which varies independently of all four. Everything else is the spine, unchanged.

A binding is the release automation capability at two dimensions: the release driver, which is the one technology among the project's technologies that states the version and takes the bot, and the forge. `release.automation` at `(rust, github)` is one binding, and a project whose profile names several technologies still selects exactly one driver. A profile with no automatic release selects no binding at all, and the local Git workflow guards and the forge's title gate land without one.

## The four technology axes

| Axis                | The question it answers                                     |
| ------------------- | ----------------------------------------------------------- |
| Version file        | Which committed artifact states the version the tag mirrors |
| Release-request bot | Which tool maintains the bump-and-changelog pull request    |
| Registry and auth   | Where a publish goes and how the workflow authenticates     |
| Artifact builder    | Which tool builds and attaches installers and binaries      |

## The answers, per technology

| Axis                | Rust            | Python                               | JS and Node                          | Go                     | Bash                        |
| ------------------- | --------------- | ------------------------------------ | ------------------------------------ | ---------------------- | --------------------------- |
| Version file        | `Cargo.toml`    | `pyproject.toml` `[project] version` | `package.json` `version`             | the tag itself         | `VERSION`                   |
| Release-request bot | release-plz     | rk with git-cliff                    | Changesets                           | none                   | rk with git-cliff           |
| Registry and auth   | crates.io, OIDC | PyPI, OIDC                           | npm, OIDC                            | none; the module proxy | none                        |
| Artifact builder    | cargo-dist      | wheels and sdist are the artifacts   | the registry tarball is the artifact | GoReleaser             | the build command's tarball |

Rust, Python, and Bash have full bindings under [bindings](../bindings/README.md). The JS and Go columns state the answers for when their bindings land; Go is the degenerate case, where the tag is the version file and pushing it is the publish.

## The fifth axis: the forge

The forge hosts the repository and enforces its protections, and it is orthogonal to the other four axes. A Rust project on GitLab has the same version file, the same release-request bot, and the same registry as a Rust project on GitHub, and a different everything else: a different CLI, a different protection model, a different bot identity, a different CI file format.

Because the two vary independently, the release automation a project selects is a pair, not a single choice:

```text
(driver, forge) -> the concrete answers
(rust,   github) -> Cargo.toml, release-plz, crates.io+OIDC, cargo-dist, gh
(rust,   gitlab) -> Cargo.toml, release-plz, crates.io+OIDC, none, glab
(python, github) -> pyproject.toml, rk with git-cliff, PyPI+OIDC, wheels, gh
(python, gitlab) -> pyproject.toml, rk with git-cliff, PyPI+OIDC, wheels, glab
(bash,   github) -> VERSION, rk with git-cliff, none, the build command's tarball, gh
(bash,   gitlab) -> VERSION, rk with git-cliff, none, the build command's tarball, glab
```

The bot is a choice per pair. A pair takes an upstream tool where one keeps the release request and writes the version file on both forges, and the release engine in `rk` where none does. Rust keeps release-plz. The python and bash pairs run the engine: `rk release request` keeps one request per base on the branch `rk-release/<base>`, and the release half runs `rk release build`, then the provenance and the registry upload CI owns, then `rk release publish`, which creates the release page and the tag last. [The engine decision](../_docs/decisions/ADR-run-the-release-through-the-rk-engine.md) records why.

An axis answer can be nothing, and the pair table already holds one: `(rust, gitlab)` has no artifact builder, the same shape as the Go column's bot and registry rows. [The Rust binding](../bindings/rust.md) carries that fact, because it is a property of the pair rather than of either axis alone. A pair the distribution ships no files for is a capability that is unavailable at those dimensions: the profile stays valid, and an apply that requires the automation refuses naming the available tuples.

What the forge axis owns: how the release request is named and refreshed, which merge verbs keep the trunk linear, how branches and tags are protected, what the bot identity is, and which CI file format the workflows use. What it never changes: the one-pull-request gate, the committed version leading the tag, the one-directional release lines, and the recovery paths.

The merge verbs differ by name and by shape: one forge restricts a pull request to squash through a ruleset and offers no fast-forward, another sets a fast-forward merge method project-wide and squashes per merge request. Linear trunk history is reachable on both, and each forge document owns its own mapping.

## Writing a new binding

A binding document answers the four technology axes for its technology, then carries only what the spine cannot say:

- The concrete commands for each stage of [operate](./03-operate.md).
- The registry's specific rejects, limits, and token scopes, so [setup](./02-setup.md) step 0 and step 5 are executable.
- The facts that disqualify or configure tools: what the bot can and cannot bump, what the artifact builder generates and owns.
- The provenance the channel offers, how it is switched on, and how a consumer verifies it; where the channel offers none, the binding says so, per [the invariants](./01-invariants.md).
- The deterministic files, added under `snippets/<driver>/<forge>/` with sentinel placeholders, and their tools pinned in the versions registry under the capability that lands them.
- Where a technology axis has no answer on a forge, that fact, stated as a smaller product rather than smoothed over.

What a binding never carries: a restatement of the spine, the invariants, another binding's facts, or a forge's own mechanics. If a sentence holds for every technology, it belongs in a method chapter; if it holds for every technology on one forge, it belongs to the forge axis.
