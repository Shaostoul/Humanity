//! Mining system — autonomous drones that fly to asteroids, extract ore over time,
//! return home, and drop the raw material into the player's inventory.
//!
//! The operator's core acquisition loop: commission a drone for an ore → it spends
//! time travelling + mining a FINITE asteroid → returns the raw ore. When an asteroid
//! is fully consumed its entity is deleted. (The MMO swarm + abandoned-deletion is the
//! server-authoritative #5b follow-up; this is the single-player loop.)

use crate::ecs::components::{AsteroidBody, Controllable, Drone, DronePhase};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::{Inventory, ItemRegistry};

/// Mission phase durations (real seconds). Dev-scale; tune later / move to data.
const OUTBOUND_SECS: f32 = 5.0;
const MINING_SECS: f32 = 5.0;
const RETURNING_SECS: f32 = 5.0;
/// Total ore units a drone's hold carries per trip (a manifest's units sum to this).
/// Exposed so the Mining UI can cap the allocation.
pub const DRONE_CAPACITY: u32 = 10;

/// Real seconds the given mission phase lasts — exposed so the Mining UI can draw
/// a per-stage progress bar (the operator's "show the drone is working" cue).
pub fn phase_secs(phase: &DronePhase) -> f32 {
    match phase {
        DronePhase::Outbound => OUTBOUND_SECS,
        DronePhase::Mining => MINING_SECS,
        DronePhase::Returning => RETURNING_SECS,
        DronePhase::Done => 0.0,
    }
}

/// What a drone needs done to OTHER entities this tick — computed while iterating
/// drones (a `&mut Drone` query) and applied afterwards, so the cross-entity
/// `&mut World` borrows never overlap the drone query.
enum DroneIntent {
    /// Fill the drone's hold per its `manifest`, pulling each ore from its ONE target
    /// asteroid (bounded by what that asteroid holds; no cross-asteroid spillover).
    Mine {
        drone: hecs::Entity,
        target: String,
        manifest: Vec<(String, u32)>,
    },
    /// Deliver the drone's whole `cargo` into `home`'s inventory, then despawn it.
    Deliver {
        drone: hecs::Entity,
        home: u64,
        cargo: Vec<(String, u32)>,
    },
}

pub struct DroneSystem;

impl DroneSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DroneSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for DroneSystem {
    fn name(&self) -> &str {
        "DroneSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let item_registry = data.get::<ItemRegistry>("item_registry");

        // ── COMMISSION: drain the channel (the Mining panel writes a TARGET asteroid id
        //    + a manifest) and launch ONE drone — home = the player, target = that asteroid.
        let order: Option<(String, Vec<(String, u32)>)> = data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("commission_drone")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some((target, manifest)) = order {
            // ONE drone per player: skip a new launch if one is already in flight.
            let already_flying = world.query::<&Drone>().iter().next().is_some();
            let manifest: Vec<(String, u32)> =
                manifest.into_iter().filter(|(_, u)| *u > 0).collect();
            if already_flying {
                log::info!("[Mining] a drone is already in flight (one per player)");
            } else if manifest.is_empty() {
                log::info!("[Mining] empty manifest; drone not launched");
            } else {
                // Only launch if the TARGET asteroid still exists — a depleted/stale id
                // shouldn't burn the player's single drone slot on a dud trip. Its
                // position scales the travel time + the map dot.
                launch(world, data, &target, &manifest);
            }
        }

        // ── STANDING ORDER refire (economy automation Phase 1, v0.663): while a
        //    standing order exists and NOTHING is flying or queued, re-commission
        //    it. Living at tick level (not inside the Deliver arm) makes the loop
        //    self-healing: it resumes after a world reload that despawned an
        //    in-flight drone, not just after a clean delivery. Runs AFTER the
        //    drain above, so it takes effect next tick (no same-tick relaunch).
        {
            let any_drone = world.query::<&Drone>().iter().next().is_some();
            if !any_drone {
                let standing: Option<(String, Vec<(String, u32)>)> = data
                    .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>(
                        "auto_mine_order",
                    )
                    .and_then(|m| m.lock().ok().and_then(|s| s.clone()));
                if let Some(order) = standing {
                    if let Some(slot) = data
                        .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>(
                            "commission_drone",
                        )
                    {
                        if let Ok(mut s) = slot.lock() {
                            if s.is_none() {
                                log::info!("[Mining] standing order: re-commissioning {}", order.0);
                                *s = Some(order);
                            }
                        }
                    }
                }
            }
        }

        // ── ADVANCE: tick each drone's phase machine, recording cross-entity intents.
        // Phase timers run on GAME time (v0.663): accelerated testing speeds the
        // trips too, not just the clock. Absent game_time (unit tests) = raw dt.
        let sdt = crate::systems::time::scaled_dt(dt, data);
        let mut intents: Vec<DroneIntent> = Vec::new();
        for (entity, drone) in world.query_mut::<&mut Drone>() {
            drone.phase_time += sdt;
            let dur = drone.phase_duration(drone.phase);
            match drone.phase {
                DronePhase::Outbound if drone.phase_time >= dur => {
                    drone.phase = DronePhase::Mining;
                    drone.phase_time = 0.0;
                    intents.push(DroneIntent::Mine {
                        drone: entity,
                        target: drone.target.clone(),
                        manifest: drone.manifest.clone(),
                    });
                }
                DronePhase::Mining if drone.phase_time >= dur => {
                    drone.phase = DronePhase::Returning;
                    drone.phase_time = 0.0;
                }
                DronePhase::Returning if drone.phase_time >= dur => {
                    drone.phase = DronePhase::Done;
                    drone.phase_time = 0.0;
                    intents.push(DroneIntent::Deliver {
                        drone: entity,
                        home: drone.home,
                        cargo: drone.cargo.clone(),
                    });
                }
                _ => {}
            }
        }

        // ── APPLY: mutate the asteroid / home inventory / despawn the drone (the drone
        //    query borrow is released now, so these &mut World gets are conflict-free).
        for intent in intents {
            match intent {
                DroneIntent::Mine { drone, target, manifest } => {
                    let collected = mine(world, &target, &manifest);
                    if let Ok(mut d) = world.get::<&mut Drone>(drone) {
                        d.cargo = collected;
                    }
                }
                DroneIntent::Deliver { drone, home, cargo } => {
                    deliver_haul(world, data, item_registry, home, &cargo);
                    let _ = world.despawn(drone);
                    // (Standing-order relaunch happens at TICK level above, not
                    // here -- see the refire block after the commission drain.)
                }
            }
        }

        remove_mined_out(world);
    }
}

/// Launch the drone at `target` with `manifest`, home = the player, when that
/// asteroid still exists. A standing order aimed at an asteroid that is gone
/// (mined out and deleted) is ended here, or it would refire a dead
/// commission every trip forever (v0.663). Shared by the tick's commission
/// and the time away (`advance_away`). Returns whether a drone launched.
fn launch(world: &mut hecs::World, data: &DataStore, target: &str, manifest: &[(String, u32)]) -> bool {
    // Only launch if the TARGET asteroid still exists: a depleted or stale id
    // shouldn't burn the player's single drone slot on a dud trip. Its
    // position scales the travel time + the map dot.
    let target_pos = world
        .query::<&AsteroidBody>()
        .iter()
        .find(|(_, a)| a.id == target)
        .map(|(_, a)| a.position);
    let home: Option<u64> = world
        .query::<(&Inventory, &Controllable)>()
        .iter()
        .next()
        .map(|(e, _)| e.to_bits().into());
    match (target_pos, home) {
        (Some(target_pos), Some(home)) => {
            world.spawn((Drone {
                home,
                target: target.to_string(),
                manifest: manifest.to_vec(),
                phase: DronePhase::Outbound,
                phase_time: 0.0,
                cargo: Vec::new(),
                home_pos: [0.0, 0.0, 0.0],
                target_pos,
            },));
            log::info!("[Mining] commissioned a drone for {target}: {manifest:?}");
            true
        }
        (None, _) => {
            log::info!("[Mining] target asteroid '{target}' not found; not launching");
            if let Some(slot) =
                data.get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
            {
                if let Ok(mut s) = slot.lock() {
                    if s.as_ref().map_or(false, |(t, _)| *t == target) {
                        log::info!("[Mining] standing order for '{target}' ended (target gone)");
                        *s = None;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Pull each requested ore from the ONE target asteroid, bounded by what it
/// holds. No spillover to other asteroids: one run mines one asteroid, so the
/// haul is capped by that asteroid's stock.
fn mine(world: &mut hecs::World, target: &str, manifest: &[(String, u32)]) -> Vec<(String, u32)> {
    let target_e = world
        .query::<&AsteroidBody>()
        .iter()
        .find(|(_, a)| a.id == target)
        .map(|(e, _)| e);
    let mut collected: Vec<(String, u32)> = Vec::new();
    if let Some(aid) = target_e {
        for (ore, units) in manifest {
            if let Ok(mut body) = world.get::<&mut AsteroidBody>(aid) {
                let took = body.take(ore, *units as f32);
                if took > 0 {
                    collected.push((ore.clone(), took));
                }
            }
        }
    }
    log::info!("[Mining] drone extracted {collected:?} from {target}");
    collected
}

/// Land a haul in the home inventory (`home` = the player's entity bits) and
/// train Mining for it. Returns the units delivered.
fn deliver_haul(
    world: &mut hecs::World,
    data: &DataStore,
    item_registry: Option<&ItemRegistry>,
    home: u64,
    cargo: &[(String, u32)],
) -> u32 {
    let Some(home_e) = hecs::Entity::from_bits(home) else { return 0 };
    let mut total = 0u32;
    for (ore, qty) in cargo {
        if *qty == 0 {
            continue;
        }
        let max_stack = item_registry.map(|r| r.max_stack_for(ore)).unwrap_or(99);
        if let Ok(mut inv) = world.get::<&mut Inventory>(home_e) {
            // Deliberately NOT volume-gated (Stage A slice 2): the operator
            // ruling below (never vanish a haul) predates and outranks the
            // volume gate here; the home stock behaves as base storage.
            // Revisit when home storage gets its own Container volumes.
            let overflow = inv.add_item(ore, *qty, max_stack);
            if overflow > 0 {
                // A hauled load must NEVER vanish because the backpack is
                // packed (operator field report 2026-07-04: a 36/36
                // seed-filled backpack silently ate an entire iron haul,
                // starving the smelter). Grow the home stock -- the same
                // ensure_slots the dev-stock path uses -- and land the rest.
                let occupied = inv.slots.iter().filter(|s| s.is_some()).count();
                let extra = (overflow as usize).div_ceil(max_stack.max(1) as usize);
                inv.ensure_slots(occupied + extra);
                inv.add_item(ore, overflow, max_stack);
            }
            total += *qty;
        }
    }
    if total > 0 {
        log::info!("[Mining] drone delivered {total} units home");
        // A delivered haul trains Mining (1 XP per ore unit).
        crate::systems::skills::award_skill_xp(data, "mining", total);
    }
    total
}

/// DELETE fully-consumed asteroids (the operator's "deleted when consumed").
fn remove_mined_out(world: &mut hecs::World) {
    let depleted: Vec<hecs::Entity> = world
        .query::<&AsteroidBody>()
        .iter()
        .filter(|(_, a)| a.total_remaining() < 1.0)
        .map(|(e, _)| e)
        .collect();
    for e in depleted {
        let _ = world.despawn(e);
        log::info!("[Mining] asteroid depleted and removed");
    }
}

// ── The time away (offline progression, 2026-09-27) ─────────────────

/// The drone's standing order ("Keep mining"), as the save stores it.
pub fn standing_order(data: &DataStore) -> Option<(String, Vec<(String, u32)>)> {
    data.get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
        .and_then(|m| m.lock().ok().and_then(|s| s.clone()))
}

/// Put back the standing order a save carried (save_load::resume_home). It is
/// the player's own setting, like a craft batch they started, so it returns
/// whether or not the time away counts.
pub fn set_standing_order(data: &DataStore, order: Option<(String, Vec<(String, u32)>)>) {
    if let Some(slot) = data.get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order") {
        if let Ok(mut s) = slot.lock() {
            *s = order;
        }
    }
}

/// Most trips one return may catch up. A trip is at least nine seconds and
/// every one that brings ore home takes it from a finite asteroid, so a real
/// home never comes near this; it only bounds a pathological save.
const MAX_AWAY_TRIPS: usize = 100_000;

/// Fly the drone through `secs` of time the player was away (offline
/// progression, docs/design/offline-progression.md), with exactly the
/// session's rules: the trip in flight finishes, and while a standing order
/// is set it keeps flying the same trip until its asteroid is mined out. Ore
/// comes only out of the asteroid, so a haul is bounded by what is really
/// there, and it lands in the home inventory the way a session haul does.
/// A trip still in the air when the time runs out is left mid-flight, where
/// the player finds it.
///
/// Returns each haul as (seconds into the time away, cargo), so the
/// automated machines, which run through the same hours afterwards
/// (crafting::away), use ore only from the moment it arrived.
///
/// Nothing here can destroy anything: a drone has no fuel or wear, so the
/// doc's "must not advance" class does not reach it.
pub fn advance_away(world: &mut hecs::World, data: &DataStore, secs: f64) -> Vec<(f64, Vec<(String, u32)>)> {
    let mut hauls = Vec::new();
    if secs <= 0.0 {
        return hauls;
    }
    let item_registry = data.get::<ItemRegistry>("item_registry");
    let mut t = 0.0_f64;
    let first_drone = |world: &hecs::World| world.query::<&Drone>().iter().next().map(|(e, _)| e);
    for _ in 0..MAX_AWAY_TRIPS {
        let in_flight = first_drone(world);
        let drone_e = match in_flight {
            Some(e) => e,
            None => {
                // In a session the standing order relaunches the trip on the
                // next frame; here, at once.
                let Some((target, manifest)) = standing_order(data) else { break };
                if !launch(world, data, &target, &manifest) {
                    break;
                }
                match first_drone(world) {
                    Some(e) => e,
                    None => break,
                }
            }
        };
        // Step this drone phase by phase until it is home or the time is up.
        let delivered = loop {
            let Ok(d) = world.get::<&Drone>(drone_e).map(|d| (*d).clone()) else { break None };
            let left = f64::from((d.phase_duration(d.phase) - d.phase_time).max(0.0));
            if t + left > secs {
                if let Ok(mut live) = world.get::<&mut Drone>(drone_e) {
                    live.phase_time += (secs - t) as f32;
                }
                return hauls;
            }
            t += left;
            match d.phase {
                DronePhase::Outbound => {
                    let cargo = mine(world, &d.target, &d.manifest);
                    if let Ok(mut live) = world.get::<&mut Drone>(drone_e) {
                        live.phase = DronePhase::Mining;
                        live.phase_time = 0.0;
                        live.cargo = cargo;
                    }
                }
                DronePhase::Mining => {
                    if let Ok(mut live) = world.get::<&mut Drone>(drone_e) {
                        live.phase = DronePhase::Returning;
                        live.phase_time = 0.0;
                    }
                }
                DronePhase::Returning | DronePhase::Done => {
                    deliver_haul(world, data, item_registry, d.home, &d.cargo);
                    let _ = world.despawn(drone_e);
                    break Some(d.cargo.clone());
                }
            }
        };
        remove_mined_out(world);
        match delivered {
            Some(cargo) if cargo.iter().any(|(_, q)| *q > 0) => hauls.push((t, cargo)),
            // An empty trip: the asteroid holds none of what the order asks
            // for, so every further trip would come home empty too.
            _ => break,
        }
    }
    hauls
}

#[cfg(test)]
mod drone_tests {
    use super::*;

    fn make_store() -> DataStore {
        let mut data = DataStore::new();
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        data.insert("item_registry", items);
        data.insert(
            "commission_drone",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
        );
        data
    }

    fn commission(data: &DataStore, target: &str, manifest: Vec<(&str, u32)>) {
        *data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("commission_drone")
            .unwrap()
            .lock()
            .unwrap() = Some((
            target.to_string(),
            manifest.into_iter().map(|(o, u)| (o.to_string(), u)).collect(),
        ));
    }

    fn asteroid(id: &str, ores: Vec<(&str, f32)>) -> AsteroidBody {
        AsteroidBody {
            id: id.to_string(),
            name: id.to_string(),
            classification: "M".into(),
            ores: ores.into_iter().map(|(o, q)| (o.to_string(), q)).collect(),
            position: [0.0, 0.0, 0.0],
        }
    }

    /// Full loop: commission a manifest for an asteroid → the drone flies out, fills its
    /// hold, returns → ore delivered; a fully-mined asteroid is deleted.
    #[test]
    fn commission_manifest_mines_and_delivers() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        let ast = world.spawn((asteroid("rock", vec![("iron_ore_0", 8.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 8)]);
        sys.tick(&mut world, 1.0, &data); // launch (Outbound)
        assert_eq!(world.query::<&Drone>().iter().count(), 1, "drone launched");

        for _ in 0..18 {
            sys.tick(&mut world, 1.0, &data);
        }

        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(iron >= 8, "manifest ore delivered (got {iron})");
        assert_eq!(world.query::<&Drone>().iter().count(), 0, "completed drone despawned");
        assert!(world.get::<&AsteroidBody>(ast).is_err(), "depleted asteroid removed");
    }

    /// Standing order (economy automation Phase 1, v0.663): with an auto_mine_order
    /// set, the Deliver arm re-commissions the SAME trip after each haul -- the
    /// drone keeps cycling until the asteroid depletes, with zero further clicks.
    #[test]
    fn standing_order_relaunches_the_drone_after_delivery() {
        let mut data = make_store();
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
        );
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        // Big enough that the asteroid cannot deplete during the test window --
        // depletion legitimately ENDS the standing-order loop (target removed),
        // which is its own designed exit, not what this test measures.
        world.spawn((asteroid("rock", vec![("iron_ore_0", 100.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 8)]);
        // The standing order mirrors the commissioned trip ("Keep mining" checked).
        *data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
            .unwrap()
            .lock()
            .unwrap() = Some(("rock".to_string(), vec![("iron_ore_0".to_string(), 8)]));

        // First full trip (launch + fly + mine + return + deliver)...
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let after_first = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(after_first >= 8, "first haul delivered (got {after_first})");

        // ...and WITHOUT any new commission, a second trip runs on the standing order.
        for _ in 0..25 {
            sys.tick(&mut world, 1.0, &data);
        }
        let after_second = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(
            after_second > after_first,
            "standing order should have relaunched and delivered a second haul \
             ({after_first} -> {after_second})"
        );
    }

    /// Review fix (2026-07-01): the standing-order refire lives at TICK level, so
    /// auto-mining resumes even when the in-flight drone was lost without a
    /// delivery (a world reload despawns drones) -- a standing order + no drone +
    /// no commission must relaunch by itself.
    #[test]
    fn standing_order_resumes_after_drone_loss() {
        let mut data = make_store();
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::Some((
                "rock".to_string(),
                vec![("iron_ore_0".to_string(), 4u32)],
            ))),
        );
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 40.0)]),));

        // NO commission was ever written -- the refire alone must launch.
        for _ in 0..3 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert_eq!(
            world.query::<&Drone>().iter().count(),
            1,
            "the standing order alone relaunches a lost trip"
        );
        // And it delivers like any trip.
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(iron >= 4, "refired trip delivered (got {iron})");
    }

    /// Review fix (2026-07-01): when the standing order's target asteroid no
    /// longer exists (mined out and deleted), the order is CLEARED instead of
    /// refiring a dead commission every trip forever.
    #[test]
    fn depleted_target_ends_the_standing_order() {
        let mut data = make_store();
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::Some((
                "gone".to_string(),
                vec![("iron_ore_0".to_string(), 4u32)],
            ))),
        );
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        // No asteroid named "gone" exists.

        for _ in 0..3 {
            sys.tick(&mut world, 1.0, &data);
        }
        let order = data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(order.is_none(), "a standing order aimed at a gone target must end");
        assert_eq!(world.query::<&Drone>().iter().count(), 0, "nothing launched");
    }

    /// Loot is BOUNDED by the target asteroid's stock: requesting more than it holds
    /// returns only what was there (the operator's "loot from one asteroid is limited").
    #[test]
    fn loot_bounded_by_target_asteroid() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 3.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 10)]); // ask 10, only 3 there
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert_eq!(iron, 3, "only the asteroid's 3 units delivered (got {iron})");
    }

    /// One asteroid per run: a manifest mines ONLY the targeted asteroid; a second
    /// asteroid holding the same ore is left untouched.
    #[test]
    fn mines_only_the_target_asteroid() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("a", vec![("iron_ore_0", 5.0)]),));
        let other = world.spawn((asteroid("b", vec![("iron_ore_0", 50.0)]),));

        commission(&data, "a", vec![("iron_ore_0", 10)]); // target "a" (has 5)
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert_eq!(iron, 5, "only target 'a' mined (got {iron})");
        let other_left = world.get::<&AsteroidBody>(other).unwrap().total_remaining();
        assert_eq!(other_left, 50.0, "the other asteroid is untouched");
    }

    /// A multi-ore manifest pulls EACH ore from the one target asteroid.
    #[test]
    fn multi_ore_manifest_from_one_asteroid() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0), ("copper_ore_0", 50.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 6), ("copper_ore_0", 4)]);
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("iron_ore_0"), 6, "6 iron delivered");
        assert_eq!(inv.count_item("copper_ore_0"), 4, "4 copper delivered");
    }

    /// One drone per player: a second commission while one is in flight is ignored.
    #[test]
    fn one_drone_per_player() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 5)]);
        sys.tick(&mut world, 1.0, &data); // one drone now Outbound
        commission(&data, "rock", vec![("iron_ore_0", 5)]); // try a second
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(world.query::<&Drone>().iter().count(), 1, "still exactly one drone");
    }

    /// A commission for an asteroid that doesn't exist launches NO drone (no dud trip
    /// that burns the single drone slot for nothing).
    #[test]
    fn missing_target_launches_nothing() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));

        commission(&data, "ghost", vec![("iron_ore_0", 5)]); // no asteroid "ghost"
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(world.query::<&Drone>().iter().count(), 0, "no drone for a missing target");
    }

    /// A hauled load must never vanish because the home stock is full (operator
    /// field report 2026-07-04: a 36/36 seed-packed backpack silently ate an
    /// entire iron haul -- add_item's overflow return was discarded -- so the
    /// smelter starved while the player watched the drone "deliver"). Delivery
    /// now grows the inventory (dev-stock's ensure_slots pattern) and lands the
    /// remainder.
    #[test]
    fn delivery_grows_a_full_backpack_instead_of_losing_the_haul() {
        let data = make_store();
        let mut world = hecs::World::new();
        // A 2-slot inventory PACKED full (unstackable junk), like the seed-full backpack.
        let mut inv = Inventory::new(2);
        inv.add_item("hammer_0", 1, 1);
        inv.add_item("bandage_0", 1, 1);
        assert_eq!(inv.slots.iter().filter(|s| s.is_none()).count(), 0, "no free slot");
        let player = world.spawn((inv, crate::ecs::components::Controllable));
        world.spawn((AsteroidBody {
            id: "rock".to_string(),
            name: "rock".to_string(),
            classification: "M".into(),
            ores: [("iron_ore_0".to_string(), 5.0)].into_iter().collect(),
            position: [0.0, 0.0, 0.0],
        },));

        commission(&data, "rock", vec![("iron_ore_0", 5)]);
        let mut sys = DroneSystem::new();
        for _ in 0..60 {
            sys.tick(&mut world, 1.0, &data);
        }

        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(
            inv.count_item("iron_ore_0"),
            5,
            "the full haul landed -- the backpack grew instead of eating the ore"
        );
    }

    fn set_order(data: &mut DataStore, order: Option<(&str, Vec<(&str, u32)>)>) {
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(order.map(|(t, m)| {
                (t.to_string(), m.into_iter().map(|(o, u)| (o.to_string(), u)).collect::<Vec<_>>())
            })),
        );
    }

    /// While the player is away the standing order keeps the drone flying,
    /// out of the asteroid's real ore and no more: 25 iron comes home as
    /// 10, 10 and 5, the mined-out asteroid is deleted and the order ends,
    /// all as a session would. Each haul carries the moment it landed. Seen
    /// red with `advance_away` returning at once (no hauls, no ore).
    #[test]
    fn a_standing_order_keeps_mining_while_away_until_the_asteroid_is_empty() {
        let mut data = make_store();
        set_order(&mut data, Some(("rock", vec![("iron_ore_0", 10)])));
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 25.0)]),));

        let hauls = advance_away(&mut world, &data, 8.0 * 3600.0);

        let got: Vec<u32> = hauls.iter().map(|(_, c)| c.iter().map(|(_, q)| q).sum()).collect();
        assert_eq!(got, vec![10, 10, 5]);
        // A trip at the origin is 2 s out, 5 s mining, 2 s back.
        let times: Vec<f64> = hauls.iter().map(|(t, _)| *t).collect();
        assert_eq!(times, vec![9.0, 18.0, 27.0]);
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"), 25);
        assert_eq!(world.query::<&AsteroidBody>().iter().count(), 0, "mined out and deleted");
        assert_eq!(standing_order(&data), None, "the order ended with its asteroid");
        assert_eq!(world.query::<&Drone>().iter().count(), 0);
    }

    /// Without a standing order only the trip in flight comes home; a short
    /// absence leaves the next one in the air where the player finds it.
    /// Seen red with `advance_away` returning at once (no haul).
    #[test]
    fn the_trip_in_flight_finishes_and_the_next_is_left_in_the_air() {
        let mut data = make_store();
        set_order(&mut data, None);
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));
        commission(&data, "rock", vec![("iron_ore_0", 4)]);
        DroneSystem::new().tick(&mut world, 0.0, &data); // launched, 0 s into the trip

        let hauls = advance_away(&mut world, &data, 3600.0);
        assert_eq!(hauls.len(), 1, "one trip, no order to send another");
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"), 4);

        // With the order, 12 s away: one trip home (9 s), the next 3 s out.
        set_order(&mut data, Some(("rock", vec![("iron_ore_0", 4)])));
        let hauls = advance_away(&mut world, &data, 12.0);
        assert_eq!(hauls.len(), 1);
        let (_, d) = world.query::<&Drone>().iter().next().map(|(e, d)| (e, d.clone())).expect("in the air");
        assert_eq!((d.phase, d.phase_time), (DronePhase::Mining, 1.0), "2 s out, then 1 s of mining");
        assert_eq!(d.cargo, vec![("iron_ore_0".to_string(), 4)], "its hold already filled");
    }

    /// A drone asking for an ore its asteroid does not hold comes home empty,
    /// and so would every trip after: the catch-up stops there rather than
    /// flying empty trips for the whole absence.
    #[test]
    fn an_empty_trip_ends_the_catch_up() {
        let mut data = make_store();
        set_order(&mut data, Some(("rock", vec![("gold_ore_0", 4)])));
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));
        assert!(advance_away(&mut world, &data, 30.0 * 86_400.0).is_empty());
    }
}
