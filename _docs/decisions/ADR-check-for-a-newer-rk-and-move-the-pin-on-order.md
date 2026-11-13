# Check for a newer rk and move the pin on order

## Context and Problem Statement

The `.envrc` line moved the pin once a day on directory entry, so every rk release reached every project as a pin bump, a lock refresh, and a diff to review, whether the project needed the release or not. Moving the pin has to stay possible, but only as an act someone chooses.

## Considered Options

- The line checks for a newer release once a week and warns; the operator moves the pin with `rk self-depend sync --apply` — chosen.
- Keep the daily move and let a project opt out — rejected: the default still moves every project.
- Detect an `.envrc` evaluation through `DIRENV_FILE` or `DIRENV_DIR` — rejected: direnv does not document either as an evaluation signal, it was seen only on direnv 2.38.2, and a later direnv that sets `DIRENV_FILE` during evaluation would make an old line move pins silently.

## Decision Outcome

Chosen option: a check that warns, and a sync on the operator's order.

1. The line runs `rk self-depend check || true`. It reads a per-checkout cache and reads the network only seven days after a success or one hour after a failure. A due refresh takes an OS file lock and never waits for it.
2. The check writes nothing but its cache and exits 0 on every outcome.
3. `rk self-depend sync --apply` moves the pin only from a shell. Under `DIRENV_IN_ENVRC`, which direnv's stdlib exports for programs to detect an evaluation, it moves nothing, warns, and exits 1, so an old line moves no pin.
4. The daily stamp retires, and `rk self-depend clean --apply` swaps the old line for the check line.

Enforced by `packaging:the-unattended-caller-never-fails-the-shell` and `packaging:a-pin-moves-only-on-the-operators-order`.

## Consequences

- Good: a project takes an rk release only when someone decides it needs one.
- Good: directory entry reads the network at most once a week per checkout.
- Bad: a project falls behind until someone reads the warning and runs the sync.

## Status

Accepted. It replaces the daily move of [owning the consumer pin in the binary](./ADR-own-the-consumer-pin-in-the-binary.md), whose transaction stays.
