# blocks

The whole texts the binary writes outside `snippets/`: the routing and hook blocks it splices into a target, the post-merge hook it installs on a host, and the self-depend texts `rk self-depend add` serves and seeds for each manager and venue pair — the three flake fragments and the seed flake, the mise entry for the crate and for the release archive with the mise seed, the devbox entry and seed, the asdf line, the `.envrc` line, and the seed `.envrc` — carrying `RK_DEVSHELL_*` tokens that `src/self_depend/fragments.rs` renders from the pin grammar, so no payload file names the flake's owner. The `depend-*` files are what `rk depend add` serves and seeds for the four tool managers, carrying `RK_DEP_*` tokens that `src/depend/fragments.rs` renders from the source's own manifest and remote, so no payload file names a dependency.

`.in` files are templates carrying `RK_*` and `OWNER` tokens, substituted in `src/landing.rs`; the hooks fragment is not standalone YAML, and the token lines must survive formatting, which is what the suffix buys.

Every file ends with one enforced newline; the `.in` readers strip exactly one because the spliced form carries none, `post-merge-hook.sh` is written verbatim because a hook file ends in its newline, and byte-equality tests pin both round trips.
