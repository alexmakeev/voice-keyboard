//! Build script for the Tauri app crate.
//! Reads APP_VERSION from tauri.conf.json and invokes tauri_build to generate bindings.

use std::fs;

fn main() {
    let conf =
        fs::read_to_string("tauri.conf.json").expect("Failed to read tauri.conf.json");
    let json: serde_json::Value =
        serde_json::from_str(&conf).expect("Failed to parse tauri.conf.json");
    let version = json["version"]
        .as_str()
        .expect("No version field in tauri.conf.json");
    println!("cargo:rustc-env=APP_VERSION={}", version);
    println!("cargo:rerun-if-changed=tauri.conf.json");

    // frontendDist ("../ui") is a plain static directory, not a JS build
    // output -- there is no bundler step that would otherwise put its files
    // on Cargo's dependency graph. Without an explicit rerun-if-changed here,
    // Cargo can decide no tracked input changed and skip re-embedding the
    // frontend assets into the binary, silently shipping a stale UI (HTML/
    // CSS/JS) even though the source files on disk are current. Watch the
    // whole tree so any edit under ui/ forces a rebuild.
    println!("cargo:rerun-if-changed=../ui");

    tauri_build::build()
}
