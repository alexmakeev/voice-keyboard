//! Minimal Voice Keyboard - Double-tap Left Control to insert "привет"
//!
//! This is a minimal test version that:
//! 1. Listens for double-tap on Left Control key
//! 2. Inserts "привет" at cursor position via clipboard
//!
//! Usage:
//!   cargo run --bin minimal

#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use rdev::{listen, Event, EventType, Key};

#[cfg(target_os = "macos")]
use arboard::Clipboard;

/// Double-tap detection timeout
#[cfg(target_os = "macos")]
const DOUBLE_TAP_TIMEOUT_MS: u64 = 500;

/// Text to insert
const INSERT_TEXT: &str = "привет";

fn main() {
    println!("Voice Keyboard Minimal Test");
    println!("============================");
    println!("Double-tap Left Control to insert: \"{}\"", INSERT_TEXT);
    println!("Press Ctrl+C to exit\n");

    #[cfg(target_os = "macos")]
    run_macos();

    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("This binary requires macOS for global hotkey support.");
        eprintln!("On Linux, you can test the clipboard injection separately.");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
fn run_macos() {
    // Tracks: (last_tap_time, last_insert_time)
    let state: Arc<Mutex<(Option<Instant>, Option<Instant>)>> = Arc::new(Mutex::new((None, None)));
    let state_clone = Arc::clone(&state);

    let callback = move |event: Event| {
        // Only react to KeyRelease of ControlLeft (double-tap detection on release)
        if let EventType::KeyRelease(Key::ControlLeft) = event.event_type {
            let mut state = state_clone.lock().unwrap();
            let now = Instant::now();

            // Ignore if we inserted recently (cooldown period)
            if let Some(last_insert) = state.1 {
                if now.duration_since(last_insert) < Duration::from_millis(500) {
                    println!("[{}] Ignoring (cooldown after insert)", chrono_lite());
                    return;
                }
            }

            if let Some(prev) = state.0 {
                let elapsed = now.duration_since(prev);
                if elapsed < Duration::from_millis(DOUBLE_TAP_TIMEOUT_MS) {
                    // Double-tap detected!
                    println!("[{}] Double-tap detected! Inserting text...", chrono_lite());

                    // Clear the last tap and set insert time
                    state.0 = None;
                    state.1 = Some(now);

                    // Drop the lock before sleeping/inserting
                    drop(state);

                    // Small delay to ensure Control key is fully released
                    std::thread::sleep(Duration::from_millis(50));

                    // Insert text
                    if let Err(e) = insert_text(INSERT_TEXT) {
                        eprintln!("Error inserting text: {}", e);
                    } else {
                        println!("[{}] Text inserted successfully!", chrono_lite());
                    }
                    return;
                }
            }

            // Record this tap (first tap or too slow = new first tap)
            state.0 = Some(now);
            println!(
                "[{}] Control released (waiting for double-tap...)",
                chrono_lite()
            );
        }
    };

    println!("[{}] Listening for hotkeys...", chrono_lite());

    if let Err(e) = listen(callback) {
        eprintln!("Error listening for hotkeys: {:?}", e);
        eprintln!("\nMake sure to grant Input Monitoring permission:");
        eprintln!("System Settings → Privacy & Security → Input Monitoring");
    }
}

/// Insert text via clipboard + Cmd+V, preserving previous clipboard content
#[cfg(target_os = "macos")]
fn insert_text(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard error: {}", e))?;

    // Save current clipboard content
    let previous = clipboard.get_text().ok();

    // Set our text
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("Failed to set clipboard: {}", e))?;

    // Small delay to ensure clipboard is updated
    std::thread::sleep(Duration::from_millis(10));

    // Simulate Cmd+V
    #[cfg(target_os = "macos")]
    {
        use enigo::{Direction, Enigo, Key as EnigoKey, Keyboard, Settings};

        let mut enigo =
            Enigo::new(&Settings::default()).map_err(|e| format!("Enigo error: {}", e))?;

        // Press Meta (Cmd)
        enigo
            .key(EnigoKey::Meta, Direction::Press)
            .map_err(|e| format!("Key press error: {}", e))?;

        // Press and release V
        enigo
            .key(EnigoKey::Unicode('v'), Direction::Click)
            .map_err(|e| format!("Key click error: {}", e))?;

        // Release Meta
        enigo
            .key(EnigoKey::Meta, Direction::Release)
            .map_err(|e| format!("Key release error: {}", e))?;
    }

    // Wait for paste to complete
    std::thread::sleep(Duration::from_millis(100));

    // Restore previous clipboard content
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }

    Ok(())
}

/// Simple timestamp for logging (no chrono dependency)
#[cfg(target_os = "macos")]
fn chrono_lite() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let secs = now.as_secs() % 86400; // seconds since midnight
    let hours = (secs / 3600) % 24;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}
