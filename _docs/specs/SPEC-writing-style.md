# Writing Style Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`writing-style:an-existing-document-converts-when-edited` — An existing document converts when edited](#writing-stylean-existing-document-converts-when-edited--an-existing-document-converts-when-edited)
  - [`writing-style:the-documentation-block-routes-to-the-style` — The documentation block routes to the style](#writing-stylethe-documentation-block-routes-to-the-style--the-documentation-block-routes-to-the-style)
  - [`writing-style:no-delivered-gate-judges-prose` — No delivered gate judges prose](#writing-styleno-delivered-gate-judges-prose--no-delivered-gate-judges-prose)
  - [`writing-style:sources-name-the-revision-read` — Sources name the revision read](#writing-stylesources-name-the-revision-read--sources-name-the-revision-read)
  - [`writing-style:the-style-lives-in-one-document` — The style lives in one document](#writing-stylethe-style-lives-in-one-document--the-style-lives-in-one-document)
- [Unenforced](#unenforced)

<!--TOC-->

## Purpose

Rules governing how a project adopts and reaches the writing style. The style is the source the project selects in its declaration under `SPEC-writing-policy.md`: this convention's chapter, served by `sdd method writing-style`, a document of the project's own, or none. This spec does not restate any prose rule. An instance that already adopted `SPEC-simple-english.md` keeps that instance-owned copy, and `sdd` stops maintaining it.

## Requirements

### `writing-style:an-existing-document-converts-when-edited` — An existing document converts when edited

Where a writing-style source is selected and an author edits a document already in the tree, the author MUST apply the selected style to that document and MUST NOT sweep untouched documents.

A project whose selection is `none` owes no conversion, under `writing-policy:none-imposes-no-obligation`.

#### Scenario: A project adopts the style with an existing corpus

- GIVEN a corpus written before the project adopted the style
- WHEN an author edits one document
- THEN that document converts and every untouched document stays as it was

Verify: reviewer confirms each document the change edits follows the selected source, and that the change converts no untouched document

### `writing-style:the-documentation-block-routes-to-the-style` — The documentation block routes to the style

The documentation block MUST route an author to the selected writing-style source in one line, or carry no route where the selection is `none`, and MUST NOT carry the style's rules.

#### Scenario: An agent prepares to author prose

- GIVEN a root author-instructions file with the managed documentation block
- WHEN the agent reads the block
- THEN one line names the selected source, or none does, and no line restates a prose rule

Verify: `sdd verify --target .`

### `writing-style:no-delivered-gate-judges-prose` — No delivered gate judges prose

The distribution MUST NOT deliver a gate that judges prose against the writing style, and it MUST leave that judgment to review.

#### Scenario: An instance contains prose written under another convention

- GIVEN an existing project that installs this documentation method
- WHEN its delivered gates run
- THEN no gate rejects the project's prose for differing from the writing style

Verify: reviewer confirms the delivered gate registry contains no prose-style judge

### `writing-style:sources-name-the-revision-read` — Sources name the revision read

Where the style merges an external source, the style MUST name that source and the exact revision read.

#### Scenario: An upstream source changes after the style is authored

- GIVEN a source whose current branch moved
- WHEN a reader traces one of the style's layers
- THEN the recorded revision identifies the source text the author read

Verify: reviewer confirms each source row names a repository and a full revision

### `writing-style:the-style-lives-in-one-document` — The style lives in one document

The project MUST keep the writing rules in the one document its selection names and MUST route every other surface to that document without restating them.

For the `builtin` selection that document is `method/writing-style.md`, served by `sdd method writing-style`. For `project` it is the document the declaration's `path` names.

#### Scenario: A template needs to tell an author how to write prose

- GIVEN a template that provides author instructions
- WHEN the template adds writing-style guidance
- THEN it names the selected source instead of copying any rule

Verify: reviewer confirms every writing-style route points to the selected document and carries no copied rule

## Unenforced

These rules require review because structure alone cannot prove author intent.

| Rule                                                      | Reviewer confirms                                                                |
| --------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `writing-style:an-existing-document-converts-when-edited` | Each edited document follows the selected source, and no untouched one converted |
| `writing-style:no-delivered-gate-judges-prose`            | No delivered gate judges the writing style                                       |
| `writing-style:sources-name-the-revision-read`            | Every source has a repository and full revision                                  |
| `writing-style:the-style-lives-in-one-document`           | No other surface restates the rules                                              |
