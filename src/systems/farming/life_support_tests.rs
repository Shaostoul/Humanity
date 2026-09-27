//! Ship life support through the real FarmingSystem tick (2026-09-26): the
//! grow rooms' and the home's air, the air handlers handing the garden's
//! water back to the tanks, the carbon dioxide and the oxygen, and both
//! shipped homes' balance. systems/life_support.rs is the model and
//! data/life_support.ron its numbers. Each test says what was broken to see
//! it red.

use std::collections::HashMap;
use std::sync::Mutex;

use super::gardening_tests::make_store;
use super::humidity::{self, GrowRoom, HumidityData, HUMIDITY_RON};
use super::lighting::GrowPlot;
use super::*;
use crate::ecs::components::{
    AirHandler, Co2Scrubber, CropInstance, HomeAirState, Irrigator, PlumbingCircuit, PowerConsumer, SoilMemory, Transform,
    WaterConsumer, WaterProducer, WaterTank,
};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::atmosphere::{EnclosedSpace, HomeAir};
use crate::systems::life_support::{self, LifeSupportData, LIFE_SUPPORT_RON};
use crate::systems::plumbing::{PlumbingSystem, WaterStatus};

fn life() -> LifeSupportData {
    LifeSupportData::parse(LIFE_SUPPORT_RON).expect("the shipped life_support.ron parses")
}

fn air() -> HumidityData {
    HumidityData::parse(HUMIDITY_RON).expect("the shipped humidity.ron parses")
}

/// The channels the farming and plumbing ticks read, with both air models'
/// data, the given room boxes and grow plots, growth at 1x, no pests.
fn store(rooms: Vec<GrowRoom>, plots: Vec<GrowPlot>) -> DataStore {
    let mut data = make_store();
    data.insert("crop_growth_speed", Mutex::new(1.0_f32));
    data.insert("garden_pest_severity", Mutex::new(0.0_f32));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
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

/// Advance the game clock by `game_s` seconds (the hour follows).
fn advance(data: &DataStore, game_s: f64) {
    let gt = data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap();
    let mut g = gt.lock().unwrap();
    let t = g.elapsed_seconds + game_s;
    g.set_elapsed(t);
}

fn set_scale(data: &DataStore, scale: f32) {
    data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap().lock().unwrap().time_scale = scale;
}

fn memory(world: &hecs::World) -> SoilMemory {
    world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
}

fn home(world: &hecs::World) -> HomeAirState {
    memory(world).home_air
}

fn demand_lpm(data: &DataStore) -> f64 {
    f64::from(*data.get::<Mutex<f32>>("irrigation_demand_lpm").unwrap().lock().unwrap())
}

/// The condensate the air handlers are handing the tanks, L/min.
fn returned_lpm(world: &hecs::World) -> f64 {
    world.query::<(&AirHandler, &WaterProducer)>().iter().map(|(_, (_, p))| f64::from(p.lpm)).sum()
}

/// A crop of `plant` on unit `slot` of `area`, just sown and well watered.
fn crop(data: &DataStore, plant: &str, area: &str, slot: u32, planted_at: f64) -> CropInstance {
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

fn machine(world: &mut hecs::World, at: [f32; 3]) -> hecs::Entity {
    world.spawn((Transform { position: glam::Vec3::from_array(at), ..Default::default() },))
}

/// A 300 m3 greenhouse (room-a) with four 10 m2 plots of lettuce in bed_a,
/// inside a sealed home of `home_m3` (no household), an air handler in the
/// greenhouse and one in the home's own air, the irrigation, and a cistern on
/// one plumbing island with them.
fn greenhouse(home_m3: f32) -> (DataStore, hecs::World, hecs::Entity) {
    let rooms = vec![GrowRoom { id: "room-a".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [10.0, 3.0, 10.0], ..Default::default() }];
    let plots = vec![GrowPlot { id: "bed_a".into(), pos: [5.0, 0.0, 5.0], footprint_m2: 40.0, ..Default::default() }];
    let mut data = store(rooms, plots);
    data.insert(units::PLOT_AREA_KEY, HashMap::from([("bed_a".to_string(), 10.0_f32)]));
    let mut world = hecs::World::new();
    world.spawn((HomeAir { metabolic_kcal_per_day: 0.0 }, EnclosedSpace::new_sealed(home_m3)));
    for slot in 0..4 {
        world.spawn((crop(&data, "lettuce", "bed_a", slot, 0.0),));
    }
    for at in [[5.0, 2.7, 1.0], [50.0, 2.7, 50.0]] {
        let e = machine(&mut world, at);
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
    let cistern = world.spawn((WaterTank { liters: 7000.0, capacity_l: 20000.0 }, PlumbingCircuit { island: 0 }));
    (data, world, cistern)
}

/// The litres of water vapour every air holds: each grow room's and the
/// home's own, at their volumes.
fn vapour_held_l(world: &hecs::World, data: &DataStore) -> f64 {
    let map = humidity::AirMap::new(world, data, &air());
    let m = memory(world);
    let rooms: f64 = map.rooms.iter().map(|r| m.rooms.get(&r.id).map_or(0.0, |a| a.vapour_g_m3) * r.volume_m3()).sum();
    (rooms + m.home_air.vapour_g_m3 * map.home_volume_m3) / 1000.0
}

/// THE BOUNDARY BETWEEN THE TWO CLOCKS (BUG-092 item 7, docs/PRIORITIES.md
/// blocked 3). The air runs on game hours and the tanks on real minutes, and
/// which is right is the operator's open question. The garden's water crosses
/// as litres a day on both sides, so a day's water must balance on the tank
/// side exactly as it does on the air side, whatever the clock:
///
///   litres drawn for the garden = litres the air handlers return
///                                 + litres the crops keep + litres lost
///
/// Here a greenhouse of lettuce in a sealed home runs until its air settles,
/// then for a day. On the tank side (the plumbing sim's own integration, real
/// minutes) the cistern changes by exactly what the irrigation drew less what
/// the coils returned; per day, drawn less returned is what the lettuce keeps
/// (6.25% of what it breathes out, BVAD 2022) plus what leaked overboard, to
/// within 0.5%. On the air side the ledger closes to the gram: what went into
/// the air less what left it is what it holds. And the same day's figures come
/// out at twice the game speed: the clock changes how long a day lasts, never
/// how many litres it moves. Seen red two ways: by handing the condensate to
/// the plumbing as litres a GAME day (72 times the litres a real day at 1x,
/// so the tanks gained water), and by drawing the lettuce's water without what
/// it keeps (the kept 6% then vanished from the balance).
#[test]
fn the_gardens_water_balances_across_the_two_clocks() {
    let ld = life();
    let mut per_day = Vec::new();
    for scale in [1.0_f32, 2.0] {
        let (data, mut world, cistern) = greenhouse(1000.0);
        set_scale(&data, scale);
        let mut sys = FarmingSystem::new();
        let mut pipes = PlumbingSystem::new();
        let dt = 1.0_f32;
        let mut tick = |world: &mut hecs::World| -> (f64, f64) {
            advance(&data, f64::from(dt * scale));
            sys.tick(world, dt, &data);
            let (d, r) = (demand_lpm(&data), returned_lpm(world));
            pipes.tick(world, dt, &data);
            (d, r)
        };
        // Settle: two game days.
        let settle = (2.0 * 1200.0 / f64::from(scale)) as usize;
        for _ in 0..settle {
            tick(&mut world);
        }
        let (tank0, held0, ledger0) = (world.get::<&WaterTank>(cistern).unwrap().liters, vapour_held_l(&world, &data), home(&world).ledger);
        // One game day, integrated on the plumbing's real minutes.
        let span = (1200.0 / f64::from(scale)) as usize;
        let (mut drawn, mut returned) = (0.0f64, 0.0f64);
        for _ in 0..span {
            let (d, r) = tick(&mut world);
            drawn += d * f64::from(dt) / 60.0;
            returned += r * f64::from(dt) / 60.0;
        }
        let tank1 = world.get::<&WaterTank>(cistern).unwrap().liters;
        let (held1, ledger1) = (vapour_held_l(&world, &data), home(&world).ledger);

        // The tank side: the cistern moved by exactly what crossed.
        assert!((f64::from(tank1 - tank0) - (returned - drawn)).abs() < 0.05, "{scale}x: tank {tank0} -> {tank1}, drawn {drawn}, returned {returned}");
        // Per day on the tanks' clock: drawn = returned + kept + lost.
        let real_days = span as f64 * f64::from(dt) / 86_400.0;
        let (drawn_d, returned_d) = (drawn / real_days, returned / real_days);
        let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
        let lettuce = reg.get("lettuce").unwrap();
        let plants = f64::from(units::plants_in_plot(lettuce, Some(10.0)));
        let breathed_d = 4.0 * plants * f64::from(lettuce.water_per_day);
        let kept_d = breathed_d * ld.exchange_for("lettuce").kept_l_per_l;
        let lost_d = (ledger1.leaked_l - ledger0.leaked_l) + (ledger1.surface_l - ledger0.surface_l); // per game day
        println!("{scale}x: breathed {breathed_d:.2} L/day, drawn {drawn_d:.2}, returned {returned_d:.2}, kept {kept_d:.2}, lost {lost_d:.4}");
        assert!((drawn_d - (breathed_d + kept_d)).abs() < 1e-3 * drawn_d, "the draw is the breath and the tissue: {drawn_d} vs {}", breathed_d + kept_d);
        let gap = drawn_d - (returned_d + kept_d + lost_d);
        assert!(gap.abs() < 0.005 * drawn_d, "{scale}x: drawn {drawn_d} = returned {returned_d} + kept {kept_d} + lost {lost_d}, off by {gap}");
        // The air side, exact: in less out is what it holds.
        let d_in = (ledger1.breathed_l - ledger0.breathed_l) + (ledger1.humidified_l - ledger0.humidified_l) + (ledger1.people_l - ledger0.people_l);
        let d_out = (ledger1.condensed_l - ledger0.condensed_l) + (ledger1.surface_l - ledger0.surface_l) + (ledger1.leaked_l - ledger0.leaked_l);
        assert!(((held1 - held0) - (d_in - d_out)).abs() < 1e-6 * d_in.max(1.0), "{scale}x air ledger: held {held0} -> {held1}, in {d_in}, out {d_out}");
        assert!((d_in - breathed_d).abs() < 1e-3 * breathed_d, "a game day's breath went into the air: {d_in} vs {breathed_d}");
        per_day.push((drawn_d, returned_d));
    }
    let ((d1, r1), (d2, r2)) = (per_day[0], per_day[1]);
    assert!((d1 - d2).abs() < 1e-3 * d1 && (r1 - r2).abs() < 2e-3 * r1, "a day moves the same litres at either speed: {per_day:?}");
}

/// A long step (a catch-up of twelve game hours in one frame) is sliced and
/// keeps the air's water exact. Seen red by solving it in one slice (the
/// controllers then acted on the air as it stood twelve hours earlier and the
/// greenhouse ended the step saturated).
#[test]
fn a_catch_up_step_keeps_the_airs_water_exact() {
    let (data, mut world, _) = greenhouse(1000.0);
    let mut sys = FarmingSystem::new();
    sys.tick(&mut world, 1.0, &data);
    let (held0, l0) = (vapour_held_l(&world, &data), home(&world).ledger);
    sys.tick(&mut world, 600.0, &data); // 600 real s = 12 game hours
    let (held1, l1) = (vapour_held_l(&world, &data), home(&world).ledger);
    let d_in = (l1.breathed_l - l0.breathed_l) + (l1.humidified_l - l0.humidified_l);
    let d_out = (l1.condensed_l - l0.condensed_l) + (l1.surface_l - l0.surface_l) + (l1.leaked_l - l0.leaked_l);
    assert!(((held1 - held0) - (d_in - d_out)).abs() < 1e-6 * d_in, "held {held0} -> {held1}, in {d_in}, out {d_out}");
    let d = air();
    let rh = d.rh_of(memory(&world).rooms["room-a"].vapour_g_m3, d.room_temp_c);
    assert!((rh - life().grow_room_setpoint_rh).abs() < 0.02, "the greenhouse holds its setpoint through the catch-up: {rh}");
}

/// The air handlers hold both setpoints, return the water and draw along
/// their fan's curve; unpowered, the greenhouse's water stays in the air and
/// its humidity climbs. Seen red by leaving the coil out of the room's sinks
/// (the greenhouse then settled where its leakage alone put it).
#[test]
fn air_handlers_hold_their_setpoints_and_return_the_water() {
    let (data, mut world, _) = greenhouse(1000.0);
    let (ld, d) = (life(), air());
    let mut sys = FarmingSystem::new();
    for _ in 0..3000 {
        advance(&data, 1.0);
        sys.tick(&mut world, 1.0, &data);
    }
    let m = memory(&world);
    let gh = d.rh_of(m.rooms["room-a"].vapour_g_m3, d.room_temp_c);
    let map = humidity::AirMap::new(&world, &data, &d);
    let hm = d.rh_of(m.home_air.vapour_g_m3, map.home_temp_c);
    assert!((gh - ld.grow_room_setpoint_rh).abs() < 0.005, "greenhouse at 75%: {gh}");
    assert!((hm - ld.home_setpoint_rh).abs() < 0.01, "home at 50%: {hm}");
    let returned = returned_lpm(&world) * 1440.0;
    assert!(returned > 0.0 && m.rooms["room-a"].condensate_l_day > 0.0, "the coils hand the water back: {returned} L a day");
    let watts: f64 = world.query::<(&AirHandler, &PowerConsumer)>().iter().map(|(_, (_, p))| f64::from(p.draw_watts)).sum();
    let want = 325.0 * (ld.fan_power_share(m.rooms["room-a"].air_handler) + ld.fan_power_share(m.home_air.air_handler));
    assert!(watts > 0.0 && (watts - want).abs() < 1e-3, "{watts} W along the fan curve, {want}");
    let view = humidity::GuiView::new(&world, &data);
    println!("{:#?}
{:#?}", view.areas, view.home);
    assert!(view.areas.iter().any(|(_, l, _)| l.contains("air handler at ") && l.contains("back to the tanks")), "{:?}", view.areas);
    assert!(view.home.first().map_or(false, |(l, _)| l.starts_with("Home air 50% humidity")), "{:?}", view.home);
    // Unpowered: no water back, and the greenhouse climbs.
    for (_, (_, pc)) in world.query_mut::<(&AirHandler, &mut PowerConsumer)>() {
        pc.enabled = false;
    }
    for _ in 0..600 {
        advance(&data, 1.0);
        sys.tick(&mut world, 1.0, &data);
    }
    assert_eq!(returned_lpm(&world), 0.0, "no power, no condensate");
    let wet = d.rh_of(memory(&world).rooms["room-a"].vapour_g_m3, d.room_temp_c);
    assert!(wet > gh + 0.03, "the greenhouse's water stays in the air: {gh} -> {wet}");
}

/// The simplified mode (Settings: Ship life support, Station-supplied, the
/// default) takes the air machines' draw off the home's grid and changes
/// nothing else: the rooms hold the same air and the same litres come back.
/// Switched to Realistic, the same machines draw along their fan curve. Seen
/// red by stopping the handlers in the simplified mode (the water then stayed
/// in the air).
#[test]
fn station_supplied_life_support_changes_only_who_pays() {
    let mut runs = Vec::new();
    for realistic in [false, true] {
        let (mut data, mut world, _) = greenhouse(1000.0);
        data.insert(life_support::MODE_KEY, Mutex::new(false));
        life_support::publish(&data, realistic);
        let mut sys = FarmingSystem::new();
        for _ in 0..1500 {
            advance(&data, 1.0);
            sys.tick(&mut world, 1.0, &data);
        }
        let watts: f64 = world.query::<(&AirHandler, &PowerConsumer)>().iter().map(|(_, (_, p))| f64::from(p.draw_watts)).sum();
        let m = memory(&world);
        runs.push((watts, returned_lpm(&world), m.rooms["room-a"].vapour_g_m3, m.home_air.vapour_g_m3));
    }
    let ((w0, r0, g0, h0), (w1, r1, g1, h1)) = (runs[0], runs[1]);
    assert_eq!(w0, 0.0, "the station's plant powers them: no draw on the home");
    assert!(w1 > 0.0, "Realistic: the home's grid pays {w1} W");
    assert!(r0 > 0.0 && (r0 - r1).abs() < 1e-9 && (g0 - g1).abs() < 1e-12 && (h0 - h1).abs() < 1e-12, "the same air and water either way: {runs:?}");
}

/// The household breathes into the home's own air in proportion to its food
/// (three at 2,200 kcal: 1.95 kg of oxygen in, 2.33 kg of carbon dioxide and
/// 5.3 L of water out a day), and above 2,636 ppm the scrubber takes the
/// carbon dioxide overboard, running the share of the time that holds it,
/// drawing 860 W for that share. With no crops the oxygen falls by what the
/// people breathe. Seen red by counting the people's breath per real day (the
/// scrubber then ran at 1/72 of the share).
#[test]
fn the_household_breathes_and_the_scrubber_answers() {
    let (ld, d) = (life(), air());
    let data = store(Vec::new(), Vec::new());
    let mut world = hecs::World::new();
    world.spawn((HomeAir { metabolic_kcal_per_day: 6600.0 }, EnclosedSpace::new_sealed(1000.0)));
    let s = machine(&mut world, [0.0, 0.0, 0.0]);
    world.insert(s, (Co2Scrubber { rated_kg_day: 4.74, watts: 860.0 }, PowerConsumer { draw_watts: 0.0, priority: 1, enabled: true })).unwrap();
    world.spawn((Irrigator,));
    let mut sys = FarmingSystem::new();
    sys.tick(&mut world, 0.001, &data);
    let map = humidity::AirMap::new(&world, &data, &d);
    // Start just under the scrubber's setpoint.
    world.query_mut::<&mut SoilMemory>().into_iter().for_each(|(_, m)| m.home_air.co2_g_m3 = life_support::co2_g_m3(&d, 2600.0, map.home_temp_c));
    let o2_start = home(&world).o2_g_m3 * map.home_volume_m3;
    for _ in 0..2400 {
        advance(&data, 1.0);
        sys.tick(&mut world, 1.0, &data);
    }
    let h = home(&world);
    let ppm = life_support::co2_ppm(&d, h.co2_g_m3, map.home_temp_c);
    assert!((ppm - ld.scrubber_setpoint_ppm).abs() < 60.0, "held at the setpoint: {ppm}");
    let b = ld.breath(6600.0);
    assert!((b.co2_g_day - 2333.0).abs() < 3.0 && (b.o2_g_day - 1952.0).abs() < 3.0, "{b:?}");
    let share = b.co2_g_day / 4740.0;
    assert!((h.scrubber - share).abs() < 0.02, "the scrubber runs {} of the time for {share}", h.scrubber);
    let draw = f64::from(world.get::<&PowerConsumer>(s).unwrap().draw_watts);
    assert!((draw - 860.0 * h.scrubber).abs() < 1e-3, "{draw} W");
    let o2_lost = o2_start - h.o2_g_m3 * map.home_volume_m3;
    assert!((o2_lost / 2.0 - b.o2_g_day).abs() < 0.03 * b.o2_g_day, "two game days of breath: {o2_lost} g of oxygen");
}

/// Crops take up carbon dioxide and give out oxygen in their lit hours, in
/// proportion to the water they breathe out (NASA's ratio: lettuce 5.1 g of
/// CO2 and 3.7 g of O2 a litre), twice over in daylight (the whole day's in
/// its lit half), and none at night; below the reference CO2 they take up
/// less (Kimball). Seen red by leaving out the light (uptake then ran all
/// night).
#[test]
fn crops_fix_carbon_by_day_and_not_by_night() {
    let (ld, d) = (life(), air());
    let (data, mut world, _) = greenhouse(1000.0);
    let mut sys = FarmingSystem::new();
    // Noon, with the greenhouse's air held at the reference 1,100 ppm.
    let hold = |world: &mut hecs::World| {
        world.query_mut::<&mut SoilMemory>().into_iter().for_each(|(_, m)| {
            if let Some(r) = m.rooms.get_mut("room-a") {
                r.co2_g_m3 = life_support::co2_g_m3(&d, 1100.0, d.room_temp_c);
            }
        })
    };
    advance(&data, 600.0); // hour 12
    sys.tick(&mut world, 0.001, &data);
    hold(&mut world);
    sys.tick(&mut world, 0.001, &data);
    let noon = memory(&world).rooms["room-a"];
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let lettuce = reg.get("lettuce").unwrap();
    let litres = 4.0 * f64::from(units::plants_in_plot(lettuce, Some(10.0))) * f64::from(lettuce.water_per_day);
    let want = litres * ld.exchange_for("lettuce").co2_g_per_l * 2.0;
    assert!((noon.co2_uptake_g_day - want).abs() < 1e-3 * want, "{} vs {want} g a day", noon.co2_uptake_g_day);
    // Midnight: none.
    advance(&data, 600.0);
    sys.tick(&mut world, 0.001, &data);
    assert_eq!(memory(&world).rooms["room-a"].co2_uptake_g_day, 0.0, "no light, no uptake");
    // Thin air: at 400 ppm, 69% of it.
    advance(&data, 600.0);
    world.query_mut::<&mut SoilMemory>().into_iter().for_each(|(_, m)| {
        m.rooms.get_mut("room-a").unwrap().co2_g_m3 = life_support::co2_g_m3(&d, 400.0, d.room_temp_c);
    });
    sys.tick(&mut world, 0.001, &data);
    let thin = memory(&world).rooms["room-a"].co2_uptake_g_day;
    assert!((thin / want - ld.co2_factor(400.0)).abs() < 0.01, "{thin} of {want}");
}

/// The mushroom room's carbon dioxide (the gap doc's first known consequence):
/// six racks' tents breathe out 148 g of CO2 an hour into a 300 m3 room that
/// changes its air half a time an hour, so the room sits about 540 ppm above
/// the home's, and each tent, whose fresh air was sized for 400 ppm intake,
/// sits well over the 1,000 ppm the mushrooms fruit under. The Garden panel
/// says so. Seen red by leaving the tents' vented CO2 out of their room's
/// sources (the room then sat at the home's air).
#[test]
fn the_mushroom_rooms_carbon_dioxide_reaches_the_tents() {
    let d = air();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let media = crate::systems::grow_machines::load_grow_media(&root);
    let tent = media.iter().find(|m| m.matches("mushroom_rack")).and_then(|m| m.enclosure.clone()).unwrap();
    let rooms = vec![GrowRoom { id: "room-m".into(), name: "Mushroom room".into(), min: [0.0, 0.0, 0.0], max: [10.0, 3.0, 10.0], ..Default::default() }];
    let plots: Vec<GrowPlot> = (0..6)
        .map(|i| GrowPlot { id: format!("rack_{i}"), pos: [1.5 + 1.4 * i as f32, 0.0, 5.0], enclosure: Some(tent.clone()), ..Default::default() })
        .collect();
    let data = store(rooms, plots);
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for i in 0..6 {
        world.spawn((crop(&data, "oyster_mushroom", &format!("rack_{i}"), 0, 0.0),));
    }
    let mut sys = FarmingSystem::new();
    for _ in 0..2400 {
        advance(&data, 1.0);
        sys.tick(&mut world, 1.0, &data);
    }
    let m = memory(&world);
    let map = humidity::AirMap::new(&world, &data, &d);
    let home_ppm = life_support::co2_ppm(&d, map.home_co2, map.home_temp_c);
    let room_ppm = life_support::co2_ppm(&d, m.rooms["room-m"].co2_g_m3, d.room_temp_c);
    let tent_ppm = life_support::co2_ppm(&d, m.rooms["tent:rack_0"].co2_g_m3, d.room_temp_c);
    println!("home {home_ppm:.0}, mushroom room {room_ppm:.0}, tent {tent_ppm:.0} ppm");
    assert!((room_ppm - home_ppm - 542.0).abs() < 10.0, "the room {room_ppm} over the home {home_ppm}");
    assert!((tent_ppm - room_ppm - 600.0).abs() < 10.0 && tent_ppm > d.fruiting_co2_limit_ppm, "the tent {tent_ppm}");
    let view = humidity::GuiView::new(&world, &data);
    let line = &view.areas.iter().find(|(a, _, _)| a == "rack_0").unwrap().1;
    assert!(line.contains("over the 1,000 the mushrooms fruit under"), "{line}");
}

/// The home's own air is saved with the soil memory and loads back the same.
/// Seen red by marking `SoilMemory::home_air` `#[serde(skip)]` (it then
/// came back empty).
#[test]
fn the_home_air_saves_with_the_soil_memory() {
    let (data, mut world, _) = greenhouse(1000.0);
    let mut sys = FarmingSystem::new();
    for _ in 0..50 {
        advance(&data, 1.0);
        sys.tick(&mut world, 1.0, &data);
    }
    let m = memory(&world);
    assert!(m.home_air.vapour_g_m3 > 0.0 && m.home_air.co2_g_m3 > 0.0 && m.home_air.o2_g_m3 > 0.0);
    let back: SoilMemory = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    // JSON carries a float to its last digit or so; compare every number.
    fn close(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        use serde_json::Value::*;
        match (a, b) {
            (Number(x), Number(y)) => {
                let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
                (x - y).abs() <= 1e-12 * x.abs().max(1.0)
            }
            (Object(x), Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map_or(false, |w| close(v, w))),
            (Array(x), Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| close(v, w)),
            _ => a == b,
        }
    }
    let (was, now) = (serde_json::to_value(&m).unwrap(), serde_json::to_value(&back).unwrap());
    // The loaded struct itself, not two serialized forms (a skipped field
    // would be missing from both and compare equal).
    let (h0, h1) = (&m.home_air, &back.home_air);
    let near = |x: f64, y: f64| (x - y).abs() <= 1e-12 * x.abs().max(1.0);
    assert!(
        near(h0.vapour_g_m3, h1.vapour_g_m3) && near(h0.co2_g_m3, h1.co2_g_m3) && near(h0.o2_g_m3, h1.o2_g_m3)
            && near(h0.ledger.breathed_l, h1.ledger.breathed_l) && near(h0.ledger.condensed_l, h1.ledger.condensed_l),
        "the home air round-trips: {h0:?} vs {h1:?}"
    );
    assert!(close(&was["rooms"], &now["rooms"]), "the rooms' air and CO2 come back");
}

// -- The shipped homes ---------------------------------------------------------------

/// The shipped homes are built through the engine's own spawn (native only).
#[cfg(feature = "native")]
mod shipped {
    use super::*;


    /// A shipped home's garden at full planting, built the way the engine builds
    /// it: the blueprint's rooms (engine::home_meshes publishes the same boxes),
    /// every machine of the home zone spawned by `spawn_home_machine_entity` at its
    /// placed offset with its own wiring's islands, the grow plots and plot areas
    /// `publish_grow_plots` makes, the household's air space, and every plot sown
    /// with the crop its grow medium and the showcase give it. The Commons (another
    /// zone, another frame) is left out.
    fn shipped_home(file: &str) -> (DataStore, hecs::World) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let data_dir = root.join("data");
        let home = crate::machines::MachineHome::load(&data_dir.join("machines").join(file)).expect("home parses");
        let ship = crate::ship::ship_structure::ShipStructure::load(&data_dir.join("blueprints").join("ship_structure.ron")).expect("blueprint");
        let hz = &ship.zones[ship.home_zone_index()];
        let rooms: Vec<(String, [f32; 3], [f32; 3])> = hz
            .body
            .zones
            .iter()
            .filter(|z| z.origin.0 >= 0.0 && z.origin.2 >= 0.0 && z.origin.0 + z.size.0 <= hz.body.width && z.origin.2 + z.size.2 <= hz.body.depth)
            .map(|z| (z.id.clone(), [z.origin.0, z.origin.1, z.origin.2], [z.origin.0 + z.size.0, z.origin.1 + z.size.1, z.origin.2 + z.size.2]))
            .collect();
        let model = crate::systems::grow_machines::GrowFoodModel::load(&data_dir).expect("grow model");
        let all: Vec<_> = home.all_instances().into_iter().filter(|i| i.zone == "home").collect();
        let mut plots = Vec::new();
        let mut areas = HashMap::new();
        let mut sow: Vec<(String, String, u32)> = Vec::new();
        for inst in &all {
            let def = &home.catalog[&inst.machine];
            let Some(medium) = model.media.iter().find(|m| m.matches(&inst.machine)) else { continue };
            let Some(plan) = model.plan(&inst.machine, def.size) else { continue };
            let footprint = def.size.0 * def.size.2;
            let cups: u32 = plan.plots.iter().filter(|p| p.1.is_none()).map(|p| p.2 as u32).sum();
            plots.push(GrowPlot {
                id: inst.id.clone(),
                aliases: Vec::new(),
                pos: [inst.offset.0, inst.offset.1, inst.offset.2],
                footprint_m2: footprint,
                cups,
                outdoors: is_field_area(&inst.machine) || is_field_area(&inst.id),
                enclosure: medium.enclosure.clone(),
            });
            let mut slot = 0u32;
            for (crop, per_plot, n) in &plan.plots {
                if let Some(a) = per_plot {
                    areas.insert(inst.id.clone(), *a);
                }
                for _ in 0..(*n as u32) {
                    sow.push((inst.id.clone(), crop.clone(), slot));
                    slot += 1;
                }
            }
        }
        let mut data = store(Vec::new(), plots);
        humidity::publish_rooms(&data, rooms.iter().map(|(id, min, max)| (id.as_str(), id.as_str(), *min, *max)));
        data.insert(units::PLOT_AREA_KEY, areas);
        let mut world = hecs::World::new();
        let (power, water) = (home.electrical_islands(&all), home.water_islands(&all));
        // Every machine but the RF emitters: the family home's Wi-Fi router, at
        // full power, stunts the whole garden to death within minutes (its own
        // lesson, farming's RF stress), and this measures the air, not that.
        for inst in all.iter().filter(|i| !(home.catalog[&i.machine].rf_emission > 0.0)) {
            crate::engine::home_spawn::spawn_home_machine_entity(&mut world, inst, &home.catalog[&inst.machine], &power, &water, None, None);
        }
        crate::engine::home_spawn::spawn_home_air_space(&mut world, crate::engine::home_spawn::home_metabolic_kcal(&home));
        for (area, plant, slot) in sow {
            world.spawn((crop(&data, &plant, &area, slot, 0.0),));
        }
        (data, world)
    }

    /// What a shipped home's air settles to over a day, with day and night.
    #[derive(Debug, Default)]
    struct HomeDay {
        greenhouse_rh: f64,
        home_rh: f64,
        tents_rh: (f64, f64),
        co2_ppm: (f64, f64),
        tents_co2_ppm: (f64, f64),
        handler_w: f64,
        scrubber_w: f64,
        fan_w: f64,
        humidifier_w: f64,
        breathed_indoor_l: f64,
        tents_breath_l: f64,
        humidifier_l: f64,
        fields_l: f64,
        kept_l: f64,
        people_l: f64,
        surface_l: f64,
        drawn_l: f64,
        returned_l: f64,
        co2_out_kg: f64,
        co2_uptake_kg: f64,
        o2_made_kg: f64,
        o2_used_kg: f64,
        leak_kg: f64,
    }

    /// Run a shipped home a game day to settle, then one to measure, each tick
    /// 6 s at 1x (a tenth of a game hour: one slice, the clock turning day and
    /// night), with `tweak` applied to the life-support data.
    fn settle_home(file: &str, tweak: impl Fn(&mut LifeSupportData)) -> HomeDay {
        use crate::ecs::components::{Humidifier, RoomAir, Ventilator};
        let (mut data, mut world) = shipped_home(file);
        let mut ld = life();
        tweak(&mut ld);
        data.insert(life_support::DATA_KEY, ld);
        let d = air();
        let mut sys = FarmingSystem::new();
        let dt = 6.0_f32;
        let per_day = (1200.0 / dt) as usize;
        for _ in 0..per_day {
            advance(&data, f64::from(dt));
            sys.tick(&mut world, dt, &data);
        }
        let mut day = HomeDay { co2_ppm: (f64::INFINITY, 0.0), tents_rh: (1.0, 0.0), tents_co2_ppm: (f64::INFINITY, 0.0), ..Default::default() };
        let map = humidity::AirMap::new(&world, &data, &d);
        let surface0 = home(&world).ledger.surface_l;
        let watts = |world: &hecs::World, pick: fn(&hecs::EntityRef) -> bool| -> f64 {
            world.iter().filter(|e| pick(e)).filter_map(|e| e.get::<&PowerConsumer>().map(|p| f64::from(p.draw_watts))).sum()
        };
        for _ in 0..per_day {
            advance(&data, f64::from(dt));
            sys.tick(&mut world, dt, &data);
            let m = memory(&world);
            let n = per_day as f64;
            day.greenhouse_rh += m.rooms.get("room-greenhouse").map_or(0.0, |a: &RoomAir| d.rh_of(a.vapour_g_m3, d.room_temp_c)) / n;
            day.home_rh += d.rh_of(m.home_air.vapour_g_m3, map.home_temp_c) / n;
            for (id, a) in &m.rooms {
                if id.starts_with("tent:") {
                    let rh = d.rh_of(a.vapour_g_m3, d.room_temp_c);
                    day.tents_rh = (day.tents_rh.0.min(rh), day.tents_rh.1.max(rh));
                    let ppm = life_support::co2_ppm(&d, a.co2_g_m3, d.room_temp_c);
                    day.tents_co2_ppm = (day.tents_co2_ppm.0.min(ppm), day.tents_co2_ppm.1.max(ppm));
                }
            }
            let ppm = life_support::co2_ppm(&d, m.home_air.co2_g_m3, map.home_temp_c);
            day.co2_ppm = (day.co2_ppm.0.min(ppm), day.co2_ppm.1.max(ppm));
            day.handler_w += watts(&world, |e| e.has::<AirHandler>()) / n;
            day.scrubber_w += watts(&world, |e| e.has::<Co2Scrubber>()) / n;
            day.fan_w += watts(&world, |e| e.has::<Ventilator>()) / n;
            day.humidifier_w += watts(&world, |e| e.has::<Humidifier>()) / n;
            day.breathed_indoor_l += m.rooms.iter().filter(|(id, _)| !id.starts_with("tent:")).map(|(_, a)| a.breathed_l_day).sum::<f64>() / n;
            day.drawn_l += demand_lpm(&data) * 1440.0 / n;
            day.returned_l += returned_lpm(&world) * 1440.0 / n;
            day.co2_out_kg += m.home_air.co2_out_kg_day / n;
            day.co2_uptake_kg += m.home_air.co2_uptake_kg_day / n;
            day.o2_made_kg += m.home_air.o2_made_kg_day / n;
            day.o2_used_kg += m.home_air.o2_used_kg_day / n;
            day.leak_kg = m.home_air.leak_kg_day;
            day.tents_breath_l += m.rooms.iter().filter(|(id, _)| id.starts_with("tent:")).map(|(_, a)| a.breathed_l_day).sum::<f64>() / n;
            day.humidifier_l += m.rooms.values().map(|a| a.humidifier_l_day).sum::<f64>() / n;
        }
        // What the crops' irrigation carries besides their breath: the fields'
        // (outdoors, into the weather) and the water every crop's tissue keeps.
        let ld = life();
        let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
        let areas = data.get::<HashMap<String, f32>>(units::PLOT_AREA_KEY);
        for (_, c) in world.query::<&CropInstance>().iter() {
            let Some(def) = reg.get(&c.crop_def_id) else { continue };
            let et = f64::from(def.water_per_day) * f64::from(units::crop_plants(c, Some(reg), areas));
            if c.tower_id.as_deref().map_or(false, is_field_area) {
                day.fields_l += et;
            }
            day.kept_l += et * ld.exchange_for(&c.crop_def_id).kept_l_per_l;
        }
        day.people_l = ld.breath(map.home_kcal).water_g_day / 1000.0;
        day.surface_l = home(&world).ledger.surface_l - surface0;
        let m = memory(&world);
        let mut rooms: Vec<_> = m.rooms.iter().filter(|(id, _)| !id.starts_with("tent:")).collect();
        rooms.sort_by(|a, b| a.0.cmp(b.0));
        for (id, a) in rooms {
            println!("  {file} {id}: {:.1}% humidity, {:.0} ppm, {:.1} L a day breathed, air handler {:.2}, {:.1} L a day back", d.rh_of(a.vapour_g_m3, d.room_temp_c) * 100.0, life_support::co2_ppm(&d, a.co2_g_m3, d.room_temp_c), a.breathed_l_day, a.air_handler, a.condensate_l_day);
        }
        println!("  {file} ledger {:?}", m.home_air.ledger);
        println!("{file}: {day:#?}");
        day
    }

    /// Both shipped homes hold their air and hand the garden's water back, at
    /// full planting over a day and a night: the greenhouse and the court (whose
    /// towers breathe out about 106 L a day) at their air handlers' 75%, the
    /// home's own air at 50%, no water condensing on any wall, every mushroom
    /// tent still inside the oyster's 85 to 95%, the carbon dioxide under the
    /// scrubber's 2,636 ppm, and the coils returning, to within 1%, every litre
    /// the indoor crops, the tents, the humidifiers and the household put in the
    /// air. The printout is the balance the homes' Energy, Water and Air loops
    /// quote (docs/design/ship-life-support.md). Seen red before the court had
    /// its air handler: the family home's court sat at 100% and about 45 L a day
    /// condensed on its walls, lost to the tanks.
    #[test]
    fn the_shipped_homes_hold_their_air_and_return_their_water() {
        let ld = life();
        for file in ["home.ron", "home_solo.ron"] {
            let day = settle_home(file, |_| {});
            assert!((day.greenhouse_rh - ld.grow_room_setpoint_rh).abs() < 0.02, "{file}: greenhouse {}", day.greenhouse_rh);
            assert!((day.home_rh - ld.home_setpoint_rh).abs() < 0.02, "{file}: home {}", day.home_rh);
            assert!(day.tents_rh.0 >= 0.85 && day.tents_rh.1 <= 0.95, "{file}: tents {:?}", day.tents_rh);
            assert!(day.co2_ppm.1 <= ld.scrubber_setpoint_ppm * 1.03, "{file}: CO2 {:?}", day.co2_ppm);
            assert!(day.surface_l < 0.01, "{file}: {} L condensed on walls, lost to the tanks", day.surface_l);
            let into_air = day.breathed_indoor_l + day.tents_breath_l + day.humidifier_l + day.people_l;
            assert!((day.returned_l - into_air).abs() < 0.01 * into_air, "{file}: {} L returned of {into_air} L put in the air", day.returned_l);
        }
    }

    /// The same two homes with the home's own air held at 60% (the EPA's
    /// "below 60 percent" line) instead of 50%: what the drier home costs.
    /// A measurement for docs/design/ship-life-support.md, not a guard.
    #[test]
    #[ignore]
    fn print_the_shipped_homes_with_the_home_at_60_percent() {
        for file in ["home.ron", "home_solo.ron"] {
            settle_home(file, |l| l.home_setpoint_rh = 0.60);
        }
    }
}
