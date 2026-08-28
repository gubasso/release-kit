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
    pre-commit run --files $(rg --files --hidden -g '!.git/**')

test:
    cargo nextest run

# Land the rust files into a scratch repository, end to end, with the real
# binary.
build:
    set -eu; d=$(mktemp -d); trap 'rm -rf "$d"' EXIT; mkdir -p "$d/.git"; cargo run -q -- init --tech rust --target "$d" --apply >/dev/null; test -f "$d/release-plz.toml"

check: lint test build

# Install this checkout as the user's rk, plus the user-scope agent skills.
install:
    cargo install --path . --locked
    rk skill install --apply

# Remove the user-scope skills, then the binary; the binary owns the file
# list, so the skills go first, while it still exists.
uninstall:
    rk skill uninstall --apply
    cargo uninstall release-kit
