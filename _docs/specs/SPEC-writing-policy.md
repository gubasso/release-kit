# Writing Policy Specification

## Purpose

Rules governing where a project's writing style comes from. A project states one source in `.spec-driven-docs/config.yaml`, the managed documentation block routes authors to it, and the style itself is whatever that source says. This spec owns the selection and its three values. How a selected style binds an author, and the guarantee that no delivered gate judges prose, belong to `SPEC-writing-style.md`. The chapter this convention offers is served by `sdd method writing-style`.

```yaml
writing_style:
  source: builtin   # builtin, project, or none
  path: null        # the project's own document, where the source is project
```

`builtin` routes authors to `sdd method writing-style`. `project` routes them to the document `path` names, relative to the repository root. `none` installs no route and imposes no conversion obligation. `sdd init --writing-style <selection>` records the choice at install time, and editing the file followed by `sdd hooks --apply` changes it afterwards, because that verb rewrites the documentation block in the root author-instructions file as well as the pre-commit block.

## Requirements

### `writing-policy:the-project-selects-one-source` — The project selects one source

The project MUST select exactly one writing-style source in its declaration, and the tool MUST route authors to that selection from the managed documentation block and refuse a declaration whose fields disagree.

`project` without a `path` names nothing. A `path` beside another source is a value nothing reads, and a value nothing reads drifts. A `path` that is absolute or leaves the repository is refused for the reason the filter grammar refuses one: the route must resolve the same in every checkout.

#### Scenario: A project with a house style adopts the method

- GIVEN a project whose contributors follow a style guide of their own at `docs/STYLE.md`
- WHEN it declares `source: project` with that path and runs `sdd hooks --apply`
- THEN the documentation block routes authors to `docs/STYLE.md`, no line names this convention's chapter, and `sdd verify` reports the block in agreement with the declaration

Verify: `sdd verify --target .`

### `writing-policy:none-imposes-no-obligation` — Selecting none imposes no obligation

Where the selection is `none`, the tool MUST install no writing-style route, and no rule of this convention MUST bind an author to convert a document's prose.

A project wanting the structure without the register declines the style rather than ignoring it. An agent editing its documents follows whatever the project's own instructions say, and a reviewer holds no conversion rule against the change.

#### Scenario: A project wants the documentation method and not the prose register

- GIVEN a project that declares `source: none`
- WHEN an author edits an existing document
- THEN the managed block carries no writing-style line, and the review confirms no conversion was owed

Verify: `sdd verify --target .`
