//! Auto-updater: checks GitHub Releases for new versions, notifies user,
//! downloads and replaces binary. Never forces updates.
//!
//! Features:
//! - Check for updates on launch (configurable interval)
//! - Notify via egui toast, never force
//! - "Always latest" (default) or disabled
//! - Version picker: install any previous release
//! - Downloads in background thread, shows progress
//! - Self-replace: renames running exe, swaps in new binary, prompts restart

#[cfg(feature = "native")]
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(feature = "native")]
use std::path::PathBuf;

/// Deserialize a JSON null as an empty string instead of failing.
#[cfg(feature = "native")]
fn deserialize_null_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// Update channel preference stored in settings.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UpdateChannel {
    /// Always download the latest release (default).
    AlwaysLatest,
    /// Never check for updates.
    Disabled,
}

#[cfg(feature = "native")]
impl Default for UpdateChannel {
    fn default() -> Self {
        UpdateChannel::AlwaysLatest
    }
}

/// Current state of the updater.
#[cfg(feature = "native")]
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    /// Haven't checked yet this session.
    Idle,
    /// Currently checking GitHub API.
    Checking,
    /// A new version is available.
    Available {
        version: String,
        download_url: String,
        release_notes: String,
        asset_size: u64,
    },
    /// Currently downloading the update.
    Downloading { version: String, progress: f32 },
    /// Download complete and applied. Restart required.
    Ready { version: String },
    /// Already on the latest version.
    UpToDate,
    /// Something went wrong.
    Error(String),
}

/// A release entry from GitHub.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub body: String,
    #[serde(default, deserialize_with = "deserialize_null_string")]
    pub published_at: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Messages sent from background threads back to the main thread.
#[cfg(feature = "native")]
enum UpdateMsg {
    Releases(Vec<ReleaseInfo>),
    DownloadProgress(f32),
    /// Download finished and binary was swapped in. Restart to use.
    Applied(String),
    Error(String),
}

/// The updater. Stores preferences and manages update lifecycle.
#[cfg(feature = "native")]
pub struct Updater {
    pub channel: UpdateChannel,
    pub state: UpdateState,
    pub current_version: String,
    pub available_releases: Vec<ReleaseInfo>,
    /// Check interval in seconds (default: 3600 = 1 hour).
    pub check_interval: f64,
    /// Seconds since last check.
    last_check_elapsed: f64,
    /// Receiver for background thread results.
    rx: Option<std::sync::mpsc::Receiver<UpdateMsg>>,
}

#[cfg(feature = "native")]
impl Updater {
    pub fn new(current_version: &str) -> Self {
        Self {
            channel: UpdateChannel::AlwaysLatest,
            state: UpdateState::Idle,
            current_version: current_version.to_string(),
            available_releases: Vec::new(),
            check_interval: 3600.0,
            last_check_elapsed: 0.0,
            rx: None,
        }
    }

    /// Check for updates (spawns background thread).
    pub fn check_now(&mut self) {
        if self.state == UpdateState::Checking {
            return;
        }
        self.state = UpdateState::Checking;
        self.last_check_elapsed = 0.0;

        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            match fetch_releases() {
                Ok(releases) => {
                    let _ = tx.send(UpdateMsg::Releases(releases));
                }
                Err(e) => {
                    let _ = tx.send(UpdateMsg::Error(e));
                }
            }
        });
    }

    /// Start downloading a specific version.
    pub fn download_version(&mut self, version: &str) {
        // Look up in available releases first
        let release = self.available_releases.iter().find(|r| r.tag_name == version);
        let release = match release {
            Some(r) => r.clone(),
            None => {
                // Fall back to the version/url from current Available state
                if let UpdateState::Available { version: ref av, download_url: ref aurl, .. } = self.state {
                    if av == version {
                        let url = aurl.clone();
                        let ver = version.to_string();
                        self.state = UpdateState::Downloading {
                            version: ver.clone(),
                            progress: 0.0,
                        };
                        let (tx, rx) = std::sync::mpsc::channel();
                        self.rx = Some(rx);
                        std::thread::spawn(move || {
                            match download_and_apply(&url, &ver, &tx) {
                                Ok(()) => {
                                    let _ = tx.send(UpdateMsg::Applied(ver));
                                }
                                Err(e) => {
                                    let _ = tx.send(UpdateMsg::Error(e));
                                }
                            }
                        });
                        return;
                    }
                }
                self.state = UpdateState::Error(format!("Version {} not found", version));
                return;
            }
        };

        let asset = match find_platform_asset(&release.assets) {
            Some(a) => a.clone(),
            None => {
                self.state = UpdateState::Error("No binary for this platform".to_string());
                return;
            }
        };

        self.state = UpdateState::Downloading {
            version: version.to_string(),
            progress: 0.0,
        };

        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        let ver = version.to_string();

        std::thread::spawn(move || {
            match download_and_apply(&asset.browser_download_url, &ver, &tx) {
                Ok(()) => {
                    let _ = tx.send(UpdateMsg::Applied(ver));
                }
                Err(e) => {
                    let _ = tx.send(UpdateMsg::Error(e));
                }
            }
        });
    }

    /// Poll for background thread results. Call once per frame.
    /// Returns true if an update just became available (for toast notifications).
    pub fn poll(&mut self, dt: f64) -> bool {
        self.last_check_elapsed += dt;
        let mut became_available = false;

        // Auto-check on interval
        if self.state == UpdateState::Idle || self.state == UpdateState::UpToDate {
            if self.last_check_elapsed >= self.check_interval
                && self.channel != UpdateChannel::Disabled
            {
                self.check_now();
            }
        }

        // Collect messages from background thread first, then process
        let mut messages = Vec::new();
        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }
        for msg in messages {
            match msg {
                UpdateMsg::Releases(releases) => {
                    self.available_releases = releases;
                    let was_available = matches!(&self.state, UpdateState::Available { .. });
                    self.evaluate_update();
                    if !was_available && matches!(&self.state, UpdateState::Available { .. }) {
                        became_available = true;
                    }
                }
                UpdateMsg::DownloadProgress(p) => {
                    if let UpdateState::Downloading { ref version, .. } = self.state {
                        self.state = UpdateState::Downloading {
                            version: version.clone(),
                            progress: p,
                        };
                    }
                }
                UpdateMsg::Applied(ver) => {
                    self.state = UpdateState::Ready { version: ver };
                }
                UpdateMsg::Error(e) => {
                    self.state = UpdateState::Error(e);
                }
            }
        }

        became_available
    }

    /// Compare available releases against current version and channel preference.
    fn evaluate_update(&mut self) {
        let target_version = match &self.channel {
            UpdateChannel::AlwaysLatest => {
                // Find the latest non-prerelease, non-draft release
                self.available_releases
                    .iter()
                    .find(|r| !r.prerelease && !r.draft)
                    .map(|r| r.tag_name.clone())
            }
            UpdateChannel::Disabled => {
                self.state = UpdateState::UpToDate;
                return;
            }
        };

        let target = match target_version {
            Some(v) => v,
            None => {
                self.state = UpdateState::UpToDate;
                return;
            }
        };

        let current = format!("v{}", self.current_version);
        let target_clean = target.trim_start_matches('v');
        if !is_newer(target_clean, &self.current_version) {
            self.state = UpdateState::UpToDate;
            return;
        }

        // Find the release info
        if let Some(release) = self.available_releases.iter().find(|r| r.tag_name == target) {
            if let Some(asset) = find_platform_asset(&release.assets) {
                self.state = UpdateState::Available {
                    version: target,
                    download_url: asset.browser_download_url.clone(),
                    release_notes: release.body.clone(),
                    asset_size: asset.size,
                };
            } else {
                self.state = UpdateState::Error("No binary for this platform".to_string());
            }
        } else {
            self.state = UpdateState::Error(format!("Version {} not found in releases", target));
        }
    }

    /// Get list of all available versions for the version picker UI.
    /// Returns: (tag, published_date, is_current).
    pub fn available_versions(&self) -> Vec<(String, String, bool)> {
        let current = format!("v{}", self.current_version);
        self.available_releases
            .iter()
            .filter(|r| !r.draft)
            .map(|r| {
                let is_current = current == r.tag_name;
                (r.tag_name.clone(), r.published_at.clone(), is_current)
            })
            .collect()
    }

    /// Clean up `.old` files from previous updates. Call once on startup.
    pub fn cleanup_old_versions() {
        if let Ok(exe_path) = std::env::current_exe() {
            // Try the double-extension variant used on Windows (e.g. "app.exe.old")
            let old1 = exe_path.with_extension("exe.old");
            if old1.exists() {
                match std::fs::remove_file(&old1) {
                    Ok(()) => log::info!("Cleaned up old update file: {}", old1.display()),
                    Err(e) => log::warn!("Failed to clean up {}: {}", old1.display(), e),
                }
            }
            // Also try the simple ".old" extension (e.g. "app.old" on Unix)
            let old2 = exe_path.with_extension("old");
            if old2.exists() && old2 != old1 {
                match std::fs::remove_file(&old2) {
                    Ok(()) => log::info!("Cleaned up old update file: {}", old2.display()),
                    Err(e) => log::warn!("Failed to clean up {}: {}", old2.display(), e),
                }
            }
        }
    }
}

/// Fetch releases from GitHub API.
#[cfg(feature = "native")]
fn fetch_releases() -> Result<Vec<ReleaseInfo>, String> {
    let url = "https://api.github.com/repos/Shaostoul/Humanity/releases?per_page=20";

    let response = ureq::get(url)
        .set("User-Agent", "HumanityOS-Updater")
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| format!("HTTP error: {}", e))?;

    let body = response
        .into_string()
        .map_err(|e| format!("Read error: {}", e))?;

    let releases: Vec<ReleaseInfo> =
        serde_json::from_str(&body).map_err(|e| format!("Parse error: {}", e))?;

    Ok(releases)
}

/// Find the correct binary asset for the current platform.
#[cfg(feature = "native")]
fn find_platform_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let platform_pattern = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos-arm64"
        } else {
            "macos-x64"
        }
    } else {
        "linux-x64"
    };

    assets
        .iter()
        .find(|a| a.name.to_lowercase().contains(platform_pattern))
}

/// Download a binary and replace the running executable.
/// Reports progress via the channel. On success, the current exe has been
/// swapped out and a restart will launch the new version.
#[cfg(feature = "native")]
fn download_and_apply(
    url: &str,
    _version: &str,
    tx: &std::sync::mpsc::Sender<UpdateMsg>,
) -> Result<(), String> {
    let response = ureq::get(url)
        .set("User-Agent", "HumanityOS-Updater")
        .call()
        .map_err(|e| format!("Download error: {}", e))?;

    let total_size = response
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let exe_path = std::env::current_exe().map_err(|e| format!("Can't find exe: {}", e))?;
    let parent = exe_path.parent().unwrap_or(std::path::Path::new("."));

    // Download to a .update temp file next to the running binary
    let update_path = parent.join(
        exe_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
            + ".update",
    );

    // Stream download in 64KB chunks with progress reporting
    let mut reader = response.into_reader();
    let mut file =
        std::fs::File::create(&update_path).map_err(|e| format!("File create error: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 65536];
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += n as u64;
        if total_size > 0 {
            let progress = downloaded as f32 / total_size as f32;
            let _ = tx.send(UpdateMsg::DownloadProgress(progress));
        }
    }
    drop(file);

    // Apply the update by replacing the running executable
    apply_update(&exe_path, &update_path)?;

    Ok(())
}

/// Replace the running executable with the downloaded update.
#[cfg(all(feature = "native", target_os = "windows"))]
fn apply_update(exe_path: &std::path::Path, update_path: &std::path::Path) -> Result<(), String> {
    // On Windows, a running exe can be renamed but not deleted.
    // 1. Rename current exe to .exe.old
    // 2. Rename .update to the original exe name
    // The .old file will be cleaned up on next launch.
    let old_path = exe_path.with_extension("exe.old");

    // Remove any existing .old file first
    let _ = std::fs::remove_file(&old_path);

    std::fs::rename(exe_path, &old_path)
        .map_err(|e| format!("Failed to rename current exe to .old: {}", e))?;

    if let Err(e) = std::fs::rename(update_path, exe_path) {
        // Try to restore the original if the swap failed
        let _ = std::fs::rename(&old_path, exe_path);
        return Err(format!("Failed to install update binary: {}", e));
    }

    log::info!(
        "Update applied: {} renamed to {}, new binary installed",
        exe_path.display(),
        old_path.display()
    );
    Ok(())
}

/// Replace the running executable with the downloaded update (Unix).
#[cfg(all(feature = "native", not(target_os = "windows")))]
fn apply_update(exe_path: &std::path::Path, update_path: &std::path::Path) -> Result<(), String> {
    // On Unix, we can replace the exe directly.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(update_path, std::fs::Permissions::from_mode(0o755));
    }

    std::fs::rename(update_path, exe_path)
        .map_err(|e| format!("Failed to replace executable: {}", e))?;

    log::info!("Update applied to {}", exe_path.display());
    Ok(())
}

/// Compare two semver strings. Returns true if `latest` is newer than `current`.
#[cfg(feature = "native")]
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };
    let l = parse(latest);
    let c = parse(current);
    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv > cv {
            return true;
        }
        if lv < cv {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.53.0", "0.52.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.52.1", "0.52.0"));
        assert!(!is_newer("0.52.0", "0.52.0"));
        assert!(!is_newer("0.51.0", "0.52.0"));
        assert!(!is_newer("0.52.0", "0.53.0"));
    }
}
