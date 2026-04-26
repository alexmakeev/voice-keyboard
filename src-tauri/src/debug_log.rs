//! Debug logging system for Voice Keyboard
//!
//! Captures detailed logs for debugging transcription issues

use std::fmt::Write;
use chrono::Local;

/// Debug log that captures all events during a recording session
pub struct DebugLog {
    content: String,
    session_start: Option<chrono::DateTime<Local>>,
}

#[allow(dead_code)]
impl DebugLog {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            session_start: None,
        }
    }

    /// Start a new recording session
    pub fn start_session(&mut self) {
        self.content.clear();
        self.session_start = Some(Local::now());
        self.log("SESSION", "Recording started");
    }

    /// End current session
    pub fn end_session(&mut self) {
        self.log("SESSION", "Recording ended");
        if let Some(start) = self.session_start {
            let duration = Local::now() - start;
            self.log("SESSION", &format!("Total duration: {:.1}s", duration.num_milliseconds() as f64 / 1000.0));
        }
    }

    /// Log a general message
    pub fn log(&mut self, category: &str, message: &str) {
        let timestamp = Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(self.content, "[{}] [{}] {}", timestamp, category, message);
    }

    /// Log VAD state
    pub fn log_vad(&mut self, time_secs: f32, in_speech: bool, silent_windows: usize, energy: f32, voice_ratio: f32) {
        self.log("VAD", &format!(
            "t={:.1}s in_speech={} silent={} energy={:.4} voice_ratio={:.2}",
            time_secs, in_speech, silent_windows, energy, voice_ratio
        ));
    }

    /// Log phrase detection
    pub fn log_phrase_detected(&mut self, duration_secs: f32) {
        self.log("PHRASE", &format!("Detected phrase, duration={:.1}s", duration_secs));
    }

    /// Log transcription result
    pub fn log_transcription(&mut self, text: &str, is_continuation: bool) {
        if is_continuation {
            self.log("TRANSCRIBE", &format!("Continuation: \"{}\"", text));
        } else {
            self.log("TRANSCRIBE", &format!("New phrase: \"{}\"", text));
        }
    }

    /// Log text insertion
    pub fn log_insert(&mut self, text: &str) {
        self.log("INSERT", &format!("Inserting: \"{}\"", text));
    }

    /// Log character deletion
    pub fn log_delete(&mut self, count: usize, deleted_chars: &str) {
        self.log("DELETE", &format!("Deleting {} chars: \"{}\"", count, deleted_chars));
    }

    /// Log hallucination filter
    pub fn log_hallucination_filtered(&mut self, text: &str) {
        self.log("FILTER", &format!("Hallucination filtered: \"{}\"", text));
    }

    /// Log noise-only phrase discard
    pub fn log_noise_discarded(&mut self, voice_percent: f32) {
        self.log("FILTER", &format!("Noise-only phrase discarded ({:.0}% voice)", voice_percent));
    }

    /// Log error
    pub fn log_error(&mut self, message: &str) {
        self.log("ERROR", message);
    }

    /// Get full log content
    pub fn get_content(&self) -> &str {
        &self.content
    }

    /// Check if log is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

impl Default for DebugLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_log() {
        let mut log = DebugLog::new();
        log.start_session();
        log.log_vad(1.5, true, 0, 0.015, 0.85);
        log.log_phrase_detected(2.3);
        log.log_transcription("Hello world", false);
        log.log_insert("Hello world ");
        log.end_session();

        let content = log.get_content();
        assert!(content.contains("[SESSION]"));
        assert!(content.contains("[VAD]"));
        assert!(content.contains("[PHRASE]"));
        assert!(content.contains("[TRANSCRIBE]"));
        assert!(content.contains("[INSERT]"));
    }
}
