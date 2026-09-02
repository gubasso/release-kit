# blocks

The whole texts the binary writes outside `snippets/`: the routing and hook blocks it splices into a target, and the post-merge hook it installs on a host.

`.in` files are templates carrying `RK_*` and `OWNER` tokens, substituted in `src/landing.rs`; the hooks fragment is not standalone YAML, and the token lines must survive formatting, which is what the suffix buys.

Every file ends with one enforced newline; the `.in` readers strip exactly one because the spliced form carries none, `post-merge-hook.sh` is written verbatim because a hook file ends in its newline, and byte-equality tests pin both round trips.
