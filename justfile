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
    sed -i '/TODO(release-kit)/d' "$d/release-plz.toml"; printf 'semver_check = true\n' >> "$d/release-plz.toml"; \
    cargo run -q -- upgrade --target "$d" --apply >/dev/null; \
    grep -q 'semver_check = true' "$d/release-plz.toml"; \
    cargo run -q -- status --check --target "$d" >/dev/null
    set -eu; n=$(mktemp -d); trap 'rm -rf "$n"' EXIT; mkdir -p "$n/.git" "$n/src"; \
    printf '[package]\nname = "widget"\nversion = "0.1.0"\n' > "$n/Cargo.toml"; \
    printf 'fn main() {}\n' > "$n/src/main.rs"; printf 'version = 4\n' > "$n/Cargo.lock"; \
    cargo run -q -- init --technology rust --forge github --repo acme/widget --nix-packaging --target "$n" --apply >/dev/null; \
    test -f "$n/nix/package.nix"; test -f "$n/flake.nix"; test -f "$n/flake.lock"; test ! -e "$n/.github/workflows/nix.yml"; \
    printf '# tuned by the target\n' >> "$n/nix/package.nix"; \
    cargo run -q -- upgrade --target "$n" --apply >/dev/null; \
    grep -q '# tuned by the target' "$n/nix/package.nix"
    cargo nextest run --run-ignored ignored-only -E 'test(the_published_crate_carries_every_root) or test(the_published_crate_carries_the_two_new_roots) or test(the_landed_nix_capability_builds_end_to_end) or test(a_release_changing_a_destination_without_guidance_is_named)'

# The pre-integrate gate, which is the one line `rk integrate` runs. Every
# check this project classifies `both` is a hook at its declared stage, so
# the desk and continuous integration reach the same set by invoking the
# same stage rather than by anyone remembering to add a check twice.
#
# The build comes first because several hooks call this checkout's own rk,
# which they find on PATH. Loading the configuration for the manual run is
# what refuses an invalid pre-commit configuration, so no separate
# validate step remains.
check:
    cargo build -q
    PATH="$(pwd)/target/debug:$PATH" pre-commit run --hook-stage manual --all-files

# Install this checkout as the user's rk, plus the user-scope agent skills.
# The installed skills are this checkout's build artifact and never an edit
# source, so the apply overwrites whatever an older release left in the home
# roots. The binary is called by the path cargo just wrote it to, since the
# first install of all runs before any shell has it on PATH.
install:
    cargo install --path . --locked
    "${CARGO_HOME:-$HOME/.cargo}/bin/rk" skill install --apply --force

# Remove the user-scope skills, then the binary; the binary owns the file
# list, so the skills go first, while it still exists.
uninstall:
    rk skill uninstall --apply
    cargo uninstall release-kit
