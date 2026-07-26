use crate::hot_reload::data_store::DataStore;
use crate::systems::skills::SkillRegistry;
use crate::systems::quests::QuestRegistry;

/// Load the small, runtime-critical data registries (items, recipes, plants,
/// status effects, skills, quests, containers) into the DataStore.
///
/// These are cheap CSV/RON parses the GAME SYSTEMS read every tick, so they MUST
/// load EAGERLY at startup — not lazily in `load_world` (which only runs when you
/// switch to the 3D world view). The menu-driven loops (inventory / crafting /
/// skills / quests) otherwise run against empty registries: raw item ids, no
/// recipes to craft, no skill names, the quest shown by its raw id. (The heavy 3D
/// mesh generation stays lazy in `load_world`.) Idempotent — safe to call twice.
pub(crate) fn load_data_registries(store: &mut DataStore, data_dir: &std::path::Path) {
    fn load_csv_registry<T: Send + Sync + 'static>(
        store: &mut DataStore,
        path: std::path::PathBuf,
        key: &str,
        build: impl Fn(&[u8]) -> Result<T, String>,
    ) {
        // Disk-first (modding), embedded fallback (v0.744: a zero-file
        // fresh install still gets every registry). The embedded key is
        // the path relative to any `data` dir component.
        let bytes: Option<Vec<u8>> = match std::fs::read(&path) {
            Ok(b) => Some(b),
            Err(_) => {
                // Relative key = everything after the LAST `data` path
                // component (the data dir itself always ends in `data`).
                let comps: Vec<_> = path.iter().collect();
                let rel = comps
                    .iter()
                    .rposition(|c| *c == std::ffi::OsStr::new("data"))
                    .map(|i| {
                        comps[i + 1..]
                            .iter()
                            .map(|c| c.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join("/")
                    })
                    .unwrap_or_default();
                crate::embedded_data::get_embedded(&rel).map(|s| {
                    log::info!("{key}: disk file absent, using embedded {rel}");
                    s.as_bytes().to_vec()
                })
            }
        };
        match bytes {
            Some(bytes) => match build(&bytes) {
                Ok(reg) => {
                    store.insert(key, reg);
                    log::info!("Loaded {key} from {}", path.display());
                }
                Err(e) => log::warn!("Failed to build {key} from {}: {e}", path.display()),
            },
            None => log::warn!(
                "Data file {} not found (no embedded copy); {key} unavailable (system on defaults)",
                path.display()
            ),
        }
    }
    load_csv_registry(
        store,
        data_dir.join("items.csv"),
        "item_registry",
        crate::systems::inventory::ItemRegistry::from_csv,
    );
    load_csv_registry(
        store,
        data_dir.join("recipes.csv"),
        "recipe_registry",
        crate::systems::crafting::RecipeRegistry::from_csv,
    );
    load_csv_registry(
        store,
        data_dir.join("plants.csv"),
        "plant_registry",
        crate::systems::farming::PlantRegistry::from_csv,
    );
    load_csv_registry(
        store,
        data_dir.join("status_effects.csv"),
        "status_effect_registry",
        crate::systems::status_effects::StatusEffectRegistry::from_csv,
    );
    load_csv_registry(
        store,
        data_dir.join("skills").join("skills.csv"),
        "skill_registry",
        SkillRegistry::from_csv,
    );
    load_csv_registry(
        store,
        data_dir.join("equipment.csv"),
        "equipment_registry",
        crate::systems::economy::EquipmentRegistry::from_csv,
    );
    // CreatureRegistry (v0.751, ladder rung 7): creatures.csv finally gets
    // its loader. Read by the livestock spawn + walk-up collect bridges.
    load_csv_registry(
        store,
        data_dir.join("creatures.csv"),
        "creature_registry",
        crate::systems::livestock::CreatureRegistry::from_csv,
    );
    // AbilityRegistry (v0.753, ladder rung 8): abilities.csv (formerly
    // spells.csv) gets its loader. Read by AbilitySystem + the Profile
    // page's Abilities panel.
    load_csv_registry(
        store,
        data_dir.join("abilities.csv"),
        "ability_registry",
        crate::systems::abilities::AbilityRegistry::from_csv,
    );
    store.insert(
        "quest_registry",
        QuestRegistry::from_ron_dir(&data_dir.join("quests")),
    );
    // Travel destinations (v0.979): named world places whose arrival radius
    // fires "travel_<id>" quest events. Read by QuestSystem::tick.
    match crate::embedded_data::read_data_or_embedded(data_dir, "entities/destinations.ron") {
        Some(text) => match crate::systems::quests::DestinationList::from_ron(text.as_bytes()) {
            Ok(list) => {
                log::info!(
                    "Loaded {} travel destinations from entities/destinations.ron",
                    list.destinations.len()
                );
                store.insert("quest_destinations", list);
            }
            Err(e) => log::warn!("Failed to parse entities/destinations.ron: {e}"),
        },
        None => log::warn!(
            "entities/destinations.ron not found (no embedded copy); Travel objectives cannot advance"
        ),
    }
    // BlueprintRegistry: read by ConstructionSystem::tick (registered 2026-07-01, see
    // lib.rs's system_runner.register list) -- basic.ron already ships a real
    // foundation/wall/door/window/roof/furniture/machine catalog that had nothing
    // loading it into the DataStore before this, so every queue_build() call would
    // have silently missed the registry lookup forever.
    match crate::embedded_data::read_data_or_embedded(data_dir, "blueprints/basic.ron") {
        Some(text) => match crate::systems::construction::BlueprintRegistry::from_ron(text.as_bytes()) {
            Ok(reg) => {
                log::info!("Loaded {} blueprints from blueprints/basic.ron", reg.blueprints.len());
                store.insert("blueprint_registry", reg);
            }
            Err(e) => log::warn!("Failed to parse blueprints/basic.ron: {e}"),
        },
        None => log::warn!(
            "blueprints/basic.ron not found (no embedded copy); blueprint_registry unavailable (ConstructionSystem idle)"
        ),
    }
    // LivestockSpawnList (v0.751, ladder rung 7): which starter animals the
    // homestead gets and near which field. Read by load_world's spawn pass.
    match crate::embedded_data::read_data_or_embedded(data_dir, "entities/livestock.ron") {
        Some(text) => match crate::systems::livestock::LivestockSpawnList::from_ron(text.as_bytes()) {
            Ok(list) => {
                log::info!("Loaded {} livestock placements from entities/livestock.ron", list.animals.len());
                store.insert("livestock_spawn_list", list);
            }
            Err(e) => log::warn!("Failed to parse entities/livestock.ron: {e}"),
        },
        None => log::warn!("entities/livestock.ron not found (no embedded copy); no starter animals"),
    }
    // WildSpawnList (v0.761, combat arc): hostile creatures placed away
    // from the homestead. Read by load_world's wild spawn pass.
    match crate::embedded_data::read_data_or_embedded(data_dir, "entities/wild_spawns.ron") {
        Some(text) => match crate::systems::livestock::WildSpawnList::from_ron(text.as_bytes()) {
            Ok(list) => {
                log::info!("Loaded {} wild spawn placements from entities/wild_spawns.ron", list.spawns.len());
                store.insert("wild_spawn_list", list);
            }
            Err(e) => log::warn!("Failed to parse entities/wild_spawns.ron: {e}"),
        },
        None => log::warn!("entities/wild_spawns.ron not found (no embedded copy); no wild creatures"),
    }
    // VehicleKitRegistry: which inventory KIT item deploys into which vehicle
    // (economy Phase 2 Stage 1). Read by VehicleSystem's deploy arm + the
    // deployed-vehicle render pass.
    // TradeGoodsRegistry (v0.747, ladder rung 3): base credit values for the
    // vendor economy. Read by the vendor bridge + the vendor modal.
    match crate::embedded_data::read_data_or_embedded(data_dir, "trade_goods.ron") {
        Some(text) => match crate::systems::economy::TradeGoodsRegistry::from_ron(text.as_bytes()) {
            Ok(reg) => {
                log::info!("Loaded {} trade goods from trade_goods.ron", reg.len());
                store.insert("trade_goods_registry", reg);
            }
            Err(e) => log::warn!("Failed to parse trade_goods.ron: {e}"),
        },
        None => log::warn!("trade_goods.ron not found (no embedded copy); vendors disabled"),
    }
    match crate::embedded_data::read_data_or_embedded(data_dir, "vehicles/kits.ron") {
        Some(text) => match crate::systems::vehicles::VehicleKitRegistry::from_ron(text.as_bytes()) {
            Ok(reg) => {
                log::info!("Loaded {} vehicle kits from vehicles/kits.ron", reg.len());
                store.insert("vehicle_kit_registry", reg);
            }
            Err(e) => log::warn!("Failed to parse vehicles/kits.ron: {e}"),
        },
        None => log::warn!(
            "vehicles/kits.ron not found (no embedded copy); vehicle_kit_registry unavailable (kit deploy refused)"
        ),
    }
    // Container types + content-class compatibility (disk-first, embedded fallback).
    {
        use crate::systems::inventory::containers::ContainerRegistry;
        let types = crate::embedded_data::read_data_or_embedded(data_dir, "containers/types.csv");
        let classes =
            crate::embedded_data::read_data_or_embedded(data_dir, "containers/content_classes.ron");
        match (types, classes) {
            (Some(types_text), Some(classes_text)) => {
                match ContainerRegistry::from_bytes(types_text.as_bytes(), classes_text.as_bytes()) {
                    Ok(reg) => {
                        log::info!(
                            "Loaded ContainerRegistry: {} container types, {} content classes",
                            reg.types.len(),
                            reg.content_classes.len()
                        );
                        store.insert("container_registry", reg);
                    }
                    Err(e) => log::warn!("ContainerRegistry parse failed: {e}"),
                }
            }
            _ => log::warn!(
                "Container data not found (containers/types.csv + content_classes.ron, no embedded copy); container compatibility disabled"
            ),
        }
    }
}
