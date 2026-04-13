//! Tests for Cargo.toml dependency version declarations.
//!
//! These tests guard against accidental version regressions for the packages
//! that were downgraded in this PR:
//!
//!   - `cpal`:       0.16 → 0.15
//!   - `enigo`:      0.6  → 0.2
//!   - `tray-icon`:  0.21 → 0.19  (macOS, Windows, and Linux targets)
//!
//! Run with: cargo test --test cargo_manifest_test

use std::fs;
use std::path::{Path, PathBuf};

/// Locate Cargo.toml relative to the workspace root.
///
/// When `cargo test` runs, the working directory is the package root, so
/// Cargo.toml is at `./Cargo.toml`.  The `CARGO_MANIFEST_DIR` environment
/// variable (set by Cargo at compile time) is the most reliable anchor.
fn manifest_path() -> PathBuf {
    // CARGO_MANIFEST_DIR is set at *compile* time to the directory containing
    // the crate's Cargo.toml, which is exactly what we want to read.
    let dir = env!("CARGO_MANIFEST_DIR");
    Path::new(dir).join("Cargo.toml")
}

/// Load the manifest content once and return it as a `String`.
fn read_manifest() -> String {
    let path = manifest_path();
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

// ---------------------------------------------------------------------------
// cpal – audio recording back-end
// ---------------------------------------------------------------------------

/// `cpal` must declare version 0.15 (downgraded from 0.16).
#[test]
fn cpal_version_is_0_15() {
    let manifest = read_manifest();
    assert!(
        manifest.contains(r#"cpal = "0.15""#),
        "Expected cpal = \"0.15\" in Cargo.toml, but it was not found.\n\
         Check that the cpal dependency was not accidentally reverted to 0.16."
    );
}

/// Ensure the old cpal 0.16 version string is NOT present anywhere.
#[test]
fn cpal_old_version_0_16_is_absent() {
    let manifest = read_manifest();
    assert!(
        !manifest.contains(r#"cpal = "0.16""#),
        "Found cpal = \"0.16\" in Cargo.toml — this version was intentionally \
         downgraded to 0.15 and should not be present."
    );
}

// ---------------------------------------------------------------------------
// enigo – keyboard simulation
// ---------------------------------------------------------------------------

/// `enigo` must declare version 0.2 (downgraded from 0.6).
#[test]
fn enigo_version_is_0_2() {
    let manifest = read_manifest();
    assert!(
        manifest.contains(r#"enigo = "0.2""#),
        "Expected enigo = \"0.2\" in Cargo.toml, but it was not found.\n\
         Check that the enigo dependency was not accidentally reverted to 0.6."
    );
}

/// Ensure the old enigo 0.6 version string is NOT present anywhere.
#[test]
fn enigo_old_version_0_6_is_absent() {
    let manifest = read_manifest();
    assert!(
        !manifest.contains(r#"enigo = "0.6""#),
        "Found enigo = \"0.6\" in Cargo.toml — this version was intentionally \
         downgraded to 0.2 and should not be present."
    );
}

// ---------------------------------------------------------------------------
// tray-icon – system tray (all three platform targets)
// ---------------------------------------------------------------------------

/// `tray-icon` must declare version 0.19 everywhere it appears (downgraded
/// from 0.21 for macOS, Windows, and Linux targets).
#[test]
fn tray_icon_version_is_0_19_everywhere() {
    let manifest = read_manifest();
    let expected = r#"tray-icon = { version = "0.19", optional = true }"#;
    let count = manifest.matches(expected).count();
    assert_eq!(
        count, 3,
        "Expected tray-icon = {{ version = \"0.19\", optional = true }} to appear \
         exactly 3 times in Cargo.toml (macOS, Windows, Linux), but found {} occurrence(s).",
        count
    );
}

/// Ensure no remnant of the old tray-icon 0.21 version string remains.
#[test]
fn tray_icon_old_version_0_21_is_absent() {
    let manifest = read_manifest();
    assert!(
        !manifest.contains(r#"tray-icon = { version = "0.21""#),
        "Found tray-icon with version 0.21 in Cargo.toml — this version was \
         intentionally downgraded to 0.19 and should not be present."
    );
}

/// tray-icon for macOS target specifically uses version 0.19.
#[test]
fn tray_icon_macos_target_version_is_0_19() {
    let manifest = read_manifest();
    // Find the macOS target section and confirm tray-icon 0.19 follows it.
    let macos_section = manifest
        .find(r#"cfg(target_os = "macos")"#)
        .expect("macOS target section not found in Cargo.toml");
    let after_macos = &manifest[macos_section..];
    // The next target section starts with cfg(target_os = "windows"), so
    // everything between the macOS header and Windows header is macOS-only.
    let macos_block_end = after_macos
        .find(r#"cfg(target_os = "windows")"#)
        .unwrap_or(after_macos.len());
    let macos_block = &after_macos[..macos_block_end];
    assert!(
        macos_block.contains(r#"tray-icon = { version = "0.19", optional = true }"#),
        "tray-icon 0.19 not found in the macOS target block of Cargo.toml."
    );
}

/// tray-icon for Windows target specifically uses version 0.19.
#[test]
fn tray_icon_windows_target_version_is_0_19() {
    let manifest = read_manifest();
    let windows_section = manifest
        .find(r#"cfg(target_os = "windows")"#)
        .expect("Windows target section not found in Cargo.toml");
    let after_windows = &manifest[windows_section..];
    let windows_block_end = after_windows
        .find(r#"cfg(target_os = "linux")"#)
        .unwrap_or(after_windows.len());
    let windows_block = &after_windows[..windows_block_end];
    assert!(
        windows_block.contains(r#"tray-icon = { version = "0.19", optional = true }"#),
        "tray-icon 0.19 not found in the Windows target block of Cargo.toml."
    );
}

/// tray-icon for Linux target specifically uses version 0.19.
#[test]
fn tray_icon_linux_target_version_is_0_19() {
    let manifest = read_manifest();
    let linux_section = manifest
        .find(r#"cfg(target_os = "linux")"#)
        .expect("Linux target section not found in Cargo.toml");
    let after_linux = &manifest[linux_section..];
    // Everything from the Linux header to end-of-file (or next header) belongs to Linux.
    let linux_block_end = after_linux
        .find("\n[")
        .unwrap_or(after_linux.len());
    let linux_block = &after_linux[..linux_block_end];
    assert!(
        linux_block.contains(r#"tray-icon = { version = "0.19", optional = true }"#),
        "tray-icon 0.19 not found in the Linux target block of Cargo.toml."
    );
}

// ---------------------------------------------------------------------------
// Regression: companion package `muda` must remain unchanged at 0.15
// ---------------------------------------------------------------------------

/// `muda` is paired with tray-icon but was NOT changed; it must still be 0.15.
#[test]
fn muda_version_unchanged_at_0_15() {
    let manifest = read_manifest();
    let count = manifest
        .matches(r#"muda = { version = "0.15", optional = true }"#)
        .count();
    assert_eq!(
        count, 3,
        "Expected muda = {{ version = \"0.15\", optional = true }} to appear \
         exactly 3 times (macOS, Windows, Linux), but found {} occurrence(s). \
         muda was not part of this PR's changes and should be unchanged.",
        count
    );
}

// ---------------------------------------------------------------------------
// Consistency: all three tray-icon entries must use the identical version
// ---------------------------------------------------------------------------

/// Extract every `tray-icon` version specifier from the manifest and assert
/// they are all the same string.  This prevents the three platform blocks
/// from diverging in the future.
#[test]
fn tray_icon_version_consistent_across_all_targets() {
    let manifest = read_manifest();
    let versions: Vec<&str> = manifest
        .lines()
        .filter(|line| line.contains("tray-icon") && line.contains("version"))
        .collect();

    assert_eq!(
        versions.len(),
        3,
        "Expected exactly 3 tray-icon version lines, found {}: {:?}",
        versions.len(),
        versions
    );

    // All three lines should be identical (same version, same optional flag).
    let first = versions[0];
    for line in &versions[1..] {
        assert_eq!(
            first.trim(),
            line.trim(),
            "tray-icon version declarations are inconsistent across platform targets:\n  \
             '{}'\n  '{}'",
            first.trim(),
            line.trim()
        );
    }
}

// ---------------------------------------------------------------------------
// Boundary / negative: version strings must be specific semver, not wildcards
// ---------------------------------------------------------------------------

/// cpal must use an exact version specifier, not a range or wildcard.
#[test]
fn cpal_version_is_not_wildcard() {
    let manifest = read_manifest();
    // Extract the cpal line from [dependencies]
    let cpal_line = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("cpal"))
        .expect("cpal dependency line not found");
    assert!(
        !cpal_line.contains('*'),
        "cpal version must not use a wildcard '*': {cpal_line}"
    );
    assert!(
        !cpal_line.contains('^'),
        "cpal should use a plain version string, not a caret '^' range: {cpal_line}"
    );
}

/// enigo must use an exact version specifier, not a range or wildcard.
#[test]
fn enigo_version_is_not_wildcard() {
    let manifest = read_manifest();
    let enigo_line = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("enigo"))
        .expect("enigo dependency line not found");
    assert!(
        !enigo_line.contains('*'),
        "enigo version must not use a wildcard '*': {enigo_line}"
    );
    assert!(
        !enigo_line.contains('^'),
        "enigo should use a plain version string, not a caret '^' range: {enigo_line}"
    );
}