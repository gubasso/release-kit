---
upstream: https://github.com/axodotdev/cargo-dist/issues
affects: .github/workflows/release.yml
state: masked
filing: deferred
checked: 2026-11-11
workaround: zizmor.yml ignores the two audits for release.yml, and .github/actionlint.yaml ignores the five shellcheck relays of the file
retire_when: each mask retires with its own upstream fix, once the cargo-dist pin here reaches the release carrying it. The template-injection ignore retires with cargo-dist#2320. The excessive-permissions ignore retires when the workflow-level contents write drops to the jobs that need it. The actionlint ignore retires with cargo-dist#62, the shell-quoting fix, which is separate from #2320.
---

# cargo-dist generates a workflow this project cannot harden

## Symptom

`zizmor` reports one high `excessive-permissions` finding and four `template-injection` findings against `.github/workflows/release.yml`, and `actionlint` relays five shellcheck notices from the same file. Neither the findings nor the file are this project's to fix.

## How it works

1. `dist generate` writes the whole of `.github/workflows/release.yml` from cargo-dist's own template. `bindings/rust.md` states that the file is never hand-edited, because the next generation reverts any edit.
2. The `dist-plan` job in `.github/workflows/ci.yml` proves the committed file is what the pinned generator produces, with `git diff --exit-code` after a `dist generate`.
3. An inline `# zizmor: ignore[...]` comment is therefore unavailable. The generator does not write one, so adding one fails the proof at the next run.
4. The generated file sets `permissions: contents: write` at workflow level rather than on the jobs that write. That is the `excessive-permissions` finding.
5. The generated file interpolates `${{ }}` expressions directly into five `run:` bodies, and leaves `$PRERELEASE_FLAG` unquoted. Those are the `template-injection` findings and the shellcheck relays.
6. `allow-dirty = ["ci"]` is cargo-dist's documented escape, and taking it means dropping the proof in step 2 and porting every generator change by hand. This project keeps the proof instead.

## Signal

```bash
prek run zizmor-own --all-files
prek run actionlint --files .github/workflows/release.yml
```

Removing the `release.yml` entries from `zizmor.yml` returns five findings. Removing the `release.yml` entry from `.github/actionlint.yaml` returns five shellcheck relays.

## Workaround

`zizmor.yml` carries `release.yml` under the `excessive-permissions` and `template-injection` ignore lists, and `.github/actionlint.yaml` ignores the two shellcheck codes, SC2086 and SC2129, for that path alone. Both linters still read the file, and both ignores carry a comment pointing here.

The two ignores are keyed by base filename, which is the only key zizmor accepts, so they would also reach `snippets/bash/github/.github/workflows/release.yml`. The `zizmor-snippets` hook runs `--no-config` for that reason, and the snippets are audited with no ignore list at all.

## Upstream

The filing is held on purpose. Every part of this case already sits on cargo-dist's tracker under the three issues below, filed by other people, so a fourth report adds noise and no evidence. That is why `upstream:` names the tracker rather than one issue.

Every retirement condition was checked on 2026-11-11 against cargo-dist 0.34.0, and none is met. Both issues are open. #2320 gained a comment on 2026-11-02 and no fix. The workflow that 0.34.0 generates still sets `contents: write` at workflow level, and zizmor and actionlint report the same five findings and five relays without the masks. 0.34.0 is the latest release, published 2026-11-02, and the pin here, so no pin bump reaches a fix either. Re-check these three before trusting the masks below to be necessary.

- <https://github.com/axodotdev/cargo-dist/issues/2320> — open, filed 2026-03-04. Reports that workflow scanning tools force `allow-dirty = ["ci"]`, and that the workflow then drifts. Links a candidate fix that routes the interpolations through intermediate environment variables.
- <https://github.com/axodotdev/cargo-dist/issues/62> — open, filed 2023-01-31. The shell-quoting half.
- <https://github.com/axodotdev/cargo-dist/issues/2133> — closed completed 2025-10-22. Moved `attestations` and `id-token` from the workflow level to the jobs that mint them, and left `contents: write` where it is.
