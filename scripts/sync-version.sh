#!/usr/bin/env bash
# Sync version across all project files from root Cargo.toml
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Read version from root Cargo.toml
VERSION=$(grep -m1 '^version' "$ROOT_DIR/Cargo.toml" | sed 's/.*"\(.*\)"/\1/')

if [ -z "$VERSION" ]; then
    echo "ERROR: Could not read version from Cargo.toml"
    exit 1
fi

echo "Syncing version: $VERSION"

# Update src-tauri/Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" "$ROOT_DIR/src-tauri/Cargo.toml"
rm -f "$ROOT_DIR/src-tauri/Cargo.toml.bak"

# Update src-tauri/tauri.conf.json
# Use python for reliable JSON editing, fallback to sed
if command -v python3 &>/dev/null; then
    python3 -c "
import json, sys
with open('$ROOT_DIR/src-tauri/tauri.conf.json', 'r') as f:
    conf = json.load(f)
conf['version'] = '$VERSION'
with open('$ROOT_DIR/src-tauri/tauri.conf.json', 'w') as f:
    json.dump(conf, f, indent=2)
    f.write('\n')
"
else
    sed -i.bak "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$ROOT_DIR/src-tauri/tauri.conf.json"
    rm -f "$ROOT_DIR/src-tauri/tauri.conf.json.bak"
fi

echo "Done. All files updated to version $VERSION"
