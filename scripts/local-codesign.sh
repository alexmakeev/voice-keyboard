#!/bin/bash
# Re-sign a locally-built app bundle (wrapper + embedded voice-typer sidecar)
# with a Developer ID identity + entitlements, mirroring the "Re-sign app
# bundle with entitlements" step in .github/workflows/release.yml.
#
# Why: `cargo tauri build` alone only produces an ad-hoc-signed bundle. Ad-hoc
# signing gets a fresh TCC identity on every rebuild, so macOS treats each
# rebuilt binary as a "new" app — permission grants (Accessibility, Input
# Monitoring, Microphone) don't persist across rebuilds, which makes local
# permission testing unreliable. Re-signing with a real Developer ID identity
# keeps the code identity stable across rebuilds, so grants persist.
#
# Run this AFTER `cargo tauri build`, as a separate, explicit, opt-in step.
# It is NOT wired into the build automatically.
#
# Usage: scripts/local-codesign.sh [path-to-.app]
#   Defaults to: src-tauri/target/release/bundle/macos/Voice Keyboard.app

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

APP="${1:-${REPO_DIR}/src-tauri/target/release/bundle/macos/Voice Keyboard.app}"
ENTITLEMENTS="${REPO_DIR}/src-tauri/entitlements.plist"
SIDECAR="${APP}/Contents/MacOS/voice-typer"

if [[ ! -d "$APP" ]]; then
    echo "ERROR: App bundle not found at: ${APP}"
    echo "Run 'cargo tauri build' first, or pass the bundle path as an argument."
    exit 1
fi

if [[ ! -f "$ENTITLEMENTS" ]]; then
    echo "ERROR: Entitlements file not found at: ${ENTITLEMENTS}"
    exit 1
fi

if [[ ! -f "$SIDECAR" ]]; then
    echo "ERROR: Sidecar binary not found at: ${SIDECAR}"
    echo "Make sure voice-typer was built and copied into the bundle before running this script."
    exit 1
fi

# Find a Developer ID Application identity in the local keychain. Do NOT fall
# back to ad-hoc signing — that defeats the whole point of this script.
APPLE_SIGNING_IDENTITY="$(security find-identity -v -p codesigning \
    | grep '"Developer ID Application:' \
    | head -n1 \
    | sed -E 's/.*"(.*)"/\1/')"

if [[ -z "$APPLE_SIGNING_IDENTITY" ]]; then
    echo "ERROR: No 'Developer ID Application' identity found in the local keychain."
    echo "Run 'security find-identity -v -p codesigning' to inspect available identities."
    exit 1
fi

echo "Signing with identity: ${APPLE_SIGNING_IDENTITY}"
echo "App bundle: ${APP}"

codesign --force --sign "$APPLE_SIGNING_IDENTITY" --entitlements "${ENTITLEMENTS}" --options runtime "${SIDECAR}"
codesign --force --deep --sign "$APPLE_SIGNING_IDENTITY" --entitlements "${ENTITLEMENTS}" --options runtime "${APP}"
codesign --verify --deep --strict "${APP}"
codesign -d --entitlements - "${APP}"

echo ""
echo "Done. Bundle re-signed with a stable Developer ID identity."
