# Guidance

One file per release that needs an operator step or an explanation a target must read before it takes the release. Guidance is versioned reference material. The installed binary ships every file whole, and `rk stage` copies them into the stage's `reference/guidance` for the agent to read. The agent selects the files above the target's recorded release whose destinations the target has. A changelog is written for a reader who decides whether to upgrade. A guidance file is read by an agent that already decided.

## Authoring rules

- Name the file by the version the release mints: `0.3.19.md` for a change that ships in release 0.3.19. On a `0.x` line a feature mints the next patch version, not the next minor one, because `features_always_increment_minor` is off here. Do not predict the version. Read it from the open release request's title, and where the file was named before that request existed, rename it there.
- The authoring gate proves the exact name only once a release candidate exists, which is the release branch, where release-plz has written the proposed version into `Cargo.toml`. On an ordinary branch it proves the weaker statement that some coverage names a version above the last tag, so a name that is wrong passes there and fails at the release.
- Open with one `#` heading naming the release.
- Follow the heading with a list of exactly two fields, in this order, each on one line. `destinations` names every landed path the step concerns, comma separated. `action` is `operator-step` for a step the operator takes or `landing-write` for a change the landing writes on its own.
- Write the body under two headings, `## What changed` and `## What to do`. State the step as a command or an edit the reader can take.
- Name every destination the step concerns. A file that names no destination cannot be filtered against a target and is refused by test.
- A release that changes a landed destination and needs no step is recorded in `index.toml` under `no_steps`, so the silence is deliberate. `index.toml` also carries `since`, the release above which every release either ships a file here or is recorded there; an agent reading a target recorded below it treats the guidance as partial and investigates the gap itself.

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
