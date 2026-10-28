# 13 — Landing

How a release-kit write reaches a repository. The installed binary stages its own candidate. The same binary compares that stage with the working tree, the receipt, and Git, and the operator answers each conflict. The binary then renders again, from scratch, into production, and compares once more as the proof. [Setup](./02-setup.md) owns why a bare repository takes the convention in its order. [Migration](./10-migration.md) owns why a repository that already releases is carried across with a findings inventory. This chapter owns what the two share once bytes move. The command form of this chapter is the landing runbook, `rk guide landing`.

## The version is the operator's

`rk` renders one release: its own. The operator chooses the installed version through the manager the project records. The `provision` task of the setup skill proposes that manager, records the answer in the repository, and runs the install as a step the operator approves. `rk self-depend` serves the manager's fragment, and a host install is recorded as the acquisition answer in `.release-kit/config.toml`. No `rk` verb installs `rk`: the manager the operator picked does. No landing verb fetches, resolves, or runs another release. A stage, a landing, and a status report therefore describe what the binary on `PATH` does, and nothing else. A record whose `rk_version` is newer than the binary refuses and names the version to install, because rewriting a newer landing with older bytes is a downgrade rather than an upgrade.

Reading how the project obtains `rk` comes before any comparison. A version the operator did not choose is not a version to migrate to. Where the request did not authorize an update, the installed state is the whole report, and the migration work waits.

## The stage is evidence

A stage is one directory the installed binary writes for one target. `stage.json` states the installed version, the target, the resolved parameters, and one entry per candidate, omission, collision, retired destination, and preserved file. `artifacts/` holds every candidate destination at its target-relative path with its complete proposed bytes. A marked region appears as the whole proposed document. `reference/` holds the version-matched knowledge: the changelog, the guidance, the method, the bindings, the runbooks, the forge notes, and every skill package as installed, with its references and its gate copies.

A stage is never an installation transaction. The production verb reads the stage's parameters, never its bytes: `rk init`, `rk upgrade`, and `rk adopt` take `--stage <root>`, read the answers the receipt resolved, and render again from scratch. A receipt another binary wrote, or one taken for another target, is refused. The choices are stated once, at the stage, so the landing cannot differ from the stage the proof compares against. That independence is what makes the stage safe to keep: it stays through the landing and its proof. It goes only when `rk stage clean <path>` names it, after every check passed.

## Ownership is elementary

Production decides each destination by its recorded kind alone, and its report uses one word per decision.

- `created`: the destination was absent, and the candidate now stands there.
- `replaced`: the receipt recorded a generated whole file or a marked region and vouches for its bytes, and the candidate replaced it. Every byte outside a region survives. A conflict the operator named with `--replace` is replaced too, after its bytes are saved.
- `matched`: no receipt named the whole file, and it already held the candidate's bytes.
- `preserved`: a seeded or state file stays as the target left it, and its current digest enters the receipt.
- `drift`: a seeded file differs from the receipt. It stays, and the report says so.
- `released`: the receipt named a destination this binary no longer produces. The file stays on disk and leaves the receipt, target-owned from that moment.
- `collision`: the verb refuses before its first write and names every such destination. It is a conflict where a whole file stands on disk that no receipt names and whose bytes differ from the candidate, or where a recorded generated file or region holds bytes the receipt no longer vouches for, because someone edited it.

A conflict is refused because its bytes are someone's own work, and Git is not the only copy a landing may count on. No flag forces past it. Only a named `--replace <path>` passes it, and the verb first saves the conflict's bytes to `replaced/<path>` under `$XDG_STATE_HOME/release-kit/snapshots/<repo>/<timestamp>/`, a migration open or not, and the report names where. A saved copy never lives inside the stage, so cleaning the stage never deletes one. Every conflict the operator did not name still refuses, all of them before the first write, so an upgrade or a repair lands whole or not at all. A file that already holds the candidate's bytes is no conflict, because that is what a landing stopped part way leaves behind.

## The evidence classes

The comparison rests on two sources the target carries: the receipt at `.release-kit/manifest.json` and the Git history. `rk stage compare` states, for each destination whose bytes differ, how much those two sources attribute:

- `attributed`: the receipt vouches for the bytes on disk, and the history touched the path. An older release wrote them, and the log says when.
- `reduced`: one of the two holds. A file someone edited after the landing reads here or below, never `attributed`, because the receipt no longer vouches for it.
- `unknown`: neither holds. Every decision about the file is the operator's.

A destination no receipt names also says whether its history shows an edit after the commit that brought it: a file untouched since then carries an older release's bytes, and one edited after it carries a person's change. No class promises certainty, and no engine reconstructs a missing history by fetching an old release. Where Git is absent, the history half reads as missing and the report says why. The agent asks the operator about each conflict, and the CLI refuses every one the operator did not name.

## Documentation is project-specific

A documentation file the projection selects for this target is an ordinary candidate: it appears in `artifacts/`, and production lands it by the same ownership rules. The `reference/` tree exists to teach the agent, and it is not copied into the project. A project receives the documentation its declared capabilities select, and nothing more.

## The sequence

1. Stage. Verify the installed version and how the project obtains it. Then have the installed binary write its candidate with the task's choices, and keep the stage path. A recorded target's configuration answers every choice the task does not change.
2. Compare. Run `rk stage compare`, and read each destination's state, kind, provenance, and the word production will print. Judge only a collision. Where the comparison marks it replaceable, ask the operator whether to keep the edit, whose copy is saved and then replaced, or to discard it. Any other collision is a path the landing refuses until the operator fixes it by hand.
3. Prepare. Gather the operator's answers and the task's own items within the request's authority. Copy nothing out of the stage into the target.
4. Land. Run the production verb with `--stage <root>`, which renders again from the binary and the target, and read its preview. Then apply it, naming each answered conflict with `--replace <path>`.
5. Prove. Run `rk stage compare` again: every rendered destination reads `equal`, and `rk status --json` reports `classification: current`. Keep the stage.

The checks every run ends with, `rk status --check` and the project's own suite, and the stage cleanup after them, belong to the setup skill's general post-flight, not to this sequence. A failure there is repaired through another fresh production invocation, never by hand from the stage.

## A landing with no release

A project that wants the Git workflow and no release lands with one preset. `--workflow-only` on `rk init` and `rk upgrade` sets four answers: `release.mode = "none"`, `git.integration = "local"`, `git.checkout_mode = "linked-worktree"`, and `capabilities.workflow_guards = true`. The preset records nothing of its own, only those four answers, because a key with one legal value restates an invariant. A flag that contradicts one of the four refuses before any write. A flag the preset does not set, such as `--merge-form`, combines with it.

Under `release.mode = "none"` the landing carries nothing whose reason is a release:

- The routing block takes its workflow variant, with no `## Releases` heading and no version, changelog, or tag reason.
- The glossary names no release term.
- The tag hook `rk-no-hand-authored-tag` does not land.
- The configuration leaves out the tag protection keys, `[security]`, and `[setup.bot]`.
- A commit scope is preferred and not required, in `rk integrate`, `rk message`, and the landed commit hooks.

The title gate lands only under forge integration or an automatic release, so a local, release-less project gets no CI file. Under local integration the workflow guards add the trunk guard at the pre-commit stage, and two pre-push guards against a force push and a merge commit on the trunk. [Setup](./02-setup.md) owns what the forge then requires.

## Legacy operation state

Release 0.4.x stored plans, results, run journals, and release caches under the state root. Before an operator-owned update from such a release, the installed binary is the one that reads them. An active operation is finished with that binary before the tool is replaced, or the operator decides to abandon it, because the next binary carries no parser for it. Inactive state is recorded as local evidence: its exact paths, and why each is obsolete. The new binary offers no verb that removes it and no recursive target. Its removal is a one-time, agent-guided host cleanup, taken only under explicit cleanup authorization and only after no active old operation remains.

## Boundary tests

- A version gap alone is not work. A recorded target whose files match the candidate has nothing to take, whatever version the record names.
- A stage that survives the landing is not a leak. It is the benchmark verification reads, and it goes last.
- A refusal at the landing is a finding for the comparison, never a flag to add. Only a named `--replace`, after the operator answered the conflict, passes it.
- A retired destination is the target's from the next receipt. Whether the migration removes it is the agent's finding for the operator, and production never deletes it.
- A partial landing is visible. Whole files stand beside the previous receipt, Git shows the diff, and the same command runs again.
- A landing from a stage reads its parameters and renders again. A receipt from another binary or another target is refused, and no byte of the stage reaches the target.
