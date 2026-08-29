# Execute the setup rather than landing it

## Context and Problem Statement

The setup steps are executable, they run once per repository, and they are the same for every project on a forge. `method/02-setup.md` told an operator what must be true with no way to make it true, and no existing artifact class was the right home for a per-forge script projects must not own and nobody invokes by name.

## Considered Options

- `Embed the scripts and execute them through rk setup` — chosen.
- `Land them into each target through rk init` — rejected: every project would own a copy of logic that changes only when `rk` changes, and the installed base would drift release by release.
- `Install them to ~/.local/bin` — rejected: eighteen commands in a shared namespace, each needing a prefix, a version, and an install lifecycle, for files that are useless without the ordering `rk` imposes.
- `Publish them as forge-CLI extensions` — rejected: an extension is authored and released independently of the CLI it extends, and these are not.

## Decision Outcome

Chosen option: `embed and execute`. Each run materializes the scripts it needs into that run's private journal directory, verifies the written bytes against the embedded bytes by digest, records the digest, and spawns each step as `sh <path>`, so a hardened `noexec` state root still executes and the journal proves which bytes ran. The directory is removed on clean completion and kept on failure. `rk setup script <name>` is the audit escape hatch. Every step follows one observe-compare-apply-verify lifecycle, and `rk setup check` calls the same observe functions with the mutating half unreachable. Enforced by `forge-setup:a-script-is-executed-never-installed` and `forge-setup:a-step-is-idempotent`.

## Consequences

- Good: one version for the CLI and its scripts, no install lifecycle, and a per-run copy no concurrent run can corrupt.
- Good: recovery is the idempotent rerun plus `rk setup step`, so no checkpoint format exists to version.
- Bad: the execution path requires a POSIX `sh`, so this is not a native Windows feature.

## Status

Accepted.
