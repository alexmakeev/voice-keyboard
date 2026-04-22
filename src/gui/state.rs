//! Shared application state for GUI
//!
//! This state is shared between the GUI thread and background worker threads.

use crate::config::{Config, UpdateChannel};
use std::path::PathBuf;

/// Application status for tray icon color
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppStatus {
    /// Red: Not configured (no API key and no Whisper model)
    #[default]
    NotConfigured,
    /// Orange: Configured but connection failed or testing
    NoConnection,
    /// Green: Ready to transcribe
    Ready,
}

/// Main application state shared across GUI and worker threads
#[derive(Debug)]
pub struct AppState {
    /// Current app status (affects tray icon color)
    pub status: AppStatus,

    /// OpenAI API key
    pub api_key: String,

    /// OpenAI API URL (e.g., https://api.openai.com/v1 or proxy URL)
    pub api_url: String,

    /// Current hotkey configuration
    pub hotkey_type: String,

    /// Text input method (keyboard or clipboard)
    pub input_method: InputMethod,

    /// Sound volume (0.0 - 1.0)
    pub volume: f32,

    /// Enable extra hotkeys (Right Cmd, Right Option)
    pub extra_keys_enabled: bool,

    /// Whisper offline settings
    pub whisper_offline: WhisperOfflineState,

    /// Auto-update enabled
    pub auto_update: bool,

    /// Update channel (stable or beta)
    pub update_channel: UpdateChannel,

    /// Status message to display
    pub status_message: String,

    /// Last transcription text
    pub last_transcription: String,

    /// Config file path
    pub config_path: PathBuf,

    /// Flag indicating config has unsaved changes
    pub has_unsaved_changes: bool,
}

/// Text input method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMethod {
    Keyboard,
    Clipboard,
}

impl Default for InputMethod {
    fn default() -> Self {
        Self::Keyboard
    }
}

impl InputMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Clipboard => "clipboard",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "clipboard" => Self::Clipboard,
            _ => Self::Keyboard,
        }
    }
}

/// Whisper offline/fallback settings
#[derive(Debug, Clone)]
pub struct WhisperOfflineState {
    /// Use Whisper as fallback when API fails
    pub enabled: bool,

    /// Use Whisper as primary (instead of OpenAI)
    pub use_as_primary: bool,

    /// Selected model name
    pub model_name: String,

    /// Model download progress (None if not downloading)
    pub download_progress: Option<f32>,

    /// Downloaded models
    pub downloaded_models: Vec<String>,
}

impl Default for WhisperOfflineState {
    fn default() -> Self {
        Self {
            enabled: false,
            use_as_primary: false,
            model_name: "large-v3-turbo".to_string(),
            download_progress: None,
            downloaded_models: Vec::new(),
        }
    }
}

impl AppState {
    /// Create state from config
    pub fn from_config(config: Config) -> Self {
        let config_path = Config::config_path().unwrap_or_else(|_| PathBuf::from("config.json"));

        // Load API key: config takes priority, then env var
        let api_key = config
            .openai_api_key
            .clone()
            .unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());

        // Load API URL: config takes priority, then env var, then default
        let api_url = config
            .openai_api_url
            .clone()
            .filter(|url| !url.is_empty())
            .or_else(|| std::env::var("OPENAI_API_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        // Parse input method from config
        let input_method = match &config.injection_method {
            crate::config::InjectionMethodConfig::Keyboard => InputMethod::Keyboard,
            crate::config::InjectionMethodConfig::Clipboard => InputMethod::Clipboard,
            crate::config::InjectionMethodConfig::ClipboardOnly => InputMethod::Clipboard,
        };

        // Get hotkey from config
        let hotkey_type = config.hotkey.trigger_key.clone();

        // Check downloaded models
        let downloaded_models = get_downloaded_models();

        // Determine initial status
        let has_api_key = !api_key.is_empty();
        let has_whisper_model = !downloaded_models.is_empty();
        let status = if has_api_key || has_whisper_model {
            AppStatus::Ready // Configured, will test connection later
        } else {
            AppStatus::NotConfigured // Red - needs setup
        };

        let status_message = match status {
            AppStatus::NotConfigured => {
                "Not configured - enter API key or download Whisper model".to_string()
            }
            AppStatus::NoConnection => "Connection failed".to_string(),
            AppStatus::Ready => "Ready".to_string(),
        };

        Self {
            status,
            api_key,
            api_url,
            hotkey_type,
            input_method,
            volume: if config.play_sounds { 0.5 } else { 0.0 },
            extra_keys_enabled: config.extra_keys_enabled,
            whisper_offline: WhisperOfflineState {
                downloaded_models,
                ..Default::default()
            },
            auto_update: config.auto_update,
            update_channel: config.update_channel,
            status_message,
            last_transcription: String::new(),
            config_path,
            has_unsaved_changes: false,
        }
    }

    /// Save current state to config file
    pub fn save_to_config(&self) -> anyhow::Result<()> {
        use crate::config::{Config, HotkeyConfigSerde, InjectionMethodConfig};

        let config = Config {
            openai_api_key: if self.api_key.is_empty() {
                None
            } else {
                Some(self.api_key.clone())
            },
            openai_api_url: if self.api_url.is_empty() {
                None
            } else {
                Some(self.api_url.clone())
            },
            model_path: PathBuf::new(), // TODO: get from whisper settings
            model_size: crate::config::ModelSizeConfig::LargeV3Turbo,
            language: "auto".to_string(),
            hotkey: HotkeyConfigSerde {
                trigger_key: self.hotkey_type.clone(),
                push_to_talk: true,
                modifiers: vec![],
            },
            injection_method: match self.input_method {
                InputMethod::Keyboard => InjectionMethodConfig::Keyboard,
                InputMethod::Clipboard => InjectionMethodConfig::Clipboard,
            },
            play_sounds: self.volume > 0.0,
            show_notifications: true,
            auto_update: self.auto_update,
            update_channel: self.update_channel,
            update_url: None,
            extra_keys_enabled: self.extra_keys_enabled,
            ..Default::default()
        };

        config.save()?;

        Ok(())
    }
}

/// Get list of downloaded Whisper models
fn get_downloaded_models() -> Vec<String> {
    let models_dir = directories::ProjectDirs::from("com", "alexmak", "voice-keyboard")
        .map(|dirs| dirs.data_dir().join("models"))
        .unwrap_or_else(|| PathBuf::from("models"));

    if !models_dir.exists() {
        return Vec::new();
    }

    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_stem() {
                let name = name.to_string_lossy();
                // Match model file patterns like "ggml-large-v3-turbo.bin"
                if name.starts_with("ggml-") && path.extension().is_some_and(|e| e == "bin") {
                    let model_name = name.strip_prefix("ggml-").unwrap_or(&name).to_string();
                    models.push(model_name);
                }
            }
        }
    }

    models
}
