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
/// Ore units a drone hauls per trip.
const DRONE_CAPACITY: f32 = 10.0;

/// What a drone needs done to OTHER entities this tick — computed while iterating
/// drones (a `&mut Drone` query) and applied afterwards, so the cross-entity
/// `&mut World` borrows never overlap the drone query.
enum DroneIntent {
    /// Extract up to `DRONE_CAPACITY` of `ore_id` from `asteroid` into `drone`'s cargo.
    Mine {
        drone: hecs::Entity,
        asteroid: u64,
        ore_id: String,
    },
    /// Deliver `qty` of `ore_id` into `home`'s inventory, then despawn `drone`.
    Deliver {
        drone: hecs::Entity,
        home: u64,
        ore_id: String,
        qty: u32,
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

        // ── COMMISSION: drain the channel (the Mining panel writes an ore id) and
        //    launch a drone — home = the player, target = an asteroid holding that ore.
        let commissioned = data
            .get::<std::sync::Mutex<Option<String>>>("commission_drone")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(ore_id) = commissioned {
            // Each query's borrow is released at its statement boundary (home/target
            // are plain u64), so the following spawn() can take &mut World.
            let home: Option<u64> = world
                .query::<(&Inventory, &Controllable)>()
                .iter()
                .next()
                .map(|(e, _)| e.to_bits().into());
            let target: Option<u64> = world
                .query::<&AsteroidBody>()
                .iter()
                .find(|(_, a)| a.has_ore(&ore_id))
                .map(|(e, _)| e.to_bits().into());
            if let (Some(home), Some(target)) = (home, target) {
                world.spawn((Drone {
                    home,
                    target,
                    ore_id: ore_id.clone(),
                    phase: DronePhase::Outbound,
                    phase_time: 0.0,
                    cargo: 0,
                },));
                log::info!("[Mining] commissioned a drone for {ore_id}");
            } else {
                log::info!("[Mining] no asteroid with {ore_id} available; drone not launched");
            }
        }

        // ── ADVANCE: tick each drone's phase machine, recording cross-entity intents.
        let mut intents: Vec<DroneIntent> = Vec::new();
        for (entity, drone) in world.query_mut::<&mut Drone>() {
            drone.phase_time += dt;
            match drone.phase {
                DronePhase::Outbound if drone.phase_time >= OUTBOUND_SECS => {
                    drone.phase = DronePhase::Mining;
                    drone.phase_time = 0.0;
                    intents.push(DroneIntent::Mine {
                        drone: entity,
                        asteroid: drone.target,
                        ore_id: drone.ore_id.clone(),
                    });
                }
                DronePhase::Mining if drone.phase_time >= MINING_SECS => {
                    drone.phase = DronePhase::Returning;
                    drone.phase_time = 0.0;
                }
                DronePhase::Returning if drone.phase_time >= RETURNING_SECS => {
                    drone.phase = DronePhase::Done;
                    drone.phase_time = 0.0;
                    intents.push(DroneIntent::Deliver {
                        drone: entity,
                        home: drone.home,
                        ore_id: drone.ore_id.clone(),
                        qty: drone.cargo,
                    });
                }
                _ => {}
            }
        }

        // ── APPLY: mutate the asteroid / home inventory / despawn the drone (the drone
        //    query borrow is released now, so these &mut World gets are conflict-free).
        for intent in intents {
            match intent {
                DroneIntent::Mine {
                    drone,
                    asteroid,
                    ore_id,
                } => {
                    let mined = hecs::Entity::from_bits(asteroid)
                        .and_then(|a| {
                            world
                                .get::<&mut AsteroidBody>(a)
                                .ok()
                                .map(|mut body| body.take(&ore_id, DRONE_CAPACITY))
                        })
                        .unwrap_or(0);
                    if let Ok(mut d) = world.get::<&mut Drone>(drone) {
                        d.cargo = mined;
                    }
                    log::info!("[Mining] drone extracted {mined}x {ore_id}");
                }
                DroneIntent::Deliver {
                    drone,
                    home,
                    ore_id,
                    qty,
                } => {
                    if qty > 0 {
                        if let Some(home_e) = hecs::Entity::from_bits(home) {
                            let max_stack =
                                item_registry.map(|r| r.max_stack_for(&ore_id)).unwrap_or(99);
                            if let Ok(mut inv) = world.get::<&mut Inventory>(home_e) {
                                inv.add_item(&ore_id, qty, max_stack);
                                log::info!("[Mining] drone delivered {qty}x {ore_id} home");
                            }
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
            std::sync::Mutex::new(Option::<String>::None),
        );
        data
    }

    /// Full mining loop: commission a drone for iron ore → it flies out, mines, returns
    /// → ore delivered into the player inventory; an asteroid mined empty is deleted.
    #[test]
    fn commission_drone_mines_asteroid_and_delivers() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        let asteroid = world.spawn((AsteroidBody {
            name: "Test Rock".into(),
            classification: "M".into(),
            ores: vec![("iron_ore_0".to_string(), 8.0)],
        },));

        // Commission a drone for iron ore (the channel the Mining panel writes).
        *data
            .get::<std::sync::Mutex<Option<String>>>("commission_drone")
            .unwrap()
            .lock()
            .unwrap() = Some("iron_ore_0".to_string());
        sys.tick(&mut world, 1.0, &data); // launches the drone (Outbound)

        let drones: Vec<hecs::Entity> = world.query::<&Drone>().iter().map(|(e, _)| e).collect();
        assert_eq!(drones.len(), 1, "drone launched");

        // Drive the full mission (5 outbound + 5 mining + 5 returning = 15s, + slack).
        for _ in 0..18 {
            sys.tick(&mut world, 1.0, &data);
        }

        let iron = world
            .get::<&Inventory>(player)
            .unwrap()
            .count_item("iron_ore_0");
        assert!(iron >= 8, "drone delivered the mined iron ore (got {iron})");
        assert_eq!(
            world.query::<&Drone>().iter().count(),
            0,
            "the completed drone despawned"
        );
        // The asteroid had 8 ore and capacity is 10 → fully mined → deleted.
        assert!(
            world.get::<&AsteroidBody>(asteroid).is_err(),
            "the depleted asteroid was removed"
        );
    }

    /// A commission for an ore no asteroid has launches no drone (no crash).
    #[test]
    fn commission_for_absent_ore_launches_nothing() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((AsteroidBody {
            name: "Iron Rock".into(),
            classification: "M".into(),
            ores: vec![("iron_ore_0".to_string(), 50.0)],
        },));

        *data
            .get::<std::sync::Mutex<Option<String>>>("commission_drone")
            .unwrap()
            .lock()
            .unwrap() = Some("platinum_ore_0".to_string());
        sys.tick(&mut world, 1.0, &data);

        assert_eq!(
            world.query::<&Drone>().iter().count(),
            0,
            "no drone launched for an unavailable ore"
        );
    }
}
