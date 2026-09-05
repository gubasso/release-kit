# Runbooks

The human-facing step by step: the chapter and its runbook state each procedure exactly once, as a pair — the chapter owns each step's why, the runbook owns its how: the commands, their order, and the check each one prints. The pair shares the numbered step spine, a test holds the two to it, and a substep elaborates its step without adding one; every why belongs to a method chapter, a forge document, or a binding, and the runbook links to it. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

| Runbook                             | Renders                        | Serve with               |
| ----------------------------------- | ------------------------------ | ------------------------ |
| [release](./release.md)             | `rk method operate`            | `rk guide release`       |
| [setup](./setup.md)                 | `rk method setup`              | `rk guide setup`         |
| [backport](./backport.md)           | `rk method branch-for-release` | `rk guide backport`      |
| [release-lines](./release-lines.md) | `rk method release-lines`      | `rk guide release-lines` |
| [worktree](./worktree.md)           | `rk method worktrees`          | `rk guide worktree`      |
| [migration](./migration.md)         | `rk method migration`          | `rk guide migration`     |

`rk guide` substitutes what detection knows — the project path as `<repo>`, the technology as `<tech>` — and nothing else. Blocks labeled `On github:`, `On gitlab:`, `On rust:`, `On python:`, `On bash:`, `On worktree:`, `On branches:`, `On trunk:`, or `On lines:` are variants: when the axis is resolved, the matching block is kept and its siblings are dropped; unresolved, every variant prints with its label. The workflow axis resolves from the landing record's recorded mode, with `--workflow` as the override; the style axis resolves from the record's release style, with `--style` as the override; the forge and technology axes resolve by detection. Placeholders such as `<release pr>` exist only once a bot has opened them and are never substituted: a stale number merges someone else's work, where a visible placeholder fails loudly.
