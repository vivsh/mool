#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo fmt --all --check
bash scripts/feature-contracts.sh
cargo test --locked --workspace --no-default-features --features mool/sqlite
cargo test --locked -p mool --no-default-features --features sqlite
cargo test --locked -p mool --no-default-features --features postgres
cargo test --locked -p mool --no-default-features --features mysql
cargo test --locked -p mool --no-default-features --features mariadb
cargo test --locked -p mool --no-default-features --features "sqlite time"
cargo test --locked -p mool --no-default-features --features "postgres time"
cargo test --locked -p mool --no-default-features --features "mysql time"
cargo test --locked -p mool --no-default-features --features "mariadb time"
cargo test --locked -p mool --no-default-features --features "sqlite migrations"
cargo test --locked -p mool --no-default-features --features "postgres migrations"
cargo check --locked -p mool --release --no-default-features --features sqlite
cargo check --locked -p mool --release --no-default-features --features "sqlite mock"
cargo check --locked -p mool --examples --no-default-features --features "postgres mock migrations time"
cargo clippy --locked -p mool --all-targets --no-deps --no-default-features --features "sqlite time migrations mock" -- -D warnings
cargo clippy --locked -p mool-macros-impl -p mool-macros --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked -p mool --no-deps --no-default-features --features "sqlite migrations time"
cargo package --locked -p mool-macros-impl
cargo package --locked -p mool-macros
cargo package --locked -p mool --features sqlite

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
