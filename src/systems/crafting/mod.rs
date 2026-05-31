//! Crafting system — recipe validation, crafting queue, and output production.
//!
//! Recipes loaded from `data/recipes.csv`.
//! Inputs/outputs use pipe-separated `item_id:quantity` format.

pub mod workstations;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

/// A crafting recipe parsed from CSV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    /// Unique recipe ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Required inputs: (item_id, quantity).
    pub inputs: Vec<(String, u32)>,
    /// Produced outputs: (item_id, quantity).
    pub outputs: Vec<(String, u32)>,
    /// Seconds to complete crafting.
    pub craft_time: f32,
    /// Required workstation type (None = hand-craftable).
    pub required_station: Option<String>,
    /// Skill whose XP this recipe grants on completion (None = no skill trained).
    /// Canonical skill id from data/skills/skills.csv (recipe_skill_lint enforces).
    pub skill_required: Option<String>,
    /// Minimum level of `skill_required` to craft (0 = none). Also scales the XP
    /// reward so harder recipes grant more.
    pub skill_level: u32,
}

impl Recipe {
    /// Parse pipe-separated ingredient string like "iron_ore:2|coal:1".
    pub fn parse_ingredients(s: &str) -> Vec<(String, u32)> {
        if s.is_empty() {
            return Vec::new();
        }
        s.split('|')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, ':');
                let id = parts.next()?.trim().to_string();
                let qty = parts
                    .next()
                    .and_then(|q| q.trim().parse::<u32>().ok())
                    .unwrap_or(1);
                if id.is_empty() {
                    None
                } else {
                    Some((id, qty))
                }
            })
            .collect()
    }
}

/// Registry of all recipes, keyed by recipe ID.
#[derive(Debug, Clone, Default)]
pub struct RecipeRegistry {
    pub recipes: HashMap<String, Recipe>,
}

impl RecipeRegistry {
    /// Find all recipes that can produce a given item.
    pub fn recipes_producing(&self, item_id: &str) -> Vec<&Recipe> {
        self.recipes
            .values()
            .filter(|r| r.outputs.iter().any(|(id, _)| id == item_id))
            .collect()
    }

    /// Find all recipes usable at a given workstation.
    pub fn recipes_for_station(&self, station: &str) -> Vec<&Recipe> {
        self.recipes
            .values()
            .filter(|r| r.required_station.as_deref() == Some(station))
            .collect()
    }

    /// Find all hand-craftable recipes (no station required).
    pub fn hand_craftable(&self) -> Vec<&Recipe> {
        self.recipes
            .values()
            .filter(|r| r.required_station.is_none())
            .collect()
    }

    /// Build the recipe registry from raw `recipes.csv` bytes.
    ///
    /// Uses the shared CSV loader (skips `#` comments, header-mapped, row-resilient).
    /// Inputs/outputs use the pipe-separated `item_id:qty` format handled by
    /// [`Recipe::parse_ingredients`]; an empty `station_required` becomes
    /// `None` (hand-craftable). This is the constructor the runtime calls to
    /// populate `DataStore["recipe_registry"]` — before v0.323 the CSV was loaded
    /// then discarded, so CraftingSystem found no recipes and could craft nothing.
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<RecipeRow> = crate::assets::loader::parse_csv(data)?;
        let mut recipes = HashMap::new();
        for row in rows {
            let station = row.station_required.trim();
            recipes.insert(
                row.id.clone(),
                Recipe {
                    id: row.id,
                    name: row.name,
                    inputs: Recipe::parse_ingredients(&row.inputs),
                    outputs: Recipe::parse_ingredients(&row.outputs),
                    craft_time: row.craft_time_sec,
                    required_station: if station.is_empty() {
                        None
                    } else {
                        Some(station.to_string())
                    },
                    skill_required: {
                        let s = row.skill_required.trim();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s.to_string())
                        }
                    },
                    skill_level: row.skill_level,
                },
            );
        }
        Ok(Self { recipes })
    }
}

/// One row of `recipes.csv` — the columns `RecipeRegistry` consumes (the
/// category/description columns are still ignored; skill_required + skill_level
/// are now parsed to drive skill XP + tech-unlock gating).
#[derive(Debug, Deserialize)]
struct RecipeRow {
    id: String,
    name: String,
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
}

#[cfg(test)]
mod recipe_registry_csv_tests {
    use super::*;

    #[test]
    fn from_csv_parses_recipes_ingredients_and_station() {
        let csv = b"id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required\n\
                    smelt_iron,Smelt Iron,smelting,iron_ore_0:2|coal_0:1,iron_ingot_0:1,10,smelter_0,smithing\n\
                    carve_stick,Carve Stick,hand,,stick_0:1,1,,\n";
        let reg = RecipeRegistry::from_csv(csv).expect("parse");
        assert_eq!(reg.recipes.len(), 2);

        let smelt = reg.recipes.get("smelt_iron").expect("smelt present");
        assert_eq!(
            smelt.inputs,
            vec![("iron_ore_0".to_string(), 2), ("coal_0".to_string(), 1)]
        );
        assert_eq!(smelt.outputs, vec![("iron_ingot_0".to_string(), 1)]);
        assert!((smelt.craft_time - 10.0).abs() < 1e-6);
        assert_eq!(smelt.required_station.as_deref(), Some("smelter_0"));

        let carve = reg.recipes.get("carve_stick").expect("carve present");
        assert!(carve.inputs.is_empty());
        assert_eq!(carve.required_station, None, "empty station -> hand-craftable");
    }
}

#[cfg(test)]
mod crafting_end_to_end_tests {
    use super::*;
    use crate::ecs::systems::System;
    use crate::hot_reload::data_store::DataStore;
    use crate::systems::inventory::{Inventory, ItemRegistry};

    #[test]
    fn real_recipes_load_and_a_timed_craft_produces_output() {
        // Loads the SHIPPED data files (compile-time embedded -> hermetic + CI-safe)
        // and drives the full crafting loop end to end: request -> consume inputs
        // -> timed completion -> output lands in inventory. This is the exact path
        // that silently no-op'd before the registries were wired into the runtime
        // DataStore (v0.323) — recipe_registry was always None, so nothing crafted.
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv parses");
        let recipes = RecipeRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/recipes.csv"
        )))
        .expect("recipes.csv parses");
        assert!(!items.items.is_empty(), "items registry empty");
        assert!(recipes.recipes.len() > 50, "too few recipes parsed from real data");

        // smelt_iron: iron_ore_0:2 + coal_0:1 -> iron_ingot_0:1 (timed 10s).
        let recipe = recipes.recipes.get("smelt_iron").expect("smelt_iron present");
        assert_eq!(
            recipe.inputs,
            vec![("iron_ore_0".to_string(), 2), ("coal_0".to_string(), 1)]
        );
        assert_eq!(recipe.outputs, vec![("iron_ingot_0".to_string(), 1)]);
        assert!(recipe.craft_time > 0.0, "smelt_iron should be a timed craft");

        let mut data = DataStore::new();
        data.insert("item_registry", items);
        data.insert("recipe_registry", recipes);

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 99);
        inv.add_item("coal_0", 1, 99);
        let entity = world.spawn((inv,));

        let mut sys = CraftingSystem::new();
        sys.request_craft("smelt_iron".to_string(), entity);

        // Tick past the 10s craft_time (dt=1s): the first tick consumes inputs and
        // queues the timed craft; later ticks drain the timer to completion.
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }

        let inv = world.get::<&Inventory>(entity).expect("entity has Inventory");
        assert!(
            inv.has_item("iron_ingot_0", 1),
            "timed craft did not produce iron_ingot_0 — crafting loop is broken"
        );
        assert_eq!(inv.count_item("iron_ore_0"), 0, "iron_ore inputs not consumed");
        assert_eq!(inv.count_item("coal_0"), 0, "coal input not consumed");
    }
}

#[cfg(test)]
mod crafting_bridge_tests {
    use super::*;
    use crate::ecs::components::Controllable;
    use crate::ecs::systems::System;
    use crate::hot_reload::data_store::DataStore;
    use crate::systems::inventory::{Inventory, ItemRegistry};

    /// Full GUI->ECS loop: the dev "stock all materials" flag provisions the player
    /// with every recipe input, then a `craft_request` (what the Craft button writes
    /// via the main-loop bridge) drives a real consume/produce on the player's actual
    /// inventory.
    #[test]
    fn dev_stock_then_gui_craft_request_runs_the_full_loop() {
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        let recipes = RecipeRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/recipes.csv"
        )))
        .expect("recipes.csv");

        let mut data = DataStore::new();
        data.insert("item_registry", items);
        data.insert("recipe_registry", recipes);
        data.insert("dev_stock_materials", std::sync::Mutex::new(true));
        data.insert("craft_request", std::sync::Mutex::new(Option::<String>::None));

        // The CraftingSystem looks for a Controllable + Inventory entity (the player).
        let mut world = hecs::World::new();
        let entity = world.spawn((Inventory::new(64), Controllable));
        let mut sys = CraftingSystem::new();

        // Tick 1: the one-shot dev flag stocks the player with raw materials.
        sys.tick(&mut world, 1.0, &data);
        let ore_after_stock = {
            let inv = world.get::<&Inventory>(entity).expect("inv");
            assert!(inv.has_item("iron_ore_0", 2), "dev-stock provisioned iron ore (a raw)");
            // coal_0 is an INTERMEDIATE (it is some recipe's output), so its presence
            // proves dev-stock provisions every input, not just the raws — without it
            // smelt_iron (iron_ore_0 + coal_0) would not be craftable in one click.
            assert!(inv.has_item("coal_0", 1), "dev-stock provisioned coal (an intermediate)");
            inv.count_item("iron_ore_0")
        };
        // Dev flag is one-shot (consumed).
        assert!(
            !*data
                .get::<std::sync::Mutex<bool>>("dev_stock_materials")
                .unwrap()
                .lock()
                .unwrap(),
            "dev_stock flag should reset after firing"
        );

        // Request a craft via the GUI channel (what the Craft button writes).
        *data
            .get::<std::sync::Mutex<Option<String>>>("craft_request")
            .unwrap()
            .lock()
            .unwrap() = Some("smelt_iron".to_string());

        // Drive past the 10s craft timer.
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }

        let inv = world.get::<&Inventory>(entity).expect("inv");
        assert!(
            inv.has_item("iron_ingot_0", 1),
            "GUI craft request did not produce iron_ingot_0"
        );
        assert_eq!(
            inv.count_item("iron_ore_0"),
            ore_after_stock - 2,
            "smelt_iron should consume exactly 2 iron_ore from the player inventory"
        );
    }
}

/// A craft in progress, tracked per-entity.
#[derive(Debug, Clone)]
pub struct ActiveCraft {
    /// Which recipe is being crafted.
    pub recipe_id: String,
    /// Seconds remaining until completion.
    pub time_remaining: f32,
    /// Entity performing the craft.
    pub crafter: hecs::Entity,
}

/// Request to start crafting — queued for the system to validate and begin.
#[derive(Debug, Clone)]
pub struct CraftRequest {
    /// Recipe to craft.
    pub recipe_id: String,
    /// Entity with the inventory to consume from / produce into.
    pub crafter: hecs::Entity,
}

/// Manages recipe validation, input consumption, and output production.
pub struct CraftingSystem {
    /// Pending craft requests to validate next tick.
    pending_requests: Vec<CraftRequest>,
    /// Active crafts being timed.
    active_crafts: Vec<ActiveCraft>,
}

impl CraftingSystem {
    pub fn new() -> Self {
        Self {
            pending_requests: Vec::new(),
            active_crafts: Vec::new(),
        }
    }

    /// Queue a craft request for validation on the next tick.
    pub fn request_craft(&mut self, recipe_id: String, crafter: hecs::Entity) {
        self.pending_requests.push(CraftRequest { recipe_id, crafter });
    }

    /// Check if an entity has all required inputs for a recipe.
    fn can_craft(inventory: &Inventory, recipe: &Recipe) -> bool {
        recipe
            .inputs
            .iter()
            .all(|(item_id, qty)| inventory.has_item(item_id, *qty))
    }

    /// Consume recipe inputs from inventory.
    fn consume_inputs(inventory: &mut Inventory, recipe: &Recipe) {
        for (item_id, qty) in &recipe.inputs {
            inventory.remove_item(item_id, *qty);
        }
    }

    /// Produce recipe outputs into inventory.
    fn produce_outputs(inventory: &mut Inventory, recipe: &Recipe, item_registry: Option<&crate::systems::inventory::ItemRegistry>) {
        for (item_id, qty) in &recipe.outputs {
            let max_stack = item_registry
                .map(|r| r.max_stack_for(item_id))
                .unwrap_or(99);
            let overflow = inventory.add_item(item_id, *qty, max_stack);
            if overflow > 0 {
                log::warn!(
                    "Crafting output overflow: {} of {} lost (inventory full)",
                    overflow,
                    item_id
                );
            }
        }
    }

    /// Award skill XP for a completed craft by pushing onto the shared
    /// "xp_grants" channel (drained by SkillSystem, which ticks last). Recipes
    /// with no `skill_required` train nothing. The reward scales with the
    /// recipe's `skill_level` so harder recipes grant more (a level-5 recipe =
    /// 35 XP, a level-1 = 15). No-ops cleanly if the channel/skill is absent.
    /// Post-craft hooks fired when a craft completes: award skill XP (#8a) and
    /// emit a quest-progress event (#8c) for any Craft objective tracking this
    /// recipe. Both no-op cleanly if their channel/skill is absent.
    fn on_craft_complete(data: &DataStore, recipe: &Recipe) {
        if let Some(skill) = &recipe.skill_required {
            crate::systems::skills::award_skill_xp(data, skill, 10 + recipe.skill_level * 5);
        }
        crate::systems::quests::push_quest_event(data, format!("craft_{}", recipe.id));
    }

    /// Tech-unlock gate: does the crafter meet the recipe's `skill_level`?
    ///
    /// Recipes at **skill_level 0 or 1 are the free "starter tier"** — always
    /// craftable. Gating only begins at level 2. This is deliberate: a fresh
    /// player is level 0 in every skill and the ONLY way to earn a skill's XP is
    /// crafting that skill's recipes, so if level-1 recipes were gated, every
    /// skill with no level-0 recipe would be un-bootstrappable (a deadlock).
    /// Level-1 recipes bootstrap the skill; level 2+ recipes are the real unlocks.
    ///
    /// An entity WITHOUT a `PlayerSkills` component (e.g. an NPC/bot crafter) is
    /// never blocked — the gate applies to skilled actors (the player). This is
    /// the AUTHORITATIVE gate (enforced system-side, not just greyed in the GUI),
    /// so a bot or non-GUI path can't craft above its level.
    fn meets_skill_requirement(
        world: &hecs::World,
        crafter: hecs::Entity,
        recipe: &Recipe,
    ) -> bool {
        let skill = match &recipe.skill_required {
            Some(s) if recipe.skill_level > 1 => s,
            _ => return true,
        };
        match world.get::<&crate::systems::skills::PlayerSkills>(crafter) {
            Ok(skills) => skills.level(skill) >= recipe.skill_level,
            Err(_) => true,
        }
    }
}

impl System for CraftingSystem {
    fn name(&self) -> &str {
        "CraftingSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let recipe_registry = data.get::<RecipeRegistry>("recipe_registry");
        let item_registry = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");

        // ── GUI / dev command channels (written by the main loop from GuiState). ──
        // Drain the command flags first — these read `data`, never `world`, so they
        // hold no ECS borrow.
        let do_stock = data
            .get::<std::sync::Mutex<bool>>("dev_stock_materials")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        let requested = data
            .get::<std::sync::Mutex<Option<String>>>("craft_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));

        // Dev/creative provisioning: precompute one full stack of EVERY distinct recipe
        // input — raw materials AND intermediates alike — so that every recipe is
        // craftable in a single click, which is the whole point of "develop as if
        // everything is unlocked 100%". Stocking only the raws (inputs that are never
        // any recipe's output) would leave every recipe needing an intermediate
        // uncraftable until you first built that intermediate — e.g. smelt_iron needs
        // coal_0, which is itself a recipe output. Built here (reading
        // `recipe_registry`) BEFORE the &mut World borrow below.
        let inputs: Vec<String> = if do_stock {
            match recipe_registry {
                Some(recipes) => {
                    let mut v: Vec<String> = Vec::new();
                    for r in recipes.recipes.values() {
                        for (id, _) in &r.inputs {
                            if !v.iter().any(|x| x == id) {
                                v.push(id.clone());
                            }
                        }
                    }
                    v.sort();
                    v
                }
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };

        // Single EXCLUSIVE &mut World pass: locate the player (the controllable entity
        // that owns an inventory) and dev-stock it in place. `query_mut` takes the
        // world mutably, so unlike `world.query(...).iter()` + `world.get::<&mut _>`
        // (whose shared borrow of the Inventory column stayed live, making the &mut get
        // fail) there is no runtime-borrow conflict with the `world.get` calls in the
        // pending-craft loop further down.
        let mut player: Option<hecs::Entity> = None;
        for (entity, (inv, _ctrl)) in
            world.query_mut::<(&mut Inventory, &crate::ecs::components::Controllable)>()
        {
            player = Some(entity);
            if do_stock {
                // Grow the inventory so the full material set fits in one pass (a
                // default 36-slot inventory can't hold all distinct inputs).
                let occupied = inv.slots.iter().filter(|s| s.is_some()).count();
                inv.ensure_slots(occupied + inputs.len());
                for id in &inputs {
                    let max_stack = item_registry.map(|r| r.max_stack_for(id)).unwrap_or(99);
                    inv.add_item(id, max_stack, max_stack);
                }
                log::info!("Dev: stocked {} material types for the player", inputs.len());
            }
            break; // only the first controllable player
        }

        // Craft request from the GUI: queue it for the player entity (the
        // pending-request loop below processes it this same tick).
        if let (Some(recipe_id), Some(entity)) = (requested, player) {
            self.request_craft(recipe_id, entity);
        }

        // Process pending craft requests
        if let Some(recipes) = recipe_registry {
            let requests: Vec<_> = self.pending_requests.drain(..).collect();
            for request in requests {
                let recipe = match recipes.recipes.get(&request.recipe_id) {
                    Some(r) => r.clone(),
                    None => {
                        log::warn!("Unknown recipe: {}", request.recipe_id);
                        continue;
                    }
                };

                // Tech-unlock gate: skip if the crafter's skill is below the
                // recipe's required level (the GUI greys these out; this is the
                // authoritative enforcement for any path, incl. bots/NPCs).
                if !Self::meets_skill_requirement(world, request.crafter, &recipe) {
                    log::debug!(
                        "Skill too low to craft {} (needs {:?} Lv {})",
                        recipe.id,
                        recipe.skill_required,
                        recipe.skill_level
                    );
                    continue;
                }

                // Validate inventory has required inputs
                let can_craft = match world.get::<&Inventory>(request.crafter) {
                    Ok(inv) => Self::can_craft(&inv, &recipe),
                    Err(_) => {
                        log::warn!("Craft request on entity without Inventory");
                        continue;
                    }
                };

                if !can_craft {
                    log::debug!("Insufficient materials for recipe: {}", recipe.id);
                    continue;
                }

                // Consume inputs immediately
                if let Ok(mut inv) = world.get::<&mut Inventory>(request.crafter) {
                    Self::consume_inputs(&mut inv, &recipe);
                }

                // If instant craft (time <= 0), produce outputs immediately
                if recipe.craft_time <= 0.0 {
                    if let Ok(mut inv) = world.get::<&mut Inventory>(request.crafter) {
                        Self::produce_outputs(&mut inv, &recipe, item_registry);
                    }
                    Self::on_craft_complete(data, &recipe);
                    log::debug!("Instant craft complete: {}", recipe.id);
                } else {
                    // Queue as active craft with timer
                    self.active_crafts.push(ActiveCraft {
                        recipe_id: recipe.id.clone(),
                        time_remaining: recipe.craft_time,
                        crafter: request.crafter,
                    });
                    log::debug!(
                        "Started crafting {} ({:.1}s)",
                        recipe.id,
                        recipe.craft_time
                    );
                }
            }
        } else {
            // No recipes loaded yet — keep requests for next tick
            if !self.pending_requests.is_empty() {
                log::debug!("Recipe registry not loaded, deferring {} craft requests", self.pending_requests.len());
            }
        }

        // Advance active crafts
        let mut completed = Vec::new();
        for (i, craft) in self.active_crafts.iter_mut().enumerate() {
            craft.time_remaining -= dt;
            if craft.time_remaining <= 0.0 {
                completed.push(i);
            }
        }

        // Process completions (reverse order to preserve indices)
        for i in completed.into_iter().rev() {
            let craft = self.active_crafts.remove(i);

            if let Some(recipes) = recipe_registry {
                if let Some(recipe) = recipes.recipes.get(&craft.recipe_id) {
                    if let Ok(mut inv) = world.get::<&mut Inventory>(craft.crafter) {
                        Self::produce_outputs(&mut inv, recipe, item_registry);
                        Self::on_craft_complete(data, recipe);
                        log::debug!("Craft complete: {}", recipe.id);
                    } else {
                        log::warn!(
                            "Craft complete but entity lost Inventory: {}",
                            craft.recipe_id
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod refining_chain_tests {
    use super::*;
    use crate::systems::inventory::ItemRegistry;

    /// The ores mined by drones (#5) must be refineable: smelt recipes exist for
    /// nickel + platinum, and a 2-tier alloy (stainless = iron + nickel ingots)
    /// proves the ore → ingot → alloy chain — with every input/output a real item.
    #[test]
    fn nickel_platinum_refining_chain_is_wired() {
        let recipes = RecipeRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/recipes.csv"
        )))
        .expect("recipes.csv");
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");

        let valid = |id: &str| items.items.contains_key(id);

        for (recipe_id, want_out) in [
            ("smelt_nickel", "nickel_ingot_0"),
            ("smelt_platinum", "platinum_ingot_0"),
            ("smelt_stainless", "stainless_steel_ingot_0"),
        ] {
            let r = recipes
                .recipes
                .get(recipe_id)
                .unwrap_or_else(|| panic!("missing recipe {recipe_id}"));
            for (id, _) in &r.inputs {
                assert!(valid(id), "{recipe_id} input {id} is not a real item");
            }
            for (id, _) in &r.outputs {
                assert!(valid(id), "{recipe_id} output {id} is not a real item");
            }
            assert!(
                r.outputs.iter().any(|(id, _)| id == want_out),
                "{recipe_id} should produce {want_out}"
            );
        }

        // Multi-tier depth: stainless consumes nickel_ingot_0, which is itself
        // smelt_nickel's output — ore → ingot → alloy, not a flat one-step tree.
        let stainless = recipes.recipes.get("smelt_stainless").unwrap();
        assert!(
            stainless.inputs.iter().any(|(id, _)| id == "nickel_ingot_0"),
            "stainless steel should consume nickel ingots (a refined intermediate)"
        );
    }
}

#[cfg(test)]
mod skill_xp_tests {
    use super::*;
    use crate::ecs::components::Controllable;
    use crate::systems::inventory::{Inventory, ItemRegistry};
    use crate::systems::skills::SkillXPEvent;

    /// End-to-end: clicking Craft on a recipe with a `skill_required` pushes a
    /// scaled XP grant onto the shared channel when the craft completes (proves
    /// the skill_required column is parsed + the completion hook fires).
    #[test]
    fn completing_a_craft_awards_skill_xp() {
        // A tiny INSTANT recipe (craft_time 0) that trains metalworking at level 2.
        let recipe_csv = b"id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required,skill_level,description\n\
            test_smelt,Test Smelt,smelting,iron_ore_0:1,iron_ingot_0:1,0,,metalworking,2,test\n";
        let recipes = RecipeRegistry::from_csv(recipe_csv).expect("recipes");
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");

        let mut data = DataStore::new();
        data.insert("recipe_registry", recipes);
        data.insert("item_registry", items);
        data.insert("dev_stock_materials", std::sync::Mutex::new(false));
        data.insert(
            "craft_request",
            std::sync::Mutex::new(Option::<String>::None),
        );
        data.insert("xp_grants", std::sync::Mutex::new(Vec::<SkillXPEvent>::new()));

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 1, 99);
        world.spawn((inv, Controllable));

        // The GUI sets craft_request when the player clicks Craft.
        *data
            .get::<std::sync::Mutex<Option<String>>>("craft_request")
            .unwrap()
            .lock()
            .unwrap() = Some("test_smelt".to_string());

        let mut sys = CraftingSystem::new();
        sys.tick(&mut world, 0.016, &data); // instant craft completes this tick

        let grants = data
            .get::<std::sync::Mutex<Vec<SkillXPEvent>>>("xp_grants")
            .unwrap()
            .lock()
            .unwrap();
        assert_eq!(grants.len(), 1, "one completed craft → one XP grant");
        assert_eq!(grants[0].skill_id, "metalworking");
        assert_eq!(grants[0].amount, 20, "10 + skill_level(2)*5 = 20");
    }

    /// #8b tech-unlock: a recipe with a skill_level requirement is BLOCKED when the
    /// crafter is under-level (no output, inputs untouched) and ALLOWED once they
    /// reach the level — enforced authoritatively in CraftingSystem, not just the GUI.
    #[test]
    fn skill_gate_blocks_under_level_then_allows() {
        use crate::systems::skills::{PlayerSkills, SkillProgress, SkillXPEvent};

        // Instant recipe requiring metalworking Lv 3.
        let recipe_csv = b"id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required,skill_level,description\n\
            test_forge,Test Forge,smelting,iron_ore_0:1,iron_ingot_0:1,0,,metalworking,3,test\n";
        let recipes = RecipeRegistry::from_csv(recipe_csv).expect("recipes");
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");

        let mut data = DataStore::new();
        data.insert("recipe_registry", recipes);
        data.insert("item_registry", items);
        data.insert("dev_stock_materials", std::sync::Mutex::new(false));
        data.insert(
            "craft_request",
            std::sync::Mutex::new(Option::<String>::None),
        );
        data.insert("xp_grants", std::sync::Mutex::new(Vec::<SkillXPEvent>::new()));

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 99);
        let mut skills = PlayerSkills::new();
        skills
            .skills
            .insert("metalworking".to_string(), SkillProgress { level: 1, xp: 0 });
        let player = world.spawn((inv, Controllable, skills));

        let mut sys = CraftingSystem::new();
        let request = |data: &DataStore| {
            *data
                .get::<std::sync::Mutex<Option<String>>>("craft_request")
                .unwrap()
                .lock()
                .unwrap() = Some("test_forge".to_string());
        };

        // Lv 1 < required Lv 3 → blocked (no ingot; ore not consumed).
        request(&data);
        sys.tick(&mut world, 0.016, &data);
        {
            let inv = world.get::<&Inventory>(player).unwrap();
            assert_eq!(inv.count_item("iron_ingot_0"), 0, "under-level craft is blocked");
            assert_eq!(inv.count_item("iron_ore_0"), 2, "blocked craft consumes nothing");
        }

        // Raise metalworking to Lv 3 → the same craft now succeeds.
        world
            .get::<&mut PlayerSkills>(player)
            .unwrap()
            .skills
            .insert("metalworking".to_string(), SkillProgress { level: 3, xp: 0 });
        request(&data);
        sys.tick(&mut world, 0.016, &data);
        {
            let inv = world.get::<&Inventory>(player).unwrap();
            assert_eq!(inv.count_item("iron_ingot_0"), 1, "at-level craft succeeds");
            assert_eq!(inv.count_item("iron_ore_0"), 1, "the craft consumed one ore");
        }
    }
}
