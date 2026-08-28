---
name: rk-setup
description: Lands the release-kit workflow in a project through the rk CLI. Use when asked to set up a release workflow, release automation, trusted publishing, release-plz, release-please, git-cliff, changelog automation, or a release gate, or to adapt the release-kit convention to a project's technology. Triggers include release-kit, rk init, release setup, and release workflow setup.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Landing files into a target needs write access to that repository.
---

# rk-setup

Land the release-kit convention in a project. The CLI carries the whole canon: every method chapter is readable with `rk method <chapter>`, every technology binding with `rk binding <tech>`, and the deterministic files land with `rk init`.

## Route to the canon

| Need                           | Command                    |
| ------------------------------ | -------------------------- |
| List method chapters           | `rk method --list`         |
| Read a chapter                 | `rk method <chapter>`      |
| List bindings                  | `rk binding --list`        |
| Read a technology binding      | `rk binding <tech>`        |
| List the landable files        | `rk snippet --list`        |
| Print one landable file        | `rk snippet <tech>/<path>` |
| Print the pinned-tool registry | `rk versions`              |

## Land the workflow

1. Detect the technology: `Cargo.toml` means rust, `pyproject.toml` means python, a `VERSION` file or a plain script tree means bash. When none of the bindings fit, stop and say so; the method still applies, the files do not.
2. Read the spine and the binding before touching anything: `rk method model`, `rk method invariants`, and `rk binding <tech>`.
3. Check freshness. `rk versions` prints each pinned tool with the URL its check queries. For every tool the chosen binding uses, fetch that URL, compare the latest version against the pin, and read the release notes when they differ. Prefer the latest version when landing; where the landed file then diverges from the snippet, say what moved and why.
4. Preview, then land: `rk init --tech <tech> --target .` lists every destination without writing; `rk init --tech <tech> --target . --apply` writes. The lander refuses to overwrite a file whose content differs, so a re-run is safe.
5. Fill the sentinels. Apply reports every `TODO(release-kit)` marker left in the landed files — the repository owner, secret names, environment names. Resolve each one from the project.
6. Walk the repository-side setup in order with `rk method setup`: the metadata gate, the branch shape, the bot identity, the protections, the first manual publish, the trusted publisher, the proven release, then enforcement. The binding's setup section carries the technology's concrete commands and registry pages.

## Verify

The landed files hold the invariants of `rk method invariants`: exactly one workflow carries the OIDC permission and its filename is the one registered; the version file leads and no tag is hand-authored; the release branch is written by automation only; every artifact a consumer downloads is attested by the run that built it. The proof of the whole setup is one release cut end to end with `rk method operate`.

## Defaults

- Never run the setup steps out of order; each one names what the next depends on.
- Never edit a generated artifact workflow by hand; change its configuration and regenerate, as the binding directs.
- Never answer provenance with a signing scheme of your own; take what the channel offers by default, and where it offers nothing, say so rather than implying otherwise.
- A project that already carries a partial setup gets the same sequence, skipping only what is verifiably done.
