# Runbooks

The human-facing step by step: the method rendered as commands, in order, with the traps called out. A runbook carries commands, their order, and the check each one prints; every why belongs to a method chapter or a binding, and the runbook links to it. A runbook never introduces a step the method does not have — its step count and order match the chapter it renders, and a test holds that.

| Runbook                 | Renders             | Serve with         |
| ----------------------- | ------------------- | ------------------ |
| [release](./release.md) | `rk method operate` | `rk guide release` |
| [setup](./setup.md)     | `rk method setup`   | `rk guide setup`   |

`rk guide` substitutes what detection knows — the project path, the forge, the technology — and nothing else. Blocks labeled `On github:`, `On gitlab:`, `On rust:`, `On python:`, or `On bash:` are variants: when the axis is resolved, the matching block is kept and its siblings are dropped; unresolved, every variant prints with its label. Placeholders such as `<release pr>` exist only once a bot has opened them and are never substituted: a stale number merges someone else's work, where a visible placeholder fails loudly.
