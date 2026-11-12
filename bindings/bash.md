# Bash binding

| Axis                | Answer                                              |
| ------------------- | --------------------------------------------------- |
| Version file        | `VERSION`, one line                                 |
| Release-request bot | git-cliff                                           |
| Registry and auth   | none                                                |
| Artifact builder    | the build command's tarball, `make dist` by default |

`rk init --technology bash --release-driver bash` lands `VERSION`, `cliff.toml`, and the release workflow `.github/workflows/release.yml`.

## The shape

There is no registry, so stages 4 and 5 of [the spine](../method/00-model.md) collapse into the tag and the release page: publishing is tagging, and the tarball attached to the release is the distribution. The one-pull-request shape is unchanged — the release request bumps `VERSION` and the changelog, and merging it is what makes the workflow tag and attach.

A one-line `VERSION` file is the committed source of truth, and `git-cliff --bump` computes the next version from Conventional Commits and writes the changelog deterministically, which is what qualifies it under [the invariants](../method/01-invariants.md). git-cliff maintains no pull request on its own; the landed workflow drives it to open and refresh the release request.

## The changelog

git-cliff writes `CHANGELOG.md` from the template in the seeded `cliff.toml`. git-cliff expects a template that the project writes, so the project owns that template and keeps it. The changelog that it renders is generated all the same: exclude it from every formatter and prose gate, as [generated files](./README.md#generated-files) says, together with the other files that the landing writes.

## Setup specifics

- No registry means no step 0 metadata gate, no bootstrap token, no trusted publisher, and no enforcement switch; setup is the branch shape, the protections, and the landed files.
- On GitLab, a project declares its own jobs in `.gitlab/ci/project.yml`, which the rendered parent triggers as a child pipeline; [the forge document](../forges/gitlab.md) carries the rules that file follows.
- The landed `VERSION` is `0.0.0`, the unreleased baseline; with no tag in the repository, the first release request bumps straight to the `initial_tag` that `cliff.toml` configures, so the baseline and the computed first version can never collide.
- The build command is `profile.release.build.command`, `make dist` where nothing answers it, and the packages the release image installs before it runs are `profile.release.build.packages`, `make` by default. The GitLab pipeline runs on `alpine:3`, so its packages are `apk` names, and the GitHub workflow runs on `ubuntu-latest`, so its packages are `apt` names.
- The command's output contract is `dist/<name>.tar.gz` and `dist/<name>.tar.gz.sha256`, the tarball a pure function of the tree. The provenance and attach jobs glob those paths, so the contract does not move with the command.
- A project whose command is make keeps the Makefile contract: it honours `PREFIX`, `DESTDIR`, `bindir`, `libdir`, and `datadir`. Every downstream packaging tool assumes that contract, so it is the installability gate this binding runs where others dry-run against a registry.
- `rk setup step package-check` judges the command before the first release: a make command needs a makefile, and `make -n` must resolve its target where make is on the host; another command's first word must resolve where the host can tell. It never inspects the tarball and names that gap: `git archive` honours the `export-ignore` attribute, so a tracked file can be absent. Run the command and `tar -tzf` the result before the first release, and check that `SECURITY.md` is in it.
- On both forges the tag is created only by the final release-create request, in the job that attaches the assets, after the build and the provenance exist, so a failed build, provenance, or upload leaves no tag. On GitLab a failed release save can still leave the tag at the pipeline's own commit, and a rerun of `attach` creates the release on it. A tag at another commit stops the job and routes to the recovery section `The tag exists and the release does not`.
- An `install.sh` one-liner must verify the checksum before extracting; a curl-pipe installer that skips it is the one fair complaint against the pattern. The checksum sits on the same release page as the tarball, so it proves the download arrived intact and nothing about where it came from; the attestation is what answers the second question.

## Provenance

- There is no registry, so the release page is the entire distribution surface, and its assets are mutable: whatever can replace the tarball can replace the `.sha256` beside it. This binding needs the attestation more than the ones with a registry behind them, not less.
- Every action this binding's workflows execute is pinned by full commit SHA, per [the invariants](../method/01-invariants.md); the readable tag beside each pin is the discovery ref `versions.toml` classifies and `rk versions --check` resolves.
- On GitHub, the landed `release.yml` attests the tarball with `actions/attest` in the same job that builds it, which is why `tag-and-attach` carries `id-token: write`, `attestations: write`, and `artifact-metadata: write` alongside `contents: read`. The attestation is minted before `gh release create` runs, and that command creates the tag when it publishes the release: the order is the rule, not an implementation detail, because the release page is public the moment it exists, and a page created first would point at an unattested tarball until the attest step ran — permanently, if the run failed between the two. A consumer's minimum identity check is `gh attestation verify <tarball> --repo <owner>/<repo>`, which proves the repository alone; the release gate in `rk guide release` step 6 is the stronger form, binding the evidence to the release commit and the signing workflow. An `install.sh` served from the release page is itself an artifact and can be attested and verified the same way before it is run.
- On GitLab, the runner itself writes a SLSA v1 provenance statement for the build when `RUNNER_GENERATE_ARTIFACTS_METADATA` is set, and the landed pipeline signs that statement keylessly with `cosign attest-blob --type slsaprovenance1` in a job isolated from the build, producing a bundle published beside the tarball. Signing the runner's statement rather than the bare tarball is what makes it a build-provenance attestation — the statement carries the source commit, the builder, and the build parameters, where a plain signature proves only that some identity signed a digest. No standing private key is managed: keyless signing mints an ephemeral key pair against the job's OIDC identity and discards it, so this stays inside the one signing scheme the channel offers.
- The GitLab half works on GitLab.com only. Sigstore's public instance trusts `gitlab.com` as an OIDC issuer and no self-managed `CI_SERVER_URL`, and the `id_tokens` configuration must live in the project being built and signed. On a self-managed instance the landed pipeline states that limitation and releases without provenance rather than failing; the release page then honestly carries the tarball and checksum alone.
- A consumer on GitLab verifies the bundle with `cosign verify-blob-attestation --type slsaprovenance1 --bundle <tarball>.sigstore.json --certificate-oidc-issuer https://gitlab.com --certificate-identity "https://gitlab.com/<project-path>//.gitlab-ci.yml@refs/heads/<ref>" <tarball>`. The certificate identity embeds the CI configuration path and the ref the release was built from, so a release cut from a `release/*` line verifies against that ref, not against `master`. Obtain cosign the way the pipeline does: download the release binary at the `versions.toml` pin and verify it against its published digest before running it.

## Downstream packaging

- AUR takes two packages: the tag-pinned one and the `-git` variant tracking the default branch.
- OBS covers rpm and deb across openSUSE, Fedora, Debian, and Ubuntu from the same tag, one service file per package.

## Recovery specifics

- There is no registry to withdraw from. A defective release is fixed forward; the defective release page keeps its tag and gains a warning line pointing at the successor, and its tarball stays for inspection.
- A failed attach re-runs the release workflow on the same commit, before any tag exists; the build command reproduces the identical tarball. A draft that a failed upload left is finished, not duplicated.
- A failed build, provenance, or upload leaves no tag, because only the final release-create request in `attach` creates it. A rerun after a fix lands is therefore a fresh release request and not a retry: un-bump `VERSION` as [recovery](../method/04-recovery.md#the-release-request-merged-and-nothing-published) states.
