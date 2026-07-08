#!/bin/bash
#
# uninstall-macos.sh — Complete uninstall of Voice Keyboard on macOS.
#
# Removes: the installed .app bundle, LaunchAgent (autostart), all TCC
# (privacy) permissions granted to this app's bundle ID, caches, logs,
# saved application state, WebKit storage, and Preferences plist.
#
# PRESERVES: the user's configuration directory
#   ~/Library/Application Support/voice-keyboard/
# which contains config.json (settings), downloaded Whisper models, and
# saved audio/transcription history. This script NEVER touches that
# directory — see the "PRESERVED" section below.
#
# This script is idempotent: safe to re-run. Missing items are skipped
# (not treated as errors); only genuine failures abort the script.
#
# Usage: ./scripts/uninstall-macos.sh
#
set -euo pipefail

# --- Facts confirmed by inspecting the repo/installed app (2026-07-05) ---
BUNDLE_ID="com.alexmak.voice-keyboard"
APP_NAME="Voice Keyboard"
APP_PATH="/Applications/${APP_NAME}.app"
LAUNCH_AGENT_LABEL="com.alexmak.voice-keyboard"
LAUNCH_AGENT_PLIST="$HOME/Library/LaunchAgents/${LAUNCH_AGENT_LABEL}.plist"

# Directory that MUST be preserved (settings, models, audio history).
# Confirmed via src/config.rs: Config::config_path() uses
# directories::BaseDirs::config_dir() ("~/Library/Application Support" on
# macOS) + "voice-keyboard" + "config.json". Config::data_dir()/models_dir()
# resolve to the SAME parent directory, so models/ and audio/ live next to
# config.json — this whole directory is treated as user data and skipped.
PRESERVED_CONFIG_DIR="$HOME/Library/Application Support/voice-keyboard"

echo "=================================================="
echo " Voice Keyboard — full uninstall (macOS)"
echo "=================================================="
echo "Bundle ID:   ${BUNDLE_ID}"
echo "App path:    ${APP_PATH}"
echo "Preserving:  ${PRESERVED_CONFIG_DIR}  (NOT touched by this script)"
echo "--------------------------------------------------"

# --- 1. Stop any running instances ---------------------------------------
echo
echo "[1/6] Stopping running processes..."
pkill -f "voice-keyboard-app" 2>/dev/null || true
pkill -f "voice-typer" 2>/dev/null || true
pkill -f "voice-typer-launcher" 2>/dev/null || true
sleep 1
echo "  Done (any matching processes were signaled; already-stopped is fine)."

# --- 2. Unload and remove the LaunchAgent (autostart) --------------------
echo
echo "[2/6] Removing LaunchAgent (autostart)..."
if launchctl list "${LAUNCH_AGENT_LABEL}" >/dev/null 2>&1; then
    launchctl bootout "gui/$(id -u)/${LAUNCH_AGENT_LABEL}" 2>/dev/null \
        || launchctl unload "${LAUNCH_AGENT_PLIST}" 2>/dev/null \
        || true
    echo "  Unloaded ${LAUNCH_AGENT_LABEL}"
else
    echo "  Not currently loaded, skipping unload"
fi
if [[ -f "${LAUNCH_AGENT_PLIST}" ]]; then
    rm -f "${LAUNCH_AGENT_PLIST}"
    echo "  Removed ${LAUNCH_AGENT_PLIST}"
else
    echo "  No LaunchAgent plist found, skipping"
fi

# --- 3. Reset TCC (privacy) permissions, scoped to this bundle ID only ---
# IMPORTANT: always pass the bundle ID — a bare `tccutil reset <Service>`
# with no bundle ID wipes that permission for EVERY app on the system.
#
# Services reset (all confirmed as required by this app's design):
#   Microphone    - cpal audio recording (NSMicrophoneUsageDescription,
#                   com.apple.security.device.audio-input entitlement)
#   Accessibility - enigo keyboard injection + rdev unstable_grab event tap
#   ListenEvent   - "Input Monitoring": rdev global hotkey listener on macOS
#   AppleEvents   - "Automation": no longer requested by current builds
#                   (Info.plist no longer declares
#                   NSAppleEventsUsageDescription — the app never used Apple
#                   Events). Kept here as a harmless no-op cleanup for
#                   permissions granted under old builds that did declare it.
echo
echo "[3/6] Resetting TCC (privacy) permissions for ${BUNDLE_ID}..."
for service in Microphone Accessibility ListenEvent AppleEvents; do
    if tccutil reset "${service}" "${BUNDLE_ID}" 2>/dev/null; then
        echo "  - ${service}: reset"
    else
        echo "  - ${service}: skipped (not granted or already reset)"
    fi
done

# --- 4. Remove the installed app bundle -----------------------------------
echo
echo "[4/6] Removing installed app bundle..."
if [[ -d "${APP_PATH}" ]]; then
    rm -rf "${APP_PATH}"
    echo "  Removed ${APP_PATH}"
else
    echo "  Not found at ${APP_PATH}, skipping"
fi

# --- 5. Remove caches, logs, saved state, prefs, WebKit storage ----------
# These are all ephemeral/derived data, distinct from the preserved config
# directory above. Removing them does not affect user settings.
echo
echo "[5/6] Removing caches, logs, saved state, and preferences..."

CACHE_DIR="$HOME/Library/Caches/${BUNDLE_ID}"
[[ -d "${CACHE_DIR}" ]] && rm -rf "${CACHE_DIR}" && echo "  Removed ${CACHE_DIR}" \
    || echo "  No cache dir at ${CACHE_DIR}, skipping"

WEBKIT_DIR="$HOME/Library/WebKit/${BUNDLE_ID}"
[[ -d "${WEBKIT_DIR}" ]] && rm -rf "${WEBKIT_DIR}" && echo "  Removed ${WEBKIT_DIR}" \
    || echo "  No WebKit dir at ${WEBKIT_DIR}, skipping"

SAVED_STATE_DIR="$HOME/Library/Saved Application State/${BUNDLE_ID}.savedState"
[[ -d "${SAVED_STATE_DIR}" ]] && rm -rf "${SAVED_STATE_DIR}" && echo "  Removed ${SAVED_STATE_DIR}" \
    || echo "  No saved state dir, skipping"

PREFS_PLIST="$HOME/Library/Preferences/${BUNDLE_ID}.plist"
[[ -f "${PREFS_PLIST}" ]] && rm -f "${PREFS_PLIST}" && echo "  Removed ${PREFS_PLIST}" \
    || echo "  No preferences plist, skipping"
defaults delete "${BUNDLE_ID}" >/dev/null 2>&1 && echo "  Removed defaults domain ${BUNDLE_ID}" \
    || echo "  No defaults domain registered, skipping"

LOGS_DIR="$HOME/Library/Logs/${APP_NAME}"
[[ -d "${LOGS_DIR}" ]] && rm -rf "${LOGS_DIR}" && echo "  Removed ${LOGS_DIR}" \
    || echo "  No Logs dir at ${LOGS_DIR}, skipping"

# Leftover temp files created by the LaunchAgent wrapper (found during
# investigation: /tmp/voice-keyboard-wrapper.sh + its stdout/stderr logs).
for f in /tmp/voice-keyboard-wrapper.sh /tmp/voice-keyboard.out.log /tmp/voice-keyboard.err.log; do
    [[ -e "${f}" ]] && rm -f "${f}" && echo "  Removed ${f}" || true
done

# --- 6. Explicitly do NOT touch the configuration directory --------------
echo
echo "[6/6] Skipping configuration directory (preserved by design)..."
if [[ -d "${PRESERVED_CONFIG_DIR}" ]]; then
    echo "  PRESERVED: ${PRESERVED_CONFIG_DIR}"
    echo "  (contains config.json, downloaded Whisper models, audio history — not deleted)"
else
    echo "  Not present (nothing to preserve)."
fi

echo
echo "=================================================="
echo " Summary"
echo "=================================================="
echo "Removed:"
echo "  - Running processes (voice-keyboard-app, voice-typer, voice-typer-launcher)"
echo "  - LaunchAgent: ${LAUNCH_AGENT_PLIST}"
echo "  - TCC permissions for ${BUNDLE_ID}: Microphone, Accessibility, ListenEvent, AppleEvents"
echo "  - App bundle: ${APP_PATH}"
echo "  - Caches, WebKit storage, Saved Application State, Preferences plist, Logs, /tmp wrapper files"
echo
echo "Preserved (NOT touched):"
echo "  - ${PRESERVED_CONFIG_DIR}"
echo "    (config.json settings, Whisper models, audio/transcription history)"
echo
echo "Note: if the app was ever run directly (outside the .app bundle, e.g."
echo "via 'cargo run' or the raw binary), macOS may have also registered TCC"
echo "grants under the executable path rather than the bundle ID. Those are"
echo "NOT touched by this script (tccutil only resets by bundle ID/service,"
echo "which is the sanctioned mechanism) — check System Settings > Privacy"
echo "& Security manually if you suspect leftover path-based grants."
echo "=================================================="
