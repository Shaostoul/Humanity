//! Every `data/` file the GUI reads, and the shapes it reads them into.
//!
//! One job, one input, one failure mode: each function here takes the resolved
//! data directory, parses one file, and returns an empty or default value if
//! anything goes wrong, because a missing data file must never stop the app
//! booting. The structs interleaved with them are the outputs of those parses
//! and nothing else, which is why they travel with the loaders rather than with
//! the rest of the GUI's value types.
//!
//! This is also where the infinite-of-x rule is actually enforced: a tower
//! config, a grow medium, a library section, a donation method and a market
//! category are all rows in a file rather than arms of a match, and adding one
//! is an edit to `data/`, not to Rust. Several of these files are read by the
//! WEB client too (the external catalog, the library manifest, the websites
//! database), which is what keeps the two clients from drifting into different
//! products; `tests/page_parity_lint.rs` checks exactly that and now looks in
//! this file for the native half.
//!
//! Extracted VERBATIM from `src/gui/mod.rs` (file-size ratchet), which stood at
//! 7,915 lines against a 7,050 budget. First of two clusters out in that pass.
//! Nothing here changed shape at all: no signature, no visibility, not one
//! attribute. The module is declared UNGATED because its contents are mixed:
//! about half carry `#[cfg(feature = "native")]` and half do not, and each item
//! keeps exactly the attributes it had, so both feature sets see what they saw.
//!
//! WHAT STAYED BEHIND, and why: `default_water_clarity`, `default_precip_density`
//! and `default_fog_density` sit in the middle of this run but are serde
//! `default = "..."` targets for `SettingsState`, which stays in `gui/mod.rs`.
//! A serde default resolves as a path in the scope of the struct that names it,
//! so moving those three would have meant rewriting the attributes, which is a
//! change rather than a motion.
//!
//! Takes `use super::*` the way the chat page's children do, so the GUI value
//! types these loaders fill in (`ToolEntry`, `GuiPlanet`, `GuiRecipe`, the
//! studio types) arrive without a single one of them being widened.

use super::*;

/// Load the external catalog from `data/external/catalog.json`: free software
/// (`kind == "software"`) and real-world help services (`kind == "service"`), in
/// one flat list tagged with its category and kind. The web Tools page reads the
/// same file, which is what keeps the two clients in step.
/// `data_dir` is the root data directory (e.g. from AssetManager).
/// Returns an empty Vec on any error (graceful degradation).
#[cfg(feature = "native")]
pub fn load_tools_catalog(data_dir: &std::path::Path) -> Vec<ToolEntry> {
    /// JSON shape for `data/external/catalog.json`: categories, each tagged
    /// with a kind, each holding entries.
    #[derive(serde::Deserialize)]
    struct Catalog {
        categories: Vec<CatalogCategory>,
    }
    #[derive(serde::Deserialize)]
    struct CatalogCategory {
        name: String,
        /// "software" or "service"; see the `kinds` array in the file.
        #[serde(default)]
        kind: String,
        #[serde(default)]
        entries: Vec<ToolEntry>,
        #[allow(dead_code)]
        #[serde(default)]
        id: String,
        #[allow(dead_code)]
        #[serde(default)]
        extensions: Vec<String>,
    }

    let path = data_dir.join("external").join("catalog.json");
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[external] Failed to read {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    let catalog: Catalog = match serde_json::from_str(&bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[external] Failed to parse external/catalog.json: {}", e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for cat in catalog.categories {
        for mut entry in cat.entries {
            entry.category = cat.name.clone();
            entry.kind = cat.kind.clone();
            out.push(entry);
        }
    }
    out
}

/// Build the Maps page's planet list from the cosmos catalog.
///
/// This used to parse `data/solar_system/bodies.json`, a file that stopped
/// shipping long ago, so the native Maps page's planet list rendered EMPTY
/// on every fresh checkout while the error line scrolled past unnoticed
/// (found by the 2026-08-12 planet-physics audit). The cosmos catalog
/// (`data/star_systems/sol.json`, parsed once by `cosmos::sol_bodies`) is
/// the single source of truth for body facts, so build from it directly:
/// one parse, no duplicated schema to drift.
#[cfg(feature = "native")]
pub fn load_planets() -> Vec<GuiPlanet> {
    crate::cosmos::sol_bodies()
        .iter()
        .filter(|b| {
            // Planets and dwarf planets only: the Maps page draws the
            // simple heliocentric view, so the Sun, moons, asteroids and
            // comets are skipped (moons still show as each planet's count).
            matches!(
                b.body_type.as_str(),
                "terrestrial" | "gas_giant" | "ice_giant" | "dwarf_planet" | "artificial"
            )
        })
        .map(|b| {
            let planet_type = match b.body_type.as_str() {
                "terrestrial" => "Rocky",
                "gas_giant" => "Gas Giant",
                "ice_giant" => "Ice Giant",
                "dwarf_planet" => "Dwarf",
                "artificial" => "Artificial",
                other => other,
            }
            .to_string();
            GuiPlanet {
                name: b.name.clone(),
                planet_type,
                radius_km: b.radius_km,
                gravity: b.surface_gravity_ms2,
                // Pre-formatted "top components" summary from the catalog;
                // empty string means no atmosphere worth listing.
                atmosphere: if b.atmosphere_summary.is_empty() {
                    "None".to_string()
                } else {
                    b.atmosphere_summary.clone()
                },
                moons: b.children.len() as u32,
                orbit_radius_au: b.semi_major_axis_au,
            }
        })
        .collect()
}

// ─── Infinite-of-X data loaders (v0.123.0) ─────────────────────────────────
//
// One small JSON file per page taxonomy. All loaders share the same shape:
// graceful fallback to an empty Vec on missing/malformed input so the GUI still
// boots — pages render an empty filter row instead of crashing. The empty-vec
// path is also what the page sees during the brief window before lib.rs wires
// the loaders into GuiState at startup.

// v0.415.0: ResourceEntry / ResourceCategory / load_resource_categories removed
// with the Resources page (retired into the Library). v0.1063: those external
// links left the Library too and now live in data/external/catalog.json,
// rendered by the Tools page.

/// A streaming-studio scene preset.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StudioScenePreset {
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub source_visibility: Vec<bool>,
}

/// A streaming-studio source preset. Kinds: `camera|screen|microphone|chat_overlay|image|text|timer`.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StudioSourcePreset {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub device: u32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub position: (f32, f32),
    #[serde(default)]
    pub size: (f32, f32),
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub z_order: u32,
}
#[cfg(feature = "native")]
fn one() -> f32 { 1.0 }

/// Convert a deserialised preset into the runtime [`StudioSource`].
/// Unknown `kind` values fall back to `Camera(0)` — the most benign default.
#[cfg(feature = "native")]
pub fn studio_source_from_preset(p: &StudioSourcePreset) -> StudioSource {
    let source_type = match p.kind.as_str() {
        "camera" => StudioSourceType::Camera(p.device),
        "screen" => StudioSourceType::Screen(p.device),
        "microphone" => StudioSourceType::Microphone(p.device),
        "chat_overlay" => StudioSourceType::ChatOverlay,
        "image" => StudioSourceType::Image(p.text.clone()),
        "text" => StudioSourceType::Text(p.text.clone()),
        "timer" => StudioSourceType::Timer,
        _ => StudioSourceType::Camera(p.device),
    };
    StudioSource {
        name: p.name.clone(),
        source_type,
        visible: p.visible,
        position: p.position,
        size: p.size,
        opacity: p.opacity,
        z_order: p.z_order,
    }
}

/// Convert a deserialised preset into the runtime [`StudioScene`].
#[cfg(feature = "native")]
pub fn studio_scene_from_preset(p: &StudioScenePreset) -> StudioScene {
    StudioScene {
        name: p.name.clone(),
        is_default: p.is_default,
        source_visibility: p.source_visibility.clone(),
    }
}

/// Read a JSON file under `data/` and deserialise into `T`. Logs and returns
/// `None` on any error so callers can fall back gracefully. Disk-first,
/// embedded fallback (v0.744) — zero-file installs keep their data-driven UI.
#[cfg(feature = "native")]
fn read_data_json<T: serde::de::DeserializeOwned>(
    data_dir: &std::path::Path,
    relative: &str,
) -> Option<T> {
    let path = data_dir.join(relative);
    let bytes = match crate::embedded_data::read_data_or_embedded(data_dir, relative) {
        Some(b) => b,
        None => {
            eprintln!("[data] failed to read {} (no embedded copy)", path.display());
            return None;
        }
    };
    match serde_json::from_str::<T>(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[data] failed to parse {}: {}", path.display(), e);
            None
        }
    }
}

/// Load equipment slot definitions from `data/inventory/equipment_slots.json`.
#[cfg(feature = "native")]
pub fn load_equipment_slots(data_dir: &std::path::Path) -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct Slot { id: String, label: String }
    #[derive(serde::Deserialize)]
    struct File { slots: Vec<Slot> }
    read_data_json::<File>(data_dir, "inventory/equipment_slots.json")
        .map(|f| f.slots.into_iter().map(|s| (s.id, s.label)).collect())
        .unwrap_or_default()
}

/// A node in the uniform entity/place/container model — the operator's "mark
/// Earth as my container" idea, generalised: ONE recursive shape spans the whole
/// scale. Top-level entries are ENTITIES (you, your home, a vehicle); each is a
/// container holding rooms / sub-containers / items, any depth. A planet, a
/// building, a backpack, and a toothbrush are all just a `Place` with children —
/// the same nesting top to bottom.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Place {
    /// Stable id (optional — items can omit it). Used by future in-app editing
    /// and `location` references.
    #[serde(default)]
    pub id: String,
    pub label: String,
    /// person | vehicle | building | property | floor | room | container |
    /// backpack | pack | duffel | bag | pouch | planet | region | locale | item
    /// | … — free-form so the DATA leads, not code (drives the node colour too).
    #[serde(default)]
    pub kind: String,
    /// Soft location reference (a label or another node's id) — e.g. a vehicle
    /// "@ Home". Shown as detail, NOT a hard tree edge, so an entity can sit at
    /// the top level yet still say where it is without deep nesting.
    #[serde(default)]
    pub location: Option<String>,
    /// `[latitude, longitude]` for geographic nodes; the bridge to real-terrain
    /// world-gen — the point that says "render THIS hillside here".
    #[serde(default)]
    pub coordinate: Option<[f64; 2]>,
    /// Leaf items held DIRECTLY in this container, by item id (resolved against
    /// items.csv for the name/details). The nested-container inventory renders these
    /// as tiles; sub-containers go in `children`. A pocket might hold `["pen_0"]`
    /// plus a `keychain` child container. Empty for pure location/spine nodes (the
    /// live backpack injects its items at the node marked `kind: "backpack"`).
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub children: Vec<Place>,
    /// For a `kind: "item"` leaf: the items.csv id it is (so it has a real
    /// volume and can go into the backpack), with `label` as its display
    /// name. Absent = the label is the key (a free-text item). 2026-09-26.
    #[serde(default)]
    pub item: Option<String>,
    /// For a `kind: "item"` leaf: how many (default 1).
    #[serde(default)]
    pub qty: Option<u32>,
}

/// Load the seeded entities from `data/places/seed.json` — top-level entries
/// (You, your home, a vehicle, …), each a container with its own contents and an
/// optional `location`. Empty vec if absent (callers fall back to a flat view).
pub fn load_places(data_dir: &std::path::Path) -> Vec<Place> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        entities: Vec<Place>,
    }
    read_data_json::<File>(data_dir, "places/seed.json")
        .map(|f| f.entities)
        .unwrap_or_default()
}

/// One item placed in a container, for the organize-layer inventory (operator
/// 2026-06-22: "one item pool; each item records WHICH container it's in", and
/// transfer = move it between containers). `container` is the container's PATH in the
/// places tree (e.g. "1/0/0"), so a transfer is just changing this string. Seeded from
/// the places spine at load; serializable so a save can persist transfers. The live
/// backpack is NOT in this pool (its items come from the ECS) until the ECS-boundary
/// transfer lands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlacedItem {
    /// Item id (resolves against items.csv) OR a descriptive label for seed items.
    pub key: String,
    /// Display name (item name if `key` is an id; else the label).
    pub name: String,
    pub qty: u32,
    /// Container PATH in the places tree this item currently sits in.
    pub container: String,
    /// Uses worn off it (a tool, 2026-09-26): it keeps its wear in storage,
    /// so putting a worn tool away and taking it back does not renew it.
    #[serde(default)]
    pub wear: u32,
    /// Grade of a crafted durable good (0 = ungraded), kept through storage.
    #[serde(default)]
    pub quality: u8,
}

/// Flatten the places spine into the organize-layer item pool: every leaf `kind:"item"`
/// child and id-based `items` entry becomes a [`PlacedItem`] tagged with its container's
/// PATH (the same scheme the inventory renderer walks). The live backpack is excluded.
pub fn flatten_placed_items(places: &[Place]) -> Vec<PlacedItem> {
    fn walk(place: &Place, path: &str, out: &mut Vec<PlacedItem>) {
        for (j, child) in place.children.iter().enumerate() {
            if child.kind == "item" {
                out.push(PlacedItem {
                    key: child.item.clone().unwrap_or_else(|| child.label.clone()),
                    name: child.label.clone(),
                    qty: child.qty.unwrap_or(1).max(1),
                    container: path.to_string(),
                    wear: 0,
                    quality: 0,
                });
            } else {
                walk(child, &format!("{path}/{j}"), out);
            }
        }
        for id in &place.items {
            out.push(PlacedItem { key: id.clone(), name: id.clone(), qty: 1, container: path.to_string(), wear: 0, quality: 0 });
        }
    }
    let mut out = Vec::new();
    for (i, p) in places.iter().enumerate() {
        walk(p, &i.to_string(), &mut out);
    }
    out
}

/// Collect (path, label) for every CONTAINER in the places spine (not leaf items, and
/// not the live-only backpack), for the "Move to..." transfer menu. Path matches the
/// inventory renderer's scheme.
pub fn collect_containers(places: &[Place]) -> Vec<(String, String)> {
    fn walk(place: &Place, path: &str, out: &mut Vec<(String, String)>) {
        if place.kind != "backpack" {
            out.push((path.to_string(), place.label.clone()));
        }
        for (j, child) in place.children.iter().enumerate() {
            if child.kind != "item" {
                walk(child, &format!("{path}/{j}"), out);
            }
        }
    }
    let mut out = Vec::new();
    for (i, p) in places.iter().enumerate() {
        walk(p, &i.to_string(), &mut out);
    }
    out
}

// ── Homestead Design (the "homes" feature, offline-first; v0.379) ──
// The Fibonacci homestead blueprint (data/blueprints/fibonacci_homestead.ron),
// surfaced read-only as a browsable Design: rooms carry their materials (the bill
// of materials / parts list), power, and water needs, so the Home page can total
// the demand + parts and show how self-sufficient the build is. More designs can
// drop in as data later. See pages/homes.rs + docs/design/homes-as-profiles.md.

/// A whole homestead blueprint (one "Design").
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HomesteadDesign {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub rooms: Vec<DesignRoom>,
    #[serde(default)]
    pub tiers: Vec<DesignTier>,
    #[serde(default)]
    pub build_order: Vec<String>,
    #[serde(default)]
    pub scaling_notes: String,
}

/// One room in a homestead Design, with its build requirements.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DesignRoom {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: Size3,
    #[serde(default)]
    pub fibonacci_index: u32,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub requirements: RoomRequirements,
    #[serde(default)]
    pub environment_notes: String,
}

/// Room dimensions in metres.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Size3 {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
}

/// What a room needs to build + run: a bill of materials, plus power + water draw.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RoomRequirements {
    /// (item_id, quantity) pairs — the proto bill-of-materials.
    #[serde(default)]
    pub materials: Vec<(String, u32)>,
    #[serde(default)]
    pub power_watts: u32,
    #[serde(default)]
    pub water_liters_per_day: u32,
}

/// A construction tier (core / residential / industrial / exterior).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DesignTier {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub wall_thickness_cm: u32,
    #[serde(default)]
    pub radiation_shielding: bool,
}

/// Load the Fibonacci homestead blueprint. None if absent/unparseable (the Home
/// page then shows an empty state). Reads RON directly, like src/ship/fibonacci.rs.
pub fn load_homestead_design(data_dir: &std::path::Path) -> Option<HomesteadDesign> {
    let path = data_dir.join("blueprints/fibonacci_homestead.ron");
    let text =
        crate::embedded_data::read_data_or_embedded(data_dir, "blueprints/fibonacci_homestead.ron")?;
    match ron::from_str::<HomesteadDesign>(&text) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("load_homestead_design: failed to parse {}: {e}", path.display());
            None
        }
    }
}

// ── Aeroponic tower configs (the homestead food loop; v0.382) ──
// Two curated 50-slot vertical aeroponic towers (nutrition + apothecary), loaded
// from data/towers/aeroponic_configs.ron. Each planting references an existing
// plant id in plants.csv. Browsed on the Home page; the 3D placeholder + planting
// integration come later. See docs/design/self-sufficiency.md.

/// One aeroponic tower configuration (a curated 50-slot plant set).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TowerConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Make / model / version, shown in the tower's title row (operator 2026-06-08:
    /// "aeroponic tower make model version"). Data-driven so the community can brand
    /// their own designs; empty strings just hide that part of the title.
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub covers: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub gaps_note: String,
    #[serde(default)]
    pub disclaimer: String,
    #[serde(default)]
    pub slots: u32,
    /// 3D geometry for the placeholder (and the eventual real design; operator
    /// 2026-06-07: design + plant amount should be dynamic + scale infinitely). The
    /// column DIAMETER + HEIGHT in metres, and how many times the plant HELIX wraps
    /// the column: low helix_turns = coarse / spread out, high = fine / dense, like
    /// thread pitch on a bolt. A wide diameter + fine helix packs more plants.
    #[serde(default = "default_diameter_m")]
    pub diameter_m: f32,
    #[serde(default = "default_height_m")]
    pub height_m: f32,
    #[serde(default = "default_helix_turns")]
    pub helix_turns: f32,
    #[serde(default)]
    pub plantings: Vec<TowerPlanting>,
    /// Real-world parts to BUILD this tower (the game->real bridge / north star:
    /// every in-game system maps to a real buildable thing). Optional starting
    /// bill of materials; refine the parts, quantities, and sources for your build.
    #[serde(default)]
    pub parts: Vec<TowerPart>,
}

fn default_diameter_m() -> f32 {
    0.4
}
fn default_height_m() -> f32 {
    2.0
}
fn default_helix_turns() -> f32 {
    4.0
}

/// One plant assigned to N slots of a tower, with its role + a nutrition/medicinal note.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TowerPlanting {
    #[serde(default)]
    pub plant: String,
    #[serde(default)]
    pub slots: u32,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub note: String,
}

/// One real-world component needed to BUILD a tower (the game->real bridge).
/// `source` is how you would obtain it: "buy" / "3d_print" / "diy" / "trade" /
/// "scavenge". A starting list; refine for your build.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TowerPart {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub qty: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub note: String,
}

/// A garden grow AREA kind + how many the homestead has, its per-unit food output,
/// and footprint. Loaded from the garden room of `data/machines/home.ron` and shown
/// in the Inventory Garden overview + per-medium edit modal.
#[derive(Debug, Clone, Default)]
pub struct GardenArea {
    pub label: String,
    pub machine_id: String,
    pub count: u32,
    /// The food line: the computed figure (`systems::grow_machines`), or the
    /// typed estimate of a machine the model cannot compute.
    pub food: String,
    /// Footprint (w, h, d) in meters from the machine catalog.
    pub size: (f32, f32, f32),
}

/// Count every growing machine in `data/machines/home.ron` (a machine a grow medium
/// matches, the same test the engine publishes plots by), grouped by type, with its
/// catalog label / food line / footprint. The food line is the one every card shows
/// (`MachineHome::stats_for`). Resolved via `data_dir` so it works regardless of the
/// process CWD. Empty if the file is absent.
pub fn load_garden_areas(data_dir: &std::path::Path) -> Vec<GardenArea> {
    let path = crate::machines::home_ron_path(data_dir);
    let Some(home) = crate::machines::MachineHome::load(&path) else {
        return Vec::new();
    };
    let media = load_grow_media(data_dir);
    let is_grow = |machine: &str| home.catalog.contains_key(machine) && media.iter().any(|m| m.matches(machine));
    // v0.538: count EVERY grow machine, not just those in a literal "garden" room. The HomeStructure
    // home's rooms are flood-fill ids (home/room_1/...) that never equal "garden", so the old
    // room-name filter silently emptied the garden inventory. The is_grow catalog predicate is the
    // real signal; room membership is not.
    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for inst in &home.instances {
        if is_grow(&inst.machine) {
            *counts.entry(inst.machine.clone()).or_insert(0) += 1;
        }
    }
    for arr in &home.arrays {
        if is_grow(&arr.machine) {
            *counts.entry(arr.machine.clone()).or_insert(0) += arr.rows * arr.cols;
        }
    }
    let mut out: Vec<GardenArea> = counts
        .into_iter()
        .map(|(machine, count)| {
            let def = home.catalog.get(&machine);
            let label = def.map(|d| d.label.clone()).unwrap_or_else(|| machine.clone());
            let food = home.stats_for(&machine).into_iter().find(|s| s.kind == "food").map(|s| s.value).unwrap_or_default();
            let size = def.map(|d| d.size).unwrap_or((0.0, 0.0, 0.0));
            GardenArea { label, machine_id: machine, count, food, size }
        })
        .collect();
    // Most-numerous first, then by name, so the overview reads stably frame to frame.
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.label.cmp(&b.label)));
    out
}

/// The grow-media registry (data/garden/grow_media.ron): its types and loader live
/// with the food model in `systems::grow_machines`, which needs them under every
/// feature set; re-exported so `crate::gui::GrowMedium` keeps its spelling.
pub use crate::systems::grow_machines::{load_grow_media, GrowControl, GrowMedium};

/// Load the aeroponic tower configs (data/towers/aeroponic_configs.ron). Empty on
/// absence/parse error.
pub fn load_tower_configs(data_dir: &std::path::Path) -> Vec<TowerConfig> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        towers: Vec<TowerConfig>,
    }
    let path = data_dir.join("towers/aeroponic_configs.ron");
    let text = match crate::embedded_data::read_data_or_embedded(data_dir, "towers/aeroponic_configs.ron")
    {
        Some(t) => t,
        None => return Vec::new(),
    };
    match ron::from_str::<File>(&text) {
        Ok(f) => f.towers,
        Err(e) => {
            eprintln!("load_tower_configs: failed to parse {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Whether the plants in one tower can share a single reservoir + air — the
/// operator's "make sure they grow together". Aeroponics shares one nutrient
/// reservoir and one air volume (NOT soil), so soil companion/adverse rules
/// relax and the real constraint becomes a COMMON pH / temperature / humidity
/// window every plant tolerates. Each axis here is the intersection of the
/// per-plant windows (from plants.csv): `Some((lo, hi))` means all plants
/// overlap and can share it; `None` means no shared window (a conflict), and
/// `conflicts` names the binding extremes to reconsider.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct TowerCompat {
    /// Distinct species considered (those found in the plant registry).
    pub species: usize,
    /// Shared reservoir pH window, °C temperature window, and 0..1 humidity
    /// window. `None` on an axis = the plants have no common window there.
    pub ph: Option<(f32, f32)>,
    pub temp: Option<(f32, f32)>,
    pub humidity: Option<(f32, f32)>,
    /// One note per conflicting axis, naming the two binding plants, e.g.
    /// "Temp: Rosemary (warm) vs Lettuce (cool) have no overlap".
    pub conflicts: Vec<String>,
    /// Total daily water draw across EVERY slot (L/day) — feeds the homestead's
    /// self-sufficiency water loop. 0 if no plant water data.
    pub water_per_day_total: f32,
    /// Harvest window: soonest and latest species maturity in days (0 if unknown).
    pub first_harvest_days: f32,
    pub full_harvest_days: f32,
}

/// Intersect one window axis across a tower's plants. Degenerate windows
/// (`hi <= lo`, i.e. an unset 0..0 column) are skipped so missing data can't
/// fake a conflict. Returns the shared window if all valid windows overlap,
/// else `None` plus a note naming the warmest-floor and coolest-ceiling plants.
#[cfg(feature = "native")]
fn intersect_axis(windows: &[(String, (f32, f32))], label: &str) -> (Option<(f32, f32)>, Option<String>) {
    let valid: Vec<&(String, (f32, f32))> =
        windows.iter().filter(|(_, (lo, hi))| hi > lo).collect();
    if valid.is_empty() {
        return (None, None);
    }
    let lo = valid.iter().map(|(_, (l, _))| *l).fold(f32::MIN, f32::max);
    let hi = valid.iter().map(|(_, (_, h))| *h).fold(f32::MAX, f32::min);
    if lo <= hi {
        return (Some((lo, hi)), None);
    }
    // Conflict: the plant with the highest floor vs the one with the lowest ceiling.
    let warm = valid
        .iter()
        .max_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
    let cool = valid
        .iter()
        .min_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
    (
        None,
        Some(format!("{}: {} vs {} have no overlap", label, warm.0, cool.0)),
    )
}

/// Compute a tower's shared-reservoir compatibility from the plant registry.
/// Plants not found in the registry are skipped (so a partial registry still
/// gives a useful answer for the plants it knows).
#[cfg(feature = "native")]
pub fn compute_tower_compat(
    tower: &TowerConfig,
    reg: &crate::systems::farming::PlantRegistry,
) -> TowerCompat {
    // Distinct plant ids (a max-variety tower is mostly distinct already).
    let mut ids: Vec<String> = Vec::new();
    for p in &tower.plantings {
        if !ids.contains(&p.plant) {
            ids.push(p.plant.clone());
        }
    }
    // (name, ph window, temp window, humidity window) for each known species.
    let mut ph_w = Vec::new();
    let mut temp_w = Vec::new();
    let mut hum_w = Vec::new();
    let mut species = 0usize;
    for id in &ids {
        if let Some(d) = reg.get(id) {
            species += 1;
            ph_w.push((d.name.clone(), (d.ph_min, d.ph_max)));
            temp_w.push((d.name.clone(), (d.temp_min_c, d.temp_max_c)));
            hum_w.push((d.name.clone(), (d.humidity_min, d.humidity_max)));
        }
    }
    let (ph, ph_c) = intersect_axis(&ph_w, "pH");
    let (temp, temp_c) = intersect_axis(&temp_w, "Temp");
    let (humidity, hum_c) = intersect_axis(&hum_w, "Humidity");
    let conflicts: Vec<String> = [ph_c, temp_c, hum_c].into_iter().flatten().collect();
    // Total daily water draw across ALL slots (not distinct species), and the
    // harvest window across the distinct species.
    let mut water_per_day_total = 0.0f32;
    for p in &tower.plantings {
        if let Some(d) = reg.get(&p.plant) {
            water_per_day_total += d.water_per_day * p.slots.max(1) as f32;
        }
    }
    let growth: Vec<f32> = ids
        .iter()
        .filter_map(|id| reg.get(id))
        .map(|d| d.growth_days)
        .filter(|g| *g > 0.0)
        .collect();
    let (first_harvest_days, full_harvest_days) = if growth.is_empty() {
        (0.0, 0.0)
    } else {
        (
            growth.iter().cloned().fold(f32::MAX, f32::min),
            growth.iter().cloned().fold(0.0_f32, f32::max),
        )
    };
    TowerCompat {
        species,
        ph,
        temp,
        humidity,
        conflicts,
        water_per_day_total,
        first_harvest_days,
        full_harvest_days,
    }
}

#[cfg(all(test, feature = "native"))]
mod tower_compat_tests {
    use super::*;
    use crate::systems::farming::PlantRegistry;

    fn reg_from(csv: &[u8]) -> PlantRegistry {
        PlantRegistry::from_csv(csv).expect("parse")
    }

    fn tower_of(ids: &[&str]) -> TowerConfig {
        let mut t = TowerConfig {
            id: "t".into(),
            name: "T".into(),
            make: String::new(),
            model: String::new(),
            version: String::new(),
            purpose: String::new(),
            description: String::new(),
            covers: vec![],
            gaps: vec![],
            gaps_note: String::new(),
            disclaimer: String::new(),
            slots: 50,
            diameter_m: 0.4,
            height_m: 2.0,
            helix_turns: 4.0,
            plantings: vec![],
            parts: vec![],
        };
        for id in ids {
            t.plantings.push(TowerPlanting {
                plant: (*id).into(),
                slots: 1,
                role: String::new(),
                note: String::new(),
            });
        }
        t
    }

    #[test]
    fn compatible_plants_share_a_window() {
        // Two plants with overlapping pH/temp/humidity windows → one shared window,
        // no conflicts.
        let csv = b"id,name,growth_days,water_liters_per_day,ph_min,ph_max,temp_min_c,temp_max_c,humidity_min,humidity_max\n\
                    lettuce,Lettuce,45,0.5,6.0,7.0,10,22,0.5,0.8\n\
                    spinach,Spinach,40,0.6,6.2,7.2,8,24,0.5,0.9\n";
        let c = compute_tower_compat(&tower_of(&["lettuce", "spinach"]), &reg_from(csv));
        assert_eq!(c.species, 2);
        assert_eq!(c.ph, Some((6.2, 7.0)));
        assert_eq!(c.temp, Some((10.0, 22.0)));
        assert!(c.conflicts.is_empty(), "no conflict expected, got {:?}", c.conflicts);
        // Water draw sums per slot; the harvest window spans soonest..latest.
        assert!((c.water_per_day_total - 1.1).abs() < 1e-6, "water {}", c.water_per_day_total);
        assert!((c.first_harvest_days - 40.0).abs() < 1e-6, "first {}", c.first_harvest_days);
        assert!((c.full_harvest_days - 45.0).abs() < 1e-6, "full {}", c.full_harvest_days);
    }

    #[test]
    fn non_overlapping_temp_is_flagged() {
        // A warm herb and a cool green that can't share an air temperature.
        let csv = b"id,name,ph_min,ph_max,temp_min_c,temp_max_c,humidity_min,humidity_max\n\
                    rosemary,Rosemary,6.0,7.0,20,30,0.3,0.6\n\
                    lettuce,Lettuce,6.0,7.0,8,18,0.5,0.8\n";
        let c = compute_tower_compat(&tower_of(&["rosemary", "lettuce"]), &reg_from(csv));
        assert!(c.temp.is_none(), "temp should conflict");
        assert_eq!(c.conflicts.len(), 1);
        assert!(c.conflicts[0].contains("Temp"), "note: {}", c.conflicts[0]);
        // pH still overlaps, so it is reported as a shared window.
        assert_eq!(c.ph, Some((6.0, 7.0)));
    }
}

/// One entry in the Library: a document to read. (The `Link` kind that used to
/// share this type went to the Tools page with the rest of the external
/// catalog in v0.1063, so every Library entry is now a document.)
pub struct LibraryEntry {
    pub title: String,
    /// Deep-link slug, derived from the shipped filename exactly the way
    /// `web/pages/library-app.js` derives it (basename minus .md, underscores
    /// to hyphens, lowercased). Having it here is what lets a `/library#slug`
    /// cross-reference resolve to a document in the native client too, so one
    /// link form works in both.
    pub slug: String,
    /// Raw markdown body, read from `data/library/<file>` at startup.
    pub body: String,
    /// Tag ids that cross-cut the categories (`data/library/tags.json` defines
    /// them, `catalog.json` assigns them). A category is the one shelf a doc
    /// sits on; these are every other way someone might look for it.
    pub tags: Vec<String>,
}

/// One tag in the Library's vocabulary.
pub struct LibraryTag {
    pub id: String,
    pub label: String,
}

/// A named group of related tags (an axis: reality, domain, kind, audience).
/// Rendered as one row of filter chips per group.
pub struct LibraryTagGroup {
    pub label: String,
    pub tags: Vec<LibraryTag>,
}

/// A named category of entries within a section.
pub struct LibraryCategory {
    pub name: String,
    pub entries: Vec<LibraryEntry>,
}

/// A top-level Library section (e.g. "HumanityOS", "Tools and Websites") that
/// groups nested categories.
pub struct LibrarySection {
    pub name: String,
    pub categories: Vec<LibraryCategory>,
}

/// Everything the Library page needs from disk: the document tree plus the tag
/// vocabulary the filter chips render.
pub struct LibraryData {
    pub sections: Vec<LibrarySection>,
    pub tag_groups: Vec<LibraryTagGroup>,
}

/// One teachable topic from `data/curriculum/syllabus.json`.
///
/// The syllabus is the Library's DENOMINATOR: it names every subject a person
/// needs, so "how complete is this" has an answer. Until v0.1311 the only way to
/// read it was `just curriculum`, a command line, which the handbook's
/// GUI-first rule says is not good enough for anything a user might want to see.
pub struct CurriculumTopic {
    pub id: String,
    pub subject: String,
    pub title: String,
    pub summary: String,
    /// absent / stub / sourced / verified. Only `verified` is teaching-grade.
    pub status: String,
    /// Library slugs that teach this, in the `/library#slug` grammar.
    pub reading: Vec<String>,
    pub skills: Vec<String>,
    /// Simulation data files behind it, which is what separates a book from a
    /// simulation.
    pub data: Vec<String>,
    pub sources: Vec<String>,
    /// none / caution / serious / lethal.
    pub hazard: String,
    /// True when the correct answer changes with where you are standing, which
    /// is 46% of the syllabus and the reason locales exist.
    pub locale_dependent: bool,
}

/// A subject: a group of topics, in teaching order.
pub struct CurriculumSubject {
    pub id: String,
    pub title: String,
    pub order: i64,
}

/// The syllabus as the Library renders it.
pub struct CurriculumData {
    pub subjects: Vec<CurriculumSubject>,
    pub topics: Vec<CurriculumTopic>,
}

impl CurriculumData {
    pub fn is_empty(&self) -> bool {
        self.topics.is_empty()
    }
    /// Topics that name `slug` as their reading, so a document can show what it
    /// teaches rather than leaving the connection implicit.
    pub fn topics_for_slug(&self, slug: &str) -> Vec<&CurriculumTopic> {
        self.topics.iter().filter(|t| t.reading.iter().any(|r| r == slug)).collect()
    }
}

/// Load `data/curriculum/syllabus.json`. Empty on error, so a missing or
/// malformed syllabus hides the Curriculum view rather than breaking the page.
#[cfg(feature = "native")]
pub fn load_curriculum(data_dir: &std::path::Path) -> CurriculumData {
    #[derive(serde::Deserialize)]
    struct SubjectDef {
        id: String,
        title: String,
        #[serde(default)]
        order: i64,
    }
    #[derive(serde::Deserialize)]
    struct TopicDef {
        id: String,
        subject: String,
        title: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        status: String,
        #[serde(default)]
        reading: Vec<String>,
        #[serde(default)]
        skills: Vec<String>,
        #[serde(default)]
        data: Vec<String>,
        #[serde(default)]
        sources: Vec<String>,
        #[serde(default)]
        hazard: String,
        #[serde(default)]
        locale_dependent: bool,
    }
    #[derive(serde::Deserialize)]
    struct Syllabus {
        #[serde(default)]
        subjects: Vec<SubjectDef>,
        #[serde(default)]
        topics: Vec<TopicDef>,
    }
    let Some(s) = read_data_json::<Syllabus>(data_dir, "curriculum/syllabus.json") else {
        return CurriculumData { subjects: Vec::new(), topics: Vec::new() };
    };
    let mut subjects: Vec<CurriculumSubject> = s
        .subjects
        .into_iter()
        .map(|x| CurriculumSubject { id: x.id, title: x.title, order: x.order })
        .collect();
    subjects.sort_by_key(|x| x.order);
    CurriculumData {
        subjects,
        topics: s
            .topics
            .into_iter()
            .map(|t| CurriculumTopic {
                id: t.id,
                subject: t.subject,
                title: t.title,
                summary: t.summary,
                status: if t.status.is_empty() { "absent".to_string() } else { t.status },
                reading: t.reading,
                skills: t.skills,
                data: t.data,
                sources: t.sources,
                hazard: if t.hazard.is_empty() { "none".to_string() } else { t.hazard },
                locale_dependent: t.locale_dependent,
            })
            .collect(),
    }
}

/// Load the in-app Library from `data/library/index.json` plus the markdown
/// files it lists: the Accord + companion docs, and the tag vocabulary that
/// cross-cuts them. Documents only (the external links moved to the Tools page
/// in v0.1063). Empty on error, so the page falls back to a "nothing loaded"
/// note. The manifest is generated by `scripts/build-library.js` from
/// `data/library/catalog.json`; never hand-edit index.json.
#[cfg(feature = "native")]
pub fn load_library(data_dir: &std::path::Path) -> LibraryData {
    let mut sections = Vec::new();
    let mut tag_groups = Vec::new();

    // HumanityOS: the Accord + its companion docs.
    {
        #[derive(serde::Deserialize)]
        struct DocEntry {
            title: String,
            file: String,
            #[serde(default)]
            tags: Vec<String>,
        }
        #[derive(serde::Deserialize)]
        struct DocCat {
            name: String,
            /// Which top-level section this category belongs to. Absent in older
            /// manifests, which then all fall into one section as before.
            #[serde(default)]
            section: Option<String>,
            #[serde(default)]
            docs: Vec<DocEntry>,
        }
        #[derive(serde::Deserialize)]
        struct TagDef {
            id: String,
            label: String,
        }
        #[derive(serde::Deserialize)]
        struct TagGroupDef {
            label: String,
            #[serde(default)]
            tags: Vec<TagDef>,
        }
        #[derive(serde::Deserialize)]
        struct Manifest {
            #[serde(default)]
            categories: Vec<DocCat>,
            /// Section order, so both clients group identically.
            #[serde(default)]
            sections: Vec<String>,
            #[serde(default)]
            tags: Vec<TagGroupDef>,
        }
        if let Some(m) = read_data_json::<Manifest>(data_dir, "library/index.json") {
            let dir = data_dir.join("library");
            let section_order = m.sections.clone();
            // (section name, category) pairs, so the grouping below can honour
            // the manifest's declared section order rather than inventing one.
            let cats: Vec<(Option<String>, LibraryCategory)> = m
                .categories
                .into_iter()
                .map(|c| (c.section.clone(), LibraryCategory {
                    name: c.name,
                    entries: c
                        .docs
                        .into_iter()
                        .filter_map(|d| {
                            std::fs::read_to_string(dir.join(&d.file))
                                .ok()
                                .map(|body| LibraryEntry {
                                    slug: d
                                        .file
                                        .trim_end_matches(".md")
                                        .trim_end_matches(".MD")
                                        .replace('_', "-")
                                        .to_lowercase(),
                                    title: d.title,
                                    body,
                                    tags: d.tags,
                                })
                        })
                        .collect(),
                }))
                .filter(|(_, c)| !c.entries.is_empty())
                .collect();

            // Three tiers: section > category > document. Until v0.1308 this
            // hardcoded ONE section called "HumanityOS" and hung all seventeen
            // categories off it, so the rail was a flat seventeen-item scroll.
            // A manifest with no sections still works: everything falls into a
            // single unnamed group, exactly as before.
            if !cats.is_empty() {
                let mut order: Vec<String> = section_order;
                for (sec, _) in cats.iter() {
                    if let Some(name) = sec {
                        if !order.iter().any(|o| o == name) {
                            order.push(name.clone());
                        }
                    }
                }
                if order.is_empty() {
                    sections.push(LibrarySection {
                        name: "HumanityOS".to_string(),
                        categories: cats.into_iter().map(|(_, c)| c).collect(),
                    });
                } else {
                    let mut remaining = cats;
                    for name in order {
                        let (mine, rest): (Vec<_>, Vec<_>) = remaining
                            .into_iter()
                            .partition(|(sec, _)| sec.as_deref() == Some(name.as_str()));
                        remaining = rest;
                        if !mine.is_empty() {
                            sections.push(LibrarySection {
                                name,
                                categories: mine.into_iter().map(|(_, c)| c).collect(),
                            });
                        }
                    }
                    // Anything the manifest forgot to place still ships, rather
                    // than silently vanishing from the Library.
                    if !remaining.is_empty() {
                        sections.push(LibrarySection {
                            name: "Other".to_string(),
                            categories: remaining.into_iter().map(|(_, c)| c).collect(),
                        });
                    }
                }
            }
            tag_groups = m
                .tags
                .into_iter()
                .map(|g| LibraryTagGroup {
                    label: g.label,
                    tags: g.tags.into_iter().map(|t| LibraryTag { id: t.id, label: t.label }).collect(),
                })
                .filter(|g| !g.tags.is_empty())
                .collect();
        }
    }

    // NOTE (v0.1063): the external links that used to load here from
    // data/resources/catalog.json moved to data/external/catalog.json and are
    // rendered by the Tools page. Library is documents-only now, so the two
    // pages split by what you DO with them: Library is what you read, Tools is
    // what you go use. See docs/PAGES.md.

    LibraryData { sections, tag_groups }
}

/// Load `(severities, categories)` for the bug reporter from `data/bugs/taxonomy.json`.
#[cfg(feature = "native")]
pub fn load_bug_taxonomy(data_dir: &std::path::Path) -> (Vec<String>, Vec<String>) {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)] severities: Vec<String>,
        #[serde(default)] categories: Vec<String>,
    }
    read_data_json::<File>(data_dir, "bugs/taxonomy.json")
        .map(|f| (f.severities, f.categories))
        .unwrap_or_default()
}

/// Load crafting recipes from `data/recipes.csv` into the Crafting page's recipe
/// browser (`GuiState.craft_recipes`).
#[cfg(feature = "native")]
pub fn load_crafting_recipes(data_dir: &std::path::Path) -> Vec<GuiRecipe> {
    // Mirrors the runtime RecipeRegistry load (Wiring-1) but builds the GUI-facing
    // GuiRecipe rows the Crafting page browses. Reuses the shared CSV loader (skips
    // # comments, row-resilient) + Recipe::parse_ingredients for the pipe-separated
    // item:qty inputs/outputs. Before this the page's craft_recipes Vec was never
    // populated, so the Crafting page always showed "No recipes match your filter"
    // even after the recipe registry loaded into the runtime.
    #[derive(serde::Deserialize)]
    struct Row {
        id: String,
        name: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        inputs: String,
        #[serde(default)]
        outputs: String,
        #[serde(default)]
        craft_time_sec: f32,
        #[serde(default)]
        station_required: String,
        #[serde(default)]
        skill_required: String,
        #[serde(default)]
        skill_level: u32,
        #[serde(default)]
        description: String,
    }
    let bytes = match std::fs::read(data_dir.join("recipes.csv")) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let rows: Vec<Row> = crate::assets::loader::parse_csv(&bytes).unwrap_or_default();
    let tool_rules = crate::systems::crafting::tools::load(data_dir);
    // Which outputs are durable (graded when made by hand, 2026-09-26).
    #[derive(serde::Deserialize)]
    struct Dur {
        id: String,
        #[serde(default)]
        durability: u32,
    }
    let durable: std::collections::HashSet<String> = std::fs::read(data_dir.join("items.csv"))
        .ok()
        .and_then(|b| crate::assets::loader::parse_csv::<Dur>(&b).ok())
        .map(|v| v.into_iter().filter(|d| d.durability > 0).map(|d| d.id).collect())
        .unwrap_or_default();
    rows.into_iter()
        .map(|r| GuiRecipe {
            graded: crate::systems::crafting::Recipe::parse_ingredients(&r.outputs)
                .iter()
                .any(|(o, _)| durable.contains(o)),
            tools: tool_rules.tools_for(
                &r.id,
                r.station_required.trim(),
                &r.category,
                &crate::systems::crafting::Recipe::parse_ingredients(&r.outputs),
            ),
            id: r.id,
            name: r.name,
            category: r.category,
            inputs: crate::systems::crafting::Recipe::parse_ingredients(&r.inputs),
            outputs: crate::systems::crafting::Recipe::parse_ingredients(&r.outputs),
            craft_time_sec: r.craft_time_sec,
            station_required: r.station_required,
            skill_required: {
                let s = r.skill_required.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            },
            skill_level: r.skill_level,
            description: r.description,
        })
        .collect()
}

/// Load the hierarchical crafting category tree from `data/crafting/categories.json`
/// (top-level groups -> leaf categories). The Crafting page renders these as
/// collapsible groups; leaf categories are matched case-insensitively against
/// `recipe.category`. Fully data-driven (infinite-of-X) — add groups/categories
/// freely; for very large categories, split a recipe's category into finer values
/// and group them here.
#[cfg(feature = "native")]
pub fn load_crafting_category_groups(data_dir: &std::path::Path) -> Vec<CraftCategoryGroup> {
    #[derive(serde::Deserialize)]
    struct File { groups: Vec<CraftCategoryGroup> }
    read_data_json::<File>(data_dir, "crafting/categories.json")
        .map(|f| f.groups)
        .unwrap_or_default()
}

/// One group in the hierarchical crafting-category tree: a collapsible group name
/// plus its leaf categories.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CraftCategoryGroup {
    pub name: String,
    pub categories: Vec<String>,
}

#[cfg(all(test, feature = "native"))]
mod crafting_recipes_load_tests {
    use super::*;

    #[test]
    fn load_crafting_recipes_populates_from_real_data() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let recipes = load_crafting_recipes(&data_dir);
        assert!(
            recipes.len() > 50,
            "Crafting page should load the real recipes.csv (got {})",
            recipes.len()
        );
        let smelt = recipes
            .iter()
            .find(|r| r.id == "smelt_iron")
            .expect("smelt_iron present in the browser");
        assert!(!smelt.inputs.is_empty(), "smelt_iron has inputs");
        assert!(!smelt.outputs.is_empty(), "smelt_iron has outputs");
    }
}

/// One entry of the federation-shared market vocabulary
/// (`data/market/categories.json`). `id` is the wire value offerings carry
/// (lowercase snake_case, validated server-side at ingest); `label` is what
/// UIs display; `desc` seeds tooltips and empty states.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MarketCategory {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub desc: String,
}

/// Load the marketplace category vocabulary from `data/market/categories.json`.
#[cfg(feature = "native")]
pub fn load_market_categories(data_dir: &std::path::Path) -> Vec<MarketCategory> {
    #[derive(serde::Deserialize)]
    struct File { categories: Vec<MarketCategory> }
    read_data_json::<File>(data_dir, "market/categories.json")
        .map(|f| f.categories)
        .unwrap_or_default()
}

/// Load streaming-studio scene presets from `data/studio/scenes.json`.
#[cfg(feature = "native")]
pub fn load_studio_scenes(data_dir: &std::path::Path) -> Vec<StudioScenePreset> {
    #[derive(serde::Deserialize)]
    struct File { scenes: Vec<StudioScenePreset> }
    read_data_json::<File>(data_dir, "studio/scenes.json")
        .map(|f| f.scenes)
        .unwrap_or_default()
}

/// Load streaming-studio source presets from `data/studio/sources.json`.
#[cfg(feature = "native")]
pub fn load_studio_sources(data_dir: &std::path::Path) -> Vec<StudioSourcePreset> {
    #[derive(serde::Deserialize)]
    struct File { sources: Vec<StudioSourcePreset> }
    read_data_json::<File>(data_dir, "studio/sources.json")
        .map(|f| f.sources)
        .unwrap_or_default()
}

/// Picker options for the Broadcasting Studio page (platforms, resolutions, etc.).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct StudioStreamingConfig {
    #[serde(default)] pub platforms: Vec<String>,
    #[serde(default)] pub resolutions: Vec<String>,
    #[serde(default)] pub fps: Vec<u32>,
    #[serde(default)] pub chat_positions: Vec<String>,
}

/// Load streaming pickers from `data/studio/streaming_config.json`.
#[cfg(feature = "native")]
pub fn load_studio_streaming_config(data_dir: &std::path::Path) -> StudioStreamingConfig {
    read_data_json::<StudioStreamingConfig>(data_dir, "studio/streaming_config.json")
        .unwrap_or_default()
}

/// One Q&A entry on the Donate page FAQ.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DonateFaqEntry {
    pub question: String,
    pub answer: String,
}

/// Load donate-page FAQ entries from `data/donate/faq.json`.
#[cfg(feature = "native")]
pub fn load_donate_faq(data_dir: &std::path::Path) -> Vec<DonateFaqEntry> {
    #[derive(serde::Deserialize)]
    struct File { entries: Vec<DonateFaqEntry> }
    read_data_json::<File>(data_dir, "donate/faq.json")
        .map(|f| f.entries)
        .unwrap_or_default()
}

/// One direct-support donation link (GitHub Sponsors, Patreon, PayPal, Cash
/// App). These go to the maintainer, not the Sponsor-A-Can 501c3. Data-driven
/// from `data/donate/methods.json` so adding a method needs no code change; the
/// same file is read by the web donate page.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DonateMethod {
    pub network: String,
    #[serde(default)] pub label: String,
    pub value: String,
    /// "url" (external link) or "address". Deserialized from the JSON `type` key.
    #[serde(rename = "type", default)] pub kind: String,
    #[serde(default)] pub abbrev: String,
    /// Badge color as "#rrggbb"; parsed to a Color32 at render time.
    #[serde(default)] pub color: String,
}

/// Load direct-support donation methods from `data/donate/methods.json`.
#[cfg(feature = "native")]
pub fn load_donate_methods(data_dir: &std::path::Path) -> Vec<DonateMethod> {
    #[derive(serde::Deserialize)]
    struct File { methods: Vec<DonateMethod> }
    read_data_json::<File>(data_dir, "donate/methods.json")
        .map(|f| f.methods)
        .unwrap_or_default()
}

/// One charity the maintainer personally endorses on the donate page. These are
/// INDEPENDENT nonprofits (not HumanityOS, not the maintainer): a gift goes to
/// that organization directly. Data-driven from `data/donate/charities.json`,
/// shared with the web donate page.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DonateCharity {
    pub name: String,
    #[serde(default)] pub mission: String,
    #[serde(default)] pub url: String,
    /// Deductibility / relationship disclosure shown under the card.
    #[serde(default)] pub note: String,
    #[serde(default)] pub abbrev: String,
    #[serde(default)] pub color: String,
}

/// Load endorsed charities from `data/donate/charities.json`.
#[cfg(feature = "native")]
pub fn load_donate_charities(data_dir: &std::path::Path) -> Vec<DonateCharity> {
    #[derive(serde::Deserialize)]
    struct File { charities: Vec<DonateCharity> }
    read_data_json::<File>(data_dir, "donate/charities.json")
        .map(|f| f.charities)
        .unwrap_or_default()
}

/// One QA test task surfaced on the Testing page.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct QaTestTask {
    pub id: String,
    #[serde(default)] pub version: String,
    pub feature: String,
    pub what_to_test: String,
    pub expected: String,
    #[serde(default)] pub category: String,
    #[serde(default)] pub note: Option<String>,
}

/// Load QA test tasks from `data/testing/qa_tasks.json`.
#[cfg(feature = "native")]
pub fn load_qa_test_tasks(data_dir: &std::path::Path) -> Vec<QaTestTask> {
    #[derive(serde::Deserialize)]
    struct File { tasks: Vec<QaTestTask> }
    read_data_json::<File>(data_dir, "testing/qa_tasks.json")
        .map(|f| f.tasks)
        .unwrap_or_default()
}

/// The websites database types live with the reader (`web_reader::sites`);
/// re-exported so pages keep the `crate::gui::WebSite` spelling the other
/// data-driven types use.
#[cfg(feature = "native")]
pub use crate::web_reader::sites::{host_of, WebSite, WebSiteAffiliate, WebSiteCategory, WebSiteEmbed, WebSites};
/// Load the websites database from `data/web/sites.json`.
#[cfg(feature = "native")]
pub fn load_web_sites(data_dir: &std::path::Path) -> WebSites {
    read_data_json::<WebSites>(data_dir, "web/sites.json").unwrap_or_default()
}

/// Load the readability hints from `data/web/readability.json` (which
/// classes and ids mark screen-only chrome on cooperating sites).
#[cfg(feature = "native")]
pub fn load_web_read_rules(data_dir: &std::path::Path) -> crate::web_reader::ReadRules {
    read_data_json::<crate::web_reader::ReadRules>(data_dir, "web/readability.json").unwrap_or_default()
}

// v0.415.0: OnboardingConcept / OnboardingCorePage + their loaders removed with
// the standalone onboarding page. The JSON files stay (the web /onboarding page
// reads them); the native consumers are gone.

// v0.197.0: AiUsageFilters and load_ai_usage_filters removed (AI Usage
// page deleted along with its data/ai_usage/filters.json loader).

/// Load default task project names from `data/tasks/default_projects.json`.
/// These seed the task board for brand-new identities; existing users keep
/// their own list.
#[cfg(feature = "native")]
pub fn load_default_task_projects(data_dir: &std::path::Path) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct File { projects: Vec<String> }
    read_data_json::<File>(data_dir, "tasks/default_projects.json")
        .map(|f| f.projects)
        .unwrap_or_default()
}

/// Load the starting per-skill XP profile applied to brand-new identities,
/// from `data/skills/default_profile.json`. The skill catalog itself lives in
/// `data/skills/skills.csv`; this file is just the initial XP weights.
#[cfg(feature = "native")]
pub fn load_default_player_skills(data_dir: &std::path::Path) -> Vec<(String, f32)> {
    #[derive(serde::Deserialize)]
    struct Skill { name: String, xp: f32 }
    #[derive(serde::Deserialize)]
    struct File { skills: Vec<Skill> }
    read_data_json::<File>(data_dir, "skills/default_profile.json")
        .map(|f| f.skills.into_iter().map(|s| (s.name, s.xp)).collect())
        .unwrap_or_default()
}

