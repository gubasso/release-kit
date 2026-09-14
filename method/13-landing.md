# 13 — Landing

How a release-kit write reaches a repository. The installed binary stages its own candidate. An agent studies that stage against the working tree, the receipt, and Git. The same binary then renders again, from scratch, into production. The operator verifies, and only then does the stage go. [Setup](./02-setup.md) owns why a bare repository takes the convention in its order. [Migration](./10-migration.md) owns why a repository that already releases is carried across with a findings inventory. This chapter owns what the two share once bytes move. The command form of this chapter is the landing runbook, `rk guide landing`.

## The version is the operator's

`rk` renders one release: its own. The operator chooses the installed version through the manager the project already runs, and `rk self-depend` can assist that manager. No landing verb fetches, resolves, or runs another release, and no verb installs `rk`. A stage, a landing, and a status report therefore describe what the binary on `PATH` would do, and nothing else. A record whose `rk_version` is newer than the binary refuses and names the version to install, because rewriting a newer landing with older bytes is a downgrade rather than an upgrade.

Reading how the project obtains `rk` comes before any comparison. A version the operator did not choose is not a version to migrate to. Where the request did not authorize an update, the installed state is the whole report, and the migration work waits.

## The stage is evidence

A stage is one directory the installed binary writes for one target. `stage.json` states the installed version, the target, the resolved parameters, and one entry per candidate, omission, collision, retired destination, and preserved file. `artifacts/` holds every candidate destination at its target-relative path with its complete proposed bytes. A marked region appears as the whole proposed document. `reference/` holds the version-matched knowledge: the changelog, the guidance, the method, the bindings, the runbooks, the forge notes, and the `rk-setup` skill with the resources it routes to.

A stage is never an installation transaction. Production reads no byte and no decision from it, and no landing verb accepts a stage or its receipt as input. That independence is what makes the stage safe to keep: it stays through the landing and through verification, so the agent can compare the real Git diff against the staged benchmark at every point. It goes only when `rk stage clean <path>` names it, after everything passed.

## Ownership is elementary

Production decides each destination by its recorded kind alone, and its report uses one word per decision.

- `created`: the destination was absent, and the candidate now stands there.
- `replaced`: the receipt recorded a generated whole file or a marked region, and the candidate replaced it. Every byte outside a region survives.
- `matched`: no receipt named the whole file, and it already held the candidate's bytes.
- `preserved`: a seeded or state file stays as the target left it, and its current digest enters the receipt.
- `drift`: a seeded file differs from the receipt. It stays, and the report says so.
- `released`: the receipt named a destination this binary no longer produces. The file stays on disk and leaves the receipt, target-owned from that moment.
- `collision`: a whole file stands on disk, no receipt names it, and its bytes differ from the candidate. The verb refuses before its first write and names every such file.

A recorded generated file is replaced even where its bytes changed, because the operator and the agent authorized the migration and Git holds the recovery. An unattributed collision is refused, because a file the receipt cannot vouch for is the target's own. No flag forces past it. The agent's migration brings the file to the candidate, or `rk adopt` records a target already at the projection.

## The evidence classes

The comparison rests on two sources the target carries: the receipt at `.release-kit/manifest.json` and the Git history. Their presence decides how much the agent can attribute, and the class is stated before any file moves.

- Receipt and useful history: an attributed migration. Every recorded destination has an owner, and every edit has an author and a reason in the log.
- Receipt without useful history: a receipt-guided comparison. The kinds and digests say what release-kit wrote. The edits since have no story, so each one is shown.
- History without receipt: a history-guided heuristic. The log says when a file arrived and who touched it. Nothing says which kind it was.
- Neither: a best-effort heuristic. Every uncertain ownership decision is shown to the operator, and none is taken silently.

No class promises certainty, and no engine reconstructs a missing history by fetching an old release. Where the receipt or the history is absent, the agent investigates, the CLI refuses the unattributed collision, and the operator decides.

## Documentation is project-specific

A documentation file the projection selects for this target is an ordinary candidate: it appears in `artifacts/`, and production lands it by the same ownership rules. The `reference/` tree exists to teach the agent, and it is not copied into the project. A project receives the documentation its declared capabilities select, and nothing more.

## The sequence

1. Stage. Verify the installed version and how the project obtains it. Then have the installed binary write its candidate and keep the stage path.
2. Investigate. Read the stage receipt, the artifacts, the working tree, the receipt, the Git history, the changelog, and the guidance. State the evidence class. Prepare what the landing needs within the request's authority.
3. Land. Run the production verb, which renders again from the binary and the target. Read each decision word it prints, and return to the investigation on a refusal.
4. Verify. Compare the real Git diff with the staged benchmark, run `rk status --check`, the project's checks, and the forge checks. Repair through another fresh production invocation.
5. Clean. Show the exact cleanup command. Run it only where the request's authority includes cleanup, and remove any inactive legacy state only under that same authority.

## Legacy operation state

Release 0.4.x stored plans, results, run journals, and release caches under the state root. Before an operator-owned update from such a release, the installed binary is the one that reads them. An active operation is finished or abandoned with that binary before the tool is replaced, because the next binary carries no parser for it. Inactive state is recorded as local evidence: its exact paths, and why each is obsolete. The new binary offers no verb that removes it and no recursive target. Its removal is a one-time, agent-guided host cleanup, taken only under explicit cleanup authorization and only after no active old operation remains.

## Boundary tests

- A version gap alone is not work. A recorded target whose files match the candidate has nothing to take, whatever version the record names.
- A stage that survives the landing is not a leak. It is the benchmark verification reads, and it goes last.
- A refusal at the landing is a finding for the investigation, never a flag to add.
- A retired destination is the target's from the next receipt. Whether the migration removes it is the agent's finding for the operator, and production never deletes it.
- A partial landing is visible. Whole files stand beside the previous receipt, Git shows the diff, and the same command runs again.
- A stage receipt offered to a landing verb is refused. Production consumes no staging.
