//! Update state persistence
//!
//! Tracks last update check time, installed version, crash count, etc.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Crash timeout in seconds - if app exits with error within this time, it's a crash
pub const CRASH_TIMEOUT_SECS: u64 = 30;
/// Maximum crashes before automatic rollback
pub const MAX_CRASHES: u32 = 3;

/// Persistent update state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    /// Last time we checked for updates
    #[serde(default)]
    pub last_check: Option<DateTime<Utc>>,

    /// Currently installed version
    #[serde(default = "default_version")]
    pub installed_version: String,

    /// Previous version (for rollback)
    #[serde(default)]
    pub previous_version: Option<String>,

    /// Time of last crash
    #[serde(default)]
    pub last_crash_time: Option<DateTime<Utc>>,

    /// Number of consecutive crashes
    #[serde(default)]
    pub crash_count: u32,

    /// Path to the state file (not serialized)
    #[serde(skip)]
    state_file: PathBuf,
}

fn default_version() -> String {
    env!("APP_VERSION").to_string()
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            last_check: None,
            installed_version: default_version(),
            previous_version: None,
            last_crash_time: None,
            crash_count: 0,
            state_file: PathBuf::new(),
        }
    }
}

impl UpdateState {
    /// Load state from file or create default
    pub fn load(data_dir: &Path) -> Result<Self> {
        let state_file = data_dir.join("update-state.json");

        let mut state = if state_file.exists() {
            let content =
                fs::read_to_string(&state_file).context("Failed to read update state file")?;

            serde_json::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse update state, using default: {}", e);
                UpdateState::default()
            })
        } else {
            UpdateState::default()
        };

        state.state_file = state_file;
        Ok(state)
    }

    /// Save state to file
    pub fn save(&self) -> Result<()> {
        if self.state_file.as_os_str().is_empty() {
            return Ok(());
        }

        if let Some(parent) = self.state_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&self.state_file, content)?;

        Ok(())
    }

    /// Record that we just checked for updates
    pub fn record_check(&mut self) {
        self.last_check = Some(Utc::now());
        let _ = self.save();
    }

    /// Record a crash
    pub fn record_crash(&mut self) {
        self.crash_count += 1;
        self.last_crash_time = Some(Utc::now());
        let _ = self.save();
    }

    /// Reset crash counter (after successful run)
    pub fn reset_crashes(&mut self) {
        if self.crash_count > 0 {
            self.crash_count = 0;
            self.last_crash_time = None;
            let _ = self.save();
        }
    }

    /// Check if we should rollback due to too many crashes
    pub fn should_rollback(&self) -> bool {
        self.crash_count >= MAX_CRASHES && self.previous_version.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_state_load_default() {
        let dir = tempdir().unwrap();
        let state = UpdateState::load(dir.path()).unwrap();
        assert!(state.last_check.is_none());
        assert_eq!(state.crash_count, 0);
    }

    #[test]
    fn test_state_save_load() {
        let dir = tempdir().unwrap();
        let mut state = UpdateState::load(dir.path()).unwrap();

        state.installed_version = "1.0.0".to_string();
        state.crash_count = 2;
        state.save().unwrap();

        let loaded = UpdateState::load(dir.path()).unwrap();
        assert_eq!(loaded.installed_version, "1.0.0");
        assert_eq!(loaded.crash_count, 2);
    }

    #[test]
    fn test_should_rollback() {
        let mut state = UpdateState::default();

        // No rollback without previous version
        state.crash_count = 5;
        assert!(!state.should_rollback());

        // Rollback with previous version and enough crashes
        state.previous_version = Some("0.9.0".to_string());
        assert!(state.should_rollback());

        // No rollback below threshold
        state.crash_count = 2;
        assert!(!state.should_rollback());
    }
}
