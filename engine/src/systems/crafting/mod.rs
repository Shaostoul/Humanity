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
}

impl System for CraftingSystem {
    fn name(&self) -> &str {
        "CraftingSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let recipe_registry = data.get::<RecipeRegistry>("recipe_registry");
        let item_registry = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");

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
