#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/mool/Cargo.toml" | head -n 1)"
ARCHIVE="$ROOT/target/package/mool-$VERSION.crate"
TEMP_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "$TEMP_DIR"
}

trap cleanup EXIT

cd "$ROOT"
bash scripts/release-dependency-check.sh
PACKAGE_ARGS=(--locked --offline)
if [[ -z "${CI:-}" ]]; then
    PACKAGE_ARGS+=(--allow-dirty)
fi
cargo package "${PACKAGE_ARGS[@]}" -p mool-macros-impl
cargo package "${PACKAGE_ARGS[@]}" -p mool-macros
cargo package "${PACKAGE_ARGS[@]}" -p mool --features sqlite
tar -xzf "$ARCHIVE" -C "$TEMP_DIR"

PACKAGE_DIR="$TEMP_DIR/mool-$VERSION"
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
EOF
    printf '%s\n' "$source" >"$consumer/src/main.rs"
    cargo generate-lockfile --offline --manifest-path "$consumer/Cargo.toml"
    cargo check --offline --locked --manifest-path "$consumer/Cargo.toml"
}

create_consumer \
    "mool-packaged-backendless" \
    '"migrations"' \
    'use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("migrations");

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
