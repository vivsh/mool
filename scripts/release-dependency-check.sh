#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if grep -Eq '^gaman = \{.*path[[:space:]]*=' Cargo.toml; then
    echo "release builds must resolve Gaman from crates.io, not a sibling path" >&2
    exit 1
fi

if ! awk '
    $0 == "name = \"gaman\"" { in_gaman = 1; next }
    in_gaman && /^source = "registry\+/ { found_registry = 1; exit }
    in_gaman && /^\[\[package\]\]/ { exit }
    END { exit !(in_gaman && found_registry) }
' Cargo.lock; then
    echo "Cargo.lock must pin Gaman to the crates.io registry" >&2
    exit 1
fi

cargo metadata --locked --format-version 1 --no-deps >/dev/null
