//! Sound catalog -- data-driven sound definitions loaded from data/sounds.toml.
//!
//! Provides `SoundCatalog` for looking up sounds by ID (e.g. "sfx.footstep_grass"),
//! with hardcoded fallback paths for core sounds if the TOML fails to load.

use std::collections::HashMap;

// ── Fallback constants (used when sounds.toml is missing or a key isn't found) ──

// UI sounds
pub const UI_CLICK: &str = "data/audio/ui/click.ogg";
pub const UI_HOVER: &str = "data/audio/ui/hover.ogg";
pub const UI_OPEN: &str = "data/audio/ui/open.ogg";
pub const UI_CLOSE: &str = "data/audio/ui/close.ogg";

// Footsteps
pub const FOOTSTEP_GRASS: &str = "data/audio/sfx/footstep_grass.ogg";
pub const FOOTSTEP_STONE: &str = "data/audio/sfx/footstep_stone.ogg";
pub const FOOTSTEP_METAL: &str = "data/audio/sfx/footstep_metal.ogg";
pub const FOOTSTEP_DIRT: &str = "data/audio/sfx/footstep_dirt.ogg";
pub const FOOTSTEP_WOOD: &str = "data/audio/sfx/footstep_wood.ogg";

// Actions
pub const MINING_HIT: &str = "data/audio/sfx/mining_hit.ogg";
pub const HARVEST: &str = "data/audio/sfx/harvest.ogg";
pub const CRAFT_COMPLETE: &str = "data/audio/sfx/craft_complete.ogg";
pub const BUILD_PLACE: &str = "data/audio/sfx/build_place.ogg";
pub const ITEM_PICKUP: &str = "data/audio/sfx/item_pickup.ogg";

// Ambient
pub const AMBIENT_FOREST: &str = "data/audio/ambient/forest.ogg";
pub const AMBIENT_SPACE: &str = "data/audio/ambient/space.ogg";
pub const AMBIENT_SHIP: &str = "data/audio/ambient/ship_interior.ogg";
pub const AMBIENT_RAIN: &str = "data/audio/ambient/rain.ogg";
pub const AMBIENT_WIND: &str = "data/audio/ambient/wind.ogg";
pub const AMBIENT_NIGHT: &str = "data/audio/ambient/night_crickets.ogg";

// Music
pub const MUSIC_MENU: &str = "data/audio/music/menu.ogg";
pub const MUSIC_EXPLORATION: &str = "data/audio/music/exploration.ogg";
pub const MUSIC_COMBAT: &str = "data/audio/music/combat.ogg";
pub const MUSIC_BUILDING: &str = "data/audio/music/building.ogg";
pub const MUSIC_PEACEFUL: &str = "data/audio/music/peaceful.ogg";

// ── Data-driven sound catalog ──

/// A single sound entry parsed from sounds.toml.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SoundEntry {
    /// Relative path to the audio file (e.g. "audio/sfx/footstep_grass.ogg").
    pub path: String,
    /// Base volume (0.0 to 1.0).
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Whether this sound should loop.
    #[serde(rename = "loop", default)]
    pub loop_enabled: bool,
    /// Whether this is a spatial (3D positioned) sound.
    #[serde(default)]
    pub spatial: bool,
    /// Minimum falloff distance for spatial sounds.
    #[serde(default)]
    pub falloff_min: f32,
    /// Maximum falloff distance for spatial sounds.
    #[serde(default = "default_falloff_max")]
    pub falloff_max: f32,
    /// Audio bus: "ambient", "music", "sfx", "voice", "ui".
    #[serde(default = "default_bus")]
    pub bus: String,
    /// Alternative audio file variations for randomization.
    #[serde(default)]
    pub variations: Vec<String>,
    /// Tags for filtering/querying sounds.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_volume() -> f32 { 0.5 }
fn default_falloff_max() -> f32 { 20.0 }
fn default_bus() -> String { "sfx".into() }

/// The sound catalog: maps dotted IDs (e.g. "sfx.footstep_grass") to SoundEntry.
#[derive(Debug, Clone)]
pub struct SoundCatalog {
    entries: HashMap<String, SoundEntry>,
}

impl SoundCatalog {
    /// Load the sound catalog from data/sounds.toml under the given data directory.
    /// Returns an empty catalog on any error (graceful degradation).
    pub fn load(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("sounds.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[sounds] Failed to read {}: {}", path.display(), e);
                return Self { entries: HashMap::new() };
            }
        };

        // sounds.toml has nested tables like [ambient.forest_day], [sfx.footstep_grass].
        // Parse as a generic TOML table, then walk the two-level structure.
        let table: toml::Table = match text.parse() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[sounds] Failed to parse sounds.toml: {}", e);
                return Self { entries: HashMap::new() };
            }
        };

        let mut entries = HashMap::new();
        for (bus_key, bus_val) in &table {
            if let Some(bus_table) = bus_val.as_table() {
                for (sound_key, sound_val) in bus_table {
                    let id = format!("{}.{}", bus_key, sound_key);
                    match sound_val.clone().try_into::<SoundEntry>() {
                        Ok(entry) => { entries.insert(id, entry); }
                        Err(e) => {
                            eprintln!("[sounds] Failed to parse {}: {}", id, e);
                        }
                    }
                }
            }
        }

        eprintln!("[sounds] Loaded {} sound entries from sounds.toml", entries.len());
        Self { entries }
    }

    /// Look up a sound by dotted ID (e.g. "sfx.footstep_grass").
    pub fn get(&self, id: &str) -> Option<&SoundEntry> {
        self.entries.get(id)
    }

    /// Get the path for a sound ID, or return the fallback path if not found.
    pub fn path_or(&self, id: &str, fallback: &str) -> String {
        self.entries
            .get(id)
            .map(|e| e.path.clone())
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Get all sound IDs in this catalog.
    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Get all entries in this catalog.
    pub fn entries(&self) -> &HashMap<String, SoundEntry> {
        &self.entries
    }

    /// Number of loaded sound entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the catalog is empty (no sounds loaded).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for SoundCatalog {
    fn default() -> Self {
        Self { entries: HashMap::new() }
    }
}

/// Surface type for selecting footstep sounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceType {
    Grass,
    Stone,
    Metal,
    Dirt,
    Wood,
}

impl SurfaceType {
    /// Get the footstep sound path from the catalog, falling back to hardcoded constants.
    pub fn footstep_sound_from(&self, catalog: &SoundCatalog) -> String {
        match self {
            SurfaceType::Grass => catalog.path_or("sfx.footstep_grass", FOOTSTEP_GRASS),
            SurfaceType::Stone => catalog.path_or("sfx.footstep_stone", FOOTSTEP_STONE),
            SurfaceType::Metal => catalog.path_or("sfx.footstep_metal", FOOTSTEP_METAL),
            SurfaceType::Dirt => catalog.path_or("sfx.footstep_dirt", FOOTSTEP_DIRT),
            SurfaceType::Wood => catalog.path_or("sfx.footstep_wood", FOOTSTEP_WOOD),
        }
    }

    /// Legacy: get the hardcoded fallback path (no catalog needed).
    pub fn footstep_sound(&self) -> &'static str {
        match self {
            SurfaceType::Grass => FOOTSTEP_GRASS,
            SurfaceType::Stone => FOOTSTEP_STONE,
            SurfaceType::Metal => FOOTSTEP_METAL,
            SurfaceType::Dirt => FOOTSTEP_DIRT,
            SurfaceType::Wood => FOOTSTEP_WOOD,
        }
    }
}
