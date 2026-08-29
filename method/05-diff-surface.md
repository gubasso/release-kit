# 05 — Diff surface

Five axes vary between projects. Four are technology answers, and a binding is those four plus the runbook lines and files that wire them. The fifth is the forge, which varies independently of all four. Everything else is the spine, unchanged.

## The four technology axes

| Axis                | The question it answers                                     |
| ------------------- | ----------------------------------------------------------- |
| Version file        | Which committed artifact states the version the tag mirrors |
| Release-request bot | Which tool maintains the bump-and-changelog pull request    |
| Registry and auth   | Where a publish goes and how the workflow authenticates     |
| Artifact builder    | Which tool builds and attaches installers and binaries      |

## The answers, per technology

| Axis                | Rust            | Python                               | JS and Node                          | Go                     | Bash                |
| ------------------- | --------------- | ------------------------------------ | ------------------------------------ | ---------------------- | ------------------- |
| Version file        | `Cargo.toml`    | `pyproject.toml` `[project] version` | `package.json` `version`             | the tag itself         | `VERSION`           |
| Release-request bot | release-plz     | release-please                       | Changesets                           | none                   | git-cliff           |
| Registry and auth   | crates.io, OIDC | PyPI, OIDC                           | npm, OIDC                            | none; the module proxy | none                |
| Artifact builder    | cargo-dist      | wheels and sdist are the artifacts   | the registry tarball is the artifact | GoReleaser             | `make dist` tarball |

Rust, Python, and Bash have full bindings under [bindings](../bindings/README.md). The JS and Go columns state the answers for when their bindings land; Go is the degenerate case, where the tag is the version file and pushing it is the publish.

## The fifth axis: the forge

The forge hosts the repository and enforces its protections, and it is orthogonal to the other four axes. A Rust project on GitLab has the same version file, the same release-request bot, and the same registry as a Rust project on GitHub, and a different everything else: a different CLI, a different protection model, a different bot identity, a different CI file format.

Because the two vary independently, a project's configuration is a pair, not a single choice:

```text
(technology, forge) -> the concrete answers
(rust,   github)    -> Cargo.toml, release-plz, crates.io+OIDC, cargo-dist, gh
(rust,   gitlab)    -> Cargo.toml, release-plz, crates.io+OIDC, none, glab
(python, github)    -> pyproject.toml, release-please, PyPI+OIDC, wheels, gh
```

An axis answer can be nothing, and the pair table already holds one: `(rust, gitlab)` has no artifact builder, the same shape as the Go column's bot and registry rows. [The Rust binding](../bindings/rust.md) carries that fact, because it is a property of the pair rather than of either axis alone.

What the forge axis owns: how the release request is named and refreshed, what shape the gate's enforcement takes, how branches and tags are protected, what the bot identity is, and which CI file format the workflows use. What it never changes: the two-pull-request shape, the pinned gate, the committed version leading the tag, the back-merge, and the recovery paths.

## Writing a new binding

A binding document answers the four technology axes for its technology, then carries only what the spine cannot say:

- The concrete commands for each stage of [operate](./03-operate.md).
- The registry's specific rejects, limits, and token scopes, so [setup](./02-setup.md) step 0 and step 5 are executable.
- The facts that disqualify or configure tools: what the bot can and cannot bump, what the artifact builder generates and owns.
- The provenance the channel offers, how it is switched on, and how a consumer verifies it; where the channel offers none, the binding says so, per [the invariants](./01-invariants.md).
- The deterministic files, added under `snippets/<technology>/` with sentinel placeholders, and their tools pinned in the versions registry.
- Where a technology axis has no answer on a forge, that fact, stated as a smaller product rather than smoothed over.

What a binding never carries: a restatement of the spine, the invariants, another binding's facts, or a forge's own mechanics. If a sentence holds for every technology, it belongs in a method chapter; if it holds for every technology on one forge, it belongs to the forge axis.
