//! Storage-mode resolution (v0.707): WHERE HumanityOS keeps a user's files.
//!
//! Operator design (2026-07-06, after his dad's exe littered a Downloads
//! folder and the v0.706 APPDATA fix raised the external-drive question):
//! the user chooses on first boot, and every existing setup keeps working
//! untouched. Four states:
//!
//! - **Portable** — a `portable.txt` marker sits beside the exe. EVERYTHING
//!   (data, saves, config incl. the encrypted identity, logs) lives beside
//!   the exe, so a USB/external-drive install travels between machines as
//!   one folder. Chosen explicitly on first boot (or by hand-creating the
//!   marker).
//! - **LegacyBesideExe** — a `data/` dir sits beside the exe but there is no
//!   marker (pre-v0.706 installs, and the operator's dad). Behaves EXACTLY
//!   like before this feature: game data reads beside the exe, while saves/
//!   config/logs stay in the OS dir where such installs already have them.
//!   Nothing strands, nothing moves.
//! - **Installed** — the OS per-user dir (%APPDATA%\HumanityOS on Windows,
//!   XDG / Application Support elsewhere) holds real content. Writable state
//!   lives there; the exe can sit anywhere, move, update, or be deleted
//!   without touching the user's files.
//! - **Undecided** — a truly fresh machine: nothing beside the exe, nothing
//!   in the OS dir. The main menu shows the "where should HumanityOS keep
//!   your files?" chooser BEFORE identity creation, so nothing is written
//!   until the user decides. (GUI-first rule: the choice is an in-app step,
//!   never a CLI flag.)
//!
//! Detection checks for meaningful CONTENT (config.json / data/ / saves/),
//! not the bare OS root dir — `AppConfig::config_path()` historically
//! `create_dir_all`s the root as a side effect, and an empty root must not
//! flip a fresh machine to Installed.

#![cfg(feature = "native")]

use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// The four storage states. See the module docs for what each means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Portable,
    LegacyBesideExe,
    Installed,
    Undecided,
}

/// The portable-mode marker filename, beside the exe.
pub const PORTABLE_MARKER: &str = "portable.txt";

static MODE: RwLock<Option<StorageMode>> = RwLock::new(None);

/// Directory the running exe lives in (`.` when unresolvable).
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The OS per-user root for HumanityOS (`%APPDATA%\HumanityOS` etc.),
/// WITHOUT any subdir. None when the platform env vars are absent.
pub fn os_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return std::env::var("APPDATA")
            .ok()
            .map(|a| PathBuf::from(a).join("HumanityOS"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("HumanityOS")
        });
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return Some(PathBuf::from(xdg).join("HumanityOS"));
        }
        return std::env::var("HOME").ok().map(|h| {
            PathBuf::from(h).join(".local").join("share").join("HumanityOS")
        });
    }
    #[allow(unreachable_code)]
    None
}

/// Pure detection given the two roots — unit-tested below.
fn detect(exe_dir: &Path, os_root: Option<&Path>) -> StorageMode {
    if exe_dir.join(PORTABLE_MARKER).exists() {
        return StorageMode::Portable;
    }
    if exe_dir.join("data").is_dir() {
        return StorageMode::LegacyBesideExe;
    }
    if let Some(root) = os_root {
        // Meaningful content only — the bare root dir may exist as a
        // config_path() side effect on a machine that never chose.
        if root.join("config.json").exists()
            || root.join("data").is_dir()
            || root.join("saves").is_dir()
        {
            return StorageMode::Installed;
        }
    }
    StorageMode::Undecided
}

/// The resolved storage mode, detected once and cached. `choose_*` updates it.
pub fn mode() -> StorageMode {
    if let Some(m) = *MODE.read().unwrap() {
        return m;
    }
    let m = detect(&exe_dir(), os_root().as_deref());
    *MODE.write().unwrap() = Some(m);
    m
}

/// First-boot choice: keep files in the OS per-user dir. Creates the root and
/// extracts the editable data copy there.
pub fn choose_installed() {
    if let Some(root) = os_root() {
        let _ = std::fs::create_dir_all(&root);
    }
    *MODE.write().unwrap() = Some(StorageMode::Installed);
    extract_data_if_needed();
    log::info!("Storage mode chosen: Installed ({:?})", os_root());
}

/// First-boot choice: portable — everything beside the exe. Writes the marker
/// and extracts the editable data copy there.
pub fn choose_portable() {
    let marker = exe_dir().join(PORTABLE_MARKER);
    let _ = std::fs::write(
        &marker,
        "HumanityOS portable mode.\n\
         This file tells HumanityOS to keep ALL of your files (game data,\n\
         saves, settings, identity, logs) in this folder, next to the app,\n\
         so the whole folder travels between machines (USB drive friendly).\n\
         Delete this file and restart to switch back to per-user storage.\n",
    );
    *MODE.write().unwrap() = Some(StorageMode::Portable);
    extract_data_if_needed();
    log::info!("Storage mode chosen: Portable ({})", exe_dir().display());
}

/// Where the encrypted-identity config.json lives for the current mode.
/// `None` = caller should use its OS-dir logic (Installed/Legacy/Undecided).
pub fn portable_config_path() -> Option<PathBuf> {
    (mode() == StorageMode::Portable).then(|| exe_dir().join("config.json"))
}

/// Saves dir override for Portable mode (else caller uses the OS dir).
pub fn portable_saves_dir() -> Option<PathBuf> {
    (mode() == StorageMode::Portable).then(|| exe_dir().join("saves"))
}

/// Logs dir override for Portable mode (else caller uses the OS dir).
pub fn portable_logs_dir() -> Option<PathBuf> {
    (mode() == StorageMode::Portable).then(|| exe_dir().join("logs"))
}

/// The writable game-data dir for the current mode (extraction target and
/// editor-save target). None while Undecided (nothing may be written yet)
/// or when the OS root is unresolvable.
pub fn writable_data_dir() -> Option<PathBuf> {
    match mode() {
        StorageMode::Portable | StorageMode::LegacyBesideExe => Some(exe_dir().join("data")),
        StorageMode::Installed => os_root().map(|r| r.join("data")),
        StorageMode::Undecided => None,
    }
}

/// Extract the embedded, editable game-data files to the mode's writable data
/// dir on first run (no-op if it already exists, if Undecided, or if the OS
/// root is unresolvable — reads fall back to the embedded copies either way).
/// This is what enables file-based modding without ever littering a folder
/// the user didn't choose (pre-v0.706 this dumped ~70 files beside the exe).
pub fn extract_data_if_needed() {
    let data_dir = match writable_data_dir() {
        Some(d) => d,
        None => return,
    };
    if data_dir.exists() {
        return;
    }
    log::info!("First run: extracting editable game data to {:?}", data_dir);

    // All embedded files with their relative paths.
    // NOTE: this hand-maintained list has drifted from the full data set
    // (e.g. status_effects.csv, containers/, food_system.ron are loaded at
    // runtime but not listed here) — distributed-build completeness is
    // tracked as a follow-up to derive this list from embedded_data's keys.
    // Dev runs are unaffected (find_data_dir prefers the live repo data),
    // and reads always fall back to the embedded copies.
    let files: &[&str] = &[
        "items.csv", "recipes.csv", "materials.csv", "components.csv",
        "plants.csv", "game.csv", "skills/skills.csv",
        "chemistry/elements.csv", "chemistry/alloys.csv",
        "chemistry/compounds.csv", "chemistry/gases.csv", "chemistry/toxins.csv",
        "asteroids/types.csv",
        "glossary.json", "solar_system/bodies.json", "solar-system.json",
        "tools/catalog.json", "cities.json", "coastlines.json",
        "constellations.json", "milky-way.json", "stars-catalog.json",
        "stars-nearby.json",
        "config.toml", "calendar.toml", "input.toml", "player.toml",
        "gui/theme.ron",
        "planets/earth.ron", "planets/mars.ron", "planets/moon.ron",
        "solar_system/earth.ron", "solar_system/mars.ron", "solar_system/sun.ron",
        "ships/bridge.ron", "ships/layout_medium.ron", "ships/reactor.ron",
        "ships/starter_fleet.ron",
        "quests/construction.ron", "quests/exploration.ron",
        "quests/farming.ron", "quests/tutorial.ron", "quests/getting_started.ron",
        "blueprints/basic.ron", "blueprints/construction.ron",
        "blueprints/habitat.ron", "blueprints/materials.ron",
        "blueprints/objects.ron",
        "entities/human/human_001.ron", "entities/plants/plant_001.ron",
        "entities/plants/tomato.ron", "entities/substrates/loam_basic.ron",
        "entities/substrates/substrate_001.ron",
        "plots/plot_001.ron",
        "world/solar_system.ron", "world/spawn.ron", "world/player.ron",
        "resources/fertilizer_basic.ron", "resources/water_clean.ron",
        "i18n/en.json", "i18n/es.json", "i18n/fr.json",
        "i18n/ja.json", "i18n/zh.json",
        "language/acronyms.json", "language/dictionary.json",
        "language/parts_of_speech.json",
    ];

    for relative_path in files {
        if let Some(content) = crate::embedded_data::get_embedded(relative_path) {
            let file_path = data_dir.join(relative_path);
            if let Some(parent) = file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&file_path, content) {
                log::warn!("Failed to extract {}: {e}", relative_path);
            }
        }
    }
    log::info!("Data extraction complete");
}

// ── Storage-mode MIGRATION (v0.742, the "move my files" Settings tool) ──
//
// Safety contract (this moves the encrypted identity, so it is copy-first,
// commit-last, delete-never):
//   1. Every file is COPIED before anything about the mode changes. A failed
//      copy aborts with the install untouched.
//   2. The mode "commit" (writing / removing portable.txt) happens LAST.
//   3. Source files are NEVER deleted. Portable -> per-user renames the
//      exe-side `data` dir to `data-backup[-N]` (rename, not delete) purely so
//      next-boot detection lands on Installed instead of LegacyBesideExe;
//      everything else stays in place as an inert backup.

/// Recursively copy a directory tree (regular files only). Returns the number
/// of files copied. A missing source is Ok(0) — "nothing to move" is a valid
/// migration state, not an error.
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<u32> {
    if !src.exists() {
        return Ok(0);
    }
    let mut copied = 0u32;
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copied += copy_tree(&entry.path(), &to)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &to)?;
            copied += 1;
        }
    }
    Ok(copied)
}

/// First free `<base>-backup[-N]` name beside `base` (never overwrites).
fn unique_backup_name(base: &Path) -> PathBuf {
    let first = base.with_file_name(format!(
        "{}-backup",
        base.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    ));
    if !first.exists() {
        return first;
    }
    for n in 1.. {
        let candidate = base.with_file_name(format!(
            "{}-backup-{n}",
            base.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// The per-user subtrees a migration carries (config.json is handled apart).
const MIGRATE_DIRS: &[&str] = &["data", "saves", "logs", "backups", "cache"];

/// Copy config.json (the ENCRYPTED IDENTITY) preserving any existing
/// destination copy as `config.json-backup[-N]` first — an identity file is
/// never overwritten, period, even though the source is always the live copy
/// (adversarial-review hardening, 2026-07-07).
fn copy_identity(src: &Path, dst: &Path) -> Result<u32, String> {
    if !src.is_file() {
        return Ok(0);
    }
    if dst.is_file() {
        let backup = unique_backup_name(dst);
        std::fs::rename(dst, &backup)
            .map_err(|e| format!("could not back up the existing {}: {e}", dst.display()))?;
    }
    std::fs::copy(src, dst).map_err(|e| e.to_string())?;
    Ok(1)
}

/// Switch a per-user (or legacy) install to PORTABLE: copy the install beside
/// the exe, then write the marker LAST. Originals stay in place as a backup.
/// Err = nothing was committed (the marker is only written on full success).
pub fn migrate_to_portable() -> Result<String, String> {
    let exe = exe_dir();
    let mut copied = 0u32;
    match mode() {
        StorageMode::Portable => return Err("Already in portable mode.".into()),
        StorageMode::Undecided => {
            return Err("Choose a storage location first (first-boot chooser).".into())
        }
        StorageMode::Installed => {
            let root = os_root().ok_or("No per-user folder resolvable on this system.")?;
            copied += copy_identity(&root.join("config.json"), &exe.join("config.json"))?;
            for d in MIGRATE_DIRS {
                copied += copy_tree(&root.join(d), &exe.join(d)).map_err(|e| e.to_string())?;
            }
        }
        StorageMode::LegacyBesideExe => {
            // Live game data already sits beside the exe — copy only the
            // per-user pieces (identity/saves/logs), and NOT the per-user
            // `data` dir, which would clobber live modded files with stale ones.
            if let Some(root) = os_root() {
                copied += copy_identity(&root.join("config.json"), &exe.join("config.json"))?;
                for d in MIGRATE_DIRS.iter().filter(|d| **d != "data") {
                    copied += copy_tree(&root.join(d), &exe.join(d)).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    // Commit LAST: the marker only appears once every copy above succeeded.
    std::fs::write(
        exe.join(PORTABLE_MARKER),
        "HumanityOS portable mode.\n\
         This file tells HumanityOS to keep ALL of your files (game data,\n\
         saves, settings, identity, logs) in this folder, next to the app,\n\
         so the whole folder travels between machines (USB drive friendly).\n\
         Delete this file and restart to switch back to per-user storage.\n",
    )
    .map_err(|e| format!("Copies succeeded but writing the portable marker failed: {e}"))?;
    *MODE.write().unwrap() = Some(StorageMode::Portable);
    log::info!("Storage migrated to Portable ({copied} files copied)");
    Ok(format!(
        "Copied {copied} files next to the app. Your old copies remain in the user folder as a backup. Restart HumanityOS to finish."
    ))
}

/// Switch a PORTABLE install back to per-user: copy everything into the OS
/// per-user folder, rename the exe-side `data` dir to a backup name (so
/// next-boot detection lands on Installed, not LegacyBesideExe), then remove
/// the marker LAST. Nothing is deleted.
pub fn migrate_to_per_user() -> Result<String, String> {
    if mode() != StorageMode::Portable {
        return Err("Not in portable mode.".into());
    }
    let exe = exe_dir();
    let root = os_root().ok_or("No per-user folder resolvable on this system.")?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let mut copied = copy_identity(&exe.join("config.json"), &root.join("config.json"))?;
    for d in MIGRATE_DIRS {
        copied += copy_tree(&exe.join(d), &root.join(d)).map_err(|e| e.to_string())?;
    }
    // Rename the exe-side data dir out of detection's way (rename, NOT delete).
    let exe_data = exe.join("data");
    let mut data_backup: Option<PathBuf> = None;
    if exe_data.is_dir() {
        let backup = unique_backup_name(&exe_data);
        std::fs::rename(&exe_data, &backup).map_err(|e| e.to_string())?;
        data_backup = Some(backup);
    }
    // Commit LAST: remove the marker; on failure, undo the rename so the
    // install stays fully portable rather than half-switched.
    if let Err(e) = std::fs::remove_file(exe.join(PORTABLE_MARKER)) {
        if let Some(backup) = data_backup {
            let _ = std::fs::rename(&backup, &exe_data);
        }
        return Err(format!("Copies succeeded but removing the portable marker failed: {e}"));
    }
    *MODE.write().unwrap() = Some(StorageMode::Installed);
    log::info!("Storage migrated to per-user ({copied} files copied)");
    Ok(format!(
        "Copied {copied} files to your user folder. The app-side copies remain as a backup. Restart HumanityOS to finish."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hos_storage_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn copy_tree_copies_nested_files_and_counts_them() {
        let src = tmp("ct_src");
        let dst = tmp("ct_dst");
        std::fs::create_dir_all(src.join("a/b")).unwrap();
        std::fs::write(src.join("top.txt"), "1").unwrap();
        std::fs::write(src.join("a/mid.txt"), "22").unwrap();
        std::fs::write(src.join("a/b/deep.txt"), "333").unwrap();
        let n = copy_tree(&src, &dst).unwrap();
        assert_eq!(n, 3);
        assert_eq!(std::fs::read_to_string(dst.join("a/b/deep.txt")).unwrap(), "333");
        // Source untouched (copy, not move).
        assert!(src.join("top.txt").exists());
        // Missing source is Ok(0), not an error.
        assert_eq!(copy_tree(&src.join("nope"), &dst).unwrap(), 0);
    }

    #[test]
    fn copy_identity_backs_up_the_destination_before_overwriting() {
        let d = tmp("ci");
        let src = d.join("src_config.json");
        let dst = d.join("config.json");
        std::fs::write(&src, "live-identity").unwrap();
        std::fs::write(&dst, "old-identity").unwrap();
        assert_eq!(copy_identity(&src, &dst).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "live-identity");
        // The pre-existing destination copy survives under a backup name.
        let backup = d.join("config.json-backup");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "old-identity");
        // Missing source = Ok(0), destination untouched.
        assert_eq!(copy_identity(&d.join("nope.json"), &dst).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "live-identity");
    }

    #[test]
    fn unique_backup_name_never_collides() {
        let d = tmp("ubn");
        let base = d.join("data");
        std::fs::create_dir_all(&base).unwrap();
        let first = unique_backup_name(&base);
        assert!(first.ends_with("data-backup"));
        std::fs::create_dir_all(&first).unwrap();
        let second = unique_backup_name(&base);
        assert!(second.ends_with("data-backup-1"));
        std::fs::create_dir_all(&second).unwrap();
        assert!(unique_backup_name(&base).ends_with("data-backup-2"));
    }

    #[test]
    fn fresh_machine_is_undecided() {
        let exe = tmp("fresh_exe");
        let os = tmp("fresh_os"); // exists but EMPTY (config_path side effect)
        assert_eq!(detect(&exe, Some(&os)), StorageMode::Undecided);
        assert_eq!(detect(&exe, None), StorageMode::Undecided);
    }

    #[test]
    fn marker_beside_exe_is_portable_and_beats_everything() {
        let exe = tmp("marker_exe");
        std::fs::write(exe.join(PORTABLE_MARKER), "x").unwrap();
        // Even with legacy data beside the exe AND installed content, the
        // explicit marker wins.
        std::fs::create_dir_all(exe.join("data")).unwrap();
        let os = tmp("marker_os");
        std::fs::write(os.join("config.json"), "{}").unwrap();
        assert_eq!(detect(&exe, Some(&os)), StorageMode::Portable);
    }

    #[test]
    fn data_beside_exe_without_marker_is_legacy() {
        let exe = tmp("legacy_exe");
        std::fs::create_dir_all(exe.join("data")).unwrap();
        // Legacy wins over Installed content (the dad case: beside-exe data
        // from a pre-v0.706 extraction + APPDATA config from normal use).
        let os = tmp("legacy_os");
        std::fs::write(os.join("config.json"), "{}").unwrap();
        assert_eq!(detect(&exe, Some(&os)), StorageMode::LegacyBesideExe);
    }

    #[test]
    fn os_root_content_is_installed() {
        let exe = tmp("inst_exe");
        for content in ["config.json", "data", "saves"] {
            let os = tmp(&format!("inst_os_{content}"));
            if content == "config.json" {
                std::fs::write(os.join(content), "{}").unwrap();
            } else {
                std::fs::create_dir_all(os.join(content)).unwrap();
            }
            assert_eq!(detect(&exe, Some(&os)), StorageMode::Installed, "content: {content}");
        }
    }
}
