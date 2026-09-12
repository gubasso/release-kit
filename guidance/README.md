# Guidance

One file per release that needs an operator step or an explanation a target must read before it takes the release. The engine ships every file whole in the bundle, selects the ones between a target's recorded release and the candidate, filters them against the destinations the target has, and carries the rest in the plan. A changelog is written for a reader who decides whether to upgrade. A guidance file is read by an agent that already decided.

## Authoring rules

- Name the file by the version that introduces the change: `0.3.19.md` for a change that ships in release 0.3.19. On a `0.x` line a feature or a fix mints the next patch version, so name the file for the version release-plz will mint.
- Open with one `#` heading naming the release.
- Follow the heading with a list of exactly two fields, in this order, each on one line. `destinations` names every landed path the step concerns, comma separated. `action` is `operator-step` for a step the operator takes or `plan-operation` for a change the plan carries as an operation.
- Write the body under two headings, `## What changed` and `## What to do`. State the step as a command or an edit the reader can take.
- Name every destination the step concerns. A file that names no destination cannot be filtered against a target and is refused by test.
- A release that changes a landed destination and needs no step is recorded in `compatibility.toml` under `guidance.no_steps`, so the silence is deliberate.

## Example

```markdown
# release-kit 0.3.19

- destinations: .envrc
- action: operator-step

## What changed

The verb moved.

## What to do

Edit the line.
```
