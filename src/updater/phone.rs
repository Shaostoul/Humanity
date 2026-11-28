//! The phone app updates itself (2026-11-27; docs/design/platforms.md, rung A2 "The app updates
//! itself"; android/README.md, "Updating").
//!
//! The operator asked on 2026-11-22 how to update the phone app. Until then a person found the new
//! APK on the release page, downloaded it and opened it, and Android offered Update because both
//! were signed with the project's key. Now the app makes the desktop's check (the parent module:
//! the same release feed, the same "Look for updates" setting, at start and once an hour), looks
//! for the release's APK and its checksum ([`apk_assets`]), offers it on a line at the bottom of
//! every page (gui/bottom_strip/update_offer.rs), downloads it inside the app with its progress
//! ([`download_apk`]), checks it against the SHA-256 checksum the release workflow publishes beside
//! it, and hands it to Android's installer ([`install`], android/java/.../UpdateInstall.java), which
//! asks the person to confirm. A release without an APK (an asset release, or one whose APK is not
//! attached yet) offers nothing; neither does one whose APK has no checksum beside it.
//!
//! WHAT PROTECTS WHAT (written out in docs/design/platforms.md):
//! - **The checksum** (`HumanityOS-android-arm64.apk.sha256`, made by android/build-apk.sh, checked
//!   here before Android sees the file): a download cut short, damaged on the way, or swapped for
//!   another file while the checksum stayed is refused and deleted ([`Refusal::Mismatch`]). It is
//!   published by the same workflow on the same release as the APK, so it does NOT protect against
//!   someone who can publish releases: they can replace both.
//! - **Android's same-key rule** (Android's own check, not ours): an update installs over the app
//!   only when it is signed with the same key as the app installed. A file made by anyone without
//!   the project's key is refused by Android, whatever its checksum. The key is kept as two of the
//!   repository's secrets, which the Android workflow signs with, so someone who takes over the
//!   account that publishes releases could build a signed APK through the workflow: neither check
//!   stops that.
//! - **HTTPS** to GitHub carries the feed, the checksum and the APK.
//!
//! WHERE THE FILE WAITS: the app's own files folder (HOME, src/android.rs), in [`UPDATE_FOLDER`],
//! named for its version ([`file_name`]). It is downloaded to a `.part` file and renamed only once
//! its checksum matched, so a file with the final name is always a checked one: the provider that
//! serves it to the installer serves only those names. A start deletes half-downloaded files and
//! every update not newer than the version running ([`stale`]); a newer one stays, so a person who
//! went to Android's settings to allow installs (Android may close the app meanwhile) comes back to
//! Install, not to a second download.

use std::io::Read;
use std::path::{Path, PathBuf};

use super::ReleaseAsset;

/// The release's APK, as .github/workflows/build-android.yml publishes it.
pub const APK_NAME: &str = "HumanityOS-android-arm64.apk";
/// Its SHA-256 checksum, published beside it (android/build-apk.sh: `sha256sum`'s line, "<hex>  <name>").
pub const APK_SUM_NAME: &str = "HumanityOS-android-arm64.apk.sha256";
/// The folder inside the app's own files folder where a downloaded update waits
/// (UpdateInstall.java's `FOLDER`; a test below checks they agree).
pub const UPDATE_FOLDER: &str = "updates";
/// The most a checksum file is read: it is one line, so anything longer is not the checksum.
const SUM_MAX_BYTES: u64 = 4096;

/// The APK and its checksum in a release's assets, by their exact names; `None` when either is
/// missing: a desktop-only release, an asset release (instrument packs, model packs), or one whose
/// APK is not attached yet.
pub fn apk_assets(assets: &[ReleaseAsset]) -> Option<(&ReleaseAsset, &ReleaseAsset)> {
    let apk = assets.iter().find(|a| a.name == APK_NAME)?;
    let sum = assets.iter().find(|a| a.name == APK_SUM_NAME)?;
    Some((apk, sum))
}

/// The SHA-256 a checksum file names for the APK, in lowercase hex. `sha256sum` writes one line,
/// "<64 hex digits>  <file name>" (a `*` before the name in binary mode); a name other than the
/// APK's, a short or non-hex digest, or more than that one line is not this APK's checksum.
pub fn parse_checksum(text: &str) -> Option<String> {
    let mut words = text.split_whitespace();
    let hex = words.next()?.to_ascii_lowercase();
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    if let Some(name) = words.next() {
        if name.trim_start_matches('*') != APK_NAME {
            return None;
        }
    }
    if words.next().is_some() {
        return None;
    }
    Some(hex)
}

/// The file a downloaded update of `version` is kept as: `HumanityOS-0.1580.0.apk`. Only letters,
/// digits, dots and dashes of the version are kept, the shape UpdateInstall.java's `safeName` serves.
pub fn file_name(version: &str) -> String {
    let v: String = version.trim_start_matches('v').chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect();
    format!("HumanityOS-{v}.apk")
}

/// The version a kept update is of, from its file name ([`file_name`]).
fn version_of(file: &str) -> Option<&str> {
    file.strip_prefix("HumanityOS-")?.strip_suffix(".apk")
}

/// Which of the files in the updates folder to delete when version `running` starts: each
/// half-downloaded one (`.part`), and each update not newer than `running` (installed already, or
/// older). A newer one stays: it was downloaded and checked, and waits for Install.
pub fn stale(names: &[String], running: &str) -> Vec<String> {
    names
        .iter()
        .filter(|n| n.ends_with(".part") || version_of(n).is_some_and(|v| !super::is_newer(v, running)))
        .cloned()
        .collect()
}

/// The updates folder, on a phone: the app's own files folder (HOME, src/android.rs) and
/// [`UPDATE_FOLDER`]. `None` on a desktop, which updates by replacing its exe instead.
pub fn folder() -> Option<PathBuf> {
    if cfg!(target_os = "android") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(UPDATE_FOLDER))
    } else {
        None
    }
}

/// The checked download of `version`, when it is waiting in the updates folder.
pub fn downloaded(version: &str) -> Option<PathBuf> {
    let path = folder()?.join(file_name(version));
    path.is_file().then_some(path)
}

/// Delete what [`stale`] names in the updates folder (a start calls it, `Updater::cleanup_old_versions`).
pub fn clean_up(running: &str) {
    let Some(dir) = folder() else { return };
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    let names: Vec<String> = entries.flatten().filter_map(|e| e.file_name().into_string().ok()).collect();
    for name in stale(&names, running) {
        match std::fs::remove_file(dir.join(&name)) {
            Ok(()) => log::info!("Updater: removed the old update file {name}"),
            Err(e) => log::warn!("Updater: could not remove the old update file {name}: {e}"),
        }
    }
}

/// Why the phone's update stopped. Each says a sentence of its own by key ([`Refusal::words_key`]);
/// [`Refusal::detail`] is the technical part, shown small under it and logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The checksum file could not be fetched, or does not hold this APK's checksum.
    NoChecksum(String),
    /// The download itself failed: the network, or writing the file.
    Download(String),
    /// The downloaded bytes do not match the published checksum. The file was deleted.
    Mismatch { expected: String, got: String },
    /// Not on a phone: a desktop updates by replacing its exe.
    NotAPhone,
    /// The checked download is no longer in the updates folder.
    Missing(String),
    /// Android's installer did not open.
    Installer(String),
}

impl Refusal {
    /// The sentence the person reads (data/i18n/en.json `app_update.*`).
    pub fn words_key(&self) -> &'static str {
        match self {
            Refusal::NoChecksum(_) => "app_update.no_checksum",
            Refusal::Download(_) => "app_update.failed_download",
            Refusal::Mismatch { .. } => "app_update.checksum_mismatch",
            Refusal::NotAPhone => "app_update.phone_only",
            Refusal::Missing(_) => "app_update.download_gone",
            Refusal::Installer(_) => "app_update.install_failed",
        }
    }

    /// The technical part, for the log and the small line under the sentence.
    pub fn detail(&self) -> String {
        match self {
            Refusal::NoChecksum(d) | Refusal::Download(d) | Refusal::Missing(d) | Refusal::Installer(d) => d.clone(),
            Refusal::Mismatch { expected, got } => format!("SHA-256 published {expected}, downloaded {got}"),
            Refusal::NotAPhone => String::new(),
        }
    }
}

/// Download the APK at `apk_url` into `folder` as the update of `version`, checked against the
/// checksum at `sum_url`: the checksum first (a release whose checksum cannot be read is not
/// downloaded at all), then the APK into a `.part` file, hashed as it streams, `progress` told the
/// share done (0 to 1) when the size is known. On a match the file is renamed to its final name and
/// its path returned; on a mismatch, or any failure, nothing is left behind.
pub fn download_apk(apk_url: &str, sum_url: &str, folder: &Path, version: &str, progress: &mut dyn FnMut(f32)) -> Result<PathBuf, Refusal> {
    let response = ureq::get(sum_url)
        .set("User-Agent", "HumanityOS-Updater")
        .call()
        .map_err(|e| Refusal::NoChecksum(format!("{sum_url}: {e}")))?;
    let mut text = String::new();
    response
        .into_reader()
        .take(SUM_MAX_BYTES)
        .read_to_string(&mut text)
        .map_err(|e| Refusal::NoChecksum(format!("{sum_url}: {e}")))?;
    let expected = parse_checksum(&text).ok_or_else(|| {
        let start: String = text.chars().take(80).collect();
        Refusal::NoChecksum(format!("{APK_SUM_NAME} does not hold the APK's SHA-256: {start:?}"))
    })?;

    std::fs::create_dir_all(folder).map_err(|e| Refusal::Download(format!("{}: {e}", folder.display())))?;
    let name = file_name(version);
    let done = folder.join(&name);
    let part = folder.join(format!("{name}.part"));
    let _ = std::fs::remove_file(&done);
    let _ = std::fs::remove_file(&part);
    let got = match super::stream_to_file(apk_url, &part, progress) {
        Ok(hex) => hex,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(Refusal::Download(e));
        }
    };
    if got != expected {
        let _ = std::fs::remove_file(&part);
        log::warn!("Updater: the phone app's download of {version} did not match its checksum (published {expected}, downloaded {got}); deleted");
        return Err(Refusal::Mismatch { expected, got });
    }
    if let Err(e) = std::fs::rename(&part, &done) {
        let _ = std::fs::remove_file(&part);
        return Err(Refusal::Download(format!("{}: {e}", done.display())));
    }
    log::info!("Updater: the phone app's update {version} is downloaded and matches its checksum ({expected})");
    Ok(done)
}

/// Hand the checked download of `version` to Android's installer, which asks the person to confirm
/// (UpdateInstall.java). `Ok` once the installer opened.
pub fn install(version: &str) -> Result<(), Refusal> {
    let dir = folder().ok_or(Refusal::NotAPhone)?;
    let name = file_name(version);
    if !dir.join(&name).is_file() {
        return Err(Refusal::Missing(name));
    }
    android_install(&name).map_err(Refusal::Installer)
}

#[cfg(target_os = "android")]
fn android_install(name: &str) -> Result<(), String> {
    humanity_core::platform::android_jni::update_install(name)
}

#[cfg(not(target_os = "android"))]
fn android_install(_name: &str) -> Result<(), String> {
    Err("only the phone app hands an update to Android's installer".into())
}

#[cfg(test)]
#[path = "phone_tests.rs"]
mod tests;
