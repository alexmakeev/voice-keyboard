//! Debug logging system for Voice Keyboard
//!
//! Captures detailed logs for debugging transcription issues

use chrono::Local;
use std::fmt::Write;

/// Debug log that captures all events during a recording session
pub struct DebugLog {
    content: String,
    session_start: Option<chrono::DateTime<Local>>,
}

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

    /// Get full log content
    pub fn get_content(&self) -> &str {
        &self.content
    }

    fn log(&mut self, category: &str, message: &str) {
        let timestamp = Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(self.content, "[{}] [{}] {}", timestamp, category, message);
    }
}

impl Default for DebugLog {
    fn default() -> Self {
        Self::new()
    }
}
