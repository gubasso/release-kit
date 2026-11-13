# 12 — Integration

Integration moves one implementation onto the trunk. A project records which authority performs that move. The choice is independent of where the implementation is edited.

## The two axes

`git.checkout_mode` answers where an implementation is edited. `linked-worktree` gives every code-changing branch its own linked working tree, and keeps the main checkout free of commits. `main-worktree` switches the original working tree to the branch. [Worktrees](./08-worktrees.md) owns that axis and its guards.

`git.integration` answers who moves the implementation onto the trunk. Under `local`, the checkout performs and records the integration. Under `forge`, the forge performs and records it through a pull request or a merge request.

The axes are orthogonal, and every pairing works. Parallel work still takes one linked working tree per writer, whatever the integration authority. Git refuses one branch in two working trees, and release-kit never overrides that refusal.

`git.integration` is a recorded Git workflow parameter. The landing verbs resolve it in this order:

1. A flag.
2. The committed configuration.
3. A compatible record.
4. A compiled default.

The verbs write it to `.release-kit/manifest.json` and render it into the committed hook block and the routing block. `rk status` reports it, and `rk status --check` judges it. To change it, run `rk upgrade --integration <mode> --apply`. Its committed diff reaches every clone through the trunk, and no ad hoc local toggle replaces it. The compiled default is `local`. A record written before this axis existed answers `forge`, because that is what such a target landed.

## The merge form

`git.merge_form` answers how the commits of a branch reach the trunk, under either authority. `squash` writes one commit for the branch, so one implementation is one commit. `rebase` rebases the branch on the trunk tip and fast-forwards the trunk to it, so every commit of the branch arrives with its own message. Both keep the trunk linear, and neither writes a merge commit.

The landing verbs resolve the form by the same precedence as `git.integration`. The compiled default is `squash`. A record written before the parameter existed answers `squash`, because that is what such a target landed.

The form decides which messages the trunk keeps, so it decides which messages a gate judges. Under `squash`, the gate judges the one message of the squash commit. Under `rebase`, it judges every commit of the branch, because each one becomes a trunk commit.

`rk integrate` reads the record alone, never the committed configuration. The record is what landed. The hook block that admits or refuses its writes renders from it. So do the routing block that an agent reads and the forge protections that the setup installed. An edited configuration is pending input to the next landing. Taking it at execution time would integrate locally in a target whose installed controls still say forge.

## The invariant split

An implementation reaches the trunk through a short-lived branch. The project validates the branch and merges it in the recorded merge form, and the trunk stays releasable at every commit. The recorded integration mode chooses the authority that performs that integration. A pull request is one integration mechanism, and it is not the definition of trunk-based development. [The model](./00-model.md) carries the branch forms. [The invariants](./01-invariants.md) carries the two rules this chapter splits.

The release request always integrates at the forge. Its bot branch carries the computed version and the new changelog entry. Merging it is the only event that authorizes these:

- The tag.
- The registry publish.
- The provenance.
- The artifacts.

Local implementation integration removes the forge from nothing in the release path. So the trunk carries ordinary commits in either mode. It carries one per implementation under `squash`, and every commit of the branch under `rebase`. A later push can carry several of them at once. The release bot reads the release intent of each one separately. It counts only a commit that [its binding](../bindings/README.md) attributes to the package. So a batch of local integrations produces the changelog that a sequence of forge merges would produce.

## The gate boundary

A project assigns every check to the earliest native `pre-commit` stage that can run it. The stage before an irreversible boundary carries every check that boundary needs. The boundary moves with the integration mode.

| Stage              | Under `forge`         | Under `local`                         |
| ------------------ | --------------------- | ------------------------------------- |
| `pre-commit`       | cheap author checks   | cheap author checks                   |
| `pre-push`         | the full branch suite | the trunk re-validation, on the push  |
| `pre-merge-commit` | does not fire         | the full suite, run by `rk integrate` |
| `manual`           | an operator sweep     | an operator sweep                     |

One rule produces both columns. Every check runs at the last moment before the work crosses a boundary that it cannot be pulled back from. Under forge integration, that boundary is the branch push, because the forge sees the branch from then on. Under local integration, the branch never leaves the checkout. So the boundary is the integration itself, and `pre-push` fires on the trunk push instead.

git fires no hook during a local integration. `rk integrate` writes the squash with `git commit-tree` and moves the trunk by a fast-forward. `git merge` runs `pre-merge-commit` only before a merge commit, and a linear trunk never takes one. So `rk integrate` asks git for that hook itself. It points the seat's `HEAD` at the trunk commit and leaves the index and the files at the branch tip. The staged change is then exactly the squash. Then it runs:

```bash
git hook run pre-merge-commit
```

git runs whatever hook the clone installed, through its own discovery, `core.hooksPath` included. The project's hook manager, such as prek or pre-commit, selects the files from the staged change natively. It skips a check whose files the branch did not touch. It runs an `always_run` check either way. A clone with no `pre-merge-commit` hook fails the gate with git's own error. Release-kit names no hook runner and looks for none.

prek runs each nested `.pre-commit-config.yaml` as a project of its own, and pre-commit does not. A project that runs prek lists a fixture directory that holds such a file in a root `.prekignore`.

Release-kit reads none of these:

- A hook identifier.
- A revision.
- A language.
- A test category.

The project owns everything in `.pre-commit-config.yaml` outside the marked block. It owns its stage assignment and the second axis below. That judgment lives in the project's own agent instructions. The `rk-setup` skill researches the current hooks for the project's technologies.

## The scope axis

The stage says when a check runs. Scope says where it can run at all. Scope comes from the same boundary rule: a check runs at every boundary that it can guard. The two values other than `both` name the two ways a check cannot.

| Scope        | What it means                                          | How a project expresses it                              |
| ------------ | ------------------------------------------------------ | ------------------------------------------------------- |
| `both`       | the check runs at a desk and in continuous integration | a hook at its native stage, with CI invoking that stage |
| `local-only` | the check is meaningless in continuous integration     | a hook whose id the CI sweep names in `SKIP`            |
| `ci-only`    | the check is meaningless at a desk                     | no hook. It names the remote boundary it guards and why |

Most checks are `both`, and that value carries a rule. Such a check must have a local execution path and a remote one of equivalent coverage. The commands do not have to be identical. A desk that runs a suite through a devshell meets the rule, and so does a runner that runs it through a task runner. A check that exists on one side alone and claims `both` does not.

Moment has two forms, because a `ci-only` check has no honest hook stage. A check with a local path declares its earliest native stage. A `ci-only` check declares the remote boundary that it guards instead, usually the pull request. It states beside it the reason that it cannot run at a desk.

Under local integration, the `pre-merge-commit` hook is the strongest gate a project has. A project whose continuous integration does not run the same checks has a gate on one side of the boundary only. This axis exists to make that failure visible.

Continuous integration invokes the stages that the project declared, through the project's own devshell or task runner. Release-kit lands no CI workflow and judges no parity. A claim reverse-engineered from a forge workflow file produces refusals that a project cannot act on. So the convention is prose, and an agent holds it at authoring time. `rk status --check` judges the marked block it owns and nothing else.

## Local integration

Local integration is the default. A single-writer project integrates one implementation after another with no forge round trip. It pushes the trunk when it decides to.

Under this mode, the branch is born and dies in the checkout. It reaches no forge, so it has none of these:

- A pull request.
- A remote tip.
- A review artifact.

That is the mode's whole saving and its whole cost.

`rk integrate <branch>` performs the transaction, and it is the only path that writes a local trunk commit. In order, it does these things:

1. It resolves the branch and its seat.
2. It refuses a dirty seat and a branch off the grammar.
3. It refreshes the trunk, and brings the branch onto that tip.
4. It runs the clone's `pre-merge-commit` hook on the staged squash.
5. It observes the branch again.
6. It builds the squash commit as an object that no ref names.
7. It stages the evidence that the prune verbs read.
8. It publishes the commit against the trunk tip that it observed before the gate ran.

Where no worktree holds the trunk, the publication is one compare-and-swap of the trunk ref, carrying that tip. Where a worktree holds it, the command reads the trunk and its holder again right before the publication, and refuses on any difference. It then fast-forwards that worktree. It confirms that the move started at the observed tip, from the trunk's reflog or from the full object names in the fast-forward's report.

Git offers no compare-and-swap for a checked-out branch that also moves its worktree. So a writer outside rk can still move the trunk between that reading and the fast-forward. Once the fast-forward succeeded, the trunk is published. A source can then show another start, or neither source can confirm the move. In both cases, the command reports the integration as published and exits nonzero with the reason `state-drift`. It names the commit where the move started, or why the start is unconfirmed, and it rolls nothing back.

A refresh of a held trunk before the gate can complete and stay unconfirmed. That refuses the integration, and the command says that it refreshed the trunk to its remote tip.

Under `rebase`, the command builds no commit. Its rebase flattens every merge and moves no other branch, whatever the Git configuration asks. The published range holds no merge commit, and the command reads it back before it publishes. It judges the message of every commit between the trunk and the branch tip. It stages evidence that names the branch tip, and moves the trunk to that tip by the same publication. The trunk then carries the same commits as the branch, in order. The command refuses `-m` under `rebase`, because every commit keeps its own message.

The command undoes nothing, because nothing is published early. A trunk that moved under the gate refuses with no ref touched. So does a trunk whose holder changed. Evidence that names a commit the trunk does not reach proves nothing. So a refusal at the last step leaves inert residue, not a false proof.

The command holds two things that a forge would hold for it. First, every message it publishes passes the whole landed `commit-msg` judgment here, because the command that moves the trunk fires no hook. So a message with agent attribution, or with a reference to an ignored path, refuses at the desk before it reaches a permanent history. Second, the command observes the branch before the gate and again after it. The integrated tree and the tip that the evidence certifies are the one object that the gate judged. So a commit that lands in the seat during the transaction refuses. It neither rides in ungated nor leaves evidence for work that the trunk does not carry. A hook that commits or stages a change in the seat also refuses, and the seat stays as the hook left it. [Recovery](./04-recovery.md) gives the repair.

The command never pushes. A preview refreshes nothing. It does none of these:

- It takes a lock.
- It runs a fetch.
- It moves a ref.

The trunk push is a separate operator action, and it takes the fast-forward form alone. Where another writer wins the push, the loser does these things in order:

- It fetches.
- It replays the implementation.
- It runs the gates again.
- It retries.

No local integration uses a force-push.

The local path cannot observe remote checks before the trunk push, because the push starts them. Whoever pushes watches the resulting run and the release request. That person repairs a failure as a fresh implementation, integrated the same way.

## Forge integration

Forge integration runs the project's own `pre-push` stage by pushing the branch. It then names the request command for the forge that the remote resolves to. Opening the request is the operator's act. No verb here authors a request body. `rk message --check --kind body` judges one. The forge holds the merge behind its required check, and performs it in the recorded merge form. The request is the integration record and the place where review happens.

Under `squash`, the title of the request becomes the message of the trunk commit, so the title gate judges the title. Under `rebase`, every commit of the branch reaches the trunk with its own message. So the request command asks for no squash, and the GitLab command carries no `--squash-before-merge`. A commit gate then judges every commit of the request on each push, in place of the title gate. A GitHub rebase merge writes the commits again with new SHAs. So on GitHub, the merged request stays the proof that retires the branch.

A project with more than one writer and a review requirement uses forge integration. No local gate replaces a second person who reads a diff, and a local default does not make that gap smaller.

A recorded default binds no single execution. `--forge` and `--local` select the other authority once, without changing the record. Work that an issue already names defaults to forge integration, because the forge minted that branch and owns it. An agent states which authority an execution uses. It never replaces a request with a local merge in silence.

## Protection and enforcement

Under forge integration, the trunk keeps every protection that [setup](./02-setup.md) installs:

- No direct push.
- No force-push.
- A request that carries the named passing check.
- The merge method of the recorded form as the only one.

Under local integration, GitHub keeps every trunk rule. This includes the request rule and the strict required check, wherever a forge gate lands. GitHub holds them in two rulesets:

- The safety ruleset carries deletion and force-push protection, and names no bypass actor.
- The trunk ruleset carries the request rule and the required check. It grants exactly the repository-administrator role an always bypass, for the deliberate direct push.

The split is not decoration. A ruleset records a bypass actor, and the ruleset excuses that actor from every rule in it. One ruleset with all four rules would hand the operator the force-push and the deletion along with the push. The release App is a distinct integration actor. It holds no repository role and reaches neither bypass. So its release request still cannot merge after the trunk advances.

GitLab carries the local push authority through `protection.gitlab.push_access_level`. `rk setup` installs the shape that the recorded mode names, and its check expects that same shape. So the setup neither locks a target that records `local` out of its own integration nor strips its atomic release freshness. Every tag protection stays the same in both modes.

A locally integrated target with no automatic release lands no forge gate, because no request merges at its forge. Its GitHub trunk ruleset then requires no status check, and its GitLab trunk requires no pipeline, because nothing would report one. No-deletion and no-force-push still hold. Under `release.mode = "none"`, the target carries no tag protection either.

The local-mode release workflow retries the merge, but it is not the atomic protection. It wakes when the named workflow completes and after its push-side request job succeeds. It finds candidates across every result page, and proves the identity of the request against the forge. It attempts the merge only once the same recorded check concluded with success for the exact head. Any other answer is a successful no-op that leaves the request open. A merge that the trunk's own rules refuse is a no-op too. The job names what the forge answered and ends green, because a stale request withheld is the protection working. The ruleset makes the final base-freshness judgment at the merge boundary.

The desk and the forge remain the two distances that [setup](./02-setup.md) describes. What changes is how far the forge's refusal reaches. Under local integration with the workflow guards, the desk refuses a commit on the trunk at the pre-commit stage. Two pre-push guards refuse a force push to the trunk and a push that carries a merge commit onto it. `rk integrate` passes them, because it moves the trunk ref and fires no commit hook. A hook is discipline, not a boundary. It dies to `--no-verify` in either mode. Four things catch a bypass:

- The integration transaction.
- The trunk gate.
- The post-push run.
- The release request.

The release request refuses to go green on a broken trunk.

## The release boundary

Pushing the trunk starts ordinary continuous integration and refreshes the release request. A failing post-push check leaves that request open, so nothing publishes. The retry job logs the non-success answer and exits with success, because the refusal is policy, not a broken workflow. It erases no local integration, and the next integration repairs it.

The release request stays the publication gate in both modes. [Operate](./03-operate.md) owns the sequence from its merge onward.

## Where this connects

- [The model](./00-model.md) holds the branch forms.
- [The invariants](./01-invariants.md) hold the split that this chapter performs.
- [Setup](./02-setup.md) chooses the mode, and [landing](./13-landing.md) brings it to a target.
- [Worktrees](./08-worktrees.md) holds the seats that integration starts from.
- [The integration runbook](../runbooks/integration.md), `rk guide integration`, is the step-by-step form.
