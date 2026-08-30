# Release guides

This repository's own instance of the release-kit convention, with its real values filled in. Every step shows the command by hand first, then the `rk` step that runs the same thing.

| Guide                      | Read it             | Covers                                                                                                                                     |
| -------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| [setup.md](./setup.md)     | Once per repository | The eighteen bootstrap steps: branches, permissions, the bot identity, the three protections, the first publish, and the trusted publisher |
| [release.md](./release.md) | Every release       | The nine operate steps, from landing the work to verifying the published version, with one worked example                                  |

The generic, technology- and forge-agnostic form of both is the shipped payload, served by `rk guide setup` and `rk guide release`; the reasoning behind either lives in `method/` and `bindings/`. These pages carry neither. They carry what a person types against `gubasso/release-kit`.
