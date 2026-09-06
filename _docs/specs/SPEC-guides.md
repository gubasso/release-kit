# Guides Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`guides:every-step-carries-its-check` — Every step carries its check](#guidesevery-step-carries-its-check--every-step-carries-its-check)
  - [`guides:a-manual-step-enumerates-its-interaction` — A manual step enumerates its interaction](#guidesa-manual-step-enumerates-its-interaction--a-manual-step-enumerates-its-interaction)
  - [`guides:a-step-follows-its-producers` — A step follows its producers](#guidesa-step-follows-its-producers--a-step-follows-its-producers)
  - [`guides:preconditions-open-and-verification-closes` — Preconditions open and verification closes](#guidespreconditions-open-and-verification-closes--preconditions-open-and-verification-closes)
  - [`guides:a-step-is-one-imperative-action` — A step is one imperative action](#guidesa-step-is-one-imperative-action--a-step-is-one-imperative-action)
  - [`guides:a-divergent-result-names-its-destination` — A divergent result names its destination](#guidesa-divergent-result-names-its-destination--a-divergent-result-names-its-destination)
  - [`guides:an-external-fact-is-verified-upstream` — An external fact is verified upstream](#guidesan-external-fact-is-verified-upstream--an-external-fact-is-verified-upstream)
  - [`guides:citations-live-in-the-reference-zone` — Citations live in the reference zone](#guidescitations-live-in-the-reference-zone--citations-live-in-the-reference-zone)

<!--TOC-->

## Purpose

Rules governing step-by-step guides — the documents a reader executes rather than reads. Covers the step, its check, its ordering, and the upstream sources behind the facts a guide states. The markdown a guide is written in belongs to `SPEC-docs-format.md`; where a guide is placed and named belongs to `SPEC-docs-foundations.md`.

## Requirements

### `guides:every-step-carries-its-check` — Every step carries its check

The author MUST give every step's outcome a check stating what a correct result looks like, at the step where that outcome becomes observable.

#### Scenario: A command prints something the reader cannot judge

- GIVEN a step whose command prints a JSON document
- WHEN the guide says nothing about what success prints
- THEN the reader cannot tell success from failure, and the step gains a check line naming the expected output

Verify: reviewer confirms each step carries a check stating what a correct result looks like

### `guides:a-manual-step-enumerates-its-interaction` — A manual step enumerates its interaction

Where a step is performed in an interface rather than a shell, the author MUST enumerate every field, control, and value the reader sets as sub-items of the step.

#### Scenario: A form carries more choices than the guide names

- GIVEN a step that says to create an access token with the right scopes
- WHEN the form offers five scope checkboxes, an expiry, and a name pattern
- THEN the reader guesses at what the author meant, and the step lists each field with its value instead

Verify: reviewer confirms each interface step lists every field, control, and value the reader sets

### `guides:a-step-follows-its-producers` — A step follows its producers

Where a step consumes a value, a file, or a state, the author MUST have an earlier step or a stated precondition produce it.

#### Scenario: A value appears before the step that mints it

- GIVEN a step that pastes a credential into a command
- WHEN no earlier step and no precondition minted that credential
- THEN the reader stops mid-procedure to hunt for it, and the producing step moves ahead of the consumer

Verify: reviewer traces each value a step consumes to an earlier step or a stated precondition

### `guides:preconditions-open-and-verification-closes` — Preconditions open and verification closes

The author MUST open a guide with the preconditions its steps assume and close it with a verification stating what a correct result looks like.

#### Scenario: A guide assumes a tool the reader lacks

- GIVEN a guide whose third step runs a tool the reader never installed
- WHEN the failure surfaces three steps in
- THEN the requirement was a precondition, checkable before step one, and the guide states it there

Verify: reviewer confirms the guide opens with checkable preconditions and closes with a verification

### `guides:a-step-is-one-imperative-action` — A step is one imperative action

The author MUST write every step as one action in the imperative, numbered in the order the reader performs it.

#### Scenario: A step bundles two actions

- GIVEN a step that creates a token and stores it in the keyring
- WHEN the first half succeeds and the second fails
- THEN the reader cannot say which step failed, and the bundle becomes two steps

Verify: reviewer confirms each step is one imperative action

### `guides:a-divergent-result-names-its-destination` — A divergent result names its destination

Where a rerun or a known failure produces a result other than the check, the author MUST state the condition and where the reader goes next.

#### Scenario: A rerun prints something the check does not name

- GIVEN a rerunnable step whose second run reports the resource already exists
- WHEN the guide names only the first run's output
- THEN the reader reads success as failure, and the step gains a line stating the condition and its destination

Verify: reviewer confirms each known divergence states its condition and its destination

### `guides:an-external-fact-is-verified-upstream` — An external fact is verified upstream

Where a guide states a fact an upstream owns — an interface field, a tool's behavior, a default — the author MUST verify it against the official upstream source before stating it.

#### Scenario: A form is described from memory

- GIVEN a step enumerating a third-party form
- WHEN the fields are written from recall rather than from the upstream source
- THEN the guide asserts fields the form no longer has, and the enumeration is rewritten from the source

Verify: reviewer confirms each upstream-owned fact was checked against the official source

### `guides:citations-live-in-the-reference-zone` — Citations live in the reference zone

Where a fact rests on an upstream source, the author MUST record the citation as a dated entry in the reference zone that the guide links, keeping the guide itself lean.

#### Scenario: Sources pile up inside the guide

- GIVEN a guide whose steps each quote the documentation behind them
- WHEN the reader is executing rather than auditing
- THEN the citations crowd out the procedure, and they move to a dated reference entry the guide links once

Verify: reviewer confirms each upstream fact has a dated citation in the reference zone
