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
                            target: target.clone(),
                            manifest: manifest.clone(),
                            phase: DronePhase::Outbound,
                            phase_time: 0.0,
                            cargo: Vec::new(),
                            home_pos: [0.0, 0.0, 0.0],
                            target_pos,
                        },));
                        log::info!("[Mining] commissioned a drone for {target}: {manifest:?}");
                    }
                    (None, _) => {
                        log::info!("[Mining] target asteroid '{target}' not found; not launching");
                        // A standing order aimed at a now-gone asteroid (mined
                        // out and deleted) would refire this dead commission
                        // every trip forever -- end the loop here (v0.663).
                        if let Some(slot) = data
                            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>(
                                "auto_mine_order",
                            )
                        {
                            if let Ok(mut s) = slot.lock() {
                                if s.as_ref().map_or(false, |(t, _)| *t == target) {
                                    log::info!(
                                        "[Mining] standing order for '{target}' ended (target gone)"
                                    );
                                    *s = None;
                                }
                            }
                        }
                    }
                    _ => {}
                }
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
                    // Pull each requested ore from the ONE target asteroid, bounded by
                    // what it holds. No spillover to other asteroids — one run mines one
                    // asteroid, so the haul is capped by that asteroid's stock.
                    let target_e = world
                        .query::<&AsteroidBody>()
                        .iter()
                        .find(|(_, a)| a.id == target)
                        .map(|(e, _)| e);
                    let mut collected: Vec<(String, u32)> = Vec::new();
                    if let Some(aid) = target_e {
                        for (ore, units) in &manifest {
                            if let Ok(mut body) = world.get::<&mut AsteroidBody>(aid) {
                                let took = body.take(ore, *units as f32);
                                if took > 0 {
                                    collected.push((ore.clone(), took));
                                }
                            }
                        }
                    }
                    log::info!("[Mining] drone extracted {collected:?} from {target}");
                    if let Ok(mut d) = world.get::<&mut Drone>(drone) {
                        d.cargo = collected;
                    }
                }
                DroneIntent::Deliver { drone, home, cargo } => {
                    if let Some(home_e) = hecs::Entity::from_bits(home) {
                        let mut total = 0u32;
                        for (ore, qty) in &cargo {
                            if *qty == 0 {
                                continue;
                            }
                            let max_stack =
                                item_registry.map(|r| r.max_stack_for(ore)).unwrap_or(99);
                            if let Ok(mut inv) = world.get::<&mut Inventory>(home_e) {
                                let overflow = inv.add_item(ore, *qty, max_stack);
                                if overflow > 0 {
                                    // A hauled load must NEVER vanish because the
                                    // backpack is packed (operator field report
                                    // 2026-07-04: a 36/36 seed-filled backpack
                                    // silently ate an entire iron haul, starving
                                    // the smelter). Grow the home stock -- the
                                    // same ensure_slots the dev-stock path uses --
                                    // and land the remainder.
                                    let occupied =
                                        inv.slots.iter().filter(|s| s.is_some()).count();
                                    let extra =
                                        (overflow as usize).div_ceil(max_stack.max(1) as usize);
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
                    }
                    let _ = world.despawn(drone);
                    // (Standing-order relaunch happens at TICK level above, not
                    // here -- see the refire block after the commission drain.)
                }
            }
        }

        // ── DELETE fully-consumed asteroids (the operator's "deleted when consumed").
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

}
