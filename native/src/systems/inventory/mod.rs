//! Inventory system — slot-based item storage with stack limits.
//!
//! Item definitions loaded from `data/items.csv`.
//! Systems process pending inventory operations each tick.

pub mod items;
pub mod containers;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Default max stack size when item definition doesn't specify one.
const DEFAULT_MAX_STACK: u32 = 99;

/// A stack of identical items in an inventory slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStack {
    /// Item definition ID from items.csv.
    pub item_id: String,
    /// Current quantity in this stack.
    pub quantity: u32,
    /// Maximum items per stack (from item def or default 99).
    pub max_stack: u32,
}

impl ItemStack {
    pub fn new(item_id: String, quantity: u32, max_stack: u32) -> Self {
        Self {
            item_id,
            quantity,
            max_stack,
        }
    }

    /// How many more items can fit in this stack.
    pub fn space_remaining(&self) -> u32 {
        self.max_stack.saturating_sub(self.quantity)
    }

    /// Whether this stack is full.
    pub fn is_full(&self) -> bool {
        self.quantity >= self.max_stack
    }
}

/// Inventory component — attach to any entity that can hold items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// Ordered list of item slots (Some = occupied, None = empty).
    pub slots: Vec<Option<ItemStack>>,
    /// Maximum number of slots.
    pub max_slots: usize,
}

impl Inventory {
    pub fn new(max_slots: usize) -> Self {
        Self {
            slots: vec![None; max_slots],
            max_slots,
        }
    }

    /// Add items to the inventory, stacking where possible.
    /// Returns the number of items that could NOT be added (overflow).
    pub fn add_item(&mut self, item_id: &str, mut quantity: u32, max_stack: u32) -> u32 {
        // First pass: fill existing stacks of the same item
        for slot in self.slots.iter_mut() {
            if quantity == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.item_id == item_id && !stack.is_full() {
                    let can_add = stack.space_remaining().min(quantity);
                    stack.quantity += can_add;
                    quantity -= can_add;
                }
            }
        }

        // Second pass: place remainder in empty slots
        for slot in self.slots.iter_mut() {
            if quantity == 0 {
                break;
            }
            if slot.is_none() {
                let stack_qty = quantity.min(max_stack);
                *slot = Some(ItemStack::new(item_id.to_string(), stack_qty, max_stack));
                quantity -= stack_qty;
            }
        }

        quantity // overflow
    }

    /// Remove items from the inventory.
    /// Returns the number of items that could NOT be removed (insufficient).
    pub fn remove_item(&mut self, item_id: &str, mut quantity: u32) -> u32 {
        // Remove from last-to-first to preserve earlier stacks
        for slot in self.slots.iter_mut().rev() {
            if quantity == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.item_id == item_id {
                    let can_remove = stack.quantity.min(quantity);
                    stack.quantity -= can_remove;
                    quantity -= can_remove;
                    if stack.quantity == 0 {
                        *slot = None;
                    }
                }
            }
        }

        quantity // deficit
    }

    /// Check if the inventory contains at least `quantity` of the given item.
    pub fn has_item(&self, item_id: &str, quantity: u32) -> bool {
        self.count_item(item_id) >= quantity
    }

    /// Count total quantity of an item across all stacks.
    pub fn count_item(&self, item_id: &str) -> u32 {
        self.slots
            .iter()
            .filter_map(|s| s.as_ref())
            .filter(|s| s.item_id == item_id)
            .map(|s| s.quantity)
            .sum()
    }

    /// Transfer items from this inventory to another.
    /// Returns the number of items that could NOT be transferred.
    pub fn transfer_to(
        &mut self,
        target: &mut Inventory,
        item_id: &str,
        quantity: u32,
        max_stack: u32,
    ) -> u32 {
        let available = self.count_item(item_id).min(quantity);
        if available == 0 {
            return quantity;
        }

        // Try to add to target first
        let overflow = target.add_item(item_id, available, max_stack);
        let actually_transferred = available - overflow;

        // Remove only what was successfully added
        if actually_transferred > 0 {
            self.remove_item(item_id, actually_transferred);
        }

        quantity - actually_transferred
    }

    /// Number of occupied slots.
    pub fn used_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Number of empty slots.
    pub fn empty_slots(&self) -> usize {
        self.max_slots - self.used_slots()
    }

    /// All distinct item IDs and their total quantities.
    pub fn summary(&self) -> HashMap<String, u32> {
        let mut map = HashMap::new();
        for slot in self.slots.iter().flatten() {
            *map.entry(slot.item_id.clone()).or_insert(0) += slot.quantity;
        }
        map
    }
}

/// Pending inventory operation — queued for processing by InventorySystem.
#[derive(Debug, Clone)]
pub enum InventoryOp {
    /// Add items to an entity's inventory.
    Add {
        item_id: String,
        quantity: u32,
    },
    /// Remove items from an entity's inventory.
    Remove {
        item_id: String,
        quantity: u32,
    },
}

/// Item definition loaded from items.csv — cached in DataStore as "item_defs".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub stackable: bool,
    pub max_stack: u32,
}

/// Registry of all item definitions, keyed by item ID.
#[derive(Debug, Clone, Default)]
pub struct ItemRegistry {
    pub items: HashMap<String, ItemDef>,
}

impl ItemRegistry {
    /// Look up max stack size for an item, defaulting to 99 if unknown.
    pub fn max_stack_for(&self, item_id: &str) -> u32 {
        self.items
            .get(item_id)
            .map(|def| def.max_stack)
            .unwrap_or(DEFAULT_MAX_STACK)
    }
}

/// Manages inventory components and processes queued operations.
pub struct InventorySystem {
    /// Pending operations to process next tick (entity-indexed).
    pending_ops: Vec<(hecs::Entity, InventoryOp)>,
}

impl InventorySystem {
    pub fn new() -> Self {
        Self {
            pending_ops: Vec::new(),
        }
    }

    /// Queue an inventory operation for the next tick.
    pub fn queue_op(&mut self, entity: hecs::Entity, op: InventoryOp) {
        self.pending_ops.push((entity, op));
    }
}

impl System for InventorySystem {
    fn name(&self) -> &str {
        "InventorySystem"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        // Drain pending operations
        let ops: Vec<_> = self.pending_ops.drain(..).collect();

        // Get item registry for stack size lookups
        let registry = data.get::<ItemRegistry>("item_registry");

        for (entity, op) in ops {
            let mut inventory = match world.get::<&mut Inventory>(entity) {
                Ok(inv) => inv,
                Err(_) => {
                    log::warn!("InventoryOp on entity without Inventory component");
                    continue;
                }
            };

            match op {
                InventoryOp::Add { item_id, quantity } => {
                    let max_stack = registry
                        .map(|r| r.max_stack_for(&item_id))
                        .unwrap_or(DEFAULT_MAX_STACK);
                    let overflow = inventory.add_item(&item_id, quantity, max_stack);
                    if overflow > 0 {
                        log::debug!(
                            "Inventory full: {} of {} could not be added",
                            overflow,
                            item_id
                        );
                    }
                }
                InventoryOp::Remove { item_id, quantity } => {
                    let deficit = inventory.remove_item(&item_id, quantity);
                    if deficit > 0 {
                        log::debug!(
                            "Insufficient items: {} more {} needed",
                            deficit,
                            item_id
                        );
                    }
                }
            }
        }
    }
}
