//! The swap and the home (ship homes increment 6, lane 2A): the home a save's start builds, and on
//! which plot; the join waiting for the server's own save; the look crossing at every entry; and a
//! home chosen on a server's card.

use super::*;
use crate::engine::home_plot::{add_join_fields, join_step, JoinGate, JoinStep};
use crate::ship::starts::{HeldHome, HomeAnswer};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The DataStore a load reads, as the save half's tests build it.
fn data_store() -> crate::hot_reload::data_store::DataStore {
    let mut data = crate::hot_reload::data_store::DataStore::new();
    data.insert("auto_mine_order", std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
    crate::systems::crafting::register(&mut data);
    crate::systems::livestock::register(&mut data);
    data
}

fn server(url: &str) -> SaveSlot {
    SaveSlot::Server { server: crate::gui::pages::chat::norm_server_url(url), identity: "ab".repeat(32) }
}

fn plot_of(ship: &ShipStructure) -> Option<String> {
    ship.home_plot().map(|p| p.id.clone())
}

/// A SERVER'S HOME IS THE SHIPPED DESIGN, NEVER THE DISK FILE (plan 3.7). Everyone on a server
/// draws your home there as the design shipped with the app (src/ship/neighbours.rs), and an
/// offline Dev session can rewrite data/homes/homestead.ron; so outside the Dev mode a server's save
/// builds the shipped design whatever that file holds, while your own home builds the file's
/// (here with one wall more than the shipped one), and the Dev mode builds the file it authors.
///
/// Seen red 2026-10-12 with `design_of` reading `HomeDesign::load` for every save: "a server's home
/// was built from the disk file: 1 walls more than the shipped homestead".
#[test]
fn a_server_home_is_the_shipped_design_never_the_disk_file() {
    let repo = data_dir();
    let dir = crate::test_temp::path("server_home_shipped_design");
    let ship_file = dir.join("blueprints").join("ship_structure.ron");
    std::fs::create_dir_all(ship_file.parent().unwrap()).unwrap();
    std::fs::copy(repo.join("blueprints").join("ship_structure.ron"), &ship_file).unwrap();
    let shipped = HomeDesign::built_in("homestead").expect("the homestead is built in");
    let mut edited = shipped.clone();
    let mut wall = edited.body.walls.first().cloned().expect("the homestead has walls");
    wall.a = (10.0, 10.0);
    wall.b = (20.0, 10.0);
    edited.body.walls.push(wall);
    edited.save(&dir.join("homes").join("homestead.ron")).unwrap();
    let starts = Starts::load(&repo);
    let file = ShipStructure::load_ship_file(&dir).expect("the ship file loads");
    let walls = |ship: &ShipStructure| ship.home_design().expect("a home").body.walls.len();
    let shipped_walls = shipped.body.walls.len();

    let on_server = home_for(file.clone(), &dir, &starts, "homestead", "family", None, None, false, &server("https://a.example"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        walls(&on_server),
        shipped_walls,
        "a server's home was built from the disk file: {} walls more than the shipped homestead",
        walls(&on_server) as i64 - shipped_walls as i64
    );
    let own_home = home_for(file.clone(), &dir, &starts, "homestead", "family", None, None, false, &SaveSlot::Offline).unwrap();
    assert_eq!(walls(&own_home), shipped_walls + 1, "your own home is the design file's");
    let dev = home_for(file, &dir, &starts, "homestead", "family", None, None, true, &server("https://a.example")).unwrap();
    assert_eq!(walls(&dev), shipped_walls + 1, "the Dev mode builds the file it authors, on a server too");
}

/// A CABIN SAVE BUILDS A CABIN ON ITS PLOT, AND A HOMESTEAD SAVE A HOMESTEAD (plan 3.7): the plot
/// is the one the save stands on when it is of the start's kind on this ship, else the ship's first
/// plot of that kind; the design is the start's (the shipped one on a server); and the character's
/// own design of another kind is never put on it.
///
/// Seen red 2026-10-12 with `home_for` building every save as the default start (`start_row`
/// answering the default whatever was asked): "the cabin save's home: Some(\"p1\") a homestead".
#[test]
fn a_cabin_save_builds_a_cabin_on_its_plot_and_a_homestead_save_a_homestead() {
    let starts = Starts::load(&data_dir());
    let file = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
    let slot = server("https://a.example");
    let c2 = place_on(Some(&file), "c2");
    let build = |start: &str, place: Option<&ShipPlace>, own: Option<&SavedHome>| {
        home_for(file.clone(), &data_dir(), &starts, start, "", place, own, false, &slot).unwrap_or_else(|e| panic!("{e}"))
    };
    let kind = |s: &ShipStructure| s.home_design().map(|d| d.kind).unwrap_or_default();

    let cabin = build("cabin", c2.as_ref(), None);
    assert_eq!((plot_of(&cabin), kind(&cabin)), (Some("c2".to_string()), "cabin".to_string()), "the cabin save's home: {:?} a {}", plot_of(&cabin), kind(&cabin));
    let first = build("cabin", None, None);
    assert_eq!(plot_of(&first).as_deref(), Some("c1"), "a cabin save with no place yet: the first cabin plot");
    let homestead = build("homestead", None, None);
    assert_eq!((plot_of(&homestead).as_deref(), kind(&homestead).as_str()), (Some("p1"), "homestead"));
    // A cabin's place is no homestead's: the homestead save builds on its own kind's plot.
    let mismatched = build("homestead", c2.as_ref(), None);
    assert_eq!(plot_of(&mismatched).as_deref(), Some("p1"), "a homestead was put on a cabin plot");
    // A place taken on another ship is not trusted.
    let elsewhere = ShipPlace { ship_id: "mothership-1".into(), ship_hash: "0123456789abcdef".into(), plot: "c2".into() };
    assert_eq!(plot_of(&build("cabin", Some(&elsewhere), None)).as_deref(), Some("c1"), "another ship's place was trusted");
    // Your own homestead design does not go on a cabin plot: the cabin's does.
    let own = SavedHome { design: HomeDesign::built_in("homestead").unwrap(), machines: Default::default() };
    assert_eq!(kind(&build("cabin", c2.as_ref(), Some(&own))), "cabin");
}

/// A SWITCH OF SERVER SWAPS THE SAVE BEFORE THE JOIN (plan 3.6). Playing server A's save, a
/// Cabin, the game switches to server B. Until B's own save is live, the join gate waits
/// (`slot_ready`): a join then would ask B for A's start. Once `follow` has taken up B's save (made
/// fresh, the default start), the join goes, naming B's save's start.
///
/// Seen red 2026-10-12 with `slot_ready` answering true whatever the live save: "joined server B
/// asking for server A's \"cabin\"".
#[test]
fn switching_servers_swaps_saves_before_the_join() {
    let data_dir = data_dir();
    let starts = Starts::load(&data_dir);
    let saves = crate::test_temp::dir("switching_servers");
    let mut data = data_store();
    let mut gui = GuiState::default();
    gui.profile_public_key = "AB".repeat(32);
    gui.launcher_last_world = format!("server:{CONNECTED_SERVER_ROW}");
    gui.connected_server_url = "https://a.example/".into();
    let mut world = hecs::World::new();
    crate::save_load::spawn_fresh_player(&mut world, &data_dir, "homestead");
    let look = Look::of_world(&world);
    let a = wanted(&gui);
    assert_eq!(a, server("https://a.example"));
    take_up_before_home(&mut world, &mut data, &mut gui, &saves, &data_dir, &starts, &a, Some("cabin"), None, &look).unwrap();
    assert!(slot_ready(&gui), "server A's own save is live");
    let ship = ShipStructure::load_and_assemble_shipped(&data_dir, None).unwrap();
    let gate = |gui: &GuiState| JoinGate {
        in_world: true,
        joined: false,
        identified: true,
        solo: false,
        aboard: true,
        refused_here: false,
        world_loaded: true,
        has_ship: true,
        slot_ready: slot_ready(gui),
    };
    let join_start = || {
        let mut join = serde_json::json!({ "type": "game_join" });
        assert!(add_join_fields(&mut join, Some(&ship), &crate::save_load::active_home().start));
        join["start"].clone()
    };

    // The switch.
    gui.connected_server_url = "https://b.example/".into();
    let b = wanted(&gui);
    assert_eq!(b, server("https://b.example"));
    if join_step(&gate(&gui)) == JoinStep::Join {
        panic!("joined server B asking for server A's {}", join_start());
    }
    // `follow`'s swap: B's save, made fresh.
    let taken = take_up_before_home(&mut world, &mut data, &mut gui, &saves, &data_dir, &starts, &b, None, None, &look).unwrap();
    assert!(taken.fresh);
    assert_eq!(join_step(&gate(&gui)), JoinStep::Join, "server B's own save is live, and the join goes");
    assert_eq!(join_start(), serde_json::json!("homestead"), "the join names server B's own save's start");
    // Your own home never joins a server's world.
    gui.launcher_last_world = "home:My Homestead".into();
    assert!(!slot_ready(&gui), "your own home's pairing joined a server's world");
}

/// THE LOOK CROSSES AT EVERY ENTRY (plan 3.6 step 4): a server's save kept from an earlier visit
/// carries the look the character had then; taking it up again puts on the look the character has
/// NOW (changed at home since), name and outfit too, and only the look: its things stay that
/// server's.
///
/// Seen red 2026-10-12 with `take_up_after_home` not putting `look` on: "the server's save brought
/// back its old look: \"Old\" at height 0.9".
#[test]
fn the_look_crosses_at_every_entry() {
    let data_dir = data_dir();
    let starts = Starts::load(&data_dir);
    let saves = crate::test_temp::dir("look_crosses");
    let slot = server("https://a.example");
    // An earlier visit: the server's save with the look the character had then, and its own water.
    let mut then = Look { name: "Old".into(), ..Default::default() };
    then.appearance.height_scale = 0.9;
    open_slot(&saves, &data_dir, &starts, &slot, Some("cabin"), None, &then).unwrap();
    // Now, at home: a new name, a new height and a jacket.
    let mut world = hecs::World::new();
    let player = crate::save_load::spawn_fresh_player(&mut world, &data_dir, "homestead");
    world.get::<&mut crate::ecs::components::Name>(player).unwrap().0 = "Ann".into();
    world.get::<&mut crate::ecs::components::Appearance>(player).unwrap().height_scale = 1.2;
    world.get::<&mut crate::ecs::components::Outfit>(player).unwrap().equipped.insert("chest".into(), "work_jacket".into());
    let now = Look::of_world(&world);
    let mut data = data_store();
    let mut gui = GuiState::default();
    let taken = take_up_before_home(&mut world, &mut data, &mut gui, &saves, &data_dir, &starts, &slot, None, None, &now).unwrap();
    assert!(!taken.fresh, "the earlier visit's save");
    take_up_after_home(&mut world, &data, &mut gui, &taken, &now);
    let name = world.get::<&crate::ecs::components::Name>(player).unwrap().0.clone();
    let height = world.get::<&crate::ecs::components::Appearance>(player).unwrap().height_scale;
    assert_eq!((name.as_str(), height), ("Ann", 1.2), "the server's save brought back its old look: {name:?} at height {height}");
    assert_eq!(
        world.get::<&crate::ecs::components::Outfit>(player).unwrap().equipped.get("chest").map(String::as_str),
        Some("work_jacket")
    );
    assert_eq!((gui.character_name.as_str(), gui.appearance.height_scale), ("Ann", 1.2), "the editing copies wear it too");
    // Only the look: the backpack is the server save's (the Cabin's water), not your own home's.
    let cabin_kit: Vec<String> = starts.get("cabin").unwrap().kit.iter().map(|(id, _)| id.clone()).collect();
    let carried: Vec<String> = world
        .get::<&crate::systems::inventory::Inventory>(player)
        .unwrap()
        .slots
        .iter()
        .flatten()
        .map(|s| s.item_id.clone())
        .collect();
    assert!(carried.iter().all(|id| cabin_kit.contains(id)), "your own home's things came along: {carried:?}");
}

/// A HOME CHOSEN AS A GUEST JOINS AGAIN TO TAKE IT (plan 3.5, 3.7). The relay hands out a plot
/// only at a join, so a player standing in a server's world as a guest who chooses a Cabin there
/// gets `chose: true` with the plot now held (c2): the answer becomes the choice for this
/// identity's save on that server (its start, the start's first household, the plot as a place on
/// this ship), and because that save is live and the game stands in the world as a guest, the game
/// steps out and joins again, so the next welcome gives the plot and builds the cabin there
/// (home_plot.rs `a_welcome_of_another_kind_builds_that_kinds_home_and_keeps_the_plot`). The other
/// cases: a save not live waits for its fresh save, or has the choice written into it; a live save
/// out of the world builds the home at once; a plain request's answer chooses nothing.
///
/// Seen red 2026-10-12 with `take_answer` ignoring `chose` (an answer only kept for the card): "a
/// home chosen as a guest was not taken: None".
#[test]
fn a_home_chosen_as_a_guest_joins_again_to_take_it() {
    let mut gui = GuiState::default();
    gui.profile_public_key = "ab".repeat(32);
    gui.connected_server_url = "https://a.example".into();
    gui.ship_structure = Some(ShipStructure::load_and_assemble_shipped(&data_dir(), None).unwrap());
    gui.save_slot = server_slot(&gui).expect("an identity on a server");
    let answer = HomeAnswer {
        plot: Some(HeldHome { id: "c2".into(), kind: "cabin".into(), start: "cabin".into() }),
        starts: Vec::new(),
        chose: true,
        refused: None,
        message: String::new(),
    };
    let taken = take_answer(&mut gui, answer.clone());
    let Some((slot, choice)) = taken else { panic!("a home chosen as a guest was not taken: {taken:?}") };
    assert_eq!(slot, gui.save_slot, "the choice is for this identity's save on this server");
    assert_eq!((choice.start.as_str(), choice.household.as_str()), ("cabin", "solo"));
    let hash = gui.ship_structure.as_ref().unwrap().ship_hash();
    assert_eq!(choice.place, Some(ShipPlace { ship_id: "mothership-1".into(), ship_hash: hash, plot: "c2".into() }));
    assert_eq!(gui.home_answer.as_ref(), Some(&answer), "the answer is kept for the card and the probe");
    assert_eq!(after_choice(true, true, true, Some("guest")), AfterChoice::JoinAgain, "a guest stays a guest, its plot held for nobody");
    // The other cases.
    assert_eq!(after_choice(false, false, false, None), AfterChoice::Wait, "chosen from your own home: waits for that server's fresh save");
    assert_eq!(after_choice(false, true, false, None), AfterChoice::WriteInto);
    assert_eq!(after_choice(true, true, false, Some("guest")), AfterChoice::BuildNow, "out of the shared world: built at once");
    assert_eq!(after_choice(true, true, true, Some("stay")), AfterChoice::Stay);
    // A plain request's answer chooses nothing.
    let asked = HomeAnswer { chose: false, ..answer };
    assert!(take_answer(&mut gui, asked).is_none());
}

/// THE CHOICE WAITS FOR ITS FRESH SAVE, AND THAT SAVE STANDS ON THE PLOT CHOSEN (plan 3.6, the
/// rig's step 4 and 5): a Cabin chosen on a server while your own home is live is kept for that
/// server's save, and the save then made is a Cabin standing on the plot chosen, so the home is
/// built there before the join and the welcome only confirms it (a Stay).
#[test]
fn a_cabin_chosen_from_home_is_the_servers_fresh_save_on_its_plot() {
    let data_dir = data_dir();
    let saves = crate::test_temp::dir("chosen_from_home");
    let slot = server("https://a.example");
    let file = ShipStructure::load_ship_file(&data_dir).unwrap();
    let choice = Choice { start: "cabin".into(), household: "solo".into(), place: place_on(Some(&file), "c2") };
    remember_choice(&slot, choice.clone());
    assert_eq!(choice_for(&slot), Some(choice.clone()));
    write_chosen(&saves, &data_dir, &slot, &choice, &Look::default()).unwrap();
    let save = crate::persistence::load_world(&crate::persistence::slot_path(&saves, &slot)).unwrap();
    assert_eq!((save.kind.as_str(), save.start.as_str(), save.household.as_str()), ("server", "cabin", "solo"));
    assert_eq!(save.ship_place, choice.place);
    let starts = Starts::load(&data_dir);
    let home = home_for(file, &data_dir, &starts, &save.start, &save.household, save.ship_place.as_ref(), None, false, &slot).unwrap();
    assert_eq!(plot_of(&home).as_deref(), Some("c2"), "the fresh save's home stands on the plot chosen");
    forget_choice(&slot);
    assert_eq!(choice_for(&slot), None);
}

/// The boot's slot and the follow's agree: an erased account on that server is your own home's
/// until a sign-up there again, and a swap waits for the world, a restore and the editor.
#[test]
fn the_slot_rules_at_boot_and_in_the_frame() {
    assert_eq!(slot_for("server:x", "AB", "https://a.example", false), SaveSlot::Server { server: "https://a.example".into(), identity: "ab".into() });
    assert_eq!(slot_for("server:x", "ab", "https://a.example", true), SaveSlot::Offline, "an erased account's save stays live");
    assert_eq!(slot_for("", "ab", "", false), SaveSlot::Offline, "no server, no server save");
    assert!(may_swap(true, false, false));
    assert!(!may_swap(false, false, false) && !may_swap(true, true, false) && !may_swap(true, false, true));
    assert_eq!(boot_slot(None), SaveSlot::Offline, "a first run is your own home");
}
