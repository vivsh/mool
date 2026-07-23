#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo test --workspace --no-default-features --features mool/sqlite
cargo test -p mool --no-default-features --features sqlite
cargo test -p mool --no-default-features --features postgres
cargo test -p mool --no-default-features --features mysql
cargo test -p mool --no-default-features --features mariadb
cargo test -p mool --no-default-features --features "sqlite migrations"
cargo test -p mool --no-default-features --features "postgres migrations"
cargo check -p mool --release --no-default-features --features sqlite
cargo check -p mool --release --no-default-features --features "sqlite mock"
cargo check -p mool --examples --no-default-features --features "sqlite mock migrations"
cargo clippy -p mool --no-deps --no-default-features --features sqlite -- -D warnings
cargo package -p mool-macros
cargo package -p mool --features sqlite

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/src"
cat >"$tmp/Cargo.toml" <<EOF
[package]
name = "mool_mock_gate_check"
version = "0.0.0"
edition = "2024"

[dependencies]
mool = { path = "$ROOT/mool", default-features = false, features = ["sqlite"] }
EOF
cat >"$tmp/src/main.rs" <<'EOF'
fn main() {
    let _ = mool::mock::MockDbSession::new();
}
EOF

if cargo check --manifest-path "$tmp/Cargo.toml" --release >/dev/null 2>&1; then
    echo "expected mool::mock to be unavailable in release without the mock feature" >&2
    exit 1
fi

cat >"$tmp/Cargo.toml" <<EOF
[package]
name = "mool_mock_gate_check"
version = "0.0.0"
edition = "2024"

[dependencies]
mool = { path = "$ROOT/mool", default-features = false, features = ["sqlite", "mock"] }
EOF

cargo check --manifest-path "$tmp/Cargo.toml" --release
