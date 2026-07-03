//! Vehicle & mech system — entering/exiting vehicles, mech torso twist,
//! jump jets, heat management, and vehicle movement control.
//!
//! Vehicle definitions loaded from `data/vehicles.csv`.
//! Ship-specific logic in `ships.rs`, propulsion physics in `propulsion.rs`.

pub mod ships;
pub mod propulsion;

use std::collections::HashMap;

use glam::{Quat, Vec3};
use serde::Deserialize;

use crate::ecs::components::{Controllable, Transform, Vehicle, Velocity, VehicleSeat};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Heat dissipation rate per second (fraction of max heat).
const HEAT_DISSIPATION_RATE: f32 = 0.05;
/// Heat threshold above which weapons are disabled (fraction, 0-1).
const OVERHEAT_THRESHOLD: f32 = 0.9;
/// Heat threshold below which weapons re-enable after overheat.
const COOLDOWN_THRESHOLD: f32 = 0.5;
/// Jump jet fuel consumption rate per second while jetting.
const JUMP_JET_FUEL_RATE: f32 = 0.15;
/// Upward impulse applied per second while jump jets fire (m/s per second).
const JUMP_JET_IMPULSE: f32 = 12.0;
/// Maximum torso twist angle in radians (~90 degrees each way).
const MAX_TORSO_YAW: f32 = std::f32::consts::FRAC_PI_2;
/// Distance from vehicle center to place player on exit (meters).
const EXIT_OFFSET: f32 = 3.0;
/// How far in front of the player a deployed kit assembles (meters).
const DEPLOY_DISTANCE: f32 = 6.0;
/// The camera IS the player's eye; its ground rest height is floor + 1.7
/// (CameraController::eye_height). Deploying drops the vehicle to floor level
/// by subtracting this from the camera's world Y.
const DEPLOY_EYE_DROP: f32 = 1.7;

// ── Vehicle kit registry (economy Phase 2 Stage 1) ──────────────────────

/// One deployable kit: which inventory item unpacks into which vehicle, and
/// the primitive body proportions the renderer draws. Loaded from
/// `data/vehicles/kits.ron` into DataStore["vehicle_kit_registry"].
#[derive(Debug, Clone, Deserialize)]
pub struct VehicleKitDef {
    /// items.csv id of the KIT (the thing in the backpack, e.g. "truck_pickup_kit_0").
    pub kit_item: String,
    /// items.csv id of the ASSEMBLED vehicle (e.g. "truck_pickup_0").
    pub vehicle_item: String,
    /// Human name for the spawned entity's Name component.
    pub display_name: String,
    /// Body box in meters, vehicle-local: (length x, height y, width z).
    pub body_m: (f32, f32, f32),
    /// Cabin box in meters, same frame.
    pub cabin_m: (f32, f32, f32),
    /// Cabin center offset forward (+x) of the body center, meters.
    pub cabin_offset_x: f32,
    /// Wheel radius in meters (4 wheels at the body corners).
    pub wheel_radius_m: f32,
    /// Self-drive transit speed in m/s (Stage 3 summon/delivery). serde-default
    /// so kit entries without it drive at a sane homestead pace.
    #[serde(default = "default_speed_mps")]
    pub speed_mps: f32,
}

fn default_speed_mps() -> f32 {
    6.0
}

/// All deployable kits, keyed both ways: by kit item (deploy consumes) and by
/// vehicle item (the renderer looks up proportions for a spawned Vehicle).
#[derive(Debug, Clone, Default)]
pub struct VehicleKitRegistry {
    by_kit: HashMap<String, VehicleKitDef>,
}

impl VehicleKitRegistry {
    /// Parse the RON array shape `data/vehicles/kits.ron` ships.
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let kits: Vec<VehicleKitDef> = ron::from_str(text).map_err(|e| e.to_string())?;
        let mut by_kit = HashMap::new();
        for k in kits {
            by_kit.insert(k.kit_item.clone(), k);
        }
        Ok(Self { by_kit })
    }

    pub fn get_kit(&self, kit_item: &str) -> Option<&VehicleKitDef> {
        self.by_kit.get(kit_item)
    }

    /// Reverse lookup by the ASSEMBLED vehicle's item id (renderer side; the
    /// registry is small, a scan is fine).
    pub fn get_vehicle(&self, vehicle_item: &str) -> Option<&VehicleKitDef> {
        self.by_kit.values().find(|k| k.vehicle_item == vehicle_item)
    }

    pub fn len(&self) -> usize {
        self.by_kit.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_kit.is_empty()
    }
}

// ── Commands passed via DataStore ───────────────────────────────────────

/// Command to enter a vehicle. Place in DataStore under key "enter_vehicle".
#[derive(Debug, Clone)]
pub struct EnterVehicleCommand {
    /// The player entity requesting entry (as entity bits).
    pub player_entity: u64,
    /// The vehicle entity to enter (as entity bits).
    pub vehicle_entity: u64,
    /// Player's public key for seat assignment.
    pub player_key: String,
}

/// Command to exit the current vehicle. Place in DataStore under key "exit_vehicle".
#[derive(Debug, Clone)]
pub struct ExitVehicleCommand {
    /// The player entity requesting exit (as entity bits).
    pub player_entity: u64,
}

/// Mech-specific per-entity state: torso rotation, heat, jump jets.
#[derive(Debug, Clone)]
pub struct MechState {
    /// Independent torso yaw relative to legs (radians, clamped to +/- MAX_TORSO_YAW).
    pub torso_yaw: f32,
    /// Current heat level (0.0 = cool, 1.0 = max heat).
    pub heat: f32,
    /// Jump jet fuel remaining (0.0 = empty, 1.0 = full).
    pub jump_jet_fuel: f32,
    /// Whether the mech is currently overheated (weapons disabled).
    pub overheated: bool,
}

impl Default for MechState {
    fn default() -> Self {
        Self {
            torso_yaw: 0.0,
            heat: 0.0,
            jump_jet_fuel: 1.0,
            overheated: false,
        }
    }
}

/// Mech input commands read from DataStore each tick under key "mech_input".
#[derive(Debug, Clone, Default)]
pub struct MechInput {
    /// Desired torso yaw delta this frame (radians).
    pub torso_yaw_delta: f32,
    /// Whether jump jets are firing this frame.
    pub jump_jets_active: bool,
    /// Heat generated by weapons this frame (0-1 scale).
    pub weapon_heat: f32,
}

/// Vehicle control system — handles enter/exit, mech features, vehicle movement.
pub struct VehicleSystem {
    /// Per-entity mech state, keyed by entity id bits.
    mech_states: HashMap<u64, MechState>,
}

impl VehicleSystem {
    pub fn new() -> Self {
        Self {
            mech_states: HashMap::new(),
        }
    }

    /// Get mech state for an entity (read-only).
    pub fn mech_state(&self, entity_bits: u64) -> Option<&MechState> {
        self.mech_states.get(&entity_bits)
    }

    /// Check whether a mech is overheated (weapons disabled).
    pub fn is_overheated(&self, entity_bits: u64) -> bool {
        self.mech_states
            .get(&entity_bits)
            .map_or(false, |s| s.overheated)
    }
}

impl System for VehicleSystem {
    fn name(&self) -> &str {
        "VehicleSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // ── 0. Deploy a vehicle kit (economy Phase 2 Stage 1) ────────
        self.handle_deploy(world, data);

        // ── 0b. Summon a parked vehicle to the player (Stage 3) ──────
        self.handle_summon(world, data);

        // ── 0c. Advance vehicles in transit (Stage 3) ─────────────────
        Self::tick_routes(world, dt, data);

        // ── 1. Handle enter vehicle commands ────────────────────────
        if let Some(cmd) = data.get::<EnterVehicleCommand>("enter_vehicle") {
            self.handle_enter(world, cmd);
        }

        // ── 2. Handle exit vehicle commands ─────────────────────────
        if let Some(cmd) = data.get::<ExitVehicleCommand>("exit_vehicle") {
            self.handle_exit(world, cmd);
        }

        // ── 3. Update mech-specific state (heat, jump jets, torso) ──
        let mech_input = data.get::<MechInput>("mech_input").cloned().unwrap_or_default();
        self.tick_mechs(world, dt, &mech_input);
    }
}

impl VehicleSystem {
    /// Deploy a vehicle KIT from the player's inventory into the world
    /// (economy Phase 2 Stage 1): drain the one-shot "deploy_kit_request"
    /// channel, validate the kit against the registry, consume the kit item
    /// in survival (creative deploys free, same semantics as manual crafting
    /// and seed planting), then spawn the Vehicle entity in front of the
    /// player at floor level.
    ///
    /// Ordering is deliberate: registry lookup BEFORE consume, so an unknown
    /// kit id never costs the player the item; and the request is take()n
    /// exactly once, so a second click while this frame's request is pending
    /// simply overwrites the same Option — one kit item can never produce two
    /// vehicles (the same-tick duplication class the Phase 1 review caught).
    fn handle_deploy(&mut self, world: &mut hecs::World, data: &DataStore) {
        let Some(kit_id) = data
            .get::<std::sync::Mutex<Option<String>>>("deploy_kit_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()))
        else {
            return;
        };

        let Some(def) = data
            .get::<VehicleKitRegistry>("vehicle_kit_registry")
            .and_then(|r| r.get_kit(&kit_id).cloned())
        else {
            log::warn!("deploy_kit_request: '{kit_id}' is not in the vehicle kit registry — refused, nothing consumed");
            return;
        };

        // Survival consumes the kit from the player's backpack; creative
        // deploys without consuming (consistent with crafting/planting).
        let creative = data
            .get::<std::sync::Mutex<bool>>("creative_mode")
            .and_then(|m| m.lock().ok().map(|g| *g))
            .unwrap_or(false);
        if !creative {
            let mut consumed = false;
            for (_e, (inv, _ctrl)) in world.query_mut::<(
                &mut crate::systems::inventory::Inventory,
                &Controllable,
            )>() {
                if inv.has_item(&kit_id, 1) {
                    inv.remove_item(&kit_id, 1);
                    consumed = true;
                }
                break;
            }
            if !consumed {
                log::warn!("deploy_kit_request: no '{kit_id}' in the backpack — refused");
                return;
            }
        }

        let (position, yaw) = Self::deploy_pose(world, data);
        let name = def.display_name.clone();
        world.spawn((
            Vehicle { item_id: def.vehicle_item.clone() },
            Transform {
                position,
                rotation: Quat::from_rotation_y(yaw),
                scale: Vec3::ONE,
            },
            Velocity::default(),
            VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
            crate::ecs::components::Name(name),
        ));
        log::info!(
            "Deployed {} ({}) from kit {kit_id} at {position}",
            def.display_name,
            def.vehicle_item
        );
    }

    /// Where a deployed vehicle assembles: DEPLOY_DISTANCE in front of the
    /// camera at floor level, facing the same way the player looks. Falls back
    /// to just beside the player's ECS Transform when the camera keys are not
    /// published (menu mode / headless tests).
    fn deploy_pose(world: &mut hecs::World, data: &DataStore) -> (Vec3, f32) {
        if let (Some(cam_pos), Some(cam_fwd)) =
            (data.get::<Vec3>("camera_position"), data.get::<Vec3>("camera_forward"))
        {
            let ground = *cam_pos + *cam_fwd * DEPLOY_DISTANCE - Vec3::new(0.0, DEPLOY_EYE_DROP, 0.0);
            let yaw = data.get::<f32>("camera_yaw").copied().unwrap_or(0.0);
            return (ground, yaw);
        }
        for (_e, (tf, _ctrl)) in world.query_mut::<(&Transform, &Controllable)>() {
            return (tf.position + Vec3::new(DEPLOY_DISTANCE, 0.0, 0.0), 0.0);
        }
        (Vec3::ZERO, 0.0)
    }

    /// Summon a parked vehicle to the player (economy Phase 2 Stage 3): drain
    /// the one-shot "summon_vehicle" channel (entity bits from the GUI's
    /// Vehicles section), validate it IS a vehicle and not already in transit,
    /// and attach a VehicleRoute to the player's ground position. The vehicle
    /// then drives itself over -- the "watch your purchase arrive" seed the
    /// operator asked for; follow/take-over build on this transit state.
    fn handle_summon(&mut self, world: &mut hecs::World, data: &DataStore) {
        let Some(bits) = data
            .get::<std::sync::Mutex<Option<u64>>>("summon_vehicle")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()))
        else {
            return;
        };
        let Some(entity) = hecs::Entity::from_bits(bits) else { return };
        let Ok(veh) = world.get::<&Vehicle>(entity).map(|v| v.item_id.clone()) else {
            log::warn!("summon_vehicle: entity {bits:#x} is not a vehicle");
            return;
        };
        if world.get::<&crate::ecs::components::VehicleRoute>(entity).is_ok() {
            return; // already in transit
        }
        let speed = data
            .get::<VehicleKitRegistry>("vehicle_kit_registry")
            .and_then(|r| r.get_vehicle(&veh))
            .map(|d| d.speed_mps)
            .unwrap_or(default_speed_mps());
        let (dest, _yaw) = Self::deploy_pose(world, data); // player's ground spot
        let _ = world.insert_one(
            entity,
            crate::ecs::components::VehicleRoute {
                dest,
                speed_mps: speed.max(0.5),
                arrive_radius: 4.0,
            },
        );
        log::info!("Vehicle {veh} summoned to {dest}");
    }

    /// Advance every vehicle in transit on GAME time: straight-line travel
    /// toward dest, yawing to face the direction of motion (the render body's
    /// long axis is +X, and Quat::from_rotation_y maps +X to (cos, 0, -sin)).
    /// Arrival (within arrive_radius) removes the route -- the vehicle parks.
    fn tick_routes(world: &mut hecs::World, dt: f32, data: &DataStore) {
        let sdt = crate::systems::time::scaled_dt(dt, data);
        let mut arrived: Vec<hecs::Entity> = Vec::new();
        for (e, (tf, route)) in world
            .query_mut::<(&mut Transform, &crate::ecs::components::VehicleRoute)>()
        {
            let to = route.dest - tf.position;
            let dist = to.length();
            let step = route.speed_mps * sdt;
            if dist <= route.arrive_radius.max(step) {
                arrived.push(e);
                continue;
            }
            let dir = to / dist;
            tf.position += dir * step;
            tf.rotation = Quat::from_rotation_y((-dir.z).atan2(dir.x));
        }
        for e in arrived {
            let _ = world.remove_one::<crate::ecs::components::VehicleRoute>(e);
            log::info!("Vehicle arrived and parked");
        }
    }

    /// Transfer `Controllable` from player to vehicle, seat the player.
    fn handle_enter(&mut self, world: &mut hecs::World, cmd: &EnterVehicleCommand) {
        let player = match hecs::Entity::from_bits(cmd.player_entity) {
            Some(e) => e,
            None => return,
        };
        let vehicle = match hecs::Entity::from_bits(cmd.vehicle_entity) {
            Some(e) => e,
            None => return,
        };

        // Verify player currently has Controllable
        if world.get::<&Controllable>(player).is_err() {
            return;
        }

        // Check vehicle has a free seat
        let seat_free = world
            .get::<&VehicleSeat>(vehicle)
            .map(|s| s.occupant_key.is_none())
            .unwrap_or(false);

        if !seat_free {
            return;
        }

        // Move Controllable: remove from player, add to vehicle
        let _ = world.remove_one::<Controllable>(player);
        let _ = world.insert_one(vehicle, Controllable);

        // Assign seat
        if let Ok(mut seat) = world.get::<&mut VehicleSeat>(vehicle) {
            seat.occupant_key = Some(cmd.player_key.clone());
        }

        // Initialize mech state if this vehicle doesn't have one yet
        self.mech_states
            .entry(cmd.vehicle_entity)
            .or_insert_with(MechState::default);
    }

    /// Transfer `Controllable` back to player, unseat, place player near vehicle.
    fn handle_exit(&mut self, world: &mut hecs::World, cmd: &ExitVehicleCommand) {
        let player = match hecs::Entity::from_bits(cmd.player_entity) {
            Some(e) => e,
            None => return,
        };

        // Find which vehicle this player occupies by scanning all VehicleSeats.
        // We need to collect first to avoid borrow conflicts.
        let occupied: Vec<(hecs::Entity, u64)> = world
            .query::<&VehicleSeat>()
            .iter()
            .filter(|(_, seat)| seat.occupant_key.is_some())
            .map(|(entity, _)| (entity, entity.to_bits().into()))
            .collect();

        let mut vehicle_entity = None;
        let mut vehicle_pos = Vec3::ZERO;

        for (entity, _bits) in &occupied {
            // Check if this vehicle has Controllable (meaning our player is driving it)
            if world.get::<&Controllable>(*entity).is_ok() {
                vehicle_entity = Some(*entity);
                if let Ok(tf) = world.get::<&Transform>(*entity) {
                    vehicle_pos = tf.position;
                }
                break;
            }
        }

        let vehicle = match vehicle_entity {
            Some(v) => v,
            None => return,
        };

        // Move Controllable back to player
        let _ = world.remove_one::<Controllable>(vehicle);
        let _ = world.insert_one(player, Controllable);

        // Clear seat
        if let Ok(mut seat) = world.get::<&mut VehicleSeat>(vehicle) {
            seat.occupant_key = None;
        }

        // Place player near vehicle
        if let Ok(mut tf) = world.get::<&mut Transform>(player) {
            tf.position = vehicle_pos + Vec3::new(EXIT_OFFSET, 0.0, 0.0);
        }

        // Stop vehicle movement
        if let Ok(mut vel) = world.get::<&mut Velocity>(vehicle) {
            vel.linear = Vec3::ZERO;
        }
    }

    /// Update all mech entities: heat dissipation, jump jets, torso twist.
    fn tick_mechs(&mut self, world: &mut hecs::World, dt: f32, input: &MechInput) {
        // Collect controlled vehicle entities (they have both Controllable and VehicleSeat)
        let controlled_vehicles: Vec<(hecs::Entity, u64)> = world
            .query::<(&Controllable, &VehicleSeat)>()
            .iter()
            .map(|(entity, _)| (entity, entity.to_bits().into()))
            .collect();

        for (_entity, bits) in &controlled_vehicles {
            let state = self.mech_states.entry(*bits).or_insert_with(MechState::default);

            // ── Heat management ─────────────────────────────────────
            // Add weapon heat
            state.heat = (state.heat + input.weapon_heat * dt).min(1.0);

            // Dissipate heat over time
            state.heat = (state.heat - HEAT_DISSIPATION_RATE * dt).max(0.0);

            // Overheat state transitions
            if !state.overheated && state.heat >= OVERHEAT_THRESHOLD {
                state.overheated = true;
                log::info!("Mech {:016x} overheated — weapons disabled", bits);
            } else if state.overheated && state.heat <= COOLDOWN_THRESHOLD {
                state.overheated = false;
                log::info!("Mech {:016x} cooled down — weapons re-enabled", bits);
            }

            // ── Torso twist ─────────────────────────────────────────
            state.torso_yaw = (state.torso_yaw + input.torso_yaw_delta)
                .clamp(-MAX_TORSO_YAW, MAX_TORSO_YAW);

            // ── Jump jets ───────────────────────────────────────────
            let entity = match hecs::Entity::from_bits(*bits) {
                Some(e) => e,
                None => continue,
            };

            if input.jump_jets_active && state.jump_jet_fuel > 0.0 {
                state.jump_jet_fuel = (state.jump_jet_fuel - JUMP_JET_FUEL_RATE * dt).max(0.0);

                if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
                    vel.linear.y += JUMP_JET_IMPULSE * dt;
                }
            }
        }
    }
}

#[cfg(test)]
mod deploy_tests {
    use super::*;
    use crate::ecs::components::Name;
    use crate::systems::inventory::Inventory;

    /// The registry as data/vehicles/kits.ron ships it (a trimmed copy so the
    /// test also locks the RON shape the loader expects).
    const KITS_RON: &str = r#"[
        (
            kit_item: "truck_pickup_kit_0",
            vehicle_item: "truck_pickup_0",
            display_name: "Pickup Truck",
            body_m: (4.9, 0.9, 1.9),
            cabin_m: (1.7, 0.75, 1.75),
            cabin_offset_x: 0.55,
            wheel_radius_m: 0.42,
        ),
    ]"#;

    fn make_store(creative: bool) -> DataStore {
        let mut data = DataStore::new();
        data.insert(
            "vehicle_kit_registry",
            VehicleKitRegistry::from_ron(KITS_RON.as_bytes()).expect("kits ron parses"),
        );
        data.insert(
            "deploy_kit_request",
            std::sync::Mutex::new(Option::<String>::None),
        );
        data.insert("creative_mode", std::sync::Mutex::new(creative));
        data
    }

    fn request_deploy(data: &DataStore, kit: &str) {
        *data
            .get::<std::sync::Mutex<Option<String>>>("deploy_kit_request")
            .unwrap()
            .lock()
            .unwrap() = Some(kit.to_string());
    }

    fn spawn_player(world: &mut hecs::World, kits: u32) -> hecs::Entity {
        let mut inv = Inventory::new(36);
        if kits > 0 {
            inv.add_item("truck_pickup_kit_0", kits, 1);
        }
        world.spawn((
            Controllable,
            inv,
            Transform::default(),
            Name("Tester".to_string()),
        ))
    }

    fn vehicle_count(world: &mut hecs::World) -> usize {
        world.query_mut::<&Vehicle>().into_iter().count()
    }

    fn player_kit_count(world: &mut hecs::World, player: hecs::Entity) -> u32 {
        world
            .get::<&Inventory>(player)
            .map(|inv| inv.count_item("truck_pickup_kit_0"))
            .unwrap_or(0)
    }

    #[test]
    fn deploying_a_kit_consumes_it_and_spawns_the_vehicle() {
        let mut world = hecs::World::new();
        let data = make_store(false);
        let player = spawn_player(&mut world, 1);
        let mut sys = VehicleSystem::new();

        request_deploy(&data, "truck_pickup_kit_0");
        sys.tick(&mut world, 0.016, &data);

        assert_eq!(vehicle_count(&mut world), 1, "one vehicle spawned");
        assert_eq!(player_kit_count(&mut world, player), 0, "the kit was consumed");
        // The spawned entity carries the full Stage 1 tuple.
        let (item_id, seat_free, name) = world
            .query_mut::<(&Vehicle, &VehicleSeat, &Name)>()
            .into_iter()
            .map(|(_e, (v, s, n))| (v.item_id.clone(), s.occupant_key.is_none(), n.0.clone()))
            .next()
            .expect("vehicle entity present");
        assert_eq!(item_id, "truck_pickup_0");
        assert!(seat_free, "pilot seat starts empty");
        assert_eq!(name, "Pickup Truck");
    }

    #[test]
    fn deploy_without_the_kit_in_survival_is_refused() {
        let mut world = hecs::World::new();
        let data = make_store(false);
        spawn_player(&mut world, 0);
        let mut sys = VehicleSystem::new();

        request_deploy(&data, "truck_pickup_kit_0");
        sys.tick(&mut world, 0.016, &data);

        assert_eq!(vehicle_count(&mut world), 0, "no kit, no vehicle");
    }

    #[test]
    fn unknown_kit_is_refused_and_nothing_is_consumed() {
        let mut world = hecs::World::new();
        let data = make_store(false);
        let player = spawn_player(&mut world, 1);
        let mut sys = VehicleSystem::new();

        request_deploy(&data, "not_a_registered_kit_0");
        sys.tick(&mut world, 0.016, &data);

        assert_eq!(vehicle_count(&mut world), 0, "unregistered kit spawns nothing");
        assert_eq!(
            player_kit_count(&mut world, player),
            1,
            "registry lookup happens BEFORE consume — the item must survive"
        );
    }

    #[test]
    fn creative_mode_deploys_without_consuming() {
        let mut world = hecs::World::new();
        let data = make_store(true);
        let player = spawn_player(&mut world, 1);
        let mut sys = VehicleSystem::new();

        request_deploy(&data, "truck_pickup_kit_0");
        sys.tick(&mut world, 0.016, &data);

        assert_eq!(vehicle_count(&mut world), 1);
        assert_eq!(
            player_kit_count(&mut world, player),
            1,
            "creative deploy is free (same semantics as creative crafting/planting)"
        );
    }

    #[test]
    fn one_kit_cannot_become_two_vehicles_across_repeated_requests() {
        // The duplication shape the Phase 1 review hunted: fire the deploy
        // request twice with only one kit in stock. The second must be refused.
        let mut world = hecs::World::new();
        let data = make_store(false);
        let player = spawn_player(&mut world, 1);
        let mut sys = VehicleSystem::new();

        request_deploy(&data, "truck_pickup_kit_0");
        sys.tick(&mut world, 0.016, &data);
        request_deploy(&data, "truck_pickup_kit_0");
        sys.tick(&mut world, 0.016, &data);

        assert_eq!(vehicle_count(&mut world), 1, "one kit -> exactly one vehicle");
        assert_eq!(player_kit_count(&mut world, player), 0);
    }

    #[test]
    fn deploy_lands_in_front_of_the_camera_at_floor_level() {
        let mut world = hecs::World::new();
        let mut data = make_store(false);
        // Player standing at the origin, camera published the way lib.rs does.
        data.insert("camera_position", Vec3::new(10.0, 1.7, 5.0));
        data.insert("camera_forward", Vec3::new(0.0, 0.0, -1.0));
        data.insert("camera_yaw", 0.5_f32);
        spawn_player(&mut world, 1);
        let mut sys = VehicleSystem::new();

        request_deploy(&data, "truck_pickup_kit_0");
        sys.tick(&mut world, 0.016, &data);

        let pos = world
            .query_mut::<(&Vehicle, &Transform)>()
            .into_iter()
            .map(|(_e, (_v, t))| t.position)
            .next()
            .expect("vehicle spawned");
        assert!((pos.x - 10.0).abs() < 1e-4);
        assert!((pos.z - (5.0 - DEPLOY_DISTANCE)).abs() < 1e-4, "6 m in front of the camera");
        assert!(pos.y.abs() < 1e-4, "camera eye height dropped to floor level");
    }

    #[test]
    fn kit_registry_parses_the_shipped_data_file() {
        // Lock the REAL data/vehicles/kits.ron: it must parse and every kit +
        // vehicle id it references must exist in items.csv.
        let reg = VehicleKitRegistry::from_ron(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/vehicles/kits.ron"
        )))
        .expect("shipped kits.ron parses");
        assert!(!reg.is_empty(), "at least one kit ships");
        let items = std::str::from_utf8(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .unwrap()
        .to_string();
        for def in reg.by_kit.values() {
            assert!(
                items.contains(&format!("{},", def.kit_item)),
                "kit item {} missing from items.csv",
                def.kit_item
            );
            assert!(
                items.contains(&format!("{},", def.vehicle_item)),
                "vehicle item {} missing from items.csv",
                def.vehicle_item
            );
        }
    }
}

#[cfg(test)]
mod transit_tests {
    use super::*;
    use crate::ecs::components::{Name, Vehicle, VehicleRoute};
    use crate::systems::inventory::Inventory;

    const KITS_RON: &str = r#"[
        (
            kit_item: "rover_kit_0",
            vehicle_item: "rover_0",
            display_name: "Rover",
            body_m: (3.2, 0.8, 1.9),
            cabin_m: (1.5, 0.65, 1.7),
            cabin_offset_x: 0.2,
            wheel_radius_m: 0.5,
            speed_mps: 6.0,
        ),
    ]"#;

    fn make_store() -> DataStore {
        let mut data = DataStore::new();
        data.insert(
            "vehicle_kit_registry",
            VehicleKitRegistry::from_ron(KITS_RON.as_bytes()).expect("kits ron parses"),
        );
        data.insert("summon_vehicle", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("camera_position", Vec3::new(50.0, 1.7, 0.0));
        data.insert("camera_forward", Vec3::new(1.0, 0.0, 0.0));
        data.insert("camera_yaw", 0.0_f32);
        data
    }

    fn parked_rover(world: &mut hecs::World, pos: Vec3) -> hecs::Entity {
        world.spawn((
            Vehicle { item_id: "rover_0".to_string() },
            Transform { position: pos, rotation: Quat::IDENTITY, scale: Vec3::ONE },
            Velocity::default(),
            VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
            Name("Rover".to_string()),
        ))
    }

    /// THE STAGE 3 HEADLINE: summon a parked rover and it DRIVES itself across
    /// the world to the player -- moving each tick, facing its travel direction,
    /// and parking (route removed) once it pulls up nearby.
    #[test]
    fn summoned_vehicle_drives_to_the_player_and_parks() {
        let mut world = hecs::World::new();
        let data = make_store();
        // Player entity (deploy_pose fallback target; camera keys take priority).
        world.spawn((Controllable, Inventory::new(4), Transform::default()));
        let rover = parked_rover(&mut world, Vec3::ZERO);

        *data
            .get::<std::sync::Mutex<Option<u64>>>("summon_vehicle")
            .unwrap()
            .lock()
            .unwrap() = Some(rover.to_bits().into());

        let mut sys = VehicleSystem::new();
        sys.tick(&mut world, 1.0, &data); // summon consumed; route attached; first step
        {
            let route = world.get::<&VehicleRoute>(rover);
            assert!(route.is_ok(), "route attached on summon");
        }
        let after_1s = world.get::<&Transform>(rover).unwrap().position;
        assert!(
            (after_1s.x - 6.0).abs() < 1e-3,
            "moved 6 m (speed_mps) toward the player in 1 s, got {after_1s}"
        );
        // Faces travel direction (+X): rotated X axis points along +X.
        let fwd = world.get::<&Transform>(rover).unwrap().rotation * Vec3::X;
        assert!(fwd.x > 0.99, "yawed to face its travel direction");

        // Drive the rest of the way: dest is the camera ground spot 6 m past
        // the camera (deploy_pose = cam + fwd*6), ~56 m total.
        for _ in 0..12 {
            sys.tick(&mut world, 1.0, &data);
        }
        let parked = world.get::<&Transform>(rover).unwrap().position;
        let dest = Vec3::new(50.0 + 6.0, 0.0, 0.0);
        assert!(
            (parked - dest).length() <= 4.0 + 1e-3,
            "pulled up within arrive_radius of the player spot (at {parked})"
        );
        assert!(
            world.get::<&VehicleRoute>(rover).is_err(),
            "route removed on arrival -- the rover is parked again"
        );
    }

    /// Summoning garbage bits or a non-vehicle entity is refused without a
    /// crash, and summoning a vehicle already in transit doesn't reset it.
    #[test]
    fn summon_validates_its_target() {
        let mut world = hecs::World::new();
        let mut data = make_store();
        world.spawn((Controllable, Inventory::new(4), Transform::default()));
        let not_a_vehicle = world.spawn((Transform::default(),));

        let mut sys = VehicleSystem::new();
        *data
            .get::<std::sync::Mutex<Option<u64>>>("summon_vehicle")
            .unwrap()
            .lock()
            .unwrap() = Some(not_a_vehicle.to_bits().into());
        sys.tick(&mut world, 1.0, &data);
        assert!(
            world.get::<&VehicleRoute>(not_a_vehicle).is_err(),
            "non-vehicle entities cannot be summoned"
        );

        // In-transit re-summon keeps the existing route (dest unchanged).
        let rover = parked_rover(&mut world, Vec3::ZERO);
        *data
            .get::<std::sync::Mutex<Option<u64>>>("summon_vehicle")
            .unwrap()
            .lock()
            .unwrap() = Some(rover.to_bits().into());
        sys.tick(&mut world, 1.0, &data);
        let dest_before = world.get::<&VehicleRoute>(rover).unwrap().dest;
        data.insert("camera_position", Vec3::new(-100.0, 1.7, 0.0)); // player moved
        *data
            .get::<std::sync::Mutex<Option<u64>>>("summon_vehicle")
            .unwrap()
            .lock()
            .unwrap() = Some(rover.to_bits().into());
        sys.tick(&mut world, 1.0, &data);
        let dest_after = world.get::<&VehicleRoute>(rover).unwrap().dest;
        assert_eq!(dest_before, dest_after, "re-summon mid-transit is a no-op");
    }

    /// The shipped kits.ron carries a real transit speed for every vehicle.
    #[test]
    fn shipped_kits_have_transit_speeds() {
        let reg = VehicleKitRegistry::from_ron(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/vehicles/kits.ron"
        )))
        .expect("kits.ron parses");
        for def in reg.by_kit.values() {
            assert!(
                def.speed_mps > 0.0,
                "{} has a positive transit speed",
                def.vehicle_item
            );
        }
    }
}
