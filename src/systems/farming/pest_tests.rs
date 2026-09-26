//! Pests and integrated pest management through the real FarmingSystem tick
//! (2026-09-26, gardening depth). The model's pieces are unit-tested in
//! pests.rs; these drive whole ticks, so they prove the pieces are wired into
//! the growth loop, the health cap, the notices, the control channel, the
//! backpack and the water. Each was seen red first; the doc comment on each
//! says what was broken to see it.

use super::gardening_tests::make_store;
use super::pests::{self, PestData};
use super::*;
use crate::ecs::components::{
    AreaPests, Controllable, CropInstance, CropSoil, Irrigator, Npk, PestPressure, SoilMemory,
};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

/// A well-watered crop of `plant` in unit `slot` of grow area `area`, at its
/// first stage, planted at game second 0.
fn crop(data: &DataStore, plant: &str, area: &str, slot: u32) -> CropInstance {
    let stage = data
        .get::<PlantRegistry>("plant_registry")
        .unwrap()
        .get(plant)
        .unwrap()
        .first_stage()
        .to_string();
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

/// A store with the channels pests use, at `speed` growth and `severity`.
fn store(speed: f32, severity: f32) -> DataStore {
    let mut data = make_store();
    data.insert("crop_growth_speed", std::sync::Mutex::new(speed));
    data.insert("garden_pest_severity", std::sync::Mutex::new(severity));
    data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
    data.insert("hand_water_draw_l", std::sync::Mutex::new(0.0_f32));
    data.insert("pest_control_request", std::sync::Mutex::new(Option::<(String, String)>::None));
    data
}

fn shipped() -> PestData {
    PestData::parse(pests::PESTS_RON).unwrap()
}

/// Run `ticks` ticks of `dt` seconds.
fn run(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, ticks: usize, dt: f32) {
    for _ in 0..ticks {
        sys.tick(world, dt, data);
    }
}

fn memory(world: &hecs::World) -> SoilMemory {
    world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
}

fn level(world: &hecs::World, area: &str, pest: &str) -> f64 {
    memory(world).pests.get(area).and_then(|a| a.pressure.get(pest)).map_or(0.0, |s| s.level)
}

/// Put `pest` at `lvl` in `area` (the player already told, so no notice).
fn seed_pest(world: &mut hecs::World, area: &str, pest: &str, lvl: f64, told: bool) {
    let e = soil::soil_memory_entity(world);
    let mut m = world.get::<&mut SoilMemory>(e).unwrap();
    m.pests
        .entry(area.to_string())
        .or_default()
        .pressure
        .insert(pest.to_string(), PestPressure { level: lvl, told });
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

fn request(data: &DataStore, area: &str, control: &str) {
    *data
        .get::<std::sync::Mutex<Option<(String, String)>>>("pest_control_request")
        .unwrap()
        .lock()
        .unwrap() = Some((area.to_string(), control.to_string()));
}

/// Pressure builds on a host crop under the conditions its pest likes, and
/// not at all on a crop no pest eats. Two indoor towers side by side: four
/// lettuces (an aphid host, UC IPM) and four wheat (no pest here eats it).
/// Over twenty garden days aphids arrive and multiply on the lettuce and
/// never on the wheat. Over-feeding favours them (UC IPM: "High levels of
/// nitrogen fertilizer favor aphid reproduction"): a third tower of lettuce
/// whose units hold far more nitrogen than the crop needs builds aphids
/// faster. And with pests switched off (severity 0) nothing builds at all.
/// Seen red by making `PestDef::hosts_plant` answer true for every plant
/// (the wheat tower then carried aphids).
#[test]
fn pressure_builds_on_a_host_under_its_conditions_and_not_on_a_non_host() {
    let run_towers = |severity: f32| -> (f64, f64, f64) {
        let data = store(100.0, severity);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        for slot in 0..4 {
            world.spawn((crop(&data, "lettuce", "ntower_1", slot),));
            world.spawn((crop(&data, "wheat", "ntower_2", slot),));
            let rich = CropSoil { store: Npk::new(1000.0, 1000.0, 1000.0), uptake: 0.0 };
            world.spawn((crop(&data, "lettuce", "ntower_3", slot), rich));
        }
        // 100x growth, 1 s ticks: a garden day is 12 ticks; twenty days.
        run(&mut sys, &mut world, &data, 240, 1.0);
        (level(&world, "ntower_1", "aphid"), level(&world, "ntower_2", "aphid"), level(&world, "ntower_3", "aphid"))
    };
    let (lettuce, wheat, fed) = run_towers(1.0);
    assert!(lettuce > 0.001, "aphids built up on the lettuce: {lettuce}");
    assert_eq!(wheat, 0.0, "and never on the wheat");
    assert!(fed > lettuce * 2.0, "over-fed lettuce builds them faster: {fed} vs {lettuce}");
    let (off, _, off_fed) = run_towers(0.0);
    assert_eq!((off, off_fed), (0.0, 0.0), "pests switched off: nothing builds");
}

/// A monoculture of a host builds its pest faster than a mixed planting
/// (the resource concentration pests.ron models as the host share): four
/// lettuces against two lettuces and two wheat in the same conditions. Seen
/// red by taking the host share as 1 whenever any host is present (the two
/// towers then built aphids at the same pace).
#[test]
fn a_monoculture_builds_its_pest_faster_than_a_mixed_planting() {
    let data = store(100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..4 {
        world.spawn((crop(&data, "lettuce", "ntower_1", slot),));
        let mixed = if slot < 2 { "lettuce" } else { "wheat" };
        world.spawn((crop(&data, mixed, "ntower_2", slot),));
    }
    run(&mut sys, &mut world, &data, 240, 1.0);
    let mono = level(&world, "ntower_1", "aphid");
    let mixed = level(&world, "ntower_2", "aphid");
    assert!(mixed > 0.0, "the mixed tower has some: {mixed}");
    assert!(mono > mixed * 1.5, "the monoculture more: {mono} vs {mixed}");
}

/// A heavy infestation lowers a host's health GRADUALLY, to its cap and no
/// further, and the season health the harvest reads carries the loss. Two
/// tomatoes in two towers, one with aphids at their worst. At the cited
/// damage (severity 1) the tomato eases down at 0.1 a second, not at once,
/// settles at 70 (aphids take at most 0.3, pests.ron) and is never killed;
/// its twin stays at 100. In the gentle mode the same aphids cap it at 85.
/// Seen red by leaving the pest cap out of the health step (both tomatoes
/// stayed at 100).
#[test]
fn an_infestation_lowers_health_gradually_and_so_the_season_health() {
    let run_pair = |severity: f32| -> (CropInstance, CropInstance, f32) {
        // 1x growth: a tomato stays growing for the whole test.
        let data = store(1.0, severity);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        let sick = world.spawn((crop(&data, "tomato", "ntower_1", 0),));
        let well = world.spawn((crop(&data, "tomato", "ntower_2", 0),));
        seed_pest(&mut world, "ntower_1", "aphid", 1.0, true);
        run(&mut sys, &mut world, &data, 60, 1.0);
        let after_a_minute = world.get::<&CropInstance>(sick).unwrap().health;
        run(&mut sys, &mut world, &data, 600, 1.0);
        let s = (*world.get::<&CropInstance>(sick).unwrap()).clone();
        let w = (*world.get::<&CropInstance>(well).unwrap()).clone();
        (s, w, after_a_minute)
    };
    let (sick, well, minute) = run_pair(1.0);
    assert!(minute < 100.0 && minute > 90.0, "gradual: about 94 after a minute, got {minute}");
    assert!((sick.health - 70.0).abs() < 0.01, "settles at the cap, got {}", sick.health);
    assert_ne!(sick.growth_stage, STAGE_DEAD, "never killed");
    // The twin's tower has only the first few arrivals (a ten-thousandth of
    // a full infestation after eleven minutes at 1x).
    assert!(well.health > 99.9, "the twin is all but untouched, got {}", well.health);
    let (s, w) = (season_health(&sick), season_health(&well));
    assert!(s < 0.9 && s > 0.7, "the season record carries the loss: {s}");
    assert!(w > 0.999, "{w}");
    let (gentle, _, _) = run_pair(0.5);
    assert!((gentle.health - 85.0).abs() < 0.01, "gentle mode: half the damage, got {}", gentle.health);
}

/// The player is told ONCE when a pest appears on a grow area: what it is,
/// where, what favours it, and its controls gentlest first. More ticks with
/// the aphids still there say nothing more. Seen red by never setting
/// `told` in `step_area` (the notice then came every tick).
#[test]
fn the_player_is_told_once_when_a_pest_appears() {
    let data = store(100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..4 {
        world.spawn((crop(&data, "lettuce", "ntower_3", slot),));
    }
    // Just under the level at which it is noticed.
    seed_pest(&mut world, "ntower_3", "aphid", 0.049, false);
    run(&mut sys, &mut world, &data, 60, 1.0);
    let said = notices(&data);
    assert_eq!(said.len(), 1, "told once: {said:?}");
    let n = &said[0];
    assert!(n.starts_with("Aphids have appeared on the crops in ntower 3."), "{n}");
    assert!(n.contains("nitrogen"), "with what favours them: {n}");
    let hose = n.find("Hose off").expect("names hosing off");
    let soap = n.find("Insecticidal soap").expect("names the soap");
    assert!(hose < soap, "gentlest first: {n}");
    run(&mut sys, &mut world, &data, 240, 1.0);
    assert!(level(&world, "ntower_3", "aphid") > 0.05, "still there");
    assert!(notices(&data).is_empty(), "and not said again");
}

/// Every control lowers the pressure of the pests it works on by the share
/// pests.ron gives it, at once, through the request channel: hosing off
/// (aphids 0.5, spider mites 0.4), hand-picking (caterpillars, slugs and
/// potato beetles 0.5), Bt (caterpillars 0.8), insecticidal soap (aphids
/// 0.8, mites 0.6) and iron phosphate bait (slugs 0.6). The shipped shares
/// are pinned too, so a data edit has to update the sources beside it. A
/// control does nothing to a pest it does not work on (soap on caterpillars,
/// CSU: "generally immune"). Seen red by applying `removes` as the share
/// LEFT instead of the share removed: hosing aphids (a half) still passed,
/// and hosing spider mites (0.4) then left 0.16 of 0.4 instead of 0.24.
#[test]
fn each_control_lowers_its_pests_by_the_cited_share() {
    let d = shipped();
    let expected: &[(&str, &str, f64)] = &[
        ("hose_off", "aphid", 0.5),
        ("hose_off", "spider_mite", 0.4),
        ("hand_pick", "cabbage_caterpillar", 0.5),
        ("hand_pick", "slug", 0.5),
        ("hand_pick", "colorado_potato_beetle", 0.5),
        ("bt_spray", "cabbage_caterpillar", 0.8),
        ("soap_spray", "aphid", 0.8),
        ("soap_spray", "spider_mite", 0.6),
        ("iron_phosphate_bait", "slug", 0.6),
    ];
    for &(control, pest, share) in expected {
        assert_eq!(d.control(control).unwrap().removes.get(pest).copied(), Some(share), "{control} on {pest}");
        let data = store(1.0, 1.0);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        // Enough of every item in the backpack.
        let mut inv = Inventory::new(16);
        for item in ["insecticidal_soap_0", "bt_kurstaki_0", "iron_phosphate_bait_0"] {
            inv.add_item(item, 10, 99);
        }
        world.spawn((inv, Controllable));
        // A host of every pest, in the place it lives: an outdoor field.
        let area = "veg_field_1";
        for (i, plant) in ["bean", "kale", "potato", "lettuce"].into_iter().enumerate() {
            world.spawn((crop(&data, plant, area, i as u32),));
        }
        seed_pest(&mut world, area, pest, 0.4, true);
        request(&data, area, control);
        sys.tick(&mut world, 0.016, &data);
        let after = level(&world, area, pest);
        assert!(
            (after - 0.4 * (1.0 - share)).abs() < 1e-4,
            "{control} takes {share} of the {pest}: 0.4 to {after}"
        );
    }
    // Soap on caterpillars: nothing, and the player is told why.
    let data = store(1.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(16);
    inv.add_item("insecticidal_soap_0", 2, 99);
    let player = world.spawn((inv, Controllable));
    world.spawn((crop(&data, "kale", "veg_field_1", 0),));
    seed_pest(&mut world, "veg_field_1", "cabbage_caterpillar", 0.4, true);
    request(&data, "veg_field_1", "soap_spray");
    sys.tick(&mut world, 0.016, &data);
    assert!((level(&world, "veg_field_1", "cabbage_caterpillar") - 0.4).abs() < 1e-4, "soap does not touch caterpillars");
    let said = notices(&data);
    assert!(said.iter().any(|n| n.contains("Caterpillars, beetles and slugs are immune")), "{said:?}");
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("insecticidal_soap_0"), 1, "one spray used");
}

/// What a control costs is real: hosing off draws half a litre a plant from
/// the home tanks (queued for the plumbing the way hand watering is) and is
/// refused with empty tanks; an item control takes one item per so many
/// plants from the backpack and is refused, with nothing spent and nothing
/// done, without them; creative mode needs none. Seen red by charging the
/// soap per request instead of per `plants_per_item` (a 25-plant tower then
/// used one spray, not two).
#[test]
fn a_control_spends_real_water_and_items_and_is_refused_without_them() {
    let draw = |d: &DataStore| *d.get::<std::sync::Mutex<f32>>("hand_water_draw_l").unwrap().lock().unwrap();
    let data = store(1.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let player = world.spawn((Inventory::new(16), Controllable));
    for slot in 0..25 {
        world.spawn((crop(&data, "lettuce", "ntower_1", slot),));
    }
    seed_pest(&mut world, "ntower_1", "aphid", 0.4, true);

    // Hosing 25 plants: 12.5 L from the tanks.
    request(&data, "ntower_1", "hose_off");
    sys.tick(&mut world, 0.016, &data);
    assert!((draw(&data) - 12.5).abs() < 1e-4, "{}", draw(&data));
    assert!((level(&world, "ntower_1", "aphid") - 0.2).abs() < 1e-4);

    // No soap in the pack: refused, nothing done.
    notices(&data);
    request(&data, "ntower_1", "soap_spray");
    sys.tick(&mut world, 0.016, &data);
    assert!((level(&world, "ntower_1", "aphid") - 0.2).abs() < 1e-4, "no soap, no effect");
    let said = notices(&data);
    assert!(said.iter().any(|n| n.contains("needs 2 x")), "told it needs two sprays: {said:?}");

    // Three sprays: two are used for 25 plants (20 a spray).
    world.get::<&mut Inventory>(player).unwrap().add_item("insecticidal_soap_0", 3, 99);
    request(&data, "ntower_1", "soap_spray");
    sys.tick(&mut world, 0.016, &data);
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("insecticidal_soap_0"), 1);
    assert!((level(&world, "ntower_1", "aphid") - 0.04).abs() < 1e-4);

    // Empty tanks: no hosing.
    let mut dry = store(1.0, 1.0);
    dry.insert(
        "water_status",
        std::sync::Mutex::new(crate::systems::plumbing::WaterStatus { stored_l: 0.0, capacity_l: 8000.0, ..Default::default() }),
    );
    let mut w2 = hecs::World::new();
    w2.spawn((crop(&dry, "lettuce", "ntower_1", 0),));
    seed_pest(&mut w2, "ntower_1", "aphid", 0.4, true);
    request(&dry, "ntower_1", "hose_off");
    sys.tick(&mut w2, 0.016, &dry);
    assert!((level(&w2, "ntower_1", "aphid") - 0.4).abs() < 1e-4, "nothing to hose with");
    assert_eq!(draw(&dry), 0.0);

    // Creative: the soap is free.
    let mut free = store(1.0, 1.0);
    free.insert("creative_mode", std::sync::Mutex::new(true));
    let mut w3 = hecs::World::new();
    w3.spawn((Inventory::new(16), Controllable));
    w3.spawn((crop(&free, "lettuce", "ntower_1", 0),));
    seed_pest(&mut w3, "ntower_1", "aphid", 0.4, true);
    request(&free, "ntower_1", "soap_spray");
    sys.tick(&mut w3, 0.016, &free);
    assert!((level(&w3, "ntower_1", "aphid") - 0.08).abs() < 1e-4, "creative sprays for free");
}

/// A release of predatory mites keeps eating spider mites day after day
/// (UC IPM: they outbreed their prey), where a spray acts once; it ends when
/// the mites are gone (they "disperse or starve"); and a soap spray kills it
/// (CSU: "Both beneficial and pest mites are affected"). Seen red by
/// dropping the release's daily share from `step_area` (the mites then went
/// on growing under the predators).
#[test]
fn predatory_mites_work_for_days_until_the_prey_is_gone_and_soap_kills_them() {
    let data = store(100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("predatory_mites_0", 2, 99);
    inv.add_item("insecticidal_soap_0", 2, 99);
    world.spawn((inv, Controllable));
    for slot in 0..4 {
        world.spawn((crop(&data, "bean", "ntower_1", slot),));
    }
    seed_pest(&mut world, "ntower_1", "spider_mite", 0.3, true);
    request(&data, "ntower_1", "predatory_mites");
    sys.tick(&mut world, 0.016, &data);
    assert!((level(&world, "ntower_1", "spider_mite") - 0.3).abs() < 1e-3, "no knock-down at once");
    assert!(memory(&world).pests["ntower_1"].releases.contains_key("predatory_mites"));
    // Five garden days (12 ticks a day at 100x).
    run(&mut sys, &mut world, &data, 60, 1.0);
    let five = level(&world, "ntower_1", "spider_mite");
    assert!(five < 0.3 * 0.1, "half a day's worth each day: down from 0.3 to {five}");
    // Twenty more days: once the mites fall below a thousandth the
    // predators starve or leave, and the few mites left (and newcomers)
    // creep back only slowly in a cool, watered tower.
    run(&mut sys, &mut world, &data, 240, 1.0);
    assert!(
        memory(&world).pests.get("ntower_1").map_or(true, |a| a.releases.is_empty()),
        "the predators starved or left"
    );
    let later = level(&world, "ntower_1", "spider_mite");
    assert!(later < 0.01, "and the mites are still low: {later}");

    // A second release, then soap: the soap kills it.
    seed_pest(&mut world, "ntower_1", "spider_mite", 0.3, true);
    request(&data, "ntower_1", "predatory_mites");
    sys.tick(&mut world, 0.016, &data);
    notices(&data);
    request(&data, "ntower_1", "soap_spray");
    sys.tick(&mut world, 0.016, &data);
    assert!(memory(&world).pests["ntower_1"].releases.is_empty(), "soap killed the predatory mites");
    assert!(notices(&data).iter().any(|n| n.contains("also killed what you released")));
}

/// Rotation, the cultural control: Colorado potato beetles live in the soil
/// where potatoes grew (Cornell). Replant potatoes in an infested field and
/// the beetles are waiting and multiply; plant wheat there instead and they
/// have nothing to eat and fade, halving every 30 garden days; and when
/// potatoes come back after the wheat, they start from far fewer. Seen red
/// by making `decay` return the pressure unchanged (the wheat field then
/// kept its beetles).
#[test]
fn rotating_away_from_the_host_clears_the_pest_from_the_soil() {
    let season = |plant: &str| -> (f64, SoilMemory) {
        let data = store(100.0, 1.0);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        for slot in 0..4 {
            world.spawn((crop(&data, plant, "tuber_field_1", slot),));
        }
        seed_pest(&mut world, "tuber_field_1", "colorado_potato_beetle", 0.5, true);
        // Thirty garden days (12 ticks a day at 100x). The field is at the
        // weather's default 20 C, above the beetle's 15.6 C.
        run(&mut sys, &mut world, &data, 360, 1.0);
        (level(&world, "tuber_field_1", "colorado_potato_beetle"), memory(&world))
    };
    let (replanted, _) = season("potato");
    let (rotated, mem) = season("wheat");
    assert!(replanted > 0.5, "potatoes again: the beetles multiply ({replanted})");
    assert!((rotated - 0.25).abs() < 0.01, "wheat: half gone in 30 days ({rotated})");

    // Potatoes after the wheat start from the lower level.
    let data = store(100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    world.spawn((mem,));
    for slot in 0..4 {
        world.spawn((crop(&data, "potato", "tuber_field_1", slot),));
    }
    sys.tick(&mut world, 0.016, &data);
    let back = level(&world, "tuber_field_1", "colorado_potato_beetle");
    assert!(back < replanted / 2.0, "after a rotation the potatoes meet {back}, not {replanted}");
}

/// Pests live where their sources say: the outdoor-only pests (caterpillars,
/// slugs, potato beetles) never appear on the same crops indoors, and slugs
/// build faster in damp weather. Seen red by making `favour` ignore where a
/// pest lives (the indoor kale then grew caterpillars).
#[test]
fn outdoor_pests_stay_outdoors_and_slugs_like_the_damp() {
    let data = store(100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..4 {
        world.spawn((crop(&data, "kale", "ntower_1", slot),));
        world.spawn((crop(&data, "potato", "potato_grow_bed", slot),));
        world.spawn((crop(&data, "kale", "veg_field_1", slot),));
    }
    run(&mut sys, &mut world, &data, 240, 1.0);
    assert_eq!(level(&world, "ntower_1", "cabbage_caterpillar"), 0.0, "no caterpillars indoors");
    assert_eq!(level(&world, "potato_grow_bed", "colorado_potato_beetle"), 0.0, "no beetles indoors");
    assert!(level(&world, "veg_field_1", "cabbage_caterpillar") > 0.0, "caterpillars in the field");

    // Slugs on lettuce outdoors, dry against foggy.
    let slugs = |condition: crate::systems::weather::WeatherCondition| -> f64 {
        let mut data = store(100.0, 1.0);
        data.insert(
            "weather",
            std::sync::Mutex::new(crate::systems::weather::Weather { condition, ..Default::default() }),
        );
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        for slot in 0..4 {
            world.spawn((crop(&data, "lettuce", "veg_field_1", slot),));
        }
        run(&mut sys, &mut world, &data, 240, 1.0);
        level(&world, "veg_field_1", "slug")
    };
    let dry = slugs(crate::systems::weather::WeatherCondition::Clear);
    let damp = slugs(crate::systems::weather::WeatherCondition::Fog);
    assert!(dry > 0.0 && damp > dry * 3.0, "damp weather favours slugs: {damp} vs {dry}");
}

/// The pests are saved: they live on `SoilMemory`, which `WorldSave` saves
/// whole as JSON, so a round trip keeps each area's pressure, whether the
/// player was told, and a release's days left; and a save from before pests
/// existed loads with none. Seen red by removing `#[serde(default)]` from
/// `SoilMemory::pests` (the old save then failed to load).
#[test]
fn pests_survive_a_save_and_old_saves_load_with_none() {
    let mut mem = SoilMemory::default();
    let mut area = AreaPests::default();
    area.pressure.insert("aphid".into(), PestPressure { level: 0.37, told: true });
    area.releases.insert("predatory_mites".into(), 12.5);
    mem.pests.insert("ntower_1".into(), area);
    let json = serde_json::to_string(&mem).unwrap();
    let back: SoilMemory = serde_json::from_str(&json).unwrap();
    let a = &back.pests["ntower_1"];
    assert_eq!(a.pressure["aphid"], PestPressure { level: 0.37, told: true });
    assert_eq!(a.releases["predatory_mites"], 12.5);
    let old: SoilMemory = serde_json::from_str(r#"{"units":{},"organic":{}}"#).unwrap();
    assert!(old.pests.is_empty());
}

/// The insecticidal soap recipe is the dilution the sources give: one 100 g
/// bar of soap in 5 L of water is 1.96% soap, inside Clemson HGIC's "usually
/// used as a 1 to 2% solution" and above the 12.5 g per litre at which
/// Tremblay et al. 2009 saw aphid mortality "close to 100%". The water comes
/// from the tap (a measure of `water_purified_0`), and the recipe makes no
/// mass. Seen red by giving the recipe 2 L of water (4.8%).
#[test]
fn the_soap_spray_is_a_one_to_two_percent_solution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let recipes = std::fs::read_to_string(root.join("data/recipes.csv")).unwrap();
    let row = recipes
        .lines()
        .find(|l| l.starts_with("mix_insecticidal_soap,"))
        .expect("the recipe is in recipes.csv");
    let cols: Vec<&str> = row.split(',').collect();
    let parse = |s: &str| -> Vec<(String, f64)> {
        s.split('|')
            .map(|p| {
                let (id, q) = p.split_once(':').unwrap();
                (id.to_string(), q.parse().unwrap())
            })
            .collect()
    };
    let (inputs, outputs) = (parse(cols[3]), parse(cols[4]));
    let items = crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(root.join("data/items.csv")).unwrap())
        .unwrap();
    let kg = |v: &[(String, f64)]| -> f64 { v.iter().map(|(id, q)| f64::from(items.mass_for(id)) * q).sum() };
    let soap = inputs.iter().find(|(id, _)| id == "soap_bar_0").expect("made from soap").1
        * f64::from(items.mass_for("soap_bar_0"));
    let water = inputs.iter().find(|(id, _)| id == "water_purified_0").expect("and tap water").1
        * f64::from(items.mass_for("water_purified_0"));
    let strength = soap / (soap + water);
    assert!((0.01..=0.02).contains(&strength), "a 1-2% solution, got {:.2}%", strength * 100.0);
    assert!((kg(&outputs) - kg(&inputs)).abs() < 1e-3, "no mass made or lost: {} vs {}", kg(&outputs), kg(&inputs));
}
