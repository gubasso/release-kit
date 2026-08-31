# Forge Setup Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`forge-setup:a-script-is-executed-never-installed` — A script is executed, never installed](#forge-setupa-script-is-executed-never-installed--a-script-is-executed-never-installed)
  - [`forge-setup:a-step-is-idempotent` — A step is idempotent](#forge-setupa-step-is-idempotent--a-step-is-idempotent)
  - [`forge-setup:a-secret-never-reaches-argv` — A secret never reaches argv](#forge-setupa-secret-never-reaches-argv--a-secret-never-reaches-argv)
  - [`forge-setup:key-material-never-reaches-the-environment` — Key material never reaches the environment](#forge-setupkey-material-never-reaches-the-environment--key-material-never-reaches-the-environment)
  - [`forge-setup:every-supported-forge-runs-every-step` — Every supported forge runs every step](#forge-setupevery-supported-forge-runs-every-step--every-supported-forge-runs-every-step)
  - [`forge-setup:a-check-reports-what-the-forge-enforces` — A check reports what the forge enforces](#forge-setupa-check-reports-what-the-forge-enforces--a-check-reports-what-the-forge-enforces)

<!--TOC-->

## Purpose

Rules governing how `rk` configures a remote forge on the operator's behalf, once per repository: the setup steps `rk setup` executes, the scripts that implement them, and the credentials they handle. Its subject is calling a remote API, which is neither carrying a payload — `SPEC-distribution.md` — nor writing into a target repository. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The forge documentation these rules rest on is in `../reference/REFERENCE-forge-setup-sources.md`, which also records what each forge does not offer.

## Requirements

### `forge-setup:a-script-is-executed-never-installed` — A script is executed, never installed

The distribution MUST materialize a setup script under a private per-run path for the duration of a run, verified by digest against the embedded bytes and executed through the interpreter, and MUST NOT write one to any directory on the user's `PATH`; `rk setup script <name>` is the audit surface for reading one.

#### Scenario: Two setup runs execute at once

- GIVEN two `rk setup --apply` invocations running concurrently on one host
- WHEN each materializes its scripts
- THEN each writes into its own run directory, neither can execute the other's bytes, and each journal records the digest of exactly what it ran

Verify: `cargo nextest run -E 'binary(cli)'`

### `forge-setup:a-step-is-idempotent` — A step is idempotent

Every mutating setup step MUST be safe to rerun, and `rk setup check` MUST mutate nothing.

#### Scenario: A setup fails at the branch-protection step and is rerun from the top

- GIVEN a repository whose earlier steps already succeeded
- WHEN `rk setup --apply` runs again
- THEN the earlier steps report satisfied rather than failing or duplicating, and only the unfinished step writes

Verify: `cargo nextest run -E 'binary(cli)'`

### `forge-setup:a-secret-never-reaches-argv` — A secret never reaches argv

A credential MUST reach a setup step through the environment or standard input, and MUST NOT appear in a process argument, in any output, or in any record — nor may any fingerprint of one, since a stable fingerprint correlates a credential across runs; the journal keeps the fact of the handling only.

#### Scenario: The bot App identifier is stored as a repository secret

- GIVEN a bot credential exported in the operator's environment
- WHEN `rk setup step bot-secrets --apply` runs
- THEN the value travels to the forge CLI on standard input, no spawned argument list contains it, and no journal file or output stream carries it

Verify: `cargo nextest run -E 'binary(cli)'`

### `forge-setup:key-material-never-reaches-the-environment` — Key material never reaches the environment

A private key MUST be named to `rk` as a path, MUST NOT be carried as an environment value by `rk` or by any child it spawns, and the distribution MUST refuse a run whose environment carries one. `rk` MUST read the named file exactly once and MUST transmit those same bytes, so that no substitution between the check and the forge is possible. Before the step spawns, the file MUST be validated for kind, mode, size, and PEM encoding, and every refusal MUST name the fact that was wrong. What the encoded key decodes to is the forge's judgment, not the distribution's.

An environment block is readable from outside the process and every later child of that shell inherits it; naming a file costs neither, and the bytes still reach standard input. An identifier or a short-lived token the forge mints and a command rotates stays a value, because the exposure and the cost of rotation are not the same.

#### Scenario: The bot private key is stored as a repository secret

- GIVEN `RK_BOT_PRIVATE_KEY_FILE` naming an owner-only PEM private key outside the repository
- WHEN `rk setup step bot-secrets --apply` runs
- THEN the bytes `rk` validated reach the forge CLI on the step's standard input, no environment `rk` constructs holds them or the path, and a group-readable file, one that is not a PEM private key by kind and encoding, or a stale `RK_BOT_PRIVATE_KEY` export refuses before any forge call

Verify: `cargo nextest run -E 'binary(cli)'`

### `forge-setup:every-supported-forge-runs-every-step` — Every supported forge runs every step

Where the distribution carries a setup step for one forge, it MUST carry a step of the same name for every forge it supports, so an operator's setup does not silently depend on which forge they chose.

#### Scenario: A step is added to one forge tree and forgotten in the other

- GIVEN a new script under `setup/github/` with no counterpart under `setup/gitlab/`
- WHEN the test suite runs
- THEN the parity test fails naming both the step and the tree missing it

Verify: `cargo nextest run -E 'binary(cli)'`

### `forge-setup:a-check-reports-what-the-forge-enforces` — A check reports what the forge enforces

Where a forge cannot enforce what a step's proof claims, `rk setup check` MUST report the weaker guarantee by name rather than reporting success.

#### Scenario: Tag protection is verified on a forge that stops accident but not authority

- GIVEN a protected `v*` pattern on a forge whose Owners can still delete a protected tag
- WHEN `rk setup check` runs
- THEN the step reports satisfied with the limitation named, so nobody believes an immutability the forge does not provide

Verify: `cargo nextest run -E 'binary(cli)'`
