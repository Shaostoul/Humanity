//! What the home's machines HOLD, kept across a restart and across world
//! entry (2026-09-27): each battery bank's charge, each water tank's litres,
//! and the contents of each machine's vessel (a genset's fuel drum, the
//! refinery's drum, the grain silo, the pantry, the freezer, the furniture
//! drawers).
//!
//! Before this none of it was saved. Every launch started each bank at its
//! spawn charge (half full, `home_spawn::spawn_home_machine_entity`) and each
//! tank at half, which undid the night's discharge or the day's charge; and
//! every vessel came back EMPTY, so whatever the player had stored in one (a
//! drum of refined fuel, a silo of grain, a stocked pantry) was destroyed.
//! Entering the world did the same within a session: `load_world` despawns
//! every home machine entity and spawns it afresh.
//!
//! Keyed by the machine's instance id in the home layout (`MachineInstanceId`,
//! "battery_bank_0"), because the entity changes at every spawn. What comes
//! from the catalog stays the catalog's (a bank's capacity, a vessel's type
//! and size); only the level is restored, clamped to what the machine holds.
//!
//! TWO SPAWNS. At startup the menu-mode entities (`spawn_home_power_entities`)
//! exist before the save is applied, so a bank and a tank take their level at
//! once; but they carry no vessel (the container registry is not threaded
//! there). A saved vessel's contents are therefore HELD on a world entity
//! (`HeldMachineLevels`) until world entry spawns the vessel, and a save
//! written before then keeps them (`levels` merges them in) rather than
//! forgetting them. World entry carries every level from the old entities to
//! the new ones (`take_all`, then `apply`).
//!
//! OFFLINE (docs/design/offline-progression.md): none of these levels moves on
//! by the time away. A bank neither charges nor runs flat while the player is
//! out, a tank does not refill, and a drum is not burned: they come back as
//! saved. The machines that run offline draw tap water from what the tanks
//! held, which is now the saved level rather than the spawn half.

use serde::{Deserialize, Serialize};

use crate::ecs::components::{Battery, MachineInstanceId, WaterTank};
use crate::systems::inventory::containers::Container;

/// One home machine's stored levels. A field is None when the machine has no
/// such store (a tank has no charge).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineLevels {
    /// The machine's instance id in the home layout.
    pub id: String,
    /// A battery bank's charge, watt-hours.
    #[serde(default)]
    pub charge_wh: Option<f32>,
    /// A water tank's level, litres.
    #[serde(default)]
    pub water_l: Option<f32>,
    /// A vessel's contents and what it remembers (residue, toxic history,
    /// damage). Its type and size come back from the catalog.
    #[serde(default)]
    pub vessel: Option<Container>,
}

impl MachineLevels {
    fn is_empty(&self) -> bool {
        self.charge_wh.is_none() && self.water_l.is_none() && self.vessel.is_none()
    }
}

/// Saved levels no live machine could take yet (the menu-mode entities carry
/// no vessel). One per world at most.
#[derive(Debug, Clone, Default)]
pub struct HeldMachineLevels(pub Vec<MachineLevels>);

/// The levels the live machines hold now, sorted by id.
fn live(world: &hecs::World) -> Vec<MachineLevels> {
    let mut out: Vec<MachineLevels> = world
        .query::<(&MachineInstanceId, Option<&Battery>, Option<&WaterTank>, Option<&Container>)>()
        .iter()
        .map(|(_e, (id, b, t, c))| MachineLevels {
            id: id.0.clone(),
            charge_wh: b.map(|b| b.charge_wh),
            water_l: t.map(|t| t.liters),
            vessel: c.cloned(),
        })
        .filter(|l| !l.is_empty())
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// What is held, if anything.
fn held(world: &hecs::World) -> Vec<MachineLevels> {
    world.query::<&HeldMachineLevels>().iter().flat_map(|(_e, h)| h.0.clone()).collect()
}

/// The live levels with the held ones filling what the live machines lack.
/// The held records are by construction the fields no live machine had, so
/// the two never disagree about one field.
fn merge(mut live: Vec<MachineLevels>, held: Vec<MachineLevels>) -> Vec<MachineLevels> {
    for h in held {
        match live.iter_mut().find(|l| l.id == h.id) {
            Some(l) => {
                l.charge_wh = l.charge_wh.or(h.charge_wh);
                l.water_l = l.water_l.or(h.water_l);
                if l.vessel.is_none() {
                    l.vessel = h.vessel;
                }
            }
            None => live.push(h),
        }
    }
    live.sort_by(|a, b| a.id.cmp(&b.id));
    live
}

/// Every machine's levels for the save: the live machines', and any saved
/// levels still held for world entry.
pub fn levels(world: &hecs::World) -> Vec<MachineLevels> {
    merge(live(world), held(world))
}

fn drop_held(world: &mut hecs::World) {
    let old: Vec<hecs::Entity> = world.query::<&HeldMachineLevels>().iter().map(|(e, _)| e).collect();
    for e in old {
        let _ = world.despawn(e);
    }
}

/// Put `saved` onto the live machines by instance id and return what did not
/// land: a record for a machine that is not there, or a field the machine
/// has no store for (yet). A vessel of another type than the one saved (the
/// catalog changed the machine's vessel) is not given the old contents, which
/// may not belong in it; that is logged and dropped.
pub fn apply(world: &mut hecs::World, saved: &[MachineLevels]) -> Vec<MachineLevels> {
    let mut left: Vec<MachineLevels> = saved.to_vec();
    for (_e, (id, b, t, c)) in world
        .query_mut::<(&MachineInstanceId, Option<&mut Battery>, Option<&mut WaterTank>, Option<&mut Container>)>()
    {
        let Some(rec) = left.iter_mut().find(|r| r.id == id.0) else { continue };
        if let (Some(b), Some(wh)) = (b, rec.charge_wh) {
            if wh.is_finite() {
                b.charge_wh = wh.clamp(0.0, b.capacity_wh);
            }
            rec.charge_wh = None;
        }
        if let (Some(t), Some(l)) = (t, rec.water_l) {
            if l.is_finite() {
                t.liters = l.clamp(0.0, t.capacity_l);
            }
            rec.water_l = None;
        }
        if let Some(c) = c {
            if let Some(v) = rec.vessel.take() {
                if v.container_type_id == c.container_type_id {
                    *c = Container { capacity_liters: c.capacity_liters, ..v };
                } else {
                    log::warn!(
                        "machine {}: saved {} contents not restored into its {} (the vessel type changed)",
                        id.0, v.container_type_id, c.container_type_id
                    );
                }
            }
        }
    }
    left.retain(|r| !r.is_empty());
    left
}

/// Apply a save's levels (`apply_save_to_world`, at startup and on character
/// select): what the live machines can take now, and the rest held for world
/// entry. Replaces anything held before, so re-applying a save lands in the
/// same place.
pub fn restore(world: &mut hecs::World, saved: &[MachineLevels]) {
    drop_held(world);
    let left = apply(world, saved);
    if !left.is_empty() {
        world.spawn((HeldMachineLevels(left),));
    }
}

/// Every level to carry onto the machines world entry is about to respawn:
/// the live machines' and the held ones, which it takes (`load_world`, before
/// it despawns the old machine entities; `apply` puts them back after).
pub fn take_all(world: &mut hecs::World) -> Vec<MachineLevels> {
    let all = levels(world);
    drop_held(world);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vessel(item: &str, qty: u32) -> Container {
        let mut c = Container::new("steel_fuel_drum", 200.0);
        c.current_content_item = Some(item.to_string());
        c.current_qty = qty;
        c.used_liters = qty as f32;
        c.last_content = Some(item.to_string());
        c
    }

    /// A level is clamped to what the machine holds, a non-finite one is
    /// ignored, and a record for a machine that is not there, or a field it
    /// has no store for, comes back as left over. Seen red by returning the
    /// saved list unchanged (the tank and the charge then showed as left).
    #[test]
    fn apply_clamps_and_returns_what_did_not_land() {
        let mut world = hecs::World::new();
        world.spawn((
            MachineInstanceId("bank".into()),
            Battery { charge_wh: 2000.0, capacity_wh: 4000.0, max_charge_w: 1.0, max_discharge_w: 1.0 },
        ));
        world.spawn((MachineInstanceId("tank".into()), WaterTank { liters: 500.0, capacity_l: 1000.0 }));
        let saved = vec![
            MachineLevels { id: "bank".into(), charge_wh: Some(9000.0), ..Default::default() },
            MachineLevels { id: "tank".into(), water_l: Some(f32::NAN), vessel: Some(vessel("fuel", 3)), ..Default::default() },
            MachineLevels { id: "gone".into(), water_l: Some(10.0), ..Default::default() },
        ];
        let left = apply(&mut world, &saved);
        let got: Vec<(String, Option<f32>, Option<f32>)> =
            live(&world).into_iter().map(|l| (l.id, l.charge_wh, l.water_l)).collect();
        assert_eq!(got, vec![("bank".into(), Some(4000.0), None), ("tank".into(), None, Some(500.0))]);
        assert_eq!(
            left,
            vec![
                MachineLevels { id: "tank".into(), vessel: Some(vessel("fuel", 3)), ..Default::default() },
                MachineLevels { id: "gone".into(), water_l: Some(10.0), ..Default::default() },
            ]
        );
    }

    /// A vessel of another type than the one saved is not handed the old
    /// contents (they may not belong in it). Seen red by dropping the type
    /// check (the silo then held fuel).
    #[test]
    fn a_changed_vessel_type_is_not_filled_with_the_old_contents() {
        let mut world = hecs::World::new();
        world.spawn((MachineInstanceId("silo".into()), Container::new("grain_silo_bin", 5000.0)));
        let saved = vec![MachineLevels { id: "silo".into(), vessel: Some(vessel("fuel", 3)), ..Default::default() }];
        assert!(apply(&mut world, &saved).is_empty(), "dropped, not held");
        let c = live(&world).remove(0).vessel.unwrap();
        assert!(c.is_empty(), "the silo stays empty: {c:?}");
    }
}
