
Push-to-talk voice keyboard with speech recognition. Supports **cloud-based GPT-4o**, **OpenRouter** (cheap cloud alternative), and **local Whisper** models.

## How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   1. HOLD the hotkey    2. SPEAK         3. RELEASE the key    │
│                                                                 │
│      ┌─────┐               🎤                  ┌─────┐          │
│      │ Fn  │  ──────►  "Hello world"  ──────►  │ Fn  │          │
│      └─────┘                                   └─────┘          │
│       Press                                    Release          │
│                                                   │             │
│                                                   ▼             │
│                                           ┌─────────────┐       │
│                                           │ Hello world │       │
│                                           └─────────────┘       │
│                                           Text appears in       │
│                                           your active app       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Voice Keyboard** uses a push-to-talk interface:
1. **Press and hold** the hotkey to start recording
2. **Speak** — your voice is recorded
3. **Release** the key — speech is transcribed and text is typed into the active application

## Transcription Modes

| Mode | Quality | Speed | Privacy | Cost | Requirements |
|------|---------|-------|---------|------|--------------|
| **GPT-4o** (recommended) | Excellent | Fast | Cloud | ~$0.001/req | OpenAI API key |
| **OpenRouter** (cheap) | Excellent | Fast | Cloud | ~$0.0001–0.0004/req | OpenRouter API key |
| **Whisper Local** | Very Good | Varies | Local | Free | ~3GB RAM |

## Quick Start

### Prerequisites

**macOS:**
```bash
brew install cmake rust
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get update
sudo apt-get install -y build-essential cmake curl git \
  libasound2-dev libxdo-dev libx11-dev libxi-dev libxtst-dev pkg-config

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Installation

```bash
# Clone repository
git clone https://github.com/alexmakeev/voice-keyboard.git
cd voice-keyboard

# macOS Apple Silicon (M1/M2/M3/M4) — recommended
cargo build --release --features "whisper,metal,opus"

# macOS Intel
cargo build --release --features "whisper,opus"

# Linux
cargo build --release --features "whisper,opus"

# Windows
cargo build --release --features "whisper,opus"
```

> **IMPORTANT:** The `opus` feature is **required** for OGG/Opus audio compression.
> Without it, audio is sent as uncompressed WAV (10x larger, slower uploads).
> Always include `opus` in your build features.

### One-Line Install (macOS)

```bash
curl -sSL https://raw.githubusercontent.com/alexmakeev/voice-keyboard/main/scripts/setup-macos.sh | bash
```

### One-Line Install (Linux)

```bash
curl -sSL https://raw.githubusercontent.com/alexmakeev/voice-keyboard/main/scripts/setup-linux.sh | bash
```

---

## Mode 1: GPT-4o Transcription (Recommended)

Best quality transcription using OpenAI's GPT-4o model. Requires an API key.

### Setup

1. Get an API key from [OpenAI Platform](https://platform.openai.com/api-keys)

2. Set the environment variable:
```bash
export OPENAI_API_KEY="sk-your-api-key-here"
```

Or add to your shell profile (`~/.bashrc`, `~/.zshrc`):
```bash
echo 'export OPENAI_API_KEY="sk-your-api-key-here"' >> ~/.zshrc
source ~/.zshrc
```

3. Run with OpenAI mode:
```bash
./target/release/voice-typer --openai
```

### GPT-4o Advantages

- **Superior accuracy** for Russian, English, and mixed language
- **Better punctuation** and formatting
- **Context understanding** for technical terms
- **No local model download** required

---

## Mode 2: OpenRouter Transcription (Cheap Cloud)

High-quality transcription via [OpenRouter](https://openrouter.ai/) using multimodal models. Very affordable — typically $0.0001–$0.0004 per transcription. Sends OGG/Opus audio encoded as base64 to the Chat Completions API.

### Setup

1. Get an API key from [OpenRouter](https://openrouter.ai/keys)

2. Set the environment variable:
```bash
export OPENROUTER_API_KEY="sk-or-your-api-key-here"
```

Or add to your shell profile (`~/.bashrc`, `~/.zshrc`):
```bash
echo 'export OPENROUTER_API_KEY="sk-or-your-api-key-here"' >> ~/.zshrc
source ~/.zshrc
```

3. Run with OpenRouter mode:
```bash
./target/release/voice-typer --openrouter
```

### Model Selection

The default model is `google/gemini-2.5-flash`. Override with `OPENROUTER_MODEL`:

```bash
# Default (recommended)
OPENROUTER_MODEL="google/gemini-2.5-flash" ./target/release/voice-typer --openrouter

# Lighter/faster variant
OPENROUTER_MODEL="google/gemini-2.5-flash-lite" ./target/release/voice-typer --openrouter
```

### OpenRouter Advantages

- **Very cheap** — ~$0.0001–$0.0004 per transcription
- **No local model** required
- **Multiple model options** through a single API
- **Good accuracy** for Russian, English, and mixed language

---

## Mode 3: Local Whisper Transcription

All processing happens locally on your device — no data is sent to the cloud.

### Download a Model

**Automatic download (recommended):**
```bash
./target/release/voice-typer --download large-v3-turbo
```

**Manual download:**
```bash
# Create models directory
mkdir -p ~/.local/share/voice-keyboard/models
cd ~/.local/share/voice-keyboard/models

# Download large-v3-turbo (recommended, 1.6GB)
curl -L -O https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin

# Or download large-v3 (best quality, 3.1GB)
curl -L -O https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin

# Or download tiny for testing (75MB)
curl -L -O https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin
```

### Run with Local Whisper

```bash
# With default model
./target/release/voice-typer

# With specific model
./target/release/voice-typer --model large-v3-turbo
./target/release/voice-typer --model large-v3
./target/release/voice-typer --model tiny
```

### Available Models

| Model | Size | RAM | Speed | Quality | Download |
|-------|------|-----|-------|---------|----------|
| tiny | 75 MB | ~400 MB | ~32x | Basic | `--download tiny` |
| base | 142 MB | ~500 MB | ~16x | Good | `--download base` |
| small | 466 MB | ~1 GB | ~6x | Very Good | `--download small` |
| medium | 1.5 GB | ~3 GB | ~2x | Excellent | `--download medium` |
| **large-v3-turbo** | **1.6 GB** | **~3 GB** | **~8x** | **Best** | `--download large-v3-turbo` |
| large-v3 | 3.1 GB | ~6 GB | ~1x | Best | `--download large-v3` |

> **Recommendation**: Use **large-v3-turbo** for the best balance of quality and speed.

---

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key for GPT-4o mode | `sk-...` |
| `OPENROUTER_API_KEY` | OpenRouter API key for OpenRouter mode | `sk-or-...` |
| `OPENROUTER_MODEL` | OpenRouter model to use (default: `google/gemini-2.5-flash`) | `google/gemini-2.5-flash-lite` |
| `MODEL_PATH` | Path to Whisper model file | `~/.local/share/voice-keyboard/models/ggml-large-v3-turbo.bin` |
| `VOICE_KEYBOARD_LANGUAGES` | Languages for auto-detection (default: Russian, English) | `Russian, English, German` |
| `VOICE_KEYBOARD_DEV` | Enable dev mode (save reports) | `1` |
| `WHISPER_ENHANCE` | Audio enhancement settings | `all`, `none`, or `normalize,dc,pre_emphasis` |
| `RUST_LOG` | Logging level | `debug`, `trace` |

### WHISPER_ENHANCE Options

Controls audio preprocessing for local Whisper:
- `all` (default) — enable all enhancements
- `none` — disable all enhancements
- `normalize` — peak normalization
- `noise_reduction` or `denoise` — noise gate
- `dc` or `dc_offset` — DC offset removal
- `pre_emphasis` or `preemph` — high-frequency boost

Example:
```bash
WHISPER_ENHANCE=normalize,dc ./target/release/voice-typer
```

---

## Command Line Options

```
Usage: voice-typer [OPTIONS]

Transcription Mode:
  --openai             Use OpenAI GPT-4o API (requires OPENAI_API_KEY)
  --openrouter         Use OpenRouter API (requires OPENROUTER_API_KEY)
  --model <MODEL>      Use local Whisper model (tiny, base, small, medium, large-v3-turbo, large-v3)

Model Management:
  --download <MODEL>   Download a model from the internet
  --list-models        List available models and download URLs

Input/Output:
  --key <KEY>          Push-to-talk hotkey (fn, ctrl, ctrlright, alt, shift, cmd)
  --keyboard           Use keyboard simulation (default)
  --clipboard          Use clipboard + paste instead of keyboard

Audio:
  --volume <0.0-1.0>   Beep sounds volume (default: 0.1 = 10%)
  --silent, -q         Disable all beep sounds

Other:
  --list-keys          List available hotkeys
  --version, -V        Show version
  --help, -h           Show help

Experimental:
  --extra-keys         Enable experimental extra hotkeys (see below)
```

### Experimental Features (Beta)

Enable with `--extra-keys` flag:

```bash
./target/release/voice-typer --openai --extra-keys
```

| Hotkey | Function | Description |
|--------|----------|-------------|
| **Right Cmd** | Structured summary | Transcribes speech and generates a structured summary in the same language |
| **Right Option** | Translate to English | Transcribes speech and translates with summary to English |

> ⚠️ **Beta**: These features are experimental and may not work perfectly. They require additional API calls to GPT-4 for text processing.

### Smart Features

#### Silence Detection

Short recordings (< 3 seconds) are automatically checked for voice content using spectral analysis. If no voice is detected (just silence or background noise), the recording is skipped and a low double beep plays ("pup-pup") to indicate cancellation.

This prevents accidental button presses from being sent to the API.

- Recordings **< 3 sec**: Checked for voice, skipped if silent
- Recordings **≥ 3 sec**: Always processed (API decides if meaningful)

#### Connection Lost Retry

If the network connection is lost during transcription:

1. A prominent `CONNECTION LOST` message is displayed
2. The failed recording is saved
3. Press the hotkey again to retry with a double beep confirmation
4. If still no connection, the message repeats

This ensures no voice recordings are lost due to temporary network issues.

### Examples

```bash
# GPT-4o mode (best quality)
OPENAI_API_KEY="sk-..." ./target/release/voice-typer --openai

# OpenRouter mode (cheap cloud)
OPENROUTER_API_KEY="sk-or-..." ./target/release/voice-typer --openrouter

# OpenRouter with a specific model
OPENROUTER_API_KEY="sk-or-..." OPENROUTER_MODEL="google/gemini-2.5-flash-lite" ./target/release/voice-typer --openrouter

# Local Whisper with turbo model
./target/release/voice-typer --model large-v3-turbo

# Silent mode (no beeps)
./target/release/voice-typer --openai --silent

# Custom hotkey (Right Ctrl)
./target/release/voice-typer --openai --key ctrlright

# Clipboard mode (paste instead of type)
./target/release/voice-typer --openai --clipboard

# Download and use a model
./target/release/voice-typer --download large-v3-turbo
./target/release/voice-typer --model large-v3-turbo

# Debug logging
RUST_LOG=debug ./target/release/voice-typer --openai
```

---

## Hotkeys

### Default Hotkey by Platform

| Platform | Default Hotkey | Notes |
|----------|---------------|-------|
| **macOS** | `Fn` (Globe key) | Works on MacBook keyboards |
| **Linux** | `Right Ctrl` | Fn key is hardware-only on most keyboards |
| **Windows** | `Right Ctrl` | Fn key is hardware-only on most keyboards |

### Available Hotkeys

| Key | Flag | Description |
|-----|------|-------------|
| `fn` | `--key fn` | Fn/Globe key (macOS only) |
| `ctrl` | `--key ctrl` | Left Control |
| `ctrlright` | `--key ctrlright` | Right Control |
| `alt` | `--key alt` | Left Alt/Option |
| `altright` | `--key altright` | Right Alt/Option |
| `shift` | `--key shift` | Left Shift |
| `cmd` | `--key cmd` | Command/Super/Win |

---

## Platform-Specific Notes

### macOS

#### Permissions

Grant these permissions in **System Settings → Privacy & Security**:

| Permission | Why | How |
|------------|-----|-----|
| **Microphone** | Record speech | Prompted automatically |
| **Accessibility** | Type text | Add terminal/app to Accessibility |
| **Input Monitoring** | Detect hotkey | Add terminal/app to Input Monitoring |

#### Hardware Acceleration

```bash
# Metal (recommended for M1/M2/M3/M4)
cargo build --release --features "whisper,metal,opus"

# CoreML (alternative)
cargo build --release --features "whisper,coreml,opus"
```

### Linux

#### Dependencies

**Ubuntu/Debian:**
```bash
sudo apt-get install -y build-essential cmake curl git \
  libasound2-dev libxdo-dev libx11-dev libxi-dev libxtst-dev pkg-config
```

**Fedora:**
```bash
sudo dnf install -y cmake gcc-c++ alsa-lib-devel libxdo-devel \
  libX11-devel libXi-devel libXtst-devel
```

**Arch Linux:**
```bash
sudo pacman -S cmake alsa-lib xdotool libx11 libxi libxtst
```

#### Running

```bash
# May need to run with sudo for input events
./target/release/voice-typer

# Or add user to input group
sudo usermod -aG input $USER
# Log out and back in
```

### Windows

#### Requirements
- Windows 10/11
- Visual Studio Build Tools 2019+
- CMake 3.20+

Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++".

---

## Configuration File

Location:
- macOS/Linux: `~/.config/voice-keyboard/config.json`
- Windows: `%APPDATA%\voice-keyboard\config.json`

Example:
```json
{
  "model_path": "~/.local/share/voice-keyboard/models/ggml-large-v3-turbo.bin",
  "language": "ru",
  "hotkey": {
    "trigger_key": "Function",
    "push_to_talk": true
  },
  "injection_method": "keyboard",
  "streaming": false
}
```

### Config Options

| Option | Values | Description |
|--------|--------|-------------|
| `language` | `"auto"`, `"en"`, `"ru"`, etc. | Recognition language |
| `injection_method` | `"keyboard"`, `"clipboard"` | How to input text |
| `trigger_key` | `"Function"`, `"ControlRight"`, etc. | Push-to-talk key |
| `streaming` | `true`, `false` | Enable fragmentary recognition |

---

## Troubleshooting

### Common Issues

**"Model not found" error:**
```bash
# Check if model exists
ls ~/.local/share/voice-keyboard/models/

# Download model
./target/release/voice-typer --download large-v3-turbo
```

**Text goes to wrong app (macOS):**
- Grant Accessibility permission to Terminal/iTerm
- Restart the application

**Hotkey not working:**
- macOS: Grant Input Monitoring permission
- Linux: Run with sudo or add user to input group
- Windows: Run as Administrator

**Transcription is slow:**
- Use GPT-4o mode (`--openai`) for fastest results
- Use Metal acceleration on macOS: `--features metal`
- Use smaller model: `--model tiny`

**Wrong language detected:**
- Set language in config: `"language": "ru"`
- GPT-4o mode auto-detects language better

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug ./target/release/voice-typer --openai

# Trace all events
RUST_LOG=trace ./target/release/voice-typer
```

---

## Development

### Building from Source

#### voice-typer (CLI binary)

```bash
git clone https://github.com/alexmakeev/voice-keyboard.git
cd voice-keyboard

# macOS Apple Silicon — full build
cargo build --release --features "whisper,metal,opus"

# macOS Intel
cargo build --release --features "whisper,opus"

# Linux / Windows
cargo build --release --features "whisper,opus"

# Run tests
cargo test
```

#### Tauri Desktop App (Voice Keyboard.app)

The Tauri app wraps voice-typer in a native macOS/Linux/Windows desktop application with GUI settings, tray icon, and model management.

```bash
# Prerequisites: install Tauri CLI
cargo install tauri-cli

# Step 1: Build voice-typer with ALL required features
cargo build --release --features "whisper,metal,opus"

# Step 2: Build the Tauri app
cargo tauri build

# Step 3: Install (macOS)
cp -R src-tauri/target/release/bundle/macos/Voice\ Keyboard.app /Applications/
cp target/release/voice-typer /Applications/Voice\ Keyboard.app/Contents/MacOS/voice-typer
```

> **CRITICAL:** Step 1 must be done BEFORE `cargo tauri build`.
> The Tauri app spawns `voice-typer` as a child process from `Contents/MacOS/voice-typer`.
> If voice-typer was not built with `opus` feature, OGG compression will silently
> fall back to WAV — the setting will appear to do nothing.

#### Versioning

The single source of truth for app version is `src-tauri/tauri.conf.json` field `"version"`. When releasing a new version, only this file needs to be updated — both Rust crates and the UI read it automatically via `build.rs`.

### Build Features Reference

| Feature | Required | Description |
|---------|----------|-------------|
| `opus` | **YES** | OGG/Opus audio compression. Without this, audio uploads are 10x larger (WAV). **Always include.** |
| `whisper` | For local mode | Local Whisper speech recognition. Not needed if using only OpenAI API mode. |
| `metal` | macOS ARM | Metal GPU acceleration for Whisper on Apple Silicon (M1/M2/M3/M4). |
| `coreml` | macOS alt | CoreML acceleration for Whisper (alternative to Metal). |

**Minimum build commands:**

| Platform | Command |
|----------|---------|
| macOS Apple Silicon | `cargo build --release --features "whisper,metal,opus"` |
| macOS Intel | `cargo build --release --features "whisper,opus"` |
| Linux | `cargo build --release --features "whisper,opus"` |
| Windows | `cargo build --release --features "whisper,opus"` |
| OpenAI-only (any platform) | `cargo build --release --features opus` |

### Critical Features Checklist

Before releasing or deploying, verify these features work correctly:

| # | Feature | How to Verify | What Breaks If Missing |
|---|---------|---------------|----------------------|
| 1 | **OGG/Opus compression** | Check logs for `audio.ogg` (not `audio.wav`). Build with `--features opus`. | Audio sent as WAV (10x larger), slow uploads, higher API costs |
| 2 | **Push-to-talk hotkey** | Press and hold configured key → mic icon appears → release → text typed | Core functionality broken |
| 3 | **Persistent audio stream** | Mic icon appears instantly on key press (not delayed 1-2 sec) | Mic activates late, recordings miss first words |
| 4 | **Volume lowering** | Play music → press hotkey → music gets quiet → release → music restores | Music interferes with speech recognition |
| 5 | **Audio device selection** | Select specific mic in settings → voice-typer uses that device | Wrong mic used (e.g. BT headset instead of built-in) |
| 6 | **Min recording duration** | Quick tap (<1s) should be ignored, not sent to API | Accidental taps waste API calls |
| 7 | **Whisper Metal acceleration** | On Apple Silicon, build with `metal` feature. Check logs for Metal init. | Whisper runs on CPU (10x slower) |
| 8 | **Model download/delete** | In Tauri app settings, download and delete models | Users can't manage local models |
| 9 | **Config save/reload** | Change settings → Save & Reload → voice-typer restarts with new config | Settings don't apply until manual restart |
| 10 | **macOS permissions** | App prompts for Microphone, Accessibility, Input Monitoring | App silently fails to record or type |

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Tauri App (src-tauri/src/main.rs)                           │
│  ├── GUI: HTML/JS/CSS (ui/)                                  │
│  ├── Tray icon, config management, model downloads           │
│  └── Spawns voice-typer as child process                     │
│         │                                                    │
│         ▼                                                    │
│  voice-typer (src/bin/voice_typer.rs)                        │
│  ├── Global hotkey listener (rdev)                           │
│  ├── Audio recording (cpal) — persistent stream              │
│  ├── OGG/Opus encoding (ogg-opus) — REQUIRES opus feature    │
│  ├── OpenAI API / Local Whisper transcription                │
│  ├── Text injection (enigo) — keyboard or clipboard          │
│  └── Volume control (src/volume.rs) — lower during recording │
└──────────────────────────────────────────────────────────────┘
```

### Project Structure

```
voice-keyboard/
├── src/
│   ├── bin/
│   │   ├── voice_typer.rs       # Main voice-to-text binary (~5500 lines)
│   │   ├── whisper_enhance.rs   # Audio preprocessing for Whisper
│   │   └── voice_recorder.rs    # Audio recorder
│   ├── lib.rs
│   ├── volume.rs                # System volume control (lower during recording)
│   ├── audio.rs                 # Audio recording (cpal)
│   ├── transcribe.rs            # Whisper integration
│   ├── hotkey.rs                # Global hotkey listener
│   ├── inject.rs                # Text injection
│   └── config.rs                # Configuration
├── src-tauri/
│   ├── src/main.rs              # Tauri app backend
│   └── Cargo.toml               # Tauri dependencies
├── ui/
│   ├── index.html               # Settings UI
│   ├── app.js                   # Settings logic
│   └── styles.css               # UI styles
├── scripts/
│   ├── setup-macos.sh           # macOS installer
│   ├── setup-linux.sh           # Linux installer
│   ├── setup-windows.ps1        # Windows installer
│   └── reset-permissions.sh     # Reset macOS TCC permissions
└── Cargo.toml                   # Root crate with feature flags
```

---

## License

MIT License - see [LICENSE](LICENSE)

## Credits

- [whisper.cpp](https://github.com/ggml-org/whisper.cpp) - Whisper C++ implementation
- [whisper-rs](https://github.com/tazz4843/whisper-rs) - Rust bindings
- [OpenAI Whisper](https://github.com/openai/whisper) - Original model
- [OpenAI GPT-4o](https://platform.openai.com/) - Cloud transcription
- [cpal](https://github.com/RustAudio/cpal) - Cross-platform audio
- [rdev](https://github.com/Narsil/rdev) - Global hotkey detection

## Support

- Issues: [GitHub Issues](https://github.com/alexmakeev/voice-keyboard/issues)
- Discussions: [GitHub Discussions](https://github.com/alexmakeev/voice-keyboard/discussions)
