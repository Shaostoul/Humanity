//! World persistence — save/load game state to JSON files.
//!
//! Save directory: platform data dir + "/saves/"
//! - Windows: `%APPDATA%/HumanityOS/saves/`
//! - Linux:   `$XDG_DATA_HOME/HumanityOS/saves/` or `~/.local/share/HumanityOS/saves/`
//! - macOS:   `~/Library/Application Support/HumanityOS/saves/`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Complete world save state. A "home" (the homes-as-save-profiles model, v0.380)
/// IS a WorldSave that knows its `kind` + `design`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSave {
    pub name: String,
    pub timestamp: u64,
    pub game_time: f64,
    pub player_position: [f32; 3],
    pub player_rotation: [f32; 4],
    pub player_health: f32,
    pub inventory: Vec<(String, u32)>,
    pub skills: HashMap<String, (u32, u32)>,
    pub constructions: Vec<ConstructionSave>,
    pub weather_state: String,
    /// What this home IS (the "who owns the truth" axis): "offline" (you own the
    /// local save), "server" (a relay owns it), or "real" (physical sensors own
    /// it). Defaults to "offline" so pre-v0.380 saves load unchanged. Server + Real
    /// are deferred; offline is the only live kind today.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// The Design (blueprint) this home is built from, e.g. "fibonacci". Defaults
    /// to "fibonacci" (the first + only design today).
    #[serde(default = "default_design")]
    pub design: String,
    /// The GAME character's name (v0.448), DECOUPLED from the chat/network profile name
    /// (which is the Dilithium identity). A character lives in the save (self-custodial,
    /// local), separate from who you are in chat. serde-default for old saves.
    #[serde(default = "default_character_name")]
    pub character_name: String,
    /// The player's avatar appearance (v0.440). serde-default so pre-v0.440 saves load
    /// unchanged (they get the default blockman).
    #[serde(default)]
    pub appearance: crate::ecs::components::Appearance,
    /// The player's equipped cosmetic outfit (v0.440). serde-default for old saves.
    #[serde(default)]
    pub outfit: crate::ecs::components::Outfit,
    /// The organize-layer container contents (v0.516): every item that lives in a
    /// container other than the live backpack, tagged with its container path. The
    /// backpack itself is `inventory` above. serde-default so pre-v0.517 saves load
    /// with an empty pool (the inventory then re-seeds from data/places/seed.json).
    #[serde(default)]
    pub placed_items: Vec<crate::gui::PlacedItem>,
}

fn default_kind() -> String {
    "offline".to_string()
}
fn default_design() -> String {
    "fibonacci".to_string()
}
fn default_character_name() -> String {
    "Wanderer".to_string()
}

impl WorldSave {
    /// A fresh OFFLINE home built from the given design, with empty progress. The
    /// "save wrapper" entry point: a home is a WorldSave that knows its kind +
    /// design. `timestamp` is left 0; the save flow stamps it when the file is
    /// written.
    pub fn new_offline(name: impl Into<String>, design: impl Into<String>) -> Self {
        WorldSave {
            name: name.into(),
            timestamp: 0,
            game_time: 0.0,
            player_position: [0.0, 0.0, 0.0],
            player_rotation: [0.0, 0.0, 0.0, 1.0],
            player_health: 100.0,
            inventory: Vec::new(),
            skills: HashMap::new(),
            constructions: Vec::new(),
            weather_state: "clear".to_string(),
            kind: "offline".to_string(),
            design: design.into(),
            character_name: default_character_name(),
            appearance: Default::default(),
            outfit: Default::default(),
            placed_items: Vec::new(),
        }
    }
}

/// A saved construction/building in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructionSave {
    pub blueprint_id: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub health: f32,
}

/// Get the platform-appropriate saves directory.
///
/// Falls back to `./saves/` if the platform data directory cannot be determined.
pub fn saves_dir() -> PathBuf {
    // Try standard platform data dirs without adding a dependency.
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata)
                .join("HumanityOS")
                .join("saves");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("HumanityOS")
                .join("saves");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg)
                .join("HumanityOS")
                .join("saves");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("HumanityOS")
                .join("saves");
        }
    }

    PathBuf::from("saves")
}

/// Save the world state to a JSON file at the given path.
pub fn save_world(path: &Path, save: &WorldSave) -> Result<(), String> {
    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create save directory: {e}"))?;
    }

    let json = serde_json::to_string_pretty(save)
        .map_err(|e| format!("Failed to serialize world save: {e}"))?;

    std::fs::write(path, json)
        .map_err(|e| format!("Failed to write save file: {e}"))?;

    log::info!("World saved to {}", path.display());
    Ok(())
}

/// Load a world save from a JSON file at the given path.
pub fn load_world(path: &Path) -> Result<WorldSave, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read save file: {e}"))?;

    let save: WorldSave = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialize world save: {e}"))?;

    log::info!("World loaded from {}", path.display());
    Ok(save)
}

/// List all save files in the saves directory with their names and timestamps.
///
/// Returns a list of `(name, timestamp)` pairs sorted by timestamp descending
/// (most recent first).
pub fn list_saves(saves_dir: &Path) -> Vec<(String, u64)> {
    let entries = match std::fs::read_dir(saves_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut saves: Vec<(String, u64)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let path = e.path();
            let json = std::fs::read_to_string(&path).ok()?;
            let save: WorldSave = serde_json::from_str(&json).ok()?;
            Some((save.name, save.timestamp))
        })
        .collect();

    // Sort by timestamp, most recent first.
    saves.sort_by(|a, b| b.1.cmp(&a.1));
    saves
}

/// Auto-save the world to `auto_save.json` in the given directory.
pub fn auto_save(saves_dir: &Path, save: &WorldSave) {
    let path = saves_dir.join("auto_save.json");
    match save_world(&path, save) {
        Ok(_) => log::info!("Auto-save complete"),
        Err(e) => log::error!("Auto-save failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_save() -> WorldSave {
        WorldSave {
            name: "Test Save".to_string(),
            timestamp: 1700000000,
            game_time: 3600.0,
            player_position: [10.0, 5.0, -3.0],
            player_rotation: [0.0, 0.0, 0.0, 1.0],
            player_health: 100.0,
            inventory: vec![
                ("wood".to_string(), 50),
                ("stone".to_string(), 25),
            ],
            skills: {
                let mut m = HashMap::new();
                m.insert("mining".to_string(), (5, 1200));
                m.insert("farming".to_string(), (3, 450));
                m
            },
            constructions: vec![
                ConstructionSave {
                    blueprint_id: "wooden_wall".to_string(),
                    position: [20.0, 0.0, 15.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    health: 100.0,
                },
            ],
            weather_state: "clear".to_string(),
            kind: "offline".to_string(),
            design: "fibonacci".to_string(),
            character_name: "Test Character".to_string(),
            appearance: Default::default(),
            outfit: Default::default(),
            placed_items: Vec::new(),
        }
    }

    #[test]
    fn new_offline_has_defaults() {
        let h = WorldSave::new_offline("My Homestead", "fibonacci");
        assert_eq!(h.kind, "offline");
        assert_eq!(h.design, "fibonacci");
        assert_eq!(h.name, "My Homestead");
        assert!(h.inventory.is_empty());
    }

    #[test]
    fn round_trip_save_load() {
        let dir = std::env::temp_dir().join("humanity_test_saves");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_roundtrip.json");

        let save = test_save();
        save_world(&path, &save).expect("save should succeed");
        let loaded = load_world(&path).expect("load should succeed");

        assert_eq!(loaded.name, save.name);
        assert_eq!(loaded.timestamp, save.timestamp);
        assert!((loaded.game_time - save.game_time).abs() < f64::EPSILON);
        assert_eq!(loaded.inventory.len(), save.inventory.len());
        assert_eq!(loaded.constructions.len(), save.constructions.len());
        assert_eq!(loaded.kind, save.kind);
        assert_eq!(loaded.design, save.design);

        // Clean up
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn legacy_save_without_kind_design_defaults() {
        // A pre-v0.380 save has no kind/design fields; serde defaults must fill them
        // (kind=offline, design=fibonacci) so old saves load unchanged.
        let dir = std::env::temp_dir().join("humanity_test_legacy_save");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("legacy.json");
        let legacy = r#"{"name":"Old","timestamp":1,"game_time":0.0,"player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],"player_health":100.0,"inventory":[],"skills":{},"constructions":[],"weather_state":"clear"}"#;
        std::fs::write(&path, legacy).unwrap();
        let loaded = load_world(&path).expect("legacy load should succeed");
        assert_eq!(loaded.kind, "offline");
        assert_eq!(loaded.design, "fibonacci");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn list_saves_works() {
        let dir = std::env::temp_dir().join("humanity_test_list_saves");
        let _ = std::fs::create_dir_all(&dir);

        let mut s1 = test_save();
        s1.name = "Save One".to_string();
        s1.timestamp = 1000;
        save_world(&dir.join("save1.json"), &s1).unwrap();

        let mut s2 = test_save();
        s2.name = "Save Two".to_string();
        s2.timestamp = 2000;
        save_world(&dir.join("save2.json"), &s2).unwrap();

        let saves = list_saves(&dir);
        assert_eq!(saves.len(), 2);
        // Most recent first
        assert_eq!(saves[0].0, "Save Two");
        assert_eq!(saves[1].0, "Save One");

        // Clean up
        let _ = std::fs::remove_file(dir.join("save1.json"));
        let _ = std::fs::remove_file(dir.join("save2.json"));
        let _ = std::fs::remove_dir(&dir);
    }
}
