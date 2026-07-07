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
    /// True for an AutoRefine MACHINE batch (v0.663): outputs always land in the
    /// HOME (player) inventory, keyed by this flag rather than a lookup on the
    /// crafter entity -- the machine may legitimately be despawned mid-batch
    /// (load_world despawns + respawns every HomeMachine), and the batch's
    /// already-consumed inputs must still come back as outputs.
    pub auto: bool,
    /// The crafter's world pose captured when the batch STARTED (economy Phase 2
    /// Stage 2, v0.679): the factory pad where vehicle-class outputs roll out.
    /// Captured at start for the same reason `auto` exists -- the machine may be
    /// despawned mid-batch, and the vehicle must still appear at its pad. None
    /// when the crafter had no Transform (pre-v0.679 menu-only machines).
    pub pad: Option<(glam::Vec3, glam::Quat)>,
}

/// How far in front of the crafter (machine or player) a finished vehicle rolls
/// out, and the spacing between pad lanes.
const PAD_OFFSET_M: f32 = 3.0;
const PAD_SPACING_M: f32 = 3.5;
/// How many pad lanes a factory has before the assembly line PAUSES (review fix,
/// v0.679): without an occupancy model every completion spawned at the identical
/// point, piling coincident permanent vehicles forever. When every lane holds a
/// vehicle, the auto arm refuses to START the next batch (inputs unconsumed) --
/// the same never-grind guarantee outputs_fit gives item recipes.
const MAX_PAD_LANES: u32 = 12;

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

    /// Would the recipe's outputs land WITHOUT overflow-loss? Conservative slot
    /// math (existing same-item stack headroom first, then free slots). Used by
    /// the AutoRefine arm so automation never grinds inputs into discarded
    /// outputs when the home inventory is full (adversarial review 2026-07-01);
    /// manual crafting keeps its existing lossy-with-warning behavior since a
    /// human sees the warning and made the click.
    fn outputs_fit(
        inventory: &Inventory,
        recipe: &Recipe,
        item_registry: Option<&crate::systems::inventory::ItemRegistry>,
        vehicle_kits: Option<&crate::systems::vehicles::VehicleKitRegistry>,
    ) -> bool {
        // Volume headroom (Stage A slice 2, v0.727): a batch only starts when
        // its outputs also fit by VOLUME, else an unattended auto-machine
        // becomes a grinder that consumes inputs and volume-overflows every
        // output. Inputs are consumed at start (freeing volume), but checking
        // outputs against CURRENT volume is the conservative, simple bound.
        let out_volume: f32 = recipe
            .outputs
            .iter()
            .filter(|(id, _)| !vehicle_kits.is_some_and(|k| k.get_vehicle(id).is_some()))
            .map(|(id, qty)| {
                item_registry.map(|r| r.volume_for(id)).unwrap_or(0.0) * *qty as f32
            })
            .sum();
        if out_volume > 0.0
            && inventory.volume_current_l + out_volume > inventory.volume_capacity_l
        {
            return false;
        }
        let mut free_slots = inventory.slots.iter().filter(|s| s.is_none()).count() as u32;
        for (item_id, qty) in &recipe.outputs {
            // Vehicle-class outputs (economy Phase 2 Stage 2) roll out onto the
            // factory pad as world entities -- they never need an inventory slot,
            // so a full backpack must not stall the assembly line.
            if vehicle_kits.is_some_and(|k| k.get_vehicle(item_id).is_some()) {
                continue;
            }
            let max_stack = item_registry.map(|r| r.max_stack_for(item_id)).unwrap_or(99);
            let headroom: u32 = inventory
                .slots
                .iter()
                .flatten()
                .filter(|s| s.item_id == *item_id)
                .map(|s| s.max_stack.min(max_stack).saturating_sub(s.quantity))
                .sum();
            if headroom >= *qty {
                continue;
            }
            let remaining = *qty - headroom;
            let per_slot = max_stack.max(1);
            let slots_needed = remaining.div_ceil(per_slot);
            if slots_needed > free_slots {
                return false;
            }
            free_slots -= slots_needed;
        }
        true
    }

    /// Produce recipe outputs into inventory.
    fn produce_outputs(inventory: &mut Inventory, recipe: &Recipe, item_registry: Option<&crate::systems::inventory::ItemRegistry>) {
        for (item_id, qty) in &recipe.outputs {
            let max_stack = item_registry
                .map(|r| r.max_stack_for(item_id))
                .unwrap_or(99);
            // Volume-gated (Stage A slice 2) — outputs_fit pre-checks volume
            // at batch start, so overflow here means the pack filled mid-batch.
            let unit_vol = item_registry.map(|r| r.volume_for(item_id)).unwrap_or(0.0);
            let overflow = inventory.add_item_volume_gated(item_id, *qty, max_stack, unit_vol);
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
        // Vehicle-class outputs (economy Phase 2 Stage 2): any output item the kit
        // registry resolves as an ASSEMBLED vehicle rolls out onto the factory pad
        // as a real world entity instead of landing in an inventory slot.
        let vehicle_kits =
            data.get::<crate::systems::vehicles::VehicleKitRegistry>("vehicle_kit_registry");
        // Creative mode (default ON in early dev): crafting skips the input
        // requirement + consumption. Absent flag (tests) = survival = consume.
        let creative = data
            .get::<std::sync::Mutex<bool>>("creative_mode")
            .and_then(|m| m.lock().ok().map(|g| *g))
            .unwrap_or(false);

        // World rewind (review fix, v0.679): applying a save mid-batch rewinds
        // the inventory (materials restored) and despawns every Vehicle -- an
        // in-flight batch completing AFTER that would deliver on top of the
        // restored materials (duplication: the routine character-pick flow made
        // one batch of inputs into inputs + a rover). When the main loop applies
        // a save it raises this flag; dropping in-flight batches makes a rewind
        // behave exactly like an app restart (which never preserved them).
        let rewound = data
            .get::<std::sync::Mutex<bool>>("abort_active_crafts")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        if rewound && !self.active_crafts.is_empty() {
            log::info!(
                "World rewound: dropping {} in-flight craft batch(es) (save is authoritative)",
                self.active_crafts.len()
            );
            self.active_crafts.clear();
        }

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

        // ── AutoRefine machines (economy automation Phase 1, v0.663) ──
        // Each home machine carrying an AutoRefine marker (smelter -> smelt_iron,
        // workbench -> craft_hammer; data-driven from home.ron's `auto_recipe`)
        // continuously runs its recipe against the HOME (player) inventory: when
        // the inputs are in stock and the machine has no craft already in flight,
        // consume the inputs now and queue a timed craft whose `crafter` is the
        // MACHINE entity (so multiple machines run concurrently); completion
        // redirects the outputs back into the home inventory (`auto` flag).
        // Deliberately bypasses the skill gate -- owning the machine IS the
        // unlock; the machine does the work, not the player's hands.
        //
        // Adversarial-review hardening (2026-07-01):
        // - Automation ALWAYS requires + consumes REAL inputs, even in creative
        //   mode. Creative's free-craft bypass is a per-click manual convenience;
        //   applied here it turned every auto machine into an unbounded item
        //   printer running from the main menu (flooding the persisted inventory
        //   and auto-firing XP/quest events). Automation exists to demonstrate
        //   material FLOW, so it uses real materials unconditionally.
        // - Each start re-validates against the NOW-current inventory right
        //   before consuming: without this, two machines sharing an input both
        //   passed the same pre-consumption snapshot and duplicated outputs from
        //   one batch of stock (consume_inputs ignores deficits).
        // - A batch only starts when its OUTPUTS also fit (`outputs_fit`):
        //   otherwise a full inventory became an unattended grinder, consuming
        //   inputs forever and overflow-discarding every output.
        // Live production status (v0.681, operator feedback: "assembling" with
        // no percentage or reason is useless): one honest line per auto machine,
        // published to the "auto_craft_status" channel for the GUI.
        let mut statuses: Vec<String> = Vec::new();
        if let (Some(recipes), Some(player_e)) = (recipe_registry, player) {
            // Home-storage stock (v0.737, operator field report: "there IS
            // iron in my inventory and/or garage"): auto machines also draw
            // from the home's organize-layer containers (garage bags, trunks,
            // duffels). lib.rs mirrors the placed-item counts into this shared
            // map right before the tick and drains whatever we consume from it
            // back out of the GUI containers right after. Backpack is consumed
            // FIRST, home storage covers the remainder.
            let home_stock = data
                .get::<std::sync::Mutex<std::collections::HashMap<String, u32>>>("home_stock");
            let home_count = |id: &str| -> u32 {
                home_stock
                    .as_ref()
                    .and_then(|m| m.lock().ok().map(|s| s.get(id).copied().unwrap_or(0)))
                    .unwrap_or(0)
            };
            let mut auto_starts: Vec<(hecs::Entity, String)> = Vec::new();
            for (machine, auto) in world
                .query::<&crate::ecs::components::AutoRefine>()
                .iter()
            {
                if let Some(craft) = self.active_crafts.iter().find(|c| c.crafter == machine) {
                    // Mid-batch: report real progress.
                    if let Some(r) = recipes.recipes.get(&craft.recipe_id) {
                        let pct = if r.craft_time > 0.0 {
                            ((1.0 - craft.time_remaining / r.craft_time) * 100.0).clamp(0.0, 99.0)
                        } else {
                            99.0
                        };
                        statuses.push(format!("{} — {pct:.0}%", r.name));
                    }
                    continue;
                }
                if recipes.recipes.contains_key(&auto.recipe_id) {
                    auto_starts.push((machine, auto.recipe_id.clone()));
                } else {
                    statuses.push(format!("Unknown recipe '{}'", auto.recipe_id));
                }
            }
            for (machine, recipe_id) in auto_starts {
                let recipe = recipes.recipes.get(&recipe_id).cloned();
                let Some(recipe) = recipe else { continue };
                // Authoritative check at consume time (not a stale snapshot):
                // inputs present AND output space available. Failures report WHY.
                let blocked: Option<String> = match world.get::<&Inventory>(player_e) {
                    Ok(inv) => {
                        // First missing input, by name, with the shortfall.
                        // Counts the backpack AND home storage (v0.737) — the
                        // v0.735 "in your backpack" qualifier is gone because
                        // ore stashed into a clothing bag now genuinely feeds
                        // the smelter.
                        let missing = recipe.inputs.iter().find_map(|(id, qty)| {
                            let have = inv.count_item(id) + home_count(id);
                            (have < *qty).then(|| {
                                let name = item_registry
                                    .and_then(|r| r.items.get(id).map(|d| d.name.clone()))
                                    .unwrap_or_else(|| id.clone());
                                format!("waiting for {name} x{}", qty - have)
                            })
                        });
                        if let Some(m) = missing {
                            Some(m)
                        } else if !Self::outputs_fit(&inv, &recipe, item_registry, vehicle_kits) {
                            Some("inventory full".to_string())
                        } else {
                            None
                        }
                    }
                    Err(_) => Some("no home inventory".to_string()),
                };
                // Vessel escape (v0.732): a machine whose OWN vessel can take
                // every output (right class + enough room) may start even when
                // the pack is full — the outputs stay in the vessel, not the
                // inventory (deliver_outputs' outputs-to-own-vessel rule).
                let blocked = if matches!(blocked.as_deref(), Some("inventory full")) {
                    let vessel_takes_all = data
                        .get::<crate::systems::inventory::containers::ContainerRegistry>(
                            "container_registry",
                        )
                        .and_then(|creg| {
                            world
                                .get::<&crate::systems::inventory::containers::Container>(machine)
                                .ok()
                                .map(|c| {
                                    let need: f32 = recipe
                                        .outputs
                                        .iter()
                                        .map(|(id, qty)| {
                                            item_registry.map(|r| r.volume_for(id)).unwrap_or(0.0)
                                                * *qty as f32
                                        })
                                        .sum();
                                    recipe.outputs.iter().all(|(id, _)| {
                                        let class = item_registry
                                            .map(|r| r.class_for(id).to_string())
                                            .unwrap_or_else(|| "solid".to_string());
                                        creg.check(&c.container_type_id, &class).is_accepted()
                                            && (c.current_content_item.is_none()
                                                || c.current_content_item.as_deref() == Some(id))
                                    }) && c.remaining_liters() >= need
                                })
                        })
                        .unwrap_or(false);
                    if vessel_takes_all { None } else { blocked }
                } else {
                    blocked
                };
                if let Some(why) = blocked {
                    statuses.push(format!("{} — {why}", recipe.name));
                    continue;
                }
                // The machine's pose IS the factory pad for vehicle outputs;
                // captured now because the machine may despawn mid-batch.
                let pad = world
                    .get::<&crate::ecs::components::Transform>(machine)
                    .ok()
                    .map(|t| (t.position, t.rotation));
                // Vehicle recipes also need a FREE pad lane before consuming
                // inputs (review fix, v0.679): a full lot pauses the line, the
                // same way a full inventory pauses an item recipe -- otherwise
                // the assembler grinds materials into coincident overlapping
                // vehicles forever.
                if Self::has_vehicle_output(&recipe, vehicle_kits) {
                    let (base, rot) = pad.unwrap_or((glam::Vec3::ZERO, glam::Quat::IDENTITY));
                    if Self::free_pad_lane(world, base, rot).is_none() {
                        statuses.push(format!("{} — pad full, line paused", recipe.name));
                        continue;
                    }
                }
                // Consume backpack-first; whatever the pack lacks comes out of
                // home storage (the map decrement is drained from the GUI's
                // placed containers by lib.rs right after this tick).
                if let Ok(mut inv) = world.get::<&mut Inventory>(player_e) {
                    for (id, qty) in &recipe.inputs {
                        let from_pack = inv.count_item(id).min(*qty);
                        if from_pack > 0 {
                            inv.remove_item(id, from_pack);
                        }
                        let remainder = qty - from_pack;
                        if remainder > 0 {
                            if let Some(m) = home_stock.as_ref() {
                                if let Ok(mut s) = m.lock() {
                                    if let Some(c) = s.get_mut(id) {
                                        *c = c.saturating_sub(remainder);
                                    }
                                }
                            }
                        }
                    }
                }
                statuses.push(format!("{} — starting", recipe.name));
                self.active_crafts.push(ActiveCraft {
                    recipe_id,
                    time_remaining: recipe.craft_time.max(0.01),
                    crafter: machine,
                    auto: true,
                    pad,
                });
            }
        }
        if let Some(slot) = data.get::<std::sync::Mutex<Vec<String>>>("auto_craft_status") {
            if let Ok(mut s) = slot.lock() {
                *s = statuses;
            }
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

                // Station gate (v0.749, ladder rung 6): a recipe that names a
                // required_station needs that machine PLACED in the home —
                // bread needs a stove, smelting needs a smelter, not a field.
                // The station id is item-style ("stove_0"); strip the suffix
                // to the machine type. Fail-OPEN when the placed-types set is
                // absent (headless tests, worlds without a home) and in
                // creative; AutoRefine machines ARE their station (auto path
                // untouched).
                if !creative {
                    if let Some(station) = recipe
                        .required_station
                        .as_deref()
                        .filter(|s| !s.is_empty() && *s != "none")
                    {
                        let machine_type = station.strip_suffix("_0").unwrap_or(station);
                        let placed_ok = data
                            .get::<std::sync::Mutex<std::collections::HashSet<String>>>(
                                "placed_machine_types",
                            )
                            .and_then(|m| m.lock().ok().map(|set| set.contains(machine_type)))
                            .unwrap_or(true);
                        if !placed_ok {
                            log::debug!(
                                "Cannot craft {}: no {} placed in the home",
                                recipe.id,
                                machine_type
                            );
                            continue;
                        }
                    }
                }

                // Validate inventory has required inputs (creative mode bypasses).
                let can_craft = if creative {
                    true
                } else {
                    match world.get::<&Inventory>(request.crafter) {
                        Ok(inv) => Self::can_craft(&inv, &recipe),
                        Err(_) => {
                            log::warn!("Craft request on entity without Inventory");
                            continue;
                        }
                    }
                };

                if !can_craft {
                    log::debug!("Insufficient materials for recipe: {}", recipe.id);
                    continue;
                }

                // Consume inputs immediately (skipped in creative mode).
                if !creative {
                    if let Ok(mut inv) = world.get::<&mut Inventory>(request.crafter) {
                        Self::consume_inputs(&mut inv, &recipe);
                    }
                }

                // The crafter's pose is the pad for vehicle outputs (a manual
                // vehicle craft rolls out in front of the PLAYER).
                let pad = world
                    .get::<&crate::ecs::components::Transform>(request.crafter)
                    .ok()
                    .map(|t| (t.position, t.rotation));

                // If instant craft (time <= 0), produce outputs immediately
                if recipe.craft_time <= 0.0 {
                    Self::deliver_outputs(
                        world,
                        data,
                        &recipe,
                        request.crafter,
                        pad,
                        item_registry,
                        vehicle_kits,
                        // Manual/instant crafts: the crafter IS the player; a
                        // player has no Container, so the vessel pass no-ops.
                        Some(request.crafter),
                    );
                    log::debug!("Instant craft complete: {}", recipe.id);
                } else {
                    // Queue as active craft with timer
                    self.active_crafts.push(ActiveCraft {
                        recipe_id: recipe.id.clone(),
                        time_remaining: recipe.craft_time,
                        crafter: request.crafter,
                        auto: false,
                        pad,
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

        // Advance active crafts. Timers run on GAME time (v0.663): scaling by the
        // current time_scale means "accelerated for testing" speeds every craft,
        // not just the wall clock. Absent game_time (unit tests) = raw dt.
        let sdt = crate::systems::time::scaled_dt(dt, data);
        let mut completed = Vec::new();
        for (i, craft) in self.active_crafts.iter_mut().enumerate() {
            craft.time_remaining -= sdt;
            if craft.time_remaining <= 0.0 {
                completed.push(i);
            }
        }

        // Process completions (reverse order to preserve indices)
        for i in completed.into_iter().rev() {
            let craft = self.active_crafts.remove(i);

            if let Some(recipes) = recipe_registry {
                if let Some(recipe) = recipes.recipes.get(&craft.recipe_id) {
                    // AutoRefine crafts carry the MACHINE as crafter (for
                    // per-machine concurrency) but their outputs land in the
                    // HOME inventory the inputs came from -- keyed on the
                    // `auto` FLAG, not a lookup on the crafter entity, because
                    // the machine may have been despawned mid-batch (load_world
                    // respawns every HomeMachine); the batch's consumed inputs
                    // must still come back as outputs (v0.663 review fix).
                    let target = if craft.auto { player } else { Some(craft.crafter) };
                    if let Some(target) = target {
                        Self::deliver_outputs(
                            world,
                            data,
                            recipe,
                            target,
                            craft.pad,
                            item_registry,
                            vehicle_kits,
                            // Auto crafts: the crafter is the MACHINE — its own
                            // vessel gets first claim on the outputs (v0.732).
                            Some(craft.crafter),
                        );
                        log::debug!("Craft complete: {}", recipe.id);
                    }
                }
            }
        }
    }
}

impl CraftingSystem {
    /// Does this recipe produce at least one vehicle-class output?
    fn has_vehicle_output(
        recipe: &Recipe,
        vehicle_kits: Option<&crate::systems::vehicles::VehicleKitRegistry>,
    ) -> bool {
        vehicle_kits.is_some_and(|k| {
            recipe.outputs.iter().any(|(id, _)| k.get_vehicle(id).is_some())
        })
    }

    /// The world position of pad lane `lane` for a factory at `base`/`rot`.
    fn pad_lane_pos(base: glam::Vec3, rot: glam::Quat, lane: u32) -> glam::Vec3 {
        base + rot * glam::Vec3::new(0.0, 0.0, PAD_OFFSET_M + lane as f32 * PAD_SPACING_M)
    }

    /// First free pad lane at this factory: the lowest lane index whose slot has
    /// no existing Vehicle parked within half a lane's spacing. Occupancy-aware
    /// by QUERYING the world (review fix, v0.679) rather than counting spawns --
    /// it survives restarts for free and reuses lanes that empty out when Stage 3
    /// lets vehicles drive away. None = every lane full (the line should pause).
    fn free_pad_lane(world: &hecs::World, base: glam::Vec3, rot: glam::Quat) -> Option<u32> {
        let clearance_sq = (PAD_SPACING_M * 0.45) * (PAD_SPACING_M * 0.45);
        'lanes: for lane in 0..MAX_PAD_LANES {
            let slot = Self::pad_lane_pos(base, rot, lane);
            for (_e, (_v, t)) in world
                .query::<(&crate::ecs::components::Vehicle, &crate::ecs::components::Transform)>()
                .iter()
            {
                if (t.position - slot).length_squared() < clearance_sq {
                    continue 'lanes;
                }
            }
            return Some(lane);
        }
        None
    }

    /// Deliver a completed recipe's outputs (economy Phase 2 Stage 2, v0.679):
    /// vehicle-class outputs -- item ids the kit registry resolves as ASSEMBLED
    /// vehicles -- spawn as real Vehicle entities at the factory pad; every other
    /// output lands in `target`'s inventory exactly as before. Fires the XP +
    /// quest-event hooks once per completion, matching the old behavior.
    ///
    /// Pad resolution: the pose captured at batch START (survives a machine
    /// despawned mid-batch; machines don't move), else the crafter/target's
    /// CURRENT Transform (manual crafts follow the player), else the origin.
    #[allow(clippy::too_many_arguments)]
    fn deliver_outputs(
        world: &mut hecs::World,
        data: &DataStore,
        recipe: &Recipe,
        target: hecs::Entity,
        pad: Option<(glam::Vec3, glam::Quat)>,
        item_registry: Option<&crate::systems::inventory::ItemRegistry>,
        vehicle_kits: Option<&crate::systems::vehicles::VehicleKitRegistry>,
        // The MACHINE that crafted this (auto crafts) — a machine with its own
        // compatible vessel keeps the output there (v0.732, fuel loop: the
        // refinery's product fills its drum). None/player for manual crafts.
        crafter_vessel: Option<hecs::Entity>,
    ) {
        // Split out the vehicle-class outputs (usually none).
        let vehicle_outputs: Vec<(String, u32)> = match vehicle_kits {
            Some(kits) => recipe
                .outputs
                .iter()
                .filter(|(id, _)| kits.get_vehicle(id).is_some())
                .cloned()
                .collect(),
            None => Vec::new(),
        };

        if !vehicle_outputs.is_empty() {
            let (base, rot) = pad
                .or_else(|| {
                    world
                        .get::<&crate::ecs::components::Transform>(target)
                        .ok()
                        .map(|t| (t.position, t.rotation))
                })
                .unwrap_or((glam::Vec3::ZERO, glam::Quat::IDENTITY));
            let kits = vehicle_kits.expect("vehicle_outputs non-empty implies registry");
            for (item_id, qty) in &vehicle_outputs {
                let Some(def) = kits.get_vehicle(item_id) else { continue };
                for _ in 0..*qty {
                    // Occupancy-aware lane pick (review fix, v0.679): querying
                    // the world each time means consecutive BATCHES advance down
                    // the line too, not just multiple units of one completion.
                    // The auto arm refuses to start when no lane is free, so a
                    // full pad here means a MANUAL craft on a full line -- the
                    // human clicked, so deliver anyway at lane 0 with a warning
                    // (same lossy-with-warning contract manual item crafts have).
                    let lane = Self::free_pad_lane(world, base, rot).unwrap_or_else(|| {
                        log::warn!(
                            "Factory pad full ({MAX_PAD_LANES} lanes): {} overlaps lane 0",
                            def.display_name
                        );
                        0
                    });
                    let pos = Self::pad_lane_pos(base, rot, lane);
                    world.spawn((
                        crate::ecs::components::Vehicle { item_id: item_id.clone() },
                        crate::ecs::components::Transform {
                            position: pos,
                            rotation: rot,
                            scale: glam::Vec3::ONE,
                        },
                        crate::ecs::components::Velocity::default(),
                        crate::ecs::components::VehicleSeat {
                            occupant_key: None,
                            seat_type: "pilot".to_string(),
                        },
                        crate::ecs::components::Name(def.display_name.clone()),
                    ));
                    log::info!("Factory pad: {} rolled out at {pos}", def.display_name);
                }
            }
        }

        // Non-vehicle outputs land in the inventory — unless the crafting
        // MACHINE carries its own compatible vessel, which keeps them first
        // (v0.732, fuel loop slice 1: the refinery's refined fuel fills its
        // steel drum; the walk-up card shows the drum filling live).
        // Compatibility is PRE-checked (try_store damages wrong-class vessels
        // by design); vessel overflow falls through to the inventory.
        if vehicle_outputs.len() < recipe.outputs.len() {
            let mut inv_recipe = recipe.clone();
            inv_recipe
                .outputs
                .retain(|(id, _)| !vehicle_outputs.iter().any(|(vid, _)| vid == id));
            if let Some(machine) = crafter_vessel {
                if let Some(creg) = data
                    .get::<crate::systems::inventory::containers::ContainerRegistry>(
                        "container_registry",
                    )
                {
                    if let Ok(mut c) = world
                        .get::<&mut crate::systems::inventory::containers::Container>(machine)
                    {
                        use crate::systems::inventory::containers::StoreOutcome;
                        for (id, qty) in inv_recipe.outputs.iter_mut() {
                            if *qty == 0 {
                                continue;
                            }
                            let class = item_registry
                                .map(|r| r.class_for(id).to_string())
                                .unwrap_or_else(|| "solid".to_string());
                            if !creg.check(&c.container_type_id, &class).is_accepted() {
                                continue;
                            }
                            let unit_vol =
                                item_registry.map(|r| r.volume_for(id)).unwrap_or(0.0);
                            if let StoreOutcome::Stored { quantity } =
                                creg.try_store(&mut c, id, &class, unit_vol, *qty)
                            {
                                *qty -= quantity;
                                log::info!(
                                    "[Crafting] {} kept {}x {} in its own vessel",
                                    recipe.id, quantity, id
                                );
                            }
                        }
                        inv_recipe.outputs.retain(|(_, qty)| *qty > 0);
                    }
                }
            }
            if inv_recipe.outputs.is_empty() {
                return;
            }
            if let Ok(mut inv) = world.get::<&mut Inventory>(target) {
                Self::produce_outputs(&mut inv, &inv_recipe, item_registry);
            } else {
                log::warn!("Craft complete but entity lost Inventory: {}", recipe.id);
            }
        }

        Self::on_craft_complete(data, recipe);
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

#[cfg(test)]
mod auto_refine_tests {
    use super::*;
    use crate::ecs::components::{AutoRefine, Controllable};
    use crate::systems::inventory::{Inventory, ItemRegistry};

    fn real_data() -> DataStore {
        let mut data = DataStore::new();
        data.insert(
            "recipe_registry",
            RecipeRegistry::from_csv(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/recipes.csv"
            )))
            .expect("recipes.csv"),
        );
        data.insert(
            "item_registry",
            ItemRegistry::from_csv(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/items.csv"
            )))
            .expect("items.csv"),
        );
        data
    }

    /// Economy automation Phase 1 (v0.663): a smelter machine with an AutoRefine
    /// marker refines ore sitting in the HOME inventory into ingots with ZERO
    /// craft clicks -- inputs consumed, timed batch, output back into the home
    /// stock. No skill gate: owning the machine is the unlock.
    #[test]
    fn auto_refine_smelts_home_stock_without_a_craft_click() {
        let data = real_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 20);
        inv.add_item("coal_0", 1, 99);
        let player = world.spawn((inv, Controllable));
        world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },));

        let mut sys = CraftingSystem::new();
        for _ in 0..12 {
            sys.tick(&mut world, 1.0, &data); // smelt_iron is a 10s recipe
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert!(inv.has_item("iron_ingot_0", 1), "ore auto-refined into an ingot");
        assert_eq!(inv.count_item("iron_ore_0"), 0, "inputs were consumed");
        assert_eq!(inv.count_item("coal_0"), 0, "coal was consumed");
    }

    /// THE LIVING-ECOSYSTEM CHAIN (operator vision, 2026-07-01): commission ONE
    /// mining drone and then touch nothing -- the drone mines + delivers iron ore
    /// to the home stock, the smelter auto-refines it into an ingot, and the
    /// workbench auto-crafts the ingot + a wood plank into a hammer. Raw rock in
    /// space becomes a tool on the shelf with zero further interaction.
    #[test]
    fn full_chain_drone_ore_becomes_a_hammer_untouched() {
        use crate::ecs::components::AsteroidBody;

        let mut data = real_data();
        data.insert(
            "commission_drone",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
        );

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        // The home already holds the auxiliary inputs; the IRON comes from space.
        inv.add_item("coal_0", 1, 99);
        inv.add_item("wood_plank_0", 1, 99);
        let player = world.spawn((inv, Controllable));
        world.spawn((AsteroidBody {
            id: "rock".to_string(),
            name: "rock".to_string(),
            classification: "M".into(),
            ores: [("iron_ore_0".to_string(), 2.0)].into_iter().collect(),
            position: [0.0, 0.0, 0.0],
        },));
        world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },));
        world.spawn((AutoRefine { recipe_id: "craft_hammer".to_string() },));

        // The ONLY player action in this test: commission the drone.
        *data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("commission_drone")
            .unwrap()
            .lock()
            .unwrap() = Some(("rock".to_string(), vec![("iron_ore_0".to_string(), 2)]));

        let mut drones = crate::systems::mining::DroneSystem::new();
        let mut crafting = CraftingSystem::new();
        for _ in 0..60 {
            drones.tick(&mut world, 1.0, &data);
            crafting.tick(&mut world, 1.0, &data);
        }

        let inv = world.get::<&Inventory>(player).unwrap();
        assert!(
            inv.has_item("hammer_0", 1),
            "drone ore should have become a hammer with zero interaction (have: ingots={}, ore={})",
            inv.count_item("iron_ingot_0"),
            inv.count_item("iron_ore_0"),
        );
    }

    /// Review fix (2026-07-01): creative mode must NOT turn auto machines into
    /// item printers -- automation always requires real inputs, so an empty
    /// home stock produces nothing even with creative_mode on (which is the
    /// app's default and used to mint hammers from the main menu forever).
    #[test]
    fn creative_mode_does_not_let_machines_print_items() {
        let mut data = real_data();
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable)); // EMPTY stock
        world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },));
        world.spawn((AutoRefine { recipe_id: "craft_hammer".to_string() },));

        let mut sys = CraftingSystem::new();
        for _ in 0..30 {
            sys.tick(&mut world, 1.0, &data);
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("iron_ingot_0"), 0, "no inputs = no ingots, even creative");
        assert_eq!(inv.count_item("hammer_0"), 0, "no inputs = no hammers, even creative");
    }

    /// Review fix (2026-07-01): two machines sharing recipe inputs must not
    /// both start from ONE batch of stock (the same-tick TOCTOU that minted 2
    /// ingots from 2 ore + 1 coal). Exactly one batch's output may appear.
    #[test]
    fn two_machines_sharing_inputs_cannot_duplicate() {
        let data = real_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 20);
        inv.add_item("coal_0", 1, 99);
        let player = world.spawn((inv, Controllable));
        world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },));
        world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },)); // second smelter

        let mut sys = CraftingSystem::new();
        for _ in 0..25 {
            sys.tick(&mut world, 1.0, &data);
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(
            inv.count_item("iron_ingot_0"),
            1,
            "one batch of inputs must yield exactly one ingot, never two"
        );
    }

    /// Review fix (2026-07-01): a machine despawned mid-batch (load_world
    /// respawns every HomeMachine on Enter World) must not destroy the batch --
    /// the consumed inputs still come back as outputs into the home inventory.
    #[test]
    fn machine_despawned_mid_batch_still_delivers_to_home() {
        let data = real_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 20);
        inv.add_item("coal_0", 1, 99);
        let player = world.spawn((inv, Controllable));
        let smelter = world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },));

        let mut sys = CraftingSystem::new();
        sys.tick(&mut world, 1.0, &data); // batch starts (inputs consumed)
        {
            let inv = world.get::<&Inventory>(player).unwrap();
            assert_eq!(inv.count_item("iron_ore_0"), 0, "batch consumed the ore");
        }
        world.despawn(smelter).unwrap(); // the world-reload despawn
        for _ in 0..12 {
            sys.tick(&mut world, 1.0, &data);
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(
            inv.count_item("iron_ingot_0"),
            1,
            "the in-flight batch must complete into the home inventory, not vanish"
        );
    }

    /// Review fix (2026-07-01): a FULL home inventory must stop batches from
    /// starting at all (no input consumption), instead of grinding the whole
    /// stock into overflow-discarded outputs.
    #[test]
    fn full_inventory_blocks_batch_start_without_consuming() {
        let data = real_data();
        let mut world = hecs::World::new();
        // 2 slots only: ore stack + coal stack -- zero room for an ingot.
        let mut inv = Inventory::new(2);
        inv.add_item("iron_ore_0", 4, 20);
        inv.add_item("coal_0", 2, 99);
        let player = world.spawn((inv, Controllable));
        world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },));

        let mut sys = CraftingSystem::new();
        for _ in 0..25 {
            sys.tick(&mut world, 1.0, &data);
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("iron_ore_0"), 4, "inputs untouched while outputs cannot fit");
        assert_eq!(inv.count_item("coal_0"), 2, "coal untouched while outputs cannot fit");
        assert_eq!(inv.count_item("iron_ingot_0"), 0);
    }

    /// Craft timers run on GAME time (v0.663): at time_scale 10 a 10s recipe
    /// completes in ~1s of real dt; at scale 1 it takes the full 10s.
    #[test]
    fn craft_timers_respect_time_scale() {
        let mut data = real_data();
        let mut gt = crate::systems::time::GameTime::default();
        gt.time_scale = 10.0;
        data.insert("game_time", std::sync::Mutex::new(gt));

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 20);
        inv.add_item("coal_0", 1, 99);
        let player = world.spawn((inv, Controllable));
        world.spawn((AutoRefine { recipe_id: "smelt_iron".to_string() },));

        let mut sys = CraftingSystem::new();
        // 3 ticks of 1s at 10x = 30 scaled seconds; the 10s smelt must be done.
        for _ in 0..3 {
            sys.tick(&mut world, 1.0, &data);
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert!(
            inv.has_item("iron_ingot_0", 1),
            "at 10x time scale the 10s smelt completes within 3 real seconds"
        );
    }
}

#[cfg(test)]
mod vehicle_factory_tests {
    use super::*;
    use crate::ecs::components::{AutoRefine, Controllable, Name, Transform, Vehicle};
    use crate::systems::inventory::{Inventory, ItemRegistry};
    use crate::systems::vehicles::VehicleKitRegistry;

    /// Real data files: recipes (incl. assemble_rover), items, and the vehicle
    /// kit registry -- the factory branch resolves vehicle outputs through it.
    fn factory_data() -> DataStore {
        let mut data = DataStore::new();
        data.insert(
            "recipe_registry",
            RecipeRegistry::from_csv(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/recipes.csv"
            )))
            .expect("recipes.csv"),
        );
        data.insert(
            "item_registry",
            ItemRegistry::from_csv(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/items.csv"
            )))
            .expect("items.csv"),
        );
        data.insert(
            "vehicle_kit_registry",
            VehicleKitRegistry::from_ron(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/vehicles/kits.ron"
            )))
            .expect("kits.ron"),
        );
        data
    }

    fn rover_materials(inv: &mut Inventory) {
        inv.add_item("steel_ingot_0", 6, 20);
        inv.add_item("iron_ingot_0", 4, 20);
        inv.add_item("rubber_sheet_0", 4, 10);
    }

    fn assembler_at(world: &mut hecs::World, pos: glam::Vec3) -> hecs::Entity {
        world.spawn((
            AutoRefine { recipe_id: "assemble_rover".to_string() },
            Transform { position: pos, rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
        ))
    }

    fn vehicles(world: &mut hecs::World) -> Vec<(String, glam::Vec3)> {
        world
            .query_mut::<(&Vehicle, &Transform)>()
            .into_iter()
            .map(|(_e, (v, t))| (v.item_id.clone(), t.position))
            .collect()
    }

    /// THE STAGE 2 HEADLINE: an assembler machine with materials in the home
    /// stock rolls a REAL rover onto the pad in front of it, untouched -- the
    /// vehicle is a world entity, NOT an inventory item.
    #[test]
    fn auto_assembler_rolls_a_rover_onto_the_pad() {
        let data = factory_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        rover_materials(&mut inv);
        let player = world.spawn((inv, Controllable));
        let machine_pos = glam::Vec3::new(10.0, 0.0, -4.0);
        assembler_at(&mut world, machine_pos);

        let mut sys = CraftingSystem::new();
        for _ in 0..125 {
            sys.tick(&mut world, 1.0, &data); // assemble_rover is a 120s recipe
        }

        let v = vehicles(&mut world);
        assert_eq!(v.len(), 1, "one rover rolled out");
        assert_eq!(v[0].0, "rover_0");
        let expected = machine_pos + glam::Vec3::new(0.0, 0.0, PAD_OFFSET_M);
        assert!(
            (v[0].1 - expected).length() < 1e-3,
            "rover at the pad in front of the machine (got {:?}, want {expected:?})",
            v[0].1
        );
        // The spawned entity is a full vehicle (seat + name), same tuple Stage 1 deploys.
        assert_eq!(
            world.query_mut::<(&Vehicle, &Name)>().into_iter().count(),
            1,
            "vehicle carries a Name"
        );
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("rover_0"), 0, "the rover is NOT an inventory item");
        assert_eq!(inv.count_item("steel_ingot_0"), 0, "steel consumed");
        assert_eq!(inv.count_item("iron_ingot_0"), 0, "iron consumed");
        assert_eq!(inv.count_item("rubber_sheet_0"), 0, "rubber consumed");
    }

    /// v0.737 (operator field report: "there IS iron in my inventory and/or
    /// garage"): auto machines draw from HOME STORAGE too. Backpack is
    /// consumed first; the shortfall comes out of the mirrored home-stock map
    /// (lib.rs drains that decrement from the GUI containers after the tick).
    #[test]
    fn auto_machine_draws_missing_inputs_from_home_stock() {
        let data = {
            let mut d = factory_data();
            // Garage holds the rubber the backpack lacks (plus spare steel that
            // must NOT be touched — the pack already covers steel in full).
            let mut stock = std::collections::HashMap::new();
            stock.insert("rubber_sheet_0".to_string(), 3u32);
            stock.insert("steel_ingot_0".to_string(), 5u32);
            d.insert("home_stock", std::sync::Mutex::new(stock));
            d
        };
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("steel_ingot_0", 6, 20);
        inv.add_item("iron_ingot_0", 4, 20);
        inv.add_item("rubber_sheet_0", 1, 10); // recipe needs 4 — 3 short
        world.spawn((inv, Controllable));
        assembler_at(&mut world, glam::Vec3::ZERO);

        let mut sys = CraftingSystem::new();
        for _ in 0..125 {
            sys.tick(&mut world, 1.0, &data);
        }

        assert_eq!(vehicles(&mut world).len(), 1, "rover built from pack + home storage");
        let stock = data
            .get::<std::sync::Mutex<std::collections::HashMap<String, u32>>>("home_stock")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(
            stock.get("rubber_sheet_0").copied().unwrap_or(0),
            0,
            "home storage covered the 3 missing rubber sheets"
        );
        assert_eq!(
            stock.get("steel_ingot_0").copied().unwrap_or(0),
            5,
            "backpack-first: fully-stocked inputs never touch home storage"
        );
    }

    /// A FULL backpack must not stall the assembly line: the vehicle output
    /// needs no inventory slot (outputs_fit skips vehicle-class outputs), so
    /// production proceeds where a normal recipe would be blocked.
    #[test]
    fn full_inventory_does_not_block_the_assembly_line() {
        let data = factory_data();
        let mut world = hecs::World::new();
        // Exactly 4 slots: 3 hold the materials, 1 holds junk. ZERO free slots
        // and no stack headroom for anything new.
        let mut inv = Inventory::new(4);
        rover_materials(&mut inv);
        inv.add_item("hammer_0", 1, 1);
        assert_eq!(inv.slots.iter().filter(|s| s.is_none()).count(), 0, "no free slot");
        world.spawn((inv, Controllable));
        assembler_at(&mut world, glam::Vec3::ZERO);

        let mut sys = CraftingSystem::new();
        for _ in 0..125 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert_eq!(vehicles(&mut world).len(), 1, "rover produced despite a full backpack");
    }

    /// The machine despawning mid-batch (load_world respawns every HomeMachine)
    /// must not lose the vehicle: it rolls out at the pad captured at START.
    #[test]
    fn machine_despawned_mid_batch_still_rolls_out_at_the_pad() {
        let data = factory_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        rover_materials(&mut inv);
        world.spawn((inv, Controllable));
        let machine_pos = glam::Vec3::new(-7.0, 0.0, 2.0);
        let machine = assembler_at(&mut world, machine_pos);

        let mut sys = CraftingSystem::new();
        sys.tick(&mut world, 1.0, &data); // batch starts, pad captured
        world.despawn(machine).expect("despawn machine mid-batch");
        for _ in 0..125 {
            sys.tick(&mut world, 1.0, &data);
        }

        let v = vehicles(&mut world);
        assert_eq!(v.len(), 1, "the batch's consumed inputs still became a vehicle");
        let expected = machine_pos + glam::Vec3::new(0.0, 0.0, PAD_OFFSET_M);
        assert!(
            (v[0].1 - expected).length() < 1e-3,
            "vehicle at the CAPTURED pad (got {:?}, want {expected:?})",
            v[0].1
        );
    }

    /// A MANUAL vehicle craft (the player clicks Assemble Rover at the station)
    /// rolls the rover out in front of the PLAYER, not into the inventory.
    #[test]
    fn manual_vehicle_craft_spawns_at_the_crafter_not_in_inventory() {
        let data = factory_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        rover_materials(&mut inv);
        // assemble_rover requires metalworking level 2 (gating starts at 2).
        let mut skills = crate::systems::skills::PlayerSkills::new();
        skills.skills.insert(
            "metalworking".to_string(),
            crate::systems::skills::SkillProgress { level: 2, xp: 0 },
        );
        let player_pos = glam::Vec3::new(1.0, 0.0, 1.0);
        let player = world.spawn((
            inv,
            Controllable,
            skills,
            Transform { position: player_pos, rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
        ));

        let mut sys = CraftingSystem::new();
        sys.request_craft("assemble_rover".to_string(), player);
        for _ in 0..125 {
            sys.tick(&mut world, 1.0, &data);
        }

        let v = vehicles(&mut world);
        assert_eq!(v.len(), 1, "manual craft produced the rover");
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("rover_0"), 0, "not an inventory item");
        assert!(
            (v[0].1 - player_pos).length() <= PAD_OFFSET_M + 1e-3,
            "rover rolled out next to the player"
        );
    }


    /// Live production status (v0.681, operator feedback): an idle assembler
    /// says exactly WHY ("waiting for Steel Ingot x6"), and a running batch
    /// reports a live percentage instead of an authored "assembling" string.
    #[test]
    fn production_status_reports_why_idle_and_live_progress() {
        let mut data = factory_data();
        data.insert("auto_craft_status", std::sync::Mutex::new(Vec::<String>::new()));
        let mut world = hecs::World::new();
        // Empty stock: the line must WAIT and say what for.
        world.spawn((Inventory::new(16), Controllable));
        assembler_at(&mut world, glam::Vec3::ZERO);

        let mut sys = CraftingSystem::new();
        sys.tick(&mut world, 1.0, &data);
        let status = data
            .get::<std::sync::Mutex<Vec<String>>>("auto_craft_status")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(status.len(), 1);
        assert!(
            status[0].contains("waiting for Steel Ingot x6"),
            "idle line names the first missing input: {status:?}"
        );

        // Stock the materials: the next ticks report starting, then a percent.
        for (_e, (inv, _c)) in world.query_mut::<(&mut Inventory, &Controllable)>() {
            rover_materials(inv);
        }
        sys.tick(&mut world, 1.0, &data); // starts
        for _ in 0..59 {
            sys.tick(&mut world, 1.0, &data); // ~60s into the 120s batch
        }
        let status = data
            .get::<std::sync::Mutex<Vec<String>>>("auto_craft_status")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(status.len(), 1);
        assert!(
            status[0].contains('%'),
            "mid-batch line reports live progress: {status:?}"
        );
    }

    /// Data wiring lint: the assembler recipes exist in the REAL recipes.csv,
    /// their outputs resolve as vehicles in the REAL kits.ron, and the REAL
    /// home.ron catalog ships the vehicle_assembler machine driving them.
    #[test]
    fn assembler_data_is_wired() {
        let recipes = RecipeRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/recipes.csv"
        )))
        .expect("recipes.csv");
        let kits = VehicleKitRegistry::from_ron(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/vehicles/kits.ron"
        )))
        .expect("kits.ron");
        for (recipe_id, vehicle_item) in
            [("assemble_rover", "rover_0"), ("assemble_truck", "truck_pickup_0")]
        {
            let r = recipes
                .recipes
                .get(recipe_id)
                .unwrap_or_else(|| panic!("{recipe_id} in recipes.csv"));
            assert!(
                r.outputs.iter().any(|(id, _)| id == vehicle_item),
                "{recipe_id} outputs {vehicle_item}"
            );
            assert!(
                kits.get_vehicle(vehicle_item).is_some(),
                "{vehicle_item} resolves as a vehicle in kits.ron"
            );
        }
        let home_ron = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/machines/home.ron"
        ));
        assert!(
            home_ron.contains("\"vehicle_assembler\""),
            "home.ron catalog ships the vehicle_assembler"
        );
        assert!(
            home_ron.contains("Some(\"assemble_rover\")"),
            "the assembler auto-runs assemble_rover"
        );
    }
}

#[cfg(test)]
mod factory_review_fix_tests {
    use super::*;
    use crate::ecs::components::{AutoRefine, Controllable, Transform, Vehicle, VehicleSeat, Velocity};
    use crate::systems::inventory::{Inventory, ItemRegistry};
    use crate::systems::vehicles::VehicleKitRegistry;

    fn factory_data() -> DataStore {
        let mut data = DataStore::new();
        data.insert(
            "recipe_registry",
            RecipeRegistry::from_csv(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/recipes.csv"
            )))
            .expect("recipes.csv"),
        );
        data.insert(
            "item_registry",
            ItemRegistry::from_csv(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/items.csv"
            )))
            .expect("items.csv"),
        );
        data.insert(
            "vehicle_kit_registry",
            VehicleKitRegistry::from_ron(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/vehicles/kits.ron"
            )))
            .expect("kits.ron"),
        );
        data.insert("abort_active_crafts", std::sync::Mutex::new(false));
        data
    }

    fn assembler_at(world: &mut hecs::World, pos: glam::Vec3) -> hecs::Entity {
        world.spawn((
            AutoRefine { recipe_id: "assemble_rover".to_string() },
            Transform { position: pos, rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
        ))
    }

    fn rover_positions(world: &mut hecs::World) -> Vec<glam::Vec3> {
        world
            .query_mut::<(&Vehicle, &Transform)>()
            .into_iter()
            .map(|(_e, (_v, t))| t.position)
            .collect()
    }

    /// Review fix (v0.679): consecutive batches must park in DIFFERENT lanes.
    /// The pre-fix per-call lane counter reset every completion, so batch after
    /// batch spawned coincident rovers at the identical pad point.
    #[test]
    fn second_batch_parks_in_the_next_lane() {
        let data = factory_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        // Materials for exactly TWO rovers.
        inv.add_item("steel_ingot_0", 12, 20);
        inv.add_item("iron_ingot_0", 8, 20);
        inv.add_item("rubber_sheet_0", 8, 10);
        world.spawn((inv, Controllable));
        assembler_at(&mut world, glam::Vec3::ZERO);

        let mut sys = CraftingSystem::new();
        for _ in 0..250 {
            sys.tick(&mut world, 1.0, &data); // two back-to-back 120s batches
        }

        let v = rover_positions(&mut world);
        assert_eq!(v.len(), 2, "both batches produced");
        let gap = (v[0] - v[1]).length();
        assert!(
            (gap - PAD_SPACING_M).abs() < 1e-3,
            "the second rover parks one lane down, not inside the first (gap {gap})"
        );
    }

    /// Review fix (v0.679): a FULL pad pauses the assembly line -- the batch is
    /// never started and NO inputs are consumed (the item-recipe grinder guard,
    /// extended to pad space). Freeing a lane resumes production.
    #[test]
    fn assembly_line_pauses_when_the_pad_is_full_and_resumes_when_freed() {
        let data = factory_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("steel_ingot_0", 6, 20);
        inv.add_item("iron_ingot_0", 4, 20);
        inv.add_item("rubber_sheet_0", 4, 10);
        let player = world.spawn((inv, Controllable));
        assembler_at(&mut world, glam::Vec3::ZERO);
        // Park a vehicle in EVERY lane.
        let mut parked = Vec::new();
        for lane in 0..MAX_PAD_LANES {
            parked.push(world.spawn((
                Vehicle { item_id: "rover_0".to_string() },
                Transform {
                    position: CraftingSystem::pad_lane_pos(
                        glam::Vec3::ZERO,
                        glam::Quat::IDENTITY,
                        lane,
                    ),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::ONE,
                },
                Velocity::default(),
                VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
            )));
        }

        let mut sys = CraftingSystem::new();
        for _ in 0..130 {
            sys.tick(&mut world, 1.0, &data);
        }
        {
            let inv = world.get::<&Inventory>(player).unwrap();
            assert_eq!(
                inv.count_item("steel_ingot_0"),
                6,
                "full pad: the line PAUSES with inputs unconsumed"
            );
        }
        assert_eq!(
            rover_positions(&mut world).len() as u32,
            MAX_PAD_LANES,
            "no new rover while the pad is full"
        );

        // Drive one off the lot (Stage 3 someday; despawn stands in for it).
        world.despawn(parked[3]).unwrap();
        for _ in 0..130 {
            sys.tick(&mut world, 1.0, &data);
        }
        {
            let inv = world.get::<&Inventory>(player).unwrap();
            assert_eq!(inv.count_item("steel_ingot_0"), 0, "line resumed once a lane freed");
        }
        assert_eq!(
            rover_positions(&mut world).len() as u32,
            MAX_PAD_LANES,
            "the freed lane holds the new rover"
        );
    }

    /// Review fix (v0.679): applying a save mid-batch REWINDS the world -- the
    /// in-flight batch must be dropped (the abort flag the apply site raises),
    /// exactly like an app restart, or the rewound inventory would ALSO receive
    /// the batch's rover (materials + vehicle from one batch of inputs).
    #[test]
    fn world_rewind_aborts_in_flight_batches() {
        let data = factory_data();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("steel_ingot_0", 6, 20);
        inv.add_item("iron_ingot_0", 4, 20);
        inv.add_item("rubber_sheet_0", 4, 10);
        world.spawn((inv, Controllable));
        assembler_at(&mut world, glam::Vec3::ZERO);

        let mut sys = CraftingSystem::new();
        sys.tick(&mut world, 1.0, &data); // batch starts, inputs consumed

        // The launcher applies a save: the main loop raises the abort flag
        // (the apply itself would also restore inventory + despawn vehicles;
        // irrelevant here -- we assert the batch side).
        *data
            .get::<std::sync::Mutex<bool>>("abort_active_crafts")
            .unwrap()
            .lock()
            .unwrap() = true;

        for _ in 0..130 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(
            rover_positions(&mut world).is_empty(),
            "the aborted batch must never deliver its rover"
        );
        // NOTE the follow-on: with materials still gone (this test never ran the
        // actual apply), the machine cannot restart -- matching restart semantics
        // where an unsaved in-flight batch simply vanishes and the SAVE decides
        // what materials exist.
    }
}
