#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MOOL="$ROOT/mool"
LOCKFILES=(
    "$MOOL/tests/fixtures/backendless-consumer/Cargo.lock"
    "$MOOL/tests/fixtures/backendless-query-rejected/Cargo.lock"
)

cleanup() {
    rm -f "${LOCKFILES[@]}"
}

trap cleanup EXIT

cargo check -p mool --no-default-features
cargo check -p mool --no-default-features --features migrations
cargo check --offline --manifest-path "$MOOL/tests/fixtures/backendless-consumer/Cargo.toml"

if cargo check --offline --manifest-path "$MOOL/tests/fixtures/backendless-query-rejected/Cargo.toml" >/dev/null 2>&1; then
    echo "expected Mool query APIs to be unavailable without a database backend" >&2
    exit 1
fi
