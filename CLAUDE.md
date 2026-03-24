# Voice Keyboard

Push-to-talk voice keyboard with speech recognition. Supports cloud-based GPT-4o (recommended) and local Whisper models. Cross-platform: macOS, Linux, Windows.

## Tech Stack

- **Language:** Rust (edition 2021)
- **Audio:** cpal (recording), ogg-opus (compression), hound (WAV)
- **Speech recognition:** OpenAI GPT-4o API (cloud), whisper-rs/whisper.cpp (local)
- **Input simulation:** enigo (keyboard/clipboard), rdev (global hotkeys)
- **Desktop app:** Tauri (optional GUI wrapper with tray icon)
- **GUI:** egui/eframe (settings window)
- **Async:** tokio
- **Error handling:** thiserror + anyhow

## Build Commands

**CRITICAL: Always include the `opus` feature flag. Without it, audio is sent as uncompressed WAV (10x larger).**

```bash
# macOS Apple Silicon (primary development platform)
cargo build --release --features "whisper,metal,opus"

# macOS Intel
cargo build --release --features "whisper,opus"

# Linux / Windows
cargo build --release --features "whisper,opus"

# OpenAI-only (no local Whisper)
cargo build --release --features opus

# Tauri desktop app (must build voice-typer FIRST, then Tauri)
cargo build --release --features "whisper,metal,opus"
cargo tauri build
```

## Run

```bash
# GPT-4o mode (recommended)
OPENAI_API_KEY="sk-..." ./target/release/voice-typer --openai

# Local Whisper
./target/release/voice-typer --model large-v3-turbo

# Debug logging
RUST_LOG=debug ./target/release/voice-typer --openai
```

## Test Commands

```bash
cargo test
```

Tests are in `tests/` directory: `transcription_test.rs`, `vad_test.rs`, `voice_typer_test.rs`, `whisper_test.rs`.

Dev dependencies: `tempfile`, `assert_cmd`.

## Feature Flags

| Feature | Required | Description |
|---------|----------|-------------|
| `opus` | **YES, always** | OGG/Opus audio compression. Without it = 10x larger uploads. |
| `whisper` | For local mode | Local Whisper speech recognition via whisper-rs. |
| `metal` | macOS ARM | Metal GPU acceleration for Apple Silicon. |
| `coreml` | macOS alt | CoreML acceleration (alternative to Metal). |
| `gui` | Optional | Full GUI: settings window + tray + HID/gamepad input. |
| `updater` | Optional | Auto-update from GitHub Releases. |

## Architecture

```
voice-typer (src/bin/voice_typer.rs)    <- Main binary (~5500 lines)
  +-- Global hotkey listener (rdev)
  +-- Audio recording (cpal) -- persistent stream
  +-- OGG/Opus encoding (ogg-opus)
  +-- OpenAI API / Local Whisper transcription
  +-- Text injection (enigo) -- keyboard or clipboard
  +-- Volume control (src/volume.rs)

Tauri App (src-tauri/)                  <- Optional desktop wrapper
  +-- GUI: HTML/JS/CSS (ui/)
  +-- Spawns voice-typer as child process
```

## Key Source Files

- `src/bin/voice_typer.rs` -- main voice-to-text binary (largest file)
- `src/audio.rs` -- audio recording via cpal
- `src/transcribe.rs` -- Whisper integration
- `src/hotkey.rs` -- global hotkey listener
- `src/inject.rs` -- text injection (keyboard/clipboard)
- `src/volume.rs` -- system volume control during recording
- `src/config.rs` -- configuration management
- `src-tauri/src/main.rs` -- Tauri app backend
- `ui/` -- settings UI (HTML/JS/CSS)

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | OpenAI API key for GPT-4o mode |
| `MODEL_PATH` | Path to Whisper model file |
| `VOICE_KEYBOARD_LANGUAGES` | Languages for auto-detection (default: Russian, English) |
| `VOICE_KEYBOARD_DEV` | Enable dev mode (save reports) |
| `WHISPER_ENHANCE` | Audio enhancement settings for local Whisper |
| `RUST_LOG` | Logging level (debug, trace) |

## Config File

Location: `~/.config/voice-keyboard/config.json`

## Project Conventions

- The `opus` feature is mandatory for all builds -- never omit it
- Tauri app requires building voice-typer binary BEFORE `cargo tauri build`
- voice-typer binary is copied into the Tauri app bundle at `Contents/MacOS/voice-typer`
- On macOS, rdev uses `unstable_grab` feature for key suppression
- Release profile uses LTO, single codegen unit, and symbol stripping
