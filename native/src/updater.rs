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
    /// Path to the exe captured at startup (before any renames).
    /// Used for restart after update.
    pub exe_path: PathBuf,
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
            exe_path: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("HumanityOS.exe")),
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

/// Minimum acceptable binary size (1 MB). Anything smaller is almost certainly
/// a corrupt or incomplete download.
#[cfg(feature = "native")]
const MIN_BINARY_SIZE: u64 = 1_048_576;

/// Download a binary and replace the running executable.
/// Reports progress via the channel. On success, the current exe has been
/// swapped out and a restart will launch the new version.
#[cfg(feature = "native")]
fn download_and_apply(
    url: &str,
    _version: &str,
    tx: &std::sync::mpsc::Sender<UpdateMsg>,
) -> Result<(), String> {
    crate::debug::push_debug(format!("Updater: starting download from {}", url));

    let response = ureq::get(url)
        .set("User-Agent", "HumanityOS-Updater")
        .call()
        .map_err(|e| {
            crate::debug::push_debug(format!("Updater: download HTTP error: {}", e));
            format!("Download error: {}", e)
        })?;

    let total_size = response
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    crate::debug::push_debug(format!("Updater: Content-Length = {} bytes", total_size));

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

    crate::debug::push_debug(format!("Updater: downloading to {}", update_path.display()));

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

    // Pre-download verification: ensure the file exists and is large enough
    let update_size = std::fs::metadata(&update_path)
        .map(|m| m.len())
        .unwrap_or(0);
    crate::debug::push_debug(format!(
        "Updater: download complete, file size = {} bytes",
        update_size
    ));

    if update_size < MIN_BINARY_SIZE {
        let msg = format!(
            "Downloaded file too small ({} bytes, expected > {} bytes). Aborting update.",
            update_size, MIN_BINARY_SIZE
        );
        crate::debug::push_debug(format!("Updater: {}", msg));
        let _ = std::fs::remove_file(&update_path);
        return Err(msg);
    }

    // Apply the update by replacing the running executable
    apply_update(&exe_path, &update_path)?;

    Ok(())
}

/// Replace the running executable with the downloaded update.
#[cfg(all(feature = "native", target_os = "windows"))]
fn apply_update(exe_path: &std::path::Path, update_path: &std::path::Path) -> Result<(), String> {
    // On Windows, a running exe can be renamed but not deleted.
    // 1. Write restart_target.txt BEFORE any renames (so we know the target path)
    // 2. Rename current exe to .exe.old
    // 3. Rename .update to the original exe name
    // 4. Verify the new binary exists and is large enough
    // The .old file will be cleaned up on next launch.
    let old_path = exe_path.with_extension("exe.old");

    crate::debug::push_debug(format!("Updater: exe_path = {}", exe_path.display()));
    crate::debug::push_debug(format!("Updater: update_path = {}", update_path.display()));
    crate::debug::push_debug(format!("Updater: old_path = {}", old_path.display()));

    let update_size = std::fs::metadata(update_path).map(|m| m.len()).unwrap_or(0);
    crate::debug::push_debug(format!("Updater: update file size = {} bytes", update_size));
    log::info!("Updater: exe_path = {}", exe_path.display());
    log::info!("Updater: update_path = {}", update_path.display());
    log::info!("Updater: old_path = {}", old_path.display());
    log::info!("Updater: update file size = {} bytes", update_size);

    // Write restart_target.txt BEFORE any renames so we always know where the
    // new binary should end up, regardless of what current_exe() returns later.
    let parent = exe_path.parent().unwrap_or(std::path::Path::new("."));
    let restart_target_path = parent.join("restart_target.txt");
    if let Err(e) = std::fs::write(&restart_target_path, exe_path.to_string_lossy().as_bytes()) {
        crate::debug::push_debug(format!("Updater: WARNING could not write restart_target.txt: {}", e));
        log::warn!("Updater: could not write restart_target.txt: {}", e);
    } else {
        crate::debug::push_debug(format!("Updater: wrote restart_target.txt -> {}", exe_path.display()));
    }

    // Remove any existing .old file first
    if old_path.exists() {
        crate::debug::push_debug("Updater: removing existing .old file");
        log::info!("Updater: removing existing .old file");
        let _ = std::fs::remove_file(&old_path);
    }

    crate::debug::push_debug("Updater: renaming running exe to .old");
    log::info!("Updater: renaming running exe to .old");
    std::fs::rename(exe_path, &old_path)
        .map_err(|e| {
            let msg = format!("Failed to rename current exe to .old: {}", e);
            crate::debug::push_debug(format!("Updater: {}", msg));
            msg
        })?;

    crate::debug::push_debug("Updater: renaming .update to exe path");
    log::info!("Updater: renaming .update to exe path");
    if let Err(e) = std::fs::rename(update_path, exe_path) {
        // Try to restore the original if the swap failed
        let msg = format!("Failed to install update binary: {}", e);
        crate::debug::push_debug(format!("Updater: swap failed, restoring original: {}", e));
        log::error!("Updater: swap failed, restoring original: {}", e);
        let _ = std::fs::rename(&old_path, exe_path);
        // Clean up restart_target.txt on failure
        let _ = std::fs::remove_file(&restart_target_path);
        return Err(msg);
    }

    // Post-swap verification: confirm the new binary exists at exe_path
    // and has a reasonable file size (> 1 MB)
    let new_size = std::fs::metadata(exe_path).map(|m| m.len()).unwrap_or(0);
    crate::debug::push_debug(format!(
        "Updater: post-swap verification, new binary size = {} bytes",
        new_size
    ));

    if new_size < MIN_BINARY_SIZE {
        // Roll back: restore the old binary
        crate::debug::push_debug(format!(
            "Updater: ROLLBACK. New binary too small ({} bytes). Restoring old binary.",
            new_size
        ));
        log::error!("Updater: new binary too small ({} bytes), rolling back", new_size);
        let _ = std::fs::remove_file(exe_path);
        let _ = std::fs::rename(&old_path, exe_path);
        let _ = std::fs::remove_file(&restart_target_path);
        return Err(format!(
            "Update verification failed: new binary is {} bytes (expected > {} bytes). Rolled back.",
            new_size, MIN_BINARY_SIZE
        ));
    }

    crate::debug::push_debug(format!(
        "Updater: SUCCESS. New binary at {} ({} bytes), old at {}",
        exe_path.display(),
        new_size,
        old_path.display()
    ));
    log::info!(
        "Updater: SUCCESS. New binary at {} ({} bytes), old at {}",
        exe_path.display(),
        new_size,
        old_path.display()
    );
    Ok(())
}

/// Create a batch script that waits for this process to exit, then launches
/// the new exe. The script deletes itself after running.
#[cfg(all(feature = "native", target_os = "windows"))]
pub fn create_restart_script(exe_path: &std::path::Path) -> Result<PathBuf, String> {
    let bat_path = exe_path.with_extension("restart.bat");
    let exe_str = exe_path.to_string_lossy();
    let script = format!(
        "@echo off\r\ntimeout /t 2 /nobreak >nul\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        exe_str
    );
    std::fs::write(&bat_path, script)
        .map_err(|e| format!("Failed to write restart script: {}", e))?;
    crate::debug::push_debug(format!("Updater: created restart script at {}", bat_path.display()));
    Ok(bat_path)
}

/// Read the restart target path from restart_target.txt (written before the
/// binary swap). Falls back to the provided exe_path if the file is missing.
#[cfg(feature = "native")]
pub fn read_restart_target(exe_path: &std::path::Path) -> PathBuf {
    let parent = exe_path.parent().unwrap_or(std::path::Path::new("."));
    let target_file = parent.join("restart_target.txt");
    if let Ok(contents) = std::fs::read_to_string(&target_file) {
        let target = contents.trim().to_string();
        if !target.is_empty() {
            crate::debug::push_debug(format!("Updater: restart target from file: {}", target));
            return PathBuf::from(target);
        }
    }
    crate::debug::push_debug(format!(
        "Updater: no restart_target.txt found, using exe_path: {}",
        exe_path.display()
    ));
    exe_path.to_path_buf()
}

/// Replace the running executable with the downloaded update (Unix).
#[cfg(all(feature = "native", not(target_os = "windows")))]
fn apply_update(exe_path: &std::path::Path, update_path: &std::path::Path) -> Result<(), String> {
    crate::debug::push_debug(format!("Updater: applying update (Unix) to {}", exe_path.display()));

    // On Unix, we can replace the exe directly.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(update_path, std::fs::Permissions::from_mode(0o755));
    }

    std::fs::rename(update_path, exe_path)
        .map_err(|e| {
            crate::debug::push_debug(format!("Updater: Unix rename failed: {}", e));
            format!("Failed to replace executable: {}", e)
        })?;

    // Post-swap verification
    let new_size = std::fs::metadata(exe_path).map(|m| m.len()).unwrap_or(0);
    crate::debug::push_debug(format!(
        "Updater: post-swap verification, new binary size = {} bytes",
        new_size
    ));

    if new_size < MIN_BINARY_SIZE {
        let msg = format!(
            "Update verification failed: new binary is {} bytes (expected > {} bytes)",
            new_size, MIN_BINARY_SIZE
        );
        crate::debug::push_debug(format!("Updater: {}", msg));
        return Err(msg);
    }

    crate::debug::push_debug(format!(
        "Updater: SUCCESS. Update applied to {} ({} bytes)",
        exe_path.display(),
        new_size
    ));
    log::info!("Update applied to {} ({} bytes)", exe_path.display(), new_size);
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
