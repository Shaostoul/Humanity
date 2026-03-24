//! Auto-updater: checks GitHub Releases for new versions, notifies user,
//! downloads and replaces binary. Never forces updates.
//!
//! Features:
//! - Check for updates on launch (configurable interval)
//! - Notify via egui toast, never force
//! - "Always latest" (default) or pin to a specific version
//! - Rollback to any previous release
//! - Downloads in background thread, shows progress

#[cfg(feature = "native")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "native")]
use std::path::PathBuf;

/// Update channel preference stored in settings.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateChannel {
    /// Always download the latest release (default).
    AlwaysLatest,
    /// Pin to a specific version tag (e.g., "v0.40.0").
    Pinned(String),
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
        published_at: String,
    },
    /// Currently downloading the update.
    Downloading { version: String, progress: f32 },
    /// Download complete, ready to install on next launch.
    Ready { version: String, path: PathBuf },
    /// Already on the latest (or pinned) version.
    UpToDate,
    /// Something went wrong.
    Error(String),
}

/// A release entry from GitHub.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: String,
    pub assets: Vec<ReleaseAsset>,
    pub prerelease: bool,
    pub draft: bool,
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
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
enum UpdateMsg {
    Releases(Vec<ReleaseInfo>),
    DownloadProgress(f32),
    DownloadComplete(PathBuf),
    Error(String),
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
        let release = self.available_releases.iter().find(|r| r.tag_name == version);
        let release = match release {
            Some(r) => r.clone(),
            None => {
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
            match download_binary(&asset.browser_download_url, &ver, &tx) {
                Ok(path) => {
                    let _ = tx.send(UpdateMsg::DownloadComplete(path));
                }
                Err(e) => {
                    let _ = tx.send(UpdateMsg::Error(e));
                }
            }
        });
    }

    /// Poll for background thread results. Call once per frame.
    pub fn poll(&mut self, dt: f64) {
        self.last_check_elapsed += dt;

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
                    self.evaluate_update();
                }
                UpdateMsg::DownloadProgress(p) => {
                    if let UpdateState::Downloading { ref version, .. } = self.state {
                        self.state = UpdateState::Downloading {
                            version: version.clone(),
                            progress: p,
                        };
                    }
                }
                UpdateMsg::DownloadComplete(path) => {
                    let ver = if let UpdateState::Downloading { ref version, .. } = self.state {
                        version.clone()
                    } else {
                        "unknown".to_string()
                    };
                    self.state = UpdateState::Ready {
                        version: ver,
                        path,
                    };
                }
                UpdateMsg::Error(e) => {
                    self.state = UpdateState::Error(e);
                }
            }
        }
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
            UpdateChannel::Pinned(v) => Some(v.clone()),
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
        if target == current {
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
                    published_at: release.published_at.clone(),
                };
            } else {
                self.state = UpdateState::Error("No binary for this platform".to_string());
            }
        } else {
            self.state = UpdateState::Error(format!("Version {} not found in releases", target));
        }
    }

    /// Get list of all available versions for the version picker UI.
    pub fn available_versions(&self) -> Vec<(String, String, bool)> {
        self.available_releases
            .iter()
            .filter(|r| !r.draft)
            .map(|r| {
                let is_current = format!("v{}", self.current_version) == r.tag_name;
                (r.tag_name.clone(), r.published_at.clone(), is_current)
            })
            .collect()
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

/// Download a binary to a temporary location.
#[cfg(feature = "native")]
fn download_binary(
    url: &str,
    version: &str,
    tx: &std::sync::mpsc::Sender<UpdateMsg>,
) -> Result<PathBuf, String> {
    let response = ureq::get(url)
        .set("User-Agent", "HumanityOS-Updater")
        .call()
        .map_err(|e| format!("Download error: {}", e))?;

    let total_size = response
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // Download to a temp file next to the current binary
    let exe_path = std::env::current_exe().map_err(|e| format!("Can't find exe: {}", e))?;
    let parent = exe_path.parent().unwrap_or(std::path::Path::new("."));
    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let update_path = parent.join(format!("humanity-engine-{}{}", version, ext));

    let mut reader = response.into_reader();
    let mut file =
        std::fs::File::create(&update_path).map_err(|e| format!("File create error: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buf)
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

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&update_path, std::fs::Permissions::from_mode(0o755));
    }

    Ok(update_path)
}
