#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GAMAN_DIR="${MOOL_GAMAN_DIR:-$ROOT/../gaman}"
CONFIG_DIR="$ROOT/.cargo"
CONFIG_FILE="$CONFIG_DIR/config.toml"
LOCK_FILE="$ROOT/Cargo.lock"
LOCK_BACKUP=""
CREATED_CONFIG_DIR=false

if [[ $# -ne 1 ]]; then
    echo "usage: $0 '<mool feature list>'" >&2
    exit 1
fi

if [[ ! -f "$GAMAN_DIR/Cargo.toml" ]]; then
    echo "local Gaman checkout not found at $GAMAN_DIR" >&2
    exit 1
fi

if [[ -e "$CONFIG_FILE" ]]; then
    echo "refusing to replace existing Cargo configuration at $CONFIG_FILE" >&2
    exit 1
fi

if [[ ! -d "$CONFIG_DIR" ]]; then
    mkdir "$CONFIG_DIR"
    CREATED_CONFIG_DIR=true
fi

cleanup() {
    if [[ -n "$LOCK_BACKUP" ]]; then
        cp "$LOCK_BACKUP" "$LOCK_FILE"
        rm -f "$LOCK_BACKUP"
    fi
    rm -f "$CONFIG_FILE"
    if [[ "$CREATED_CONFIG_DIR" == true ]]; then
        rmdir "$CONFIG_DIR"
    fi
}

LOCK_BACKUP="$(mktemp)"
cp "$LOCK_FILE" "$LOCK_BACKUP"
trap cleanup EXIT

cat >"$CONFIG_FILE" <<EOF
[patch.crates-io]
gaman = { path = "$GAMAN_DIR" }
EOF

cargo test -p mool --no-default-features --features "$1" --test trybuild
