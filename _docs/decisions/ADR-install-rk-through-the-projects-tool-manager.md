# Install rk through the project's tool manager

## Context and Problem Statement

The consumer-pin verb made `rk` a dependency of a consumer project and kept its pin fresh. It wired four managers across three venues, moved a flake pin through a fenced transaction, and kept a weekly check cache: about 6,600 lines of Rust, twelve block files, and twelve packaging rules. Each manager already installs and moves one exact release by its own commands. On 2026-11-21 the operator ordered the verb removed whole, with no leftover.

## Considered Options

- Remove the verb, and install `rk` through the project's tool manager by that manager's own commands — chosen.
- Keep the verb for the flake pair alone — rejected: the flake already moves a tag with `nix flake update`, so the verb repeats it.
- Deprecate the verb across one transition release — rejected: rk is pre-1.0, and the operator's projects drop the verb before the release that removes it.

## Decision Outcome

Chosen option: remove the verb, and route installation to each manager.

1. No `rk` verb installs, pins, checks, or moves `rk`.
2. The `provision` task of the setup skill asks which manager the project runs, and points to the install section of the README for that manager.
3. A record newer than the binary refuses with the version to install and the `provision` task. It reads no pin.
4. The `[acquisition]` table of the target configuration goes. An old table fails as any unknown key fails.
5. A warning that a newer release is out comes from a tool outside release-kit.

Enforced by `packaging:the-operator-selects-the-installed-version` and `version-gate:a-gated-command-names-its-fix`.

## Consequences

- Good: release-kit drops a second installer, its cache, and its rules, and each manager keeps its own workflow.
- Bad: a project with a pin learns of a newer release only from a tool outside release-kit.

## Status

Accepted. Supersedes [owning the consumer pin in the binary](./ADR-own-the-consumer-pin-in-the-binary.md), [reading the pin through a manager axis](./ADR-read-the-rk-pin-through-a-manager-axis.md), [crossing the venue and the manager in one matrix](./ADR-cross-the-venue-and-the-manager-in-one-matrix.md), and [checking for a newer rk and moving the pin on order](./ADR-check-for-a-newer-rk-and-move-the-pin-on-order.md).
