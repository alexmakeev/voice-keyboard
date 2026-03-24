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

    tauri_build::build()
}
