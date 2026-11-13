default:
    @just --list

fmt:
    cargo fmt
    dprint fmt

# The scratch round trip, end to end with the real binary: land, tune the
# seeded file, upgrade, and assert the tune survived with the record moved
# — the same again for the nix opt-in — then assert the published crate
# carries every distribution root and the landed seed actually builds.
build:
    set -eu; d=$(mktemp -d); trap 'rm -rf "$d"' EXIT; mkdir -p "$d/.git"; \
    cargo run -q -- init --technology rust --forge github --repo acme/widget --target "$d" --apply >/dev/null; \
    test -f "$d/release-plz.toml"; test -f "$d/.release-kit/manifest.json"; test -f "$d/.release-kit/config.toml"; \
    sed -i -e '/TODO(release-kit)/d' -e '/^semver_check = /d' "$d/release-plz.toml"; printf 'semver_check = true\n' >> "$d/release-plz.toml"; \
    cargo run -q -- upgrade --target "$d" --apply >/dev/null; \
    grep -q 'semver_check = true' "$d/release-plz.toml"; \
    { printf 'name: ci\non:\n  pull_request:\njobs:\n'; \
      awk '/^```yaml$/ { getline line; if (line == "dist-plan:") { on = 1; print "  " line; next } } on && /^```$/ { on = 0 } on { print "  " $0 }' bindings/rust.md; \
      printf '  gate:\n    needs: [dist-plan, dist-build]\n    runs-on: ubuntu-latest\n    steps:\n      - run: "true"\n'; } > "$d/.github/workflows/ci.yml"; \
    sed -i 's/^required_check = "[^"]*"/required_check = "gate"/' "$d/.release-kit/config.toml"; \
    status="$(cargo run -q -- status --check --json --target "$d" || true)"; \
    printf '%s' "$status" | jq -e '.violations == ["setup proof: setup-proof-absent"]' >/dev/null
    set -eu; n=$(mktemp -d); trap 'rm -rf "$n"' EXIT; mkdir -p "$n/.git" "$n/src"; \
    printf '[package]\nname = "widget"\nversion = "0.1.0"\n' > "$n/Cargo.toml"; \
    printf 'fn main() {}\n' > "$n/src/main.rs"; printf 'version = 4\n' > "$n/Cargo.lock"; \
    cargo run -q -- init --technology rust --forge github --repo acme/widget --nix-packaging --target "$n" --apply >/dev/null; \
    test -f "$n/nix/package.nix"; test -f "$n/flake.nix"; test -f "$n/flake.lock"; test ! -e "$n/.github/workflows/nix.yml"; \
    printf '# tuned by the target\n' >> "$n/nix/package.nix"; \
    cargo run -q -- upgrade --target "$n" --apply >/dev/null; \
    grep -q '# tuned by the target' "$n/nix/package.nix"
    cargo nextest run --run-ignored ignored-only -E 'test(the_published_crate_carries_every_root) or test(the_published_crate_carries_the_two_new_roots) or test(the_landed_nix_capability_builds_end_to_end) or test(a_release_changing_a_destination_without_guidance_is_named)'

# The handoff sweep: the manual stage across all files, which CI runs too.
# Every check this project classifies `both` is a hook at its declared
# stage, so the desk and continuous integration reach the same set by
# invoking the same stage rather than by anyone remembering to add a check
# twice. rk integrate runs the pre-merge-commit hook on the staged squash
# instead, and every check on the manual stage carries pre-merge-commit
# too, so this sweep is a superset of the integration gate.
#
# The hooks that manage this repository, rk message and rk status, run the
# rk release that the devshell pins. The hooks that test the tree build it
# themselves: cargo test, and just build through cargo run. Loading the
# configuration for the manual run is what refuses an invalid hook
# configuration, so no separate validate step remains.
check:
    prek run --hook-stage manual --all-files

# Remove the skill copies an earlier release installed under the home. Every
# landing now carries the skills, so the home keeps none.
uninstall:
    rk skill uninstall --apply
