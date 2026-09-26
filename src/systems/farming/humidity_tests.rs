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
    Controllable, CropInstance, Humidifier, Irrigator, PestPressure, PowerConsumer, RoomAir, SoilMemory, Transform,
    Ventilator,
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

// -- The humidifier and the fungi (2026-09-26, the mushroom room) ----------------

/// A powered humidifier of `l_h` litres an hour and `watts` at full output,
/// standing in the Greenhouse (room-a).
fn humidifier(world: &mut hecs::World, l_h: f32, watts: f32) -> hecs::Entity {
    world.spawn((
        Humidifier { output_l_h: l_h, watts },
        Transform { position: glam::Vec3::new(8.5, 0.0, 9.0), ..Default::default() },
        PowerConsumer { draw_watts: watts, priority: 4, enabled: true },
    ))
}

fn draw(world: &hecs::World, e: hecs::Entity) -> f64 {
    f64::from(world.get::<&PowerConsumer>(e).unwrap().draw_watts)
}

/// The humidifier holds a dry room at its setpoint. The Greenhouse (300 m3,
/// at a quarter air change an hour here so one T7 has headroom) with four
/// oyster mushrooms starts at the home air's 37%; its humidifier runs flat out
/// until the room reaches the shipped 90% and then holds it there at part
/// output, a day later still, while the Plant room next door with none stays
/// dry. The Garden panel's line says what it is doing, and no damp-disease
/// notice is given for a room humidified on purpose. Seen red by making
/// `humidifier_share` always 0 (it then read 0 while the room was dry, and
/// the room stayed at the home air's humidity).
#[test]
fn a_humidifier_raises_a_dry_room_to_its_setpoint_and_holds_it() {
    let data = store(0.25, 1.0, 0.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..4 {
        world.spawn((crop(&data, "oyster_mushroom", "bed_a", slot),));
        world.spawn((crop(&data, "oyster_mushroom", "bed_b", slot),));
    }
    humidifier(&mut world, 1.3, 100.0);
    let d = shipped_air();
    run(&mut sys, &mut world, &data, 5, 1.0);
    assert!(rh(&world, "room-a") < 0.5, "it starts dry: {}", rh(&world, "room-a"));
    assert_eq!(room(&world, "room-a").humidifier, 1.0, "flat out while dry");
    run(&mut sys, &mut world, &data, 500, 1.0); // ten game hours
    let held = rh(&world, "room-a");
    assert!((held - d.humidifier_setpoint_rh).abs() < 0.002, "held at 90%: {held}");
    let share = room(&world, "room-a").humidifier;
    assert!(share > 0.2 && share < 0.9, "part output once there: {share}");
    run(&mut sys, &mut world, &data, 1200, 1.0); // a game day more
    assert!((rh(&world, "room-a") - d.humidifier_setpoint_rh).abs() < 0.002, "and holds it: {}", rh(&world, "room-a"));
    assert!(rh(&world, "room-b") < 0.45, "no humidifier next door: {}", rh(&world, "room-b"));
    assert!(notices(&data).iter().all(|n| !n.contains("Greenhouse air")), "no damp notice for a humidified room");
    let view = humidity::GuiView::new(&world, &data);
    let line = &view.areas.iter().find(|(area, _, _)| area == "bed_a").unwrap().1;
    assert!(line.starts_with("Air 90% humidity in the Greenhouse") && line.contains("humidifier at "), "{line}");
    assert!(line.contains(" W, ") && line.contains(" L of water a day") && !line.contains("exhaust fan"), "{line}");
    // The oysters are in their window there: nothing caps them.
    assert!(world.query::<&CropInstance>().iter().filter(|(_, c)| c.tower_id.as_deref() == Some("bed_a")).all(|(_, c)| c.health > 99.9));
}

/// Its power follows its output: 100 W at the T7's full 1.3 L/h, and in
/// proportion below it (humidity.ron, THE HUMIDIFIER). The same room held at
/// 90% on a tighter and a leakier envelope needs different outputs, and each
/// draws its watts times its share. Seen red by writing `watts` for `watts *
/// share[room]` in `step_rooms` (every humidifier then drew 100 W).
#[test]
fn a_humidifiers_power_scales_with_its_output() {
    let held = |base: f64| -> (f64, f64) {
        let data = store(base, 1.0, 0.0);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        let h = humidifier(&mut world, 1.3, 100.0);
        run(&mut sys, &mut world, &data, 1, 1.0);
        assert!((draw(&world, h) - 100.0).abs() < 1e-3, "flat out from dry: {} W", draw(&world, h));
        run(&mut sys, &mut world, &data, 800, 1.0);
        (room(&world, "room-a").humidifier, draw(&world, h))
    };
    let (tight, tight_w) = held(0.15);
    let (leaky, leaky_w) = held(0.35);
    assert!(tight > 0.1 && leaky < 1.0 && leaky > tight * 2.0, "{tight} vs {leaky}");
    assert!((tight_w - 100.0 * tight).abs() < 1e-3 && (leaky_w - 100.0 * leaky).abs() < 1e-3, "{tight_w} W, {leaky_w} W");
}

/// Its water comes out of the home's tanks, and a dry cistern stops it. With
/// no crops in the room, the irrigation's published draw is exactly the
/// humidifier's litres (1.3 L/h flat out, 31.2 L a day, billed per day like
/// the crops' water) and the plumbing takes them from the cistern. With the
/// cistern dry it stops: no vapour, no draw, no watts, the Garden panel says
/// why, and the room dries back toward the home air. Seen red twice: by
/// dropping the humidifier's litres from `irrigation_l_per_day` in the tick
/// (the draw was then 0), and by passing `true` for the water gate (it ran on
/// a dry cistern).
#[test]
fn a_humidifier_draws_its_water_from_the_tanks_and_stops_when_they_are_dry() {
    use crate::ecs::components::{WaterConsumer, WaterTank};
    use crate::systems::plumbing::{PlumbingSystem, WaterStatus};
    let mut data = store(0.5, 1.0, 0.0);
    data.insert("irrigation_demand_lpm", Mutex::new(0.0_f32));
    data.insert("water_status", Mutex::new(WaterStatus { stored_l: 7000.0, capacity_l: 8000.0, ..Default::default() }));
    let mut sys = FarmingSystem::new();
    let mut pipes = PlumbingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator, WaterConsumer { lpm: 0.0, needs_power: false }));
    let cistern = world.spawn((WaterTank { liters: 7000.0, capacity_l: 8000.0 },));
    let h = humidifier(&mut world, 1.3, 100.0);
    sys.tick(&mut world, 1.0, &data);
    let demand = *data.get::<Mutex<f32>>("irrigation_demand_lpm").unwrap().lock().unwrap();
    assert!((f64::from(demand) - 1.3 * 24.0 / 1440.0).abs() < 1e-6, "the humidifier's 31.2 L a day: {demand} L/min");
    pipes.tick(&mut world, 60.0, &data);
    let left = world.get::<&WaterTank>(cistern).unwrap().liters;
    assert!((f64::from(7000.0 - left) - 1.3 * 24.0 / 1440.0).abs() < 1e-3, "a minute's draw left the cistern: {left}");
    run(&mut sys, &mut world, &data, 300, 1.0);
    let wet = rh(&world, "room-a");
    // The cistern runs dry (the plumbing publishes it): the humidifier stops.
    world.get::<&mut WaterTank>(cistern).unwrap().liters = 0.0;
    pipes.tick(&mut world, 1.0, &data);
    run(&mut sys, &mut world, &data, 300, 1.0);
    let demand = *data.get::<Mutex<f32>>("irrigation_demand_lpm").unwrap().lock().unwrap();
    assert_eq!(demand, 0.0, "nothing drawn from dry tanks");
    let air = room(&world, "room-a");
    assert!(air.humidifier == 0.0 && air.humidifier_l_day == 0.0 && air.humidifier_dry, "{air:?}");
    assert_eq!(draw(&world, h), 0.0, "no mist, no watts");
    assert!(rh(&world, "room-a") < wet - 0.2, "the room dries: {wet} to {}", rh(&world, "room-a"));
    world.spawn((crop(&data, "oyster_mushroom", "bed_a", 0),)); // so the panel has a line for bed_a
    let view = humidity::GuiView::new(&world, &data);
    assert!(view.areas[0].1.contains("humidifier stopped: no water from the tanks"), "{:?}", view.areas);
    assert_eq!(view.areas[0].2, 1, "a humidifier that cannot run is a warning");
}

/// Mushrooms below their range lose far more than a green crop out of its
/// own: a fungus's pins dry out (humidity.ron, FUNGI; Kim 2013's yields fitted
/// as the square of the shortfall). 37 points under their windows, an oyster
/// mushroom (85 to 95%) is held to about 38 and kale (40 to 70%) to 90;
/// Kim's own points come back (4.5% lost 10 under, 18.1% 20 under); wet air
/// above a fungus's window takes the green crops' gentle cap; the house floor
/// holds. Through the tick, oysters in a room held at 48% sink to that cap.
/// Seen red by removing the fungi branch from `health_ceiling` (the oyster
/// then kept 90, the same as the kale).
#[test]
fn mushrooms_below_their_range_lose_more_than_a_green_crop_out_of_its_range() {
    let data = store(0.5, 1.0, 0.0);
    let d = shipped_air();
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let (oyster, kale) = (reg.get("oyster_mushroom"), reg.get("kale"));
    assert!(!oyster.unwrap().needs_light && kale.unwrap().needs_light);
    let cap = |def, rh| f64::from(humidity::health_ceiling(&d, def, rh));
    let (o, k) = (cap(oyster, 0.85 - 0.37), cap(kale, 0.40 - 0.37));
    assert!((o - 38.0).abs() < 1.0 && (k - 90.0).abs() < 1e-3, "oyster {o}, kale {k}");
    assert!((cap(oyster, 0.75) - 95.47).abs() < 0.01 && (cap(oyster, 0.65) - 81.89).abs() < 0.01, "Kim's points");
    assert!((cap(oyster, 1.0) - 98.0).abs() < 1e-3, "5 points too wet: the gentle cap");
    assert_eq!(cap(oyster, 0.2), f64::from(d.health_floor), "never under the floor");
    assert_eq!(cap(oyster, 0.9), 100.0, "in its window");
    // Through the tick: oysters in a room held at 48% ease down to the cap.
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    world.spawn((crop(&data, "oyster_mushroom", "bed_a", 0),));
    let dry = d.vapour_at(0.48, d.room_temp_c);
    for _ in 0..1000 {
        set_room(&mut world, "room-a", dry);
        sys.tick(&mut world, 1.0, &data);
    }
    let want = cap(oyster, 0.48);
    assert!((f64::from(health(&world, "oyster_mushroom")) - want).abs() < 0.5, "{} vs cap {want}", health(&world, "oyster_mushroom"));
}

/// The shipped mushroom rooms are in range at full planting. Each home's
/// mushroom racks (the showcase sows oyster mushrooms in all four shelves)
/// stand in the blueprint's 10 x 3 x 10 m mushroom room with the home's
/// humidifier, built from the catalog def; after two game days the room is
/// inside the oyster's 85 to 95% window and no oyster is capped. Without the
/// humidifier's power the same room settles near 48% (family) and 41% (solo).
/// Seen red by turning home.ron's `humidifier_1` into another machine (the
/// family room then read 48%).
#[test]
fn the_placed_humidifier_keeps_the_shipped_mushroom_room_in_range() {
    use crate::machines::{MachineHome, MachinePower};
    let settle = |file: &str, powered: bool| -> (f64, f64, usize, bool) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let home = MachineHome::load(&root.join("data").join("machines").join(file)).expect("home parses");
        let blueprint = std::fs::read_to_string(root.join("data").join("blueprints").join("ship_structure.ron")).unwrap();
        let block = &blueprint[blueprint.find("id: \"room-mushroom\"").expect("the blueprint has the mushroom room")..];
        let triple = |key: &str| -> [f32; 3] {
            let s = &block[block.find(key).unwrap() + key.len()..];
            let v: Vec<f32> = s[..s.find(')').unwrap()].split(',').map(|x| x.trim().parse().unwrap()).collect();
            [v[0], v[1], v[2]]
        };
        let (o, s) = (triple("origin: ("), triple("size: ("));
        let mushroom = GrowRoom { id: "room-mushroom".into(), name: "Mushroom room".into(), min: o, max: [o[0] + s[0], o[1] + s[1], o[2] + s[2]] };
        let all = home.all_instances();
        let racks: Vec<_> = all.iter().filter(|i| i.machine == "mushroom_rack" && i.room == "room-mushroom").collect();
        let mut data = store(0.5, 1.0, 0.0);
        data.insert(humidity::ROOMS_KEY, Mutex::new(vec![mushroom]));
        let plots = racks.iter().map(|r| GrowPlot { id: r.id.clone(), pos: [r.offset.0, r.offset.1, r.offset.2], ..Default::default() });
        data.insert("grow_plots", plots.collect::<Vec<_>>());
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        for r in &racks {
            for shelf in 0..4 {
                world.spawn((crop(&data, "oyster_mushroom", &r.id, shelf),));
            }
        }
        let placed: Vec<_> = all.iter().filter(|i| i.machine == "humidifier").collect();
        for p in &placed {
            let def = &home.catalog["humidifier"];
            let Some(MachinePower::Consumer { watts, .. }) = def.power else { panic!("a Consumer") };
            world.spawn((
                Humidifier { output_l_h: def.humidifies_l_h, watts },
                Transform { position: glam::Vec3::new(p.offset.0, p.offset.1, p.offset.2), ..Default::default() },
                PowerConsumer { draw_watts: watts, priority: 4, enabled: powered },
            ));
        }
        run(&mut sys, &mut world, &data, 2400, 1.0); // two game days
        let rh = rh(&world, "room-mushroom");
        let healthy = world.query::<&CropInstance>().iter().all(|(_, c)| c.health > 99.9);
        println!("{file} powered={powered}: {} racks, {rh:.4} RH, humidifier {:.3} ({:.1} L/day)", racks.len(), room(&world, "room-mushroom").humidifier, room(&world, "room-mushroom").humidifier_l_day);
        (rh, room(&world, "room-mushroom").humidifier, racks.len(), healthy)
    };
    for (file, racks, dry_rh) in [("home.ron", 6, 0.483), ("home_solo.ron", 2, 0.411)] {
        let (rh, share, n, healthy) = settle(file, true);
        assert_eq!(n, racks, "{file}: racks in the mushroom room");
        assert!((0.85..=0.95).contains(&rh), "{file}: the room at {rh}, the oyster's window is 85 to 95%");
        assert!(share > 0.5, "{file}: its humidifier is working ({share})");
        assert!(healthy, "{file}: no oyster capped");
        let (dry, _, _, _) = settle(file, false);
        assert!((dry - dry_rh).abs() < 0.01, "{file}: unpowered, the room sits near {dry_rh}: {dry}");
    }
}
