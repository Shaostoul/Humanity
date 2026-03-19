//! Local-first data persistence for HumanityOS desktop app.
//!
//! Every player's data lives on their own machine first. The server is optional
//! backup infrastructure, not the source of truth. This module owns the on-disk
//! layout so the rest of the app (and the JS frontend) never deals with raw
//! filesystem paths.
//!
//! Directory layout under `{app_data_dir}`:
//! ```text
//! identity/
//!   keys.json.enc       ← Ed25519 keypair, AES-256-GCM encrypted at rest
//!   recovery.json       ← public key + display name + created timestamp
//! saves/
//!   {save_name}/        ← named save slots ("main", "creative", …)
//!     profile.json      ← display name, avatar, bio, settings
//!     inventory.json    ← items, quantities
//!     farm.json         ← plot states, planted crops, growth timers
//!     quests.json       ← quest progress, completed quests
//!     skills.json       ← skill levels, XP
//!     world.json        ← explored areas, structures, world state
//! settings/
//!   preferences.json    ← theme, keybinds, audio levels
//!   sync.json           ← tiered backup config (what syncs where)
//!   display.json        ← window size, position, UI layout
//! cache/
//!   messages/           ← offline message cache by channel
//!   avatars/            ← cached avatar images
//!   manifests/          ← web sync manifests
//! backups/
//!   {timestamp}/        ← periodic local backups (auto-rotate, keep last 5)
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// ── Sync configuration ─────────────────────────────────────────────────────

/// Controls which data categories replicate to which tier of server.
/// Users configure this in Settings → Data & Sync. The default is
/// conservative: identity stays local-only, everything else syncs to
/// the user's own server but NOT to public infrastructure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub items: Vec<SyncItem>,
}

/// One row in the sync-tier matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncItem {
    /// Category key: "identity", "saves", "settings", "messages", "vault"
    pub category: String,
    /// Human-readable label shown in the UI
    pub label: String,
    /// Always true — local storage is mandatory
    pub local: bool,
    /// Sync to the user's own self-hosted server
    pub own_server: bool,
    /// Sync to a trusted community server (encrypted)
    pub trusted_server: bool,
    /// Sync minimal recovery info to a public relay
    pub public_server: bool,
}

/// Metadata about an external drive that contains HumanityOS data,
/// used by the import/export UI to show discovered USB drives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedDrive {
    /// Root path of the HumanityOS folder on the drive (e.g. "E:\\HumanityOS")
    pub path: String,
    /// Volume label if the OS provides one, otherwise the drive letter
    pub label: String,
    /// Whether the drive has a saves/ subdirectory
    pub has_saves: bool,
    /// Whether the drive has an identity/ subdirectory
    pub has_identity: bool,
    /// Most recent file modification time (UNIX seconds) across all files
    pub last_modified: Option<u64>,
}

/// Size statistics for the settings UI ("Your data is using X MB").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// Total bytes across all local data
    pub total_bytes: u64,
    /// Number of save slots
    pub save_count: usize,
    /// Number of backup snapshots
    pub backup_count: usize,
    /// Absolute path to the data directory (for display)
    pub data_dir: String,
}

// ── Top-level directories we always create ──────────────────────────────────

const REQUIRED_DIRS: &[&str] = &[
    "identity",
    "saves",
    "settings",
    "cache/messages",
    "cache/avatars",
    "cache/manifests",
    "backups",
];

/// Files that every new save slot starts with (empty JSON objects/arrays).
const SAVE_SLOT_FILES: &[&str] = &[
    "profile.json",
    "inventory.json",
    "farm.json",
    "quests.json",
    "skills.json",
    "world.json",
];

// ── LocalStorage ────────────────────────────────────────────────────────────

/// Thread-safe wrapper so Tauri can `.manage()` it as shared state.
pub struct LocalStorageState(pub Mutex<LocalStorage>);

/// Owns the on-disk data directory and provides typed read/write helpers.
pub struct LocalStorage {
    base_dir: PathBuf,
}

impl LocalStorage {
    /// Create a handle pointing at `app_data_dir`. Does NOT touch the
    /// filesystem yet — call [`init`] once at startup.
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            base_dir: app_data_dir,
        }
    }

    // ── Bootstrap ───────────────────────────────────────────────────────

    /// Create the full directory tree on first launch.
    /// Idempotent — safe to call every startup.
    pub fn init(&self) -> Result<(), String> {
        for dir in REQUIRED_DIRS {
            let path = self.base_dir.join(dir);
            fs::create_dir_all(&path).map_err(|e| {
                format!("Failed to create directory {}: {e}", path.display())
            })?;
        }

        // Ensure a default sync config exists
        let sync_path = self.base_dir.join("settings/sync.json");
        if !sync_path.exists() {
            self.write_json("settings/sync.json", &Self::default_sync_config())?;
        }

        // Ensure a "main" save slot exists so new users have somewhere to play
        let main_save = self.base_dir.join("saves/main");
        if !main_save.exists() {
            self.create_save("main")?;
        }

        Ok(())
    }

    // ── Generic JSON I/O ────────────────────────────────────────────────

    /// Read and deserialize a JSON file relative to the data directory.
    /// Returns a descriptive error if the file is missing or malformed.
    pub fn read_json<T: serde::de::DeserializeOwned>(&self, sub_path: &str) -> Result<T, String> {
        let path = self.base_dir.join(sub_path);
        let data = fs::read_to_string(&path)
            .map_err(|e| format!("Read {}: {e}", path.display()))?;
        serde_json::from_str(&data)
            .map_err(|e| format!("Parse {}: {e}", path.display()))
    }

    /// Serialize and write a JSON file relative to the data directory.
    /// Creates parent directories if needed so callers never worry about it.
    pub fn write_json<T: serde::Serialize>(&self, sub_path: &str, data: &T) -> Result<(), String> {
        let path = self.base_dir.join(sub_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Create dirs for {}: {e}", path.display()))?;
        }
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Serialize for {}: {e}", path.display()))?;
        fs::write(&path, json)
            .map_err(|e| format!("Write {}: {e}", path.display()))
    }

    // ── Save slots ──────────────────────────────────────────────────────

    /// List the names of all save slots (subdirectories of `saves/`).
    pub fn list_saves(&self) -> Vec<String> {
        let saves_dir = self.base_dir.join("saves");
        let mut names = Vec::new();
        if let Ok(entries) = fs::read_dir(&saves_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        names.push(name.to_string());
                    }
                }
            }
        }
        names.sort();
        names
    }

    /// Create a new save slot with empty JSON stub files.
    pub fn create_save(&self, name: &str) -> Result<(), String> {
        Self::validate_save_name(name)?;
        let save_dir = self.base_dir.join("saves").join(name);
        if save_dir.exists() {
            return Err(format!("Save slot '{name}' already exists"));
        }
        fs::create_dir_all(&save_dir)
            .map_err(|e| format!("Create save dir: {e}"))?;

        // Seed each file with an empty JSON object so reads never fail
        for file in SAVE_SLOT_FILES {
            let path = save_dir.join(file);
            fs::write(&path, "{}")
                .map_err(|e| format!("Write {file}: {e}"))?;
        }
        Ok(())
    }

    /// Permanently delete a save slot and all its data.
    pub fn delete_save(&self, name: &str) -> Result<(), String> {
        Self::validate_save_name(name)?;
        let save_dir = self.base_dir.join("saves").join(name);
        if !save_dir.exists() {
            return Err(format!("Save slot '{name}' does not exist"));
        }
        fs::remove_dir_all(&save_dir)
            .map_err(|e| format!("Delete save '{name}': {e}"))
    }

    /// Copy a save slot to an external location (USB drive, cloud folder).
    /// The target directory will contain the save files directly.
    pub fn export_save(&self, name: &str, target: &Path) -> Result<(), String> {
        Self::validate_save_name(name)?;
        let save_dir = self.base_dir.join("saves").join(name);
        if !save_dir.exists() {
            return Err(format!("Save slot '{name}' does not exist"));
        }
        copy_dir_recursive(&save_dir, target)
    }

    /// Import a save from an external directory. The directory must contain
    /// at least one of the standard save-slot JSON files.
    /// Returns the name assigned to the imported save.
    pub fn import_save(&self, source: &Path) -> Result<String, String> {
        if !source.is_dir() {
            return Err("Source path is not a directory".to_string());
        }

        // Determine save name from the source directory name
        let dir_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("imported");

        // Deduplicate: if "imported" exists, try "imported_2", etc.
        let saves_dir = self.base_dir.join("saves");
        let mut final_name = dir_name.to_string();
        let mut counter = 2u32;
        while saves_dir.join(&final_name).exists() {
            final_name = format!("{dir_name}_{counter}");
            counter += 1;
        }

        let dest = saves_dir.join(&final_name);
        copy_dir_recursive(source, &dest)?;
        Ok(final_name)
    }

    // ── Backups ─────────────────────────────────────────────────────────

    /// Snapshot the entire data directory (minus cache/ and backups/) into
    /// `backups/{timestamp}/`. Old snapshots beyond `max_backups` are deleted
    /// oldest-first to prevent unbounded disk growth.
    pub fn create_backup(&self, max_backups: usize) -> Result<String, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let backup_name = format!("{timestamp}");
        let backup_dir = self.base_dir.join("backups").join(&backup_name);

        fs::create_dir_all(&backup_dir)
            .map_err(|e| format!("Create backup dir: {e}"))?;

        // Copy identity/, saves/, settings/ into the backup
        for folder in &["identity", "saves", "settings"] {
            let src = self.base_dir.join(folder);
            if src.exists() {
                let dst = backup_dir.join(folder);
                copy_dir_recursive(&src, &dst)?;
            }
        }

        // Rotate: keep only the most recent `max_backups`
        self.rotate_backups(max_backups)?;

        Ok(backup_name)
    }

    /// Delete the oldest backup directories until at most `max` remain.
    fn rotate_backups(&self, max: usize) -> Result<(), String> {
        let backups_dir = self.base_dir.join("backups");
        let mut entries: Vec<(String, PathBuf)> = Vec::new();

        if let Ok(iter) = fs::read_dir(&backups_dir) {
            for entry in iter.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        entries.push((name.to_string(), entry.path()));
                    }
                }
            }
        }

        // Sort by name ascending (names are timestamps, so oldest first)
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        while entries.len() > max {
            let (_, path) = entries.remove(0);
            let _ = fs::remove_dir_all(path);
        }
        Ok(())
    }

    // ── External drive detection ────────────────────────────────────────

    /// Scan mounted volumes for a `HumanityOS/` directory.
    /// On Windows: checks drive letters D: through Z:.
    /// On other platforms: checks common mount points.
    pub fn detect_external_drives(&self) -> Vec<DetectedDrive> {
        let mut drives = Vec::new();

        #[cfg(target_os = "windows")]
        {
            // Skip A:, B: (floppy), C: (system) — check D: through Z:
            for letter in b'D'..=b'Z' {
                let drive_root = format!("{}:\\", letter as char);
                let hos_path = PathBuf::from(&drive_root).join("HumanityOS");
                if hos_path.is_dir() {
                    let has_saves = hos_path.join("saves").is_dir();
                    let has_identity = hos_path.join("identity").is_dir();
                    let last_modified = newest_mtime_in(&hos_path);

                    drives.push(DetectedDrive {
                        path: hos_path.to_string_lossy().to_string(),
                        label: format!("Drive {}:", letter as char),
                        has_saves,
                        has_identity,
                        last_modified,
                    });
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Linux/macOS: check /media, /mnt, /Volumes
            let mount_roots = ["/media", "/mnt", "/Volumes", "/run/media"];
            for root in &mount_roots {
                let root_path = Path::new(root);
                if !root_path.is_dir() {
                    continue;
                }
                if let Ok(entries) = fs::read_dir(root_path) {
                    for entry in entries.flatten() {
                        // On /media/$USER/..., descend one more level
                        let check_paths = if entry.path().is_dir() {
                            let mut paths = vec![entry.path().join("HumanityOS")];
                            // Also check subdirectories (e.g. /media/user/USBDRIVE/HumanityOS)
                            if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                                for sub in sub_entries.flatten() {
                                    if sub.path().is_dir() {
                                        paths.push(sub.path().join("HumanityOS"));
                                    }
                                }
                            }
                            paths
                        } else {
                            vec![]
                        };

                        for hos_path in check_paths {
                            if hos_path.is_dir() {
                                let has_saves = hos_path.join("saves").is_dir();
                                let has_identity = hos_path.join("identity").is_dir();
                                let last_modified = newest_mtime_in(&hos_path);
                                let label = entry
                                    .file_name()
                                    .to_string_lossy()
                                    .to_string();

                                drives.push(DetectedDrive {
                                    path: hos_path.to_string_lossy().to_string(),
                                    label,
                                    has_saves,
                                    has_identity,
                                    last_modified,
                                });
                            }
                        }
                    }
                }
            }
        }

        drives
    }

    // ── Sync config ─────────────────────────────────────────────────────

    /// Load the tiered sync configuration. Falls back to sensible defaults
    /// if the file doesn't exist or is corrupt.
    pub fn get_sync_config(&self) -> SyncConfig {
        self.read_json::<SyncConfig>("settings/sync.json")
            .unwrap_or_else(|_| Self::default_sync_config())
    }

    /// Persist updated sync tier assignments.
    pub fn set_sync_config(&self, config: &SyncConfig) -> Result<(), String> {
        self.write_json("settings/sync.json", config)
    }

    /// Conservative defaults: identity never leaves the device; everything
    /// else syncs to the user's own server only.
    fn default_sync_config() -> SyncConfig {
        SyncConfig {
            items: vec![
                SyncItem {
                    category: "identity".into(),
                    label: "Identity & Keys".into(),
                    local: true,
                    own_server: false,
                    trusted_server: false,
                    public_server: false,
                },
                SyncItem {
                    category: "saves".into(),
                    label: "Save Games".into(),
                    local: true,
                    own_server: true,
                    trusted_server: false,
                    public_server: false,
                },
                SyncItem {
                    category: "settings".into(),
                    label: "App Settings".into(),
                    local: true,
                    own_server: true,
                    trusted_server: false,
                    public_server: false,
                },
                SyncItem {
                    category: "messages".into(),
                    label: "Message History".into(),
                    local: true,
                    own_server: true,
                    trusted_server: false,
                    public_server: false,
                },
                SyncItem {
                    category: "vault".into(),
                    label: "Encrypted Vault".into(),
                    local: true,
                    own_server: true,
                    trusted_server: true,
                    public_server: false,
                },
            ],
        }
    }

    // ── Introspection ───────────────────────────────────────────────────

    /// Absolute path to the data directory, for display in the settings UI.
    pub fn data_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Walk the entire data directory and sum file sizes.
    pub fn total_size(&self) -> u64 {
        dir_size(&self.base_dir)
    }

    /// Gather stats for the settings UI.
    pub fn storage_stats(&self) -> StorageStats {
        let saves = self.list_saves();
        let backup_count = fs::read_dir(self.base_dir.join("backups"))
            .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
            .unwrap_or(0);

        StorageStats {
            total_bytes: self.total_size(),
            save_count: saves.len(),
            backup_count,
            data_dir: self.base_dir.to_string_lossy().to_string(),
        }
    }

    /// Move the entire data directory to a new location. Updates the
    /// internal base_dir so subsequent calls use the new path.
    /// This is a heavy operation — copies everything then deletes the old tree.
    pub fn relocate(&mut self, new_path: PathBuf) -> Result<(), String> {
        if new_path == self.base_dir {
            return Ok(());
        }
        if new_path.exists() && fs::read_dir(&new_path).map(|mut d| d.next().is_some()).unwrap_or(false) {
            return Err("Target directory is not empty".to_string());
        }

        copy_dir_recursive(&self.base_dir, &new_path)?;

        // Only delete old dir after successful copy
        fs::remove_dir_all(&self.base_dir)
            .map_err(|e| format!("Remove old data dir: {e}"))?;

        self.base_dir = new_path;
        Ok(())
    }

    // ── Name validation ─────────────────────────────────────────────────

    /// Save names must be filesystem-safe: alphanumeric, hyphens, underscores.
    fn validate_save_name(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("Save name cannot be empty".to_string());
        }
        if name.len() > 64 {
            return Err("Save name too long (max 64 characters)".to_string());
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(
                "Save name may only contain letters, numbers, hyphens, and underscores".to_string(),
            );
        }
        Ok(())
    }
}

// ── Filesystem helpers ──────────────────────────────────────────────────────

/// Recursively copy a directory tree. Creates `dst` if it doesn't exist.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("Create dir {}: {e}", dst.display()))?;

    let entries = fs::read_dir(src)
        .map_err(|e| format!("Read dir {}: {e}", src.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Dir entry: {e}"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!("Copy {} → {}: {e}", src_path.display(), dst_path.display())
            })?;
        }
    }
    Ok(())
}

/// Sum the byte sizes of all files under `dir`, recursively.
fn dir_size(dir: &Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += dir_size(&path);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Find the newest file modification time (UNIX seconds) under `dir`.
fn newest_mtime_in(dir: &Path) -> Option<u64> {
    let mut newest: Option<u64> = None;

    fn walk(dir: &Path, newest: &mut Option<u64>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, newest);
                } else if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(dur) = modified.duration_since(UNIX_EPOCH) {
                            let secs = dur.as_secs();
                            match *newest {
                                Some(n) if secs > n => *newest = Some(secs),
                                None => *newest = Some(secs),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    walk(dir, &mut newest);
    newest
}
