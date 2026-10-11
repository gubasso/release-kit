# rk-setup evaluation fixtures

One file per situation the `rk-setup` skill must handle, read by the evaluation tests in `tests/cli.rs`. Each fixture states the situation, the confirmation the gate presents, the expected route through the task files and the landing pipeline by `rk guide landing` step number, the authority boundaries the route holds, and how project documentation and the reference corpus stay distinct.

The header fields are what the tests select on. `classification` and `intent` name the gate's cell, `tasks` is that cell's text as the gate table states it, and `provision` says whether the `provision` task runs before the cell (`first`), alone (`only`), or not at all (`no`). `landing`, `receipt`, `history`, `legacy`, and `stage` describe the run.

The fixtures live here rather than beside the skill because `skills/` is a distribution root the binary embeds whole, and a fixture is not a skill.
