# Forges

One document per supported forge, served by `rk forge <name>` and listed by `rk forge --list`. A forge document is the sibling of a binding: a binding answers the four technology axes, a forge document answers [the fifth](../method/05-diff-surface.md). Each carries four sections, in this order: Answers, how the forge names the release request, the gate, the protections, and the bot identity; Bootstrap, the step by step for the one action no command performs; Mapping, the concrete commands; Limitations, what the forge cannot enforce that the method asks for, stated plainly.

| Forge                 | CLI    | Setup tree      |
| --------------------- | ------ | --------------- |
| [github](./github.md) | `gh`   | `setup/github/` |
| [gitlab](./gitlab.md) | `glab` | `setup/gitlab/` |

What a forge document never carries: the release sequence, the invariants, another forge's facts, or any technology's specifics.
