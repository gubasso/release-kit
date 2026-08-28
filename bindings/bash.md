# Bash binding

| Axis                | Answer                                 |
| ------------------- | -------------------------------------- |
| Version file        | `VERSION`, one line                    |
| Release-request bot | git-cliff                              |
| Registry and auth   | none                                   |
| Artifact builder    | `make dist` tarball from `git archive` |

`rk init --tech bash` lands `VERSION`, `cliff.toml`, and the release workflow `.github/workflows/release.yml`.

## The shape

There is no registry, so stages 4 and 5 of [the spine](../method/00-model.md) collapse into the tag and the release page: publishing is tagging, and the tarball attached to the release is the distribution. The two-pull-request shape is unchanged — the release request bumps `VERSION` and the changelog, the gate pins and approves, and the workflow on `master` tags and attaches.

A one-line `VERSION` file is the committed source of truth, and `git-cliff --bump` computes the next version from Conventional Commits and writes the changelog deterministically, which is what qualifies it under [the invariants](../method/01-invariants.md). git-cliff maintains no pull request on its own; the landed workflow drives it to open and refresh the release request.

## Setup specifics

- No registry means no step 0 metadata gate, no bootstrap token, no trusted publisher, and no enforcement switch; setup is the branch shape, the protections, and the landed files.
- The landed `VERSION` is `0.0.0`, the unreleased baseline; with no tag in the repository, the first release request bumps straight to the `initial_tag` that `cliff.toml` configures, so the baseline and the computed first version can never collide.
- The Makefile honours `PREFIX`, `DESTDIR`, `bindir`, `libdir`, and `datadir`. Every downstream packaging tool assumes that contract, so it is the installability gate this binding runs where others dry-run against a registry.
- `make dist` produces the tarball and its `.sha256` from `git archive`, so the artifact is a pure function of the tag.
- An `install.sh` one-liner must verify the checksum before extracting; a curl-pipe installer that skips it is the one fair complaint against the pattern.

## Downstream packaging

- AUR takes two packages: the tag-pinned one and the `-git` variant tracking the default branch.
- OBS covers rpm and deb across openSUSE, Fedora, Debian, and Ubuntu from the same tag, one service file per package.

## Recovery specifics

- There is no registry to withdraw from. A defective release is fixed forward; the defective release page keeps its tag and gains a warning line pointing at the successor, and its tarball stays for inspection.
- A failed attach re-runs the release workflow on the same tag; `git archive` reproduces the identical tarball.
