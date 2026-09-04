default:
    @just --list

fmt:
    cargo fmt
    dprint fmt

lint:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo deny check
    dprint check
    editorconfig-checker -disable-insert-final-newline
    typos
    markdownlint-cli2 "**/*.md" "#target/**"
    pre-commit validate-config .pre-commit-config.yaml
    cargo build -q
    PATH="$(pwd)/target/debug:$PATH" pre-commit run --files $(rg --files --hidden -g '!.git')

test:
    cargo nextest run

# The scratch round trip, end to end with the real binary: land, tune the
# seeded file, upgrade, and assert the tune survived with the record moved
# — the same again for the nix opt-in — then assert the published crate
# carries every payload root and the landed seed actually builds.
build:
    set -eu; d=$(mktemp -d); trap 'rm -rf "$d"' EXIT; mkdir -p "$d/.git"; \
    cargo run -q -- init --tech rust --forge github --repo acme/widget --scopes api,cli --target "$d" --apply >/dev/null; \
    test -f "$d/release-plz.toml"; test -f "$d/.release-kit/manifest.json"; \
    sed -i '/TODO(release-kit)/d' "$d/release-plz.toml"; printf 'semver_check = true\n' >> "$d/release-plz.toml"; \
    cargo run -q -- upgrade --target "$d" --apply >/dev/null; \
    grep -q 'semver_check = true' "$d/release-plz.toml"; \
    cargo run -q -- status --check --target "$d" >/dev/null
    set -eu; n=$(mktemp -d); trap 'rm -rf "$n"' EXIT; mkdir -p "$n/.git"; \
    printf '[package]\nname = "widget"\nversion = "0.1.0"\n' > "$n/Cargo.toml"; \
    cargo run -q -- init --tech rust --forge github --repo acme/widget --scopes api,cli --nix --target "$n" --apply >/dev/null; \
    test -f "$n/nix/package.nix"; test -f "$n/flake.nix"; test -f "$n/flake.lock"; test -f "$n/.github/workflows/nix.yml"; \
    printf '# tuned by the target\n' >> "$n/nix/package.nix"; \
    cargo run -q -- upgrade --target "$n" --apply >/dev/null; \
    grep -q '# tuned by the target' "$n/nix/package.nix"
    cargo nextest run --run-ignored ignored-only -E 'test(the_published_crate_carries_every_root) or test(the_landed_nix_capability_builds_end_to_end)'

check: lint test build

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
