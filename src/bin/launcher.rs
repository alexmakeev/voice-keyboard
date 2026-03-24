//! Voice Keyboard Launcher
//!
//! This is the entry point for auto-update functionality.
//! It checks for updates, downloads and installs them, then launches the main voice-typer binary.
//! It also monitors for crashes and handles automatic rollback.

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};
use voice_keyboard::config::Config;
use voice_keyboard::updater::{UpdateLogger, UpdateState, Updater};

/// Crash timeout - if app exits with error within this time, it's considered a crash
const CRASH_TIMEOUT: Duration = Duration::from_secs(30);

fn main() -> ExitCode {
    // Get data directory
    let data_dir = match Config::data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to get data directory: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Setup logging
    let log_path = data_dir.join("logs.txt");
    let logger = match UpdateLogger::new(log_path.clone()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to setup logging: {}", e);
            return ExitCode::FAILURE;
        }
    };

    logger.log(&format!("Launcher started v{}", env!("APP_VERSION")));

    // Load config
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            logger.log(&format!("Failed to load config, using defaults: {}", e));
            Config::default()
        }
    };

    // Load update state
    let mut state = match UpdateState::load(&data_dir) {
        Ok(s) => s,
        Err(e) => {
            logger.log(&format!("Failed to load update state: {}", e));
            UpdateState::default()
        }
    };

    // Check if we need to rollback due to too many crashes
    if state.should_rollback() {
        logger.log("Too many crashes detected, rolling back to previous version");

        let updater = match Updater::new(&config, data_dir.clone()) {
            Ok(u) => u,
            Err(e) => {
                logger.log(&format!("Failed to create updater: {}", e));
                return launch_and_monitor(&config, &data_dir, &mut state, &logger);
            }
        };

        if let Err(e) = updater.rollback(&mut state) {
            logger.log(&format!("Rollback failed: {}", e));
        }
    }

    // Check for updates if enabled
    if config.auto_update {
        let updater = match Updater::new(&config, data_dir.clone()) {
            Ok(u) => u,
            Err(e) => {
                logger.log(&format!("Failed to create updater: {}", e));
                return launch_and_monitor(&config, &data_dir, &mut state, &logger);
            }
        };

        if updater.should_check_for_update(&state) {
            match check_and_install_update(&updater, &mut state, &logger) {
                Ok(_) => {
                    state.record_check();
                }
                Err(e) => {
                    logger.log(&format!("Update check failed: {}", e));
                }
            }
        }
    }

    // Launch the main application
    launch_and_monitor(&config, &data_dir, &mut state, &logger)
}

/// Check for updates and install if available
fn check_and_install_update(
    updater: &Updater,
    state: &mut UpdateState,
    logger: &UpdateLogger,
) -> Result<()> {
    let release = updater.check_for_update()?;

    if let Some(release) = release {
        logger.log(&format!("Installing update v{}", release.version));
        updater.download_and_install(&release, state)?;
    }

    Ok(())
}

/// Launch the main application and monitor for crashes
fn launch_and_monitor(
    config: &Config,
    data_dir: &PathBuf,
    state: &mut UpdateState,
    logger: &UpdateLogger,
) -> ExitCode {
    let core_binary = find_core_binary(data_dir);

    if !core_binary.exists() {
        logger.log(&format!(
            "Core binary not found at: {}",
            core_binary.display()
        ));
        logger.log("Please install voice-typer first or run with --install");
        return ExitCode::FAILURE;
    }

    // Build arguments for the core app
    let mut args: Vec<String> = env::args().skip(1).collect();

    // Default: launch GUI mode (user can start voice capture from there)
    // Only go to CLI mode if explicitly requested via --cli flag
    if args.is_empty() {
        logger.log("Starting GUI mode");
        // No args = GUI mode (default)
    } else if args.contains(&"--cli".to_string()) {
        // CLI mode requested explicitly
        if !args.contains(&"--openai".to_string()) {
            // Add --openai if API key is available
            if config.openai_api_key.is_some() || std::env::var("OPENAI_API_KEY").is_ok() {
                args.push("--openai".to_string());
            }
        }
        // Pass extra keys setting if enabled and not already set
        if config.extra_keys_enabled && !args.contains(&"--extra-keys".to_string()) {
            args.push("--extra-keys".to_string());
        }
        logger.log(&format!("Starting CLI mode with args: {:?}", args));
    }

    logger.log(&format!(
        "Launching core app: {} {}",
        core_binary.display(),
        args.join(" ")
    ));

    let start = Instant::now();

    let mut child = match Command::new(&core_binary).args(&args).spawn() {
        Ok(c) => c,
        Err(e) => {
            logger.log(&format!("Failed to launch core app: {}", e));
            return ExitCode::FAILURE;
        }
    };

    // Wait for the process to exit
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            logger.log(&format!("Failed to wait for core app: {}", e));
            return ExitCode::FAILURE;
        }
    };

    let elapsed = start.elapsed();

    // Handle exit status
    let exit_code = status.code().unwrap_or(-1);

    if status.success() {
        logger.log(&format!("Core app exited successfully after {:?}", elapsed));
        state.reset_crashes();
        ExitCode::SUCCESS
    } else if elapsed < CRASH_TIMEOUT {
        // Quick exit with error = crash
        logger.log(&format!(
            "Core app crashed after {:?} with exit code {}",
            elapsed, exit_code
        ));
        state.record_crash();

        if state.should_rollback() {
            logger.log(&format!(
                "Crash limit reached ({} crashes). Will rollback on next launch.",
                state.crash_count
            ));
        }

        ExitCode::from(exit_code as u8)
    } else {
        // Long-running before error = normal exit with error
        logger.log(&format!(
            "Core app exited with code {} after {:?}",
            exit_code, elapsed
        ));
        state.reset_crashes();
        ExitCode::from(exit_code as u8)
    }
}

/// Find the core binary path
fn find_core_binary(data_dir: &PathBuf) -> PathBuf {
    // First check data directory
    let data_binary = if cfg!(target_os = "windows") {
        data_dir.join("voice-typer.exe")
    } else {
        data_dir.join("voice-typer")
    };

    if data_binary.exists() {
        return data_binary;
    }

    // Then check same directory as launcher
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling_binary = if cfg!(target_os = "windows") {
                parent.join("voice-typer.exe")
            } else {
                parent.join("voice-typer")
            };

            if sibling_binary.exists() {
                return sibling_binary;
            }
        }
    }

    // Default to data directory path
    data_binary
}
