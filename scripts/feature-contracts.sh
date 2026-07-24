#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MOOL="$ROOT/mool"
LOCKFILES=(
    "$MOOL/tests/fixtures/backendless-consumer/Cargo.lock"
    "$MOOL/tests/fixtures/backendless-query-rejected/Cargo.lock"
    "$MOOL/tests/fixtures/testing-consumer/Cargo.lock"
    "$MOOL/tests/fixtures/testing-rejected/Cargo.lock"
    "$MOOL/tests/fixtures/testing-backendless-rejected/Cargo.lock"
    "$MOOL/tests/fixtures/testing-mysql-migrations-rejected/Cargo.lock"
)

cleanup() {
    rm -f "${LOCKFILES[@]}"
}

trap cleanup EXIT

cargo check -p mool --no-default-features
cargo check -p mool --no-default-features --features migrations
cargo check -p mool --no-default-features --features "sqlite test-support"
cargo check -p mool --no-default-features --features "postgres migrations test-support"
cargo check --offline --manifest-path "$MOOL/tests/fixtures/backendless-consumer/Cargo.toml"
cargo check --offline --manifest-path "$MOOL/tests/fixtures/testing-consumer/Cargo.toml"

if cargo check --offline --manifest-path "$MOOL/tests/fixtures/backendless-query-rejected/Cargo.toml" >/dev/null 2>&1; then
    echo "expected Mool query APIs to be unavailable without a database backend" >&2
    exit 1
fi

for fixture in testing-rejected testing-backendless-rejected testing-mysql-migrations-rejected; do
    if cargo check --offline --manifest-path "$MOOL/tests/fixtures/$fixture/Cargo.toml" >/dev/null 2>&1; then
        echo "expected $fixture to reject an unavailable Mool testing API" >&2
        exit 1
    fi
done
