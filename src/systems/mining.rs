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
                // The target asteroid's position (home = origin for now), so travel
                // time + the map dot scale with how far the asteroid is.
                let target_pos = world
                    .query::<&AsteroidBody>()
                    .iter()
                    .find(|(_, a)| a.id == target)
                    .map(|(_, a)| a.position)
                    .unwrap_or([0.0, 0.0, 0.0]);
                let home: Option<u64> = world
                    .query::<(&Inventory, &Controllable)>()
                    .iter()
                    .next()
                    .map(|(e, _)| e.to_bits().into());
                if let Some(home) = home {
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
            }
        }

        // ── ADVANCE: tick each drone's phase machine, recording cross-entity intents.
        let mut intents: Vec<DroneIntent> = Vec::new();
        for (entity, drone) in world.query_mut::<&mut Drone>() {
            drone.phase_time += dt;
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
                                inv.add_item(ore, *qty, max_stack);
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
}
