//! THE OPENING (2026-10-04): a new player's first ten minutes, as the quest
//! data in data/quests/getting_started.ron, played on the shipped data with
//! the real systems. "Check your vitals, eat, plant, craft a tool, send the
//! drone for iron, smelt it, build one thing, and end at your front door
//! looking out on the Commons" (the operator, docs/design/first-hour-audit-
//! 2026-10-04.md).
//!
//! Each new objective kind has its test against the system that reports it
//! (the food system for Eat, the farming system for Plant, the Inventory page
//! and the frame glue for View, the ship for the own front door), then the
//! walk through the whole opening, the HUD's 64 characters for every step of
//! every shipped quest, and a returning player who finished the old two-step
//! First Steps. Native only: it reads the starting kit and the tower designs
//! through the app's own loaders, and draws the Inventory page.

use super::*;
use crate::ecs::components::{AsteroidBody, Controllable, StatusEffects, Transform, Vitals, Wallet};
use crate::ship::ship_structure::ShipStructure;
use crate::systems::construction::{placement, BlueprintRegistry, BuildRequest, ConstructionSystem};
use crate::systems::crafting::{CraftingSystem, RecipeRegistry};
use crate::systems::farming::{FarmingSystem, PlantRegistry};
use crate::systems::food::FoodSystem;
use crate::systems::inventory::ItemRegistry;
use crate::systems::mining::DroneSystem;
use crate::systems::skills::SkillRegistry;
use glam::Vec3;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(data_dir().join(rel)).unwrap_or_else(|e| panic!("data/{rel}: {e}"))
}

/// The shipped quests, destinations and registries, and every channel the
/// opening's systems read, registered as lib.rs registers them.
fn opening_store() -> DataStore {
    let dir = data_dir();
    let mut data = DataStore::new();
    data.insert("quest_registry", QuestRegistry::from_ron_dir(&dir.join("quests")));
    data.insert("quest_destinations", DestinationList::from_ron(&read("entities/destinations.ron")).expect("destinations parse"));
    data.insert("item_registry", ItemRegistry::from_csv(&read("items.csv")).expect("items.csv"));
    data.insert("skill_registry", SkillRegistry::from_csv(&read("skills/skills.csv")).expect("skills.csv"));
    let tools = crate::systems::crafting::tools::load(&dir);
    data.insert("recipe_registry", RecipeRegistry::from_csv(&read("recipes.csv")).expect("recipes.csv").with_tools(&tools));
    data.insert("blueprint_registry", BlueprintRegistry::from_ron(&read("blueprints/basic.ron")).expect("basic.ron"));
    data.insert("plant_registry", PlantRegistry::from_csv(&read("plants.csv")).expect("plants.csv"));
    data.insert(
        "status_effect_registry",
        crate::systems::status_effects::StatusEffectRegistry::from_csv(&read("status_effects.csv")).expect("status_effects.csv"),
    );
    for key in ["quest_events", "player_notices"] {
        data.insert(key, Mutex::new(Vec::<String>::new()));
    }
    data.insert("xp_grants", Mutex::new(Vec::<crate::systems::skills::SkillXPEvent>::new()));
    for key in ["consume_request", "drink_request", "plant_request", "craft_request"] {
        data.insert(key, Mutex::new(Option::<String>::None));
    }
    data.insert("plant_tower_request", Mutex::new(Option::<(String, Vec<String>)>::None));
    data.insert("plant_bed_request", Mutex::new(Option::<(String, String, u32)>::None));
    data.insert("showcase_tower_request", Mutex::new(Option::<(String, String)>::None));
    data.insert("commission_drone", Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
    data.insert("build_request", Mutex::new(Vec::<BuildRequest>::new()));
    data.insert("build_status", Mutex::new(String::new()));
    data.insert("creative_mode", Mutex::new(false));
    // The home's storage as a new home holds it (data/places/seed.json, the
    // Barn), and the channel what lands in it rides until the main loop files it.
    data.insert("home_stock", Mutex::new(barn()));
    data.insert("home_stock_outputs", Mutex::new(Vec::<(String, u32)>::new()));
    data
}

/// The machine types a home layout places (data/machines/<file>), as the
/// game publishes them for the station gate (`placed_machine_types`,
/// engine::built_uses::publish_stations: every placed row and array cell).
fn placed_in(home_file: &str) -> std::collections::HashSet<String> {
    let home = crate::machines::MachineHome::load(&data_dir().join("machines").join(home_file)).unwrap_or_else(|| panic!("{home_file} parses"));
    home.all_instances().into_iter().map(|i| i.machine).collect()
}

/// What the home's Barn holds at the start (data/places/seed.json).
fn barn() -> HashMap<String, u32> {
    fn walk(v: &serde_json::Value, out: &mut HashMap<String, u32>) {
        if let (Some(item), Some(qty)) = (v.get("item").and_then(|i| i.as_str()), v.get("qty").and_then(|q| q.as_u64())) {
            *out.entry(item.to_string()).or_insert(0) += qty as u32;
        }
        for c in v.get("children").and_then(|c| c.as_array()).into_iter().flatten() {
            walk(c, out);
        }
    }
    let seed: serde_json::Value = serde_json::from_slice(&read("places/seed.json")).expect("seed.json parses");
    let mut out = HashMap::new();
    for e in seed["entities"].as_array().expect("seed.json lists entities") {
        if e["id"] == "home" {
            walk(e, &mut out);
        }
    }
    assert!(out.get("wood_plank_0").copied().unwrap_or(0) > 0, "the Barn parsed: {out:?}");
    out
}

/// The ship a new player boots, with the shipped home on `plot`.
fn ship_on(plot: &str) -> ShipStructure {
    ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).unwrap_or_else(|e| panic!("the home assembles on {plot}: {e}"))
}

/// The world lib.rs spawns for a new player: the starting kit, fresh vitals,
/// the wallet, and First Steps accepted. Standing where they wake up.
fn new_player(world: &mut hecs::World, ship: &ShipStructure) -> hecs::Entity {
    let mut inv = Inventory::new(36);
    for (id, qty) in crate::save_load::starting_kit(&data_dir()) {
        inv.add_item(&id, qty, 99);
    }
    let mut tracker = QuestTracker::default();
    tracker.accept_quest("gs_first_steps");
    let spawn = ship.home_spawn_world().expect("the home has a spawn");
    world.spawn((
        Transform { position: spawn, ..Default::default() },
        Controllable,
        inv,
        Vitals::default(),
        StatusEffects::default(),
        Wallet::default(),
        crate::systems::skills::PlayerSkills::new(),
        tracker,
    ))
}

/// The game's systems of the opening, ticked in lib.rs's order, with the main
/// loop's two parts that matter here: the camera published where the player
/// stands, and what landed in home storage filed after the tick.
struct Game {
    data: DataStore,
    world: hecs::World,
    player: hecs::Entity,
    ship: ShipStructure,
    farming: FarmingSystem,
    crafting: CraftingSystem,
    construction: ConstructionSystem,
    food: FoodSystem,
    drones: DroneSystem,
    quests: QuestSystem,
}

impl Game {
    /// The family home on `plot`, in the Normal play mode.
    fn new(plot: &str) -> Self {
        Self::of("home.ron", plot, false)
    }

    /// A new player in the home laid out by `home_file` (its placed machines
    /// are the stations), on `plot`, in the Dev play mode when `dev` (its free
    /// resources: crafting and planting take nothing) or else Normal.
    fn of(home_file: &str, plot: &str, dev: bool) -> Self {
        let mut data = opening_store();
        data.insert("placed_machine_types", Mutex::new(placed_in(home_file)));
        *data.get::<Mutex<bool>>("creative_mode").unwrap().lock().unwrap() = dev;
        let ship = ship_on(plot);
        // The game publishes the front door every frame (engine::quest_hooks).
        publish_front_door(&mut data, Some(&ship));
        let mut world = hecs::World::new();
        let player = new_player(&mut world, &ship);
        data.insert("camera_position", ship.home_spawn_world().unwrap());
        // The asteroid the drone mines iron from (lib.rs spawns M-12 at boot).
        world.spawn((AsteroidBody {
            id: "m12".to_string(),
            name: "Asteroid M-12 (metallic)".to_string(),
            classification: "M".to_string(),
            ores: vec![("iron_ore_0".to_string(), 120.0)],
            position: [60.0, 12.0, -30.0],
        },));
        Game {
            data,
            world,
            player,
            ship,
            farming: FarmingSystem::new(),
            crafting: CraftingSystem::new(),
            construction: ConstructionSystem::new(),
            food: FoodSystem::new(&data_dir()),
            drones: DroneSystem::new(),
            quests: QuestSystem::new(),
        }
    }

    fn tick(&mut self) {
        let dt = 1.0;
        self.farming.tick(&mut self.world, dt, &self.data);
        self.crafting.tick(&mut self.world, dt, &self.data);
        self.construction.tick(&mut self.world, dt, &self.data);
        self.food.tick(&mut self.world, dt, &self.data);
        self.drones.tick(&mut self.world, dt, &self.data);
        self.quests.tick(&mut self.world, dt, &self.data);
        // engine::stock_piles::receive_machine_outputs: what landed is filed.
        let landed: Vec<(String, u32)> =
            std::mem::take(&mut *self.data.get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs").unwrap().lock().unwrap());
        let mut stock = self.data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap();
        for (id, q) in landed {
            *stock.entry(id).or_insert(0) += q;
        }
    }

    /// The opening's step the player is on (its length once it is done).
    fn step(&self) -> usize {
        let t = self.world.get::<&QuestTracker>(self.player).unwrap();
        match t.active_quests.iter().find(|q| q.quest_id == "gs_first_steps") {
            Some(q) => q.current_step,
            None if t.is_completed("gs_first_steps") => opening().steps.len(),
            None => panic!("First Steps is neither under way nor done"),
        }
    }

    fn notices(&self) -> Vec<String> {
        std::mem::take(&mut *self.data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
    }

    fn request(&self, key: &str, value: &str) {
        *self.data.get::<Mutex<Option<String>>>(key).unwrap().lock().unwrap() = Some(value.to_string());
    }

    fn stand_at(&mut self, xz: (f32, f32)) {
        self.data.insert("camera_position", Vec3::new(xz.0, 1.7, xz.1));
    }

    fn doorstep(&self) -> (f32, f32) {
        let door = self.data.get::<FrontDoor>(FRONT_DOOR_KEY).expect("the front door is published");
        door.doorstep(DOORSTEP_OUT_M)
    }

    /// Tick until the opening leaves step `from`, at most `max` ticks, and
    /// return the notices it posted on the way.
    fn until_past(&mut self, from: usize, max: usize, what: &str) -> Vec<String> {
        let mut said = Vec::new();
        for _ in 0..max {
            self.tick();
            said.extend(self.notices());
            if self.step() != from {
                assert_eq!(self.step(), from + 1, "{what}: the opening moved on by exactly one step");
                return said;
            }
        }
        panic!("{what}: the opening stayed on step {} for {max} ticks (notices: {said:?})", from + 1);
    }
}

/// The shipped opening.
fn opening() -> QuestDef {
    QuestRegistry::from_ron_dir(&data_dir().join("quests")).get("gs_first_steps").expect("First Steps ships").clone()
}

/// How far out of the door the shipped front-door destination stands.
const DOORSTEP_OUT_M: f32 = 1.5;

/// What the player reads for step `i` of the opening on `game`'s plot.
fn line(game: &Game, i: usize) -> String {
    step_text(&opening().steps[i].description, &game.data)
}

// ── Eat ────────────────────────────────────────────────────────────────

/// AN EAT STEP COUNTS WHAT THE PLAYER EATS (2026-10-04). The food system
/// reports an item as eaten where it leaves the backpack; a click on food the
/// player does not carry, and a drink, count nothing.
///
/// Red, run with the food system's report taken out: "eating a ration
/// finishes the Eat step".
#[test]
fn eating_counts_only_what_the_player_really_eats() {
    /// A world with one player carrying `carried`, on a one-step Eat quest
    /// naming `item` (None: any food), and the food and quest systems.
    struct Meal {
        data: DataStore,
        world: hecs::World,
        player: hecs::Entity,
        food: FoodSystem,
        quests: QuestSystem,
    }
    impl Meal {
        fn new(item: Option<&str>, carried: &[&str]) -> Self {
            let mut data = opening_store();
            let mut reg = QuestRegistry::default();
            let objective = QuestObjective::Eat { item_id: item.map(String::from), quantity: 1 };
            reg.quests.insert(
                "q_eat".into(),
                QuestDef {
                    id: "q_eat".into(),
                    name: "Eat".into(),
                    description: String::new(),
                    steps: vec![QuestStep { description: "Eat".into(), objective }],
                    rewards: vec![],
                    xp_rewards: vec![],
                    prerequisite: None,
                },
            );
            data.insert("quest_registry", reg);
            let mut tracker = QuestTracker::default();
            tracker.accept_quest("q_eat");
            let mut inv = Inventory::new(16);
            for id in carried {
                inv.add_item(id, 1, 99);
            }
            let mut world = hecs::World::new();
            let player = world.spawn((inv, Vitals::default(), StatusEffects::default(), Controllable, tracker));
            Meal { data, world, player, food: FoodSystem::new(&data_dir()), quests: QuestSystem::new() }
        }
        /// Press Eat (or Drink) on `id`, and run a frame.
        fn press(&mut self, channel: &str, id: &str) {
            *self.data.get::<Mutex<Option<String>>>(channel).unwrap().lock().unwrap() = Some(id.to_string());
            self.food.tick(&mut self.world, 1.0, &self.data);
            self.quests.tick(&mut self.world, 1.0, &self.data);
        }
        fn carries(&self, id: &str) -> u32 {
            self.world.get::<&Inventory>(self.player).unwrap().count_item(id)
        }
        fn done(&self) -> bool {
            self.world.get::<&QuestTracker>(self.player).unwrap().is_completed("q_eat")
        }
    }

    let mut meal = Meal::new(None, &["water_purified_0"]);
    meal.press("consume_request", "ration_basic_0");
    assert!(!meal.done(), "a click on food the player does not carry is not eating");
    meal.press("drink_request", "water_purified_0");
    assert_eq!(meal.carries("water_purified_0"), 0, "the water was drunk");
    assert!(!meal.done(), "drinking is not eating");
    meal.world.get::<&mut Inventory>(meal.player).unwrap().add_item("ration_basic_0", 1, 99);
    meal.press("consume_request", "ration_basic_0");
    assert_eq!(meal.carries("ration_basic_0"), 0, "the ration was eaten");
    assert!(meal.done(), "eating a ration finishes the Eat step");

    // An Eat step that names an item counts only that item.
    let mut meal = Meal::new(Some("canned_food_0"), &["ration_basic_0", "canned_food_0"]);
    meal.press("consume_request", "ration_basic_0");
    assert!(!meal.done(), "a ration is not the canned food asked for");
    meal.press("consume_request", "canned_food_0");
    assert!(meal.done(), "the canned food finishes it");
}

// ── Plant ──────────────────────────────────────────────────────────────

/// A PLANT STEP COUNTS EVERY CROP THE PLAYER SOWS (2026-10-04), by any of the
/// three ways the game plants: a seed from the backpack, a tower design's
/// Plant button, a bed's. The showcase garden filling a tower is not the
/// player planting, and a Plant step naming a crop counts only that crop.
///
/// Red, run with the farming system's reports taken out: "a seed from the
/// backpack counts as planting" (left: 0, right: 1).
#[test]
fn planting_counts_every_way_the_player_sows_and_nothing_else() {
    let data = opening_store();
    let mut world = hecs::World::new();
    let mut tracker = QuestTracker::default();
    // Progress of an active quest records every event; a Plant step that is
    // never done keeps the quest open while the counts are read.
    let mut reg = QuestRegistry::default();
    for (id, crop) in [("q_any", None), ("q_tomato", Some("tomato"))] {
        reg.quests.insert(
            id.into(),
            QuestDef {
                id: id.into(),
                name: id.into(),
                description: String::new(),
                steps: vec![QuestStep { description: String::new(), objective: QuestObjective::Plant { crop_id: crop.map(String::from), quantity: 99 } }],
                rewards: vec![],
                xp_rewards: vec![],
                prerequisite: None,
            },
        );
        tracker.accept_quest(id);
    }
    let mut data = data;
    data.insert("quest_registry", reg);
    let mut inv = Inventory::new(36);
    for seed in ["seed_lettuce_0", "seed_tomato_0", "seed_potato_0", "seed_wheat_0"] {
        inv.add_item(seed, 5, 99);
    }
    let player = world.spawn((inv, Controllable, tracker));
    let mut farming = FarmingSystem::new();
    let mut quests = QuestSystem::new();
    let mut run = |world: &mut hecs::World| {
        farming.tick(world, 1.0, &data);
        quests.tick(world, 1.0, &data);
    };
    let planted = |world: &hecs::World, quest: &str| {
        let t = world.get::<&QuestTracker>(player).unwrap();
        let q = t.active_quests.iter().find(|q| q.quest_id == quest).unwrap();
        counted(&q.progress, PLANT_EVENT, None)
    };

    // A seed from the backpack.
    *data.get::<Mutex<Option<String>>>("plant_request").unwrap().lock().unwrap() = Some("seed_lettuce_0".into());
    run(&mut world);
    assert_eq!(planted(&world, "q_any"), 1, "a seed from the backpack counts as planting");
    // A tower design: one crop per cup the player has a seed for.
    *data.get::<Mutex<Option<(String, Vec<String>)>>>("plant_tower_request").unwrap().lock().unwrap() =
        Some(("nutrition".into(), vec!["lettuce".into(), "tomato".into(), "kale".into()]));
    run(&mut world);
    assert_eq!(planted(&world, "q_any"), 3, "the tower planted the lettuce and the tomato, not the kale it had no seed for");
    // A bed of potatoes, two plots.
    *data.get::<Mutex<Option<(String, String, u32)>>>("plant_bed_request").unwrap().lock().unwrap() =
        Some(("potato_grow_bed".into(), "potato".into(), 2));
    run(&mut world);
    assert_eq!(planted(&world, "q_any"), 5, "the bed planted its two plots");
    // The showcase fills a tower: not the player planting.
    *data.get::<Mutex<Option<(String, String)>>>("showcase_tower_request").unwrap().lock().unwrap() =
        Some(("apothecary".into(), "basil".into()));
    run(&mut world);
    assert!(world.query::<&crate::ecs::components::CropInstance>().iter().count() > 50, "the showcase filled its tower");
    assert_eq!(planted(&world, "q_any"), 5, "the showcase's crops are not counted");
    // A Plant step naming a crop counts only that crop.
    assert_eq!(
        counted(&world.get::<&QuestTracker>(player).unwrap().active_quests[1].progress, PLANT_EVENT, Some("tomato")),
        1,
        "one tomato was planted"
    );
}

/// A PLANT BUTTON THAT PLANTS NOTHING SAYS WHY (2026-10-04). It was only in
/// the log, so the opening's plant step could sit on a button that did
/// nothing and no reason on screen.
///
/// Red, run without the notice: "a tower press with no seed for its plants
/// says so" (left: [], right: the line below).
#[test]
fn a_plant_button_that_plants_nothing_says_why() {
    let data = opening_store();
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    let mut farming = FarmingSystem::new();
    let notices = |data: &DataStore| std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap());
    *data.get::<Mutex<Option<(String, Vec<String>)>>>("plant_tower_request").unwrap().lock().unwrap() =
        Some(("apothecary".into(), vec!["basil".into(), "mint".into()]));
    farming.tick(&mut world, 1.0, &data);
    assert_eq!(
        notices(&data),
        vec!["Nothing planted: your backpack holds no seed for what this grows (harvesting most ripe plants gives their seed back).".to_string()],
        "a tower press with no seed for its plants says so"
    );
    // Every cup taken: in Dev's free planting the tower fills, and a second press says it is full.
    *data.get::<Mutex<bool>>("creative_mode").unwrap().lock().unwrap() = true;
    for _ in 0..2 {
        *data.get::<Mutex<Option<(String, Vec<String>)>>>("plant_tower_request").unwrap().lock().unwrap() =
            Some(("apothecary".into(), vec!["basil".into(), "mint".into()]));
        farming.tick(&mut world, 1.0, &data);
    }
    assert_eq!(
        notices(&data),
        vec!["Nothing planted: every cup of this tower already has a plant. Harvest what is ripe to make room.".to_string()],
        "a press on a full tower says so, and the first press planted without a word"
    );
}

// ── View ───────────────────────────────────────────────────────────────

/// A VIEW STEP IS DONE BY OPENING THE VIEW (2026-10-04). "Check your
/// vitals": the Inventory page, drawn with the vitals synced, marks them on
/// screen in its Status section; the frame glue (engine::quest_hooks) turns
/// that into one quest event; the step is done. Before the world syncs the
/// vitals (the menus) the page shows none, and nothing counts.
///
/// Why opening the view and not a timer on the HUD's rows: with the HUD's
/// default "When low", a healthy player has no survival rows on screen at all,
/// so a timer keyed to them would never fire, or would have to force the rows
/// on and then finish on its own whether or not anyone looked.
///
/// Red, run with the page's mark taken out: "opening Inventory > Status
/// finishes the View step" (left: 0, right: 1).
#[test]
fn opening_inventory_status_finishes_the_vitals_step() {
    let mut data = opening_store();
    let mut world = hecs::World::new();
    let mut tracker = QuestTracker::default();
    tracker.accept_quest("gs_first_steps");
    let player = world.spawn((Controllable, tracker, Inventory::new(8)));
    let mut quests = QuestSystem::new();
    let mut gui = crate::gui::GuiState::default();
    gui.active_page = crate::gui::GuiPage::Inventory;
    let draw = |gui: &mut crate::gui::GuiState| {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 3000.0))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| crate::gui::pages::inventory::draw(ctx, &theme, gui));
        }
    };
    let frame = |gui: &mut crate::gui::GuiState, data: &mut DataStore| {
        for view in crate::engine::quest_hooks::views_that_came_on_screen(gui) {
            push_quest_event(data, view_event_key(view));
        }
    };

    // The menus: no vitals synced, none shown, nothing counts.
    draw(&mut gui);
    frame(&mut gui, &mut data);
    quests.tick(&mut world, 1.0, &data);
    assert_eq!(world.get::<&QuestTracker>(player).unwrap().active_quests[0].current_step, 0, "no vitals on screen before the world");
    // In the world: the vitals synced (lib.rs's bridge), the page drawn.
    gui.vitals.satiation_max = 100.0;
    gui.vitals.satiation = 80.0;
    gui.vitals.hydration_max = 100.0;
    gui.vitals.hydration = 80.0;
    draw(&mut gui);
    frame(&mut gui, &mut data);
    quests.tick(&mut world, 1.0, &data);
    assert_eq!(world.get::<&QuestTracker>(player).unwrap().active_quests[0].current_step, 1, "opening Inventory > Status finishes the View step");
}

// ── The own front door ─────────────────────────────────────────────────

/// THE FRONT DOOR IS THE DOOR OF THE PLOT YOU WERE GIVEN (2026-10-04), never a
/// fixed coordinate. For every plot of the shipped ship, with the shipped
/// home on it: the door is where that plot's corridor leaves the home, it
/// opens on that plot's door zone (the Commons for p1, First Street for p2),
/// the doorstep the opening's last step asks for is a step out of it, the
/// spot the player wakes up in is not, and standing on the doorstep finishes
/// the step. A guest, whose home is put away, has no front door aboard.
///
/// Red, run with the destination as the fixed point outside p1's door,
/// (56.5, 40): "on plot p2, standing on its own doorstep finishes the step"
/// (left: 7, right: 8).
#[test]
fn the_front_door_is_the_door_of_the_plot_you_were_given() {
    let file = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
    assert!(file.plots.len() >= 2, "the ship has a family plot and a street plot");
    let design = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the homestead design is built in");
    let mut opens_on = Vec::new();
    for plot in &file.plots {
        let ship = ship_on(&plot.id);
        let door = front_door_of(&ship).unwrap_or_else(|| panic!("plot {} has a front door", plot.id));
        let expected = (plot.origin.0 + design.door.0, plot.origin.2 + design.door.1);
        assert!(
            (door.door.0 - expected.0).abs() < 1e-3 && (door.door.1 - expected.1).abs() < 1e-3,
            "plot {}: the door is the home's own, at {expected:?}, got {:?}",
            plot.id,
            door.door
        );
        let zone = ship.zones.iter().find(|z| z.id == plot.door.zone).expect("the door zone exists");
        assert_eq!(door.opens_on, zone.label, "plot {}: it opens on its door zone", plot.id);
        // The way out leads toward that zone.
        let to_zone = (zone.origin.0 + zone.body.width * 0.5 - door.door.0, zone.origin.2 + zone.body.depth * 0.5 - door.door.1);
        assert!(door.outward.0 * to_zone.0 + door.outward.1 * to_zone.1 > 0.0, "plot {}: out of the door is toward {}", plot.id, zone.label);
        opens_on.push(door.opens_on.clone());

        let mut game = Game::new(&plot.id);
        game.world.get::<&mut QuestTracker>(game.player).unwrap().active_quests[0].current_step = opening().steps.len() - 1;
        game.tick();
        assert_eq!(game.step(), opening().steps.len() - 1, "plot {}: waking up just inside the door is not stepping out of it", plot.id);
        let at = game.doorstep();
        game.stand_at(at);
        game.tick();
        assert_eq!(game.step(), opening().steps.len(), "on plot {}, standing on its own doorstep finishes the step", plot.id);
    }
    assert!(opens_on.contains(&"The Commons".to_string()), "a family plot opens on the Commons: {opens_on:?}");
    assert!(opens_on.contains(&"First Street".to_string()), "a street plot opens on First Street: {opens_on:?}");

    // A guest: the home put away, no door aboard, the step stays.
    let away = ship_on("p1").put_home_away().expect("a home can be put away");
    assert!(front_door_of(&away).is_none(), "a guest has no front door aboard");
    let mut data = DataStore::new();
    publish_front_door(&mut data, Some(&ship_on("p1")));
    assert!(data.contains(FRONT_DOOR_KEY));
    publish_front_door(&mut data, Some(&away));
    assert!(!data.contains(FRONT_DOOR_KEY), "the door is taken away when the home is put away");
}

/// TRAVEL COUNTS ONLY ON ITS OWN STEP (2026-10-04). A curious player looks
/// out of the front door before they have eaten; the opening must still end
/// at the door, not the moment the chest is built in the workshop.
///
/// Red, run with Travel counted from the quest's acceptance (an event for
/// each place the player stood in, as before): "passing the door early does
/// not end the opening when the chest is built" (left: 8, right: 7).
#[test]
fn passing_the_door_early_does_not_end_the_opening() {
    let mut game = Game::new("p1");
    let inside = game.ship.home_spawn_world().map(|p| (p.x, p.z)).unwrap();
    let steps = opening().steps.len();
    // Out of the door and back, on step 1.
    let at = game.doorstep();
    game.stand_at(at);
    game.tick();
    game.stand_at(inside);
    game.tick();
    // On to the build step, and the chest built, standing inside.
    let build = opening().steps.iter().position(|s| matches!(s.objective, QuestObjective::Build { .. })).expect("the opening builds");
    game.world.get::<&mut QuestTracker>(game.player).unwrap().active_quests[0].current_step = build;
    push_quest_event(&game.data, format!("build_{}", "storage_chest"));
    game.tick();
    assert_eq!(game.step(), steps - 1, "the chest is built: on to the door");
    // The door step is checked from the next frame on; still standing inside.
    game.tick();
    game.tick();
    assert_eq!(game.step(), steps - 1, "passing the door early does not end the opening when the chest is built");
    let at = game.doorstep();
    game.stand_at(at);
    game.tick();
    assert_eq!(game.step(), steps, "stepping out now does");
}

// ── The whole opening ──────────────────────────────────────────────────

/// THE OPENING, WALKED (2026-10-04): a new player with the starting kit, the
/// home's Barn and the shipped data does what each step's line says, and the
/// systems that really do it (food, farming, crafting with the trading post's
/// coal, the drone, construction, the quests) take the opening from waking up
/// to the front door: every step done in order, each with its notice, the last
/// with the quest's and the next quest's. On the way the player looks out of
/// the door early, which must not end it. Walked three times: the family home
/// on the default plot (offline play) in the Normal play mode, nothing free;
/// the one-person home on a First Street plot (what a server hands the second
/// player) in Normal; and the family home in the Dev play mode, where crafting
/// and planting take nothing.
///
/// Red, run on the shipped two-step First Steps of before: "the opening's
/// steps are the eight the operator accepted, in his order" (left: ["gather",
/// "smelt"], right: the eight).
#[test]
fn the_opening_walks_from_waking_up_to_the_front_door() {
    let kinds: Vec<&str> = opening()
        .steps
        .iter()
        .map(|s| match &s.objective {
            QuestObjective::View { .. } => "view",
            QuestObjective::Eat { .. } => "eat",
            QuestObjective::Plant { .. } => "plant",
            QuestObjective::Make { item_id, .. } if item_id == "iron_ingot_0" => "smelt",
            QuestObjective::Make { .. } => "make",
            QuestObjective::Gather { .. } => "gather",
            QuestObjective::Build { .. } => "build",
            QuestObjective::Travel { destination } if destination == "front_door" => "front door",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        ["view", "eat", "plant", "make", "gather", "smelt", "build", "front door"],
        "the opening's steps are the eight the operator accepted, in his order"
    );
    walk_the_opening("home.ron", "p1", false);
    walk_the_opening("home_solo.ron", "p2", false);
    walk_the_opening("home.ron", "p1", true);
}

/// One walk through the opening (`the_opening_walks_from_waking_up_to_the_front_door`).
fn walk_the_opening(home_file: &str, plot: &str, dev: bool) {
    let mode = if dev { "Dev" } else { "Normal" };
    let walk = format!("{home_file} on {plot} in {mode}");
    let n = opening().steps.len();
    let mut game = Game::of(home_file, plot, dev);
    game.tick();
    assert_eq!(game.step(), 0, "{walk}: nothing is done by waking up");
    assert!(game.notices().is_empty(), "{walk}: no notice for waking up");
    let expect_step_notice = |game: &Game, said: &[String], done: usize| {
        let want = step_notice("First Steps", done, n, &line(game, done));
        assert!(said.contains(&want), "{walk}: step {done} posts its notice {want:?}, got {said:?}");
    };

    // 1. Check your vitals: press I, see Inventory > Status. The page marks
    //    the view and the frame glue reports it (both tested above).
    push_quest_event(&game.data, view_event_key("vitals"));
    let said = game.until_past(0, 2, &format!("{walk}: the vitals"));
    expect_step_notice(&game, &said, 1);

    // A curious player looks out of the front door before eating.
    let inside = game.ship.home_spawn_world().map(|p| (p.x, p.z)).unwrap();
    let at = game.doorstep();
    game.stand_at(at);
    game.tick();
    game.stand_at(inside);
    assert!(game.notices().is_empty(), "{walk}: looking out of the door early finishes nothing");

    // 2. Eat something: press I, click a Basic Ration, then Eat.
    game.request("consume_request", "ration_basic_0");
    let said = game.until_past(1, 2, &format!("{walk}: eating a ration"));
    expect_step_notice(&game, &said, 2);

    // 3. Plant: press I, Garden > Variety Greens and Beans > Plant. The
    //    button sends the tower design and one plant per cup (inventory.rs).
    let towers = crate::gui::load_tower_configs(&data_dir());
    let greens = towers.iter().find(|t| line(&game, 2).contains(&t.name)).expect("the step names a tower design");
    let cups: Vec<String> = greens.plantings.iter().flat_map(|p| std::iter::repeat(p.plant.clone()).take(p.slots.max(1) as usize)).collect();
    *game.data.get::<Mutex<Option<(String, Vec<String>)>>>("plant_tower_request").unwrap().lock().unwrap() = Some((greens.id.clone(), cups));
    let said = game.until_past(2, 2, &format!("{walk}: planting the tower"));
    expect_step_notice(&game, &said, 3);

    // 4. Craft a tool: Esc > Crafting > Carve Fishing Rod > Craft. A plank
    //    and a wire from the Barn, at the workbench, 30 seconds.
    game.request("craft_request", "craft_fishing_rod");
    let said = game.until_past(3, 40, &format!("{walk}: carving the fishing rod"));
    expect_step_notice(&game, &said, 4);
    assert_eq!(game.world.get::<&Inventory>(game.player).unwrap().count_item("fishing_rod_0"), 1, "{walk}: the rod is in the backpack");

    // 5. Get 3 iron ore: Inventory > Mining. The drone's 15-second trip
    //    lands the ore in home storage.
    *game.data.get::<Mutex<Option<(String, Vec<(String, u32)>)>>>("commission_drone").unwrap().lock().unwrap() =
        Some(("m12".into(), vec![("iron_ore_0".into(), 3)]));
    let said = game.until_past(4, 30, &format!("{walk}: the drone's iron ore"));
    expect_step_notice(&game, &said, 5);

    // 6. Smelt iron: Crafting > Smelt Iron (coal: the trading post). The
    //    trading post sells the coal (the frame bridge's vendor_buy), the
    //    hand craft takes the ore from home storage and the coal from the pack.
    {
        let goods = crate::systems::economy::TradeGoodsRegistry::from_ron(&read("trade_goods.ron")).expect("trade_goods.ron");
        let items = game.data.get::<ItemRegistry>("item_registry");
        let mut q = game.world.query_one::<(&mut Inventory, &mut Wallet)>(game.player).unwrap();
        let (inv, wallet) = q.get().unwrap();
        crate::systems::economy::vendor_buy(inv, &mut wallet.credits, &goods, items, "coal_0", 1).expect("the trading post sells coal");
    }
    game.request("craft_request", "smelt_iron");
    let said = game.until_past(5, 20, &format!("{walk}: smelting the iron"));
    expect_step_notice(&game, &said, 6);

    // 7. Build a chest: Crafting > Structures > Build, aim, then E. Eight
    //    planks from the Barn, placed in the workshop of this plot's home.
    {
        let origin = game.ship.home_plot().expect("the home stands on a plot").origin;
        let reg = game.data.get::<BlueprintRegistry>("blueprint_registry").unwrap();
        let chest = reg.get("storage_chest").expect("the storage chest ships").clone();
        let ghost = placement::placement_pose(&chest, Vec3::new(origin.0 + 27.0, 0.0, origin.2 + 40.0), 0, &game.world, reg, None);
        game.data.get::<Mutex<Vec<BuildRequest>>>("build_request").unwrap().lock().unwrap().push(BuildRequest::new("storage_chest", ghost));
    }
    let said = game.until_past(6, 10, &format!("{walk}: building the chest"));
    expect_step_notice(&game, &said, 7);
    let opens_on = front_door_of(&game.ship).expect("the home has a front door").opens_on_in_a_sentence();
    assert!(line(&game, 7).ends_with(&format!("look out on {opens_on}")), "{walk}: the last line names what this plot's door opens on: {}", line(&game, 7));

    // Still inside: not done.
    game.tick();
    assert_eq!(game.step(), n - 1, "{walk}: the opening ends at the door, not in the workshop");

    // 8. Step out your front door and look out on the Commons (First Street).
    let at = game.doorstep();
    game.stand_at(at);
    game.tick();
    assert_eq!(game.step(), n, "{walk}: out of the front door, the opening is done");
    assert_eq!(
        game.notices(),
        vec![
            "Quest complete: First Steps. You received 2 Iron Ingot and 30 Metalworking XP.".to_string(),
            "New quest: Toolsmith. Forge a hammer: Esc > Crafting > Craft Hammer > Craft.".to_string(),
        ],
        "{walk}: the end of the opening says what it gave and what comes next"
    );
    let t = game.world.get::<&QuestTracker>(game.player).unwrap();
    assert!(t.is_active("gs_toolsmith"), "{walk}: the chain runs on into Toolsmith");
}

// ── The HUD's line, and the names in it ────────────────────────────────

/// EVERY STEP OF EVERY SHIPPED QUEST FITS THE HUD'S QUEST LINE (2026-10-04):
/// the line is the step as the player reads it with its "(2/8)", cut at 64
/// characters (src/gui/pages/hud.rs `quest_line`, `QUEST_LINE_CHARS`). A step
/// naming what the front door opens on is read on every plot of the ship, and
/// with no door at all ("the ship").
///
/// Red, run on a draft of the last step, "Step out your front door, by the
/// Entry, to see {front_door_opens_on}": 64 on the Commons, 65 on First
/// Street: "the HUD's quest line cuts these: First Steps, step 8, on plot p2 is
/// 65 characters: Step out your front door, by the Entry, to see First Street
/// (8/8)".
#[test]
fn every_step_of_every_shipped_quest_fits_the_hud_line() {
    use crate::gui::pages::hud::{quest_line, QUEST_LINE_CHARS};
    let reg = QuestRegistry::from_ron_dir(&data_dir().join("quests"));
    let file = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
    // Every way a front door can read: each plot's, and none.
    let mut doors: Vec<(String, DataStore)> = vec![("no plot".to_string(), DataStore::new())];
    for plot in &file.plots {
        let mut data = DataStore::new();
        publish_front_door(&mut data, Some(&ship_on(&plot.id)));
        doors.push((format!("plot {}", plot.id), data));
    }
    let mut checked = 0;
    let mut over = Vec::new();
    for q in reg.quests.values() {
        for (i, s) in q.steps.iter().enumerate() {
            for (where_, data) in &doors {
                let hud = quest_line(&step_text(&s.description, data), i, q.steps.len());
                checked += 1;
                if hud.chars().count() > QUEST_LINE_CHARS {
                    over.push(format!("{}, step {}, on {where_} is {} characters: {hud}", q.name, i + 1, hud.chars().count()));
                }
            }
        }
    }
    assert!(checked > 30, "every shipped step was read, {checked}");
    assert!(over.is_empty(), "the HUD's quest line cuts these:\n  {}", over.join("\n  "));
}

/// EVERY STEP OF THE OPENING NAMES WHAT THE GAME SHOWS (2026-10-04): the
/// keys, pages, sections, buttons, recipes and items its lines name are the
/// ones the game really has, so a rename cannot leave the opening pointing at
/// nothing. Each name is read from where the game takes it (the page sources,
/// recipes.csv, items.csv, the tower designs, the blueprints).
///
/// Red, run with the tool step reading "Carve a Fishing Rod": "the opening's
/// line \"Craft a tool: Esc > Crafting > Carve a Fishing Rod > Craft\" names
/// none of the recipes that make fishing_rod_0".
#[test]
fn every_step_of_the_opening_names_what_the_game_shows() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let inventory = src("src/gui/pages/inventory.rs");
    let crafting = src("src/gui/pages/crafting.rs");
    let nav = src("src/gui/pages/escape_menu.rs");
    let keys = String::from_utf8(read("keymaps.ron")).unwrap();
    let recipes = RecipeRegistry::from_csv(&read("recipes.csv")).unwrap();
    let items = ItemRegistry::from_csv(&read("items.csv")).unwrap();
    let blueprints = BlueprintRegistry::from_ron(&read("blueprints/basic.ron")).unwrap();
    let towers = crate::gui::load_tower_configs(&data_dir());
    let kit: Vec<String> = crate::save_load::starting_kit(&data_dir()).into_iter().map(|(id, _)| id).collect();
    let def = opening();
    let has = |text: &str, part: &str, why: &str| assert!(text.contains(part), "the opening's line {text:?} names {part:?}: {why}");
    // The keys it names are the World keys (data/keymaps.ron).
    assert!(keys.contains("(action: \"Inventory\", keys: \"I\")"), "I opens the Inventory");
    assert!(keys.contains("(action: \"Menu / back\", keys: \"Esc\")"), "Esc opens the menu");
    assert!(nav.contains("NavItem { label: \"Crafting\""), "the menu has a Crafting tab");
    for s in &def.steps {
        let t = &s.description;
        match &s.objective {
            QuestObjective::View { view } => {
                assert_eq!(view, "vitals");
                has(t, "press I", "the key");
                has(t, "Inventory > Status", "the section");
                assert!(inventory.contains("(\"inv_sec\", \"status\"), \"Status\""), "the Inventory page has a Status section");
            }
            QuestObjective::Eat { .. } => {
                has(t, "press I", "the key");
                has(t, "then Eat", "the button");
                assert!(inventory.contains("\"Eat\""), "food in the backpack has an Eat button");
                let food = kit
                    .iter()
                    .find(|id| crate::systems::food::consume_kinds().get(*id) == Some(&false))
                    .expect("the starting kit holds food");
                has(t, &items.items[food].name, "the food in the starting kit");
            }
            QuestObjective::Plant { .. } => {
                has(t, "press I", "the key");
                has(t, "Garden", "the section");
                assert!(inventory.contains("(\"inv_sec\", \"garden\"), \"Garden\""), "the Inventory page has a Garden section");
                assert!(inventory.contains("\"Plant this tower\""), "a tower design's button plants it");
                let tower = towers.iter().find(|tw| t.contains(&tw.name)).unwrap_or_else(|| panic!("the opening's line {t:?} names a tower design"));
                assert!(
                    tower.plantings.iter().any(|p| kit.contains(&format!("seed_{}_0", p.plant))),
                    "the {} tower grows something the starting kit has a seed for",
                    tower.name
                );
            }
            QuestObjective::Make { item_id, .. } => {
                has(t, "Crafting > ", "the page");
                let named: Vec<&str> = recipes.recipes_producing(item_id).iter().map(|r| r.name.as_str()).filter(|n| t.contains(n)).collect();
                assert!(!named.is_empty(), "the opening's line {t:?} names none of the recipes that make {item_id}");
            }
            QuestObjective::Gather { .. } => {
                has(t, "Inventory > Mining", "the drone");
                has(t, "trading post (E)", "the vendor, and the key that opens it");
                assert!(inventory.contains("(\"inv_sec\", \"mining\"), \"Mining\""), "the Inventory page has a Mining section");
                assert!(keys.contains("keys: \"E (look at a target)\""), "E opens what the player looks at");
            }
            QuestObjective::Build { blueprint_id } => {
                has(t, "Crafting > Structures", "the section");
                has(t, "then E", "the key that builds it");
                assert!(crafting.contains("RichText::new(\"Structures\")"), "the Crafting page has Structures");
                assert!(blueprints.blueprints.contains_key(blueprint_id), "the blueprint ships");
            }
            QuestObjective::Travel { destination } => {
                assert_eq!(destination, "front_door");
                has(t, "front door", "where");
                has(t, FRONT_DOOR_OPENS_ON, "what it opens on, for the plot the player was given");
            }
            other => panic!("the opening has a step the game cannot see done: {other:?}"),
        }
    }
}

// ── A returning player ─────────────────────────────────────────────────

/// A PLAYER WHO FINISHED THE OLD FIRST STEPS IS NOT SENT BACK (2026-10-04).
/// First Steps was two steps (iron ore, an ingot); it is now the eight-step
/// opening under the same id. A save from before, with First Steps finished
/// and Toolsmith under way, is applied the way a launch applies it (the
/// fresh player accepts First Steps, then the save replaces their quests),
/// and the quests run: First Steps stays finished and the HUD's line is
/// Toolsmith's.
///
/// Red, run with the save's quests not applied: "a player who finished the
/// old First Steps is not sent back to its first step".
#[test]
fn a_player_who_finished_the_old_first_steps_is_not_sent_back() {
    let data = opening_store();
    let ship = ship_on("p1");
    let mut world = hecs::World::new();
    let player = new_player(&mut world, &ship);
    world
        .insert(
            player,
            (
                crate::ecs::components::Name("Returning".into()),
                crate::ecs::components::Appearance::default(),
                crate::ecs::components::Outfit::default(),
            ),
        )
        .unwrap();
    let mut save = crate::persistence::WorldSave::new_offline("My Homestead", "fibonacci");
    let mut before = QuestTracker::default();
    before.completed_quests.push("gs_first_steps".into());
    before.accept_quest("gs_toolsmith");
    save.quests = Some(before);
    crate::save_load::apply_save_to_world(&mut world, &save);
    let mut quests = QuestSystem::new();
    for _ in 0..3 {
        quests.tick(&mut world, 1.0, &data);
    }
    let t = world.get::<&QuestTracker>(player).unwrap();
    assert!(!t.is_active("gs_first_steps"), "a player who finished the old First Steps is not sent back to its first step");
    assert!(t.is_completed("gs_first_steps"), "First Steps stays finished");
    assert_eq!(t.active_quests.first().map(|q| q.quest_id.as_str()), Some("gs_toolsmith"), "the HUD's quest line is Toolsmith's");
    assert!(
        QuestRegistry::from_ron_dir(&data_dir().join("quests")).get("gs_toolsmith").unwrap().prerequisite.as_deref() == Some("gs_first_steps"),
        "Toolsmith still follows First Steps, by the id the old saves hold"
    );
}
