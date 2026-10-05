//! The player's body in the save (first-hour audit S1, docs/design/first-hour-audit-2026-10-04.md):
//! health, the vitals, the status effects, a death, and the home's urine tank come back after
//! a save as they were left, instead of quitting healing and refilling everything.
//!
//! A child of `save_load` (`use super::*`), kept in its own file so save_load.rs does not grow
//! into a monolith.

use super::*;
use crate::ecs::components::{Dead, Health, StatusEffects, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::food::FoodSystem;
use std::sync::Mutex;

fn data_dir() -> &'static std::path::Path {
    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
}

/// The DataStore the FoodSystem reads, as lib.rs wires it, with the needs held still (the
/// Vitals drain slider at 0) so a test's long tick changes only what it is about.
fn food_store() -> DataStore {
    let mut data = DataStore::new();
    let reg = crate::systems::status_effects::StatusEffectRegistry::from_csv(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/status_effects.csv"
    )))
    .expect("status_effects.csv");
    data.insert("status_effect_registry", reg);
    data.insert("consume_request", Mutex::new(Option::<String>::None));
    data.insert("drink_request", Mutex::new(Option::<String>::None));
    data.insert("rest_request", Mutex::new(false));
    data.insert("compost_request", Mutex::new(false));
    data.insert("player_death", Mutex::new(Option::<String>::None));
    data.insert("vitals_drain_scale", Mutex::new(0.0_f32));
    data
}

/// A player as lib.rs spawns one, with the body given.
fn player(world: &mut hecs::World, health: Health, vitals: Vitals, effects: StatusEffects) -> hecs::Entity {
    world.spawn((
        Controllable,
        Inventory::new(16),
        PlayerSkills::new(),
        crate::ecs::components::Name("Astra".to_string()),
        crate::ecs::components::Appearance::default(),
        crate::ecs::components::Outfit::default(),
        health,
        vitals,
        effects,
    ))
}

/// A new character's body, as lib.rs spawns it.
fn new_player(world: &mut hecs::World) -> hecs::Entity {
    player(world, Health::default(), Vitals::default(), StatusEffects::default())
}

/// Through the file format and back, as a save on disk is.
fn round_trip(save: &WorldSave) -> WorldSave {
    serde_json::from_str(&serde_json::to_string(save).unwrap()).unwrap()
}

fn hurt_body() -> (Health, Vitals, StatusEffects) {
    let vitals = Vitals {
        satiation: 23.5,
        hydration: 41.25,
        energy: 12.0,
        oxygen: 88.0,
        body_temp_c: 36.25,
        waste: 66.0,
        ..Vitals::default()
    };
    let mut effects = StatusEffects::default();
    effects.apply("food_poisoning", 1200.0);
    effects.apply("well_fed", 300.0);
    (Health { current: 37.5, max: 100.0 }, vitals, effects)
}

/// QUITTING NO LONGER HEALS (first-hour audit S1, 2026-10-04): health, every vital the game
/// tracks (food, water, energy, blood oxygen, core temperature and the waste meter) and the
/// status effects still running, each with the time it has left, come back after a save as
/// they were left, through the file format, onto a new character's body.
///
/// Seen red 2026-10-04 on 1b66dd36d (nothing of the body was saved, so the load left the new
/// body as it was): "health as saved / left: 100.0 / right: 37.5".
#[test]
fn the_body_comes_back_as_it_was_left() {
    let (health, vitals, effects) = hurt_body();
    let mut world = hecs::World::new();
    player(&mut world, health, vitals, effects);
    let save = round_trip(&extract_world_save(&world));

    let mut fresh = hecs::World::new();
    let p = new_player(&mut fresh);
    apply_save_to_world(&mut fresh, &save);
    let h = fresh.get::<&Health>(p).unwrap().clone();
    assert_eq!(h.current, 37.5, "health as saved");
    assert_eq!(h.max, 100.0);
    let v = fresh.get::<&Vitals>(p).unwrap().clone();
    assert_eq!(
        (v.satiation, v.hydration, v.energy, v.oxygen, v.body_temp_c, v.waste),
        (23.5, 41.25, 12.0, 88.0, 36.25, 66.0),
        "food, water, energy, oxygen, core temperature and the waste meter as saved"
    );
    let fx = fresh.get::<&StatusEffects>(p).unwrap().clone();
    let left = |id: &str| fx.active.iter().find(|e| e.id == id).map(|e| e.remaining);
    assert_eq!(left("food_poisoning"), Some(1200.0), "the food poisoning, with the time it had left");
    assert_eq!(left("well_fed"), Some(300.0));
    assert_eq!(fx.active.len(), 2);
    assert!(fresh.get::<&Dead>(p).is_err(), "alive");
}

/// The home's urine tank keeps its level across a save (first-hour audit S1): a day and a
/// half of a living player fills it 1.5 person-days; in the next session, after the load,
/// Compost draws the whole person-day off as one stored urine. It used to live only in the
/// running FoodSystem, so every launch emptied it.
///
/// Seen red 2026-10-04 on 1b66dd36d: "the saved person-day is drawn off after the load /
/// left: 0 / right: 1".
#[test]
fn the_urine_tank_keeps_its_level_across_a_save() {
    let data = food_store();
    let mut sys = FoodSystem::new(data_dir());
    let mut world = hecs::World::new();
    new_player(&mut world);
    sys.tick(&mut world, 1.5 * 86_400.0, &data);
    let save = round_trip(&extract_world_save(&world));

    // The next session: a new FoodSystem, a new player, the save applied.
    let mut sys = FoodSystem::new(data_dir());
    let mut fresh = hecs::World::new();
    let p = new_player(&mut fresh);
    apply_save_to_world(&mut fresh, &save);
    *data.get::<Mutex<bool>>("compost_request").unwrap().lock().unwrap() = true;
    sys.tick(&mut fresh, 0.0, &data);
    let drawn = fresh.get::<&Inventory>(p).unwrap().count_item("urine_stored_0");
    assert_eq!(drawn, 1, "the saved person-day is drawn off after the load");
}

/// Dead when the game was left, dead when it comes back (first-hour audit S1): a player who
/// died of thirst and quit before pressing Respawn returns to the death screen with its
/// cause, not alive at full health; Respawn stays the way back. And a living save loaded over
/// a dead player (the character picker) brings them back alive, with the death screen gone.
///
/// Seen red 2026-10-04 on 1b66dd36d: "the death screen comes back with its cause / left: None
/// / right: Some(\"dehydration\")".
#[test]
fn a_player_saved_dead_comes_back_dead() {
    let data = food_store();
    let mut sys = FoodSystem::new(data_dir());
    let mut world = hecs::World::new();
    let thirsty = Vitals { hydration: 0.0, ..Vitals::default() };
    let p = player(&mut world, Health { current: 0.5, max: 100.0 }, thirsty, StatusEffects::default());
    sys.tick(&mut world, 3600.0, &data);
    assert!(world.get::<&Dead>(p).is_ok(), "the setup: an hour of no water killed them");
    let dead_save = round_trip(&extract_world_save(&world));

    let mut gui = crate::gui::GuiState::default();
    let mut fresh = hecs::World::new();
    let q = new_player(&mut fresh);
    apply_save_to_world(&mut fresh, &dead_save);
    after_resume(&mut gui, &dead_save, &Resumed::default());
    assert_eq!(gui.player_death_cause.as_deref(), Some("dehydration"), "the death screen comes back with its cause");
    assert!(fresh.get::<&Dead>(q).is_ok(), "dead, as they were left");
    assert!(fresh.get::<&Health>(q).unwrap().current <= 0.0);

    // A living save over the dead player: alive again, the death screen gone.
    let mut living = hecs::World::new();
    let (health, vitals, effects) = hurt_body();
    player(&mut living, health, vitals, effects);
    let alive_save = round_trip(&extract_world_save(&living));
    apply_save_to_world(&mut fresh, &alive_save);
    after_resume(&mut gui, &alive_save, &Resumed::default());
    assert!(fresh.get::<&Dead>(q).is_err(), "the living save is alive");
    assert_eq!(fresh.get::<&Health>(q).unwrap().current, 37.5);
    assert_eq!(gui.player_death_cause, None, "and the death screen is gone");
}

/// "START EVERY SESSION FROM THE DEFAULT HOME" STARTS EACH SESSION WITH A NEW BODY (first-hour
/// audit S1): the setting keeps the character's look and nothing else, so the body starts new
/// each launch, as the backpack and the home do, whatever the progress save holds; and the
/// progress save's own body is left as it was in the file, for when the setting is turned off.
/// No play mode changes this: the modes do not touch how the body works (Creative's needs
/// follow the Vitals drain slider), so the body is saved alike in every one.
///
/// A guard on the setting, written with the fix. Seen red 2026-10-04 with
/// `identity_only_save` copying the live body into the save: "the progress save's body is
/// left as it was / left: 60.0 / right: 37.5".
#[test]
fn the_default_home_setting_starts_each_session_with_a_new_body() {
    let (health, vitals, effects) = hurt_body();
    let mut before = hecs::World::new();
    player(&mut before, health, vitals, effects);
    let progress = round_trip(&extract_world_save(&before));

    // A session with the setting on: the save's character only, so a new body.
    let mut session = hecs::World::new();
    let p = new_player(&mut session);
    apply_identity(&mut session, &progress);
    assert_eq!(session.get::<&Health>(p).unwrap().current, 100.0, "a new body this session");
    assert_eq!(session.get::<&Vitals>(p).unwrap().satiation, Vitals::default().satiation);

    // Hurt in that session and saved character-only: the progress save keeps its own body.
    session.get::<&mut Health>(p).unwrap().current = 60.0;
    let kept = identity_only_save(Some(progress), &session);
    let body = kept.body.as_ref().expect("the progress save's body");
    assert_eq!(body.health.current, 37.5, "the progress save's body is left as it was");
    // With no save yet, the character-only save carries no body: the next session is new too.
    assert!(identity_only_save(None, &session).body.is_none());
}

/// A save from before the body was saved still loads (first-hour audit S1; nobody plays yet,
/// so there is no compatibility code, but an old save must not break the load): it carries
/// no body, so the character gets the body a new character starts with, whatever the body
/// was before (the picker loads saves over a live player), and an empty urine tank.
///
/// Seen red 2026-10-04 on 1b66dd36d (the load left the live body as it was): "an old save
/// gives the body a new character starts with / left: 37.5 / right: 100.0".
#[test]
fn a_save_from_before_the_body_loads_with_a_new_body() {
    let old_json = r#"{"name":"Old","timestamp":0,"game_time":0.0,
        "player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],
        "player_health":100.0,"inventory":[],"skills":{},"constructions":[],
        "weather_state":"clear"}"#;
    let old: WorldSave = serde_json::from_str(old_json).expect("an old save loads");
    let mut world = hecs::World::new();
    let (health, vitals, effects) = hurt_body();
    let p = player(&mut world, health, vitals, effects);
    world.insert_one(p, Dead::default()).unwrap();
    apply_save_to_world(&mut world, &old);
    let h = world.get::<&Health>(p).unwrap().current;
    assert_eq!(h, 100.0, "an old save gives the body a new character starts with");
    let v = world.get::<&Vitals>(p).unwrap().clone();
    let new = Vitals::default();
    assert_eq!((v.satiation, v.hydration, v.energy, v.waste), (new.satiation, new.hydration, new.energy, new.waste));
    assert!(world.get::<&StatusEffects>(p).unwrap().active.is_empty(), "no effects");
    assert!(world.get::<&Dead>(p).is_err(), "alive");
    assert_eq!(crate::systems::food::urine_tank_level(&world), 0.0, "an empty urine tank");
}
