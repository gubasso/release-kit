# Method

A release convention with one technology-agnostic spine and a small, named surface where technologies differ. A project adopts the spine as-is, then takes the binding for its technology.

Start with [the model](./00-model.md), hold [the invariants](./01-invariants.md), then follow the chapter for the work at hand: [setup](./02-setup.md) once per repository, [operate](./03-operate.md) for every release, [recovery](./04-recovery.md) when one goes wrong. [The diff surface](./05-diff-surface.md) is where the bindings plug in.

| Chapter                                   | Owns                                                     |
| ----------------------------------------- | -------------------------------------------------------- |
| [00 — Model](./00-model.md)               | The six-stage spine and the two-pull-request shape       |
| [01 — Invariants](./01-invariants.md)     | What never varies, in any technology                     |
| [02 — Setup](./02-setup.md)               | Bootstrapping a repository onto the convention, in order |
| [03 — Operate](./03-operate.md)           | Cutting a release, end to end                            |
| [04 — Recovery](./04-recovery.md)         | Withdrawing, unsticking, and hand-publishing             |
| [05 — Diff surface](./05-diff-surface.md) | The four axes a binding declares                         |

Per-technology bindings live under [bindings](../bindings/README.md). The deterministic files a binding lands come from `rk init`; the pinned tool versions they assume are declared once, in the versions registry, readable with `rk versions`.
