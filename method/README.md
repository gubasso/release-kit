# Method

A release convention with one technology-agnostic spine and a small, named surface where technologies differ. A project adopts the spine as-is, then takes the binding for its technology.

Start with [the model](./00-model.md), hold [the invariants](./01-invariants.md), then follow the chapter for the work at hand: [setup](./02-setup.md) once per repository, [operate](./03-operate.md) for every release, [recovery](./04-recovery.md) when one goes wrong. [The diff surface](./05-diff-surface.md) is where the bindings plug in, and the two walkthroughs — [release from trunk](./06-release-from-trunk.md), [branch for release](./07-branch-for-release.md) — show each release style end to end. [Worktrees](./08-worktrees.md) owns the working-copy forms and the workflow mode that picks between them, and [release lines](./09-release-lines.md) owns a line's whole life.

| Chapter                                               | Owns                                                              |
| ----------------------------------------------------- | ----------------------------------------------------------------- |
| [00 — Model](./00-model.md)                           | The spine, the branch forms, the one-pull-request gate            |
| [01 — Invariants](./01-invariants.md)                 | What never varies, in any technology                              |
| [02 — Setup](./02-setup.md)                           | Bootstrapping a repository onto the convention, in order          |
| [03 — Operate](./03-operate.md)                       | Cutting a release, end to end                                     |
| [04 — Recovery](./04-recovery.md)                     | Withdrawing, unsticking, and hand-publishing                      |
| [05 — Diff surface](./05-diff-surface.md)             | The four axes a binding declares                                  |
| [06 — Release from trunk](./06-release-from-trunk.md) | The default style, walked through one release and one fix         |
| [07 — Branch for release](./07-branch-for-release.md) | The whole life of an older line, cut to deletion                  |
| [08 — Worktrees](./08-worktrees.md)                   | The two working-copy forms, the mode, and the worktree lifecycle  |
| [09 — Release lines](./09-release-lines.md)           | The line's own life: style, cut, candidate, promotion, retirement |

Every chapter is operator procedure. An agent reads one to say which step comes next and to check that a step was done; it runs a git or forge step only where the operator's request named that step.

Per-technology bindings live under [bindings](../bindings/README.md). The deterministic files a binding lands come from `rk init`; the pinned tool versions they assume are declared once, in the versions registry, readable with `rk versions`.
