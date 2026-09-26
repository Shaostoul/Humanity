//! Fluids are litres (2026-09-26; the containers arc, increment 3d,
//! docs/design/containers.md). Until this, the home's water lived in two
//! worlds that never met: the plumbing sim's tanks held real litres, and the
//! water items (a bottle, a litre of purified water) came from recipes and
//! vendors with no tank behind them. Now:
//!
//! - a recipe that needs a measure of tap water (`tap` in
//!   `data/containers/fluids.ron`) draws it from the home tanks when the
//!   backpack has none, the way a cook uses the tap;
//! - a vessel you carry (`fillables`: a bottle, a jerrycan) is filled at a
//!   water tank's walk-up card and poured back there;
//! - drinking from a vessel hands the empty vessel back.
//!
//! Each vessel holds ONE fluid, as in real life (a water jerrycan is never a
//! fuel can), so a vessel is two items, empty and full, rather than an item
//! that remembers a fluid. Tanks hold water only today; `fluid` is on the
//! rows so fuel and milk vessels can join when their tanks hold litres.

use std::collections::HashMap;

use serde::Deserialize;

use crate::ecs::components::{MachineInstanceId, WaterTank};
use crate::systems::inventory::{Inventory, ItemRegistry};

/// One carryable vessel: the empty item, the same vessel full, what it holds
/// and how much.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Fillable {
    pub empty: String,
    pub full: String,
    pub fluid: String,
    pub litres: f32,
}

/// `data/containers/fluids.ron`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FluidTable {
    /// Items that ARE a measure of tap water: anything that needs one can
    /// draw it from the home tanks. Item id to litres per unit.
    #[serde(default)]
    pub tap: HashMap<String, f32>,
    /// Vessels the player fills at a tank, pours back and drinks from.
    #[serde(default)]
    pub fillables: Vec<Fillable>,
}

impl FluidTable {
    pub const FILE: &'static str = "containers/fluids.ron";

    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        ron::from_str(s).map_err(|e| format!("{}: {e}", Self::FILE))
    }

    /// Litres one unit of `item` is, when it is a measure of tap water.
    pub fn tap_litres(&self, item: &str) -> Option<f32> {
        self.tap.get(item).copied().filter(|l| *l > 0.0)
    }

    /// The empty vessel a full one leaves behind when it is drunk or poured.
    pub fn empty_of(&self, full: &str) -> Option<&str> {
        self.fillables.iter().find(|f| f.full == full).map(|f| f.empty.as_str())
    }
}

/// Litres of water in every tank in the world (the home's plumbing).
pub fn tank_litres(world: &hecs::World) -> f32 {
    world.query::<&WaterTank>().iter().map(|(_, t)| t.liters.max(0.0)).sum()
}

/// Take up to `litres` out of the tanks, fullest first. Returns what came out.
pub fn draw_from_tanks(world: &mut hecs::World, litres: f32) -> f32 {
    let mut tanks: Vec<(hecs::Entity, f32)> =
        world.query::<&WaterTank>().iter().map(|(e, t)| (e, t.liters)).collect();
    tanks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut want = litres.max(0.0);
    for (e, _) in tanks {
        if want <= 0.0 {
            break;
        }
        if let Ok(mut t) = world.get::<&mut WaterTank>(e) {
            let take = want.min(t.liters.max(0.0));
            t.liters -= take;
            want -= take;
        }
    }
    litres.max(0.0) - want
}

/// How many whole units of the tap item `item` the tanks can supply now.
pub fn tap_units(table: &FluidTable, world: &hecs::World, item: &str) -> u32 {
    match table.tap_litres(item) {
        Some(l) => (tank_litres(world) / l + 1e-4).floor() as u32,
        None => 0,
    }
}

/// One fill-or-pour choice on a water tank's walk-up card.
#[derive(Debug, Clone, PartialEq)]
pub struct FluidAction {
    /// "fill:<empty id>" or "pour:<full id>".
    pub key: String,
    pub label: String,
    pub tip: String,
}

fn carried(world: &hecs::World, item: &str) -> u32 {
    world
        .query::<(&Inventory, &crate::ecs::components::Controllable)>()
        .iter()
        .next()
        .map(|(_, (inv, _))| inv.count_item(item))
        .unwrap_or(0)
}

fn tank_of(world: &hecs::World, machine_id: &str) -> Option<(hecs::Entity, WaterTank)> {
    world
        .query::<(&WaterTank, &MachineInstanceId)>()
        .iter()
        .find(|(_, (_, id))| id.0 == machine_id)
        .map(|(e, (t, _))| (e, t.clone()))
}

fn name_of(reg: Option<&ItemRegistry>, id: &str) -> String {
    reg.and_then(|r| r.items.get(id).map(|d| d.name.clone())).unwrap_or_else(|| id.to_string())
}

/// The vessel's plain name ("Water Bottle"), without the empty item's suffix.
fn vessel_name(reg: Option<&ItemRegistry>, f: &Fillable) -> String {
    let n = name_of(reg, &f.empty);
    n.strip_suffix(" (empty)").map(str::to_string).unwrap_or(n)
}

/// What the player could fill from, or pour into, the water tank `machine_id`
/// with the vessels they carry. Empty when it is not a water tank or they
/// carry no water vessel.
pub fn card_actions(
    table: &FluidTable,
    world: &hecs::World,
    reg: Option<&ItemRegistry>,
    machine_id: &str,
) -> Vec<FluidAction> {
    let Some((_, tank)) = tank_of(world, machine_id) else { return Vec::new() };
    let mut out = Vec::new();
    for f in table.fillables.iter().filter(|f| f.fluid == "water") {
        let n = carried(world, &f.empty).min((tank.liters / f.litres + 1e-4).floor().max(0.0) as u32);
        if n > 0 {
            out.push(FluidAction {
                key: format!("fill:{}", f.empty),
                label: format!("Fill {n} x {} ({:.1} L)", vessel_name(reg, f), n as f32 * f.litres),
                tip: format!("Fill the empty {} you carry from this tank.", vessel_name(reg, f)),
            });
        }
        let room = (tank.capacity_l - tank.liters).max(0.0);
        let m = carried(world, &f.full).min((room / f.litres + 1e-4).floor() as u32);
        if m > 0 {
            out.push(FluidAction {
                key: format!("pour:{}", f.full),
                label: format!("Pour back {m} x {} ({:.1} L)", vessel_name(reg, f), m as f32 * f.litres),
                tip: "Empty them into this tank; you keep the empty vessels.".to_string(),
            });
        }
    }
    out
}

/// Carry out one card action on the water tank `machine_id`. Ok is the
/// notice to show; Err says why nothing happened.
pub fn apply_action(
    table: &FluidTable,
    world: &mut hecs::World,
    reg: Option<&ItemRegistry>,
    machine_id: &str,
    key: &str,
) -> Result<String, String> {
    let (verb, item) = key.split_once(':').ok_or_else(|| format!("unknown action {key}"))?;
    let f = table
        .fillables
        .iter()
        .find(|f| (verb == "fill" && f.empty == item) || (verb == "pour" && f.full == item))
        .cloned()
        .ok_or_else(|| format!("{item} is not a water vessel"))?;
    let (tank_e, tank) = tank_of(world, machine_id).ok_or("that is not a water tank")?;
    let (take, give, n) = if verb == "fill" {
        let n = carried(world, &f.empty).min((tank.liters / f.litres + 1e-4).floor().max(0.0) as u32);
        (f.empty.clone(), f.full.clone(), n)
    } else {
        let room = (tank.capacity_l - tank.liters).max(0.0);
        let n = carried(world, &f.full).min((room / f.litres + 1e-4).floor() as u32);
        (f.full.clone(), f.empty.clone(), n)
    };
    if n == 0 {
        return Err(if verb == "fill" {
            "The tank does not have enough water.".to_string()
        } else {
            "The tank is full.".to_string()
        });
    }
    let max_stack = reg.map(|r| r.max_stack_for(&give)).unwrap_or(99);
    let unit_vol = reg.map(|r| r.volume_for(&give)).unwrap_or(0.0);
    let take_vol = reg.map(|r| r.volume_for(&take)).unwrap_or(0.0);
    let take_stack = reg.map(|r| r.max_stack_for(&take)).unwrap_or(99);
    let mut done = 0u32;
    for (_e, (inv, _c)) in world.query_mut::<(&mut Inventory, &crate::ecs::components::Controllable)>() {
        // A swap, not an add (2026-09-26): the vessels going out free their
        // volume before the ones coming in are counted, so a pack with room
        // for the swap is not refused.
        inv.remove_item(&take, n);
        inv.volume_current_l = (inv.volume_current_l - n as f32 * take_vol).max(0.0);
        let lost = inv.add_item_volume_gated(&give, n, max_stack, unit_vol);
        if lost > 0 {
            // Whatever did not fit goes back the way it was, with a slot made
            // for it so nothing is dropped.
            let occupied = inv.slots.iter().filter(|s| s.is_some()).count();
            inv.ensure_slots(occupied + lost as usize);
            inv.add_item(&take, lost, take_stack);
            inv.volume_current_l += lost as f32 * take_vol;
        }
        done = n - lost;
        break;
    }
    if done == 0 {
        return Err("No room in your backpack.".to_string());
    }
    let litres = done as f32 * f.litres;
    if let Ok(mut t) = world.get::<&mut WaterTank>(tank_e) {
        if verb == "fill" {
            t.liters = (t.liters - litres).max(0.0);
        } else {
            t.liters = (t.liters + litres).min(t.capacity_l);
        }
    }
    Ok(if verb == "fill" {
        format!("Filled {done} x {} ({litres:.1} L).", vessel_name(reg, &f))
    } else {
        format!("Poured {litres:.1} L back into the tank.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::Controllable;

    fn table() -> FluidTable {
        let root = env!("CARGO_MANIFEST_DIR");
        let bytes = std::fs::read(format!("{root}/data/{}", FluidTable::FILE)).expect("fluids.ron");
        FluidTable::from_ron(&bytes).expect("parse fluids.ron")
    }

    fn registry() -> ItemRegistry {
        let root = env!("CARGO_MANIFEST_DIR");
        ItemRegistry::from_csv(&std::fs::read(format!("{root}/data/items.csv")).unwrap()).unwrap()
    }

    fn world_with(tank_l: f32, cap: f32, pack: &[(&str, u32)]) -> hecs::World {
        let mut w = hecs::World::new();
        w.spawn((WaterTank { liters: tank_l, capacity_l: cap }, MachineInstanceId("cistern".into())));
        let mut inv = Inventory::new(36);
        for (id, q) in pack {
            inv.add_item(id, *q, 99);
        }
        w.spawn((inv, Controllable));
        w
    }

    fn pack(w: &hecs::World, id: &str) -> u32 {
        carried(w, id)
    }

    /// Every vessel in fluids.ron is two real items of the same outer volume,
    /// the full one weighing its empty weight plus its water, and filed under
    /// the water content class; every tap item is real.
    #[test]
    fn every_vessel_is_a_real_empty_and_full_pair() {
        let t = table();
        let reg = registry();
        assert!(!t.fillables.is_empty() && !t.tap.is_empty());
        for f in &t.fillables {
            let e = reg.items.get(&f.empty).unwrap_or_else(|| panic!("{} not in items.csv", f.empty));
            let full = reg.items.get(&f.full).unwrap_or_else(|| panic!("{} not in items.csv", f.full));
            assert!((e.volume_l - full.volume_l).abs() < 1e-3, "{}: a vessel is the same size full or empty", f.full);
            let want = e.mass_kg + f.litres; // water is 1 kg per litre
            assert!((full.mass_kg - want).abs() <= 0.1 * want.max(1.0), "{}: weighs {} kg, expected about {want}", f.full, full.mass_kg);
            assert_eq!(full.content_class, f.fluid, "{} holds {}", f.full, f.fluid);
        }
        for id in t.tap.keys() {
            assert!(reg.items.contains_key(id), "tap item {id} not in items.csv");
        }
    }

    #[test]
    fn the_tanks_give_what_they_have_fullest_first() {
        let mut w = hecs::World::new();
        let a = w.spawn((WaterTank { liters: 10.0, capacity_l: 100.0 },));
        let b = w.spawn((WaterTank { liters: 3.0, capacity_l: 100.0 },));
        assert_eq!(draw_from_tanks(&mut w, 8.0), 8.0);
        assert_eq!(w.get::<&WaterTank>(a).unwrap().liters, 2.0, "fullest first");
        assert_eq!(w.get::<&WaterTank>(b).unwrap().liters, 3.0);
        assert_eq!(draw_from_tanks(&mut w, 100.0), 5.0, "no more than there is");
        assert_eq!(tank_litres(&w), 0.0);
    }

    /// Fill the bottles you carry at the cistern, then pour them back: the
    /// litres leave the tank and come back, and the bottles change state.
    #[test]
    fn fill_at_a_tank_and_pour_back() {
        let t = table();
        let reg = registry();
        let mut w = world_with(10.0, 20.0, &[("water_bottle_empty_0", 3)]);
        let acts = card_actions(&t, &w, Some(&reg), "cistern");
        let fill = acts.iter().find(|a| a.key == "fill:water_bottle_empty_0").expect("a fill action");
        assert_eq!(fill.label, "Fill 3 x Water Bottle (1.5 L)");
        apply_action(&t, &mut w, Some(&reg), "cistern", &fill.key).unwrap();
        assert_eq!(pack(&w, "water_bottle_0"), 3);
        assert_eq!(pack(&w, "water_bottle_empty_0"), 0);
        assert!((tank_litres(&w) - 8.5).abs() < 1e-4, "{}", tank_litres(&w));

        apply_action(&t, &mut w, Some(&reg), "cistern", "pour:water_bottle_0").unwrap();
        assert_eq!(pack(&w, "water_bottle_empty_0"), 3);
        assert!((tank_litres(&w) - 10.0).abs() < 1e-4);

        // Not a water tank: no actions. An empty tank: no fill.
        assert!(card_actions(&t, &w, Some(&reg), "not-a-tank").is_empty());
        let mut dry = world_with(0.2, 20.0, &[("water_bottle_empty_0", 2)]);
        assert!(card_actions(&t, &dry, Some(&reg), "cistern").is_empty());
        assert!(apply_action(&t, &mut dry, Some(&reg), "cistern", "fill:water_bottle_empty_0").is_err());
    }

    /// Filling a jerrycan is a swap: a pack with room for it is not refused
    /// because the empty can's own volume was counted twice (2026-09-26).
    #[test]
    fn filling_a_vessel_in_a_nearly_full_pack_works() {
        let t = table();
        let reg = registry();
        let mut w = world_with(100.0, 200.0, &[("water_jerrycan_empty_0", 1)]);
        for (_e, (inv, _c)) in w.query_mut::<(&mut Inventory, &Controllable)>() {
            inv.volume_current_l = inv.volume_capacity_l - 16.5;
        }
        apply_action(&t, &mut w, Some(&reg), "cistern", "fill:water_jerrycan_empty_0").expect("the swap fits");
        assert_eq!(pack(&w, "water_jerrycan_0"), 1);
        assert_eq!(pack(&w, "water_jerrycan_empty_0"), 0);
    }

    /// A full tank takes nothing back: the pour is refused and nothing moves.
    #[test]
    fn a_full_tank_takes_nothing_back() {
        let t = table();
        let reg = registry();
        let mut w = world_with(20.0, 20.0, &[("water_jerrycan_0", 1)]);
        assert!(apply_action(&t, &mut w, Some(&reg), "cistern", "pour:water_jerrycan_0").is_err());
        assert_eq!(pack(&w, "water_jerrycan_0"), 1);
        assert_eq!(tank_litres(&w), 20.0);
    }
}
