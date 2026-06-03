#!/usr/bin/env bash

# Asserts that the solana-program version in `cli/solana-program-version`
# matches the version declared in the workspace `Cargo.toml`.
#
# This prevents the hardcoded version used for the duplicate-dependency check
# from drifting out of sync when the workspace dependency is bumped.

set -euo pipefail

CARGO_TOML="Cargo.toml"
VERSION_FILE="cli/solana-program-version"

if [[ ! -f "$VERSION_FILE" ]]; then
    echo "[!] Version file '$VERSION_FILE' not found" >&2
    exit 1
fi

cargo_version=$(grep '^solana-program = ' "$CARGO_TOML" | sed 's/^solana-program = "\([^"]*\)".*/\1/')
file_version=$(cat "$VERSION_FILE")

if [[ "$cargo_version" != "$file_version" ]]; then
    echo "[!] Version mismatch:" >&2
    echo "    $CARGO_TOML:          solana-program = \"$cargo_version\"" >&2
    echo "    $VERSION_FILE: \"$file_version\"" >&2
    echo "    Please update $VERSION_FILE to match $CARGO_TOML" >&2
    exit 1
fi

echo "[+] solana-program version matches ($file_version)"
