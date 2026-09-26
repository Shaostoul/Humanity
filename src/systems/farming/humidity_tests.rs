//! Greenhouse humidity and the diseases it brings, through the real
//! FarmingSystem tick (2026-09-26, gardening depth). humidity.rs is the model
//! and data/garden/humidity.ron its numbers; these prove the pieces are wired
//! into the tick: the crops' breath, the rooms, the fans and their power, the
//! diseases' Humidity condition, the controls, the crops' window and the
//! severity switch. Each was seen red first; the doc comment on each says
//! what was broken to see it.

use std::collections::HashMap;
use std::sync::Mutex;

use super::gardening_tests::make_store;
use super::humidity::{self, GrowRoom, HumidityData, HUMIDITY_RON};
use super::lighting::GrowPlot;
use super::pests::{self, PestData};
use super::*;
use crate::ecs::components::{
    Controllable, CropInstance, Irrigator, PestPressure, PowerConsumer, RoomAir, SoilMemory, Transform, Ventilator,
};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

/// A well-watered crop of `plant` in unit `slot` of grow area `area`, at its
/// first stage, planted at game second 0.
fn crop(data: &DataStore, plant: &str, area: &str, slot: u32) -> CropInstance {
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
fn store(base: f64, speed: f32, severity: f32) -> DataStore {
    let mut data = make_store();
    data.insert("crop_growth_speed", Mutex::new(speed));
    data.insert("garden_pest_severity", Mutex::new(severity));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert("hand_water_draw_l", Mutex::new(0.0_f32));
    data.insert("pest_control_request", Mutex::new(Option::<(String, String)>::None));
    let mut d = shipped_air();
    d.base_air_changes_per_hour = base;
    data.insert(humidity::DATA_KEY, d);
    data.insert(
        humidity::ROOMS_KEY,
        Mutex::new(vec![
            GrowRoom { id: "room-a".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [10.0, 3.0, 10.0] },
            GrowRoom { id: "room-b".into(), name: "Plant room".into(), min: [20.0, 0.0, 0.0], max: [30.0, 3.0, 10.0] },
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

fn shipped_air() -> HumidityData {
    HumidityData::parse(HUMIDITY_RON).expect("the shipped humidity.ron parses")
}

fn run(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, ticks: usize, dt: f32) {
    for _ in 0..ticks {
        sys.tick(world, dt, data);
    }
}

fn memory(world: &hecs::World) -> SoilMemory {
    world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
}

fn room(world: &hecs::World, id: &str) -> RoomAir {
    memory(world).rooms.get(id).copied().unwrap_or_default()
}

fn rh(world: &hecs::World, id: &str) -> f64 {
    let d = shipped_air();
    d.rh_of(room(world, id).vapour_g_m3, d.room_temp_c)
}

fn set_room(world: &mut hecs::World, id: &str, vapour: f64) {
    let e = soil::soil_memory_entity(world);
    world.get::<&mut SoilMemory>(e).unwrap().rooms.entry(id.to_string()).or_default().vapour_g_m3 = vapour;
}

fn level(world: &hecs::World, area: &str, pest: &str) -> f64 {
    memory(world).pests.get(area).and_then(|a| a.pressure.get(pest)).map_or(0.0, |s| s.level)
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

fn request(data: &DataStore, area: &str, control: &str) {
    *data.get::<Mutex<Option<(String, String)>>>("pest_control_request").unwrap().lock().unwrap() =
        Some((area.to_string(), control.to_string()));
}

fn health(world: &hecs::World, plant: &str) -> f32 {
    world.query::<&CropInstance>().iter().find(|(_, c)| c.crop_def_id == plant).map(|(_, c)| c.health).unwrap()
}

/// The psychrometric formula is FAO-56's: its equation 11 gives its own
/// worked Example 3 (3.075 kPa at 24.5 C, 1.705 kPa at 15 C), saturated air
/// at the rooms' 21 C holds 18.3 g/m3, water vapour's gas constant is 287 /
/// 0.622, and relative and absolute humidity convert both ways. Seen red by
/// writing `t_c - svp_c_c` for `t_c + svp_c_c` in `saturation_kpa`.
#[test]
fn the_psychrometric_formula_is_fao56s() {
    let d = shipped_air();
    assert!((d.saturation_kpa(24.5) - 3.075).abs() < 0.001, "{}", d.saturation_kpa(24.5));
    assert!((d.saturation_kpa(15.0) - 1.705).abs() < 0.001, "{}", d.saturation_kpa(15.0));
    assert!((d.vapour_gas_constant() - 461.4).abs() < 0.1);
    assert!((d.room_saturation() - 18.32).abs() < 0.01, "{}", d.room_saturation());
    let v = d.vapour_at(0.6, 21.0);
    assert!((d.rh_of(v, 21.0) - 0.6).abs() < 1e-12);
    // The home air, 40% at 20 C, is 37% when it is warmed to the rooms' 21 C.
    let home = d.vapour_at(0.4, 20.0);
    assert!((d.rh_of(home, 21.0) - 0.377).abs() < 0.002, "{}", d.rh_of(home, 21.0));
    // The rooms take the pests' climate-controlled temperature.
    let p = PestData::parse(pests::PESTS_RON).unwrap();
    assert_eq!(d.room_temp_c, p.indoor_temp_c, "humidity.ron and pests.ron agree on the rooms' temperature");
}

/// The balance is solved exactly: a stretch of time in one tick or in many
/// gives the same vapour, it settles at the home air plus what the crops add
/// over the air changes, a closed room gains exactly what is breathed into
/// it, and nothing passes saturation. Seen red by stepping it with one Euler
/// step (a 10-hour tick then overshot).
#[test]
fn the_vapour_balance_is_exact_however_the_time_is_sliced() {
    let (source, n, out, sat) = (6.0, 0.5, 6.85, 18.32);
    let once = humidity::step_vapour(6.85, source, n, out, 3.0, sat);
    let mut sliced = 6.85;
    for _ in 0..3000 {
        sliced = humidity::step_vapour(sliced, source, n, out, 0.001, sat);
    }
    assert!((once - sliced).abs() < 1e-9, "{once} vs {sliced}");
    let settled = humidity::step_vapour(6.85, 1.0, n, out, 1000.0, sat);
    assert!((settled - (out + 1.0 / n)).abs() < 1e-9, "settles at out + S/n: {settled}");
    assert!((humidity::step_vapour(6.85, 2.0, 0.0, out, 1.5, sat) - 9.85).abs() < 1e-12, "closed: +S an hour");
    assert_eq!(humidity::step_vapour(6.85, source, 0.1, out, 1e3, sat), sat, "never past saturation");
}

/// A closed greenhouse (no leakage, no fan) gains exactly the water its
/// growing crops breathe out: plants.csv's litres a day for each plant in
/// each unit (two tomato plants to a 1 m2 plot, at plants.csv's spacing),
/// times vapour_share, over the room's 300 m3, on the game clock. Twenty
/// units for 1.2 game hours. The Plant room next door, with no crops, stays
/// at the home's air. Seen red by dropping `* plants_here` from the breath
/// tally in the crop loop (the room then gained half as much).
#[test]
fn transpiration_raises_a_closed_greenhouse_at_the_cited_rate() {
    let mut data = store(0.0, 1.0, 0.0);
    data.insert(units::PLOT_AREA_KEY, HashMap::from([("bed_a".to_string(), 1.0_f32)]));
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..20 {
        world.spawn((crop(&data, "tomato", "bed_a", slot),));
    }
    // The first tick starts the room at the home air's vapour and steps it:
    // the atmosphere's default home air, 40% at 293 K (held as f32 there).
    let d = shipped_air();
    let home = d.vapour_at(f64::from(0.4f32), f64::from(293.0f32) - 273.15);
    run(&mut sys, &mut world, &data, 60, 1.0);
    let tomato = data.get::<PlantRegistry>("plant_registry").unwrap().get("tomato").unwrap().clone();
    let plants = units::plants_in_plot(&tomato, Some(1.0));
    assert_eq!(plants, 2, "two tomato plants to a square metre (0.418 m2 each)");
    let litres = f64::from(tomato.water_per_day) * f64::from(plants) * 20.0;
    let hours = 60.0 / SECONDS_PER_DAY * 24.0;
    let expected = home + litres * d.vapour_share * 1000.0 / 24.0 / 300.0 * hours;
    let got = room(&world, "room-a").vapour_g_m3;
    assert!((got - expected).abs() < 1e-6, "breathed {litres} L a day: expected {expected} g/m3, got {got}");
    assert!((room(&world, "room-a").breathed_l_day - litres).abs() < 1e-9);
    assert!((room(&world, "room-b").vapour_g_m3 - home).abs() < 1e-9, "no crops, no breath");
}

/// Ventilation holds it down. Two rooms breathing the same 46 L a day over
/// ten game hours, on the rooms' shipped leakage of half an air change an
/// hour: the Plant room, with no fan, runs up to saturation and the player
/// is told once, naming the diseases that spread there; the Greenhouse, with
/// a powered exhaust fan, is held at the fan's 80% setpoint, the fan running
/// part speed and drawing its watts at the cube of its speed. Shed its power
/// and the Greenhouse climbs past the diseases' 85% line. Seen red by making
/// `fan_speed` always 0 (the fan then held nothing).
#[test]
fn an_exhaust_fan_holds_the_room_at_its_setpoint() {
    let mut data = store(0.5, 1.0, 0.0);
    data.insert(units::PLOT_AREA_KEY, HashMap::from([("bed_a".to_string(), 1.0_f32), ("bed_b".to_string(), 1.0_f32)]));
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..20 {
        world.spawn((crop(&data, "tomato", "bed_a", slot),));
        world.spawn((crop(&data, "tomato", "bed_b", slot),));
    }
    let fan = world.spawn((
        Ventilator { airflow_m3_h: 2725.0, watts: 250.0 },
        Transform { position: glam::Vec3::new(5.0, 2.2, 9.5), ..Default::default() },
        PowerConsumer { draw_watts: 250.0, priority: 4, enabled: true },
    ));
    run(&mut sys, &mut world, &data, 500, 1.0);
    let d = shipped_air();
    let (held, open) = (rh(&world, "room-a"), rh(&world, "room-b"));
    assert!(held <= d.fan_setpoint_rh + 1e-6 && held > d.fan_setpoint_rh - 0.02, "the fan holds 80%: {held}");
    assert!(open > 0.999, "without a fan the room saturates: {open}");
    let s = room(&world, "room-a").fan_speed;
    assert!(s > 0.0 && s < 1.0, "part speed: {s}");
    let draw = world.get::<&PowerConsumer>(fan).unwrap().draw_watts;
    assert!((f64::from(draw) - 250.0 * s.powi(3)).abs() < 1e-3, "{draw} W at speed {s}");
    let said: Vec<String> = notices(&data).into_iter().filter(|n| n.contains("humidity")).collect();
    assert_eq!(said.len(), 1, "told once, about the Plant room only: {said:?}");
    assert!(said[0].contains("Plant room") && said[0].contains("gray mold"), "{}", said[0]);
    world.get::<&mut PowerConsumer>(fan).unwrap().enabled = false;
    run(&mut sys, &mut world, &data, 500, 1.0);
    assert!(rh(&world, "room-a") > d.notice_above_rh, "shed, it no longer holds: {}", rh(&world, "room-a"));
}

/// A disease builds only where its humidity condition is met. Tomatoes (a
/// gray mold host, OSU) in a closed, saturated Greenhouse and in a Plant
/// room held at the home's drier air, for twenty garden days: gray mold
/// builds past the level the player notices in the first, and hardly at all
/// in the second (UMass: infection needs "relative humidity 85% or
/// greater"). Seen red by making `met` answer true for every Humidity
/// condition (the dry room then built it as fast).
#[test]
fn a_disease_builds_only_where_the_air_is_humid_enough() {
    let data = store(0.0, 100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..4 {
        world.spawn((crop(&data, "tomato", "bed_a", slot),));
        world.spawn((crop(&data, "tomato", "bed_b", slot),));
    }
    let d = shipped_air();
    let (sat, dry) = (d.room_saturation(), d.vapour_at(0.4, 20.0));
    set_room(&mut world, "room-a", sat);
    // 100x growth, 1 s ticks: twelve ticks a garden day, twenty days.
    for _ in 0..240 {
        set_room(&mut world, "room-b", dry);
        sys.tick(&mut world, 1.0, &data);
    }
    let (humid, dried) = (level(&world, "bed_a", "gray_mold"), level(&world, "bed_b", "gray_mold"));
    assert!(humid > 0.05, "gray mold built in the saturated room: {humid}");
    assert!(dried < 0.01, "and hardly at all in the dry one: {dried}");
    assert!(humid > dried * 20.0, "{humid} vs {dried}");
}

/// Off turns the diseases off: the same saturated Greenhouse full of hosts
/// builds gray mold at Realistic and nothing at all at Off (the severity
/// setting covers the diseases as well as the pests). Seen red by stepping
/// the pests whatever the severity (`if pest_severity > 0.0` made `if true`
/// before the pest step).
#[test]
fn off_turns_the_diseases_off() {
    let grown = |severity: f32| -> f64 {
        let data = store(0.0, 100.0, severity);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        for slot in 0..4 {
            world.spawn((crop(&data, "tomato", "bed_a", slot),));
        }
        set_room(&mut world, "room-a", shipped_air().room_saturation());
        run(&mut sys, &mut world, &data, 240, 1.0);
        level(&world, "bed_a", "gray_mold")
    };
    assert!(grown(1.0) > 0.05, "Realistic: it builds");
    assert_eq!(grown(0.0), 0.0, "Off: nothing builds");
}

/// The disease controls come in the IPM order whatever order the file lists
/// them in: Ventilate (cultural) first, removing infected leaves, then the
/// least-toxic sprays. Each does what its source says, through the control
/// channel: Ventilate lowers the room's humidity (one air change) and kills
/// nothing; removing leaves halves the gray mold; potassium bicarbonate
/// spends a jar from the backpack and starts guarding the leaves; sulfur
/// is refused, with nothing spent, where a cucumber grows (its label: "Do
/// not use on Cucurbits") and spends a bag where it may go. Seen red twice:
/// by declaring `Cultural` after `LeastToxic` (Ventilate then came last),
/// and by removing the `not_on` refusal (the sulfur then went on the
/// cucumber and was spent).
#[test]
fn disease_controls_work_in_the_ipm_order_and_sprays_spend_their_items() {
    let p = PestData::parse(pests::PESTS_RON).unwrap();
    let mut reversed = p.clone();
    reversed.controls.reverse();
    let ids = |d: &PestData, pest: &str| -> Vec<String> { d.controls_for(pest).iter().map(|c| c.id.clone()).collect() };
    for d in [&p, &reversed] {
        assert_eq!(ids(d, "gray_mold"), ["ventilate", "remove_leaves", "potassium_bicarbonate"]);
        assert_eq!(ids(d, "downy_mildew"), ["ventilate", "remove_leaves", "potassium_bicarbonate"]);
        let pm = ids(d, "powdery_mildew");
        assert_eq!(&pm[..2], ["ventilate", "remove_leaves"]);
        assert!(pm[2..].contains(&"sulfur".to_string()) && pm[2..].contains(&"potassium_bicarbonate".to_string()), "{pm:?}");
    }

    let data = store(0.5, 1.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("potassium_bicarbonate_0", 2, 99);
    inv.add_item("garden_sulfur_0", 2, 99);
    let player = world.spawn((inv, Controllable));
    for slot in 0..4 {
        world.spawn((crop(&data, "tomato", "bed_a", slot),));
    }
    world.spawn((crop(&data, "cucumber", "bed_c", 0),));
    let d = shipped_air();
    set_room(&mut world, "room-a", d.room_saturation());
    let e = soil::soil_memory_entity(&mut world);
    world
        .get::<&mut SoilMemory>(e)
        .unwrap()
        .pests
        .entry("bed_a".into())
        .or_default()
        .pressure
        .insert("gray_mold".into(), PestPressure { level: 0.4, told: true });
    let count = |w: &hecs::World, item: &str| w.get::<&Inventory>(player).unwrap().count_item(item);

    // 1. Ventilate: one air change takes 63% of the vapour above the home air's.
    request(&data, "bed_a", "ventilate");
    sys.tick(&mut world, 0.016, &data);
    let after = rh(&world, "room-a");
    let home = d.vapour_at(f64::from(0.4f32), f64::from(293.0f32) - 273.15); // the default home air, 40% at 293 K
    let expect = d.rh_of(home + (d.room_saturation() - home) * (-1.0f64).exp(), d.room_temp_c);
    assert!((after - expect).abs() < 0.01, "vented from 100% to {after}, expected {expect}");
    assert!(notices(&data).iter().any(|n| n.contains("Heat and vent in the Greenhouse")));
    assert!((level(&world, "bed_a", "gray_mold") - 0.4).abs() < 1e-3, "it kills nothing");

    // 2. Remove infected leaves: half the gray mold.
    request(&data, "bed_a", "remove_leaves");
    sys.tick(&mut world, 0.016, &data);
    assert!((level(&world, "bed_a", "gray_mold") - 0.2).abs() < 1e-3, "{}", level(&world, "bed_a", "gray_mold"));

    // 3. Potassium bicarbonate: one jar for four plants, and it guards the leaves.
    request(&data, "bed_a", "potassium_bicarbonate");
    sys.tick(&mut world, 0.016, &data);
    assert_eq!(count(&world, "potassium_bicarbonate_0"), 1, "one jar spent");
    assert!(memory(&world).pests["bed_a"].releases.contains_key("potassium_bicarbonate"), "guarding");

    // 4. Sulfur on the cucumber: refused, nothing spent. On the tomatoes: spent.
    request(&data, "bed_c", "sulfur");
    sys.tick(&mut world, 0.016, &data);
    let said = notices(&data);
    assert!(said.iter().any(|n| n.contains("not for the cucumber")), "{said:?}");
    assert_eq!(count(&world, "garden_sulfur_0"), 2, "nothing spent on a refusal");
    request(&data, "bed_a", "sulfur");
    sys.tick(&mut world, 0.016, &data);
    assert_eq!(count(&world, "garden_sulfur_0"), 1, "one bag spent on the tomatoes");
}

/// A protectant stops new infection while it lasts and then wears off: gray
/// mold on tomatoes in a saturated room grows far less over five garden days
/// under potassium bicarbonate (guarding 0.7 of its growth) than without it,
/// and the guard is gone after its seven days. Seen red by leaving the guard
/// out of `step_area` (both rooms then grew alike).
#[test]
fn a_protectant_spray_slows_a_disease_while_it_lasts() {
    let grown = |sprayed: bool| -> (f64, bool) {
        let data = store(0.0, 100.0, 1.0);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        let mut inv = Inventory::new(16);
        inv.add_item("potassium_bicarbonate_0", 1, 99);
        world.spawn((inv, Controllable));
        for slot in 0..4 {
            world.spawn((crop(&data, "tomato", "bed_a", slot),));
        }
        set_room(&mut world, "room-a", shipped_air().room_saturation());
        let e = soil::soil_memory_entity(&mut world);
        world
            .get::<&mut SoilMemory>(e)
            .unwrap()
            .pests
            .entry("bed_a".into())
            .or_default()
            .pressure
            .insert("gray_mold".into(), PestPressure { level: 0.01, told: true });
        if sprayed {
            request(&data, "bed_a", "potassium_bicarbonate");
        }
        run(&mut sys, &mut world, &data, 60, 1.0); // five garden days
        let lvl = level(&world, "bed_a", "gray_mold");
        run(&mut sys, &mut world, &data, 36, 1.0); // three more: past its seven
        (lvl, memory(&world).pests["bed_a"].releases.contains_key("potassium_bicarbonate"))
    };
    let (bare, _) = grown(false);
    let (guarded, still) = grown(true);
    assert!(guarded < bare * 0.5, "guarded {guarded} against bare {bare}");
    assert!(!still, "the guard wears off after its seven days");
}

/// A crop outside its plants.csv humidity window is capped, gently, and one
/// inside it is not. In a room held at 75%: a tomato (50 to 80%) keeps full
/// health, kale (40 to 70%, five points over) eases to 98 and sunflower (30
/// to 60%, fifteen over) to 94, Bakker's "about 10%" reached 25 points out.
/// Pests and diseases are off, so nothing else caps them. Seen red by
/// leaving `air_ceiling` out of the crop's ceiling (all three stayed at 100).
#[test]
fn a_crop_outside_its_humidity_window_is_capped_gently() {
    let data = store(0.5, 1.0, 0.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for (i, plant) in ["tomato", "kale", "sunflower"].into_iter().enumerate() {
        world.spawn((crop(&data, plant, "bed_a", i as u32),));
    }
    let d = shipped_air();
    let at75 = d.vapour_at(0.75, d.room_temp_c);
    for _ in 0..200 {
        set_room(&mut world, "room-a", at75);
        sys.tick(&mut world, 1.0, &data);
    }
    assert!((health(&world, "tomato") - 100.0).abs() < 1e-3, "in its window: {}", health(&world, "tomato"));
    assert!((health(&world, "kale") - 98.0).abs() < 0.01, "kale {}", health(&world, "kale"));
    assert!((health(&world, "sunflower") - 94.0).abs() < 0.01, "sunflower {}", health(&world, "sunflower"));
    // The cap itself: nothing past the loss at 25 points, never under the floor.
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let kale = reg.get("kale");
    assert_eq!(humidity::health_ceiling(&d, kale, 0.55), 100.0);
    assert!((humidity::health_ceiling(&d, kale, 1.0) - 90.0).abs() < 1e-3, "30 points over: the whole 10%");
    let mut harsh = d.clone();
    harsh.stress_loss_max = 1.0;
    assert_eq!(humidity::health_ceiling(&harsh, kale, 1.0), harsh.health_floor, "never under the floor");
}

/// The engine's room bounds are published with a name a notice can use: the
/// display name the engine has is the room's TYPE ("Garden" for the
/// greenhouse and the mushroom room alike), so a zone id is read instead,
/// and publishing the same rooms again changes nothing. Seen red by keeping
/// the display name (both rooms were then "Garden").
#[test]
fn published_rooms_are_named_for_the_player() {
    let mut data = DataStore::new();
    humidity::register(&mut data);
    let rooms = [
        ("room-greenhouse", "Garden", [10.0, 0.0, 51.0], [55.0, 3.0, 73.0]),
        ("room-mushroom", "Garden", [0.0, 0.0, 51.0], [10.0, 3.0, 61.0]),
        ("galley", "Kitchen", [0.0, 0.0, 0.0], [5.0, 3.0, 5.0]),
    ];
    humidity::publish_rooms(&data, rooms.iter().copied());
    humidity::publish_rooms(&data, rooms.iter().copied());
    let got = data.get::<Mutex<Vec<GrowRoom>>>(humidity::ROOMS_KEY).unwrap().lock().unwrap().clone();
    let names: Vec<&str> = got.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Greenhouse", "Mushroom room", "Kitchen"]);
    assert!((got[0].volume_m3() - 2970.0).abs() < 1e-6, "45 x 3 x 22 m");
}

/// The Garden panel's lines: a crop's card row states the air against its
/// window and the cap, and each area's line names its room, the water its
/// crops breathe out and its fan. Seen red by reading the Plant room's state
/// for every room in `rh_for` (the Greenhouse card then showed 37%).
#[test]
fn the_garden_panel_shows_each_areas_air_and_each_crops_window() {
    let data = store(0.5, 1.0, 0.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    world.spawn((crop(&data, "kale", "bed_a", 0),));
    world.spawn((crop(&data, "kale", "bed_b", 0),));
    let d = shipped_air();
    sys.tick(&mut world, 1.0, &data);
    set_room(&mut world, "room-a", d.vapour_at(0.9, d.room_temp_c));
    let view = humidity::GuiView::new(&world, &data);
    let kale = data.get::<PlantRegistry>("plant_registry").unwrap().get("kale").cloned();
    let (a, b) = {
        let mut q = world.query::<&CropInstance>();
        let crops: Vec<CropInstance> = q.iter().map(|(_, c)| c.clone()).collect();
        let a = crops.iter().find(|c| c.tower_id.as_deref() == Some("bed_a")).unwrap().clone();
        let b = crops.iter().find(|c| c.tower_id.as_deref() == Some("bed_b")).unwrap().clone();
        (a, b)
    };
    let row = view.crop_row(&a, kale.as_ref());
    assert_eq!(row, "90% (grows best at 40% to 70%), holds health to 92%", "{row}");
    assert!(view.crop_row(&b, kale.as_ref()).starts_with("37%"), "{}", view.crop_row(&b, kale.as_ref()));
    let line = view.areas.iter().find(|(area, _, _)| area == "bed_a").unwrap();
    assert!(line.1.contains("Air 90% humidity in the Greenhouse") && line.1.contains("no exhaust fan"), "{}", line.1);
    assert_eq!(line.2, 2, "at the diseases' line");
}
