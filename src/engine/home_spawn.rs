use glam::{Quat, Vec3};

/// Spawn ONLY the electrical-role ECS entities for the home's machines (no meshes),
/// so SolarSystem + ElectricalSystem tick against the real home + publish a live
/// PowerStatus even in MENU mode (the Home page reads it, instead of authored
/// strings). load_world re-spawns these WITH meshes on Enter World after despawning
/// every HomeMachine, so there is no double-spawn. Silent no-op if home.ron is absent.
pub(crate) fn spawn_home_power_entities(world: &mut hecs::World, data_dir: &std::path::Path) {
    let path = crate::machines::home_ron_path(data_dir);
    let Some(home) = crate::machines::MachineHome::load(&path) else {
        return;
    };
    let all = home.all_instances();
    // Each machine carries its electrical + plumbing ISLAND so the sims flow per circuit. (v0.607/v0.608)
    let power_islands = home.electrical_islands(&all);
    let water_islands = home.water_islands(&all);
    for inst in &all {
        let Some(def) = home.catalog.get(&inst.machine) else {
            continue;
        };
        spawn_home_machine_entity(world, inst, def, &power_islands, &water_islands, None, None);
    }
    spawn_home_air_space(world);
}

/// Spawn the home's sealed AIR space (v0.617) if one doesn't already exist: a HomeMachine + HomeAir
/// tagged `EnclosedSpace` with an Earth-like atmosphere. The AtmosphereSystem ticks it + publishes the
/// live AirStatus. Sealed (a habitat/ship hull), so it doesn't equalize with the outside (space). The
/// HomeMachine tag means load_world's despawn-on-reenter clears it, then this re-creates it once.
pub(crate) fn spawn_home_air_space(world: &mut hecs::World) {
    use crate::ecs::components::HomeMachine;
    use crate::systems::atmosphere::{EnclosedSpace, HomeAir};
    if world.query::<&HomeAir>().iter().next().is_some() {
        return; // already present
    }
    world.spawn((HomeMachine, HomeAir, EnclosedSpace::new_sealed(14_000.0)));
}

/// Spawn ONE ECS entity for a placed home machine, attaching its power role + electrical island AND
/// its water role (producer / consumer / tank) + plumbing island (v0.608). One entity per machine so
/// the PlumbingSystem can gate a water producer/consumer on the SAME entity's power state (the
/// power -> water consequence chain). No-op if the machine has neither a power nor a water role.
/// Shared by `spawn_home_power_entities` (MENU mode) + `load_world` (in-world), so both stay in sync.
pub(crate) fn spawn_home_machine_entity(
    world: &mut hecs::World,
    inst: &crate::machines::MachineInstance,
    def: &crate::machines::MachineDef,
    power_islands: &std::collections::HashMap<String, u32>,
    water_islands: &std::collections::HashMap<String, u32>,
    // The machine's RESOLVED world position where the caller has one
    // (load_world's placement pass); None falls back to the instance's raw
    // offset, which IS absolute world coords in the HomeStructure box model
    // (menu mode has no resolve pass; corrected on Enter World when machines
    // despawn + respawn with resolved positions).
    world_pos: Option<Vec3>,
    // Typed-container archetypes (v0.728). None in MENU mode (the DataStore
    // isn't threaded there and those entities are despawned + respawned by
    // load_world before the walk-up cards can ever render them).
    containers: Option<&crate::systems::inventory::containers::ContainerRegistry>,
) {
    use crate::ecs::components::{
        Battery, HomeMachine, PlumbingCircuit, PowerCircuit, PowerConsumer, PowerGenerator, SolarPanel,
        WaterConsumer, WaterProducer, WaterTank,
    };
    use crate::machines::MachinePower;
    let is_water = def.is_water_machine();
    // Air OUT capacity (L/min) -- a scrubber/recycler that cleans the home air. (v0.618)
    let air_out: f32 = def
        .derive_ports()
        .iter()
        .filter(|p| p.utility == crate::utilities::Utility::Air && p.dir == crate::utilities::PortDir::Out)
        .map(|p| p.flow_lpm)
        .sum();
    if def.power.is_none()
        && !is_water
        && air_out <= 0.0
        && def.rf_emission <= 0.0
        && def.auto_recipe.is_none()
        && def.container_type.is_none()
    {
        return;
    }
    let e = world.spawn((HomeMachine,));
    // Instance id on the entity so the walk-up cards can look up THIS
    // machine's live component state per frame. (v0.724)
    let _ = world.insert_one(
        e,
        crate::ecs::components::MachineInstanceId(inst.id.clone()),
    );
    // Every machine entity carries its world pose (economy Phase 2 Stage 2,
    // v0.679): CraftingSystem captures it as the FACTORY PAD where a
    // vehicle-class craft output rolls out, and it anchors any future
    // per-machine spatial behavior.
    let _ = world.insert_one(
        e,
        crate::ecs::components::Transform {
            position: world_pos
                .unwrap_or_else(|| Vec3::new(inst.offset.0, inst.offset.1, inst.offset.2)),
            rotation: Quat::from_rotation_y(inst.rotation.to_radians()),
            scale: Vec3::ONE,
        },
    );
    if let Some(power) = &def.power {
        let _ = world.insert_one(e, PowerCircuit { island: power_islands.get(&inst.id).copied().unwrap_or(0) });
        match power {
            MachinePower::Solar { peak_watts } => {
                let _ = world.insert(
                    e,
                    (
                        PowerGenerator { output_watts: *peak_watts, fuel_per_second: 0.0, active: true },
                        SolarPanel { peak_watts: *peak_watts },
                    ),
                );
            }
            MachinePower::Generator { watts, fuel_lph } => {
                // fuel_per_second > 0 marks a backstop genset: the
                // ElectricalSystem gates it on need + drum fuel (v0.733).
                let _ = world.insert_one(
                    e,
                    PowerGenerator {
                        output_watts: *watts,
                        fuel_per_second: fuel_lph / 3600.0,
                        active: *fuel_lph <= 0.0,
                    },
                );
            }
            MachinePower::Consumer { watts, priority } => {
                let _ = world.insert_one(e, PowerConsumer { draw_watts: *watts, priority: *priority, enabled: true });
            }
            MachinePower::Battery { capacity_wh, max_charge_w, max_discharge_w } => {
                let _ = world.insert_one(
                    e,
                    Battery {
                        charge_wh: capacity_wh * 0.5,
                        capacity_wh: *capacity_wh,
                        max_charge_w: *max_charge_w,
                        max_discharge_w: *max_discharge_w,
                    },
                );
            }
        }
    }
    if is_water {
        let _ = world.insert_one(e, PlumbingCircuit { island: water_islands.get(&inst.id).copied().unwrap_or(0) });
        // Does this machine need power to move/produce water? Only true when it has a power-CONSUMER
        // role -- that is the only case where a `PowerConsumer` entity exists for the plumbing tick to
        // gate on. A machine that declares an electrical PORT but no Consumer role (e.g. a sun-lit
        // tower) has no PowerConsumer, so gating on it would silently freeze its water forever; treat
        // that water as ungated instead. (v0.608 fix)
        let needs_power = matches!(&def.power, Some(MachinePower::Consumer { .. }));
        let cap = def.water_capacity_l();
        if cap > 0.0 {
            let _ = world.insert_one(e, WaterTank { liters: cap * 0.5, capacity_l: cap });
        }
        let prod = def.water_production_lpm();
        if prod > 0.0 {
            let _ = world.insert_one(e, WaterProducer { lpm: prod, needs_power });
        }
        let dem = def.water_demand_lpm();
        if dem > 0.0 {
            let _ = world.insert_one(e, WaterConsumer { lpm: dem, needs_power });
        }
    }
    // AIR handler (v0.618): a machine with an Air OUT port scrubs the home air while powered.
    if air_out > 0.0 {
        let _ = world.insert_one(
            e,
            crate::systems::atmosphere::AirScrubber {
                o2_regen_per_s: air_out * 0.001,
                co2_scrub_per_s: air_out * 0.0003,
                needs_power: matches!(&def.power, Some(MachinePower::Consumer { .. })),
            },
        );
    }
    // RF emitter (v0.620): a wireless device (WiFi router) bathes the home in RF while powered.
    if def.rf_emission > 0.0 {
        let _ = world.insert_one(
            e,
            crate::ecs::components::RfEmitter {
                strength: def.rf_emission,
                needs_power: matches!(&def.power, Some(MachinePower::Consumer { .. })),
            },
        );
    }
    // Economy automation (v0.663): a machine with an `auto_recipe` continuously
    // runs that recipe against the home inventory (CraftingSystem's AutoRefine
    // arm). The smelter auto-smelts drone-delivered ore; the workbench turns the
    // ingots into tools -- the operator's living-ecosystem loop.
    if let Some(recipe_id) = &def.auto_recipe {
        let _ = world.insert_one(
            e,
            crate::ecs::components::AutoRefine { recipe_id: recipe_id.clone() },
        );
    }
    // Typed container (v0.728, "containers show contents"): the machine IS
    // a volume-capped, content-class-typed vessel (grain silo, fuel drum).
    // First runtime spawn of the containers.rs system — it was tests-only
    // until now. The walk-up card reads this component's live fill.
    if let Some(ct) = &def.container_type {
        match containers.and_then(|r| r.container_type(ct)) {
            Some(t) => {
                let _ = world.insert_one(
                    e,
                    crate::systems::inventory::containers::Container::from_type(t),
                );
            }
            None => {
                if containers.is_some() {
                    log::warn!(
                        "machine {} declares unknown container_type '{}' (not in data/containers/types.csv)",
                        inst.id, ct
                    );
                }
            }
        }
    }
}
