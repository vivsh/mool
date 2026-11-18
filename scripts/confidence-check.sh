#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export RUSTUP_TOOLCHAIN=1.94.0

cargo fmt --all --check
bash scripts/release-dependency-check.sh
cargo deny --locked --all-features check advisories

for backend in postgres sqlite mysql mariadb; do
    features="$backend time test-support text-search"
    if [[ "$backend" == postgres || "$backend" == sqlite ]]; then
        features="$features migrations fuzzy-search vector"
    fi
    if [[ "$backend" == mariadb ]]; then
        features="$features vector"
    fi
    if [[ "$backend" == postgres ]]; then
        features="$features tracing"
    fi
    cargo test --locked --workspace --no-default-features --features "$features"
    cargo clippy --locked -p mool --all-targets --no-deps \
        --no-default-features --features "$features" -- -D warnings
    cargo test --locked -p mool --no-default-features \
        --features "$features compile-contracts" --test trybuild
done

cargo test --locked -p mool --no-default-features \
    --features "postgres compile-contracts" --test trybuild
bash scripts/feature-contracts.sh
cargo check --locked -p mool --examples --no-default-features \
    --features "postgres migrations mock time"
cargo clippy --locked -p mool-macros-impl -p mool-macros \
    --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked -p mool --no-deps \
    --no-default-features --features "postgres migrations time text-search fuzzy-search vector"
