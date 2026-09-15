# Bindings

One document per release driver: the four [diff-surface](../method/05-diff-surface.md) answers, the concrete commands, and the registry facts the spine cannot state. A binding is the `release.automation` capability at one driver and one forge, selected where the project profile's release is automatic and that driver is among its technologies. Each binding assumes the whole [method](../method/README.md); nothing here repeats it.

| Binding               | Version file     | Bot            | Registry            | Artifacts           |
| --------------------- | ---------------- | -------------- | ------------------- | ------------------- |
| [Rust](./rust.md)     | `Cargo.toml`     | release-plz    | crates.io over OIDC | cargo-dist          |
| [Python](./python.md) | `pyproject.toml` | release-please | PyPI over OIDC      | wheels and sdist    |
| [Bash](./bash.md)     | `VERSION`        | git-cliff      | none                | `make dist` tarball |

The deterministic files each binding lands live under `snippets/<driver>/<forge>/` in this repository and reach a project through `rk init` once the profile's release driver selects them; `rk profile` reports the selection before anything lands. The tool versions they assume are pinned once, in the versions registry at the repository root, readable with `rk versions`.
