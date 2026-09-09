# Template — Guide

Copy the block below to `<root>/guides/<topic>/<task>.md`. A guide is executed rather than read: every step is one imperative action carrying its check, and the reader never guesses a field or a value.

````markdown
# <Imperative title naming the task>

<One sentence: what the reader has when the last step passes.>

## Preconditions

- <tool or access the steps assume>: `<command that proves it>`
- <state the steps assume>: `<command that proves it>`

## Steps

1. <One action in the imperative.>

   ```bash
   <command>
   # check: <what success prints>
   # <known divergence>: <the condition, and the step or page the reader goes to>
   ```

2. <One manual action, in the interface that owns it.>

   1. Open `<page>`, then click <control>.
   2. <Field>: `<value>`.
      - <the one fact the reader needs about this field>
   3. Click <control>.
      - check: <what the interface shows when it worked>

3. Verify the result.

   ```bash
   <command that shows the outcome>
   # check: <what a correct result looks like>
   ```
````

Checks before committing:

- Every step is one imperative action and carries its check (`guides:a-step-is-one-imperative-action`, `guides:every-step-carries-its-check`).
- Every value a step consumes is produced by an earlier step or a precondition (`guides:a-step-follows-its-producers`).
- Every interface step enumerates its fields, controls, and values (`guides:a-manual-step-enumerates-its-interaction`).
- Every known divergence states its condition and destination (`guides:a-divergent-result-names-its-destination`).
- Every upstream-owned fact was verified against the official source, cited as a dated entry in the reference zone (`guides:an-external-fact-is-verified-upstream`, `guides:citations-live-in-the-reference-zone`).
