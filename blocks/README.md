# blocks

The whole texts the binary writes outside `snippets/`: the routing and hook blocks it splices into a target, the post-merge hook it installs on a host, and the devshell texts `rk devshell add` serves and seeds — the three flake fragments, the `.envrc` line, and the seed pair — carrying an `RK_DEVSHELL_PIN` token that `src/devshell/fragments.rs` renders from the pin grammar, so no payload file names the flake's owner.

`.in` files are templates carrying `RK_*` and `OWNER` tokens, substituted in `src/landing.rs`; the hooks fragment is not standalone YAML, and the token lines must survive formatting, which is what the suffix buys.

Every file ends with one enforced newline; the `.in` readers strip exactly one because the spliced form carries none, `post-merge-hook.sh` is written verbatim because a hook file ends in its newline, and byte-equality tests pin both round trips.
