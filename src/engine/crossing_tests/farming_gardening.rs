//! From systems/farming/mod.rs (its `gardening_tests`): the garden's time away through a save
//! applied and caught up (`save_load`), and a router spawned the way the engine spawns it
//! (`engine::home_spawn`).

use humanity_core::ecs::systems::System;
use humanity_systems::farming::*;
use humanity_systems::farming::fixtures::gardening::*;

/// Offline progression end to end (2026-09-25): a garden saved, then loaded
/// after time away, must be further along when the toggle is on and exactly
/// where it was when it is off. Goes through the real catch-up and the real
/// FarmingSystem tick, so it proves the shifted planted_at actually turns
/// into growth, not just that a number moved. Native-gated because
/// save_load is (the relay build has no offline home).
#[cfg(feature = "native")]
#[test]
fn time_away_grows_the_garden_only_with_offline_progression_on() {
    use humanity_core::ecs::components::CropInstance;

    let run = |offline_on: bool| -> usize {
        let data = make_store();
        let (growth_seconds, stages): (f64, Vec<&str>) = {
            let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
            let def = reg.get("tomato").unwrap();
            (def.growth_days as f64 * SECONDS_PER_DAY, def.stages())
        };
        // Saved one minute after planting, at game second 1000.
        let mut save = humanity_systems::persistence::WorldSave::new_offline("t", "homestead");
        save.game_time = 1060.0;
        save.timestamp = 1_000_000;
        save.crops = vec![CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: stages[0].to_string(),
            planted_at: 1000.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: None,
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }];
        let mut world = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut world, &save);
        // Away for 60% of the tomato's whole real growth window.
        let now = save.timestamp + (growth_seconds * 0.6) as u64;
        let r = crate::save_load::catch_up_world(&mut world, &save, offline_on, 1.0, now);
        assert!((r.clock - 1060.0).abs() < 1e-9, "the clock resumes where it was saved");
        data.get::<std::sync::Mutex<humanity_systems::time::GameTime>>("game_time")
            .unwrap()
            .lock()
            .unwrap()
            .set_elapsed(r.clock);
        let mut sys = FarmingSystem::new();
        sys.tick(&mut world, 1.0, &data);
        let (_e, c) = world.query_mut::<&CropInstance>().into_iter().next().unwrap();
        stage_index(&c.growth_stage, &stages).unwrap()
    };

    let off = run(false);
    let on = run(true);
    assert_eq!(off, 0, "toggle off: one minute of growth is still the first stage");
    assert!(on > off, "toggle on: 60% of the window away must grow the crop (on {on}, off {off})");
}

/// A powered Wi-Fi router in the grow room leaves crop health exactly
/// where it would be without one (2026-09-27). The v0.620 radio-frequency
/// crop harm was removed on the operator's decision, "We'll assume no
/// wi-fi crop harm at this time", on the evidence in
/// docs/reference/findings/2026-09-27-wifi-and-plants.md (no source shows a
/// household router harming a garden at the distances plants sit from
/// one). The router is spawned from the shipped catalog through the same
/// path the game uses, with its power on, beside a well-watered crop, and
/// the crop's health is compared with the same crop in a room with no
/// router. Seen red with the old coupling restored (the RfEmitter spawn
/// plus the FarmingSystem's home-RF drain): after five seconds the crop by
/// the router fell to 78.0 while the one without it recovered to 82.5.
/// Native only: it spawns the router through `engine::home_spawn`, which
/// the relay build does not have.
#[cfg(feature = "native")]
#[test]
fn powered_wifi_router_leaves_crop_health_unchanged() {
    use humanity_core::ecs::components::{CropInstance, MachineInstanceId, PowerConsumer};
    let well_watered = || CropInstance {
        crop_def_id: "tomato".to_string(),
        growth_stage: "sprout".to_string(),
        planted_at: 0.0,
        water_level: 1.0, // not water-stressed, so only the router differs
        health: 80.0,
        tower_id: None,
        tower_slot: None,
        health_seconds: 0.0,
        growing_seconds: 0.0,
    };
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join("machines").join("home.ron");
    let home = humanity_systems::grow_machines::load_home(&path).expect("home.ron parses");
    let router = home.catalog.get("wifi_router").expect("home.ron catalogs the wifi_router");
    let inst = humanity_ship::machines::MachineInstance {
        id: "router_test".to_string(),
        machine: "wifi_router".to_string(),
        room: "room-greenhouse".to_string(),
        offset: (0.0, 0.0, 0.0),
        rotation: 0.0,
        zone: "home".to_string(),
        screen_source: None,
        screen_size_m: None,
    };
    let data = make_store();
    let empty = std::collections::HashMap::new();

    // The grow room with the router, powered on.
    let mut with_router = hecs::World::new();
    let c = with_router.spawn((well_watered(),));
    crate::engine::home_spawn::spawn_home_machine_entity(&mut with_router, &inst, router, &empty, &empty, None, None);
    let powered: Vec<bool> = with_router
        .query::<(&PowerConsumer, &MachineInstanceId)>()
        .iter()
        .map(|(_, (p, _))| p.enabled)
        .collect();
    assert_eq!(powered, vec![true], "the router spawns as a powered network device");

    // The same grow room with no router.
    let mut without = hecs::World::new();
    let c2 = without.spawn((well_watered(),));

    let mut sys = FarmingSystem::new();
    let mut sys2 = FarmingSystem::new();
    for _ in 0..5 {
        sys.tick(&mut with_router, 1.0, &data);
        sys2.tick(&mut without, 1.0, &data);
    }
    let near_router = with_router.get::<&CropInstance>(c).unwrap().health;
    let no_router = without.get::<&CropInstance>(c2).unwrap().health;
    assert_eq!(near_router, no_router, "a powered Wi-Fi router changes nothing about the crop's health");
    assert!(no_router >= 80.0, "the well-watered crop holds or recovers, got {no_router}");
}
