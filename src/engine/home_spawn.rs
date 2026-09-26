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
        && !def.pollinates_crops
        && def.ventilation_m3_h <= 0.0
        && def.humidifies_l_h <= 0.0
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
    let _ = world.insert_one(e, crate::ecs::components::MachineType(inst.machine.clone()));
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
            MachinePower::Consumer { watts, priority, idle_watts } => {
                // A work station starts idle; the crafting system raises it
                // to its working draw while a craft runs (2026-09-26).
                let draw = idle_watts.unwrap_or(*watts);
                let _ = world.insert_one(e, PowerConsumer { draw_watts: draw, priority: *priority, enabled: true });
                if let Some(idle) = idle_watts {
                    let _ = world.insert_one(
                        e,
                        crate::ecs::components::StationLoad { active_watts: *watts, idle_watts: *idle },
                    );
                }
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
        if def.irrigates {
            let _ = world.insert_one(e, crate::ecs::components::Irrigator);
        }
    }
    // Grow light (2026-09-26): FarmingSystem counts the indoor grow areas as
    // lit while this entity's PowerConsumer is enabled. Outside the water
    // block above because a light has no water role; it always has a power
    // role (the machines.rs data test insists), so this entity exists.
    if def.lights_crops {
        let _ = world.insert_one(e, crate::ecs::components::GrowLight);
    }
    // Bumblebee hive (2026-09-26): FarmingSystem pollinates the indoor grow
    // areas around this entity's Transform (farming::pollination).
    if def.pollinates_crops {
        let _ = world.insert_one(e, crate::ecs::components::PollinatorHive);
    }
    // Exhaust fan (2026-09-26): FarmingSystem runs it to hold the humidity of
    // the grow room around this entity's Transform (farming::humidity). Its
    // full-speed draw is its Consumer watts; the controller scales it.
    if def.ventilation_m3_h > 0.0 {
        let watts = match &def.power {
            Some(MachinePower::Consumer { watts, .. }) => *watts,
            _ => 0.0,
        };
        let _ = world.insert_one(e, crate::ecs::components::Ventilator { airflow_m3_h: def.ventilation_m3_h, watts });
    }
    // Humidifier (2026-09-26): FarmingSystem runs it to hold the humidity of
    // the grow room around this entity's Transform, drawing its litres from
    // the tanks with the irrigation (farming::humidity). Its full-output draw
    // is its Consumer watts; the controller scales it with the output.
    if def.humidifies_l_h > 0.0 {
        let watts = match &def.power {
            Some(MachinePower::Consumer { watts, .. }) => *watts,
            _ => 0.0,
        };
        let _ = world.insert_one(e, crate::ecs::components::Humidifier { output_l_h: def.humidifies_l_h, watts });
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
            crate::ecs::components::AutoRefine { recipe_id: recipe_id.clone(), keep: def.auto_keep },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Gardening depth, rung 2 (2026-09-26): a grow light placed from either
    /// shipped catalog spawns as a GrowLight with a PowerConsumer, which is
    /// exactly the pair FarmingSystem reads to decide whether the indoor
    /// garden is lit. The irrigation machine next to it gets no GrowLight.
    #[test]
    fn shipped_grow_light_spawns_a_powered_grow_light() {
        use crate::ecs::components::{GrowLight, MachineInstanceId, PowerConsumer};
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join("machines")
                .join(file);
            let home = crate::machines::MachineHome::load(&path)
                .unwrap_or_else(|| panic!("{file} parses"));
            let mut world = hecs::World::new();
            let empty = std::collections::HashMap::new();
            for (id, machine) in [("gl_test", "grow_light"), ("irr_test", "irrigation_system")] {
                let inst = crate::machines::MachineInstance {
                    id: id.to_string(),
                    machine: machine.to_string(),
                    room: "room-greenhouse".to_string(),
                    offset: (0.0, 0.0, 0.0),
                    rotation: 0.0,
                    zone: "home".to_string(),
                    screen_source: None,
                };
                let def = home
                    .catalog
                    .get(machine)
                    .unwrap_or_else(|| panic!("{file} catalogs {machine}"));
                spawn_home_machine_entity(&mut world, &inst, def, &empty, &empty, None, None);
            }
            let lit: Vec<String> = world
                .query::<(&GrowLight, &PowerConsumer, &MachineInstanceId)>()
                .iter()
                .map(|(_, (_, _, id))| id.0.clone())
                .collect();
            assert_eq!(lit, vec!["gl_test".to_string()], "{file}: only the grow light lights crops");
        }
    }

    /// Greenhouse humidity (2026-09-26): the exhaust fan in either shipped
    /// catalog spawns as a Ventilator carrying its airflow and full-speed
    /// watts, with a PowerConsumer, at its Transform: the three FarmingSystem
    /// reads to run it (farming::humidity). Seen red by not inserting the
    /// Ventilator (the fan then spawned as a plain 250 W load).
    #[test]
    fn shipped_exhaust_fan_spawns_a_ventilator() {
        use crate::ecs::components::{MachineInstanceId, PowerConsumer, Transform, Ventilator};
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
            let home = crate::machines::MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            let mut world = hecs::World::new();
            let empty = std::collections::HashMap::new();
            let inst = crate::machines::MachineInstance {
                id: "fan_test".to_string(),
                machine: "exhaust_fan".to_string(),
                room: "room-greenhouse".to_string(),
                offset: (54.5, 2.2, 62.0),
                rotation: 0.0,
                zone: "home".to_string(),
                screen_source: None,
            };
            spawn_home_machine_entity(&mut world, &inst, &home.catalog["exhaust_fan"], &empty, &empty, None, None);
            let found: Vec<(String, f32, f32, bool, [f32; 3])> = world
                .query::<(&Ventilator, &PowerConsumer, &MachineInstanceId, &Transform)>()
                .iter()
                .map(|(_, (v, p, id, t))| (id.0.clone(), v.airflow_m3_h, v.watts, p.enabled, t.position.to_array()))
                .collect();
            assert_eq!(found, vec![("fan_test".to_string(), 2725.0, 250.0, true, [54.5, 2.2, 62.0])], "{file}");
        }
    }

    /// The mushroom racks (2026-09-26): both humidifiers in either shipped
    /// catalog (the room T7 and the tent T3) spawn as a Humidifier carrying
    /// their output and full-output watts, with an enabled PowerConsumer, at
    /// their Transform: what FarmingSystem reads to run them
    /// (farming::humidity). Seen red by not inserting the Humidifier (each
    /// then spawned as a plain load).
    #[test]
    fn shipped_humidifier_spawns_a_humidifier() {
        use crate::ecs::components::{Humidifier, MachineInstanceId, PowerConsumer, Transform};
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
            let home = crate::machines::MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            for (machine, l_h, watts) in [("humidifier", 1.3, 100.0), ("tent_humidifier", 0.24, 24.0)] {
                let mut world = hecs::World::new();
                let empty = std::collections::HashMap::new();
                let inst = crate::machines::MachineInstance {
                    id: "hum_test".to_string(),
                    machine: machine.to_string(),
                    room: "room-mushroom".to_string(),
                    offset: (2.0, 0.0, 53.0),
                    rotation: 0.0,
                    zone: "home".to_string(),
                    screen_source: None,
                };
                spawn_home_machine_entity(&mut world, &inst, &home.catalog[machine], &empty, &empty, None, None);
                let found: Vec<(String, f32, f32, bool, [f32; 3])> = world
                    .query::<(&Humidifier, &PowerConsumer, &MachineInstanceId, &Transform)>()
                    .iter()
                    .map(|(_, (h, p, id, t))| (id.0.clone(), h.output_l_h, h.watts, p.enabled, t.position.to_array()))
                    .collect();
                assert_eq!(found, vec![("hum_test".to_string(), l_h, watts, true, [2.0, 0.0, 53.0])], "{file} {machine}");
            }
        }
    }

    /// Pollination (2026-09-26): the bumblebee hive in either shipped
    /// catalog spawns as a PollinatorHive at its Transform, the pair
    /// FarmingSystem reads to decide which grow areas the bees reach. A hive
    /// has no power, water or air role, so without its own case it would
    /// spawn no entity at all. No other catalog machine is a hive. Seen red
    /// by dropping `pollinates_crops` from the no-entity test above (the
    /// hive then spawned nothing).
    #[test]
    fn shipped_bumblebee_hive_spawns_a_hive() {
        use crate::ecs::components::{MachineInstanceId, PollinatorHive, Transform};
        for file in ["home.ron", "home_solo.ron"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
            let home = crate::machines::MachineHome::load(&path).unwrap_or_else(|| panic!("{file} parses"));
            let hives: Vec<&String> = home.catalog.iter().filter(|(_, d)| d.pollinates_crops).map(|(id, _)| id).collect();
            assert_eq!(hives, vec!["bumblebee_hive"], "{file}: the hive and nothing else pollinates");
            assert!(
                !home.instances.iter().any(|i| i.machine == "bumblebee_hive"),
                "{file}: the seed design places no hive (hand pollination until the player chooses bees)"
            );
            let mut world = hecs::World::new();
            let empty = std::collections::HashMap::new();
            let inst = crate::machines::MachineInstance {
                id: "hive_test".to_string(),
                machine: "bumblebee_hive".to_string(),
                room: "room-greenhouse".to_string(),
                offset: (2.0, 0.0, 3.0),
                rotation: 0.0,
                zone: "home".to_string(),
                screen_source: None,
            };
            spawn_home_machine_entity(&mut world, &inst, &home.catalog["bumblebee_hive"], &empty, &empty, None, None);
            let found: Vec<(String, [f32; 3])> = world
                .query::<(&PollinatorHive, &MachineInstanceId, &Transform)>()
                .iter()
                .map(|(_, (_, id, t))| (id.0.clone(), t.position.to_array()))
                .collect();
            assert_eq!(found, vec![("hive_test".to_string(), [2.0, 0.0, 3.0])], "{file}");
        }
    }
}
