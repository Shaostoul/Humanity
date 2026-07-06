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
    /// Current total weight of all items in kg (recalculated each tick).
    pub weight_current: f32,
    /// Maximum carry capacity in kg.
    pub weight_capacity: f32,
    /// Current total storage volume of all items in LITERS (recalculated each
    /// tick alongside weight). Material-storage Stage A (v0.726): "the real
    /// limit of a container is its volume; slots are for bandolier-likes."
    /// serde(default) so pre-v0.726 saved inventories load as 0 and the next
    /// tick fills the real number.
    #[serde(default)]
    pub volume_current_l: f32,
    /// Maximum storage volume in liters. Stage A tracks + displays it;
    /// Stage A slice 2 turns it into a hard add/craft gate.
    #[serde(default = "default_volume_capacity")]
    pub volume_capacity_l: f32,
    /// True when weight_current exceeds weight_capacity (movement penalty).
    pub encumbered: bool,
}

/// Default backpack volume: 65 L — the classic large expedition pack (the
/// mountaineering pack in the default home is literally "65 L").
fn default_volume_capacity() -> f32 {
    65.0
}

impl Inventory {
    pub fn new(max_slots: usize) -> Self {
        Self {
            slots: vec![None; max_slots],
            max_slots,
            weight_current: 0.0,
            weight_capacity: 50.0,
            volume_current_l: 0.0,
            volume_capacity_l: default_volume_capacity(),
            encumbered: false,
        }
    }

    /// Grow the inventory to at least `min_slots` total slots (never shrinks; existing
    /// contents are preserved). Used by dev/creative provisioning to fit a full
    /// material set in one pass without overflowing a default-sized inventory.
    pub fn ensure_slots(&mut self, min_slots: usize) {
        if min_slots > self.max_slots {
            self.slots.resize(min_slots, None);
            self.max_slots = min_slots;
        }
    }

    /// Volume-gated add (material-storage Stage A slice 2, v0.727): caps the
    /// accepted quantity by the inventory's remaining VOLUME before the slot
    /// pass — "the real limit of a container is its volume". Tracks
    /// `volume_current_l` incrementally so several adds within one tick can't
    /// overshoot (the per-tick recalc trues it up afterwards).
    ///
    /// `unit_volume_l <= 0.0` (unknown/legacy item) skips the volume gate and
    /// behaves exactly like `add_item` — which also remains the right primitive
    /// for bandolier-like BY-COUNT holders and for restore paths (loading a
    /// save must never drop items).
    ///
    /// Returns the number NOT added (volume overflow + slot overflow).
    pub fn add_item_volume_gated(
        &mut self,
        item_id: &str,
        quantity: u32,
        max_stack: u32,
        unit_volume_l: f32,
    ) -> u32 {
        if unit_volume_l <= 0.0 {
            return self.add_item(item_id, quantity, max_stack);
        }
        let remaining_l = (self.volume_capacity_l - self.volume_current_l).max(0.0);
        let fits = (remaining_l / unit_volume_l).floor() as u32;
        let accepted = quantity.min(fits);
        let slot_overflow = self.add_item(item_id, accepted, max_stack);
        let added = accepted - slot_overflow;
        self.volume_current_l += added as f32 * unit_volume_l;
        (quantity - accepted) + slot_overflow
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
    /// Mass per unit in kilograms (from items.csv weight_kg column).
    pub mass_kg: f32,
    /// Effective storage volume per unit in LITERS (items.csv volume_l,
    /// generated by scripts/gen-item-volumes.js from mass / material density
    /// with a per-category packing fraction). Material-storage Stage A
    /// (v0.726): "the real limit of a container is its volume" — this is the
    /// number volume-capped storage counts against. 0.0 = unknown (legacy row).
    pub volume_l: f32,
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

    /// Look up mass in kg for one unit of an item, defaulting to 0 if unknown.
    pub fn mass_for(&self, item_id: &str) -> f32 {
        self.items
            .get(item_id)
            .map(|def| def.mass_kg)
            .unwrap_or(0.0)
    }

    /// Look up storage volume in liters for one unit, 0.0 if unknown. (v0.726)
    pub fn volume_for(&self, item_id: &str) -> f32 {
        self.items
            .get(item_id)
            .map(|def| def.volume_l)
            .unwrap_or(0.0)
    }

    /// Build the item registry from raw `items.csv` bytes.
    ///
    /// Uses the shared CSV loader (skips `#` comments, header-mapped, row-resilient
    /// so one malformed row never blanks the catalog). Only the columns the
    /// registry needs are deserialized; the rest of items.csv (category,
    /// subcategory, base_material, durability, description, content_class) is
    /// ignored. This is the constructor the runtime calls to populate
    /// `DataStore["item_registry"]` — before v0.323 the CSV was loaded then
    /// discarded, so item-name/stack/mass lookups silently fell back to defaults.
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<ItemRow> = crate::assets::loader::parse_csv(data)?;
        let mut items = HashMap::new();
        for row in rows {
            let max_stack = row.stack_size.max(1);
            items.insert(
                row.id.clone(),
                ItemDef {
                    id: row.id,
                    name: row.name,
                    mass_kg: row.weight_kg,
                    volume_l: row.volume_l,
                    stackable: max_stack > 1,
                    max_stack,
                },
            );
        }
        Ok(Self { items })
    }
}

/// One row of `items.csv` — only the columns `ItemRegistry` consumes. Extra
/// columns are ignored by the header-mapped CSV deserializer.
#[derive(Debug, Deserialize)]
struct ItemRow {
    id: String,
    name: String,
    #[serde(default)]
    weight_kg: f32,
    /// Storage volume in liters (v0.726, material-storage Stage A). Default 0
    /// so a mod's items.csv without the column still parses.
    #[serde(default)]
    volume_l: f32,
    #[serde(default = "default_item_stack")]
    stack_size: u32,
}

fn default_item_stack() -> u32 {
    DEFAULT_MAX_STACK
}

#[cfg(test)]
mod item_registry_csv_tests {
    use super::*;

    #[test]
    fn from_csv_parses_items_stack_and_mass() {
        // Header order differs from the real file + carries extra columns: the
        // loader maps by header name and ignores the unknown ones.
        let csv = b"id,name,category,weight_kg,stack_size,content_class\n\
                    iron_ore_0,Iron Ore,material,2.5,50,ore\n\
                    sword_0,Iron Sword,tool,3.0,1,none\n";
        let reg = ItemRegistry::from_csv(csv).expect("parse");
        assert_eq!(reg.items.len(), 2);
        let ore = reg.items.get("iron_ore_0").expect("ore present");
        assert_eq!(ore.name, "Iron Ore");
        assert_eq!(ore.max_stack, 50);
        assert!(ore.stackable);
        assert!((reg.mass_for("iron_ore_0") - 2.5).abs() < 1e-6);
        let sword = reg.items.get("sword_0").expect("sword present");
        assert_eq!(sword.max_stack, 1);
        assert!(!sword.stackable, "stack_size 1 is not stackable");
    }

    #[test]
    fn from_csv_parses_volume_and_defaults_to_zero() {
        // Stage A (v0.726): volume_l is parsed when present; absent column
        // (older/modded CSVs) defaults to 0 = no volume gate.
        let with = b"id,name,weight_kg,stack_size,volume_l\niron_ore_0,Iron Ore,4.5,20,0.95\n";
        let reg = ItemRegistry::from_csv(with).expect("parse");
        assert!((reg.volume_for("iron_ore_0") - 0.95).abs() < 1e-6);
        let without = b"id,name,weight_kg,stack_size\niron_ore_0,Iron Ore,4.5,20\n";
        let reg2 = ItemRegistry::from_csv(without).expect("parse");
        assert_eq!(reg2.volume_for("iron_ore_0"), 0.0);
        assert_eq!(reg2.volume_for("nonexistent"), 0.0);
    }
}

#[cfg(test)]
mod volume_gate_tests {
    use super::*;

    /// Stage A slice 2 (v0.727): the volume gate caps an add at the remaining
    /// litres and reports the rest as overflow; the raw add_item stays
    /// ungated (bandolier-likes + save restore).
    #[test]
    fn volume_gate_caps_adds_and_reports_overflow() {
        let mut inv = Inventory::new(16);
        inv.volume_capacity_l = 10.0;
        // 2 L per unit -> only 5 fit by volume even though slots allow more.
        let overflow = inv.add_item_volume_gated("crate_0", 8, 99, 2.0);
        assert_eq!(overflow, 3, "3 of 8 must not fit in 10 L at 2 L each");
        assert_eq!(inv.count_item("crate_0"), 5);
        assert!((inv.volume_current_l - 10.0).abs() < 1e-6);
        // Full by volume: nothing more fits, even with free slots.
        let overflow2 = inv.add_item_volume_gated("crate_0", 1, 99, 2.0);
        assert_eq!(overflow2, 1);
        assert_eq!(inv.count_item("crate_0"), 5);
    }

    #[test]
    fn zero_volume_items_bypass_the_gate() {
        let mut inv = Inventory::new(4);
        inv.volume_capacity_l = 1.0;
        // unit_volume 0 (unknown/legacy or by-count holder) = pure slot logic.
        let overflow = inv.add_item_volume_gated("chip_0", 10, 10, 0.0);
        assert_eq!(overflow, 0);
        assert_eq!(inv.count_item("chip_0"), 10);
        assert_eq!(inv.volume_current_l, 0.0, "no volume accrues for 0-vol items");
    }

    #[test]
    fn slot_overflow_does_not_charge_volume() {
        // Slots bind before volume: only what actually lands charges litres.
        let mut inv = Inventory::new(1);
        inv.volume_capacity_l = 100.0;
        // One slot, stack of 5 -> only 5 of 9 land; volume charged for 5.
        let overflow = inv.add_item_volume_gated("box_0", 9, 5, 1.0);
        assert_eq!(overflow, 4);
        assert_eq!(inv.count_item("box_0"), 5);
        assert!((inv.volume_current_l - 5.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod transfer_tests {
    use super::*;
    use crate::ecs::components::Controllable;
    use crate::ecs::systems::System;
    use crate::hot_reload::data_store::DataStore;

    /// Backpack <-> container transfer: the GUI pushes (item_id, qty, is_add) into the
    /// inventory_transfer_ops channel; InventorySystem applies them to the player's
    /// backpack. is_add=true adds (container -> backpack), false removes (backpack ->
    /// container).
    #[test]
    fn transfer_channel_adds_and_removes_from_backpack() {
        let mut data = DataStore::new();
        let reg =
            ItemRegistry::from_csv(b"id,name,weight_kg,stack_size\niron_ore_0,Iron Ore,2.5,50\n")
                .expect("registry");
        data.insert("item_registry", reg);
        data.insert(
            "inventory_transfer_ops",
            std::sync::Mutex::new(Vec::<(String, u32, bool)>::new()),
        );

        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        let mut sys = InventorySystem::new();

        let push = |data: &DataStore, op: (String, u32, bool)| {
            data.get::<std::sync::Mutex<Vec<(String, u32, bool)>>>("inventory_transfer_ops")
                .unwrap()
                .lock()
                .unwrap()
                .push(op);
        };

        // Container -> backpack: add 5.
        push(&data, ("iron_ore_0".into(), 5, true));
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"),
            5,
            "transfer add put 5 iron_ore into the backpack"
        );

        // Backpack -> container: remove 3.
        push(&data, ("iron_ore_0".into(), 3, false));
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"),
            2,
            "transfer remove took 3 back out, leaving 2"
        );

        // The channel is drained each tick (no double-apply).
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"),
            2,
            "ops are drained; a tick with no new ops changes nothing"
        );
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

        // Get item registry for stack size and mass lookups
        let registry = data.get::<ItemRegistry>("item_registry");

        // GUI-driven backpack transfers: move items in/out of the player's backpack to
        // the organize-layer container pool. Drain the channel + apply each op to the
        // Controllable (player) inventory. (item_id, quantity, is_add): is_add adds to
        // the backpack (container -> backpack); else removes (backpack -> container).
        if let Some(slot) = data
            .get::<std::sync::Mutex<Vec<(String, u32, bool)>>>("inventory_transfer_ops")
        {
            let xfers: Vec<(String, u32, bool)> =
                slot.lock().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default();
            if !xfers.is_empty() {
                if let Some((_e, (inv, _))) = world
                    .query_mut::<(&mut Inventory, &crate::ecs::components::Controllable)>()
                    .into_iter()
                    .next()
                {
                    for (item_id, quantity, is_add) in xfers {
                        if is_add {
                            let max_stack =
                                registry.map(|r| r.max_stack_for(&item_id)).unwrap_or(DEFAULT_MAX_STACK);
                            // Volume-gated (Stage A slice 2): a full backpack
                            // refuses the transfer instead of over-filling.
                            let unit_vol = registry.map(|r| r.volume_for(&item_id)).unwrap_or(0.0);
                            inv.add_item_volume_gated(&item_id, quantity, max_stack, unit_vol);
                        } else {
                            inv.remove_item(&item_id, quantity);
                        }
                    }
                }
            }
        }

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

        // Recalculate total weight, volume and encumbrance for all inventories
        for (_entity, inventory) in world.query_mut::<&mut Inventory>() {
            let mut total_weight: f32 = 0.0;
            let mut total_volume: f32 = 0.0;
            for slot in &inventory.slots {
                if let Some(stack) = slot {
                    let unit_mass = registry
                        .map(|r| r.mass_for(&stack.item_id))
                        .unwrap_or(0.0);
                    total_weight += unit_mass * stack.quantity as f32;
                    // Storage volume (material-storage Stage A, v0.726).
                    let unit_vol = registry
                        .map(|r| r.volume_for(&stack.item_id))
                        .unwrap_or(0.0);
                    total_volume += unit_vol * stack.quantity as f32;
                }
            }
            inventory.weight_current = total_weight;
            inventory.volume_current_l = total_volume;

            let was_encumbered = inventory.encumbered;
            inventory.encumbered = total_weight > inventory.weight_capacity;

            // Log encumbrance state transitions
            if inventory.encumbered && !was_encumbered {
                log::debug!(
                    "Encumbered! {:.1}/{:.1} kg",
                    total_weight,
                    inventory.weight_capacity
                );
            } else if !inventory.encumbered && was_encumbered {
                log::debug!(
                    "No longer encumbered: {:.1}/{:.1} kg",
                    total_weight,
                    inventory.weight_capacity
                );
            }
        }
    }
}
