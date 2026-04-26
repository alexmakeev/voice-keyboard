//! System volume control — lowers output volume during recording to reduce
//! music/audio interference with microphone input.
//!
//! Platform support:
//!   - macOS: `osascript` (AppleScript)
//!   - Linux: `pactl` (PulseAudio/PipeWire)
//!   - Windows: direct winmm.dll FFI (waveOutGet/SetVolume)

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Minimum volume during recording (not silent — user still hears feedback beeps)
const MIN_VOLUME: u32 = 10;

/// Get current system output volume (0–100).
fn get_system_volume() -> Option<u32> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args(["-e", "output volume of (get volume settings)"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().parse::<u32>().ok()
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("pactl")
            .args(["get-sink-volume", "@DEFAULT_SINK@"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        // pactl output looks like: "Volume: front-left: 65536 / 100% / 0.00 dB ..."
        for part in stdout.split('/') {
            let trimmed = part.trim();
            if let Some(pct) = trimmed.strip_suffix('%') {
                if let Ok(v) = pct.trim().parse::<u32>() {
                    return Some(v);
                }
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    {
        // Direct winmm.dll FFI. Earlier this shelled out to `powershell.exe`
        // running the same Add-Type Add-Type winmm wrapper, which cost
        // 500-1000ms per call (PowerShell startup + JIT). With volume
        // control invoked from the hotkey worker on every record-start /
        // record-stop, that delay was visible to the user as "recording
        // doesn't start until ~2s after the press." The native call here
        // is sub-millisecond.
        extern "system" {
            fn waveOutGetVolume(hwo: *mut std::ffi::c_void, pdwVolume: *mut u32) -> u32;
        }
        let mut v: u32 = 0;
        let result = unsafe { waveOutGetVolume(std::ptr::null_mut(), &mut v) };
        if result != 0 {
            return None;
        }
        let left = v & 0xFFFF;
        Some(((left as f64 / 65535.0) * 100.0).round() as u32)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Set system output volume (0–100).
fn set_system_volume(volume: u32) {
    let vol = volume.min(100);

    #[cfg(target_os = "macos")]
    {
        let script = format!("set volume output volume {}", vol);
        let _ = Command::new("osascript").args(["-e", &script]).output();
    }

    #[cfg(target_os = "linux")]
    {
        let arg = format!("{}%", vol);
        let _ = Command::new("pactl")
            .args(["set-sink-volume", "@DEFAULT_SINK@", &arg])
            .output();
    }

    #[cfg(target_os = "windows")]
    {
        // Direct winmm.dll FFI — see get_system_volume() for context.
        extern "system" {
            fn waveOutSetVolume(hwo: *mut std::ffi::c_void, dwVolume: u32) -> u32;
        }
        let val = (vol as f64 / 100.0 * 65535.0) as u32;
        let both = val | (val << 16);
        unsafe {
            waveOutSetVolume(std::ptr::null_mut(), both);
        }
    }
}

/// Controls system volume — instantly lowers on recording start, restores on stop.
pub struct VolumeController {
    /// Volume saved before lowering
    original_volume: AtomicU32,
    /// Whether volume is currently lowered
    is_lowered: AtomicBool,
    /// Whether this controller is enabled
    enabled: bool,
}

impl VolumeController {
    pub fn new(enabled: bool) -> Self {
        Self {
            original_volume: AtomicU32::new(0),
            is_lowered: AtomicBool::new(false),
            enabled,
        }
    }

    /// Instantly lower system volume to MIN_VOLUME.
    pub fn lower(&self) {
        if !self.enabled {
            return;
        }

        // Prevent double-lower
        if self.is_lowered.swap(true, Ordering::SeqCst) {
            return;
        }

        let current = match get_system_volume() {
            Some(v) => v,
            None => {
                self.is_lowered.store(false, Ordering::SeqCst);
                return;
            }
        };

        self.original_volume.store(current, Ordering::SeqCst);

        if current > MIN_VOLUME {
            set_system_volume(MIN_VOLUME);
        }
    }

    /// Instantly restore system volume to saved level.
    pub fn restore(&self) {
        if !self.enabled {
            return;
        }

        if !self.is_lowered.swap(false, Ordering::SeqCst) {
            return; // Not lowered — nothing to restore
        }

        let target = self.original_volume.load(Ordering::SeqCst);
        if target > 0 {
            set_system_volume(target);
        }
    }
}

impl Drop for VolumeController {
    fn drop(&mut self) {
        // Safety net: restore volume if app exits while lowered
        if *self.is_lowered.get_mut() {
            let target = *self.original_volume.get_mut();
            if target > 0 {
                set_system_volume(target);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_controller_noop_when_disabled() {
        let vc = VolumeController::new(false);
        // These should do nothing and not panic
        vc.lower();
        vc.restore();
        assert!(!vc.is_lowered.load(Ordering::SeqCst));
    }
}
