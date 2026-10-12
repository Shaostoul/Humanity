//! One save per world (ship homes increment 6, lane 1C, docs/design/ship-homes-increment-6-plan.md
//! 3.6): a server's save applies like your own home's, a fresh save puts every machine back to its
//! spawn level, the live save's file follows the slot, a restore of a server's save goes back
//! where the game reads it and is loaded by its path, the asteroid belt is data, and the mess
//! hall's meter rides the save.

use super::*;
use crate::ecs::components::{AsteroidBody, Battery, HomeMachine, MachineInstanceId, Name, WaterTank};
use crate::persistence::{SaveSlot, ShipMeals, ShipPlace};
use crate::systems::inventory::containers::Container;
use crate::systems::machine_levels::{HeldMachineLevels, MachineLevels};

fn data_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// A player in a world of its own, carrying `items`.
fn player_with(items: &[(&str, u32)]) -> hecs::World {
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(36);
    for (id, n) in items {
        inv.add_item(id, *n, 99);
    }
    world.spawn((
        Controllable,
        inv,
        PlayerSkills::new(),
        Name("Ann".to_string()),
        crate::ecs::components::Appearance::default(),
        crate::ecs::components::Outfit::default(),
        crate::ecs::components::Wallet::default(),
    ));
    world
}

fn items_of(world: &hecs::World) -> Vec<(String, u32)> {
    extract_world_save(world).inventory
}

/// The DataStore a load reads, as the save_load tests build it.
fn data_store() -> crate::hot_reload::data_store::DataStore {
    let mut data = crate::hot_reload::data_store::DataStore::new();
    data.insert("auto_mine_order", std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
    crate::systems::crafting::register(&mut data);
    crate::systems::livestock::register(&mut data);
    data
}

fn a_server() -> SaveSlot {
    SaveSlot::Server { server: "https://example.test".into(), identity: "ab".repeat(32) }
}

/// A SERVER'S SAVE APPLIES. Your home and things on a server are a save of their own ("server",
/// kept on this computer), applied to the world exactly as your own home's is; before increment 6
/// `apply_save_to_world` returned at once for anything but "offline", so taking up a server's save
/// would have left your own home's backpack, skills and credits in that server's world.
///
/// Seen red 2026-10-12 with the "offline only" return kept (`if save.kind != "offline" { return;
/// }`): "a server's save puts its backpack on the world / left: [(\"wood_plank_0\", 5)] / right:
/// [(\"water_purified_0\", 2)]".
#[test]
fn a_server_save_applies() {
    let mut world = player_with(&[("wood_plank_0", 5)]);
    let mut save = WorldSave::new_offline("My Cabin", "cabin");
    save.kind = "server".into();
    save.server = "https://example.test".into();
    save.inventory = vec![("water_purified_0".into(), 2)];
    save.credits = 1_234;
    apply_save_to_world(&mut world, &save);
    assert_eq!(items_of(&world), vec![("water_purified_0".to_string(), 2)], "a server's save puts its backpack on the world");
    assert_eq!(extract_world_save(&world).credits, 1_234, "and its credits");
    assert!(applies_to_the_world(&save));
    // Its progress loads whole even with "Start every session from the default home" on, which is
    // about your own home; your own home's gives only the character then.
    assert!(applies_progress(&save, true));
    let mut own = save.clone();
    own.kind = "offline".into();
    assert!(!applies_progress(&own, true) && applies_progress(&own, false));
    own.progress_saved = false;
    assert!(!applies_progress(&own, false), "a save that carries no progress gives only the character");
}

/// A FRESH SAVE PUTS EVERY MACHINE BACK TO ITS SPAWN LEVEL. A fresh save lists no machine levels,
/// and an empty list used to change nothing, so a server's fresh save applied over your own home
/// kept your home's charged bank, full tank and stocked fuel drum (and a silo's grain still held
/// for world entry). `machine_levels::reset_all` runs before the save's levels are put back: half
/// a bank, half a tank, an empty clean drum, nothing held.
///
/// Seen red 2026-10-12 without the `reset_all` call in `apply_save_to_world`: "the offline
/// battery's 9.0 kWh came into the fresh home / left: [9000.0] / right: [5000.0]".
#[test]
fn a_fresh_save_resets_every_machines_level() {
    let mut world = player_with(&[]);
    world.spawn((
        HomeMachine,
        MachineInstanceId("battery_bank_0".into()),
        Battery { charge_wh: 9_000.0, capacity_wh: 10_000.0, max_charge_w: 1.0, max_discharge_w: 1.0 },
    ));
    world.spawn((HomeMachine, MachineInstanceId("water_tank_0".into()), WaterTank { liters: 900.0, capacity_l: 1_000.0 }));
    let mut drum = Container::new("steel_fuel_drum", 200.0);
    drum.current_content_item = Some("fuel_refined_0".into());
    drum.current_qty = 20;
    drum.used_liters = 20.0;
    drum.last_content = Some("fuel_refined_0".into());
    drum.toxic_from = Some("fuel_refined_0".into());
    world.spawn((HomeMachine, MachineInstanceId("generator_0".into()), drum));
    let mut grain = Container::new("grain_silo_bin", 5_000.0);
    grain.current_content_item = Some("wheat_grain_0".into());
    grain.current_qty = 400;
    world.spawn((HeldMachineLevels(vec![MachineLevels { id: "grain_silo_0".into(), vessel: Some(grain), ..Default::default() }]),));

    let data_dir = data_dir();
    let starts = crate::ship::starts::Starts::load(&data_dir);
    let fresh = crate::engine::save_slot::fresh_save(
        &data_dir,
        &starts,
        "homestead",
        "family",
        "https://example.test",
        &crate::engine::save_slot::Look::default(),
    );
    assert!(fresh.machine_levels.is_empty(), "a fresh save lists no machine");
    apply_save_to_world(&mut world, &fresh);

    let bank: Vec<f32> = world.query::<&Battery>().iter().map(|(_, b)| b.charge_wh).collect();
    assert_eq!(bank, vec![5_000.0], "the offline battery's 9.0 kWh came into the fresh home");
    let tank: Vec<f32> = world.query::<&WaterTank>().iter().map(|(_, t)| t.liters).collect();
    assert_eq!(tank, vec![500.0], "the tank at its spawn half");
    let drums: Vec<Container> = world.query::<&Container>().iter().map(|(_, c)| c.clone()).collect();
    assert_eq!(drums.len(), 1);
    assert!(drums[0].is_empty() && drums[0].last_content.is_none() && drums[0].toxic_from.is_none(), "an empty, clean drum: {:?}", drums[0]);
    assert_eq!(drums[0].capacity_liters, 200.0, "its type and size kept");
    assert_eq!(world.query::<&HeldMachineLevels>().iter().count(), 0, "nothing of your home waits for world entry");
}

/// THE LIVE SAVE'S FILE FOLLOWS THE SLOT. With a server's save live, every save of the live save
/// (the periodic one, the one on quit) goes to that server's file, says it is that server's save
/// with its start, household and place, and keeps its progress even while "Start every session
/// from the default home" is on (every caller passes that setting as `keep_progress`). The path is
/// checked BEFORE anything is written, so the red run below wrote nothing to a real saves folder.
///
/// Seen red 2026-10-12 with the fixed offline path (`active_home` returning
/// `ActiveHome::offline()` whatever was set): "every save of a server's save went to your own
/// home's file / left: \"C:\\Users\\..\\AppData\\Roaming\\HumanityOS\\saves\\offline_home.json\" /
/// right: \"..\\hum_active_slot_..\\servers\\a8b66addfd4bfab2-abababababababab.json\"" (and the real
/// offline_home.json was untouched: the assertion stops the test before the save).
#[test]
fn the_active_save_path_follows_the_slot() {
    let saves = crate::test_temp::dir("active_slot");
    let slot = a_server();
    let path = persistence::slot_path(&saves, &slot);
    let place = ShipPlace { ship_id: "mothership-1".into(), ship_hash: "h".into(), plot: "c2".into() };
    set_active_home(ActiveHome {
        slot: slot.clone(),
        path: path.clone(),
        start: "cabin".into(),
        household: "solo".into(),
        server: "https://example.test".into(),
        ship_place: Some(place.clone()),
    });
    assert_eq!(active_home_path(), path, "every save of a server's save went to your own home's file");

    let world = player_with(&[("water_purified_0", 2)]);
    let data = crate::hot_reload::data_store::DataStore::new();
    save_active_home(&world, &[], &data, false, None);
    let written = persistence::load_world(&path).expect("the server's save was written to its own file");
    assert_eq!((written.kind.as_str(), written.start.as_str(), written.household.as_str()), ("server", "cabin", "solo"));
    assert_eq!(written.server, "https://example.test");
    assert_eq!(written.ship_place, Some(place));
    assert!(written.progress_saved, "a server's save keeps its progress whatever your own home's setting says");
    assert_eq!(written.inventory, vec![("water_purified_0".to_string(), 2)]);
    assert!(!saves.join(persistence::OFFLINE_SAVE_FILE).exists(), "your own home's file is not touched");
}

/// A RESTORE RELOADS THE LIVE SAVE BY ITS PATH. Settings > Data lists a server's save's snapshots
/// with your own home's (they are kept in backups/saves/ like every other, not in the saves
/// folder) and names its save saves/<slot>.json; the snapshot goes back to saves/servers/, where the
/// game reads it, never beside your own home as a new character; and since it is the live save it
/// is held and loaded into the running game by its path (`take_pending_load`), because a server's
/// save is not among the saves the picker finds by name. Your own home's restore still names it for
/// today's loader, and the path load drops that name so it is loaded once.
///
/// Seen red 2026-10-12 with the fixed offline path (`active_home` returning
/// `ActiveHome::offline()`): "a restore of the live server save holds the saves until it is
/// loaded".
#[test]
fn a_restore_reloads_the_active_slot() {
    let root = crate::test_temp::dir("restore_slot");
    let saves = root.join("saves");
    let slot = a_server();
    let path = persistence::slot_path(&saves, &slot);
    let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
    let mut before = WorldSave::new_offline("My Cabin", "cabin");
    before.kind = "server".into();
    before.inventory = vec![("water_purified_0".into(), 2)];
    persistence::save_world(&path, &before).unwrap();
    let t = 1_759_400_000_000u64;
    let snap = persistence::snapshot_save(&path, &persistence::snapshots_root_for(&path), t, true).unwrap().expect("kept");
    assert!(
        snap.starts_with(root.join("backups").join("saves").join(&stem)),
        "a server's save keeps its snapshots with the others: {}",
        snap.display()
    );
    let mut after = before.clone();
    after.inventory.clear();
    persistence::write_atomic(&path, serde_json::to_string_pretty(&after).unwrap().as_bytes()).unwrap();

    set_active_home(ActiveHome::of_save(slot, path.clone(), &before));
    let mut gui = crate::gui::GuiState::default();
    let named_by_settings = persistence::snapshot_slot_save(&saves, &stem);
    assert_eq!(named_by_settings, path, "the slot's save is the server's file");
    let line = restore_snapshot_into_game(&mut gui, &snap, &saves.join(format!("{stem}.json"))).unwrap();
    assert!(restored_save_waiting(), "a restore of the live server save holds the saves until it is loaded");
    assert!(line.starts_with("Restored. Your home is being loaded back"), "{line}");
    assert!(!saves.join(format!("{stem}.json")).exists(), "not put beside your own home as a new character");
    assert!(gui.launcher_pending_load.is_none(), "a server's save is never looked up by name");
    let loaded = take_pending_load(&mut gui).expect("the restored server save is loaded into the game");
    assert_eq!(loaded.inventory, before.inventory, "the snapshot's backpack");
    assert!(take_pending_load(&mut gui).is_none(), "loaded once");
    let mut world = player_with(&[]);
    apply_save_to_world(&mut world, &loaded);
    assert!(!restored_save_waiting(), "the load lets the saves go on");

    // Your own home: also named for the loader lib.rs has today, and loaded once.
    let home = saves.join(persistence::OFFLINE_SAVE_FILE);
    persistence::save_world(&home, &WorldSave::new_offline("My Homestead", "homestead")).unwrap();
    let snap = persistence::snapshot_save(&home, &persistence::snapshots_root_for(&home), t + 1_000, true).unwrap().expect("kept");
    let mut own = ActiveHome::offline();
    own.path = home.clone();
    set_active_home(own);
    restore_snapshot_into_game(&mut gui, &snap, &home).unwrap();
    assert_eq!(gui.launcher_pending_load.as_deref(), Some("My Homestead"));
    assert_eq!(take_pending_load(&mut gui).map(|s| s.name), Some("My Homestead".to_string()));
    assert!(gui.launcher_pending_load.is_none(), "the same save is not loaded a second time by its name");
    apply_identity(&mut world, &WorldSave::new_offline("x", "homestead"));
}

/// THE ASTEROID BELT IS DATA, AND THE SAME AS BEFORE. The three asteroids a new world starts with
/// moved from lib.rs's startup into data/asteroids/belt.ron, so a server's fresh save starts with
/// the same belt. Here they are exactly the three lib.rs spawned (written out below as they stood
/// there, src/lib.rs:1418-1463 at e03db4a7e), the copy built into the exe is the file, every ore
/// is a real item, and `fresh_asteroids` spawns them.
///
/// Seen red 2026-10-12 with m12's iron changed to 121.0 in data/asteroids/belt.ron: "the belt is
/// not the three asteroids lib.rs spawned / left: Array [Object {.. \"id\": String(\"m12\"), ..
/// \"ores\": Array [Array [String(\"iron_ore_0\"), Number(121.0)], .. / right: .. Number(120.0) ..".
#[test]
fn the_asteroid_belt_is_data_and_the_same_as_before() {
    let rock = |id: &str, name: &str, class: &str, ores: &[(&str, f32)], position: [f32; 3]| AsteroidBody {
        id: id.into(),
        name: name.into(),
        classification: class.into(),
        ores: ores.iter().map(|(o, n)| (o.to_string(), *n)).collect(),
        position,
    };
    let before = vec![
        rock(
            "m12",
            "Asteroid M-12 (metallic)",
            "M",
            &[("iron_ore_0", 120.0), ("nickel_ore_0", 60.0), ("platinum_ore_0", 20.0), ("gold_ore_0", 15.0), ("silver_ore_0", 25.0)],
            [60.0, 12.0, -30.0],
        ),
        rock(
            "s7",
            "Asteroid S-7 (silicaceous)",
            "S",
            &[("iron_ore_0", 40.0), ("copper_ore_0", 50.0), ("bauxite_0", 45.0), ("rutile_0", 30.0)],
            [-45.0, 8.0, 55.0],
        ),
        rock("c3", "Asteroid C-3 (carbonaceous)", "C", &[("graphite_0", 90.0), ("iron_ore_0", 25.0)], [30.0, -6.0, 70.0]),
    ];
    let json = |rocks: &[AsteroidBody]| serde_json::to_value(rocks).unwrap();
    let belt = asteroid_belt(&data_dir());
    assert_eq!(json(&belt), json(&before), "the belt is not the three asteroids lib.rs spawned");

    // The copy built into the exe is the file (a data folder without it gets the same belt).
    let empty = crate::test_temp::dir("no_belt");
    assert_eq!(json(&asteroid_belt(&empty)), json(&before), "the built-in belt");

    let items = crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap();
    for a in &belt {
        for (ore, n) in &a.ores {
            assert!(items.items.contains_key(ore) && *n > 0.0, "{}'s {ore} is not an item, or none", a.id);
        }
    }
    let mut world = hecs::World::new();
    fresh_asteroids(&mut world, &data_dir());
    let mut ids: Vec<String> = world.query::<&AsteroidBody>().iter().map(|(_, a)| a.id.clone()).collect();
    ids.sort();
    assert_eq!(ids, vec!["c3", "m12", "s7"]);
}

/// THE MESS HALL'S METER RIDES THE SAVE. The meals and water the ship's stores gave this save away
/// from a server are counted on this computer (`ShipMeals`, in the DataStore under
/// `SHIP_MEALS_KEY`): a save writes it, a load at startup (`resume_home`) and a load onto the
/// running world (`put_save_on_world`) put it back, and a session that never registered the
/// meter keeps the save's rather than writing it back to zero.
///
/// Seen red 2026-10-12 with the meter not written by `save_home_at`: "the meter is in the save /
/// left: ShipMeals { taken: 0, next_at_game_s: 0.0, water_l: 0.0 } / right: ShipMeals { taken: 3,
/// next_at_game_s: 86400.5, water_l: 4.5 }".
#[test]
fn the_ship_meals_meter_round_trips() {
    let root = crate::test_temp::dir("ship_meals");
    let path = root.join("saves").join(persistence::OFFLINE_SAVE_FILE);
    let meals = ShipMeals { taken: 3, next_at_game_s: 86_400.5, water_l: 4.5 };
    let mut data = crate::hot_reload::data_store::DataStore::new();
    put_ship_meals(&mut data, &meals);
    let world = player_with(&[]);
    save_home_at(&path, &world, &[], &data, true, None);
    let saved = persistence::load_world(&path).unwrap();
    assert_eq!(saved.ship_meals, meals, "the meter is in the save");

    // At startup: registered empty, then the save's put back by the load.
    let mut back = data_store();
    register_ship_meals(&mut back);
    assert_eq!(ship_meals(&back), Some(ShipMeals::default()));
    let mut fresh = player_with(&[]);
    apply_save_to_world(&mut fresh, &saved);
    resume_home(&mut fresh, &back, &saved, false, 1.0, None);
    assert_eq!(ship_meals(&back), Some(meals.clone()), "a load at startup puts it back");

    // Onto the running world (the picker, a restore, a change of save slot), registered or not.
    let mut running = crate::hot_reload::data_store::DataStore::new();
    let mut gui = crate::gui::GuiState::default();
    put_save_on_world(&mut fresh, &mut running, &mut gui, &data_dir(), &saved, true);
    assert_eq!(ship_meals(&running), Some(meals.clone()), "a load onto the running world puts it back");

    // A session with no meter (nothing counted) keeps what the save had.
    save_home_at(&path, &world, &[], &crate::hot_reload::data_store::DataStore::new(), true, None);
    assert_eq!(persistence::load_world(&path).unwrap().ship_meals, meals, "not written back to zero");
}
