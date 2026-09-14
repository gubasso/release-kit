# rk-setup evaluation fixtures

One file per situation the `rk-setup` skill must handle, read by the evaluation tests in `tests/cli.rs`. Each fixture states the situation, the expected route through `rk guide landing` by step number, the authority boundaries the route holds, and how project documentation and the reference corpus stay distinct. The header fields `case`, `landing`, `receipt`, `history`, `legacy`, and `stage` are what the tests select on.

The fixtures live here rather than beside the skill because `skills/` is a distribution root the binary embeds whole, and a fixture is not a skill.
