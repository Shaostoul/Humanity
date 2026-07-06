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
