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
//!
//! THE PHONE (2026-11-27, [`phone`]): the same check, against the same release feed, with the same
//! setting and the same timing ([`Updater::check_at_start`], [`Updater::poll`]), but it looks for
//! the release's APK and its checksum instead of a desktop binary ([`UpdateTarget`]), downloads and
//! checks it inside the app, and hands it to Android's installer instead of replacing an exe.
//!
//! THE OFFER (2026-11-27): a new version is offered on a line at the bottom of every app page, on a
//! desktop and a phone alike (gui/bottom_strip/update_offer.rs), never in a window that interrupts;
//! Settings > Updates shows the same words (gui/pages/settings/updates.rs). "Look for updates" is
//! kept with the other settings (config.rs `update_channel`) since that day; before, it went back
//! to Always latest at every start.

#[cfg(feature = "native")]
pub mod phone;

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

/// What this build updates itself with: a desktop replaces its exe with the release's binary for
/// its platform; the phone app installs the release's APK through Android's installer ([`phone`]).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTarget {
    Desktop,
    Phone,
}

#[cfg(feature = "native")]
impl UpdateTarget {
    /// The phone app on Android, a desktop everywhere else (the phone preview on a desktop included:
    /// it has no APK to install).
    pub fn this_build() -> Self {
        if cfg!(target_os = "android") {
            UpdateTarget::Phone
        } else {
            UpdateTarget::Desktop
        }
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
    /// Download complete and applied. Restart required. On the phone: downloaded and checked
    /// against its checksum, waiting for Install.
    Ready { version: String },
    /// Already on the latest version.
    UpToDate,
    /// The check went wrong (no network, GitHub's answer unreadable). It is tried again at the
    /// next hourly check.
    Error(String),
    /// A download the person started stopped (2026-11-27): `key` is the sentence they read
    /// (data/i18n/en.json `app_update.*`), `detail` the technical part, shown small and logged.
    /// The update line offers Try again.
    Failed { version: String, key: &'static str, detail: String },
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
    /// Download finished and binary was swapped in. Restart to use. On the phone: downloaded and
    /// checked.
    Applied(String),
    /// The check failed.
    Error(String),
    /// A download failed: the sentence's key and the technical detail ([`UpdateState::Failed`]).
    Failed(&'static str, String),
}

/// Wakes the frame when a background thread has news (2026-11-27): a phone draws only on change
/// (engine/redraw.rs), so the update line would otherwise wait for the next touch. The context is
/// handed over by the update line's first draw ([`Updater::set_waker`]); a thread started before
/// that (the check at start) wakes it once it is there.
#[cfg(feature = "native")]
#[derive(Clone, Default)]
struct Waker(std::sync::Arc<std::sync::Mutex<Option<egui::Context>>>);

#[cfg(feature = "native")]
impl Waker {
    fn wake(&self) {
        if let Some(ctx) = self.0.lock().ok().and_then(|c| c.clone()) {
            ctx.request_repaint();
        }
    }
}

/// A background thread's way back: each message sent also wakes the frame.
#[cfg(feature = "native")]
struct Reply {
    tx: std::sync::mpsc::Sender<UpdateMsg>,
    waker: Waker,
}

#[cfg(feature = "native")]
impl Reply {
    fn send(&self, msg: UpdateMsg) {
        let _ = self.tx.send(msg);
        self.waker.wake();
    }
}

/// Where the release list comes from: GitHub's release feed ([`fetch_releases`]); a test hands in
/// its own list so no test reaches the network.
#[cfg(feature = "native")]
type Feed = fn() -> Result<Vec<ReleaseInfo>, String>;

/// The updater. Stores preferences and manages update lifecycle.
#[cfg(feature = "native")]
pub struct Updater {
    pub channel: UpdateChannel,
    pub state: UpdateState,
    pub current_version: String,
    pub available_releases: Vec<ReleaseInfo>,
    /// Check interval in seconds (default: 3600 = 1 hour).
    pub check_interval: f64,
    /// When the last check started (or the updater was made). A clock, not a sum of frame times
    /// (2026-11-27): the frame's time step is capped for the simulation, and a phone draws only on
    /// change, so summing frames reached an hour far later than an hour.
    last_check: std::time::Instant,
    /// What this build installs: a desktop binary or the phone's APK.
    pub target: UpdateTarget,
    /// Where the release list comes from ([`Feed`]).
    feed: Feed,
    /// Wakes the frame when a background thread has news ([`Waker`]).
    waker: Waker,
    /// The offered version the person said Not now to: the update line hides it for this run
    /// (Settings > Updates still offers it). A newer offer shows again.
    offer_hidden: Option<String>,
    /// Receiver for background thread results.
    rx: Option<std::sync::mpsc::Receiver<UpdateMsg>>,
    /// Path to the exe captured at startup (before any renames).
    /// Used for restart after update.
    pub exe_path: PathBuf,
    /// This instance was launched behind the window in front (a script's launch, not a person's:
    /// engine/launch_focus.rs `launch_in_background`), so the restart script brings the new exe
    /// back behind too (`create_restart_script`). The engine fills it in at startup (lib.rs): the
    /// launch policy is the engine's, and this updater is the GUI crate's since the crate split's
    /// rung 0, part B (2026-10-31), so it is handed the answer instead of asking. False until then,
    /// a person's launch.
    pub relaunch_behind: bool,
    /// This instance's launcher ASKED for a quiet boot (a developer's script: engine/launch_focus.rs
    /// `quiet_asked`), so a restart after an update comes back quiet and hidden too; a background
    /// launch nobody asked to be quiet comes back visible (BUG-302). Filled in by the engine beside
    /// `relaunch_behind`, for the same reason: this crate does not reach the engine.
    pub relaunch_quiet: bool,
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
            last_check: std::time::Instant::now(),
            target: UpdateTarget::this_build(),
            feed: fetch_releases,
            waker: Waker::default(),
            offer_hidden: None,
            rx: None,
            exe_path: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("HumanityOS.exe")),
            relaunch_behind: false,
            relaunch_quiet: false,
        }
    }

    /// Check for updates (spawns background thread). Also Settings' Check now, which looks once
    /// even while looking is turned off.
    pub fn check_now(&mut self) {
        if self.state == UpdateState::Checking {
            return;
        }
        self.state = UpdateState::Checking;
        self.last_check = std::time::Instant::now();

        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        let reply = Reply { tx, waker: self.waker.clone() };
        let feed = self.feed;

        std::thread::spawn(move || match feed() {
            Ok(releases) => reply.send(UpdateMsg::Releases(releases)),
            Err(e) => reply.send(UpdateMsg::Error(e)),
        });
    }

    /// The check at start (engine_boot.rs): only while "Look for updates" is on. Turned off, the
    /// app never asks GitHub whether there is a new version, at start or later ([`Self::poll`]).
    pub fn check_at_start(&mut self) {
        if self.channel == UpdateChannel::AlwaysLatest {
            self.check_now();
        }
    }

    /// Hand over the context whose frame a background thread wakes ([`Waker`]). Cheap; the update
    /// line calls it as it draws.
    pub fn set_waker(&self, ctx: &egui::Context) {
        if let Ok(mut held) = self.waker.0.lock() {
            if held.is_none() {
                *held = Some(ctx.clone());
            }
        }
    }

    /// Start downloading a specific version.
    pub fn download_version(&mut self, version: &str) {
        if self.target == UpdateTarget::Phone {
            self.download_for_phone(version);
            return;
        }
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
                        // Degraded fallback path: we only have the binary URL,
                        // not the release's asset list, so no manifest/sig URLs.
                        // With signing provisioned this fails closed (safe);
                        // the normal path below supplies the manifest.
                        let (tx, rx) = std::sync::mpsc::channel();
                        self.rx = Some(rx);
                        let reply = Reply { tx, waker: self.waker.clone() };
                        std::thread::spawn(move || match download_and_apply(&url, &ver, None, None, &reply) {
                            Ok(()) => reply.send(UpdateMsg::Applied(ver)),
                            Err(e) => reply.send(UpdateMsg::Failed("app_update.failed_download", e)),
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
        // The signed manifest + signature ride as release assets; pass their
        // URLs so download_and_apply can verify the download before installing.
        let manifest_url = find_asset_url(&release.assets, MANIFEST_NAME);
        let sig_url = find_asset_url(&release.assets, MANIFEST_SIG_NAME);

        self.state = UpdateState::Downloading {
            version: version.to_string(),
            progress: 0.0,
        };

        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        let reply = Reply { tx, waker: self.waker.clone() };
        let ver = version.to_string();

        std::thread::spawn(move || match download_and_apply(&asset.browser_download_url, &ver, manifest_url, sig_url, &reply) {
            Ok(()) => reply.send(UpdateMsg::Applied(ver)),
            Err(e) => reply.send(UpdateMsg::Failed("app_update.failed_download", e)),
        });
    }

    /// The phone's download ([`phone::download_apk`]): the release's APK, checked against the
    /// checksum beside it, into the app's updates folder. Ready means downloaded and checked.
    fn download_for_phone(&mut self, version: &str) {
        let fail = |r: phone::Refusal| UpdateState::Failed { version: version.to_string(), key: r.words_key(), detail: r.detail() };
        let Some(release) = self.available_releases.iter().find(|r| r.tag_name == version) else {
            self.state = fail(phone::Refusal::Download(format!("{version} is not in the release list")));
            return;
        };
        let Some((apk, sum)) = phone::apk_assets(&release.assets) else {
            self.state = fail(phone::Refusal::NoChecksum(format!("{version} carries no {} and {}", phone::APK_NAME, phone::APK_SUM_NAME)));
            return;
        };
        let Some(folder) = phone::folder() else {
            self.state = fail(phone::Refusal::NotAPhone);
            return;
        };
        let (apk_url, sum_url) = (apk.browser_download_url.clone(), sum.browser_download_url.clone());
        self.state = UpdateState::Downloading { version: version.to_string(), progress: 0.0 };
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        let reply = Reply { tx, waker: self.waker.clone() };
        let ver = version.to_string();
        std::thread::spawn(move || {
            // The bar moves by whole percents: one message (and one frame) per percent, not per chunk.
            let mut shown = -1i32;
            let mut progress = |p: f32| {
                let pct = (p * 100.0) as i32;
                if pct != shown {
                    shown = pct;
                    reply.send(UpdateMsg::DownloadProgress(p));
                }
            };
            match phone::download_apk(&apk_url, &sum_url, &folder, &ver, &mut progress) {
                Ok(_) => reply.send(UpdateMsg::Applied(ver)),
                Err(r) => reply.send(UpdateMsg::Failed(r.words_key(), r.detail())),
            }
        });
    }

    /// The phone's Install: hand the checked download to Android's installer, which asks the person
    /// to confirm ([`phone::install`]). The state stays Ready, so a person who closed Android's
    /// question can tap Install again; a failure says why.
    pub fn install_downloaded(&mut self) {
        let UpdateState::Ready { version } = &self.state else { return };
        let version = version.clone();
        if let Err(r) = phone::install(&version) {
            log::warn!("Updater: the installer did not open for {version}: {}", r.detail());
            self.state = UpdateState::Failed { version, key: r.words_key(), detail: r.detail() };
        }
    }

    /// A desktop's Restart to apply (Settings > Updates and the update line): start the swapped-in
    /// exe once this one has closed; the caller then asks the app to quit.
    pub fn restart_into_update(&self) {
        // Read the restart target from restart_target.txt (written
        // before the binary swap) to get the correct exe path.
        let target = read_restart_target(&self.exe_path);
        humanity_core::debug::push_debug(format!("Updater: restart target = {}", target.display()));
        log::info!("Restarting from: {}", target.display());

        #[cfg(target_os = "windows")]
        {
            // Use a batch script to wait for this process to exit
            // before launching the new binary. This avoids the race
            // where the old process hasn't fully released the exe.
            match create_restart_script(&target, self.relaunch_behind, self.relaunch_quiet) {
                Ok(bat) => {
                    humanity_core::debug::push_debug(format!("Updater: launching restart script {}", bat.display()));
                    use std::os::windows::process::CommandExt;
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", &bat.to_string_lossy()])
                        .creation_flags(0x00000008) // DETACHED_PROCESS
                        .spawn();
                }
                Err(e) => {
                    // Fallback: try direct spawn if batch script fails
                    humanity_core::debug::push_debug(format!("Updater: batch script failed ({}), trying direct spawn", e));
                    log::warn!("Updater: batch script failed: {}", e);
                    let _ = std::process::Command::new(&target).spawn();
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new(&target).spawn();
        }
    }

    /// Not now on the update line: hide the offered version for this run.
    pub fn hide_offer(&mut self) {
        if let UpdateState::Available { version, .. } = &self.state {
            self.offer_hidden = Some(version.clone());
        }
    }

    /// Whether the update line shows: an offer the person has not hidden, and every step of a
    /// download they started (its progress, Install or Restart, a failure). A failed check does not
    /// show there (Settings says it): a phone offline at start is not news on every page.
    pub fn offer_shown(&self) -> bool {
        match &self.state {
            UpdateState::Available { version, .. } => self.offer_hidden.as_deref() != Some(version.as_str()),
            UpdateState::Downloading { .. } | UpdateState::Ready { .. } | UpdateState::Failed { .. } => true,
            UpdateState::Idle | UpdateState::Checking | UpdateState::UpToDate | UpdateState::Error(_) => false,
        }
    }

    /// Poll for background thread results. Call once per frame.
    /// Returns true if an update just became available (for toast notifications).
    pub fn poll(&mut self) -> bool {
        let mut became_available = false;

        // Auto-check on the interval (an hour), while "Look for updates" is on. A failed check is
        // tried again then too (2026-11-27): before, one failure (a phone offline at start) ended
        // the looking for the rest of the run.
        if matches!(self.state, UpdateState::Idle | UpdateState::UpToDate | UpdateState::Error(_))
            && self.last_check.elapsed().as_secs_f64() >= self.check_interval
            && self.channel != UpdateChannel::Disabled
        {
            self.check_now();
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
                UpdateMsg::Failed(key, detail) => {
                    let version = match &self.state {
                        UpdateState::Downloading { version, .. } => version.clone(),
                        _ => String::new(),
                    };
                    log::warn!("Updater: the download of {version} stopped: {detail}");
                    self.state = UpdateState::Failed { version, key, detail };
                }
            }
        }

        became_available
    }

    /// Compare available releases against current version and channel preference.
    fn evaluate_update(&mut self) {
        // Once the operator's release-signing keys are embedded, only ever
        // OFFER a release that carries a signed manifest asset — so an unsigned
        // release (a not-yet-signed publish, or a stray/malicious tag) is simply
        // invisible to auto-update rather than erroring. The download path still
        // verifies the signature as the authoritative gate (defense in depth).
        // Until provisioning, every release is eligible (legacy behaviour).
        //
        // The phone (2026-11-27) offers a release only when it carries the APK and its checksum
        // (phone::apk_assets): a desktop-only release, an asset release or one whose APK is not
        // attached yet offers nothing. The signed manifest lists the desktop binaries only, so it
        // is not asked for there; the APK is checked against its checksum and by Android's
        // same-key rule (phone.rs, "What protects what").
        let require_manifest = humanity_protocol::release_signature::embedded_pubkeys().is_provisioned();
        let target_kind = self.target;
        let eligible = |r: &ReleaseInfo| -> bool {
            !r.prerelease
                && !r.draft
                && match target_kind {
                    UpdateTarget::Desktop => !require_manifest || find_asset_url(&r.assets, MANIFEST_NAME).is_some(),
                    UpdateTarget::Phone => phone::apk_assets(&r.assets).is_some(),
                }
        };
        // "Look for updates" decides only whether the app looks by itself (check_at_start, poll):
        // a check made anyway (Check now, which looks once while it is off) offers what it found.
        // Before 2026-11-27 a check while it was off always said "up to date".
        let target_version = self
            .available_releases
            .iter()
            .find(|r| eligible(r))
            .map(|r| r.tag_name.clone());

        let target = match target_version {
            Some(v) => v,
            None => {
                self.state = UpdateState::UpToDate;
                return;
            }
        };

        let target_clean = target.trim_start_matches('v');
        if !is_newer(target_clean, &self.current_version) {
            self.state = UpdateState::UpToDate;
            return;
        }

        // Find the release info
        if let Some(release) = self.available_releases.iter().find(|r| r.tag_name == target) {
            let asset = match self.target {
                UpdateTarget::Desktop => find_platform_asset(&release.assets),
                UpdateTarget::Phone => phone::apk_assets(&release.assets).map(|(apk, _)| apk),
            };
            if let Some(asset) = asset {
                // A phone already holding this version's checked download (Android may have closed
                // the app while the person allowed installs) goes straight to Install.
                if self.target == UpdateTarget::Phone && phone::downloaded(&target).is_some() {
                    self.state = UpdateState::Ready { version: target };
                    return;
                }
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
        // The phone keeps its downloaded updates in its own folder: half-downloaded ones and those
        // not newer than this version go (phone::stale); its exe is not ours to touch.
        if UpdateTarget::this_build() == UpdateTarget::Phone {
            phone::clean_up(env!("CARGO_PKG_VERSION"));
            return;
        }
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

    parse_feed(&body)
}

/// GitHub's release list (the JSON the feed answers with) as releases.
#[cfg(feature = "native")]
fn parse_feed(body: &str) -> Result<Vec<ReleaseInfo>, String> {
    serde_json::from_str(body).map_err(|e| format!("Parse error: {}", e))
}

/// Stream `url` into the file `dest`, hashing the bytes as they pass (so a download is checked
/// without reading the file again), `progress` told the share done (0 to 1) when the size is known.
/// Returns the SHA-256 in lowercase hex. The desktop's download and the phone's both use it
/// (2026-11-27). A failure leaves whatever was written: the caller deletes it.
#[cfg(feature = "native")]
fn stream_to_file(url: &str, dest: &std::path::Path, progress: &mut dyn FnMut(f32)) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let response = ureq::get(url)
        .set("User-Agent", "HumanityOS-Updater")
        .call()
        .map_err(|e| {
            humanity_core::debug::push_debug(format!("Updater: download HTTP error: {}", e));
            format!("Download error: {}", e)
        })?;
    let total_size = response
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    humanity_core::debug::push_debug(format!("Updater: Content-Length = {} bytes", total_size));
    humanity_core::debug::push_debug(format!("Updater: downloading to {}", dest.display()));

    // Stream download in 64KB chunks with progress reporting
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("File create error: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut buf = vec![0u8; 65536];
    let mut hasher = Sha256::new();
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf).map_err(|e| format!("Read error: {}", e))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("Write error: {}", e))?;
        hasher.update(&buf[..n]);
        downloaded += n as u64;
        if total_size > 0 {
            progress(downloaded as f32 / total_size as f32);
        }
    }
    file.sync_all().map_err(|e| format!("Write error: {}", e))?;
    Ok(hex::encode(hasher.finalize()))
}

/// Find the correct binary asset for the current platform.
///
/// Prefers a **raw** binary (e.g. `HumanityOS-windows-x64.exe`) over an
/// archive (`.tar.gz`, `.zip`). The updater writes whatever it downloads
/// straight to disk and renames into place — if it grabs an archive it
/// produces a corrupt "binary" that Windows reports as a 16-bit application
/// (see BUG-034). Releases starting at v0.124.0 ship both formats; older
/// releases only ship archives, which the updater now refuses with a
/// helpful error rather than silently corrupting the install.
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

    let is_archive = |name: &str| -> bool {
        let n = name.to_lowercase();
        n.ends_with(".tar.gz") || n.ends_with(".tgz") || n.ends_with(".gz")
            || n.ends_with(".zip")
    };

    // First pass: a raw binary for this platform.
    if let Some(raw) = assets.iter().find(|a| {
        a.name.to_lowercase().contains(platform_pattern) && !is_archive(&a.name)
    }) {
        return Some(raw);
    }

    // No raw asset present — this is a pre-v0.124.0 release. Returning the
    // archive would corrupt the install (the bytes downloaded are gzipped tar
    // contents but get written straight to disk as the new exe). Refusing
    // here surfaces a clear error in the UI.
    None
}

/// Minimum acceptable binary size (1 MB). Anything smaller is almost certainly
/// a corrupt or incomplete download.
#[cfg(feature = "native")]
const MIN_BINARY_SIZE: u64 = 1_048_576;

/// The signed release manifest + its detached signature, uploaded as release
/// assets alongside the binaries. The updater fetches these to verify a
/// download before installing it (audit 2026-06-12 CRITICAL fix).
#[cfg(feature = "native")]
const MANIFEST_NAME: &str = "release-manifest.json";
#[cfg(feature = "native")]
const MANIFEST_SIG_NAME: &str = "release-manifest.json.sig.json";

/// Find a release asset's download URL by exact filename.
#[cfg(feature = "native")]
fn find_asset_url(assets: &[ReleaseAsset], exact_name: &str) -> Option<String> {
    assets
        .iter()
        .find(|a| a.name == exact_name)
        .map(|a| a.browser_download_url.clone())
}

/// Fetch a small asset (manifest / signature) fully into memory.
#[cfg(feature = "native")]
fn http_get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", "HumanityOS-Updater")
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut buf)
        .map_err(|e| format!("read error: {e}"))?;
    Ok(buf)
}

/// Download a binary and replace the running executable.
/// Reports progress via the channel. On success, the current exe has been
/// swapped out and a restart will launch the new version.
#[cfg(feature = "native")]
fn download_and_apply(
    url: &str,
    version: &str,
    manifest_url: Option<String>,
    sig_url: Option<String>,
    reply: &Reply,
) -> Result<(), String> {
    humanity_core::debug::push_debug(format!("Updater: starting download from {}", url));

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

    // Stream it, hashing the bytes as they pass so the download is checked against the signed
    // manifest without a second read of the file (stream_to_file, shared with the phone since
    // 2026-11-27). The bar moves by whole percents: one message and one frame per percent.
    let mut shown = -1i32;
    let mut progress = |p: f32| {
        let pct = (p * 100.0) as i32;
        if pct != shown {
            shown = pct;
            reply.send(UpdateMsg::DownloadProgress(p));
        }
    };
    let digest_hex = match stream_to_file(url, &update_path, &mut progress) {
        Ok(hex) => hex,
        Err(e) => {
            let _ = std::fs::remove_file(&update_path);
            return Err(e);
        }
    };

    // Pre-download verification: ensure the file exists and is large enough
    let update_size = std::fs::metadata(&update_path)
        .map(|m| m.len())
        .unwrap_or(0);
    humanity_core::debug::push_debug(format!(
        "Updater: download complete, file size = {} bytes",
        update_size
    ));

    if update_size < MIN_BINARY_SIZE {
        let msg = format!(
            "Downloaded file too small ({} bytes, expected > {} bytes). Aborting update.",
            update_size, MIN_BINARY_SIZE
        );
        humanity_core::debug::push_debug(format!("Updater: {}", msg));
        let _ = std::fs::remove_file(&update_path);
        return Err(msg);
    }

    // ── SIGNATURE VERIFICATION (audit 2026-06-12, the CRITICAL fix). ──
    // If the operator's release-signing keys are embedded in this build, the
    // release MUST carry a valid hybrid-signed manifest (Ed25519 + Dilithium3,
    // both must verify) whose recorded SHA-256 for THIS artifact matches the
    // bytes we just downloaded. Otherwise we delete the download and refuse to
    // install — a GitHub/release compromise can no longer push code to users.
    // Until the operator runs `just gen-release-key` (the embedded pubkeys are
    // empty), we log a loud warning and fall back to the legacy behaviour so
    // the app keeps working during that one-time provisioning window.
    let asset_name = url.rsplit('/').next().unwrap_or_default().to_string();
    if humanity_protocol::release_signature::embedded_pubkeys().is_provisioned() {
        let (Some(murl), Some(surl)) = (manifest_url.as_deref(), sig_url.as_deref()) else {
            let _ = std::fs::remove_file(&update_path);
            return Err(
                "This release is not signed (no release-manifest). Refusing to install it for \
                 your safety."
                    .into(),
            );
        };
        let manifest_bytes = http_get_bytes(murl).map_err(|e| {
            let _ = std::fs::remove_file(&update_path);
            format!("Could not fetch the release manifest: {e}")
        })?;
        let sig_bytes = http_get_bytes(surl).map_err(|e| {
            let _ = std::fs::remove_file(&update_path);
            format!("Could not fetch the release signature: {e}")
        })?;
        match humanity_protocol::release_signature::verify_release_artifact(
            &manifest_bytes,
            &sig_bytes,
            &asset_name,
            version,
            &digest_hex,
        ) {
            Ok(humanity_protocol::release_signature::VerifyOutcome::Verified) => {
                humanity_core::debug::push_debug("Updater: release signature VERIFIED.".to_string());
            }
            // Unreachable: we already confirmed the pubkeys are provisioned.
            Ok(humanity_protocol::release_signature::VerifyOutcome::Unprovisioned) => {}
            Err(e) => {
                let _ = std::fs::remove_file(&update_path);
                humanity_core::debug::push_debug(format!("Updater: VERIFICATION FAILED: {e}"));
                return Err(format!(
                    "Release verification FAILED — refusing to install. {e}"
                ));
            }
        }
    } else {
        humanity_core::debug::push_debug(
            "Updater: WARNING release-signing keys not provisioned; installing UNVERIFIED \
             (legacy). Run `just gen-release-key` to activate signed updates."
                .to_string(),
        );
        log::warn!(
            "Updater: release-signing not provisioned; installing without signature verification."
        );
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

    humanity_core::debug::push_debug(format!("Updater: exe_path = {}", exe_path.display()));
    humanity_core::debug::push_debug(format!("Updater: update_path = {}", update_path.display()));
    humanity_core::debug::push_debug(format!("Updater: old_path = {}", old_path.display()));

    let update_size = std::fs::metadata(update_path).map(|m| m.len()).unwrap_or(0);
    humanity_core::debug::push_debug(format!("Updater: update file size = {} bytes", update_size));
    log::info!("Updater: exe_path = {}", exe_path.display());
    log::info!("Updater: update_path = {}", update_path.display());
    log::info!("Updater: old_path = {}", old_path.display());
    log::info!("Updater: update file size = {} bytes", update_size);

    // Write restart_target.txt BEFORE any renames so we always know where the
    // new binary should end up, regardless of what current_exe() returns later.
    let parent = exe_path.parent().unwrap_or(std::path::Path::new("."));
    let restart_target_path = parent.join("restart_target.txt");
    if let Err(e) = std::fs::write(&restart_target_path, exe_path.to_string_lossy().as_bytes()) {
        humanity_core::debug::push_debug(format!("Updater: WARNING could not write restart_target.txt: {}", e));
        log::warn!("Updater: could not write restart_target.txt: {}", e);
    } else {
        humanity_core::debug::push_debug(format!("Updater: wrote restart_target.txt -> {}", exe_path.display()));
    }

    // Remove any existing .old file first
    if old_path.exists() {
        humanity_core::debug::push_debug("Updater: removing existing .old file");
        log::info!("Updater: removing existing .old file");
        let _ = std::fs::remove_file(&old_path);
    }

    humanity_core::debug::push_debug("Updater: renaming running exe to .old");
    log::info!("Updater: renaming running exe to .old");
    std::fs::rename(exe_path, &old_path)
        .map_err(|e| {
            let msg = format!("Failed to rename current exe to .old: {}", e);
            humanity_core::debug::push_debug(format!("Updater: {}", msg));
            msg
        })?;

    humanity_core::debug::push_debug("Updater: renaming .update to exe path");
    log::info!("Updater: renaming .update to exe path");
    if let Err(e) = std::fs::rename(update_path, exe_path) {
        // Try to restore the original if the swap failed
        let msg = format!("Failed to install update binary: {}", e);
        humanity_core::debug::push_debug(format!("Updater: swap failed, restoring original: {}", e));
        log::error!("Updater: swap failed, restoring original: {}", e);
        let _ = std::fs::rename(&old_path, exe_path);
        // Clean up restart_target.txt on failure
        let _ = std::fs::remove_file(&restart_target_path);
        return Err(msg);
    }

    // Post-swap verification: confirm the new binary exists at exe_path
    // and has a reasonable file size (> 1 MB)
    let new_size = std::fs::metadata(exe_path).map(|m| m.len()).unwrap_or(0);
    humanity_core::debug::push_debug(format!(
        "Updater: post-swap verification, new binary size = {} bytes",
        new_size
    ));

    if new_size < MIN_BINARY_SIZE {
        // Roll back: restore the old binary
        humanity_core::debug::push_debug(format!(
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

    humanity_core::debug::push_debug(format!(
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
/// the new exe. The script deletes itself after running. `behind`: this
/// instance was launched behind the window in front (`Updater::relaunch_behind`).
#[cfg(all(feature = "native", target_os = "windows"))]
pub fn create_restart_script(exe_path: &std::path::Path, behind: bool, quiet: bool) -> Result<PathBuf, String> {
    let bat_path = exe_path.with_extension("restart.bat");
    let exe_str = exe_path.to_string_lossy();
    // The relaunched exe's parent is cmd.exe, which the focus policy
    // (engine/launch_focus.rs) reads as a script launch -> background. So
    // the restart script must carry this instance's OWN focus state
    // forward (restart_focus_line).
    let focus_line = restart_focus_line(behind, quiet).map(|l| format!("{l}\r\n")).unwrap_or_default();
    let script = format!(
        "@echo off\r\ntimeout /t 2 /nobreak >nul\r\n{}start \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        focus_line, exe_str
    );
    std::fs::write(&bat_path, script)
        .map_err(|e| format!("Failed to write restart script: {}", e))?;
    humanity_core::debug::push_debug(format!("Updater: created restart script at {}", bat_path.display()));
    Ok(bat_path)
}

/// The line the restart script sets before it starts the new exe, carrying this instance's focus
/// state forward (its parent will be cmd.exe, which engine/launch_focus.rs reads as a script
/// launch): a foreground app comes back foreground; a quiet developer boot (a rig, `just
/// launch-bg`) comes back quiet and hidden; and a background launch nobody asked to be quiet (a
/// terminal, a .bat, Steam) sets nothing, so it comes back behind the other windows and VISIBLE, as
/// it started (the batch critic, 2026-11-14: NO_FOCUS also hides the window since 8278f84f7).
#[cfg(all(feature = "native", target_os = "windows"))]
pub fn restart_focus_line(behind: bool, quiet: bool) -> Option<&'static str> {
    match (behind, quiet) {
        (false, _) => Some("set HUMANITY_TAKE_FOCUS=1"),
        (true, true) => Some("set HUMANITY_NO_FOCUS=1"),
        (true, false) => None,
    }
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
            humanity_core::debug::push_debug(format!("Updater: restart target from file: {}", target));
            return PathBuf::from(target);
        }
    }
    humanity_core::debug::push_debug(format!(
        "Updater: no restart_target.txt found, using exe_path: {}",
        exe_path.display()
    ));
    exe_path.to_path_buf()
}

/// Replace the running executable with the downloaded update (Unix).
#[cfg(all(feature = "native", not(target_os = "windows")))]
fn apply_update(exe_path: &std::path::Path, update_path: &std::path::Path) -> Result<(), String> {
    humanity_core::debug::push_debug(format!("Updater: applying update (Unix) to {}", exe_path.display()));

    // On Unix, we can replace the exe directly.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(update_path, std::fs::Permissions::from_mode(0o755));
    }

    std::fs::rename(update_path, exe_path)
        .map_err(|e| {
            humanity_core::debug::push_debug(format!("Updater: Unix rename failed: {}", e));
            format!("Failed to replace executable: {}", e)
        })?;

    // Post-swap verification
    let new_size = std::fs::metadata(exe_path).map(|m| m.len()).unwrap_or(0);
    humanity_core::debug::push_debug(format!(
        "Updater: post-swap verification, new binary size = {} bytes",
        new_size
    ));

    if new_size < MIN_BINARY_SIZE {
        let msg = format!(
            "Update verification failed: new binary is {} bytes (expected > {} bytes)",
            new_size, MIN_BINARY_SIZE
        );
        humanity_core::debug::push_debug(format!("Updater: {}", msg));
        return Err(msg);
    }

    humanity_core::debug::push_debug(format!(
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

    fn release(tag: &str, signed: bool) -> ReleaseInfo {
        let asset = |name: &str| ReleaseAsset { name: name.to_string(), browser_download_url: format!("https://example.invalid/{tag}/{name}"), size: 1 };
        let mut assets = vec![
            asset("HumanityOS-windows-x64.exe"),
            asset("HumanityOS-linux-x64"),
            asset("HumanityOS-macos-arm64"),
            asset("HumanityOS-macos-x64"),
        ];
        if signed {
            assets.push(asset(MANIFEST_NAME));
            assets.push(asset("release-manifest.json.sig.json"));
        }
        ReleaseInfo { tag_name: tag.to_string(), name: tag.to_string(), body: String::new(), published_at: String::new(), assets, prerelease: false, draft: false }
    }

    /// AN UNSIGNED RELEASE IS NEVER OFFERED (the 2026-06-12 audit's rule, checked again after the
    /// verification moved to humanity_protocol::release_signature on 2026-11-02): with the release
    /// keys compiled in, a newer release without the signed manifest is passed over for the newest
    /// one that has it, and with no signed release newer than this build nothing is offered.
    ///
    /// Seen red 2026-11-02 with `require_manifest` forced to false in evaluate_update: "the unsigned
    /// v0.1549.0 was offered: Available { version: \"v0.1549.0\", ... }".
    #[test]
    fn an_unsigned_release_is_never_offered() {
        assert!(humanity_protocol::release_signature::embedded_pubkeys().is_provisioned(), "the release keys are compiled in");
        let mut u = Updater::new("0.1546.0");
        u.channel = UpdateChannel::AlwaysLatest;
        u.available_releases = vec![release("v0.1549.0", false), release("v0.1548.0", false), release("v0.1547.0", true), release("v0.1543.0", true)];
        u.evaluate_update();
        match &u.state {
            UpdateState::Available { version, .. } => assert_eq!(version, "v0.1547.0", "the unsigned {version} was offered: {:?}", u.state),
            other => panic!("the signed v0.1547.0 was not offered: {other:?}"),
        }
        u.available_releases = vec![release("v0.1549.0", false), release("v0.1543.0", true)];
        u.evaluate_update();
        assert!(matches!(u.state, UpdateState::UpToDate), "only an unsigned release is newer, so nothing is offered: {:?}", u.state);
    }

    /// AN UPDATE NEVER HIDES A PERSON'S GAME (the batch critic, 2026-11-14): a game a person started
    /// from a terminal, a .bat or another launcher opens behind the other windows but visible; after
    /// an in-app update its restart script must not set HUMANITY_NO_FOCUS, which since 8278f84f7 also
    /// hides the window (engine::rig_window), with no taskbar button and no sound. Only a quiet
    /// developer boot comes back quiet; a foreground game comes back foreground.
    ///
    /// Seen red 2026-11-14 with the rule before the fix (every background launch got NO_FOCUS).
    #[cfg(all(feature = "native", target_os = "windows"))]
    #[test]
    fn an_update_never_hides_a_persons_game() {
        let l = restart_focus_line(true, false);
        assert_eq!(l, None, "a person's background launch comes back hidden: {l:?}");
        assert_eq!(restart_focus_line(true, true), Some("set HUMANITY_NO_FOCUS=1"), "a quiet developer boot stays quiet");
        assert_eq!(restart_focus_line(false, false), Some("set HUMANITY_TAKE_FOCUS=1"), "a foreground game comes back foreground");
        assert_eq!(restart_focus_line(false, true), Some("set HUMANITY_TAKE_FOCUS=1"), "foreground wins over quiet, as launch_in_background reads it");
    }

    /// A feed with nothing in it, so a check a test starts never reaches the network.
    fn no_releases() -> Result<Vec<ReleaseInfo>, String> {
        Ok(Vec::new())
    }

    /// TURNING "LOOK FOR UPDATES" OFF STOPS EVERY CHECK the app makes by itself, on a desktop and
    /// the phone alike (one setting, 2026-11-27): the one at start and the hourly one. Check now
    /// still looks once when pressed. Turned on, both run.
    ///
    /// Seen red 2026-11-27 with check_at_start and poll not asking the setting: "Desktop: looking is
    /// off, yet the start checked: Checking".
    #[test]
    fn turning_looking_off_stops_every_check() {
        for target in [UpdateTarget::Desktop, UpdateTarget::Phone] {
            let made = |channel: UpdateChannel| {
                let mut u = Updater::new("0.1578.0");
                u.target = target;
                u.feed = no_releases;
                u.channel = channel;
                u
            };
            let mut off = made(UpdateChannel::Disabled);
            off.check_at_start();
            assert_eq!(off.state, UpdateState::Idle, "{target:?}: looking is off, yet the start checked: {:?}", off.state);
            // The hour is up (an interval of nothing, rather than a clock moved back, which a
            // freshly started machine cannot do).
            off.check_interval = 0.0;
            off.poll();
            assert_eq!(off.state, UpdateState::Idle, "{target:?}: looking is off, yet the hourly check ran: {:?}", off.state);
            off.check_now();
            assert_eq!(off.state, UpdateState::Checking, "{target:?}: Check now looks once even while looking is off");

            let mut on = made(UpdateChannel::AlwaysLatest);
            on.check_at_start();
            assert_eq!(on.state, UpdateState::Checking, "{target:?}: looking is on, yet the start did not check");
            let mut on = made(UpdateChannel::AlwaysLatest);
            on.check_interval = 0.0;
            on.poll();
            assert_eq!(on.state, UpdateState::Checking, "{target:?}: looking is on, yet the hourly check did not run");
        }
    }

    /// "Look for updates" turned off STAYS off across a restart (2026-11-27, config.json
    /// `update_channel`): before, it went back to looking at every start, so off never held on a
    /// phone, which Android starts again often. A config from before the setting looks.
    ///
    /// Seen red 2026-11-27 with apply_to_gui_state not reading it back: "looking turned off came
    /// back on after a restart".
    #[test]
    fn looking_off_is_kept_across_a_restart() {
        let mut state = crate::gui::GuiState::default();
        state.updater.channel = UpdateChannel::Disabled;
        let json = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).expect("serialize the config");
        let reloaded: crate::config::AppConfig = serde_json::from_str(&json).expect("read it back");
        let mut fresh = crate::gui::GuiState::default();
        reloaded.apply_to_gui_state(&mut fresh);
        assert_eq!(fresh.updater.channel, UpdateChannel::Disabled, "looking turned off came back on after a restart");
        let old: crate::config::AppConfig = serde_json::from_str("{}").expect("a config from before the setting parses");
        assert_eq!(old.update_channel, UpdateChannel::AlwaysLatest, "a config from before the setting does not look for updates");
    }

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
