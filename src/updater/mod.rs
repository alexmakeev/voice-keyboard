//! Auto-update functionality for Voice Keyboard
//!
//! Provides update checking, downloading, installation, and rollback capabilities.
//! Uses GitHub Releases as the update source.

mod logger;
mod state;

pub use logger::UpdateLogger;
pub use state::UpdateState;

use crate::config::{Config, UpdateChannel};
use anyhow::{anyhow, Context, Result};
use semver::Version;
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

/// GitHub repository owner
const REPO_OWNER: &str = "alexmakeev";
/// GitHub repository name
const REPO_NAME: &str = "voice-keyboard";
/// Time between update checks (24 hours)
const UPDATE_CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// Release information from GitHub
#[derive(Debug, Clone)]
pub struct Release {
    pub tag_name: String,
    pub version: Version,
    pub prerelease: bool,
    pub assets: Vec<ReleaseAsset>,
}

/// Asset within a release
#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

/// Update manager
pub struct Updater {
    current_version: Version,
    channel: UpdateChannel,
    data_dir: PathBuf,
    update_url: Option<String>,
    logger: UpdateLogger,
}

impl Updater {
    /// Create new updater instance
    pub fn new(config: &Config, data_dir: PathBuf) -> Result<Self> {
        let current_version =
            Version::parse(env!("APP_VERSION")).context("Failed to parse current version")?;

        let logger = UpdateLogger::new(data_dir.join("logs.txt"))?;

        Ok(Self {
            current_version,
            channel: config.update_channel.clone(),
            data_dir,
            update_url: config.update_url.clone(),
            logger,
        })
    }

    /// Log a message
    pub fn log(&self, message: &str) {
        self.logger.log(message);
    }

    /// Check if enough time has passed since last update check
    pub fn should_check_for_update(&self, state: &UpdateState) -> bool {
        match state.last_check {
            Some(last) => {
                let elapsed = chrono::Utc::now().signed_duration_since(last).num_seconds() as u64;
                elapsed >= UPDATE_CHECK_INTERVAL_SECS
            }
            None => true,
        }
    }

    /// Check GitHub Releases for a newer version
    pub fn check_for_update(&self) -> Result<Option<Release>> {
        self.log("Checking for updates...");

        let releases = self.fetch_releases()?;

        // Filter by channel
        let releases: Vec<_> = releases
            .into_iter()
            .filter(|r| {
                if self.channel == UpdateChannel::Stable {
                    !r.prerelease
                } else {
                    true // Beta channel gets all releases
                }
            })
            .collect();

        // Find the latest release newer than current version
        for release in releases {
            if release.version > self.current_version {
                self.log(&format!("Found update: v{}", release.version));
                return Ok(Some(release));
            }
        }

        self.log("No updates available");
        Ok(None)
    }

    /// Fetch releases from GitHub API
    fn fetch_releases(&self) -> Result<Vec<Release>> {
        let url = self.update_url.clone().unwrap_or_else(|| {
            format!(
                "https://api.github.com/repos/{}/{}/releases",
                REPO_OWNER, REPO_NAME
            )
        });

        let client = reqwest::blocking::Client::builder()
            .user_agent("voice-keyboard-updater")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client.get(&url).send()?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().unwrap_or_default()
            ));
        }

        let json: serde_json::Value = response.json()?;
        let releases_arr = json
            .as_array()
            .ok_or_else(|| anyhow!("Invalid releases response"))?;

        let mut releases = Vec::new();
        for release_json in releases_arr {
            if let Ok(release) = self.parse_release(release_json) {
                releases.push(release);
            }
        }

        // Sort by version descending
        releases.sort_by(|a, b| b.version.cmp(&a.version));

        Ok(releases)
    }

    /// Parse a release from JSON
    fn parse_release(&self, json: &serde_json::Value) -> Result<Release> {
        let tag_name = json["tag_name"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing tag_name"))?
            .to_string();

        // Parse version from tag (e.g., "v0.1.0" -> "0.1.0")
        let version_str = tag_name.trim_start_matches('v');
        let version = Version::parse(version_str)?;

        let prerelease = json["prerelease"].as_bool().unwrap_or(false);

        let mut assets = Vec::new();
        if let Some(assets_arr) = json["assets"].as_array() {
            for asset_json in assets_arr {
                if let (Some(name), Some(url), Some(size)) = (
                    asset_json["name"].as_str(),
                    asset_json["browser_download_url"].as_str(),
                    asset_json["size"].as_u64(),
                ) {
                    assets.push(ReleaseAsset {
                        name: name.to_string(),
                        download_url: url.to_string(),
                        size,
                    });
                }
            }
        }

        Ok(Release {
            tag_name,
            version,
            prerelease,
            assets,
        })
    }

    /// Download and install an update
    pub fn download_and_install(&self, release: &Release, state: &mut UpdateState) -> Result<()> {
        // Find the appropriate asset for this platform
        let asset = self.find_platform_asset(&release.assets)?;

        self.log(&format!(
            "Downloading {} ({:.2} MB)",
            asset.name,
            asset.size as f64 / 1_000_000.0
        ));

        // Download to temp file
        let temp_dir = self.data_dir.join("temp");
        fs::create_dir_all(&temp_dir)?;
        let temp_archive = temp_dir.join(&asset.name);

        self.download_file(&asset.download_url, &temp_archive)?;

        // Verify SHA256 checksum if SHA256SUMS.txt is available in the release
        self.verify_asset_checksum(release, &asset.name, &temp_archive)?;

        // Extract binary
        let temp_binary = temp_dir.join(self.binary_name());
        self.extract_binary(&temp_archive, &temp_binary)?;

        // Backup current binary
        let core_binary = self.data_dir.join(self.binary_name());
        let backup_binary = self.data_dir.join(format!("{}.backup", self.binary_name()));

        if core_binary.exists() {
            self.log("Backing up current version");
            fs::copy(&core_binary, &backup_binary)?;
            state.previous_version = Some(self.current_version.to_string());
        }

        // Install new binary
        self.log(&format!("Installing v{}", release.version));

        #[cfg(target_os = "windows")]
        {
            // On Windows, use self_replace to handle file locking
            self_replace::self_replace(&temp_binary)?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            fs::copy(&temp_binary, &core_binary)?;

            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&core_binary)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&core_binary, perms)?;
            }
        }

        // Cleanup temp files
        let _ = fs::remove_file(&temp_archive);
        let _ = fs::remove_file(&temp_binary);
        let _ = fs::remove_dir(&temp_dir);

        // Update state
        state.installed_version = release.version.to_string();
        state.crash_count = 0;

        self.log(&format!("Successfully installed v{}", release.version));

        Ok(())
    }

    /// Find the appropriate asset for current platform
    fn find_platform_asset<'a>(&self, assets: &'a [ReleaseAsset]) -> Result<&'a ReleaseAsset> {
        let target = self.target_triple();

        // Look for matching asset
        for asset in assets {
            if asset.name.contains(&target) {
                return Ok(asset);
            }
        }

        Err(anyhow!(
            "No compatible release asset found for target: {}",
            target
        ))
    }

    /// Get target triple for current platform
    fn target_triple(&self) -> String {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return "aarch64-apple-darwin".to_string();

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return "x86_64-apple-darwin".to_string();

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        return "x86_64-unknown-linux-gnu".to_string();

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        return "x86_64-pc-windows-msvc".to_string();

        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        return "aarch64-pc-windows-msvc".to_string();

        #[cfg(not(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "aarch64"),
        )))]
        return "unknown".to_string();
    }

    /// Get binary name for current platform
    fn binary_name(&self) -> String {
        #[cfg(target_os = "windows")]
        return "voice-typer.exe".to_string();

        #[cfg(not(target_os = "windows"))]
        return "voice-typer".to_string();
    }

    /// Download a file from URL (streams to disk without buffering entire file in memory)
    fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("voice-keyboard-updater")
            .timeout(std::time::Duration::from_secs(300))
            .build()?;

        let mut response = client.get(url).send()?;

        if !response.status().is_success() {
            return Err(anyhow!("Download failed: {}", response.status()));
        }

        let mut file = File::create(dest)?;
        io::copy(&mut response, &mut file)?;

        Ok(())
    }

    /// Verify SHA256 checksum of a downloaded asset against SHA256SUMS.txt from the release.
    /// Returns an error if SHA256SUMS.txt is missing (fail-close: never install unverified binaries).
    fn verify_asset_checksum(
        &self,
        release: &Release,
        asset_name: &str,
        file_path: &Path,
    ) -> Result<()> {
        // Find SHA256SUMS.txt asset in the release
        let checksums_asset = release.assets.iter().find(|a| a.name == "SHA256SUMS.txt");

        let checksums_asset = match checksums_asset {
            Some(a) => a,
            None => {
                return Err(anyhow!(
                    "SHA256SUMS.txt not found in release — refusing to install unverified binary"
                ));
            }
        };

        self.log("Downloading SHA256SUMS.txt for verification");
        let checksums_content = self.download_text(&checksums_asset.download_url)?;

        // Parse expected hash for our asset
        let expected_hash = self.parse_checksum(&checksums_content, asset_name)?;

        // Compute actual hash
        let mut file = File::open(file_path)
            .with_context(|| format!("Failed to open {} for checksum", file_path.display()))?;
        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher)?;
        let actual_hash = format!("{:x}", hasher.finalize());

        if actual_hash != expected_hash {
            let _ = fs::remove_file(file_path);
            return Err(anyhow!(
                "Checksum verification failed for {}: expected {}, got {}",
                asset_name,
                expected_hash,
                actual_hash
            ));
        }

        self.log(&format!("Checksum verified for {}", asset_name));
        Ok(())
    }

    /// Download a URL and return its text content
    fn download_text(&self, url: &str) -> Result<String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("voice-keyboard-updater")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client.get(url).send()?;

        if !response.status().is_success() {
            return Err(anyhow!("Download failed: {}", response.status()));
        }

        Ok(response.text()?)
    }

    /// Parse SHA256SUMS.txt content and find the hash for a specific filename.
    /// Format: `<hash>  <filename>` (two spaces between hash and name).
    fn parse_checksum(&self, checksums_content: &str, filename: &str) -> Result<String> {
        for line in checksums_content.lines() {
            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.len() == 2 {
                let hash = parts[0].trim();
                let name = parts[1].trim();
                if name == filename {
                    return Ok(hash.to_lowercase());
                }
            }
        }

        Err(anyhow!("Asset '{}' not found in SHA256SUMS.txt", filename))
    }

    /// Extract binary from archive
    fn extract_binary(&self, archive_path: &Path, dest: &Path) -> Result<()> {
        let extension = archive_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension {
            "gz" => self.extract_tar_gz(archive_path, dest),
            "zip" => self.extract_zip(archive_path, dest),
            _ => Err(anyhow!("Unknown archive format: {}", extension)),
        }
    }

    /// Extract from .tar.gz archive
    fn extract_tar_gz(&self, archive_path: &Path, dest: &Path) -> Result<()> {
        let file = File::open(archive_path)?;
        let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
        let mut archive = tar::Archive::new(decoder);

        let binary_name = self.binary_name();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            if path.file_name().and_then(|n| n.to_str()) == Some(&binary_name) {
                let mut file = File::create(dest)?;
                io::copy(&mut entry, &mut file)?;
                return Ok(());
            }
        }

        Err(anyhow!("Binary not found in archive"))
    }

    /// Extract from .zip archive
    fn extract_zip(&self, archive_path: &Path, dest: &Path) -> Result<()> {
        let file = File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(BufReader::new(file))?;

        let binary_name = self.binary_name();

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();

            if name.ends_with(&binary_name) || name == binary_name {
                let mut file = File::create(dest)?;
                io::copy(&mut entry, &mut file)?;
                return Ok(());
            }
        }

        Err(anyhow!("Binary not found in archive"))
    }

    /// Rollback to previous version
    pub fn rollback(&self, state: &mut UpdateState) -> Result<()> {
        let core_binary = self.data_dir.join(self.binary_name());
        let backup_binary = self.data_dir.join(format!("{}.backup", self.binary_name()));

        if !backup_binary.exists() {
            return Err(anyhow!("No backup available for rollback"));
        }

        self.log("Rolling back to previous version");

        #[cfg(target_os = "windows")]
        {
            self_replace::self_replace(&backup_binary)?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            fs::copy(&backup_binary, &core_binary)?;
        }

        // Update state
        if let Some(prev) = &state.previous_version {
            state.installed_version = prev.clone();
        }
        state.previous_version = None;
        state.crash_count = 0;

        self.log("Rollback completed");

        Ok(())
    }

    /// Get the path to the core binary
    pub fn core_binary_path(&self) -> PathBuf {
        self.data_dir.join(self.binary_name())
    }
}
