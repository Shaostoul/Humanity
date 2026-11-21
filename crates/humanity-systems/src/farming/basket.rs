//! The harvest basket: produce picked toward the next whole item (2026-11-21).
//!
//! WHY. plants.csv gives each plant's season yield in ITEMS, and many crops
//! give far less than one item a plant: one radish tower cup yields 0.0079 to
//! 0.0273 of a Radish (2.4 to 8.2 g) while the Radish item is a 300 g bunch
//! (items.csv). Until this rung a harvest turned its fractional yield into a
//! whole count by probabilistic rounding (floor, plus one more with the
//! fraction's probability), which kept the AVERAGE right but meant a radish
//! cup handed over a bunch in about 1.6% of harvests and nothing in the rest:
//! a player who grew and harvested radish got nothing (found by the survival
//! quests agent, 2026-11-21). The data stays true (no inflated yields, no
//! item shrunk to hide it); instead every gram a harvest yields is kept and
//! added to the next harvest of the same produce, so the grams add up to whole
//! items the way a gardener's basket fills.
//!
//! THE RULE. A harvest adds what it picked (fractional items, `add`) to what
//! the basket already holds of that produce; the whole items that makes come
//! out into the pack, and the rest (always under one item) stays for next
//! time. Nothing is rounded and nothing is lost: over any run of harvests the
//! whole items handed over plus what the basket holds equal exactly what was
//! picked. A picked crop (a tomato, picking.rs) adds each pick the same way,
//! so its picks and a once harvest share one rule.
//!
//! WHERE IT LIVES: on the PLAYER, the `Controllable` entity whose pack the
//! harvest fills, one remainder per harvest ITEM (not per plant or per grow
//! area). Why per player: the remainder is produce already picked, so it has
//! left the plant and belongs to whoever picked it, not to the bed it grew
//! in; two radish beds fill one basket, the way a gardener working several
//! beds fills one; keyed by grow area, grams would sit stranded in a bed that
//! is torn down or resown with another crop, and the same grams would make
//! items more slowly. Why per item: the remainder is a part of one item, so
//! two plants that harvest into the same item share it. On a server world each
//! player's save is their own (a fresh start per server), so the basket is too.
//!
//! SAVED in `WorldSave::harvest_basket` (the engine's save_load.rs writes it
//! with `of_player` and puts it back with `restore`), so a restart keeps the
//! grams already picked.
//!
//! The report: every harvest press says, per produce, what it picked and what
//! is now kept toward the next whole one ("Picked 4.2 g of radish, short of a
//! whole one (300 g): 112 g is kept toward it"), `report_line`.

use std::collections::BTreeMap;

use humanity_core::ecs::components::Controllable;
use serde::{Deserialize, Serialize};

use crate::inventory::Inventory;

/// A remainder this close to a whole item is the whole item: float sums of
/// fractions (0.1 ten times is 0.9999999999999999) must not leave a basket
/// showing "300 g of 300 g". 1e-9 of a 300 g item is 0.3 micrograms.
const WHOLE_EPS: f64 = 1e-9;

/// What the player has picked toward the next whole item of each produce
/// (the module doc). A component on the `Controllable` entity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HarvestBasket {
    /// Part of one whole item kept, per harvest item id ("vegetable_radish_0"),
    /// always in (0, 1). An item with nothing kept has no entry.
    #[serde(default)]
    pub toward: BTreeMap<String, f64>,
}

impl HarvestBasket {
    /// Put `got` items of `item` (fractional, as harvested) in with what is
    /// kept of it, and return the whole items that makes; the rest is kept.
    /// A negative or non-finite `got` adds nothing.
    pub fn add(&mut self, item: &str, got: f64) -> u32 {
        let got = if got.is_finite() { got.max(0.0) } else { 0.0 };
        let held = self.toward(item) + got;
        let mut whole = held.floor();
        let mut rest = held - whole;
        if rest > 1.0 - WHOLE_EPS {
            whole += 1.0;
            rest = 0.0;
        }
        if rest > WHOLE_EPS {
            self.toward.insert(item.to_string(), rest);
        } else {
            self.toward.remove(item);
        }
        whole.min(f64::from(u32::MAX)) as u32
    }

    /// The part of one whole `item` kept, 0 when nothing is.
    pub fn toward(&self, item: &str) -> f64 {
        self.toward.get(item).copied().unwrap_or(0.0)
    }
}

/// Give every player (a `Controllable` with an `Inventory`, the entity a
/// harvest fills) a basket if they have none yet: a fresh player is spawned
/// without one (the engine's spawn sites), and the harvest wants one to add to.
pub fn ensure(world: &mut hecs::World) {
    let missing: Vec<hecs::Entity> = world
        .query::<hecs::Without<(&Inventory, &Controllable), &HarvestBasket>>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in missing {
        let _ = world.insert_one(e, HarvestBasket::default());
    }
}

/// The player's basket for the save (an empty one when the player has none).
pub fn of_player(world: &hecs::World) -> HarvestBasket {
    world
        .query::<(&HarvestBasket, &Controllable)>()
        .iter()
        .next()
        .map(|(_, (b, _))| b.clone())
        .unwrap_or_default()
}

/// Put a saved basket on the player. Authoritative, like the pack it travels
/// with: the save's basket replaces whatever the player held (an empty one
/// empties it), so a load never mixes two worlds' grams.
pub fn restore(world: &mut hecs::World, saved: &HarvestBasket) {
    let players: Vec<hecs::Entity> =
        world.query::<(&Inventory, &Controllable)>().iter().map(|(e, _)| e).collect();
    if let Some(e) = players.into_iter().next() {
        let _ = world.insert_one(e, saved.clone());
    }
}

/// An amount of produce as people say it: grams of it when its item has a
/// mass ("4.2 g", "112 g", "1.3 kg", "14 mg" for saffron), else the share of
/// one item ("2%").
pub fn amount_words(items: f64, kg_each: f64) -> String {
    if !(kg_each > 0.0) {
        return format!("{:.0}%", items * 100.0);
    }
    let g = items * kg_each * 1000.0;
    if g >= 1000.0 {
        format!("{:.1} kg", g / 1000.0)
    } else if g >= 10.0 {
        format!("{g:.0} g")
    } else if g >= 1.0 {
        format!("{g:.1} g")
    } else {
        format!("{:.0} mg", g * 1000.0)
    }
}

/// The line a harvest press says for one produce: what it picked (`got`
/// items), the whole items that came out (`whole`), and what is now kept
/// toward the next (`toward`, a part of one item). `name` is the produce's
/// name as a person reads it, `kg_each` one item's mass (items.csv).
pub fn report_line(name: &str, kg_each: f64, got: f64, whole: u32, toward: f64) -> String {
    let fill = |key: &'static str| {
        crate::crafting::home_store::words(key)
            .replace("{{got}}", &amount_words(got, kg_each))
            .replace("{{name}}", name)
            .replace("{{n}}", &whole.to_string())
            .replace("{{whole}}", &amount_words(1.0, kg_each))
            .replace("{{toward}}", &amount_words(toward, kg_each))
    };
    if whole == 0 {
        fill("farming.harvest_part")
    } else if toward > WHOLE_EPS {
        fill("farming.harvest_whole")
    } else {
        fill("farming.harvest_whole_exact")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE BASKET'S ARITHMETIC, on its own: a harvest under one item makes no
    /// whole item and is kept; the item comes out on the harvest that brings
    /// the kept part to one, with the excess kept; a harvest of several items
    /// gives the whole ones and keeps the fraction; produce is kept apart by
    /// item; nothing is lost on the way (whole + kept = picked). Exact binary
    /// fractions, so every comparison is exact.
    #[test]
    fn the_basket_keeps_the_fraction_and_gives_whole_items() {
        let mut b = HarvestBasket::default();
        assert_eq!(b.add("radish", 0.375), 0, "under one item: nothing whole yet");
        assert_eq!(b.toward("radish"), 0.375, "and the part is kept");
        assert_eq!(b.add("radish", 0.375), 0);
        assert_eq!(b.toward("radish"), 0.75);
        assert_eq!(b.add("radish", 0.5), 1, "the harvest that brings it to one gives the item");
        assert_eq!(b.toward("radish"), 0.25, "and keeps the excess");
        assert_eq!(b.add("lettuce", 15.25), 15, "a big harvest gives its whole items");
        assert_eq!(b.toward("lettuce"), 0.25, "and keeps its fraction");
        assert_eq!(b.toward("radish"), 0.25, "each produce keeps its own");
        assert_eq!(b.add("radish", 0.75), 1, "exactly one: the item, nothing kept");
        assert!(!b.toward.contains_key("radish"), "an emptied entry is gone, not 0");
        assert_eq!(b.add("radish", -3.0), 0, "a negative take adds nothing");
        assert_eq!(b.add("radish", f64::NAN), 0, "nor does a NaN");
        assert!(b.toward.get("radish").is_none());
        // Ten tenths make one, despite 0.1's binary rounding.
        let mut c = HarvestBasket::default();
        let whole: u32 = (0..10).map(|_| c.add("x", 0.1)).sum();
        assert_eq!(whole, 1, "ten tenths make one whole item");
        assert_eq!(c.toward("x"), 0.0, "with nothing left over");
    }

    #[test]
    fn amounts_read_as_grams() {
        assert_eq!(amount_words(0.014, 0.3), "4.2 g");
        assert_eq!(amount_words(0.3733, 0.3), "112 g");
        assert_eq!(amount_words(1.0, 0.3), "300 g");
        assert_eq!(amount_words(15.19, 0.45), "6.8 kg");
        assert_eq!(amount_words(0.00028, 0.05), "14 mg");
        assert_eq!(amount_words(0.5, 0.0), "50%", "no mass: the share of one item");
    }
}
