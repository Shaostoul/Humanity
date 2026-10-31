//! The save half of the swap (ship homes increment 6, lane 1C): which save is live, and a server's
//! save made fresh, carrying only the look across.

use super::*;
use crate::ecs::components::{
    AsteroidBody, Battery, Controllable, CropInstance, Drone, DronePhase, Health, HomeMachine, MachineInstanceId,
    Name, Transform, Vehicle, Wallet,
};
use crate::systems::inventory::Inventory;
use crate::systems::quests::QuestTracker;
use crate::systems::skills::{PlayerSkills, SkillProgress};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The DataStore a load reads (the registries `resume_home` reaches for), as the save_load tests
/// build it.
fn data_store() -> crate::hot_reload::data_store::DataStore {
    let mut data = crate::hot_reload::data_store::DataStore::new();
    data.insert("auto_mine_order", std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
    crate::systems::crafting::register(&mut data);
    crate::systems::livestock::register(&mut data);
    data
}

/// The backpack, as (item, how many) summed per item and sorted.
fn backpack(world: &hecs::World) -> Vec<(String, u32)> {
    let mut out: std::collections::BTreeMap<String, u32> = Default::default();
    for (_e, (inv, _)) in world.query::<(&Inventory, &Controllable)>().iter() {
        for s in inv.slots.iter().flatten() {
            *out.entry(s.item_id.clone()).or_default() += s.quantity;
        }
    }
    out.into_iter().collect()
}

/// A kit as `backpack` reads one.
fn summed(kit: &[(String, u32)]) -> Vec<(String, u32)> {
    let mut out: std::collections::BTreeMap<String, u32> = Default::default();
    for (id, n) in kit {
        *out.entry(id.clone()).or_default() += n;
    }
    out.into_iter().collect()
}

/// A SERVER WORLD STARTS EVERY PLAYER FRESH; ONLY THE LOOK CROSSES (the operator, 2026-10-04).
/// Your own home after a long time playing: a backpack with seven gold ingots in it, a farming
/// skill, 50,000 CR, First Steps done, a home of your own, a crop growing, a truck parked, the
/// drone out with ore in its hold, an asteroid mined down, a charged battery bank, a hurt body, a
/// barn of gold. Taking up a server's save that does not exist yet (`take_up_before_home` and
/// `take_up_after_home`, all of `take_up` but the home's rebuild) writes a fresh Cabin save there and
/// puts it on the world: the Cabin's kit only, no skills, the default credits, no own home, the
/// Cabin's opening (never the Homestead's First Steps), no crops, vehicles or drone, the belt's
/// fresh asteroids, a fresh body, the bank back at its spawn charge, the Cabin's own storage, and
/// the same name, look and outfit. "Start every session from the default home" is ON here, as in
/// the operator's own settings: it is about your own home and must not make a server's save load
/// only the character (which would keep the whole offline world).
///
/// Seen red 2026-10-12 with `take_up_before_home` carrying the live backpack into the fresh save
/// (`applied.inventory = save_load::extract_world_save(world).inventory` before the apply): "the
/// offline backpack's 7 gold ingots came into the server's save: [(\"gold_ingot_0\", 7), ...]".
#[test]
fn a_fresh_server_save_carries_only_the_look() {
    let data_dir = data_dir();
    let starts = Starts::load(&data_dir);
    let saves = crate::test_temp::dir("fresh_server_save");
    let mut data = data_store();
    let mut gui = crate::gui::GuiState::default();
    gui.settings.fresh_world_each_launch = true;

    // ── Your own home, well lived in ──
    let mut world = hecs::World::new();
    let player = crate::save_load::spawn_fresh_player(&mut world, &data_dir, "homestead");
    world.get::<&mut Inventory>(player).unwrap().add_item("gold_ingot_0", 7, 99);
    world.get::<&mut PlayerSkills>(player).unwrap().skills.insert("farming".into(), SkillProgress { level: 5, xp: 40 });
    world.get::<&mut Wallet>(player).unwrap().credits = 50_000;
    {
        let mut q = world.get::<&mut QuestTracker>(player).unwrap();
        q.active_quests.clear();
        q.completed_quests.push("gs_first_steps".into());
    }
    world.get::<&mut Name>(player).unwrap().0 = "Ann".into();
    world.get::<&mut crate::ecs::components::Appearance>(player).unwrap().height_scale = 1.2;
    world.get::<&mut crate::ecs::components::Outfit>(player).unwrap().equipped.insert("chest".into(), "work_jacket".into());
    world.get::<&mut Health>(player).unwrap().current = 20.0;
    world.spawn((CropInstance {
        crop_def_id: "tomato".into(),
        growth_stage: "seedling".into(),
        planted_at: 5.0,
        water_level: 0.8,
        health: 90.0,
        tower_id: None,
        tower_slot: None,
        health_seconds: 0.0,
        growing_seconds: 0.0,
    },));
    world.spawn((Vehicle { item_id: "truck_pickup_0".into() }, Transform::default()));
    world.spawn((Drone {
        home: 1,
        target: "m12".into(),
        manifest: vec![("iron_ore_0".into(), 4)],
        phase: DronePhase::Returning,
        phase_time: 1.0,
        cargo: vec![("iron_ore_0".into(), 4)],
        home_pos: [0.0; 3],
        target_pos: [0.0; 3],
    },));
    world.spawn((AsteroidBody {
        id: "m12".into(),
        name: "Asteroid M-12 (metallic)".into(),
        classification: "M".into(),
        ores: vec![("iron_ore_0".into(), 3.0)],
        position: [60.0, 12.0, -30.0],
    },));
    world.spawn((
        HomeMachine,
        MachineInstanceId("battery_bank_0".into()),
        Battery { charge_wh: 9_000.0, capacity_wh: 10_000.0, max_charge_w: 1.0, max_discharge_w: 1.0 },
    ));
    gui.own_home = Some(SavedHome {
        design: crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the homestead is built in"),
        machines: Default::default(),
    });
    gui.placed_items = vec![crate::systems::inventory::placed::PlacedItem {
        key: "gold_ingot_0".into(),
        name: "Gold ingot".into(),
        qty: 50,
        container: "1/0".into(),
        ..Default::default()
    }];

    // ── A server's save, taken up for the first time, as a Cabin ──
    let slot = SaveSlot::Server { server: "https://example.test".into(), identity: "ab".repeat(32) };
    let look = Look::of_world(&world);
    let taken = take_up_before_home(&mut world, &mut data, &mut gui, &saves, &data_dir, &starts, &slot, Some("cabin"), None, &look)
        .expect("a fresh save is made and taken up");
    take_up_after_home(&mut world, &data, &mut gui, &taken, &look);
    assert!(taken.fresh, "there was no save of this server yet");

    let cabin = starts.get("cabin").expect("the Cabin start");
    let kit = summed(&cabin.kit);
    let carried = backpack(&world);
    assert!(
        !carried.iter().any(|(id, _)| id == "gold_ingot_0"),
        "the offline backpack's 7 gold ingots came into the server's save: {carried:?}"
    );
    assert_eq!(carried, kit, "the Cabin's kit and nothing else");
    assert!(world.get::<&PlayerSkills>(player).unwrap().skills.is_empty(), "no skills");
    assert_eq!(world.get::<&Wallet>(player).unwrap().credits, Wallet::default().credits, "the default credits");
    assert!(gui.own_home.is_none(), "no home of your own there");
    let quests = world.get::<&QuestTracker>(player).unwrap().clone();
    let opening: Vec<String> = cabin.opening.iter().cloned().collect();
    assert_eq!(quests.active_quests.iter().map(|q| q.quest_id.clone()).collect::<Vec<_>>(), opening, "the Cabin's opening");
    assert!(quests.completed_quests.is_empty() && !quests.is_active("gs_first_steps"), "never the Homestead's First Steps: {quests:?}");
    assert_eq!(world.query::<&CropInstance>().iter().count(), 0, "no crops");
    assert_eq!(world.query::<&Vehicle>().iter().count(), 0, "no vehicles");
    assert_eq!(world.query::<&Drone>().iter().count(), 0, "no drone");
    let rocks: Vec<serde_json::Value> =
        world.query::<&AsteroidBody>().iter().map(|(_, a)| serde_json::to_value(a).unwrap()).collect();
    let belt: Vec<serde_json::Value> =
        crate::save_load::asteroid_belt(&data_dir).iter().map(|a| serde_json::to_value(a).unwrap()).collect();
    assert_eq!(rocks, belt, "the belt's fresh asteroids, not the ones mined down at home");
    assert_eq!(world.get::<&Health>(player).unwrap().current, Health::default().current, "a fresh body");
    let bank: Vec<f32> = world.query::<&Battery>().iter().map(|(_, b)| b.charge_wh).collect();
    assert_eq!(bank, vec![5_000.0], "the bank at its spawn charge, not your home's 9.0 kWh");
    assert!(!gui.placed_items.iter().any(|p| p.key == "gold_ingot_0"), "your barn's gold stays home: {:?}", gui.placed_items);
    let cabin_pool = crate::gui::flatten_placed_items(&crate::save_load::start_places(&data_dir, "cabin"));
    assert_eq!(
        gui.placed_items.iter().map(|p| p.key.clone()).collect::<Vec<_>>(),
        cabin_pool.iter().map(|p| p.key.clone()).collect::<Vec<_>>(),
        "the Cabin's own storage"
    );
    // The look crossed.
    assert_eq!(world.get::<&Name>(player).unwrap().0, "Ann");
    assert_eq!(world.get::<&crate::ecs::components::Appearance>(player).unwrap().height_scale, 1.2);
    assert_eq!(
        world.get::<&crate::ecs::components::Outfit>(player).unwrap().equipped.get("chest").map(String::as_str),
        Some("work_jacket")
    );
    assert_eq!(gui.character_name, "Ann");

    // The file the server's save is kept in says the same, and is the live save now.
    let path = crate::persistence::slot_path(&saves, &slot);
    assert_eq!(taken.path, path);
    let on_disk = crate::persistence::load_world(&path).expect("the fresh save was written");
    assert_eq!((on_disk.kind.as_str(), on_disk.start.as_str(), on_disk.household.as_str()), ("server", "cabin", "solo"));
    assert_eq!(on_disk.server, "https://example.test");
    assert_eq!(summed(&on_disk.inventory), kit);
    assert!(on_disk.skills.is_empty() && on_disk.crops.is_empty() && on_disk.deployed_vehicles.is_empty());
    assert!(on_disk.drone.is_none() && on_disk.home.is_none() && on_disk.body.is_none());
    assert!(on_disk.machine_levels.is_empty() && on_disk.settled_trades.is_empty() && on_disk.home_id.is_empty());
    assert_eq!(on_disk.credits, Wallet::default().credits);
    assert!(on_disk.progress_saved);
    assert_eq!(on_disk.character_name, "Ann");
    let active = crate::persistence::live_save::active_home();
    assert_eq!((active.slot, active.path, active.start.as_str()), (slot.clone(), path, "cabin"));
    assert_eq!(gui.save_slot, slot);
}

/// THE SAVE FOLLOWS THE PAIRING, NEVER A STEP OUT. The picker's pairing says which save is live:
/// a home pairing is your own home's, a server pairing (or none yet: a rig's fresh folder) is the
/// connected server's for your identity, and without an identity there is only your own home.
/// Dev travel, fly mode and the `solo` verb set `copresence_solo` for a moment while a server is
/// paired; that is a step out of the shared world, and must never swap the save (and so the
/// backpack) under the player.
///
/// Seen red 2026-10-12 with `wanted` reading `copresence_solo` (`if gui.copresence_solo { return
/// SaveSlot::Offline; }`): "a step out for Dev travel swapped the server's save for your own home /
/// left: Offline / right: Server { server: \"https://united-humanity.us\", identity: \"abab...\" }".
#[test]
fn the_slot_follows_the_pairing_not_a_step_out() {
    let mut gui = crate::gui::GuiState::default();
    gui.profile_public_key = "AB".repeat(32);
    gui.server_url = "https://united-humanity.us/".into();
    gui.connected_server_url = String::new();
    let official = SaveSlot::Server { server: "https://united-humanity.us".into(), identity: "ab".repeat(32) };

    gui.launcher_last_world = "home:My Homestead".into();
    assert_eq!(wanted(&gui), SaveSlot::Offline, "a home pairing is your own home");
    gui.launcher_last_world = "server:official".into();
    assert_eq!(wanted(&gui), official, "a server pairing is that server's save for you");
    gui.launcher_last_world = String::new();
    assert_eq!(wanted(&gui), official, "no pairing yet (a rig's fresh folder) is the server's, as copresence starts");

    // Steps out of the shared world, never the save.
    gui.launcher_last_world = "server:official".into();
    gui.copresence_solo = true;
    assert_eq!(wanted(&gui), official, "a step out for Dev travel swapped the server's save for your own home");
    gui.launcher_last_world = "home:My Homestead".into();
    gui.copresence_solo = false;
    assert_eq!(wanted(&gui), SaveSlot::Offline, "your own home stays yours whatever the join gate says");

    // The connected server's save, whichever it is; none without an identity.
    gui.launcher_last_world = "server:other".into();
    gui.connected_server_url = "https://other.example.test/".into();
    assert_eq!(
        wanted(&gui),
        SaveSlot::Server { server: "https://other.example.test".into(), identity: "ab".repeat(32) }
    );
    gui.profile_public_key = String::new();
    assert_eq!(wanted(&gui), SaveSlot::Offline, "no identity, no server save");
}

/// Your own home while "Start every session from the default home" is on is the DEFAULT home when
/// it is taken up again, as at a launch: a fresh home of its start with the character from the file,
/// never what the server's world held (a character-only apply would have left the server's
/// backpack in your own home). The file keeps its progress.
#[test]
fn your_own_home_with_a_fresh_start_each_session_comes_back_as_the_default_home() {
    let data_dir = data_dir();
    let starts = Starts::load(&data_dir);
    let mut saved = WorldSave::new_offline("My Homestead", "homestead");
    saved.inventory = vec![("gold_ingot_0".into(), 7)];
    saved.character_name = "Ann".into();
    saved.settled_trades = vec!["t1".into()];
    let applied = what_to_apply(&data_dir, &starts, &saved, true);
    assert_eq!(summed(&applied.inventory), summed(&starts.get("homestead").unwrap().kit), "the default home's kit");
    assert_eq!((applied.kind.as_str(), applied.character_name.as_str()), ("offline", "Ann"));
    assert_eq!(applied.settled_trades, vec!["t1".to_string()], "no old trade settles twice");
    let whole = what_to_apply(&data_dir, &starts, &saved, false);
    assert_eq!(whole.inventory, saved.inventory, "with the setting off, the save itself");
    let mut server = saved.clone();
    server.kind = "server".into();
    assert_eq!(what_to_apply(&data_dir, &starts, &server, true).inventory, saved.inventory, "a server's save loads whole");
}

/// A server's save file that this version cannot read is set aside beside it, never deleted, and a
/// fresh save made in its place; a save that reads is taken as it is.
#[test]
fn an_unreadable_server_save_is_set_aside_and_a_fresh_one_made() {
    let data_dir = data_dir();
    let starts = Starts::load(&data_dir);
    let saves = crate::test_temp::dir("unreadable_server_save");
    let slot = SaveSlot::Server { server: "https://example.test".into(), identity: "cd".repeat(32) };
    let path = crate::persistence::slot_path(&saves, &slot);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{ not a save").unwrap();
    let (save, at, fresh) = open_slot(&saves, &data_dir, &starts, &slot, Some("cabin"), None, &Look::default()).unwrap();
    assert!(fresh && at == path && save.start == "cabin");
    let aside: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".unreadable-"))
        .collect();
    assert_eq!(aside.len(), 1, "the unreadable file is kept beside it");
    assert_eq!(std::fs::read(aside[0].path()).unwrap(), b"{ not a save");
    // Taken again: the save just written, as it is.
    let (again, _, fresh) = open_slot(&saves, &data_dir, &starts, &slot, Some("homestead"), None, &Look::default()).unwrap();
    assert!(!fresh && again.start == "cabin", "an existing save is never remade");
}
