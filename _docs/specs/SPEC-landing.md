# Landing Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`landing:a-landing-leaves-a-record` — A landing leaves a record](#landinga-landing-leaves-a-record--a-landing-leaves-a-record)
  - [`landing:a-record-states-its-schema` — A record states its schema](#landinga-record-states-its-schema--a-record-states-its-schema)
  - [`landing:a-rendered-file-is-reproducible` — A rendered file is reproducible](#landinga-rendered-file-is-reproducible--a-rendered-file-is-reproducible)
  - [`landing:the-release-style-is-a-landing-parameter` — The release style is a landing parameter](#landingthe-release-style-is-a-landing-parameter--the-release-style-is-a-landing-parameter)
  - [`landing:a-rendered-file-carries-no-judgment` — A rendered file carries no judgment](#landinga-rendered-file-carries-no-judgment--a-rendered-file-carries-no-judgment)
  - [`landing:an-upgrade-refuses-on-owned-drift` — An upgrade refuses on owned drift](#landingan-upgrade-refuses-on-owned-drift--an-upgrade-refuses-on-owned-drift)
  - [`landing:a-seeded-file-is-never-rewritten` — A seeded file is never rewritten](#landinga-seeded-file-is-never-rewritten--a-seeded-file-is-never-rewritten)
  - [`landing:a-seeded-file-still-carries-the-invariants` — A seeded file still carries the invariants](#landinga-seeded-file-still-carries-the-invariants--a-seeded-file-still-carries-the-invariants)
  - [`landing:a-dropped-file-stays` — A dropped file stays](#landinga-dropped-file-stays--a-dropped-file-stays)
  - [`landing:a-target-is-never-downgraded` — A target is never downgraded](#landinga-target-is-never-downgraded--a-target-is-never-downgraded)
  - [`landing:status-judges-only-under-check` — Status judges only under check](#landingstatus-judges-only-under-check--status-judges-only-under-check)
  - [`landing:a-landing-classifies-its-target-first` — A landing classifies its target first](#landinga-landing-classifies-its-target-first--a-landing-classifies-its-target-first)
  - [`landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else](#landingan-adoption-writes-the-record-and-nothing-else--an-adoption-writes-the-record-and-nothing-else)
  - [`landing:a-block-destination-owns-its-marked-lines-alone` — A block destination owns its marked lines alone](#landinga-block-destination-owns-its-marked-lines-alone--a-block-destination-owns-its-marked-lines-alone)
  - [`landing:the-shared-zone-composes-into-every-pair` — The shared zone composes into every pair](#landingthe-shared-zone-composes-into-every-pair--the-shared-zone-composes-into-every-pair)
  - [`landing:a-landed-hook-serves-the-release-convention-alone` — A landed hook serves the release convention alone](#landinga-landed-hook-serves-the-release-convention-alone--a-landed-hook-serves-the-release-convention-alone)
  - [`landing:the-landed-guards-hold-the-message-content` — The landed guards hold the message content](#landingthe-landed-guards-hold-the-message-content--the-landed-guards-hold-the-message-content)
  - [`landing:the-arming-identity-is-the-bot` — The arming identity is the bot](#landingthe-arming-identity-is-the-bot--the-arming-identity-is-the-bot)
  - [`landing:the-changelog-quality-gate-is-the-squash-message` — The changelog quality gate is the squash message](#landingthe-changelog-quality-gate-is-the-squash-message--the-changelog-quality-gate-is-the-squash-message)
  - [`landing:the-routing-block-bounds-the-agents-initiative` — The routing block bounds the agent's initiative and reads as plain prose](#landingthe-routing-block-bounds-the-agents-initiative--the-routing-block-bounds-the-agents-initiative-and-reads-as-plain-prose)
  - [`landing:the-nix-capability-is-a-recorded-opt-in` — The Nix capability is a recorded opt-in](#landingthe-nix-capability-is-a-recorded-opt-in--the-nix-capability-is-a-recorded-opt-in)
  - [`landing:the-flake-pair-lands-all-or-nothing` — The flake pair lands all-or-nothing](#landingthe-flake-pair-lands-all-or-nothing--the-flake-pair-lands-all-or-nothing)

<!--TOC-->

## Purpose

Rules governing what `rk init`, `rk status`, `rk upgrade`, and `rk adopt` owe a target repository: the landing record at `.release-kit/manifest.json`, the ownership kinds `rendered`, `seeded`, and `state`, and the comparisons each verb may make from them. Its subject is writing into a target and staying truthful about what was written, which is neither carrying a payload — `SPEC-distribution.md` — nor acting on a remote forge — `SPEC-forge-setup.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The comparable tools these rules were checked against are in `../reference/REFERENCE-landing-sources.md`.

## Requirements

### `landing:a-landing-leaves-a-record` — A landing leaves a record

A successful `rk init --apply` MUST write `.release-kit/config.toml` inside `.release-kit/` before `.release-kit/manifest.json`, with the record last, after every payload file has landed through the temp-plus-rename writer, and a refused landing MUST leave the target unchanged, the record included. The record is committed with the landing: every reader it exists for — a clone, a CI job, an agent — sees only committed files, and it carries digests of committed files, nothing secret and nothing machine-specific.

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

A `rendered` file's landed bytes MUST be a deterministic function of the payload and the recorded parameters only, and every substituted value MUST be recorded in the manifest's `parameters`, so every re-render and comparison reads the manifest parameters alone, whatever a later configuration says. A value the payload substitutes from one constant — the commit scope's shape — is not a parameter, so it MUST NOT be recorded, and a parameter an earlier schema recorded and this binary substitutes nowhere MUST read without it and rewrite without it.

#### Scenario: The owner substitutes from the repo parameter

- GIVEN a landing run with `--repo acme/widget`
- WHEN a workflow and `SECURITY.md` land
- THEN owner and full-path tokens resolve from the same recorded `parameters.repo`: the owner reads `acme`, the policy path reads `acme/widget`, and neither token survives

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-release-style-is-a-landing-parameter` — The release style is a landing parameter

The release style MUST be recorded in the manifest as `trunk` or `lines`, reported by every parameter-bearing report, rendered into the landed release workflow as the one value that arms or does not arm the bot's request, resolved as the runbooks' style axis, and taken from committed configuration or an invocation flag by the landing verbs; a record predating the field carries no style, and `rk upgrade` MUST refuse until `landing.style` in the config or `--style` answers it, because neither value is a compatibility-safe reading of a target nobody asked.

#### Scenario: A pre-style record upgrades

- GIVEN a landed target whose record predates the style parameter
- WHEN `rk upgrade --apply` runs with no `--style`
- THEN it refuses naming the parameter and the two values, nothing is written, and a rerun naming `--style trunk` records the answer

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-rendered-file-carries-no-judgment` — A rendered file carries no judgment

A sentinel needing operator judgment MUST NOT appear in a `rendered` file: a value a `rendered` file needs becomes a landing parameter, and a judgment stays in a `seeded` file or the committed configuration, where an edit is expected and costs nothing.

#### Scenario: A landing reports its remaining sentinels

- GIVEN a rust landing with the repository resolved
- WHEN `rk init --apply` reports the sentinels left to fill
- THEN every reported line sits in a `seeded` file or names an unanswered setup key in the config, and the landed workflow carries none

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

`rk status` MUST judge a landed file's effective configuration against the invariants its `(technology, forge, destination)` key owns, and MUST judge, per `(technology, forge)`, target-wide relationships that read a generated file the distribution ships no copy of or a target-owned root manifest landed configuration requires — for `(rust, github)`, `.github/workflows/release.yml` and the root `Cargo.toml` against `dist-workspace.toml` — reporting each failure with a stable code, the destination to change, the reason, and the exact remediation, and `--check` MUST count each one a violation. The relationship to `landing:a-seeded-file-is-never-rewritten` is deliberate: nothing is rewritten — the files stay the target's to tune — and the narrow part the invariants own is judged. The judgment over a landed configuration MUST read the parsed configuration, never match text: a commented key, a disabled value, a defaulted phase, or an unpaired phase fails, and whitespace or key order changes nothing. For `(rust, github)`, parsed `ci = "github"` or an array containing `"github"` MUST require a root `[profile.dist]` table; an absent, unreadable, or unparsable configuration or root manifest and configuration that does not select GitHub MUST report no profile failure. The judgment over a generated file MUST read that file's own text in the grammar its generator writes, because the generator is not installed and the text is what the forge executes, and MUST report a value it cannot resolve rather than pass it, since a step nobody can read is not a step nobody runs; a presentation outside that grammar is beyond a text reader, so the generator's own check stays the whole-file proof and the rule claims no more than it holds; where the generated file or the configuration it comes from is absent, the run reports nothing, because the distribution writes neither and an absence is the generator's story rather than drift. Where the pair's generated file would report a status check on a pull request, both the configuration that asks for it and the generated file that carries the trigger MUST fail, because the forge resolves a job's `needs` inside one workflow file and the trunk protection requires one project-owned context beside the title check, so a job in the generated file is in no gate's `needs` and the forge merges over its failure.

#### Scenario: A landed target turns attestations off and reports a second check

- GIVEN a landed rust/github target whose `dist-workspace.toml` sets `github-attestations = false` and leaves `pr-run-mode` at a value other than `skip`, beside a generated `.github/workflows/release.yml` that triggers on `pull_request`
- WHEN `rk status` and `rk status --check` run
- THEN both report each failure with its code and remediation — the disabled attestation, the run mode, and the workflow's request trigger — the plain run exits 0, the check exits 1, and neither file is touched

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

Plain `rk status` MUST report and exit 0 for every reportable state — drift, staleness, unresolved sentinels, invariant failures, a pending payload, and no landing at all — and `rk status --check` MUST compute the identical report and exit 1 exactly on a violation: drift to a `rendered` file, a record whose own parameters do not reproduce its recorded bytes or its recorded destination set, an invalid or missing landing, an unresolved judgment sentinel, or an invariant failure under `landing:a-seeded-file-still-carries-the-invariants`. Seeded drift, pin staleness, a pending payload, and committed configuration the landing verbs have yet to take up stay informational in both modes. A pending payload is what the report MUST route an upgrade from, counted as the destinations this binary's projection under the recorded parameters would add, drop, reclassify, or rewrite; the recorded `rk_version` names the binary that wrote the record and MUST prompt nothing on its own, because a release that changes no landed file leaves the target with nothing to take and the two facts answer different questions.

#### Scenario: The same target, judged and not

- GIVEN a landed target recorded by an older `rk` whose destinations this payload projects byte for byte, with a tuned seeded file and an edited rendered file
- WHEN `rk status` and `rk status --check` run
- THEN both print the same report, the plain run exits 0, the check exits 1 naming the rendered drift in its violations, and neither counts a pending destination nor routes to an upgrade the version gap alone does not earn

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-landing-classifies-its-target-first` — A landing classifies its target first

Where a setup or migration task finds no landing record at a target, the skills and the shared pre-flight gate MUST route by `rk assess`, which MUST report its evidence and exactly one verdict — `brownfield` where another tool's release marker or a payload destination is already present, `greenfield` where no release mechanism and no release history exists, and `needs-decision` where tags or a second long-lived branch exist with no mechanism behind them — writing nothing, touching no network, and exiting 0 on every verdict, because a target already releasing somehow that reads as a fresh start is how a repository ends up with two release paths.

#### Scenario: A target releasing through another tool carries no record

- GIVEN a repository holding a release tool's configuration and no `.release-kit/manifest.json`
- WHEN `rk assess --target . --json` runs
- THEN the report classifies `brownfield`, names the marker, and exits 0, so the routing skill loads the migration procedure instead of landing the payload beside the tool

#### Scenario: A recorded target is assessed

- GIVEN a repository carrying a landing record
- WHEN `rk assess --target .` runs
- THEN the report states the record and its version, and its next lines route to `rk status` rather than to a migration, whatever the verdict says

Verify: `cargo nextest run -E 'test(/^assess_/)'`

### `landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else

`rk adopt` MUST verify every `rendered` destination byte for byte against the rendered candidate, refuse listing every mismatch and every missing expected file in one run, and end a successful pass by writing only inside `.release-kit/` — the config and then the record, last, with its origin stating the adoption — leaving every payload destination untouched.

#### Scenario: A pre-record target is adopted

- GIVEN a repository running the convention with no record, matching what this payload renders
- WHEN `rk adopt --apply` runs
- THEN the manifest appears with `origin` set to `adopt`, the config appears beside it, every payload destination stays unchanged, and `rk status` then reports the landing

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

### `landing:the-landed-guards-hold-the-message-content` — The landed guards hold the message content

The landed commit-msg hook MUST refuse a message referencing a git-ignored path, carrying agent attribution, or naming a scope outside the one shape the landed title check admits, exempting only the release bot's request, recognized by exactly the title shape the landed title check admits for the bot. The scope's shape is a gate and its vocabulary is not: no landing records a scope list, the landed hooks require that a scope is present, and the routing block carries the guidance an author reads to choose the word. Where the forge carries the request's description onto the trunk, the landed required check MUST refuse a body matching the same patterns — the bot's request exempt whole, since the check has no ignore rules to consult — and the two pattern copies MUST be held equal by test.

#### Scenario: A message references an internal planning artifact

- GIVEN a commit whose body names a path the repository git-ignores, and a second whose subject scope reads `Specs Ugly` where `record/ids` would have passed
- WHEN the landed rk-message hook judges each message
- THEN each is refused, the first naming the line and the ignored path before it becomes the trunk's permanent record, the second naming the scope and the shape it must take

#### Scenario: The release bot's request passes with its generated body

- GIVEN release-plz's request titled `chore: release v0.3.0` whose body carries its own generated-with line and bot co-author trailer
- WHEN the landed rk-message hook judges the message
- THEN the attribution class is exempt by the title, the ignored-path class still runs, and a clean body passes

#### Scenario: The forge gate refuses a leaking body before the merge

- GIVEN a pull request on a forge whose squash message source is the request's body, its description naming a `.draft/` path or carrying attribution
- WHEN the landed pr-title check runs its body step
- THEN the check fails naming the finding's class, and the same body under the bot's title passes whole

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-arming-identity-is-the-bot` — The arming identity is the bot

Where the recorded style arms the release request, the landed workflow MUST arm it with the bot identity's token, MUST NOT arm it with the forge's default CI token, and MUST re-arm on every refresh of the request, because a merge made under the default token starts no workflow — leaving the bump merged, untagged, and unpublished with nothing reporting a failure — and because a forge that refreshes by replacing the request drops the arming with the request it replaces.

#### Scenario: The arm is made with the default CI token

- GIVEN a release request armed by a job authenticating as the forge's default CI token
- WHEN every required check passes and the forge merges
- THEN the bump lands, no workflow run starts, no tag and no publish follow, and nothing reports it — which arming under the bot identity is what prevents

#### Scenario: The forge replaces the request instead of refreshing it

- GIVEN a forge on which the bot closes an outdated request and opens a fresh one
- WHEN new work lands on the trunk
- THEN the same job arms the fresh request under the same bot identity, because the arming died with the request it was made on

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-changelog-quality-gate-is-the-squash-message` — The changelog quality gate is the squash message

Where the recorded style arms the release request, the changelog's quality MUST be held at the point the entry is generated from — the squash title the landed title check holds to the scoped convention and the body the landed content guard judges — because an armed request offers no window in which a human edit on its branch survives to the merge.

#### Scenario: An entry reads badly on an armed request

- GIVEN an armed trunk-style project whose generated entry misstates a change
- WHEN the operator looks for the correction window
- THEN there is none while the request stands armed: the correction is a disarm before the checks finish, or a changelog commit landed on the trunk that ships with the next release

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-routing-block-bounds-the-agents-initiative` — The routing block bounds the agent's initiative and reads as plain prose

The routing block MUST state that an agent acting in the target guides and never drives: that a request to change code authorizes the file changes alone, and that a git or forge action — creating, switching or deleting a branch, creating or removing a worktree, committing, pushing, tagging, and opening, updating or merging a pull request among them — happens only where the operator's request named that action. It is the one landed line no mechanism enforces — a hook and a forge protection bound the end state and cannot tell an agent from a person — so the target carries it as a sentence rather than leaving an agent to discover it by refusal. Every line of the block MUST also read as plain prose — a simple tense, no contraction, no modal outside can, will and must, no semicolon, no dash splicing two statements, and no sentence over 25 words — because the target owns none of these lines, so a prose gate the target runs over its own `AGENTS.md` finds a defect it cannot repair and the renderer is the only place that can answer it.

#### Scenario: The block is read for what it authorizes

- GIVEN the routing block release-kit renders into a target's `AGENTS.md`
- WHEN the test suite reads it
- THEN it carries the line bounding the agent's initiative, no line ordering an agent to branch, commit, or merge on its own, and no line a prose gate would report

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-nix-capability-is-a-recorded-opt-in` — The Nix capability is a recorded opt-in

A landing MUST include the Nix destinations only under an explicit opt-in recorded as a landing parameter, defaulting off, with a record predating the parameter reading as opt-out, because the projection must stay reproducible from the record: without the parameter, `status` cannot tell an absent-because-not-wanted file from a drifted one, and `upgrade` cannot decide whether to add the files.

#### Scenario: An old record meets a newer binary

- GIVEN a landed target whose record predates the parameter
- WHEN `rk upgrade` runs
- THEN no Nix destination joins the landing, and the rewritten record states the opt-out explicitly

Verify: `cargo nextest run -E 'test(a_pre_nix_record_upgrades_to_nothing_unrequested)'`

### `landing:the-flake-pair-lands-all-or-nothing` — The flake pair lands all-or-nothing

A landing MUST land the seed `flake.nix` and its matching `flake.lock` as a pair only where the target carries neither, withholding the pair with the reason named where either exists, because a seed lock beside a foreign flake describes the wrong input graph; the seeded package expression still lands, and a crate shape the seed does not support withholds the whole capability by name. The capability MUST land no CI file, because a job proving the build holds a merge only where the forge's own gate reaches it — inside the workflow the required check needs on GitHub, and inside the child pipeline the rendered parent triggers on GitLab, since that forge deep-merges an include and only a separate configuration isolates — and both of those files are the target's own.

#### Scenario: A target with its own flake opts in

- GIVEN a rust target carrying a `flake.nix` of its own
- WHEN `rk init --nix --apply` runs
- THEN `nix/package.nix` lands, the pair is withheld with the reason reported, no workflow is written, the withheld destinations stay out of the record, and a later `rk upgrade` reproduces the same decision

Verify: `cargo nextest run -E 'test(a_target_with_its_own_flake_keeps_it_and_the_pair_is_withheld)'`
