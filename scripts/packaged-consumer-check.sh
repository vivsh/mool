#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/mool/Cargo.toml" | head -n 1)"
MACROS_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/mool-macros/Cargo.toml" | head -n 1)"
MACROS_IMPL_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/mool-macros-impl/Cargo.toml" | head -n 1)"
ARCHIVE="$ROOT/target/package/mool-$VERSION.crate"
MACROS_ARCHIVE="$ROOT/target/package/mool-macros-$MACROS_VERSION.crate"
MACROS_IMPL_ARCHIVE="$ROOT/target/package/mool-macros-impl-$MACROS_IMPL_VERSION.crate"
TEMP_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "$TEMP_DIR"
}

trap cleanup EXIT

cd "$ROOT"
bash scripts/release-dependency-check.sh
PACKAGE_ARGS=(--locked --no-verify)
if [[ -z "${CI:-}" ]]; then
    PACKAGE_ARGS+=(--allow-dirty)
fi
cargo package "${PACKAGE_ARGS[@]}" -p mool-macros-impl
cargo package "${PACKAGE_ARGS[@]}" -p mool-macros
cargo package "${PACKAGE_ARGS[@]}" -p mool --features sqlite
tar -xzf "$ARCHIVE" -C "$TEMP_DIR"
tar -xzf "$MACROS_ARCHIVE" -C "$TEMP_DIR"
tar -xzf "$MACROS_IMPL_ARCHIVE" -C "$TEMP_DIR"

PACKAGE_DIR="$TEMP_DIR/mool-$VERSION"
MACROS_DIR="$TEMP_DIR/mool-macros-$MACROS_VERSION"
MACROS_IMPL_DIR="$TEMP_DIR/mool-macros-impl-$MACROS_IMPL_VERSION"
create_consumer() {
    local name="$1"
    local features="$2"
    local source="$3"
    local consumer="$TEMP_DIR/$name"

    mkdir -p "$consumer/src" "$consumer/migrations"
    cat >"$consumer/Cargo.toml" <<EOF
[package]
name = "$name"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
mool = { path = "$PACKAGE_DIR", default-features = false, features = [$features] }

[patch.crates-io]
mool-macros = { path = "$MACROS_DIR" }
mool-macros-impl = { path = "$MACROS_IMPL_DIR" }
EOF
    printf '%s\n' "$source" >"$consumer/src/main.rs"
    cargo generate-lockfile --manifest-path "$consumer/Cargo.toml"
    cargo check --locked --manifest-path "$consumer/Cargo.toml"
}

create_consumer \
    "mool-packaged-backendless" \
    '"migrations"' \
    'use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embed_migrations!("migrations");

fn main() {
    let _pool = db::DbPool::from_pool(db::backend::Pool);
    let _config = db::DbConf::default();
    let _storage = db::schema::GeneratedStorage::Stored;
    let _migrations = MIGRATIONS;
}'

create_consumer \
    "mool-packaged-sqlite" \
    '"sqlite", "migrations"' \
    'use mool as db;
use mool::prelude::*;

#[derive(db::Model)]
#[table(name = "widgets")]
struct Widget {
    #[column(primary_key)]
    id: i64,
    name: String,
}

fn main() -> Result<(), db::QueryError> {
    let widgets = Widget::table();
    let plan = db::from(&widgets)
        .filter(widgets.id.eq(db::val(1_i64)))
        .all::<Widget>()
        .plan()?;
    assert!(plan.sql.contains("widgets"));
    Ok(())
}'
