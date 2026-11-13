//! The helpers other crates' tests share with the farm's own tests (the engine's save, spawn and
//! home tests in src/engine/crossing_tests/farming_*.rs): a module of their own, apart from the
//! tests, so that the crate split can put them behind the `test-support` feature when the
//! simulation becomes a crate of its own (rung 2, docs/design/crate-split.md: a crate's
//! #[cfg(test)] items never reach another crate's tests). One module per test file they came from.

/// The store every farm test starts from, and the garden's speed (from mod.rs's `gardening_tests`).
pub mod gardening {
    use super::super::*;
    use humanity_core::hot_reload::data_store::DataStore;
    use crate::inventory::ItemRegistry;

    /// The garden test helpers' `speed`: garden days per 1,200 real
    /// seconds, what a growth speed of 1 meant on the 20-minute day. Set as
    /// the clock's time scale, the one speed there is now (2026-09-27), so
    /// the tests keep their garden days a tick. At 1 it is 72 game seconds a
    /// real second.
    pub fn set_garden_speed(data: &DataStore, speed: f32) {
        data.get::<std::sync::Mutex<crate::time::GameTime>>("game_time")
            .unwrap()
            .lock()
            .unwrap()
            .time_scale = speed * (SECONDS_PER_DAY / 1200.0) as f32;
    }

    /// DataStore with plant + item registries and the four gardening channels,
    /// mirroring the runtime wiring in lib.rs. Shared with nutrient_tests.
    pub fn make_store() -> DataStore {
        let mut data = DataStore::new();
        let plants = PlantRegistry::from_csv(include_bytes!(concat!(
            env!("HUMANITY_REPO_ROOT"),
            "/data/plants.csv"
        )))
        .expect("plants.csv");
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("HUMANITY_REPO_ROOT"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        data.insert("plant_registry", plants);
        data.insert("item_registry", items);
        // The clock at 72 game seconds a real second: the pace these tests
        // were written at, when a garden day was 1,200 real seconds and a
        // crop's water and health ran on real seconds (the one clock,
        // 2026-09-27, made both game time; see set_garden_speed).
        data.insert(
            "game_time",
            std::sync::Mutex::new(crate::time::GameTime {
                time_scale: (SECONDS_PER_DAY / 1200.0) as f32,
                ..Default::default()
            }),
        );
        data.insert("plant_request", std::sync::Mutex::new(Option::<String>::None));
        data.insert(
            "plant_tower_request",
            std::sync::Mutex::new(Option::<(String, Vec<String>)>::None),
        );
        data.insert("water_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("harvest_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("dev_grow_crops", std::sync::Mutex::new(false));
        data.insert(
            "fertilize_crop_request",
            std::sync::Mutex::new(Option::<u64>::None),
        );
        data.insert(
            "stock_seeds_request",
            std::sync::Mutex::new(Option::<Vec<String>>::None),
        );
        data.insert(
            "plant_bed_request",
            std::sync::Mutex::new(Option::<(String, String, u32)>::None),
        );
        data.insert("harvest_many_request", std::sync::Mutex::new(Vec::<u64>::new()));
        // One tower the light tests stand a grow light over (lighting.rs).
        data.insert("garden_lighting", lighting::LightingData::parse(lighting::LIGHTING_RON).unwrap());
        data.insert(
            "grow_plots",
            vec![lighting::GrowPlot { id: "ntower_3".into(), cups: 12, ..Default::default() }],
        );
        data
    }
}

/// From heat_tests.rs: a grow room, a space heater and the air they share.
pub mod heat {
    use std::sync::Mutex;

    use super::gardening::make_store;
    use super::super::humidity::{self, GrowRoom, HumidityData, HUMIDITY_RON};
    use super::super::lighting::GrowPlot;
    use super::super::*;
    use humanity_core::ecs::components::{PowerConsumer, RoomAir, SoilMemory, SpaceHeater, Transform};
    use humanity_core::ecs::systems::System;
    use humanity_core::hot_reload::data_store::DataStore;

    pub fn air() -> HumidityData {
        HumidityData::parse(HUMIDITY_RON).expect("the shipped humidity.ron parses")
    }

    /// The channels the farming tick reads, the shipped air data, no pests, and
    /// the given room boxes and grow plots.
    pub fn store(rooms: Vec<GrowRoom>, plots: Vec<GrowPlot>) -> DataStore {
        let mut data = make_store();
        data.insert("garden_pest_severity", Mutex::new(0.0_f32));
        crate::notices::register(&mut data);
        data.insert("hand_water_draw_l", Mutex::new(0.0_f32));
        data.insert("pest_control_request", Mutex::new(Option::<(String, String)>::None));
        data.insert(humidity::DATA_KEY, air());
        data.insert(humidity::ROOMS_KEY, Mutex::new(rooms));
        data.insert("grow_plots", plots);
        data
    }

    /// A grow room `id`, `w` x `h` x `w` metres from the origin, with one grow bed
    /// ("bed_a") standing in it: the bed is what makes it a room with its own air.
    pub fn one_room(id: &str, w: f32, h: f32) -> DataStore {
        let room = GrowRoom { id: id.into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [w, h, w], ..Default::default() };
        let bed = GrowPlot { id: "bed_a".into(), pos: [w / 2.0, 0.0, w / 2.0], ..Default::default() };
        store(vec![room], vec![bed])
    }

    /// A space heater at `at`, `watts` (a resistance heater: its heat is its
    /// draw), its thermostat at `setpoint_c`, with or without power.
    pub fn heater(world: &mut hecs::World, at: [f32; 3], watts: f32, setpoint_c: f32, powered: bool) -> hecs::Entity {
        world.spawn((
            SpaceHeater { heat_w: watts, watts, setpoint_c },
            Transform { position: glam::Vec3::from_array(at), ..Default::default() },
            PowerConsumer { draw_watts: watts, priority: 4, enabled: powered },
        ))
    }

    /// `ticks` farming ticks of `dt` real seconds (the store's clock runs 72 game
    /// seconds a real second, so 5 s is a tenth of a game hour).
    pub fn run(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, ticks: usize, dt: f32) {
        for _ in 0..ticks {
            sys.tick(world, dt, data);
        }
    }

    pub fn memory(world: &hecs::World) -> SoilMemory {
        world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
    }

    pub fn room(world: &hecs::World, id: &str) -> RoomAir {
        memory(world).rooms.get(id).copied().unwrap_or_default()
    }

    pub fn draw(world: &hecs::World, e: hecs::Entity) -> f32 {
        world.get::<&PowerConsumer>(e).unwrap().draw_watts
    }

    /// A cubic metre of air's heat capacity, J/K, at `t_c`, BY HAND: the density
    /// FAO-56's way, P / (R T) with P 101.3 kPa and R 287 J/(kg K) (humidity.ron),
    /// times its cp, 1,013 J/(kg K).
    pub fn air_j_m3_k(t_c: f64) -> f64 {
        101_300.0 / (287.0 * (t_c + 273.15)) * 1013.0
    }

    /// A well-watered `plant` on unit `slot` of `area`, at its first stage: the
    /// Garden panel only has a line for an area with a living crop.
    pub fn crop(data: &DataStore, plant: &str, area: &str, slot: u32) -> humanity_core::ecs::components::CropInstance {
        let stage = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().first_stage().to_string();
        humanity_core::ecs::components::CropInstance {
            crop_def_id: plant.to_string(),
            growth_stage: stage,
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(area.to_string()),
            tower_slot: Some(slot),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }
}

/// From humidity_tests.rs: two grow rooms and the shipped humidity data.
pub mod humidity {
    use std::sync::Mutex;

    use super::gardening::make_store;
    use super::super::humidity::{self, GrowRoom, HumidityData, HUMIDITY_RON};
    use super::super::lighting::GrowPlot;
    use super::super::*;
    use humanity_core::ecs::components::{CropInstance, RoomAir, SoilMemory};
    use humanity_core::hot_reload::data_store::DataStore;

    /// A well-watered crop of `plant` in unit `slot` of grow area `area`, at its
    /// first stage, planted at game second 0.
    pub fn crop(data: &DataStore, plant: &str, area: &str, slot: u32) -> CropInstance {
        let stage = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().first_stage().to_string();
        CropInstance {
            crop_def_id: plant.to_string(),
            growth_stage: stage,
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(area.to_string()),
            tower_slot: Some(slot),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }

    /// Two 300 m3 grow rooms side by side, the "Greenhouse" (bed_a, bed_c) and
    /// the "Plant room" (bed_b), with the channels the farming tick reads, the
    /// shipped humidity data given `base` air changes an hour, at `speed` growth
    /// and pest `severity`.
    pub fn store(base: f64, speed: f32, severity: f32) -> DataStore {
        let mut data = make_store();
        super::gardening::set_garden_speed(&data, speed);
        data.insert("garden_pest_severity", Mutex::new(severity));
        crate::notices::register(&mut data);
        data.insert("hand_water_draw_l", Mutex::new(0.0_f32));
        data.insert("pest_control_request", Mutex::new(Option::<(String, String)>::None));
        let mut d = shipped_air();
        d.base_air_changes_per_hour = base;
        data.insert(humidity::DATA_KEY, d);
        data.insert(
            humidity::ROOMS_KEY,
            Mutex::new(vec![
                GrowRoom { id: "room-a".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [10.0, 3.0, 10.0], ..Default::default() },
                GrowRoom { id: "room-b".into(), name: "Plant room".into(), min: [20.0, 0.0, 0.0], max: [30.0, 3.0, 10.0], ..Default::default() },
            ]),
        );
        data.insert(
            "grow_plots",
            vec![
                GrowPlot { id: "bed_a".into(), pos: [3.0, 0.0, 5.0], ..Default::default() },
                GrowPlot { id: "bed_c".into(), pos: [7.0, 0.0, 5.0], ..Default::default() },
                GrowPlot { id: "bed_b".into(), pos: [25.0, 0.0, 5.0], ..Default::default() },
            ],
        );
        data
    }

    pub fn shipped_air() -> HumidityData {
        HumidityData::parse(HUMIDITY_RON).expect("the shipped humidity.ron parses")
    }

    pub fn memory(world: &hecs::World) -> SoilMemory {
        world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
    }

    pub fn room(world: &hecs::World, id: &str) -> RoomAir {
        memory(world).rooms.get(id).copied().unwrap_or_default()
    }
}

/// From life_support_tests.rs: the home's air and water through the farming tick.
pub mod life_support {
    use std::sync::Mutex;

    use super::gardening::make_store;
    use super::super::humidity::{self, GrowRoom, HumidityData, HUMIDITY_RON};
    use super::super::lighting::GrowPlot;
    use super::super::*;
    use humanity_core::ecs::components::{
        AirHandler, CropInstance, HomeAirState, SoilMemory, WaterProducer,
    };
    use humanity_core::hot_reload::data_store::DataStore;
    
    use crate::life_support::{self, LifeSupportData, LIFE_SUPPORT_RON};
    use crate::plumbing::WaterStatus;

    pub fn life() -> LifeSupportData {
        LifeSupportData::parse(LIFE_SUPPORT_RON).expect("the shipped life_support.ron parses")
    }

    pub fn air() -> HumidityData {
        HumidityData::parse(HUMIDITY_RON).expect("the shipped humidity.ron parses")
    }

    /// The channels the farming and plumbing ticks read, with both air models'
    /// data, the given room boxes and grow plots, growth at 1x, no pests.
    pub fn store(rooms: Vec<GrowRoom>, plots: Vec<GrowPlot>) -> DataStore {
        let mut data = make_store();
        data.insert("garden_pest_severity", Mutex::new(0.0_f32));
        crate::notices::register(&mut data);
        data.insert("hand_water_draw_l", Mutex::new(0.0_f32));
        data.insert("irrigation_demand_lpm", Mutex::new(0.0_f32));
        data.insert("water_status", Mutex::new(WaterStatus::default()));
        data.insert("pest_control_request", Mutex::new(Option::<(String, String)>::None));
        data.insert(humidity::DATA_KEY, air());
        data.insert(life_support::DATA_KEY, life());
        data.insert(humidity::ROOMS_KEY, Mutex::new(rooms));
        data.insert("grow_plots", plots);
        data
    }

    /// The game seconds in one second of the 20-minute day these tests were
    /// written on (the one clock, 2026-09-27: a day is 86,400 game seconds).
    pub const OLD_S: f64 = 72.0;

    /// Advance the game clock by `old_s` seconds of the 20-minute day, 72 game
    /// seconds each (the hour follows): advance(600) is still noon.
    pub fn advance(data: &DataStore, old_s: f64) {
        let gt = data.get::<Mutex<crate::time::GameTime>>("game_time").unwrap();
        let mut g = gt.lock().unwrap();
        let t = g.elapsed_seconds + old_s * OLD_S;
        g.set_elapsed(t);
    }

    pub fn memory(world: &hecs::World) -> SoilMemory {
        world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
    }

    pub fn home(world: &hecs::World) -> HomeAirState {
        memory(world).home_air
    }

    pub fn demand_lpm(data: &DataStore) -> f64 {
        f64::from(*data.get::<Mutex<f32>>("irrigation_demand_lpm").unwrap().lock().unwrap())
    }

    /// The condensate the air handlers are handing the tanks, L/min.
    pub fn returned_lpm(world: &hecs::World) -> f64 {
        world.query::<(&AirHandler, &WaterProducer)>().iter().map(|(_, (_, p))| f64::from(p.lpm)).sum()
    }

    /// A crop of `plant` on unit `slot` of `area`, just sown and well watered.
    pub fn crop(data: &DataStore, plant: &str, area: &str, slot: u32, planted_at: f64) -> CropInstance {
        let stage = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().first_stage().to_string();
        CropInstance {
            crop_def_id: plant.to_string(),
            growth_stage: stage,
            planted_at,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(area.to_string()),
            tower_slot: Some(slot),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }
}

/// From soil_ph_tests.rs: a unit's pH and the store pH runs in.
pub mod soil_ph {
    use std::sync::Mutex;

    use super::gardening::make_store;
    use super::super::soil_ph::{self, SoilPhData};
    use super::super::*;
    use humanity_core::ecs::components::{CropInstance, SoilMemory, UnitPh};
    use humanity_core::hot_reload::data_store::DataStore;

    pub fn shipped() -> SoilPhData {
        SoilPhData::parse(soil_ph::SOIL_PH_RON).expect("the shipped soil_ph.ron parses")
    }

    /// A store with the channels pH uses, pests off (so only pH moves health),
    /// crops at 100x growth, soil pH Realistic when `on`, else Off.
    pub fn store(on: bool) -> DataStore {
        store_at(if on { 1.0 } else { 0.0 })
    }

    /// `store` at the soil pH level `severity` (0 Off, 0.5 Gentle, 1 Realistic).
    pub fn store_at(severity: f32) -> DataStore {
        let mut data = make_store();
        data.insert("garden_pest_severity", Mutex::new(0.0_f32));
        crate::notices::register(&mut data);
        // Time speed 1 (the one clock, 2026-09-27): a real second is a game second.
        data.get::<Mutex<crate::time::GameTime>>("game_time").unwrap().lock().unwrap().time_scale = 1.0;
        soil_ph::register(&mut data);
        *data.get::<Mutex<f32>>(soil_ph::MODE_KEY).unwrap().lock().unwrap() = severity;
        data
    }

    /// A well-watered crop of `plant` in unit `slot` of `area`, at its first
    /// stage, planted at game second 0.
    pub fn crop(data: &DataStore, plant: &str, area: &str, slot: u32) -> CropInstance {
        let stage = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().first_stage().to_string();
        CropInstance {
            crop_def_id: plant.to_string(),
            growth_stage: stage,
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(area.to_string()),
            tower_slot: Some(slot),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }

    pub fn memory(world: &hecs::World) -> SoilMemory {
        world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
    }

    pub fn unit(world: &hecs::World, area: &str, slot: u32) -> Option<UnitPh> {
        memory(world).ph.get(area).and_then(|m| m.get(&slot)).cloned()
    }

    /// Give unit `slot` of `area` a remembered pH (and pending pools).
    pub fn set_unit(world: &mut hecs::World, area: &str, slot: u32, ph: f64, pending: &[(&str, f64)]) {
        let e = soil::soil_memory_entity(world);
        let mut m = world.get::<&mut SoilMemory>(e).unwrap();
        m.ph.entry(area.to_string()).or_default().insert(
            slot,
            UnitPh { ph, pending: pending.iter().map(|(k, v)| (k.to_string(), *v)).collect() },
        );
    }
}

/// From weed_tests.rs: the soil's memory, where an area's weeds live.
pub mod weeds {
    use humanity_core::ecs::components::SoilMemory;

    pub fn memory(world: &hecs::World) -> SoilMemory {
        world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
    }
}

/// From golden_tests.rs: the golden garden's store and world (every plant sown in every area, two
/// grow rooms, the home's air), which the step and clock tests and the engine's save tests start
/// from.
pub mod golden {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::gardening::{make_store, set_garden_speed};
    use super::super::humidity::{self, GrowRoom, HumidityData, HUMIDITY_RON};
    use super::super::lighting::GrowPlot;
    use super::super::*;
    use humanity_core::ecs::components::{
        AirHandler, CropInstance, Irrigator, PlumbingCircuit, PowerConsumer, Transform, WaterConsumer, WaterProducer,
        WaterTank,
    };
    use humanity_core::hot_reload::data_store::DataStore;
    use crate::atmosphere::{EnclosedSpace, HomeAir};
    use crate::life_support::{self, LifeSupportData, LIFE_SUPPORT_RON};
    use crate::plumbing::WaterStatus;

    /// The clock's speed in the 20-minute day's units (set_garden_speed): growth fast enough that
    /// crops cross stages, pests build and weeds come within TICKS ticks of one real second each.
    pub const GARDEN_SPEED: f32 = 400.0;
    /// The areas every plant is sown in: a bed in each of two grow rooms, a tower slot, and an outdoor
    /// field (a name ending `_field_<n>` is a field, `is_field_area`).
    pub const AREAS: [&str; 4] = ["bed_a", "bed_b", "aeroponic_tower_1", "veg_field_1"];

    /// The channels the tick reads, two grow rooms of different size, the four areas as plots, pests
    /// and weeds at a middling severity.
    pub fn store() -> DataStore {
        let mut data = make_store();
        set_garden_speed(&data, GARDEN_SPEED);
        data.insert("garden_pest_severity", Mutex::new(0.5_f32));
        crate::notices::register(&mut data);
        data.insert("hand_water_draw_l", Mutex::new(0.0_f32));
        data.insert("irrigation_demand_lpm", Mutex::new(0.0_f32));
        data.insert("water_status", Mutex::new(WaterStatus::default()));
        data.insert("pest_control_request", Mutex::new(Option::<(String, String)>::None));
        data.insert(super::super::weeds::REQUEST_KEY, Mutex::new(Option::<(String, String)>::None));
        data.insert(humidity::DATA_KEY, HumidityData::parse(HUMIDITY_RON).expect("humidity.ron"));
        data.insert(life_support::DATA_KEY, LifeSupportData::parse(LIFE_SUPPORT_RON).expect("life_support.ron"));
        let rooms = vec![
            GrowRoom { id: "room-a".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [10.0, 3.0, 10.0], ..Default::default() },
            GrowRoom { id: "room-b".into(), name: "Grow room".into(), min: [20.0, 0.0, 0.0], max: [24.0, 2.5, 6.0], ..Default::default() },
        ];
        data.insert(humidity::ROOMS_KEY, Mutex::new(rooms));
        let plots = vec![
            GrowPlot { id: "bed_a".into(), pos: [5.0, 0.0, 5.0], footprint_m2: 40.0, ..Default::default() },
            GrowPlot { id: "bed_b".into(), pos: [22.0, 0.0, 3.0], footprint_m2: 12.0, ..Default::default() },
            GrowPlot { id: "aeroponic_tower_1".into(), pos: [2.0, 0.0, 2.0], footprint_m2: 1.0, ..Default::default() },
            GrowPlot { id: "veg_field_1".into(), pos: [60.0, 0.0, 60.0], footprint_m2: 14.4, ..Default::default() },
        ];
        data.insert("grow_plots", plots);
        let areas: HashMap<String, f32> =
            [("bed_a", 10.0_f32), ("bed_b", 3.0), ("aeroponic_tower_1", 0.25), ("veg_field_1", 1.44)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        data.insert(super::super::units::PLOT_AREA_KEY, areas);
        let instances: HashMap<String, Vec<(String, u32)>> =
            [("veg_field".to_string(), vec![("veg_field_1".to_string(), 10)])].into_iter().collect();
        data.insert("grow_instances", instances);
        data
    }

    /// Every plant in the registry, in id order, sown in every area on its own slot, the sowing times
    /// spread so the garden holds every stage at once. Plus the home's air, two air handlers, the
    /// irrigation and a cistern, as the life-support tests build them.
    pub fn world(data: &DataStore) -> (hecs::World, usize) {
        let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
        let mut ids: Vec<String> = reg.plants.keys().cloned().collect();
        ids.sort();
        let mut world = hecs::World::new();
        world.spawn((HomeAir { metabolic_kcal_per_day: 0.0, ..Default::default() }, EnclosedSpace::new_sealed(2000.0)));
        let mut sown = 0;
        for (i, plant) in ids.iter().enumerate() {
            let def = reg.get(plant).unwrap();
            for area in AREAS {
                world.spawn((CropInstance {
                    crop_def_id: plant.clone(),
                    growth_stage: def.first_stage().to_string(),
                    planted_at: -(i as f64 % 9.0) * 3_600.0,
                    water_level: 0.4 + (i % 7) as f32 * 0.1,
                    health: 100.0,
                    tower_id: Some(area.to_string()),
                    tower_slot: Some(i as u32),
                    health_seconds: 0.0,
                    growing_seconds: 0.0,
                },));
                sown += 1;
            }
        }
        for at in [[5.0, 2.7, 1.0], [22.0, 2.2, 3.0]] {
            let e = world.spawn((Transform { position: glam::Vec3::from_array(at), ..Default::default() },));
            world
                .insert(
                    e,
                    (
                        AirHandler { airflow_m3_h: 1842.0, watts: 325.0 },
                        PowerConsumer { draw_watts: 0.0, priority: 2, enabled: true },
                        WaterProducer { lpm: 0.0, needs_power: true },
                        PlumbingCircuit { island: 0 },
                    ),
                )
                .unwrap();
        }
        world.spawn((Irrigator, WaterConsumer { lpm: 0.0, needs_power: false }, PlumbingCircuit { island: 0 }));
        world.spawn((WaterTank { liters: 7000.0, capacity_l: 20000.0 }, PlumbingCircuit { island: 0 }));
        (world, sown)
    }
}

/// From step_tests.rs: the golden garden on the real-time clock, where a run ended, and how two
/// ends differ (the clock tests and the engine's save and Garden-page tests use them too).
pub mod step {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Mutex;

    use super::golden::{store as golden_store, world as golden_world};
    use super::super::*;
    use humanity_core::ecs::components::{CropInstance, CropSoil, GrowLight, PowerConsumer, Transform};
    use humanity_core::hot_reload::data_store::DataStore;

    /// Where the clock starts: 05:20:34.5 on day 0, so the first hour-long step spans the sunrise.
    pub const START_S: f64 = 5.0 * 3600.0 + 1234.5;
    /// The tick every run takes first, game seconds (see the module comment).
    pub const WARM_S: f64 = 1.0;
    /// The golden garden on the real-time clock, made to cross every regime (see the module comment).
    /// `air`: with the golden garden's grow rooms and the home's air space, or with no air tracked.
    pub fn garden(air: bool) -> (DataStore, hecs::World) {
        let mut data = golden_store();
        {
            let gt = data.get::<Mutex<crate::time::GameTime>>("game_time").unwrap();
            let mut g = gt.lock().unwrap();
            g.time_scale = 1.0;
            g.set_elapsed(START_S);
        }
        data.insert("garden_irrigation", Mutex::new(HashMap::from([("bed_b".to_string(), 0.15_f32)])));
        data.insert("garden_nutrient", Mutex::new(HashMap::from([("bed_a".to_string(), 0.25_f32)])));
        let doses = soil::NutrientData::parse(soil::NUTRIENTS_RON).expect("nutrients.ron");
        let stock: HashMap<String, u32> = doses.fertilizers.iter().map(|f| (f.item.clone(), 1_000_000)).collect();
        data.insert("home_stock", Mutex::new(stock));
        if !air {
            data.insert(humidity::ROOMS_KEY, Mutex::new(Vec::<humidity::GrowRoom>::new()));
        }
        let (mut world, _) = golden_world(&data);
        if !air {
            let homes: Vec<hecs::Entity> = world.query::<&crate::atmosphere::HomeAir>().iter().map(|(e, _)| e).collect();
            for e in homes {
                world.despawn(e).unwrap();
            }
        }
        let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
        let mut bed_a: Vec<hecs::Entity> = Vec::new();
        for (e, c) in world.query_mut::<&mut CropInstance>() {
            let growth_s = reg.get(&c.crop_def_id).map_or(0.0, |d| f64::from(d.growth_days) * SECONDS_PER_DAY);
            let slot = f64::from(c.tower_slot.unwrap_or(0));
            // Ten ages a plant apart, from just sown to 90% of the way through its season.
            c.planted_at = START_S - (slot % 10.0) / 10.0 * growth_s;
            if c.tower_id.as_deref() == Some("bed_a") {
                bed_a.push(e);
            }
        }
        for e in bed_a {
            world.insert_one(e, CropSoil { store: Npk::ZERO, uptake: 0.0 }).unwrap();
        }
        world.spawn((
            GrowLight { watts: 100.0 },
            PowerConsumer { draw_watts: 100.0, priority: 5, enabled: true },
            Transform { position: glam::Vec3::new(2.0, 2.0, 2.0), ..Default::default() },
        ));
        (data, world)
    }

    /// One crop's end state, the parts compared.
    #[derive(Debug, Clone)]
    pub struct Crop {
        pub plant: String,
        pub area: String,
        pub stage: String,
        pub water: f64,
        pub health: f64,
        pub health_s: f64,
        pub growing_s: f64,
        pub planted_at: f64,
        pub store: [f64; 3],
        pub uptake: f64,
    }

    /// Where one run ended: every crop in entity order, and the soil memory (pests, rooms, pH, weeds,
    /// organic N, the home's air) as canonical JSON.
    pub type Ended = (Vec<Crop>, serde_json::Value);

    /// Where a garden ended: every crop in entity order, and the soil memory as canonical JSON (also
    /// clock_tests.rs's, for the farm on its own clock).
    pub fn ended(world: &hecs::World) -> Ended {
        let mut crops: Vec<(u64, Crop)> = world
            .query::<(&CropInstance, Option<&CropSoil>)>()
            .iter()
            .map(|(e, (c, s))| {
                let store = s.map_or([0.0; 3], |s| [s.store.n, s.store.p2o5, s.store.k2o]);
                (
                    e.to_bits().get(),
                    Crop {
                        plant: c.crop_def_id.clone(),
                        area: c.tower_id.clone().unwrap_or_default(),
                        stage: c.growth_stage.clone(),
                        water: f64::from(c.water_level),
                        health: f64::from(c.health),
                        health_s: c.health_seconds,
                        growing_s: c.growing_seconds,
                        planted_at: c.planted_at,
                        store,
                        uptake: f64::from(s.map_or(0.0, |s| s.uptake)),
                    },
                )
            })
            .collect();
        crops.sort_by_key(|(id, _)| *id);
        let memory: Vec<serde_json::Value> =
            world.query::<&humanity_core::ecs::components::SoilMemory>().iter().map(|(_, m)| serde_json::to_value(m).unwrap()).collect();
        (crops.into_iter().map(|(_, c)| c).collect(), canonical(serde_json::Value::Array(memory)))
    }

    /// `ended`, its crops in (area, plant) order instead of entity order, for two worlds whose crops
    /// were spawned in different orders (a save and its load; each plant grows once per area here).
    // For save_load_farm_tests.rs, which only the native build has.
    #[cfg(feature = "native")]
    pub fn ended_by_place(world: &hecs::World) -> Ended {
        let (mut crops, memory) = ended(world);
        crops.sort_by(|a, b| (&a.area, &a.plant).cmp(&(&b.area, &b.plant)));
        (crops, memory)
    }

    pub fn canonical(v: serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Object(m) => {
                let sorted: BTreeMap<String, serde_json::Value> = m.into_iter().map(|(k, v)| (k, canonical(v))).collect();
                serde_json::Value::Object(sorted.into_iter().collect())
            }
            serde_json::Value::Array(a) => serde_json::Value::Array(a.into_iter().map(canonical).collect()),
            other => other,
        }
    }

    /// The largest difference seen in one quantity, and where.
    #[derive(Debug, Default, Clone)]
    pub struct Worst {
        pub abs: f64,
        pub rel: f64,
        pub at: String,
    }

    impl Worst {
        fn see(&mut self, a: f64, b: f64, at: impl FnOnce() -> String) {
            let abs = (a - b).abs();
            let rel = abs / a.abs().max(b.abs()).max(1e-300);
            if abs > self.abs {
                self.abs = abs;
                self.rel = rel;
                self.at = at();
            }
        }
    }

    /// Every numeric leaf of two JSON trees compared, grouped by the memory field it sits under and its
    /// own key ("memory.pests.level"); a key one side lacks is compared against zero on the other.
    pub fn json_diff(a: &serde_json::Value, b: &serde_json::Value, path: &str, group: &str, out: &mut BTreeMap<String, Worst>) {
        use serde_json::Value as V;
        let zero = V::from(0.0);
        match (a, b) {
            (V::Object(x), V::Object(y)) => {
                let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
                for k in keys {
                    let g = if group.is_empty() { k.as_str() } else { group };
                    json_diff(x.get(k).unwrap_or(&zero), y.get(k).unwrap_or(&zero), &format!("{path}.{k}"), g, out);
                }
            }
            (V::Array(x), V::Array(y)) => {
                for i in 0..x.len().max(y.len()) {
                    json_diff(x.get(i).unwrap_or(&zero), y.get(i).unwrap_or(&zero), &format!("{path}[{i}]"), group, out);
                }
            }
            (V::Object(x), z) | (z, V::Object(x)) if z.as_f64() == Some(0.0) => {
                for (k, v) in x {
                    let g = if group.is_empty() { k.as_str() } else { group };
                    json_diff(v, &zero, &format!("{path}.{k}"), g, out);
                }
            }
            _ => {
                if let (Some(p), Some(q)) = (a.as_f64(), b.as_f64()) {
                    let leaf = path.rsplit('.').next().unwrap_or(path);
                    let leaf = leaf.split('[').next().unwrap_or(leaf);
                    out.entry(format!("memory.{group}.{leaf}")).or_default().see(p, q, || path.to_string());
                }
            }
        }
    }

    /// How two runs' ends differ: stage mismatches by name, and the worst difference per quantity.
    pub fn compare(base: &Ended, other: &Ended) -> (Vec<String>, BTreeMap<String, Worst>) {
        let mut stages = Vec::new();
        let mut worst: BTreeMap<String, Worst> = BTreeMap::new();
        assert_eq!(base.0.len(), other.0.len(), "the same crops in both runs");
        for (i, (a, b)) in base.0.iter().zip(other.0.iter()).enumerate() {
            assert_eq!((&a.plant, &a.area), (&b.plant, &b.area), "crop {i} is the same plant in both runs");
            let at = || format!("{} in {} (crop {i})", a.plant, a.area);
            if a.stage != b.stage {
                stages.push(format!("{}: {} vs {}", at(), a.stage, b.stage));
            }
            worst.entry("crop.water".into()).or_default().see(a.water, b.water, at);
            worst.entry("crop.health".into()).or_default().see(a.health, b.health, at);
            worst.entry("crop.health_seconds".into()).or_default().see(a.health_s, b.health_s, at);
            worst.entry("crop.growing_seconds".into()).or_default().see(a.growing_s, b.growing_s, at);
            worst.entry("crop.planted_at".into()).or_default().see(a.planted_at, b.planted_at, at);
            for (k, name) in ["n", "p2o5", "k2o"].iter().enumerate() {
                worst.entry(format!("soil.store.{name}")).or_default().see(a.store[k], b.store[k], at);
            }
            worst.entry("soil.uptake".into()).or_default().see(a.uptake, b.uptake, at);
        }
        json_diff(&base.1, &other.1, "memory", "", &mut worst);
        (stages, worst)
    }
}
