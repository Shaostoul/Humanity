//! Farming system -- crop growth simulation driven by time, water, and plant data.
//!
//! Queries all entities with `CropInstance` and advances growth stages.
//! Plant definitions loaded from `data/plants.csv`.
//! Growth stages are data-driven: each plant species defines its own stage
//! names in plants.csv (colon-separated). Default stages are used when missing.

pub mod crops;
pub mod soil;
pub mod automation;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::components::{CropInstance, DEFAULT_GROWTH_STAGES, STAGE_DEAD};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Plant definition loaded from plants.csv -- cached in DataStore as "plant_registry".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantDef {
    /// Unique plant ID (e.g., "tomato").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Total real-world days from seed to harvest.
    pub growth_days: f32,
    /// Water consumption in liters per day per plant.
    pub water_per_day: f32,
    /// Preferred growing seasons.
    pub seasons: Vec<String>,
    /// Ordered growth stage names for this plant species.
    /// Loaded from plants.csv `growth_stages` column (colon-separated).
    /// Falls back to DEFAULT_GROWTH_STAGES when empty.
    pub growth_stages: Vec<String>,
    /// Harvest yield range (units of produce per fully-grown plant). f32 because real
    /// crops can yield LESS than one unit per plant per harvest (saffron: 0.3 -- a few
    /// stigma threads); the harvest roll converts the continuous roll to whole inventory
    /// items by probabilistic rounding, preserving the expected value. Was u32, which made
    /// serde reject (and the registry silently drop) any plants.csv row with a fractional
    /// yield -- saffron was un-plantable for months before the 2026-07-01 fix.
    pub yield_min: f32,
    pub yield_max: f32,
    /// Relative nutrient demand fractions (N, P, K) from plants.csv. Shown per
    /// crop in the Garden table; feed the future shared-reservoir mix math.
    pub nutrient_n: f32,
    pub nutrient_p: f32,
    pub nutrient_k: f32,
    /// Preferred reservoir pH window (for the future tower compatibility check).
    pub ph_min: f32,
    pub ph_max: f32,
    /// Tolerated air/water temperature window, Celsius.
    pub temp_min_c: f32,
    pub temp_max_c: f32,
    /// Preferred relative-humidity window, 0..1.
    pub humidity_min: f32,
    pub humidity_max: f32,
}

impl PlantDef {
    /// Returns this plant's growth stages, falling back to defaults if empty.
    pub fn stages(&self) -> Vec<&str> {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES.iter().copied().collect()
        } else {
            self.growth_stages.iter().map(|s| s.as_str()).collect()
        }
    }

    /// Returns the first stage name (the initial stage when planted).
    pub fn first_stage(&self) -> &str {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES[0]
        } else {
            &self.growth_stages[0]
        }
    }

    /// Returns the last stage name (the harvest-ready stage).
    pub fn last_stage(&self) -> &str {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES[DEFAULT_GROWTH_STAGES.len() - 1]
        } else {
            &self.growth_stages[self.growth_stages.len() - 1]
        }
    }
}

/// Registry of all plant definitions, keyed by plant ID.
#[derive(Debug, Clone, Default)]
pub struct PlantRegistry {
    pub plants: HashMap<String, PlantDef>,
}

impl PlantRegistry {
    /// Look up a plant definition by ID.
    pub fn get(&self, id: &str) -> Option<&PlantDef> {
        self.plants.get(id)
    }

    /// Build the plant registry from raw `plants.csv` bytes.
    ///
    /// Uses the shared CSV loader (skips `#` comments, header-mapped, row-resilient).
    /// `growth_stages` and `seasons` are colon-separated lists. This is the
    /// constructor the runtime calls to populate `DataStore["plant_registry"]` —
    /// before v0.323 the CSV was loaded then discarded, so FarmingSystem ran on
    /// default growth stages with no species data.
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<PlantRow> = crate::assets::loader::parse_csv(data)?;
        let mut plants = HashMap::new();
        for row in rows {
            plants.insert(
                row.id.clone(),
                PlantDef {
                    id: row.id,
                    name: row.name,
                    growth_days: row.growth_days,
                    water_per_day: row.water_liters_per_day,
                    seasons: split_colon_list(&row.seasons),
                    growth_stages: split_colon_list(&row.growth_stages),
                    yield_min: row.yield_min,
                    yield_max: row.yield_max,
                    nutrient_n: row.nutrient_n,
                    nutrient_p: row.nutrient_p,
                    nutrient_k: row.nutrient_k,
                    ph_min: row.ph_min,
                    ph_max: row.ph_max,
                    temp_min_c: row.temp_min_c,
                    temp_max_c: row.temp_max_c,
                    humidity_min: row.humidity_min,
                    humidity_max: row.humidity_max,
                },
            );
        }
        Ok(Self { plants })
    }
}

/// One row of `plants.csv`. The columns `PlantRegistry` consumes; extra CSV
/// columns (value/skill/companions/adverse) are still ignored. Nutrient demand
/// (N/P/K), the pH window, the temperature window, and the humidity window are
/// now parsed so the Garden table can show per-crop needs and a future tower
/// compatibility check can compute the shared-reservoir window. Every numeric
/// field is `#[serde(default)]`, so older/leaner CSVs (and the test fixture)
/// that omit these columns simply default them to 0.
#[derive(Debug, Deserialize)]
struct PlantRow {
    id: String,
    name: String,
    #[serde(default)]
    growth_days: f32,
    #[serde(default)]
    water_liters_per_day: f32,
    #[serde(default)]
    nutrient_n: f32,
    #[serde(default)]
    nutrient_p: f32,
    #[serde(default)]
    nutrient_k: f32,
    #[serde(default)]
    ph_min: f32,
    #[serde(default)]
    ph_max: f32,
    #[serde(default)]
    temp_min_c: f32,
    #[serde(default)]
    temp_max_c: f32,
    #[serde(default)]
    humidity_min: f32,
    #[serde(default)]
    humidity_max: f32,
    #[serde(default)]
    growth_stages: String,
    #[serde(default)]
    seasons: String,
    #[serde(default)]
    yield_min: f32,
    #[serde(default)]
    yield_max: f32,
}

/// Split a colon-separated list field into trimmed, non-empty entries.
fn split_colon_list(s: &str) -> Vec<String> {
    s.split(':')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

#[cfg(test)]
mod plant_registry_csv_tests {
    use super::*;

    #[test]
    fn from_csv_parses_plants_and_colon_lists() {
        let csv = b"id,name,type,growth_days,water_liters_per_day,growth_stages,seasons\n\
                    tomato,Tomato,fruit,80,0.5,seed:sprout:vegetative:mature,spring:summer\n";
        let reg = PlantRegistry::from_csv(csv).expect("parse");
        assert_eq!(reg.plants.len(), 1);
        let t = reg.get("tomato").expect("tomato present");
        assert!((t.growth_days - 80.0).abs() < 1e-6);
        assert!((t.water_per_day - 0.5).abs() < 1e-6);
        assert_eq!(t.growth_stages, vec!["seed", "sprout", "vegetative", "mature"]);
        assert_eq!(t.seasons, vec!["spring", "summer"]);
        assert_eq!(t.first_stage(), "seed");
        assert_eq!(t.last_stage(), "mature");
    }

    #[test]
    fn from_csv_parses_nutrient_temp_and_humidity_columns() {
        // A full-width row (mirroring data/plants.csv) must populate the N/P/K,
        // pH, temperature, and humidity fields the Garden table shows and the
        // future tower compatibility check will read. Locks the column names.
        let csv = b"id,name,description,type,growth_days,water_liters_per_day,nutrient_n,nutrient_p,nutrient_k,ph_min,ph_max,temp_min_c,temp_max_c,humidity_min,humidity_max,yield_min,yield_max,growth_stages,seasons\n\
                    tomato,Tomato,desc,fruit,70,1.5,0.15,0.05,0.20,6.0,6.8,18,30,0.50,0.80,2,8,seed:sprout:mature,spring:summer\n";
        let reg = PlantRegistry::from_csv(csv).expect("parse");
        let t = reg.get("tomato").expect("tomato present");
        assert!((t.nutrient_n - 0.15).abs() < 1e-6, "N");
        assert!((t.nutrient_p - 0.05).abs() < 1e-6, "P");
        assert!((t.nutrient_k - 0.20).abs() < 1e-6, "K");
        assert!((t.water_per_day - 1.5).abs() < 1e-6, "water/day");
        assert!((t.temp_min_c - 18.0).abs() < 1e-6, "temp_min");
        assert!((t.temp_max_c - 30.0).abs() < 1e-6, "temp_max");
        assert!((t.ph_min - 6.0).abs() < 1e-6, "ph_min");
        assert!((t.ph_max - 6.8).abs() < 1e-6, "ph_max");
        assert!((t.humidity_min - 0.50).abs() < 1e-6, "humidity_min");
        assert!((t.humidity_max - 0.80).abs() < 1e-6, "humidity_max");
        assert!((t.yield_max - 8.0).abs() < 1e-6, "yield still parses past the new columns");
    }

    /// ZERO-DROP GUARD: every data row in the shipped `data/plants.csv` must survive
    /// `PlantRegistry::from_csv` -- registry size == raw data-row count. The shared CSV
    /// loader is row-resilient (a row that fails serde is skipped with only a log::warn),
    /// which silently ate `saffron` for months: its fractional yield_min (0.3) failed the
    /// old `u32` yield field, so the registry never contained it and it was un-plantable
    /// in-game. Any future schema drift that starts eating rows fails HERE, loudly.
    #[test]
    fn shipped_plants_csv_parses_with_zero_dropped_rows() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("plants.csv");
        let text = std::fs::read_to_string(&path).expect("data/plants.csv reads");
        // Raw data-row count: non-comment, non-blank lines, minus the header line.
        let data_rows = text
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#')
            })
            .count()
            - 1;
        let reg = PlantRegistry::from_csv(text.as_bytes()).expect("data/plants.csv parses");
        assert_eq!(
            reg.plants.len(),
            data_rows,
            "PlantRegistry dropped {} of {} plants.csv rows (row-resilient parsing silently \
             ate them -- check the loader warn log for which rows failed serde)",
            data_rows - reg.plants.len().min(data_rows),
            data_rows
        );
        // The row that exposed the bug: saffron's fractional yield survives as-is.
        let saffron = reg.get("saffron").expect("saffron present (fractional-yield row)");
        assert!(
            (saffron.yield_min - 0.3).abs() < 1e-6,
            "saffron yield_min survives as 0.3, got {}",
            saffron.yield_min
        );
        assert!(
            (saffron.yield_max - 1.0).abs() < 1e-6,
            "saffron yield_max survives as 1.0, got {}",
            saffron.yield_max
        );
    }

    /// The shipped `data/plants.csv` carries real edible mushroom crops (added 2026-07-01 to
    /// back the `mushroom_rack` machine's "+50 kcal/d" claim -- see
    /// docs/design/homestead-solo-design.md gap #1). Guards the mushroom_rack's food-loop
    /// story against a silent regression to the old alien-only fungi.
    #[test]
    fn shipped_plants_csv_has_real_edible_mushrooms() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("plants.csv");
        let bytes = std::fs::read(&path).expect("data/plants.csv reads");
        let reg = PlantRegistry::from_csv(&bytes).expect("data/plants.csv parses");
        for id in ["oyster_mushroom", "shiitake", "button_mushroom"] {
            let def = reg.get(id).unwrap_or_else(|| panic!("{id} present in plants.csv"));
            assert!(def.growth_days > 0.0, "{id} has a real growth cycle");
            assert!(def.humidity_min > 0.5, "{id} is a high-humidity crop, not a desert plant");
        }
    }
}

/// Rate at which water_level decreases per second (base dehydration).
const DEHYDRATION_RATE: f32 = 0.002;

/// Water level below which crop health starts dropping.
const WATER_STRESS_THRESHOLD: f32 = 0.2;

/// Health recovery rate per second when well-watered.
const HEALTH_RECOVERY_RATE: f32 = 0.5;

/// Health decay rate per second when water-stressed.
const HEALTH_DECAY_RATE: f32 = 1.0;

/// Home RF level above which crops start taking RF stress (v0.620). Any notable wireless emission.
const RF_HARM_THRESHOLD: f32 = 0.1;
/// Crop health lost per second per unit of home RF level. Scaled so one WiFi router (~0.6) outpaces the
/// well-watered recovery rate, so the grow visibly declines while RF is present + recovers once it stops.
const RF_HEALTH_PENALTY: f32 = 1.5;

/// Seconds per in-game day (must match time system).
const SECONDS_PER_DAY: f64 = 1200.0;

/// Determine growth stage from progress fraction (0.0 to 1.0+) using
/// a data-driven stage list. Stages are evenly distributed across the
/// 0.0-1.0 range unless custom thresholds are added later.
fn stage_from_progress<'a>(progress: f32, stages: &'a [&'a str]) -> &'a str {
    if stages.is_empty() {
        return DEFAULT_GROWTH_STAGES[0];
    }
    let n = stages.len();
    // Each stage occupies an equal fraction of the 0.0-1.0 range.
    // stage[i] starts at i/n and runs until (i+1)/n.
    let idx = ((progress * n as f32).floor() as usize).min(n - 1);
    stages[idx]
}

/// Returns the index of a stage name in the stage list, or None if not found.
fn stage_index(stage: &str, stages: &[&str]) -> Option<usize> {
    stages.iter().position(|s| *s == stage)
}

/// Map a seed item id (`seed_<plant>_0`) to its plant-definition id (`<plant>`).
/// Strips the `seed_` prefix and a trailing `_<n>` item-instance suffix.
fn plant_id_from_seed(seed_id: &str) -> Option<String> {
    let body = seed_id.strip_prefix("seed_")?;
    if let Some((base, suffix)) = body.rsplit_once('_') {
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return Some(base.to_string());
        }
    }
    Some(body.to_string())
}

/// Resolve a plant id to the produce item it yields, VALIDATED against the item
/// registry so a harvest only ever produces an item that actually exists. Tries
/// the `vegetable_/fruit_/grain_` naming convention. (A `harvest_item` column on
/// plants.csv would make this fully data-driven — tracked in gameplay-loops.md.)
fn harvest_item_for(
    plant_id: &str,
    items: Option<&crate::systems::inventory::ItemRegistry>,
) -> Option<String> {
    for prefix in ["vegetable", "fruit", "grain"] {
        let candidate = format!("{prefix}_{plant_id}_0");
        if items.map(|r| r.items.contains_key(&candidate)).unwrap_or(false) {
            return Some(candidate);
        }
    }
    None
}

/// Simulates crop growth based on elapsed time and environmental factors.
pub struct FarmingSystem {
    _initialized: bool,
}

impl FarmingSystem {
    pub fn new() -> Self {
        Self {
            _initialized: false,
        }
    }
}

impl System for FarmingSystem {
    fn name(&self) -> &str {
        "FarmingSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let plant_registry = data.get::<PlantRegistry>("plant_registry");

        // Get current elapsed time from TimeSystem's GameTime if available
        let elapsed_seconds = data
            .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
            .and_then(|m| m.lock().ok())
            .map(|gt| gt.elapsed_seconds)
            .unwrap_or(0.0);

        // Build default stages vec once for plants without custom stages
        let default_stages: Vec<&str> = DEFAULT_GROWTH_STAGES.iter().copied().collect();

        let item_registry = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");

        // Per-area irrigation: a grow area the player has configured (in the garden
        // edit modal) tops its crops up to a target water level each tick. Keyed by
        // tower_id (e.g. "nutrition"). A neutral HashMap<String, f32> so the sim never
        // imports a GUI type; the GUI publishes it via lib.rs -> "garden_irrigation".
        // Empty/missing = no automated irrigation (crops dehydrate normally).
        let irrigation: std::collections::HashMap<String, f32> = data
            .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>("garden_irrigation")
            .and_then(|m| m.lock().ok())
            .map(|g| g.clone())
            .unwrap_or_default();
        // Per-area nutrient strength (garden edit slider), keyed by tower_id. Scales
        // each tower crop's growth speed. Same neutral-map pattern as irrigation.
        let nutrient: std::collections::HashMap<String, f32> = data
            .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>("garden_nutrient")
            .and_then(|m| m.lock().ok())
            .map(|g| g.clone())
            .unwrap_or_default();

        // Water -> FOOD coupling (v0.611): the downstream end of the power -> water -> food chain. If the
        // home has a real water system (a cistern) and it has run DRY, automated irrigation can no longer
        // top crops up -- so they dehydrate + wilt. (Cut the power -> the well pump sheds -> the cistern
        // drains -> days later the garden starts to die.) Read from PlumbingSystem's live WaterStatus.
        // Absent water_status (tests / a home with no plumbing) OR no cistern (capacity 0) = water
        // available, so existing gardening behaviour + un-plumbed homes are unchanged.
        let water_available = data
            .get::<std::sync::Mutex<crate::systems::plumbing::WaterStatus>>("water_status")
            .and_then(|m| m.lock().ok())
            .map(|ws| ws.capacity_l <= 0.0 || ws.stored_l > ws.capacity_l * 0.02)
            .unwrap_or(true);

        // RF -> FOOD coupling (v0.620): sum every POWERED RF emitter (a WiFi router) into a home RF
        // level. Sensitive crops lose health under RF -- the operator's "the user doesn't want a WiFi
        // router because it harms a plant they're growing." Run wired (Cat6/fibre, zero RF) to stay clean.
        let home_rf: f32 = {
            use crate::ecs::components::{PowerConsumer, RfEmitter};
            let mut rf = 0.0f32;
            for (_, (em, power)) in world.query::<(&RfEmitter, Option<&PowerConsumer>)>().iter() {
                let powered = !em.needs_power || power.map(|c| c.enabled).unwrap_or(false);
                if powered {
                    rf += em.strength;
                }
            }
            rf
        };

        // Creative mode (default ON in early dev): planting + fertilizing skip the
        // inventory requirement + consumption. Absent flag (tests) = survival =
        // consume, so the existing gardening tests still hold.
        let creative = data
            .get::<std::sync::Mutex<bool>>("creative_mode")
            .and_then(|m| m.lock().ok().map(|g| *g))
            .unwrap_or(false);

        // ── GUI / dev gardening commands (the inventory page writes these via the
        //    main-loop bridge): plant a seed, water a crop, dev-grow all, harvest. ──
        // PLANT: consume one matching seed from the player, spawn a CropInstance.
        let plant_seed = data
            .get::<std::sync::Mutex<Option<String>>>("plant_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(seed_id) = plant_seed {
            if let Some(plant_id) = plant_id_from_seed(&seed_id) {
                // Resolve the first growth stage now (immutable plant_registry borrow),
                // copied out before the &mut World pass below.
                let first_stage = plant_registry
                    .and_then(|reg| reg.get(&plant_id))
                    .map(|d| d.first_stage().to_string());
                if let Some(first_stage) = first_stage {
                    let mut planted = false;
                    for (_e, (inv, _ctrl)) in world.query_mut::<(
                        &mut crate::systems::inventory::Inventory,
                        &crate::ecs::components::Controllable,
                    )>() {
                        if creative || inv.has_item(&seed_id, 1) {
                            if !creative {
                                inv.remove_item(&seed_id, 1);
                            }
                            planted = true;
                            break;
                        }
                    }
                    if planted {
                        world.spawn((CropInstance {
                            crop_def_id: plant_id.clone(),
                            growth_stage: first_stage,
                            planted_at: elapsed_seconds,
                            water_level: 1.0,
                            health: 100.0,
                            tower_id: None,
                            tower_slot: None,
                        },));
                        log::info!("[Farming] planted {plant_id} (from {seed_id})");
                    }
                } else {
                    log::debug!("[Farming] no plant def for seed {seed_id}; not planted");
                }
            }
        }

        // PLANT TOWER (v0.386): spawn a CropInstance for each plant id sent by the
        // GUI. A tower's curated varieties all become growing crops at once;
        // growth/water/harvest reuse the logic below. v0.398: in SURVIVAL mode each
        // variety consumes one seed_<plant>_0 from the player (a variety with no seed
        // is skipped); CREATIVE mode plants every variety free. The seed is
        // plot-agnostic — the same seed plants a crop in any plot type, so this
        // generalizes when non-aeroponic gardens (soil / sand / pots) arrive.
        let plant_tower = data
            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some((tower_id, plant_ids)) = plant_tower {
            let mut planted = 0u32;
            let mut skipped = 0u32;
            for (slot_idx, plant_id) in plant_ids.into_iter().enumerate() {
                let slot_idx = slot_idx as u32;
                let first_stage = plant_registry
                    .and_then(|reg| reg.get(&plant_id))
                    .map(|d| d.first_stage().to_string());
                let first_stage = match first_stage {
                    Some(s) => s,
                    None => continue,
                };
                // SLOT FILL (v0.410): a tower has fixed slots. Skip if a LIVE crop
                // already occupies this slot, so replanting is IDEMPOTENT (fills only
                // empty / harvested / dead slots) instead of stacking a new set every
                // time. Despawn any DEAD crop in the slot first so it gets refilled.
                let mut occupied = false;
                let mut dead_in_slot: Vec<hecs::Entity> = Vec::new();
                for (e, c) in world.query::<&CropInstance>().iter() {
                    if c.tower_id.as_deref() == Some(tower_id.as_str())
                        && c.tower_slot == Some(slot_idx)
                    {
                        if c.growth_stage.as_str() == crate::ecs::components::STAGE_DEAD {
                            dead_in_slot.push(e);
                        } else {
                            occupied = true;
                        }
                    }
                }
                if occupied {
                    continue;
                }
                for e in dead_in_slot {
                    let _ = world.despawn(e);
                }
                // Survival: consume one seed for this variety, skip if absent.
                if !creative {
                    let seed_id = format!("seed_{plant_id}_0");
                    let mut had = false;
                    for (_e, (inv, _ctrl)) in world.query_mut::<(
                        &mut crate::systems::inventory::Inventory,
                        &crate::ecs::components::Controllable,
                    )>() {
                        if inv.has_item(&seed_id, 1) {
                            inv.remove_item(&seed_id, 1);
                            had = true;
                        }
                        break;
                    }
                    if !had {
                        skipped += 1;
                        continue;
                    }
                }
                world.spawn((CropInstance {
                    crop_def_id: plant_id,
                    growth_stage: first_stage,
                    planted_at: elapsed_seconds,
                    water_level: 1.0,
                    health: 100.0,
                    tower_id: Some(tower_id.clone()),
                    tower_slot: Some(slot_idx),
                },));
                planted += 1;
            }
            if planted > 0 || skipped > 0 {
                log::info!("[Farming] planted tower: {planted} crops, {skipped} skipped (no seed)");
            }
        }

        // DEV: stock one of each requested seed (the "one seed of each" starter set,
        // granted on demand so survival mode is testable now; the on-new-game grant
        // comes when the game is closer to ready). Grows the inventory to fit.
        let stock_seeds = data
            .get::<std::sync::Mutex<Option<Vec<String>>>>("stock_seeds_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(seed_ids) = stock_seeds {
            for (_e, (inv, _ctrl)) in world.query_mut::<(
                &mut crate::systems::inventory::Inventory,
                &crate::ecs::components::Controllable,
            )>() {
                let want = inv.max_slots + seed_ids.len();
                inv.ensure_slots(want);
                for seed_id in &seed_ids {
                    inv.add_item(seed_id, 1, 99);
                }
                break;
            }
            log::info!("[Farming] dev-stocked {} seed varieties", seed_ids.len());
        }

        // WATER: top up one crop's water + a little health.
        let water_bits = data
            .get::<std::sync::Mutex<Option<u64>>>("water_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(bits) = water_bits {
            if let Some(entity) = hecs::Entity::from_bits(bits) {
                if let Ok(mut crop) = world.get::<&mut CropInstance>(entity) {
                    crop.water_level = 1.0;
                    crop.health = (crop.health + 10.0).min(100.0);
                }
            }
        }

        // FERTILIZE: consume 1 fertilizer_0 from the player -> boost a crop's health
        // (growth is health-weighted, so fertilizing speeds it up). Closes the
        // food -> waste -> compost -> fertilizer -> crop cycle (#7c sanitation).
        let fertilize_bits = data
            .get::<std::sync::Mutex<Option<u64>>>("fertilize_crop_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(bits) = fertilize_bits {
            if let Some(entity) = hecs::Entity::from_bits(bits) {
                let mut had_fertilizer = false;
                for (_e, (inv, _ctrl)) in world.query_mut::<(
                    &mut crate::systems::inventory::Inventory,
                    &crate::ecs::components::Controllable,
                )>() {
                    if creative || inv.has_item("fertilizer_0", 1) {
                        if !creative {
                            inv.remove_item("fertilizer_0", 1);
                        }
                        had_fertilizer = true;
                    }
                    break;
                }
                if had_fertilizer {
                    if let Ok(mut crop) = world.get::<&mut CropInstance>(entity) {
                        crop.health = (crop.health + 40.0).min(100.0);
                        crop.water_level = crop.water_level.max(0.5);
                        log::info!("[Farming] fertilized a crop (+health)");
                    }
                }
            }
        }

        // DEV: instantly mature every living crop (a testing affordance, like
        // "Dev: stock all materials" — so the loop is verifiable without waiting
        // game-days for growth).
        let dev_grow = data
            .get::<std::sync::Mutex<bool>>("dev_grow_crops")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        if dev_grow {
            for (_e, crop) in world.query_mut::<&mut CropInstance>() {
                if crop.growth_stage == STAGE_DEAD {
                    continue;
                }
                if let Some(last) = plant_registry
                    .and_then(|reg| reg.get(&crop.crop_def_id))
                    .map(|d| d.last_stage().to_string())
                {
                    crop.growth_stage = last;
                }
            }
        }

        // HARVEST: a fully-grown crop -> produce items into the player + despawn it.
        let harvest_bits = data
            .get::<std::sync::Mutex<Option<u64>>>("harvest_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(bits) = harvest_bits {
            if let Some(entity) = hecs::Entity::from_bits(bits) {
                // Read the crop (immutable, scoped) to confirm maturity + plant id.
                let plant_id = world.get::<&CropInstance>(entity).ok().and_then(|crop| {
                    let stages: Vec<&str> = plant_registry
                        .and_then(|reg| reg.get(&crop.crop_def_id))
                        .map(|d| d.stages())
                        .unwrap_or_else(|| default_stages.clone());
                    let mature = stages.last().map(|l| crop.growth_stage == *l).unwrap_or(false);
                    if mature {
                        Some(crop.crop_def_id.clone())
                    } else {
                        None
                    }
                });
                if let Some(plant_id) = plant_id {
                    if let Some(yield_item) = harvest_item_for(&plant_id, item_registry) {
                        // Yield range from the plant def. Yields are FRACTIONAL (f32):
                        // saffron's 0.3 means less than one unit per plant per harvest.
                        // Sanitize the window (min >= 0, max >= min); unknown plants
                        // fall back to exactly 1 unit as before.
                        let (ymin, ymax) = plant_registry
                            .and_then(|reg| reg.get(&plant_id))
                            .map(|d| {
                                let lo = d.yield_min.max(0.0);
                                (lo, d.yield_max.max(lo))
                            })
                            .unwrap_or((1.0, 1.0));
                        // Roll a continuous yield in [ymin, ymax], then convert to a
                        // whole item count by PROBABILISTIC ROUNDING (floor + Bernoulli
                        // on the fraction): a 0.3 roll yields 1 item 30% of the time and
                        // 0 items 70%, so the expected value equals the roll. Fractional
                        // crops average their real output over repeated harvests instead
                        // of being silently rounded up to a full unit (3x inflation for
                        // saffron) or floored to permanent zero.
                        let rolled = ymin + rand::random::<f32>() * (ymax - ymin);
                        let qty = rolled.floor() as u32
                            + u32::from(rand::random::<f32>() < rolled.fract());
                        let max_stack =
                            item_registry.map(|r| r.max_stack_for(&yield_item)).unwrap_or(99);
                        for (_e, (inv, _ctrl)) in world.query_mut::<(
                            &mut crate::systems::inventory::Inventory,
                            &crate::ecs::components::Controllable,
                        )>() {
                            // Volume-gated (Stage A slice 2): a full pack loses
                            // the surplus — logged so it never vanishes silently.
                            let unit_vol =
                                item_registry.map(|r| r.volume_for(&yield_item)).unwrap_or(0.0);
                            let lost = inv.add_item_volume_gated(&yield_item, qty, max_stack, unit_vol);
                            if lost > 0 {
                                log::warn!("[Farming] pack full: {lost}x {yield_item} lost at harvest");
                            }
                            // Saved-seed loop (operator's "harvest yields seeds"):
                            // a SURVIVAL harvest returns a few seeds of this plant, so
                            // the garden is self-sustaining (plant 1 -> harvest -> get 2
                            // back -> replant + surplus). Creative needs no seeds, so it
                            // stays clean. Plot-agnostic: works for any plot type.
                            if !creative {
                                let seed_id = format!("seed_{plant_id}_0");
                                let seed_stack =
                                    item_registry.map(|r| r.max_stack_for(&seed_id)).unwrap_or(99);
                                let seed_vol =
                                    item_registry.map(|r| r.volume_for(&seed_id)).unwrap_or(0.0);
                                inv.add_item_volume_gated(&seed_id, 2, seed_stack, seed_vol);
                            }
                            log::info!("[Farming] harvested {qty}x {yield_item} from {plant_id}");
                            // Harvesting trains Farming (scales lightly with yield).
                            crate::systems::skills::award_skill_xp(data, "farming", 10 + qty * 2);
                            // Quest progress: a harvest of this crop (Harvest objectives).
                            crate::systems::quests::push_quest_event(
                                data,
                                format!("harvest_{}", plant_id),
                            );
                            break;
                        }
                    } else {
                        log::warn!(
                            "[Farming] {plant_id} has no produce item in items.csv; harvest yielded nothing"
                        );
                    }
                    let _ = world.despawn(entity);
                }
            }
        }

        // Collect entities to update (avoid borrow conflict with world)
        let mut updates: Vec<(hecs::Entity, CropInstance)> = Vec::new();

        for (entity, crop) in world.query_mut::<&CropInstance>() {
            // Skip dead crops
            if crop.growth_stage == STAGE_DEAD {
                continue;
            }

            // Resolve this plant's stage list
            let plant_stages: Vec<&str> = plant_registry
                .as_ref()
                .and_then(|reg| reg.get(&crop.crop_def_id))
                .map(|def| def.stages())
                .unwrap_or_else(|| default_stages.clone());

            // Skip crops already at their final stage (they sit until harvested)
            if let Some(last) = plant_stages.last() {
                if crop.growth_stage == *last {
                    continue;
                }
            }

            let mut crop = crop.clone();

            // Dehydration: water level drops over time
            crop.water_level = (crop.water_level - DEHYDRATION_RATE * dt).max(0.0);

            // Per-area irrigation: if the crop's grow area is configured with a water
            // target, automated irrigation keeps it topped up to that level. A high
            // setting holds crops well-watered (healthy, fast growth); a low setting
            // lets them dehydrate and wilt -- so the garden edit slider is meaningful.
            // GATED on the home's water sim (v0.611): a dry cistern can't feed the
            // irrigation, so the top-up is suppressed and the crops dehydrate.
            if water_available {
                if let Some(tid) = &crop.tower_id {
                    if let Some(target) = irrigation.get(tid) {
                        crop.water_level = crop.water_level.max(*target);
                    }
                }
            }

            // Health effects from water level
            if crop.water_level < WATER_STRESS_THRESHOLD {
                // Water stress -- health decays
                crop.health = (crop.health - HEALTH_DECAY_RATE * dt).max(0.0);
            } else {
                // Well watered -- health recovers toward 100
                crop.health = (crop.health + HEALTH_RECOVERY_RATE * dt).min(100.0);
            }

            // RF stress (v0.620): a powered wireless emitter (WiFi router) bathes the grow in RF; crops
            // lose health proportional to the home RF level. Run wired / Li-Fi or remove the emitter to
            // protect the grow (the operator's "tradeoffs bite"). Outpaces recovery at one router's worth.
            if home_rf > RF_HARM_THRESHOLD {
                crop.health = (crop.health - RF_HEALTH_PENALTY * home_rf * dt).max(0.0);
            }

            // If health hits zero, crop dies
            if crop.health <= 0.0 {
                crop.growth_stage = STAGE_DEAD.to_string();
                updates.push((entity, crop));
                continue;
            }

            // Calculate growth progress based on elapsed time since planting
            if let Some(registry) = plant_registry {
                if let Some(plant_def) = registry.get(&crop.crop_def_id) {
                    // Total growth time in game seconds
                    let growth_seconds = plant_def.growth_days as f64 * SECONDS_PER_DAY;

                    if growth_seconds > 0.0 {
                        let age = elapsed_seconds - crop.planted_at;
                        let progress = (age / growth_seconds) as f32;

                        // Health-weighted progress: unhealthy crops grow slower
                        let health_factor = (crop.health / 100.0).max(0.1);
                        // Per-area nutrient strength (garden edit slider) scales growth
                        // speed: the 0..1 slider maps to a 0.5x..1.5x multiplier, so the
                        // default 0.5 is neutral, a rich feed grows faster, a starved
                        // area grows slower. Un-configured crops grow at the 1.0x base.
                        let nutrient_factor = crop
                            .tower_id
                            .as_ref()
                            .and_then(|tid| nutrient.get(tid))
                            .map_or(1.0, |n| 0.5 + n);
                        let effective_progress = progress * health_factor * nutrient_factor;

                        let new_stage =
                            stage_from_progress(effective_progress, &plant_stages);

                        // Only advance forward, never regress (except to Dead)
                        let current_idx = stage_index(&crop.growth_stage, &plant_stages);
                        let new_idx = stage_index(new_stage, &plant_stages);

                        if let (Some(cur), Some(nxt)) = (current_idx, new_idx) {
                            if nxt > cur {
                                crop.growth_stage = new_stage.to_string();
                                log::debug!(
                                    "Crop {} advanced to {}",
                                    crop.crop_def_id,
                                    crop.growth_stage
                                );
                            }
                        }
                    }
                }
            }

            updates.push((entity, crop));
        }

        // Apply updates back to the world
        for (entity, crop) in updates {
            if let Ok(mut existing) = world.get::<&mut CropInstance>(entity) {
                *existing = crop;
            }
        }

        self._initialized = true;
    }
}

#[cfg(test)]
mod gardening_tests {
    use super::*;
    use crate::ecs::components::{Controllable, CropInstance};
    use crate::ecs::systems::System;
    use crate::hot_reload::data_store::DataStore;
    use crate::systems::inventory::{Inventory, ItemRegistry};

    /// DataStore with plant + item registries and the four gardening channels,
    /// mirroring the runtime wiring in lib.rs.
    fn make_store() -> DataStore {
        let mut data = DataStore::new();
        let plants = PlantRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/plants.csv"
        )))
        .expect("plants.csv");
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        data.insert("plant_registry", plants);
        data.insert("item_registry", items);
        data.insert(
            "game_time",
            std::sync::Mutex::new(crate::systems::time::GameTime::default()),
        );
        data.insert("plant_request", std::sync::Mutex::new(Option::<String>::None));
        data.insert(
            "plant_tower_request",
            std::sync::Mutex::new(Option::<(String, Vec<String>)>::None),
        );
        data.insert("water_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("harvest_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("dev_grow_crops", std::sync::Mutex::new(false));
        data.insert(
            "fertilize_crop_request",
            std::sync::Mutex::new(Option::<u64>::None),
        );
        data.insert(
            "stock_seeds_request",
            std::sync::Mutex::new(Option::<Vec<String>>::None),
        );
        data
    }

    fn set_string(data: &DataStore, key: &str, v: &str) {
        *data
            .get::<std::sync::Mutex<Option<String>>>(key)
            .unwrap()
            .lock()
            .unwrap() = Some(v.to_string());
    }

    /// Full gardening loop: plant a seed (consumed → crop spawned) → dev-grow to
    /// maturity → harvest (produce yielded into the player + crop despawned).
    #[test]
    fn plant_grow_harvest_full_loop() {
        let data = make_store();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("seed_tomato_0", 1, 99);
        let player = world.spawn((inv, Controllable));

        // PLANT.
        set_string(&data, "plant_request", "seed_tomato_0");
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("seed_tomato_0"),
            0,
            "seed consumed by planting"
        );
        let crops: Vec<hecs::Entity> =
            world.query::<&CropInstance>().iter().map(|(e, _)| e).collect();
        assert_eq!(crops.len(), 1, "exactly one crop planted");
        let crop_entity = crops[0];
        assert_eq!(
            world.get::<&CropInstance>(crop_entity).unwrap().crop_def_id,
            "tomato",
            "seed mapped to the tomato plant def"
        );

        // DEV-GROW to maturity (tomato's last stage is `ripe`).
        *data.get::<std::sync::Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.get::<&CropInstance>(crop_entity).unwrap().growth_stage,
            "ripe",
            "dev-grow matured the crop"
        );

        // HARVEST: yield produce + despawn the crop.
        *data
            .get::<std::sync::Mutex<Option<u64>>>("harvest_request")
            .unwrap()
            .lock()
            .unwrap() = Some(crop_entity.to_bits().into());
        sys.tick(&mut world, 1.0, &data);
        assert!(
            world.get::<&CropInstance>(crop_entity).is_err(),
            "harvested crop was despawned"
        );
        let tomatoes = world
            .get::<&Inventory>(player)
            .unwrap()
            .count_item("vegetable_tomato_0");
        assert!(
            tomatoes >= 2,
            "harvest yielded produce (>= yield_min 2 tomatoes), got {tomatoes}"
        );
        // Saved-seed loop: this survival harvest returned seeds (planted 1 -> 0, then
        // the harvest granted 2 back), so the garden is self-sustaining.
        let seeds = world.get::<&Inventory>(player).unwrap().count_item("seed_tomato_0");
        assert_eq!(seeds, 2, "survival harvest yielded 2 seeds, got {seeds}");
    }

    /// Creative mode: planting spawns a crop WITHOUT needing or consuming a seed,
    /// so the seed economy can be built out before it bites. (Survival mode, the
    /// absent-flag default, still consumes — proven by plant_grow_harvest_full_loop.)
    #[test]
    fn creative_mode_plants_without_consuming_seed() {
        let mut data = make_store();
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let mut sys = FarmingSystem::new();

        // Player holds NO seeds — creative mode plants anyway.
        let mut world = hecs::World::new();
        let _player = world.spawn((Inventory::new(16), Controllable));
        set_string(&data, "plant_request", "seed_tomato_0");
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            1,
            "creative mode planted a crop with no seed in inventory"
        );

        // And a held seed is NOT consumed in creative mode.
        let mut world2 = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("seed_tomato_0", 1, 99);
        let p2 = world2.spawn((inv, Controllable));
        set_string(&data, "plant_request", "seed_tomato_0");
        sys.tick(&mut world2, 1.0, &data);
        assert_eq!(
            world2.get::<&Inventory>(p2).unwrap().count_item("seed_tomato_0"),
            1,
            "creative mode did not consume the held seed"
        );
    }

    /// Survival mode: planting a tower consumes one seed per variety and skips
    /// varieties the player has no seed for.
    #[test]
    fn survival_tower_planting_consumes_seeds() {
        let data = make_store(); // no creative flag = survival = consume
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("seed_tomato_0", 1, 99); // has tomato, lacks lettuce
        let player = world.spawn((inv, Controllable));

        *data
            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("nutrition".to_string(), vec!["tomato".to_string(), "lettuce".to_string()]));
        sys.tick(&mut world, 1.0, &data);

        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            1,
            "only the seeded variety (tomato) was planted; lettuce skipped"
        );
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("seed_tomato_0"),
            0,
            "the tomato seed was consumed"
        );
    }

    /// Creative mode: planting a tower spawns every variety free, no seeds needed.
    #[test]
    fn creative_tower_planting_is_free() {
        let mut data = make_store();
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let _player = world.spawn((Inventory::new(16), Controllable)); // no seeds
        *data
            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("nutrition".to_string(), vec!["tomato".to_string(), "lettuce".to_string()]));
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            2,
            "creative planted both varieties free"
        );
    }

    /// Replanting a tower FILLS its fixed slots idempotently — it must NOT stack a
    /// fresh set of crops each time (the v0.410 fix for the 33 -> 66 -> 99 bug).
    #[test]
    fn tower_replant_fills_slots_idempotently() {
        let mut data = make_store();
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let _player = world.spawn((Inventory::new(16), Controllable));
        let set_req = |data: &DataStore| {
            *data
                .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
                .unwrap()
                .lock()
                .unwrap() = Some(("nutrition".to_string(), vec!["tomato".to_string(), "lettuce".to_string()]));
        };
        set_req(&data);
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(world.query::<&CropInstance>().iter().count(), 2, "first plant fills 2 slots");
        // Plant AGAIN: slots already occupied -> still 2 crops, not 4.
        set_req(&data);
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            2,
            "replant is idempotent (no stacking)"
        );
        let slots: Vec<Option<u32>> =
            world.query::<&CropInstance>().iter().map(|(_, c)| c.tower_slot).collect();
        assert!(slots.contains(&Some(0)) && slots.contains(&Some(1)), "slots 0 and 1 recorded");
    }

    #[test]
    fn seed_and_harvest_id_mapping() {
        assert_eq!(plant_id_from_seed("seed_tomato_0").as_deref(), Some("tomato"));
        assert_eq!(plant_id_from_seed("seed_sweet_potato_0").as_deref(), Some("sweet_potato"));
        assert_eq!(plant_id_from_seed("iron_ore_0"), None);
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        assert_eq!(
            harvest_item_for("tomato", Some(&items)).as_deref(),
            Some("vegetable_tomato_0")
        );
        // A plant with no produce item in items.csv yields nothing (no crash).
        assert_eq!(harvest_item_for("void_orchid", Some(&items)), None);
    }

    /// Fertilizing a crop consumes one fertilizer_0 from the player and boosts the
    /// crop's health (closing the compost → fertilizer → crop cycle).
    #[test]
    fn fertilize_consumes_fertilizer_and_boosts_crop_health() {
        use crate::ecs::components::{Controllable, CropInstance};
        let data = make_store();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item("fertilizer_0", 2, 99);
        world.spawn((inv, Controllable));
        let crop = world.spawn((CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 0.5,
            health: 40.0,
            tower_id: None,
            tower_slot: None,
        },));

        *data
            .get::<std::sync::Mutex<Option<u64>>>("fertilize_crop_request")
            .unwrap()
            .lock()
            .unwrap() = Some(crop.to_bits().into());
        sys.tick(&mut world, 1.0, &data);

        let fert_count = world
            .query::<(&Inventory, &Controllable)>()
            .iter()
            .next()
            .map(|(_, (i, _))| i.count_item("fertilizer_0"))
            .unwrap();
        assert_eq!(fert_count, 1, "fertilizing consumed one fertilizer_0");
        assert!(
            world.get::<&CropInstance>(crop).unwrap().health > 40.0,
            "fertilizing boosted the crop's health"
        );
    }

    /// Per-area irrigation: a crop whose grow area is configured with a water target
    /// (the garden edit modal's water slider) stays topped up and holds health, while
    /// an un-configured crop dehydrates and loses health. Proves the slider is wired
    /// through to the sim -- editing a grow area actually changes crop survival.
    #[test]
    fn per_area_irrigation_keeps_configured_crops_watered() {
        use crate::ecs::components::CropInstance;
        let mut data = make_store();
        // Configure the "nutrition" tower for full irrigation (water slider = 1.0).
        let mut irr = std::collections::HashMap::new();
        irr.insert("nutrition".to_string(), 1.0_f32);
        data.insert("garden_irrigation", std::sync::Mutex::new(irr));

        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let dry = |tower: &str| CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 0.15, // below WATER_STRESS_THRESHOLD
            health: 80.0,
            tower_id: Some(tower.to_string()),
            tower_slot: Some(0),
        };
        // Irrigated crop lives in the configured "nutrition" tower.
        let irrigated = world.spawn((dry("nutrition"),));
        // Parched crop lives in an un-configured tower (not in the irrigation map).
        let parched = world.spawn((dry("apothecary"),));

        for _ in 0..5 {
            sys.tick(&mut world, 1.0, &data);
        }

        let irr_c = world.get::<&CropInstance>(irrigated).unwrap();
        let dry_c = world.get::<&CropInstance>(parched).unwrap();
        assert!(
            irr_c.water_level > 0.9,
            "irrigated crop stays topped up, got {}",
            irr_c.water_level
        );
        assert!(
            irr_c.health >= 80.0,
            "irrigated crop held/recovered health, got {}",
            irr_c.health
        );
        assert!(
            dry_c.water_level < irr_c.water_level,
            "un-irrigated crop is drier ({} vs {})",
            dry_c.water_level,
            irr_c.water_level
        );
        assert!(
            dry_c.health < irr_c.health,
            "un-irrigated crop lost health vs the irrigated one ({} vs {})",
            dry_c.health,
            irr_c.health
        );
    }

    /// Water -> FOOD coupling (v0.611): the SAME configured irrigation that keeps a crop topped up with
    /// a full cistern FAILS when the cistern is dry, so the crop dehydrates + loses health. This is the
    /// downstream end of power -> water -> food (a power cut drains the cistern, then the garden wilts).
    #[test]
    fn dry_cistern_stops_irrigation_and_wilts_crops() {
        use crate::ecs::components::CropInstance;
        use crate::systems::plumbing::WaterStatus;

        let configured_crop = || CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 0.15, // below WATER_STRESS_THRESHOLD
            health: 80.0,
            tower_id: Some("nutrition".to_string()),
            tower_slot: Some(0),
        };

        // Control: a FULL cistern -> irrigation works -> the crop stays topped up.
        let mut full = make_store();
        let mut irr = std::collections::HashMap::new();
        irr.insert("nutrition".to_string(), 1.0_f32);
        full.insert("garden_irrigation", std::sync::Mutex::new(irr.clone()));
        full.insert("water_status", std::sync::Mutex::new(WaterStatus { stored_l: 7000.0, capacity_l: 8000.0, ..Default::default() }));
        let mut sys = FarmingSystem::new();
        let mut w_full = hecs::World::new();
        let c_full = w_full.spawn((configured_crop(),));
        for _ in 0..5 { sys.tick(&mut w_full, 1.0, &full); }
        let wet = w_full.get::<&CropInstance>(c_full).unwrap().water_level;
        assert!(wet > 0.9, "full cistern -> irrigation tops the crop up, got {wet}");

        // A DRY cistern (same capacity) -> irrigation can't deliver -> the crop dehydrates.
        let mut empty = make_store();
        empty.insert("garden_irrigation", std::sync::Mutex::new(irr));
        empty.insert("water_status", std::sync::Mutex::new(WaterStatus { stored_l: 0.0, capacity_l: 8000.0, ..Default::default() }));
        let mut w_dry = hecs::World::new();
        let c_dry = w_dry.spawn((configured_crop(),));
        for _ in 0..5 { sys.tick(&mut w_dry, 1.0, &empty); }
        let dry = w_dry.get::<&CropInstance>(c_dry).unwrap();
        assert!(dry.water_level < 0.15, "dry cistern -> the crop dehydrates (not topped up), got {}", dry.water_level);
        assert!(dry.health < 80.0, "dry cistern -> the water-stressed crop loses health, got {}", dry.health);
    }

    /// RF -> FOOD coupling (v0.620): a POWERED WiFi router (RF emitter) harms a well-watered crop (RF
    /// stress outpaces recovery); with NO emitter the same crop holds/recovers. The operator's tradeoff.
    #[test]
    fn powered_rf_emitter_harms_crops() {
        use crate::ecs::components::{CropInstance, PowerConsumer, RfEmitter};
        let well_watered = || CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 1.0, // not water-stressed, so we isolate RF
            health: 80.0,
            tower_id: None,
            tower_slot: None,
        };
        let data = make_store();
        let mut sys = FarmingSystem::new();

        // A powered WiFi router (RF 0.6) bathes the grow -> the crop loses health.
        let mut world = hecs::World::new();
        let c = world.spawn((well_watered(),));
        world.spawn((RfEmitter { strength: 0.6, needs_power: true }, PowerConsumer { draw_watts: 8.0, priority: 4, enabled: true }));
        for _ in 0..5 { sys.tick(&mut world, 1.0, &data); }
        let harmed = world.get::<&CropInstance>(c).unwrap().health;
        assert!(harmed < 80.0, "powered RF harms the crop, got {harmed}");

        // No emitter -> the same well-watered crop holds or recovers.
        let mut world2 = hecs::World::new();
        let c2 = world2.spawn((well_watered(),));
        for _ in 0..5 { sys.tick(&mut world2, 1.0, &data); }
        let safe = world2.get::<&CropInstance>(c2).unwrap().health;
        assert!(safe >= 80.0, "no RF -> the crop holds/recovers, got {safe}");
    }

    /// Per-area nutrient strength (garden edit slider) scales growth speed: a
    /// rich-fed tower (nutrient 1.0 -> 1.5x) grows further in the same elapsed time
    /// than a starved one (nutrient 0.0 -> 0.5x), proving the nutrient slider reaches
    /// the sim. Both crops are equally healthy + watered, so only the feed differs.
    #[test]
    fn per_area_nutrient_speeds_growth() {
        use crate::ecs::components::CropInstance;
        let mut data = make_store();
        let mut nut = std::collections::HashMap::new();
        nut.insert("nutrition".to_string(), 1.0_f32); // rich feed -> 1.5x
        nut.insert("apothecary".to_string(), 0.0_f32); // starved   -> 0.5x
        data.insert("garden_nutrient", std::sync::Mutex::new(nut));

        // Advance game time to 60% of tomato's growth window so the two feed rates
        // land the crops on different stages.
        let (growth_seconds, stages): (f64, Vec<&str>) = {
            let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
            let def = reg.get("tomato").unwrap();
            (def.growth_days as f64 * SECONDS_PER_DAY, def.stages())
        };
        {
            let gt = data
                .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
                .unwrap();
            gt.lock().unwrap().elapsed_seconds = growth_seconds * 0.6;
        }

        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let young = |tower: &str| CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: stages[0].to_string(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(tower.to_string()),
            tower_slot: Some(0),
        };
        let rich = world.spawn((young("nutrition"),));
        let starved = world.spawn((young("apothecary"),));
        sys.tick(&mut world, 1.0, &data);

        let rich_c = world.get::<&CropInstance>(rich).unwrap();
        let starved_c = world.get::<&CropInstance>(starved).unwrap();
        let rich_idx = stage_index(&rich_c.growth_stage, &stages).unwrap();
        let starved_idx = stage_index(&starved_c.growth_stage, &stages).unwrap();
        assert!(
            rich_idx > starved_idx,
            "rich-fed crop ({}, idx {}) outgrew starved ({}, idx {})",
            rich_c.growth_stage,
            rich_idx,
            starved_c.growth_stage,
            starved_idx
        );
    }
}
