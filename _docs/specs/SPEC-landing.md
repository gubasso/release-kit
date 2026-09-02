# Landing Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`landing:a-landing-leaves-a-record` — A landing leaves a record](#landinga-landing-leaves-a-record--a-landing-leaves-a-record)
  - [`landing:a-record-states-its-schema` — A record states its schema](#landinga-record-states-its-schema--a-record-states-its-schema)
  - [`landing:a-rendered-file-is-reproducible` — A rendered file is reproducible](#landinga-rendered-file-is-reproducible--a-rendered-file-is-reproducible)
  - [`landing:a-rendered-file-carries-no-judgment` — A rendered file carries no judgment](#landinga-rendered-file-carries-no-judgment--a-rendered-file-carries-no-judgment)
  - [`landing:an-upgrade-refuses-on-owned-drift` — An upgrade refuses on owned drift](#landingan-upgrade-refuses-on-owned-drift--an-upgrade-refuses-on-owned-drift)
  - [`landing:a-seeded-file-is-never-rewritten` — A seeded file is never rewritten](#landinga-seeded-file-is-never-rewritten--a-seeded-file-is-never-rewritten)
  - [`landing:a-seeded-file-still-carries-the-invariants` — A seeded file still carries the invariants](#landinga-seeded-file-still-carries-the-invariants--a-seeded-file-still-carries-the-invariants)
  - [`landing:a-dropped-file-stays` — A dropped file stays](#landinga-dropped-file-stays--a-dropped-file-stays)
  - [`landing:a-target-is-never-downgraded` — A target is never downgraded](#landinga-target-is-never-downgraded--a-target-is-never-downgraded)
  - [`landing:status-judges-only-under-check` — Status judges only under check](#landingstatus-judges-only-under-check--status-judges-only-under-check)
  - [`landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else](#landingan-adoption-writes-the-record-and-nothing-else--an-adoption-writes-the-record-and-nothing-else)
  - [`landing:a-block-destination-owns-its-marked-lines-alone` — A block destination owns its marked lines alone](#landinga-block-destination-owns-its-marked-lines-alone--a-block-destination-owns-its-marked-lines-alone)
  - [`landing:the-shared-zone-composes-into-every-pair` — The shared zone composes into every pair](#landingthe-shared-zone-composes-into-every-pair--the-shared-zone-composes-into-every-pair)
  - [`landing:a-landed-hook-serves-the-release-convention-alone` — A landed hook serves the release convention alone](#landinga-landed-hook-serves-the-release-convention-alone--a-landed-hook-serves-the-release-convention-alone)
  - [`landing:the-routing-block-bounds-the-agents-initiative` — The routing block bounds the agent's initiative](#landingthe-routing-block-bounds-the-agents-initiative--the-routing-block-bounds-the-agents-initiative)

<!--TOC-->

## Purpose

Rules governing what `rk init`, `rk status`, `rk upgrade`, and `rk adopt` owe a target repository: the landing record at `.release-kit/manifest.json`, the ownership kinds `rendered`, `seeded`, and `state`, and the comparisons each verb may make from them. Its subject is writing into a target and staying truthful about what was written, which is neither carrying a payload — `SPEC-distribution.md` — nor acting on a remote forge — `SPEC-forge-setup.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The comparable tools these rules were checked against are in `../reference/REFERENCE-landing-sources.md`.

## Requirements

### `landing:a-landing-leaves-a-record` — A landing leaves a record

A successful `rk init --apply` MUST write `.release-kit/manifest.json` last, after every file has landed through the temp-plus-rename writer, and a refused landing MUST leave the target unchanged, the record included. The record is committed with the landing: every reader it exists for — a clone, a CI job, an agent — sees only committed files, and it carries digests of committed files, nothing secret and nothing machine-specific.

#### Scenario: A rendered destination conflicts on apply

- GIVEN a target whose workflow destination already holds bytes that differ from the rendered candidate
- WHEN `rk init --apply` runs
- THEN it exits 73 naming the conflict, no file lands, and no `.release-kit/` directory appears

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-record-states-its-schema` — A record states its schema

The record MUST carry an integer `schema_version`, and a record at a version this binary does not know MUST refuse naming the record, never a best-effort read, because commands make decisions from it and must be able to say when they cannot.

#### Scenario: A record from a future release is read by an older binary

- GIVEN a `.release-kit/manifest.json` declaring `schema_version: 999`
- WHEN `rk status` or `rk upgrade` runs
- THEN each exits 73 naming the record and the version it found, and nothing is written

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-rendered-file-is-reproducible` — A rendered file is reproducible

A `rendered` file's landed bytes MUST be a deterministic function of the payload and the recorded parameters only, and every substituted value MUST be recorded in the manifest's `parameters`, so a later command can re-render the candidate and compare it against the disk.

#### Scenario: The owner substitutes from the repo parameter

- GIVEN a landing run with `--repo acme/widget`
- WHEN the workflow file lands
- THEN no `OWNER` token survives in it, the owner reads `acme`, and the record's `parameters.repo` carries `acme/widget` whole

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-rendered-file-carries-no-judgment` — A rendered file carries no judgment

A sentinel needing operator judgment MUST NOT appear in a `rendered` file: a value a `rendered` file needs becomes a landing parameter, and a judgment stays in a `seeded` file, where an edit is expected and costs nothing.

#### Scenario: A landing reports its remaining sentinels

- GIVEN a rust landing with the repository resolved
- WHEN `rk init --apply` reports the sentinels left to fill
- THEN every reported line sits in a `seeded` file, and the landed workflow carries none

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:an-upgrade-refuses-on-owned-drift` — An upgrade refuses on owned drift

Where a `rendered` file's bytes on disk differ from the digest the record says was written, `rk upgrade` MUST collect every such conflict and refuse the whole run in one pass, leaving every file and the record as found, so an operator resolves everything and re-runs once rather than discovering conflicts one at a time.

#### Scenario: Two owned files were edited

- GIVEN a landed target whose workflow and routing block were both edited
- WHEN `rk upgrade --apply` runs
- THEN it exits 73 naming both files in one refusal, and neither the files nor the record change

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-seeded-file-is-never-rewritten` — A seeded file is never rewritten

`rk upgrade` MUST NOT rewrite a `seeded` file: a difference from the recorded baseline is reported as drift, and the rewritten record follows the target's bytes while keeping the baseline the target tunes away from — the seeding payload, or the last rendered bytes where the payload reclassified the file from `rendered` — because a `seeded` file is a starting point the target is expected to tune.

#### Scenario: A tuned configuration survives an upgrade

- GIVEN a landed target whose `release-plz.toml` the operator filled
- WHEN `rk upgrade --apply` runs
- THEN the tuned bytes survive, the run reports `drift`, and the new record's digest for that file matches the disk

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-seeded-file-still-carries-the-invariants` — A seeded file still carries the invariants

`rk status` MUST judge a landed file's effective configuration against the invariants its `(technology, forge, destination)` key owns, reporting each failure with a stable code, the destination, the reason, and the exact remediation, and `--check` MUST count each one a violation. The relationship to `landing:a-seeded-file-is-never-rewritten` is deliberate: nothing is rewritten — the file stays the target's to tune — and the narrow part the invariants own is judged. The judgment MUST read the parsed configuration, never match text: a commented key, a disabled value, a defaulted phase, or an unpaired phase fails, and whitespace or key order changes nothing.

#### Scenario: A landed target turns attestations off

- GIVEN a landed rust/github target whose `dist-workspace.toml` sets `github-attestations = false`
- WHEN `rk status` and `rk status --check` run
- THEN both report the failure with its code and remediation, the plain run exits 0, the check exits 1, and the file is untouched

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-dropped-file-stays` — A dropped file stays

A file the payload stops shipping MUST be left in place and named in the upgrade's output, and the rewritten record stops carrying it, because a file release-kit stops shipping is a file the target owns from that moment.

#### Scenario: A newer payload drops a workflow

- GIVEN a record naming a destination this binary's payload no longer ships
- WHEN `rk upgrade --apply` runs
- THEN the file survives on disk, the output names it dropped, and the record no longer lists it

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-target-is-never-downgraded` — A target is never downgraded

Where the record names an `rk_version` newer than this binary's, `rk upgrade` MUST refuse and name the version to install, because rewriting a newer landing with older bytes is not an upgrade.

#### Scenario: An old binary meets a new landing

- GIVEN a record whose `rk_version` is above this binary's version
- WHEN `rk upgrade --apply` runs
- THEN it exits 73 telling the operator to install the matching release, and nothing is written

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:status-judges-only-under-check` — Status judges only under check

Plain `rk status` MUST report and exit 0 for every reportable state — drift, staleness, unresolved sentinels, invariant failures, and no landing at all — and `rk status --check` MUST compute the identical report and exit 1 exactly on a violation: drift to a `rendered` file, an invalid or missing landing, an unresolved judgment sentinel, or an invariant failure under `landing:a-seeded-file-still-carries-the-invariants`. Seeded drift and pin staleness stay informational in both modes.

#### Scenario: The same target, judged and not

- GIVEN a landed target with a tuned seeded file and an edited rendered file
- WHEN `rk status` and `rk status --check` run
- THEN both print the same report, the plain run exits 0, and the check exits 1 naming the rendered drift in its violations

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else

`rk adopt` MUST verify every `rendered` destination byte for byte against the rendered candidate, refuse listing every mismatch and every missing expected file in one run, and end a successful pass with exactly one write — the record, last, with its origin stating the adoption — leaving every target file untouched.

#### Scenario: A pre-record target is adopted

- GIVEN a repository running the convention with no record, matching what this payload renders
- WHEN `rk adopt --apply` runs
- THEN the manifest appears with `origin` set to `adopt`, no other file changes, and `rk status` then reports the landing

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-block-destination-owns-its-marked-lines-alone` — A block destination owns its marked lines alone

A block-placed artifact MUST own exactly the lines between its markers: a landing splices the block into the target's document — fresh where none exists, in place where marked, under the owning key otherwise — and MUST refuse by name a document that offers the block no place, leaving the target unchanged, because rewriting a document the target owns is not a landing.

#### Scenario: A hooks file with no repos list

- GIVEN a target whose `.pre-commit-config.yaml` exists carrying no `repos:` line
- WHEN `rk init --apply` runs
- THEN it exits 73 naming the file, no file lands, and no `.release-kit/` directory appears

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-shared-zone-composes-into-every-pair` — The shared zone composes into every pair

`snippets/_shared/<forge>` MUST land with every `(technology, forge)` pair for its forge, MUST never be selectable as a technology, and a destination the shared zone and a pair both ship MUST refuse as a payload defect, never one zone silently winning.

#### Scenario: The shared zone is offered as a technology

- GIVEN the embedded payload carrying `snippets/_shared/`
- WHEN `rk init --tech _shared` runs, and the supported pairs are listed for an unknown pair
- THEN the tech refuses as unknown, and neither listing names `_shared`

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-landed-hook-serves-the-release-convention-alone` — A landed hook serves the release convention alone

The hook block MUST carry only hooks enforcing the release convention's own rules — the commit contract and the local mirrors of the forge protections — never general hygiene, which stays the target's own, and every third-party hook it names MUST be pinned in `versions.toml`.

#### Scenario: The block's hooks are enumerated

- GIVEN a landed `.pre-commit-config.yaml` block
- WHEN its hooks are read beside `rk versions`
- THEN each hook maps to a rule the method states, and each third-party repository the block names carries a registry pin

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-routing-block-bounds-the-agents-initiative` — The routing block bounds the agent's initiative

The routing block MUST state that an agent acting in the target guides and never drives: that a request to change code authorizes the file changes alone, and that creating, switching or deleting a branch, committing, pushing, tagging, and opening, updating or merging a pull request happen only where the operator's request named that action. It is the one landed line no mechanism enforces — a hook and a forge protection bound the end state and cannot tell an agent from a person — so the target carries it as a sentence rather than leaving an agent to discover it by refusal.

#### Scenario: The block is read for what it authorizes

- GIVEN the routing block release-kit renders into a target's `AGENTS.md`
- WHEN the test suite reads it
- THEN it carries the line bounding the agent's initiative, and no line ordering an agent to branch, commit, or merge on its own

Verify: `cargo nextest run -E 'binary(cli)'`
