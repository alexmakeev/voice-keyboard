//! Build script for the voice-keyboard workspace root crate.
//! Reads APP_VERSION from src-tauri/tauri.conf.json and sets it as a compile-time env var.

use std::fs;

fn main() {
    let conf = fs::read_to_string("src-tauri/tauri.conf.json")
        .expect("Failed to read src-tauri/tauri.conf.json");
    let json: serde_json::Value =
        serde_json::from_str(&conf).expect("Failed to parse tauri.conf.json");
    let version = json["version"]
        .as_str()
        .expect("No version field in tauri.conf.json");
    println!("cargo:rustc-env=APP_VERSION={}", version);
    println!("cargo:rerun-if-changed=src-tauri/tauri.conf.json");
}
