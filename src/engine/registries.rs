use std::path::Path;

use crate::embedded_data::load_data_or_embedded;
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
///
/// EVERY file here loads through ONE rule, `embedded_data::load_data_or_embedded`
/// (BUG-163): the data folder's copy when this version can read all of it, so a
/// modded file wins; otherwise the copy built into the exe, with one log line
/// naming the file and why. An installed game's data folder can be older than the
/// game (an update replaces only the exe), and before this a stale file that the
/// code refused row by row loaded an EMPTY registry.
pub(crate) fn load_data_registries(store: &mut DataStore, data_dir: &Path) {
    /// One registry from one file, by the shared rule, into the store under `key`.
    fn put<T: Send + Sync + 'static>(
        store: &mut DataStore,
        data_dir: &Path,
        rel: &str,
        key: &str,
        build: impl Fn(&[u8]) -> Result<T, String>,
    ) {
        match load_data_or_embedded(data_dir, rel, build) {
            Ok(reg) => {
                store.insert(key, reg);
                log::info!("Loaded {key} (data/{rel})");
            }
            Err(e) => log::warn!("{e}; {key} unavailable (its system runs on defaults)"),
        }
    }
    put(store, data_dir, "items.csv", "item_registry", crate::systems::inventory::ItemRegistry::from_csv);
    // Garden nutrients, N-P-K (data/garden/nutrients.ron, 2026-09-26): the
    // farming system prefers this entry, so an edited file takes effect.
    store.insert("garden_nutrients", crate::systems::farming::soil::NutrientData::load());
    // Quality grades for hand-made goods (data/manufacturing.ron, 2026-09-26).
    store.insert("quality_levels", crate::systems::crafting::quality::load(data_dir));
    // Recipes, with the hand tools each needs (data/crafting/tools.ron, 2026-09-26).
    let tool_rules = crate::systems::crafting::tools::load(data_dir);
    put(store, data_dir, "recipes.csv", "recipe_registry", |b: &[u8]| {
        crate::systems::crafting::RecipeRegistry::from_csv(b).map(|r| r.with_tools(&tool_rules))
    });
    put(store, data_dir, "plants.csv", "plant_registry", crate::systems::farming::PlantRegistry::from_csv);

    // Environment region kinds (v0.1329). Same loader as every other registry:
    // it is bytes in, parsed table out, so the RON gets disk-first modding and
    // the embedded fallback for free (the built-in copy since BUG-163; with
    // neither there are no regions, which is inert rather than broken). See
    // docs/design/environment-fields.md.
    put(
        store,
        data_dir,
        "environment/region_kinds.ron",
        "region_kinds",
        crate::renderer::env_regions::RegionKinds::from_bytes,
    );
    // Environment Layer 1 (2026-09-27): each world's analytic climate, read by
    // WeatherSystem for the air at the player. Disk first so a modder's edit
    // wins; the embedded copy otherwise (and WeatherSystem falls back to the
    // same embedded copy if this key is ever absent).
    put(
        store,
        data_dir,
        "environment/climate.ron",
        crate::systems::env_layer1::STORE_KEY,
        crate::systems::env_layer1::ClimateTable::from_bytes,
    );
    put(
        store,
        data_dir,
        "status_effects.csv",
        "status_effect_registry",
        crate::systems::status_effects::StatusEffectRegistry::from_csv,
    );
    put(store, data_dir, "skills/skills.csv", "skill_registry", SkillRegistry::from_csv);
    put(store, data_dir, "equipment.csv", "equipment_registry", crate::systems::economy::EquipmentRegistry::from_csv);
    // CreatureRegistry (v0.751, ladder rung 7): creatures.csv finally gets
    // its loader. Read by the livestock spawn + walk-up collect bridges.
    put(store, data_dir, "creatures.csv", "creature_registry", crate::systems::livestock::CreatureRegistry::from_csv);
    // AbilityRegistry (v0.753, ladder rung 8): abilities.csv (formerly
    // spells.csv) gets its loader. Read by AbilitySystem + the Profile
    // page's Abilities panel.
    put(store, data_dir, "abilities.csv", "ability_registry", crate::systems::abilities::AbilityRegistry::from_csv);
    // Each quest file the game ships by the same rule, then any other in
    // data/quests/ (a modder's).
    store.insert("quest_registry", QuestRegistry::load(data_dir));
    // Travel destinations (v0.979): named world places; a Travel step is done
    // while the player stands in one and it is the current step (2026-10-04,
    // including the player's own front door). Read by QuestSystem::tick.
    match load_data_or_embedded(data_dir, "entities/destinations.ron", crate::systems::quests::DestinationList::from_ron) {
        Ok(list) => {
            log::info!("Loaded {} travel destinations from entities/destinations.ron", list.destinations.len());
            store.insert("quest_destinations", list);
        }
        Err(e) => log::warn!("{e}; Travel objectives cannot advance"),
    }
    // BlueprintRegistry: read by ConstructionSystem::tick (registered 2026-07-01, see
    // lib.rs's system_runner.register list) -- basic.ron already ships a real
    // foundation/wall/door/window/roof/furniture/machine catalog that had nothing
    // loading it into the DataStore before this, so every queue_build() call would
    // have silently missed the registry lookup forever.
    match load_data_or_embedded(data_dir, "blueprints/basic.ron", crate::systems::construction::BlueprintRegistry::from_ron) {
        Ok(reg) => {
            log::info!("Loaded {} blueprints from blueprints/basic.ron", reg.blueprints.len());
            store.insert("blueprint_registry", reg);
        }
        Err(e) => log::warn!("{e}; blueprint_registry unavailable (ConstructionSystem idle)"),
    }
    // LivestockSpawnList (v0.751, ladder rung 7): which starter animals the
    // homestead gets and near which field. Read by load_world's spawn pass.
    match load_data_or_embedded(data_dir, "entities/livestock.ron", crate::systems::livestock::LivestockSpawnList::from_ron) {
        Ok(list) => {
            log::info!("Loaded {} livestock placements from entities/livestock.ron", list.animals.len());
            store.insert("livestock_spawn_list", list);
        }
        Err(e) => log::warn!("{e}; no starter animals"),
    }
    // WildSpawnList (v0.761, combat arc): hostile creatures placed away
    // from the homestead. Read by load_world's wild spawn pass.
    match load_data_or_embedded(data_dir, "entities/wild_spawns.ron", crate::systems::livestock::WildSpawnList::from_ron) {
        Ok(list) => {
            log::info!("Loaded {} wild spawn placements from entities/wild_spawns.ron", list.spawns.len());
            store.insert("wild_spawn_list", list);
        }
        Err(e) => log::warn!("{e}; no wild creatures"),
    }
    // WeatherEventRegistry (v0.1034, extreme-weather schema increment):
    // tornado/blizzard/meteor-shower definitions. Loaded + validated now;
    // WeatherSystem starts CONSUMING it on the next rung (trigger rolls,
    // wind profiles, hazards) - until then it is inert data.
    match load_data_or_embedded(data_dir, "weather/events.ron", crate::systems::weather_events::WeatherEventRegistry::from_ron) {
        Ok(reg) => {
            log::info!("Loaded {} weather events from weather/events.ron", reg.len());
            store.insert("weather_event_registry", reg);
        }
        Err(e) => log::warn!("{e}; no extreme weather"),
    }
    // VehicleKitRegistry: which inventory KIT item deploys into which vehicle
    // (economy Phase 2 Stage 1). Read by VehicleSystem's deploy arm + the
    // deployed-vehicle render pass.
    // TradeGoodsRegistry (v0.747, ladder rung 3): base credit values for the
    // vendor economy. Read by the vendor bridge + the vendor modal.
    match load_data_or_embedded(data_dir, "trade_goods.ron", crate::systems::economy::TradeGoodsRegistry::from_ron) {
        Ok(reg) => {
            // The parts prices that cap what a better craft grade fetches
            // (BUG-146), from the items and recipes loaded above. With
            // either missing nothing can be crafted, so there is no loop
            // to stop and the goods go in without them.
            let reg = match (
                store.get::<crate::systems::inventory::ItemRegistry>("item_registry"),
                store.get::<crate::systems::crafting::RecipeRegistry>("recipe_registry"),
            ) {
                (Some(items), Some(recipes)) => reg.with_parts_prices(items, recipes),
                _ => {
                    log::warn!("trade goods loaded without items or recipes: no parts prices, every grade paid in full");
                    reg
                }
            };
            log::info!(
                "Loaded {} trade goods from trade_goods.ron ({} with a parts price)",
                reg.len(),
                reg.parts.len()
            );
            store.insert("trade_goods_registry", reg);
        }
        Err(e) => log::warn!("{e}; vendors disabled"),
    }
    match load_data_or_embedded(data_dir, "vehicles/kits.ron", crate::systems::vehicles::VehicleKitRegistry::from_ron) {
        Ok(reg) => {
            log::info!("Loaded {} vehicle kits from vehicles/kits.ron", reg.len());
            store.insert("vehicle_kit_registry", reg);
        }
        Err(e) => log::warn!("{e}; vehicle_kit_registry unavailable (kit deploy refused)"),
    }
    // Container types + content-class compatibility. Each of the registry's files
    // is chosen on its own by the shared rule, so one file this version cannot
    // read costs only that file, never the others' (BUG-163).
    {
        use crate::systems::inventory::containers::ContainerRegistry;
        let types = load_data_or_embedded(data_dir, "containers/types.csv", ContainerRegistry::types_from_csv);
        let classes =
            load_data_or_embedded(data_dir, "containers/content_classes.ron", ContainerRegistry::classes_from_ron);
        match (types, classes) {
            (Ok(types), Ok(classes)) => {
                // Contact rules (2026-09-26): materials, content traits and
                // the food profile map. A missing or broken file leaves the
                // registry without material rules, never without a registry.
                let reg = ContainerRegistry::from_parts(types, classes);
                let reg = match (
                    load_data_or_embedded(data_dir, "containers/materials.csv", ContainerRegistry::materials_from_csv),
                    load_data_or_embedded(data_dir, "containers/content_traits.ron", ContainerRegistry::traits_from_ron),
                    load_data_or_embedded(data_dir, "food/item_profiles.ron", ContainerRegistry::profiles_from_ron),
                ) {
                    (Ok(m), Ok(t), Ok(p)) => reg.with_contact(m, t, p),
                    (m, t, p) => {
                        let why: Vec<String> = [m.err(), t.err(), p.err()].into_iter().flatten().collect();
                        log::warn!("container contact rules did not load ({}); no material rules this session", why.join("; "));
                        reg
                    }
                };
                log::info!(
                    "Loaded ContainerRegistry: {} container types, {} content classes",
                    reg.types.len(),
                    reg.content_classes.len()
                );
                store.insert("container_registry", reg);
            }
            (types, classes) => {
                let why: Vec<String> = [types.err(), classes.err()].into_iter().flatten().collect();
                log::warn!("{}; container compatibility disabled", why.join("; "));
            }
        }
        // Fluids are litres (2026-09-26): tap-water items and carryable vessels.
        {
            use crate::systems::fluids::FluidTable;
            match load_data_or_embedded(data_dir, FluidTable::FILE, FluidTable::from_ron) {
                Ok(t) => {
                    log::info!("Loaded fluid table: {} tap items, {} vessels", t.tap.len(), t.fillables.len());
                    store.insert("fluid_table", t);
                }
                Err(e) => log::warn!("{e}; no tap water or vessels this session"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BUG-146 (2026-10-05): the trading post the game loads caps a better
    /// grade by the parts price. The economy's own checks build the registry
    /// with its parts prices themselves, so without this one the game could
    /// load trade goods without them, paying every grade in full (the money
    /// loop), and every check would still pass. Loaded here through
    /// `load_data_registries`, the game's own loader, from the shipped data.
    ///
    /// Seen red with the parts prices left out of the load:
    ///   the game's trading post has no parts price for the hammer
    #[test]
    fn the_loaded_trading_post_caps_a_grade_by_its_parts() {
        let mut store = DataStore::new();
        load_data_registries(&mut store, &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data"));
        let goods = store
            .get::<crate::systems::economy::TradeGoodsRegistry>("trade_goods_registry")
            .expect("the trade goods load");
        let levels = store
            .get::<crate::systems::crafting::quality::QualityLevels>("quality_levels")
            .expect("the grades load");
        let parts = goods.parts_price("hammer_0").expect("the game's trading post has no parts price for the hammer");
        let top = levels.levels.len() as u8;
        let master = goods.vendor_buy_price_graded("hammer_0", top, Some(levels)).expect("it buys a masterwork hammer");
        assert!(
            (master as f64) < parts,
            "a masterwork hammer fetches {master} at the loaded trading post, its parts cost {parts:.2} there"
        );
        assert!(goods.parts.len() >= 60, "only {} goods have a parts price", goods.parts.len());
    }

    // ── A data folder older than the game (BUG-163) ──

    use crate::embedded_data::STATUS_EFFECTS_CSV;
    use crate::systems::status_effects::StatusEffectRegistry;

    /// A data folder holding only `files` (path under data/, text), loaded the way the game
    /// loads its own, with every warning logged while it loaded. The folder is named `data`, as
    /// an installed game's is. The guard deletes it when the test ends.
    fn load_from(files: &[(&str, &str)]) -> (DataStore, Vec<String>, crate::test_temp::TempPath) {
        let root = crate::test_temp::dir("data_folder");
        let data = root.join("data");
        for (rel, text) in files {
            let path = data.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("make the folder");
            std::fs::write(&path, text).expect("write the file");
        }
        let mut store = DataStore::new();
        let ((), lines) = crate::test_log::capture(|| load_data_registries(&mut store, &data));
        (store, lines, root)
    }

    /// The header of today's status_effects.csv.
    fn effects_header() -> &'static str {
        STATUS_EFFECTS_CSV.lines().find(|l| l.starts_with("id,")).expect("the header")
    }

    /// One status_effects.csv row in the column order of today's header, from (column, value)
    /// pairs; a column not named gets a value any row may hold.
    fn effect_row(values: &[(&str, &str)]) -> String {
        effects_header()
            .split(',')
            .map(|col| {
                values.iter().find(|(c, _)| *c == col).map(|(_, v)| *v).unwrap_or(match col {
                    "stackable" => "false",
                    "max_stacks" => "1",
                    "stat_modifier" => "none:0:none",
                    "damage_type" => "none",
                    "tags" | "description" => "",
                    _ => "0",
                })
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The warnings that name status_effects.csv.
    fn about_effects(lines: &[String]) -> Vec<&String> {
        lines.iter().filter(|l| l.contains("status_effects.csv")).collect()
    }

    fn built_in_effects() -> StatusEffectRegistry {
        StatusEffectRegistry::from_csv(STATUS_EFFECTS_CSV.as_bytes()).expect("the built-in status_effects.csv")
    }

    /// The installed game's own case: a data folder written before 2026-10-05 still has the
    /// `dispel_type` column that BUG-162 removed, and the status effect rows now refuse a
    /// column they do not declare. That file must not empty the registry: the copy built into
    /// the game is used, and the log says so once, naming the file and the column.
    ///
    /// Seen red on main at 347c8f77b:
    ///   a status_effects.csv with an old column loaded 0 status effects, not the 70 built in
    #[test]
    fn a_status_effects_file_with_an_old_column_loads_the_built_in_copy() {
        let stale = crate::systems::status_effects::with_old_dispel_type_column(STATUS_EFFECTS_CSV);
        let (store, lines, _dir) = load_from(&[("status_effects.csv", &stale)]);
        let built_in = built_in_effects();
        let reg = store.get::<StatusEffectRegistry>("status_effect_registry").expect("a status effect registry");
        assert_eq!(
            reg.len(),
            built_in.len(),
            "a status_effects.csv with an old column loaded {} status effects, not the {} built in",
            reg.len(),
            built_in.len()
        );
        for id in built_in.effects.keys() {
            assert!(reg.get(id).is_some(), "{id} is missing");
        }
        assert_eq!(reg.duration("food_poisoning"), built_in.duration("food_poisoning"));
        let about = about_effects(&lines);
        assert_eq!(about.len(), 1, "one line about the file, not {}: {about:#?}", about.len());
        assert!(about[0].contains("dispel_type"), "the line says which column: {}", about[0]);
        assert!(!lines.iter().any(|l| l.contains("skipping row")), "no line per row: {lines:#?}");
    }

    /// A file with one row this version cannot read is not half loaded either: the copy built
    /// into the game is used, and the line names the row.
    ///
    /// Seen red on main at 347c8f77b:
    ///   a status_effects.csv with one bad row loaded 1 status effects, not the 70 built in
    #[test]
    fn a_status_effects_file_with_one_bad_row_loads_the_built_in_copy() {
        let file = format!(
            "{}\n{}\n{}\n",
            effects_header(),
            effect_row(&[("id", "modded_glow"), ("name", "Glow"), ("type", "buff"), ("duration_s", "60")]),
            effect_row(&[("id", "modded_ache"), ("name", "Ache"), ("type", "debuff"), ("duration_s", "two days")]),
        );
        let (store, lines, _dir) = load_from(&[("status_effects.csv", &file)]);
        let built_in = built_in_effects();
        let reg = store.get::<StatusEffectRegistry>("status_effect_registry").expect("a status effect registry");
        assert_eq!(
            reg.len(),
            built_in.len(),
            "a status_effects.csv with one bad row loaded {} status effects, not the {} built in",
            reg.len(),
            built_in.len()
        );
        assert!(reg.get("modded_glow").is_none(), "none of a refused file is used, not even its good rows");
        let about = about_effects(&lines);
        assert_eq!(about.len(), 1, "one line about the file, not {}: {about:#?}", about.len());
        assert!(about[0].contains("modded_ache"), "the line names the row it could not read: {}", about[0]);
        assert!(!lines.iter().any(|l| l.contains("skipping row")), "no line per row: {lines:#?}");
    }

    /// Modding keeps working: a file this version reads in full wins over the copy built into
    /// the game, whatever it changes, adds or leaves out, and nothing is said about it.
    ///
    /// Passes on main, where mods already won. Seen red with the loader made to always use the
    /// built-in copy:
    ///   a current status_effects.csv on disk was not used: 70 status effects loaded, not its 2
    #[test]
    fn a_current_status_effects_file_on_disk_wins_over_the_built_in_copy() {
        let file = format!(
            "{}\n{}\n{}\n",
            effects_header(),
            effect_row(&[("id", "well_fed"), ("name", "Well Fed"), ("type", "buff"), ("duration_s", "999")]),
            effect_row(&[("id", "modded_glow"), ("name", "Glow"), ("type", "buff"), ("duration_s", "60")]),
        );
        let (store, lines, _dir) = load_from(&[("status_effects.csv", &file)]);
        let reg = store.get::<StatusEffectRegistry>("status_effect_registry").expect("a status effect registry");
        assert_eq!(
            reg.len(),
            2,
            "a current status_effects.csv on disk was not used: {} status effects loaded, not its 2",
            reg.len()
        );
        assert_eq!(reg.duration("well_fed"), 999.0, "the file's own Well Fed");
        assert!(reg.get("modded_glow").is_some(), "the file's own new effect");
        assert!(about_effects(&lines).is_empty(), "nothing to say about a file that loads: {lines:#?}");
    }

    /// Every registry here loads by the one rule (BUG-163): none reads its file another way,
    /// which would bring back a stale file emptying a registry.
    ///
    /// Seen red with a registry reading the built-in table itself:
    ///   registries.rs loads a file with get_embedded, not load_data_or_embedded
    #[test]
    fn every_registry_here_loads_by_the_shared_rule() {
        let src = include_str!("registries.rs");
        let loader = &src[..src.find("#[cfg(test)]").expect("the tests module")];
        for other in ["read_data_or_embedded", "fs::read", "get_embedded", "from_ron_dir"] {
            assert!(!loader.contains(other), "registries.rs loads a file with {other}, not load_data_or_embedded");
        }
    }

    /// The data folder as shipped loads every registry from its own files with nothing to say:
    /// no row refused, no file that does not parse, no registry missing. A data edit that this
    /// version cannot read fails here, before a rig refuses a run for serving a built-in copy.
    ///
    /// Passes on main too. Seen red with a row the code cannot read appended to abilities.csv:
    ///   loading the shipped data folder warned: ...
    ///   WARN: CSV parse warning (skipping row): line 158 (junk_ability), column mana_cost: invalid float literal
    #[test]
    fn the_shipped_data_folder_loads_every_registry_without_a_warning() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let mut store = DataStore::new();
        let ((), lines) = crate::test_log::capture(|| load_data_registries(&mut store, &data));
        assert!(lines.is_empty(), "loading the shipped data folder warned:\n{}", lines.join("\n"));
    }
}
