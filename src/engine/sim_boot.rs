//! The world's simulation, built without the GPU (headless sim runner increment 1, 2026-11-15,
//! docs/design/headless-sim-runner.md section 5). `boot` builds the ECS world, the DataStore's
//! slots, every system in tick order, the live save, the fresh player, the asteroids and the
//! menu's live home power; `starter_vehicles` and `apply_home_save` finish it once the data
//! registries and the settings are in. The windowed game's startup (lib.rs `resumed`) calls all
//! three exactly where it built these inline before; a test or the headless runner calls them
//! with no window and no wgpu device (`world_sim_tests.rs`). The home itself is built at world
//! entry by `engine::world_sim::build`.
//!
//! Nothing here may name a GPU, window or egui type, nor `EngineState`, which holds the renderer
//! (tests/sim_gpu_free_lint.rs), and nothing here asks whether it runs headless: one setup, two
//! drivers.
use std::path::PathBuf;
use glam::{Quat, Vec3};
use humanity_core::ecs::GameWorld;
use humanity_core::ecs::systems::SystemRunner;
use humanity_core::hot_reload::data_store::DataStore;
use humanity_core::input::InputState;
use humanity_systems::crafting::CraftingSystem;
use humanity_systems::farming::FarmingSystem;
use humanity_systems::food::FoodSystem;
use humanity_systems::interaction::InteractionSystem;
use humanity_systems::inventory::InventorySystem;
use humanity_systems::inventory::containers::ContainerCompatibilitySystem;
use humanity_systems::mining::DroneSystem;
use humanity_systems::persistence::{SaveSlot, WorldSave};
use humanity_systems::player::PlayerControllerSystem;
use humanity_systems::quests::QuestSystem;
use humanity_systems::skills::{SkillSystem, SkillXPEvent};
use humanity_systems::time::TimeSystem;
use humanity_systems::weather::WeatherSystem;
use crate::engine::home_spawn::spawn_home_power_entities;

/// What the simulation is built from.
pub(crate) struct SimBootOptions {
    /// The game data folder (`data/`).
    pub data_dir: PathBuf,
    /// Where the saves are: `persistence::saves_dir()` for the game, a scratch folder for a test
    /// (never a player's saves).
    pub saves_dir: PathBuf,
    /// config.json as read (`AppConfig::load_if_exists`); None is a fresh install, the built-in
    /// defaults.
    pub config: Option<crate::config::AppConfig>,
}

/// The simulation `boot` built: nothing in it needs a window or a GPU.
pub(crate) struct SimBoot {
    pub game_world: GameWorld,
    /// Every system, registered in tick order.
    pub system_runner: SystemRunner,
    pub data_store: DataStore,
    /// The options' config, handed back for the settings (lib.rs applies it to the GUI state).
    pub config: Option<crate::config::AppConfig>,
    /// The live save's slot, and its save (None: your own home has none yet).
    pub slot: SaveSlot,
    pub home_save: Option<WorldSave>,
}

/// Build the world's simulation: the ECS world, the DataStore's slots, the systems, the live
/// save chosen (`save_slot::boot`), the fresh player, the asteroids and the menu's live home
/// power. Moved verbatim from lib.rs `resumed` (2026-11-15); config.json is now read just
/// before it (it was read after the slots and the systems, which never touch it).
pub(crate) fn boot(opts: SimBootOptions) -> SimBoot {
    let SimBootOptions { data_dir, saves_dir, config: boot_config } = opts;
    let mut game_world = GameWorld::new();
    let mut system_runner = SystemRunner::new();
    let mut data_store = DataStore::new();
    data_store.insert("input_state", InputState::default());
    data_store.insert("camera_position", Vec3::new(0.0, 2.0, 5.0));
    data_store.insert("camera_forward", Vec3::NEG_Z);
    data_store.insert("camera_yaw", 0.0_f32);
    // Flight tolerances, inserted ONCE: the rows never change at
    // runtime, and FoodSystem/FarmingSystem only get `&DataStore`, so
    // this is how the g-load consequences reach them. A missing or
    // malformed data/ship/flight.ron loads empty, and an empty
    // tolerance list means nothing is ever harmed.
    data_store.insert(
        "flight_data",
        humanity_systems::flight::FlightData::load(&data_dir),
    );
    // GameTime lives behind a Mutex in the DataStore so TimeSystem (which
    // only gets &DataStore in tick) can write the advanced time each frame
    // and farming/ecology/weather/hydrology + the HUD can read it. Same
    // interior-mutability pattern as interaction_prompt below.
    data_store.insert(
        "game_time",
        std::sync::Mutex::new(humanity_systems::time::GameTime::default()),
    );
    // Weather behind a Mutex (the TimeSystem/game_time pattern): WeatherSystem
    // writes it each tick; the survival env (exposed temp) + HUD read it.
    data_store.insert(
        "weather",
        std::sync::Mutex::new(humanity_systems::weather::Weather::default()),
    );
    // Live weather control (v0.1050, F11 panel): the GUI publishes the
    // operator's picks here and WeatherSystem consumes them, since the
    // system itself lives inside the SystemRunner and is not reachable
    // from the GUI. Same Mutex-slot pattern as time_set_hour_request.
    data_store.insert(
        "weather_control",
        std::sync::Mutex::new(humanity_systems::weather::WeatherControl::default()),
    );
    // Live home electrical readout (gen/use/balance watts): ElectricalSystem writes
    // it each tick, the HUD + Home page read it. Same Mutex pattern as game_time.
    data_store.insert(
        "power_status",
        std::sync::Mutex::new(humanity_systems::electrical::PowerStatus::default()),
    );
    // What each machine drew over its last game day (2026-10-22): ElectricalSystem records it.
    humanity_ship::machines::day_energy::register(&mut data_store);
    // Live home WATER readout (production/demand/stored L): PlumbingSystem writes it each tick.
    data_store.insert(
        "water_status",
        std::sync::Mutex::new(humanity_systems::plumbing::WaterStatus::default()),
    );
    // Live home AIR readout (O2/CO2/pressure): AtmosphereSystem writes it from the home space. (v0.617)
    data_store.insert(
        "air_status",
        std::sync::Mutex::new(humanity_systems::atmosphere::AirStatus::default()),
    );
    system_runner.register(TimeSystem::new());
    // WeatherSystem ticks after TimeSystem (reads the exported season) and
    // exports Weather; the exposed-environment temperature consumes it.
    system_runner.register(WeatherSystem::new());
    // SolarSystem scales solar PowerGenerators by the time of day, then
    // ElectricalSystem sums supply/demand and sheds load. These tick the LIVE
    // home power sim against the machine entities spawned in load_world (v0.437).
    system_runner.register(humanity_systems::solar::SolarSystem::new());
    system_runner.register(humanity_systems::electrical::ElectricalSystem::new(&data_dir));
    // PlumbingSystem ticks AFTER electrical so it sees this frame's shed state -- a pump/purifier
    // shed in a power deficit stops producing water THIS tick (the power -> water chain). (v0.608)
    system_runner.register(humanity_systems::plumbing::PlumbingSystem::new());
    // AtmosphereSystem (v0.617): ticks the home's sealed air space + publishes AirStatus. Earth-
    // like for now (Stage 1); occupancy + powered scrubbers + the power -> air -> Vitals
    // consequence are Stage 2.
    system_runner.register(humanity_systems::atmosphere::AtmosphereSystem::new());
    system_runner.register(PlayerControllerSystem);
    data_store.insert("interaction_prompt", std::sync::Mutex::new(String::new()));
    // GUI -> ECS command channels (interior-mutable; the main loop writes
    // these from GuiState before each tick, the owning System reads + acts in
    // its tick). craft_request = a recipe id the player clicked Craft on;
    // dev_stock_materials = dev/creative provisioning (stock every recipe input);
    // consume_request = an item id the player clicked Eat on (FoodSystem applies
    // its nutrition to the player's Vitals).
    data_store.insert(
        "craft_request",
        std::sync::Mutex::new(Option::<String>::None),
    );
    data_store.insert("dev_stock_materials", std::sync::Mutex::new(false));
    // Settings > Gameplay "Vitals drain" scale (v0.791): the food system
    // multiplies hunger/thirst/energy decay by this each tick; the frame
    // loop publishes the persisted slider value here.
    data_store.insert("vitals_drain_scale", std::sync::Mutex::new(1.0_f32));
    data_store.insert(
        "consume_request",
        std::sync::Mutex::new(Option::<String>::None),
    );
    data_store.insert(
        "drink_request",
        std::sync::Mutex::new(Option::<String>::None),
    );
    // Gardening command channels (inventory page -> FarmingSystem): plant a
    // seed by item id, water/harvest a crop by entity bits, dev-grow all.
    data_store.insert(
        "plant_request",
        std::sync::Mutex::new(Option::<String>::None),
    );
    // Plant a whole aeroponic tower at once (v0.386): (tower config id, plant ids).
    data_store.insert(
        "plant_tower_request",
        std::sync::Mutex::new(Option::<(String, Vec<String>)>::None),
    );
    // Showcase-fill a tower with one species at staggered growth stages
    // (v0.862): (tower config id, plant id). Demo tool for the plant
    // visuals; also triggerable via debug/showcase_request.json.
    data_store.insert(
        "showcase_tower_request",
        std::sync::Mutex::new(Option::<(String, String)>::None),
    );
    // Jump the game clock to an hour of day (v0.871): the TimeSystem's
    // accumulator is authoritative, so the capture hook + future in-app
    // dev controls set time through this channel.
    data_store.insert(
        "time_set_hour_request",
        std::sync::Mutex::new(Option::<f32>::None),
    );
    // The one game clock's settings and its speed hold (2026-09-27).
    humanity_systems::time::insert_slots(&mut data_store);
    // Craft batches in flight both ways, and the time away handed to
    // the automated machines (systems::crafting::register).
    humanity_systems::crafting::register(&mut data_store);
    humanity_systems::livestock::register(&mut data_store); // the herd's saved timers until world entry
    // Absolute clock restore from a save (2026-09-25, offline
    // progression): see systems::time::request_restore_elapsed.
    data_store.insert(
        "time_restore_elapsed_request",
        std::sync::Mutex::new(Option::<f64>::None),
    );
    // Plant a bed/tray/field grow area (v0.738 grain loop): (machine id,
    // plant id, unit count). One CropInstance per unit, tagged with the
    // machine id as its grow-area so the Garden GUI groups them.
    data_store.insert(
        "plant_bed_request",
        std::sync::Mutex::new(Option::<(String, String, u32)>::None),
    );
    data_store.insert("water_request", std::sync::Mutex::new(Option::<u64>::None));
    data_store.insert("harvest_request", std::sync::Mutex::new(Option::<u64>::None));
    // Bulk harvest (v0.739): the Garden group "Harvest N ready" button.
    data_store.insert("harvest_many_request", std::sync::Mutex::new(Vec::<u64>::new()));
    data_store.insert("dev_grow_crops", std::sync::Mutex::new(false));
    // Per-area irrigation targets the garden edit modal publishes (tower_id ->
    // water level 0..1). FarmingSystem tops matching crops up to it; mirrored
    // from GuiState.garden_irrigation each frame by the bridge below.
    data_store.insert(
        "garden_irrigation",
        std::sync::Mutex::new(std::collections::HashMap::<String, f32>::new()),
    );
    // Per-area nutrient strength (garden edit slider, tower_id -> 0..1) scaling
    // crop growth speed in FarmingSystem; mirrored from GuiState each frame.
    data_store.insert(
        "garden_nutrient",
        std::sync::Mutex::new(std::collections::HashMap::<String, f32>::new()),
    );
    // Garden pests (2026-09-26, systems::farming::pests): the data, the
    // Settings severity, and the Garden panel's control buttons.
    data_store.insert("garden_pests", humanity_systems::farming::pests::PestData::load());
    data_store.insert("garden_lighting", humanity_systems::farming::lighting::LightingData::load());
    data_store.insert("garden_pest_severity", std::sync::Mutex::new(humanity_systems::farming::pests::DEFAULT_PEST_SEVERITY));
    data_store.insert("pest_control_request", std::sync::Mutex::new(Option::<(String, String)>::None));
    humanity_systems::farming::soil_ph::register(&mut data_store); // soil pH: data, Settings switch, Lime/Sulfur
    humanity_systems::farming::pollination::register(&mut data_store); // pollination data, Settings mode, Hand-pollinate
    humanity_systems::farming::picking::register(&mut data_store); // harvest windows, Settings picking mode, Clear
    humanity_systems::farming::humidity::register(&mut data_store); // greenhouse air: its data and the home's room boxes
    humanity_systems::life_support::register(&mut data_store); // ship life support: the home's air and water loops
    humanity_systems::mining::register(&mut data_store); // the mining drone's running costs: its data, its state, the Mining panel's requests (first-hour audit S5)
    humanity_systems::farming::weeds::register(&mut data_store); // weeds: their data and the Hoe / Mulch request
    // Backpack <-> container transfers (organize-layer inventory): the GUI pushes
    // (item_id, qty, is_add) ops; InventorySystem applies them to the player's
    // backpack. Mirrored from GuiState.pending_inventory_transfers each frame.
    // What automated machines made, for the main loop to file into
    // home storage (2026-09-26, engine::stock_piles::receive_machine_outputs).
    data_store.insert("home_stock_outputs", std::sync::Mutex::new(Vec::<(String, u32)>::new()));
    data_store.insert(
        "inventory_transfer_ops",
        std::sync::Mutex::new(Vec::<humanity_systems::inventory::TransferOp>::new()),
    );
    // One-line notices from the sim systems for the player (2026-09-25), on two
    // channels (BUG-258): the world's own lines (a finished craft waiting for room)
    // and replies to a page's press (an empty cistern at Water).
    humanity_systems::notices::register(&mut data_store);
    // The garden's real water use (2026-09-25): FarmingSystem publishes
    // litres per minute, PlumbingSystem draws them through the
    // irrigation machine; hand watering queues litres the same way.
    data_store.insert("irrigation_demand_lpm", std::sync::Mutex::new(0.0_f32));
    data_store.insert("hand_water_draw_l", std::sync::Mutex::new(0.0_f32));
    // What a backpack could not take back out of a transfer (2026-09-25):
    // InventorySystem fills it, the main loop puts it back in storage.
    data_store.insert(
        "inventory_transfer_returns",
        std::sync::Mutex::new(Vec::<(String, u32)>::new()),
    );
    // Creative mode: the resource-consuming systems (farming seeds/fertilizer,
    // crafting materials) skip the inventory requirement + consumption when
    // this is true. Mirrored from GuiState.creative_mode each frame by the
    // bridge below; it starts as the default play mode gives it (Normal since
    // 2026-10-04: off), until the config's mode lands.
    data_store.insert(
        "creative_mode",
        std::sync::Mutex::new(crate::config::PlayMode::default().allows(crate::config::Capability::FreeResources)),
    );
    // Dev: stock the "one seed of each" starter set into the player inventory
    // (FarmingSystem drains it). Lets survival mode be tested in early dev.
    data_store.insert(
        "stock_seeds_request",
        std::sync::Mutex::new(Option::<Vec<String>>::None),
    );
    // Mining: commission the player's drone with a MANIFEST (ores + units to
    // fetch); DroneSystem launches one drone per player.
    data_store.insert(
        "commission_drone",
        std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
    );
    // Mining: STANDING order (economy automation, v0.663). While Some, the
    // drone's Deliver arm re-commissions the same trip automatically ("Keep
    // mining" toggle on the Mining panel). Cleared when the toggle goes off.
    data_store.insert(
        "auto_mine_order",
        std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
    );
    // Vehicles: deploy a vehicle KIT item from the backpack into the world
    // (economy Phase 2 Stage 1). One-shot; VehicleSystem take()s it.
    data_store.insert(
        "deploy_kit_request",
        std::sync::Mutex::new(Option::<String>::None),
    );
    // Vehicles: summon a parked vehicle (by entity bits) to drive itself
    // to the player (Stage 3 transit). One-shot; VehicleSystem take()s it.
    data_store.insert(
        "summon_vehicle",
        std::sync::Mutex::new(Option::<u64>::None),
    );
    // Live production status (v0.681): CraftingSystem writes one honest
    // line per auto machine each tick ("Assemble Rover — 42%", "waiting
    // for Steel Ingot x6", "pad full, line paused"); the GUI shows them.
    data_store.insert("auto_craft_status", std::sync::Mutex::new(Vec::<String>::new()));
    // Death & recovery (v0.745, loop-map rung 1): FoodSystem writes the
    // death cause here when the player's health reaches zero; the frame
    // bridge surfaces it as the death screen.
    data_store.insert("player_death", std::sync::Mutex::new(Option::<String>::None));
    // Blueprint building (v0.746, ladder rung 2): the Crafting page's
    // Structures section queues builds (engine/build_place.rs, E while placing);
    // ConstructionSystem drains them + reports one honest status line.
    data_store.insert("build_request", std::sync::Mutex::new(Vec::<humanity_systems::construction::BuildRequest>::new()));
    data_store.insert("build_status", std::sync::Mutex::new(String::new()));
    data_store.insert(humanity_systems::construction::shared::OUT_CHANNEL, humanity_systems::construction::shared::OutQueue::default()); // paid-for shared builds, sent by engine::shared_build
    // World rewind signal (v0.679 review fix): raised right after
    // apply_save_to_world rewinds the live world (launcher character
    // pick); CraftingSystem drops its in-flight batches so a rewound
    // inventory can't ALSO receive the batch's outputs (duplication).
    data_store.insert("abort_active_crafts", std::sync::Mutex::new(false));
    // Survival: rest to refill energy (FoodSystem drains it).
    data_store.insert("rest_request", std::sync::Mutex::new(false));
    data_store.insert(humanity_systems::sleep::REQUEST_SLOT, std::sync::Mutex::new(None::<String>));
    // Sanitation: compost accumulated waste -> fertilizer (FoodSystem);
    // fertilize a crop by entity bits (FarmingSystem).
    data_store.insert("compost_request", std::sync::Mutex::new(false));
    data_store.insert(
        "fertilize_crop_request",
        std::sync::Mutex::new(Option::<u64>::None),
    );
    // Skill XP grants: the action systems (crafting/farming/mining) push
    // SkillXPEvents onto this channel; SkillSystem (registered LAST) drains
    // + applies them the same frame → level-ups.
    data_store.insert(
        "xp_grants",
        std::sync::Mutex::new(Vec::<SkillXPEvent>::new()),
    );
    // Practice quality's channels (skills rung 2): the level of skill learning, the people near
    // who could teach, the guides read and what they did.
    humanity_systems::skills::quality::register(&mut data_store);
    // Dev: max all skills (testing affordance — keeps every recipe craftable
    // under the #8b skill-gate, like "Dev: stock materials" for inventory).
    data_store.insert("dev_max_skills", std::sync::Mutex::new(false));
    // Quest progress events: action systems push "craft_<recipe>" /
    // "harvest_<crop>" keys; QuestSystem (registered after them) drains them
    // each frame to advance count-based Craft/Harvest objectives.
    data_store.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
    // One-shot SFX channel for ECS systems (v0.985): construction /
    // crafting completions ding through here; see audio::sounds::
    // push_sfx_event + the drain in the audio frame-sync block.
    data_store.insert(
        "sfx_events",
        std::sync::Mutex::new(Vec::<(String, String)>::new()),
    );
    // Game events for the notices (notifications rung 3): systems::push_game_event.
    data_store.insert(humanity_systems::GAME_EVENTS_KEY, std::sync::Mutex::new(Vec::<(String, String)>::new()));
    system_runner.register(InteractionSystem::new());
    // The farm on its own clock (2026-10-31, docs/design/farm-clock.md rung 2): the one
    // FarmingSystem also sits in the DataStore, so what reads the garden outside the
    // ECS tick (the Garden page's data, the plant meshes, the save) brings it up to now
    // first (`farming::settle`); its interval channel starts at the default until the
    // Settings value reaches it below, each frame.
    humanity_systems::farming::clock::register(&mut data_store);
    let farm: humanity_systems::farming::Shared = std::sync::Arc::new(std::sync::Mutex::new(FarmingSystem::new()));
    data_store.insert(humanity_systems::farming::SHARED_KEY, farm.clone());
    system_runner.register(humanity_systems::farming::SharedFarm(farm));
    system_runner.register(InventorySystem::new());
    // Enforces typed-container / content-class compatibility (wrong
    // material damages then breaks the vessel). Reads the
    // "container_registry" loaded into the DataStore above.
    system_runner.register(ContainerCompatibilitySystem::new());
    system_runner.register(CraftingSystem::new());
    // ConstructionSystem: blueprint placement -> timed build -> Structure entity.
    // Reads "blueprint_registry" (loaded in load_data_registries from
    // data/blueprints/basic.ron). Registered 2026-07-01 (was fully coded but never
    // turned on, per the afternoon audit) -- nothing calls queue_build() yet, so
    // this ticks as a safe no-op until a GUI/economy-automation caller is wired in
    // a later cycle; that's the honest current state, not a regression.
    system_runner.register(humanity_systems::construction::ConstructionSystem::new());
    // ManufacturingSystem: ProductionFacility entities -> timed recipe -> output_count.
    // Loads data/manufacturing.ron itself. Registered 2026-07-01 (same "coded but
    // never turned on" story as ConstructionSystem) -- safe no-op until something
    // spawns a ProductionFacility entity.
    system_runner.register(humanity_systems::manufacturing::ManufacturingSystem::new(&data_dir));
    // FoodSystem: nutrition (eat -> Vitals + buffs), hunger/thirst decay,
    // and spoilage. Reads consume_request + status_effect_registry from the
    // DataStore. Loads food_system.ron from the data dir at construction.
    system_runner.register(FoodSystem::new(&data_dir));
    // DroneSystem: autonomous mining drones (commission → trip → mine a finite
    // asteroid → deliver ore home). Reads commission_drone + item_registry.
    system_runner.register(DroneSystem::new());
    // VehicleSystem (economy Phase 2 Stage 1, first registration): its deploy
    // arm drains deploy_kit_request (kit item -> real Vehicle entity in the
    // world). The enter/exit/mech arms it also carries are dormant until
    // Stage 3 wires an interaction that publishes their commands.
    system_runner.register(humanity_systems::vehicles::VehicleSystem::new());
    // LivestockSystem (v0.751, ladder rung 7): ages every Harvestable
    // toward ready and ambles the farm animals around their anchors.
    system_runner.register(humanity_systems::livestock::LivestockSystem::new());
    // AbilitySystem (v0.753, ladder rung 8): drains ability_request
    // casts (validate skill gate + energy cost + cooldown, apply the
    // self-scoped effect) and publishes live cooldowns for the GUI.
    data_store.insert(
        "ability_request",
        std::sync::Mutex::new(Vec::<(String, Option<u64>)>::new()),
    );
    data_store.insert("ability_status", std::sync::Mutex::new(String::new()));
    data_store.insert(
        "ability_cooldowns",
        std::sync::Mutex::new(std::collections::HashMap::<String, f32>::new()),
    );
    system_runner.register(humanity_systems::abilities::AbilitySystem::new());
    // CombatSystem (v0.760, combat arc): drains damage_events hits
    // (armor mitigation, death, loot rolls into loot_drops, corpse
    // decay). Offensive ability casts are its first damage source.
    data_store.insert(
        "damage_events",
        std::sync::Mutex::new(
            Vec::<(u64, humanity_systems::combat::damage::DamageEvent)>::new(),
        ),
    );
    data_store.insert(
        "loot_drops",
        std::sync::Mutex::new(Vec::<(String, u32)>::new()),
    );
    system_runner.register(humanity_systems::combat::CombatSystem::new());
    // AISystem (v0.761, combat arc): the behavior state machine for
    // AI creatures - predators hunt prey INCLUDING the player and
    // their bites ride the damage_events channel above.
    system_runner.register(humanity_systems::ai::AISystem::new());
    // EconomySystem (v0.747, ladder rung 3, FIRST registration): pays the
    // passive income (1 CR per game day) into every Wallet — economy.ron's
    // "nobody is ever stuck at zero". Vendor buy/sell runs in the frame
    // bridge (one-shot GUI ops), not here.
    system_runner.register(humanity_systems::economy::EconomySystem::new());
    // SkillSystem ticks LAST so it drains the xp_grants the action systems
    // pushed THIS frame (craft → recipe skill, harvest → farming, mine → mining)
    // and applies level-ups before the frame's ECS→GUI sync reads them.
    system_runner.register(SkillSystem::new());
    // QuestSystem ticks after the action + skill systems so it sees this
    // frame's quest_events; advances/completes quests + grants rewards.
    system_runner.register(QuestSystem::new());
    // Which save is live (ship homes increment 6, engine/save_slot.rs `boot`): the config's
    // pairing picks your own home or this server's save (made fresh when there is none), and
    // it becomes the live one before the player is spawned, so the player carries its
    // start's kit and opening and the household file loaded below is its own. Its progress
    // is APPLIED after the config loads (below), because whether your own home's progress
    // is used at all is a setting ("Start every session from the default home",
    // 2026-09-25). Nothing ticks in between. The mess hall's meter is registered first, so
    // the save's meals land in it (engine/mess_hall.rs).
    crate::save_load::register_ship_meals(&mut data_store);
    let boot = crate::engine::save_slot::boot(&saves_dir, &data_dir, boot_config.as_ref());
    let (boot_slot, home_save) = (boot.slot, boot.save);
    crate::save_load::spawn_fresh_player(&mut game_world.world, &data_dir, &humanity_systems::persistence::live_save::active_home().start);
    // A new world's asteroids for the mining loop (finite ore; DroneSystem deletes one when
    // mined out), from data/asteroids/belt.ron. A save's replace them.
    crate::save_load::fresh_asteroids(&mut game_world.world, &data_dir);

    // Live HOME POWER in MENU mode (v0.518): spawn the home's electrical-role
    // entities now (no meshes) so SolarSystem + ElectricalSystem publish a live
    // PowerStatus the Home page reads even before Enter World. load_world
    // despawns every HomeMachine then re-spawns these WITH meshes on entry, so
    // there is no double-spawn.
    spawn_home_power_entities(&mut game_world.world, &data_dir);
    SimBoot { game_world, system_runner, data_store, config: boot_config, slot: boot_slot, home_save }
}

/// STARTER VEHICLES (v0.694, operator: "update the player home to have
/// a prebuilt vehicle so I don't have to assemble the factory, mine
/// resources, etc." -- headlined by their 1975 Chevy Nova, the first
/// real-world vehicle recreated in HumanityOS). Data-driven per
/// infinite-of-X: any kits.ron entry with `starter: true` spawns
/// pre-built in a BRAND-NEW home only (the save had no vehicles and
/// none exist -- an owned fleet is never duplicated or resurrected).
/// Called by lib.rs `resumed` once the data registries are in (the kits are one of them).
pub(crate) fn starter_vehicles(game_world: &mut GameWorld, data_store: &DataStore) {
    let has_vehicles = game_world
        .world
        .query::<&humanity_core::ecs::components::Vehicle>()
        .iter()
        .next()
        .is_some();
    if !has_vehicles {
        if let Some(kits) = data_store
            .get::<humanity_systems::vehicles::VehicleKitRegistry>("vehicle_kit_registry")
        {
            let starters: Vec<_> = kits.starters().cloned().collect();
            for (i, def) in starters.iter().enumerate() {
                // Park along the homestead frontage, one lane apart.
                let pos = Vec3::new(14.0 + i as f32 * 6.0, 0.0, 10.0);
                game_world.world.spawn((
                    humanity_core::ecs::components::Vehicle {
                        item_id: def.vehicle_item.clone(),
                    },
                    humanity_core::ecs::components::Transform {
                        position: pos,
                        rotation: Quat::from_rotation_y(0.0),
                        scale: Vec3::ONE,
                    },
                    humanity_core::ecs::components::Velocity::default(),
                    humanity_core::ecs::components::VehicleSeat {
                        occupant_key: None,
                        seat_type: "pilot".to_string(),
                    },
                    humanity_core::ecs::components::Name(def.display_name.clone()),
                ));
                log::info!("Starter vehicle parked: {}", def.display_name);
            }
        }
    }
}

/// The save's progress, applied once the settings have landed (lib.rs `resumed`, after the
/// config is applied to the GUI state): the clock held, the save's world, the character's own
/// home, and the time-away catch-up. Moved verbatim from lib.rs (2026-11-15).
pub(crate) fn apply_home_save(
    game_world: &mut GameWorld,
    data_store: &mut DataStore,
    gui_state: &mut crate::gui::GuiState,
    home_save: Option<&WorldSave>,
    data_dir: &std::path::Path,
) {
    // Resume the world clock where the save left it, and age the garden
    // by the time away when offline progression is on (2026-09-25).
    // Here and not at the apply above because the toggle is a setting,
    // and settings land only now. Nothing has ticked in between.
    //
    // "Start every session from the default home" (off by default since
    // 2026-10-04; the operator's own config keeps it on) keeps only the
    // character of your own home, and so does a save that carries no
    // progress; a server's save always keeps its progress (increment 6).
    //
    // The world is held still from startup until the person enters it (BUG-253,
    // gui/world_entry.rs): its saves are stamped with the moment it is current to, which
    // the catch-up below sets to now.
    crate::save_load::hold_world();
    if let Some(save) = home_save {
        if !crate::save_load::applies_progress(save, gui_state.settings.fresh_world_each_launch) {
            crate::save_load::apply_identity(&mut game_world.world, save);
            log::info!(
                "Fresh home this session: kept the character '{}', started from the default home (Settings > Gameplay)",
                save.character_name
            );
        } else {
            crate::save_load::apply_save_to_world(&mut game_world.world, save);
            // The character's own home (engine/own_home.rs, 2026-10-04): outside the Dev
            // mode the save's machines take the household's place in the layout loaded
            // from the data files, before anything reads it (the Home page, the offline
            // catch-up below, the menu's live power, whose entities are respawned from
            // it with the levels just restored). Its design goes on the plot at world load.
            if crate::engine::own_home::adopt_saved_home(gui_state, save) {
                if let Some(home) = gui_state.home_machines.as_ref() {
                    crate::engine::home_spawn::respawn_home_power_entities(&mut game_world.world, home, data_dir);
                    gui_state.garden_areas = crate::gui::garden_areas_of(home, data_dir);
                }
                log::info!("Loaded the character's own home (built outside the Dev mode) from the save");
            }
            // Where its home stood: the world load carries its pieces to the plot the
            // home is built on (engine/home_plot.rs `carry_loaded_save_home`).
            if let Some(b) = save.home_plot_box {
                data_store.insert(crate::save_load::LOADED_HOME_BOX_KEY, crate::save_load::LoadedHomeBox(b));
            }
            log::info!(
                "Loaded offline home: {} item stacks, {} skills",
                save.inventory.len(),
                save.skills.len()
            );
            // The player's modes before the time-away catch-up below, which charges the drone
            // (and decides whether the ship powers it) by them (first-hour audit S5).
            humanity_systems::mining::publish_mode(data_store, humanity_systems::mining::Mode::from_level(gui_state.settings.level("mining_drone")).unwrap_or_default());
            // The garden's and the ship's levels too (2026-11-12): who powers the air machines, and
            // the garden levels the catch-up's growth plays at.
            crate::engine::garden_levels::GardenLevels::of(&gui_state.settings.gameplay).publish(data_store, &mut gui_state.garden_pests);
            let resumed = crate::save_load::resume_home(
                &mut game_world.world,
                data_store,
                save,
                gui_state.settings.offline_progression,
                gui_state.settings.time_speed,
                gui_state.home_machines.as_ref(),
            );
            // Home storage, the notice, the Keep mining switch.
            crate::save_load::after_resume(gui_state, save, &resumed);
        }
    }
}

