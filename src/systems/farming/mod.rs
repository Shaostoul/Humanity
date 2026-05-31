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
    /// Harvest yield range (units of produce per fully-grown plant).
    pub yield_min: u32,
    pub yield_max: u32,
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
                },
            );
        }
        Ok(Self { plants })
    }
}

/// One row of `plants.csv` — only the columns `PlantRegistry` consumes (the
/// nutrient/ph/temp/humidity/yield/value columns are ignored for now).
#[derive(Debug, Deserialize)]
struct PlantRow {
    id: String,
    name: String,
    #[serde(default)]
    growth_days: f32,
    #[serde(default)]
    water_liters_per_day: f32,
    #[serde(default)]
    growth_stages: String,
    #[serde(default)]
    seasons: String,
    #[serde(default)]
    yield_min: u32,
    #[serde(default)]
    yield_max: u32,
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
}

/// Rate at which water_level decreases per second (base dehydration).
const DEHYDRATION_RATE: f32 = 0.002;

/// Water level below which crop health starts dropping.
const WATER_STRESS_THRESHOLD: f32 = 0.2;

/// Health recovery rate per second when well-watered.
const HEALTH_RECOVERY_RATE: f32 = 0.5;

/// Health decay rate per second when water-stressed.
const HEALTH_DECAY_RATE: f32 = 1.0;

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
                        if inv.has_item(&seed_id, 1) {
                            inv.remove_item(&seed_id, 1);
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
                        },));
                        log::info!("[Farming] planted {plant_id} (from {seed_id})");
                    }
                } else {
                    log::debug!("[Farming] no plant def for seed {seed_id}; not planted");
                }
            }
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
                    if inv.has_item("fertilizer_0", 1) {
                        inv.remove_item("fertilizer_0", 1);
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
                        let (ymin, ymax) = plant_registry
                            .and_then(|reg| reg.get(&plant_id))
                            .map(|d| (d.yield_min.max(1), d.yield_max.max(d.yield_min).max(1)))
                            .unwrap_or((1, 1));
                        let qty = if ymax > ymin {
                            ymin + (rand::random::<u32>() % (ymax - ymin + 1))
                        } else {
                            ymin
                        };
                        let max_stack =
                            item_registry.map(|r| r.max_stack_for(&yield_item)).unwrap_or(99);
                        for (_e, (inv, _ctrl)) in world.query_mut::<(
                            &mut crate::systems::inventory::Inventory,
                            &crate::ecs::components::Controllable,
                        )>() {
                            inv.add_item(&yield_item, qty, max_stack);
                            log::info!("[Farming] harvested {qty}x {yield_item} from {plant_id}");
                            // Harvesting trains Farming (scales lightly with yield).
                            crate::systems::skills::award_skill_xp(data, "farming", 10 + qty * 2);
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

            // Health effects from water level
            if crop.water_level < WATER_STRESS_THRESHOLD {
                // Water stress -- health decays
                crop.health = (crop.health - HEALTH_DECAY_RATE * dt).max(0.0);
            } else {
                // Well watered -- health recovers toward 100
                crop.health = (crop.health + HEALTH_RECOVERY_RATE * dt).min(100.0);
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
                        let effective_progress = progress * health_factor;

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
        data.insert("water_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("harvest_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("dev_grow_crops", std::sync::Mutex::new(false));
        data.insert(
            "fertilize_crop_request",
            std::sync::Mutex::new(Option::<u64>::None),
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
}
