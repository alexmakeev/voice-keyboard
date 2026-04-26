//! Voice Recorder - Record audio and paste as OGG file
//!
//! Double-tap Left Control to start recording
//! Single tap Left Control to stop and paste OGG file
//!
//! Usage:
//!   cargo run --bin voice-recorder

#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use rdev::{listen, Event, EventType, Key};

#[cfg(target_os = "macos")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Double-tap detection timeout
#[cfg(target_os = "macos")]
const DOUBLE_TAP_TIMEOUT_MS: u64 = 500;

/// Recording state
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq)]
enum RecordingState {
    Idle,
    Recording,
}

fn main() {
    println!("Voice Recorder");
    println!("==============");
    println!("Double-tap Left Control to START recording");
    println!("Single tap Left Control to STOP and paste OGG file");
    println!("Press Ctrl+C to exit\n");

    #[cfg(target_os = "macos")]
    run_macos();

    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("This binary requires macOS.");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
fn run_macos() {
    use cpal::Stream;

    // Shared state
    let state: Arc<Mutex<RecordingState>> = Arc::new(Mutex::new(RecordingState::Idle));
    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let stream: Arc<Mutex<Option<Stream>>> = Arc::new(Mutex::new(None));
    let last_tap: Arc<Mutex<(Option<Instant>, Option<Instant>)>> =
        Arc::new(Mutex::new((None, None)));

    let state_clone = Arc::clone(&state);
    let samples_clone = Arc::clone(&samples);
    let stream_clone = Arc::clone(&stream);
    let last_tap_clone = Arc::clone(&last_tap);

    let callback = move |event: Event| {
        if let EventType::KeyRelease(Key::ControlLeft) = event.event_type {
            let mut tap_state = last_tap_clone.lock().unwrap();
            let now = Instant::now();

            // Cooldown after action
            if let Some(last_insert) = tap_state.1 {
                if now.duration_since(last_insert) < Duration::from_millis(500) {
                    return;
                }
            }

            let mut rec_state = state_clone.lock().unwrap();

            match *rec_state {
                RecordingState::Idle => {
                    // Check for double-tap to start recording
                    if let Some(prev) = tap_state.0 {
                        if now.duration_since(prev) < Duration::from_millis(DOUBLE_TAP_TIMEOUT_MS) {
                            // Double-tap detected - start recording
                            println!("[{}] Double-tap! Starting recording...", timestamp());

                            tap_state.0 = None;
                            tap_state.1 = Some(now);

                            // Start recording
                            let samples_for_stream = Arc::clone(&samples_clone);
                            match start_recording(samples_for_stream) {
                                Ok(new_stream) => {
                                    *stream_clone.lock().unwrap() = Some(new_stream);
                                    *rec_state = RecordingState::Recording;
                                    println!(
                                        "[{}] Recording started! Tap Control to stop.",
                                        timestamp()
                                    );
                                }
                                Err(e) => {
                                    eprintln!("Failed to start recording: {}", e);
                                }
                            }
                            return;
                        }
                    }
                    // First tap - record time
                    tap_state.0 = Some(now);
                    println!(
                        "[{}] Control released (double-tap to start recording...)",
                        timestamp()
                    );
                }
                RecordingState::Recording => {
                    // Single tap while recording - stop and save
                    println!("[{}] Stopping recording...", timestamp());

                    tap_state.1 = Some(now);

                    // Stop stream
                    if let Some(s) = stream_clone.lock().unwrap().take() {
                        drop(s);
                    }

                    // Get samples
                    let recorded_samples: Vec<f32> = {
                        let mut s = samples_clone.lock().unwrap();
                        std::mem::take(&mut *s)
                    };

                    *rec_state = RecordingState::Idle;
                    tap_state.0 = None;

                    // Drop locks before processing
                    drop(rec_state);
                    drop(tap_state);

                    if recorded_samples.is_empty() {
                        println!("[{}] No audio recorded", timestamp());
                        return;
                    }

                    let duration_secs = recorded_samples.len() as f32 / 48000.0;
                    println!(
                        "[{}] Recorded {:.1}s of audio ({} samples)",
                        timestamp(),
                        duration_secs,
                        recorded_samples.len()
                    );

                    // Save and paste OGG file
                    match save_and_paste_ogg(&recorded_samples) {
                        Ok(path) => {
                            println!("[{}] OGG file pasted: {}", timestamp(), path.display());
                        }
                        Err(e) => {
                            eprintln!("Failed to save/paste OGG: {}", e);
                        }
                    }
                }
            }
        }
    };

    println!("[{}] Listening for hotkeys...", timestamp());

    if let Err(e) = listen(callback) {
        eprintln!("Error: {:?}", e);
        eprintln!("\nGrant Input Monitoring permission:");
        eprintln!("System Settings → Privacy & Security → Input Monitoring");
    }
}

#[cfg(target_os = "macos")]
fn start_recording(samples: Arc<Mutex<Vec<f32>>>) -> Result<cpal::Stream, String> {
    use cpal::SampleFormat;

    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No input device found")?;

    println!("Using input device: {}", device.name().unwrap_or_default());

    let config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {}", e))?;

    println!(
        "Input config: {} channels, {} Hz",
        config.channels(),
        config.sample_rate().0
    );

    let channels = config.channels() as usize;

    // Clear previous samples
    samples.lock().unwrap().clear();

    let err_fn = |err| eprintln!("Audio stream error: {}", err);

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut s = samples.lock().unwrap();
                // Convert to mono
                for chunk in data.chunks(channels) {
                    let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    s.push(mono);
                }
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => {
            let samples_clone = Arc::clone(&samples);
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    let mut s = samples_clone.lock().unwrap();
                    for chunk in data.chunks(channels) {
                        let mono: f32 = chunk
                            .iter()
                            .map(|&x| x as f32 / i16::MAX as f32)
                            .sum::<f32>()
                            / channels as f32;
                        s.push(mono);
                    }
                },
                err_fn,
                None,
            )
        }
        _ => return Err("Unsupported sample format".to_string()),
    }
    .map_err(|e| format!("Failed to build stream: {}", e))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start stream: {}", e))?;

    Ok(stream)
}

#[cfg(target_os = "macos")]
fn save_and_paste_ogg(samples: &[f32]) -> Result<PathBuf, String> {
    // Save to temp file as WAV (OGG encoding will be added later)
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join(format!("voice_{}.wav", timestamp().replace(":", "-")));
    save_wav(samples, &wav_path)?;

    // Copy file to clipboard and paste
    #[cfg(target_os = "macos")]
    {
        copy_file_to_clipboard_and_paste(&wav_path)?;
    }

    Ok(wav_path)
}

#[cfg(target_os = "macos")]
fn save_wav(samples: &[f32], path: &PathBuf) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV: {}", e))?;

    for &sample in samples {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer
            .write_sample(sample_i16)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn copy_file_to_clipboard_and_paste(path: &PathBuf) -> Result<(), String> {
    use std::process::Command;

    // Use osascript to copy file to clipboard
    let script = format!(r#"set the clipboard to POSIX file "{}""#, path.display());

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Small delay
    std::thread::sleep(Duration::from_millis(50));

    // Simulate Cmd+V
    use enigo::{Direction, Enigo, Key as EnigoKey, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Enigo error: {}", e))?;

    enigo
        .key(EnigoKey::Meta, Direction::Press)
        .map_err(|e| format!("Key error: {}", e))?;
    enigo
        .key(EnigoKey::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Key error: {}", e))?;
    enigo
        .key(EnigoKey::Meta, Direction::Release)
        .map_err(|e| format!("Key error: {}", e))?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn timestamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let secs = now.as_secs() % 86400;
    let hours = (secs / 3600) % 24;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}
