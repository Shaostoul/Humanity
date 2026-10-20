//! Wounds and healing (first-hour audit S8, docs/design/first-hour-audit-2026-10-04.md).
//!
//! Until 2026-10-12 a bite took health and left nothing, nothing healed by itself but Well
//! Fed's 1 health a second, and the medical catalogue and its MedicalSystem were never run.
//! These tests drive the food system, which owns the body, the way the game does: the combat
//! and fire systems hurt a body, the food system's tick turns that into wounds, and the Use
//! button treats them. They read the data files themselves, so they check that the game did
//! what the files say.
//!
//! A child of `food` (`use super::*`), so a test can set a wound's chances on the running
//! system. RED CHECKS: each test's "Seen red" names the deliberate break it was run against
//! on 2026-10-12, and what it said.

use super::*;
use crate::ecs::components::{Controllable, Dead, Fire, Health, StatusEffects, Transform, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::combat::damage::{DamageEvent, DamageType};
use crate::systems::illness::{Mode, MODE_KEY};
use crate::systems::inventory::Inventory;
use crate::systems::status_effects::StatusEffectRegistry;
use crate::systems::wounds::{injure, time_words, Injury, Wounds};
use std::sync::Mutex;

fn data_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(data_dir().join(rel)).unwrap_or_else(|e| panic!("data/{rel}: {e}"))
}

fn registry() -> StatusEffectRegistry {
    StatusEffectRegistry::from_csv(read("status_effects.csv").as_bytes()).expect("status_effects.csv")
}

/// data/medical/wounds.ron, read here on its own.
fn shipped() -> Wounds {
    Wounds::from_ron(&read("medical/wounds.ron")).expect("wounds.ron parses")
}

/// The DataStore as lib.rs wires the food and combat systems, with the needs held still (the
/// Vitals drain slider at 0) so a long run changes only what a test is about. `mode` is the
/// Settings > Gameplay > Illness and injury mode (absent, as in a headless test, is Realistic).
fn store(mode: Option<Mode>) -> DataStore {
    let mut data = DataStore::new();
    data.insert("status_effect_registry", registry());
    for slot in ["consume_request", "drink_request", "use_item_request"] {
        data.insert(slot, Mutex::new(Option::<String>::None));
    }
    data.insert("rest_request", Mutex::new(false));
    data.insert("compost_request", Mutex::new(false));
    data.insert("player_death", Mutex::new(Option::<String>::None));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert("vitals_drain_scale", Mutex::new(0.0_f32));
    data.insert("damage_events", Mutex::new(Vec::<(u64, DamageEvent)>::new()));
    data.insert("loot_drops", Mutex::new(Vec::<(String, u32)>::new()));
    if let Some(mode) = mode {
        data.insert(MODE_KEY, mode);
    }
    data
}

/// A fed and watered player standing at the origin.
fn player(world: &mut hecs::World) -> hecs::Entity {
    world.spawn((
        Controllable,
        Inventory::new(36),
        Vitals { satiation: 80.0, hydration: 80.0, ..Vitals::default() },
        StatusEffects::default(),
        Health::default(),
        Transform::default(),
    ))
}

fn effects(world: &hecs::World, who: hecs::Entity) -> StatusEffects {
    StatusEffects::clone(&world.get::<&StatusEffects>(who).unwrap())
}

fn ids(fx: &StatusEffects) -> Vec<String> {
    fx.active.iter().map(|e| e.id.clone()).collect()
}

fn health(world: &hecs::World, who: hecs::Entity) -> f64 {
    f64::from(world.get::<&Health>(who).unwrap().current)
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

/// A wolf bites `who` for `amount` (the AI system's bite, through the one damage channel).
fn wolf_bites(data: &DataStore, who: hecs::Entity, amount: f32) {
    data.get::<Mutex<Vec<(u64, DamageEvent)>>>("damage_events").unwrap().lock().unwrap().push((
        who.to_bits().into(),
        DamageEvent { damage_type: DamageType::Kinetic, amount, source_name: Some("Wolf".into()), source_is_player: false },
    ));
}

/// Use one `item_id` from the Inventory (given first), returning what the player was told.
fn use_item(sys: &mut FoodSystem, world: &mut hecs::World, data: &DataStore, who: hecs::Entity, item_id: &str) -> String {
    world.get::<&mut Inventory>(who).unwrap().add_item(item_id, 1, 99);
    *data.get::<Mutex<Option<String>>>("use_item_request").unwrap().lock().unwrap() = Some(item_id.to_string());
    sys.tick(world, 0.0, data);
    notices(data).join(" ")
}

/// Tick `steps` steps of `step_s` game seconds.
fn run(sys: &mut FoodSystem, world: &mut hecs::World, data: &DataStore, steps: u32, step_s: f32) {
    for _ in 0..steps {
        sys.tick(world, step_s, data);
    }
}

// ── A bite ──

/// A WOLF'S BITE LEAVES A WOUND, AND A BANDAGE TREATS IT. Through the combat system the bite
/// takes its health now and leaves Bleeding and a Bite Wound, and the player is told what
/// happened and what helps; a Bandage stops the bleeding, puts 5 health back, and cleans and
/// covers the bite wound.
///
/// Seen red with the combat system's `injure` call removed (a bite only took health, as it did
/// before): "the bite left Bleeding and a Bite Wound: []".
#[test]
fn a_bite_bleeds_and_a_bandage_stops_it() {
    let w = shipped();
    let mut sys = FoodSystem::new(data_dir());
    let mut combat = crate::systems::combat::CombatSystem::new();
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    wolf_bites(&data, p, 10.0);
    combat.tick(&mut world, 0.016, &data);
    sys.tick(&mut world, 0.0, &data);
    let fx = effects(&world, p);
    assert!(fx.has("bleeding") && fx.has("bite_wound"), "the bite left Bleeding and a Bite Wound: {:?}", ids(&fx));
    assert_eq!(health(&world, p), 90.0, "the bite itself took its 10");
    let said = notices(&data).join(" ");
    let bleeding = w.get("bleeding").expect("bleeding in wounds.ron");
    assert!(said.contains("You are bitten") && said.contains(&bleeding.helps), "told what happened and what helps: {said:?}");
    assert!(said.contains("10 minutes"), "told how long it bleeds: {said:?}");

    let said = use_item(&mut sys, &mut world, &data, p, "bandage_0");
    let fx = effects(&world, p);
    assert!(!fx.has("bleeding"), "the bandage stopped the bleeding");
    assert!(fx.has("bite_wound") && fx.has(&w.care.effect), "the bite wound is still there, cleaned and covered: {:?}", ids(&fx));
    assert!(said.contains(&w.care.given), "told the wounds are cared for: {said:?}");
    assert_eq!(health(&world, p), 95.0, "and the bandage put 5 back");
}

/// BLEEDING STOPS ON ITS OWN, AND TAKES WHAT ITS DATA SAYS. Left alone, Bleeding lasts its
/// course (Realistic: status_effects.csv's 600 s; Forgiving: its course share) and takes its
/// `health_per_h` for that long (Forgiving: its harm share), then stops, and the player is told.
///
/// Seen red with `Wounds::harm` leaving out the mode's harm share: "Forgiving: bleeding took
/// 3.00 health over 300 s; its data says 1.50 over 300 s".
#[test]
fn bleeding_stops_on_its_own_and_takes_what_its_data_says() {
    let w = shipped();
    let row = w.get("bleeding").expect("bleeding").clone();
    let course_r = f64::from(registry().duration("bleeding"));
    for mode in [Mode::Realistic, Mode::Forgiving] {
        let mut sys = FoodSystem::new(data_dir());
        let data = store(Some(mode));
        let mut world = hecs::World::new();
        let p = player(&mut world);
        injure(&mut world, p, Injury::Bite);
        sys.tick(&mut world, 0.0, &data);
        notices(&data);
        let before = health(&world, p);
        let mut secs = 0.0;
        while effects(&world, p).has("bleeding") && secs < 3600.0 {
            sys.tick(&mut world, 10.0, &data);
            secs += 10.0;
        }
        let course = course_r * w.course_share(mode);
        let want = f64::from(row.health_per_h) / 3600.0 * course * w.harm_share(mode);
        let took = before - health(&world, p);
        assert!(
            (took - want).abs() < 0.02 && (secs - course).abs() <= 10.0,
            "{mode:?}: bleeding took {took:.2} health over {secs:.0} s; its data says {want:.2} over {course:.0} s"
        );
        let said = notices(&data).join(" ");
        assert!(said.contains(&row.healed), "{mode:?}: told it stopped: {said:?}");
    }
}

/// A KILLING BITE LEAVES NO WOUND ON THE BODY YOU RESPAWN IN. The bite that kills waits on the
/// dead body; a respawn gives a whole body, not a bleeding one.
///
/// Seen red with the dead body's waiting injuries left in place: "the respawned body has
/// [\"bleeding\", \"bite_wound\"]".
#[test]
fn a_killing_bite_leaves_no_wound_on_the_respawned_body() {
    let mut sys = FoodSystem::new(data_dir());
    let mut combat = crate::systems::combat::CombatSystem::new();
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    world.get::<&mut Health>(p).unwrap().current = 5.0;
    wolf_bites(&data, p, 10.0);
    combat.tick(&mut world, 0.016, &data);
    assert!(world.get::<&Dead>(p).is_ok(), "the setup: the bite killed them");
    sys.tick(&mut world, 0.0, &data);
    // Respawn, as lib.rs does it: alive again, whole, effects cleared.
    world.remove_one::<Dead>(p).unwrap();
    world.get::<&mut Health>(p).unwrap().current = 100.0;
    world.get::<&mut StatusEffects>(p).unwrap().active.clear();
    sys.tick(&mut world, 0.0, &data);
    let fx = effects(&world, p);
    assert!(fx.active.is_empty(), "the respawned body has {:?}", ids(&fx));
}

// ── Healing by itself ──

/// Health a body at 50 heals in a game hour, in ten-minute steps, after `setup`.
fn healed_in_an_hour(mode: Mode, setup: impl Fn(&mut FoodSystem, &mut hecs::World, hecs::Entity)) -> f64 {
    let mut sys = FoodSystem::new(data_dir());
    let data = store(Some(mode));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    world.get::<&mut Health>(p).unwrap().current = 50.0;
    setup(&mut sys, &mut world, p);
    run(&mut sys, &mut world, &data, 6, 600.0);
    health(&world, p) - 50.0
}

/// A BODY HEALS BY ITSELF. Fed, watered and unhurt it heals at the data's rate (Realistic:
/// nearly dead to whole in `realistic_days` ordinary days, a night's sleep in each; Forgiving
/// in `forgiving_days`), `asleep` times as fast asleep, at `slowed_share` while Hungry, and not
/// at all while ill or bleeding.
///
/// Seen red with `healing_stopped` not counting an illness: "ill with Food Poisoning it healed
/// 0.1566 in an hour; it heals nothing while ill".
#[test]
fn a_body_heals_by_itself_faster_asleep_and_not_while_ill_or_bleeding() {
    let w = shipped();
    let h = &w.healing;
    // 100 health over the days, each day 16 waking hours and 8 asleep at the asleep rate.
    let per_hour = |days: f32| 100.0 / (f64::from(days) * (16.0 + 8.0 * f64::from(h.asleep)));
    let near = |got: f64, want: f64| (got - want).abs() <= want.abs() * 0.01 + 1e-9;

    let awake = healed_in_an_hour(Mode::Realistic, |_, _, _| {});
    assert!(near(awake, per_hour(h.realistic_days)), "awake, Realistic: {awake:.4} in an hour, want {:.4}", per_hour(h.realistic_days));
    assert!(awake > 0.1 && awake < 0.3, "Realistic heals slowly: {awake:.4} an hour");

    let forgiving = healed_in_an_hour(Mode::Forgiving, |_, _, _| {});
    assert!(near(forgiving, per_hour(h.forgiving_days)), "awake, Forgiving: {forgiving:.4} in an hour");
    assert!(forgiving > 5.0 * awake, "Forgiving heals much faster: {forgiving:.4} against {awake:.4}");

    let asleep = healed_in_an_hour(Mode::Realistic, |sys, _, _| {
        sys.asleep = Some(crate::systems::sleep::Asleep {
            started_at: 0.0,
            wake_at: f64::MAX,
            resume_hold: None,
            place: "Bed".into(),
        });
    });
    assert!(near(asleep, awake * f64::from(h.asleep)), "asleep: {asleep:.4}, {}x awake's {awake:.4}", h.asleep);

    let hungry = healed_in_an_hour(Mode::Realistic, |_, world, p| world.get::<&mut Vitals>(p).unwrap().satiation = 20.0);
    assert!(near(hungry, awake * f64::from(h.slowed_share)), "Hungry: {hungry:.4}");

    let ill = healed_in_an_hour(Mode::Realistic, |_, world, p| {
        world.get::<&mut StatusEffects>(p).unwrap().apply("food_poisoning", 172_800.0)
    });
    assert!(ill == 0.0, "ill with Food Poisoning it healed {ill:.4} in an hour; it heals nothing while ill");

    let bleeding = healed_in_an_hour(Mode::Realistic, |_, world, p| {
        world.get::<&mut StatusEffects>(p).unwrap().apply("bleeding", 600.0)
    });
    assert!(bleeding < 0.0, "bleeding, it lost health ({bleeding:.4}) and healed none");
}

/// WELL FED NO LONGER HEALS A BODY IN TWO MINUTES. Its 1 health a second (status_effects.csv
/// until 2026-10-12) healed half a body in under a minute after any filling meal; healing is now
/// the body's own, at its own pace.
///
/// Seen red with Well Fed's `healing_per_tick` put back to 1: "Well Fed healed 99.0 in two
/// minutes; a body heals 0.0052 by itself in that time" (from 1 health to full).
#[test]
fn well_fed_no_longer_heals_a_body_in_two_minutes() {
    let mut sys = FoodSystem::new(data_dir());
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    world.get::<&mut Health>(p).unwrap().current = 1.0;
    world.get::<&mut StatusEffects>(p).unwrap().apply("well_fed", 1800.0);
    run(&mut sys, &mut world, &data, 120, 1.0);
    let healed = health(&world, p) - 1.0;
    let own = sys.wounds.healing_per_awake_s(Mode::Realistic, 100.0) * 120.0;
    assert!(healed <= own * 1.01 + 1e-6, "Well Fed healed {healed:.1} in two minutes; a body heals {own:.4} by itself in that time");
}

// ── Fire ──

/// One tenth of a second beside `fire`: the fire system, then the food system, as lib.rs
/// orders them.
fn burn_for(fire: &mut crate::systems::fire::FireSystem, sys: &mut FoodSystem, world: &mut hecs::World, data: &DataStore, secs: f32) {
    for _ in 0..(secs * 10.0).round() as u32 {
        fire.tick(world, 0.1, data);
        sys.tick(world, 0.1, data);
    }
}

/// A FIRE BURNS DEEPER THE LONGER YOU STAND IN IT. Every second at full intensity takes the
/// data's health, and the exposure's burn deepens through the steps (red skin, blistered,
/// deep), each replacing the shallower one it grew from, with the player told at each. Out of
/// the fire for longer than `new_burn_after_s` and back in, it is a new burn.
///
/// Seen red with the shallower burn left in place when the exposure deepened: "at 3.5 s the
/// burn is blistered and nothing else: [\"burn_superficial\", \"burn_partial\"]".
#[test]
fn a_fire_burns_deeper_the_longer_you_stand_in_it() {
    let w = shipped();
    let mut fire = crate::systems::fire::FireSystem::new(data_dir());
    let mut sys = FoodSystem::new(data_dir());
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    world.spawn((Transform::default(), Fire { intensity: 1.0, fuel_remaining: 1.0e6 }));
    let burns = |world: &hecs::World| -> Vec<String> {
        ids(&effects(world, p)).into_iter().filter(|id| id.starts_with("burn_")).collect()
    };

    burn_for(&mut fire, &mut sys, &mut world, &data, 0.5);
    assert!(burns(&world).is_empty(), "half a second: no burn yet");
    burn_for(&mut fire, &mut sys, &mut world, &data, 1.0);
    assert_eq!(burns(&world), vec!["burn_superficial"], "at 1.5 s, red skin");
    burn_for(&mut fire, &mut sys, &mut world, &data, 2.0);
    assert_eq!(burns(&world), vec!["burn_partial"], "at 3.5 s the burn is blistered and nothing else: {:?}", burns(&world));
    burn_for(&mut fire, &mut sys, &mut world, &data, 7.0);
    assert_eq!(burns(&world), vec!["burn_full"], "at 10.5 s it is deep");
    let took = 100.0 - health(&world, p);
    let want = 10.5 * f64::from(w.burns.health_per_s);
    assert!((took - want).abs() < 0.01, "10.5 s in the fire took {took:.3} health; the data says {want:.3}");
    let said = notices(&data).join(" ");
    for step in &w.burns.steps {
        let onset = &w.get(&step.effect).expect("each burn is a wound").onset;
        assert!(said.contains(onset.as_str()), "told of the {}: {said:?}", step.effect);
    }

    // Out of the fire, then back in: a new burn beside the old one.
    world.get::<&mut Transform>(p).unwrap().position = glam::Vec3::new(100.0, 0.0, 0.0);
    burn_for(&mut fire, &mut sys, &mut world, &data, w.burns.new_burn_after_s + 1.0);
    world.get::<&mut Transform>(p).unwrap().position = glam::Vec3::ZERO;
    burn_for(&mut fire, &mut sys, &mut world, &data, 1.5);
    let mut now = burns(&world);
    now.sort();
    assert_eq!(now, vec!["burn_full", "burn_superficial"], "a second exposure is a new burn");
}

/// A FIRE TAKES ITS SHARE OF HEALTH EXACTLY FROM WHAT IS NOT A BODY. A twentieth-strength fire
/// at 240 frames a second for a minute takes 3 health from a creature, to within a tenth of a
/// percent (BUG-164's note on fire.rs: in f32 a frame's share rounded to whole steps).
///
/// Seen red with the old f32 subtraction: "a weak fire took 2.9663 health in 60 s at 240
/// frames a second; it should take 3.0000".
#[test]
fn a_weak_fire_takes_its_share_exactly_at_any_frame_rate() {
    let mut fire = crate::systems::fire::FireSystem::new(data_dir());
    let data = store(None);
    let mut world = hecs::World::new();
    let hen = world.spawn((Transform::default(), Health { current: 100.0, max: 100.0 }));
    world.spawn((Transform::default(), Fire { intensity: 0.05, fuel_remaining: 1.0e6 }));
    for _ in 0..240 * 60 {
        fire.tick(&mut world, 1.0 / 240.0, &data);
    }
    let took = 100.0 - health(&world, hen);
    assert!((took - 3.0).abs() < 0.003, "a weak fire took {took:.4} health in 60 s at 240 frames a second; it should take 3.0000");
    assert!(world.get::<&crate::systems::wounds::Injuries>(hen).is_err(), "a creature is not a body: no wound");
}

/// THE SAME SECONDS IN A FIRE DO THE SAME HARM AT 30 AND 240 FRAMES A SECOND (BUG-164's note on
/// fire.rs, 2026-10-20). Each case runs at both rates and must come out at the value worked by
/// hand, to a thousandth:
/// - a weak fire (0.05) with 3,000 s of fuel, a minute: 3 health off a creature, and exactly 3 s
///   of its fuel gone;
/// - a full fire under a sprinkler taking 0.1 of its intensity a second, out at 10 s: the dose is
///   the trapezoid, 5 health;
/// - a fire at 0.5 under a sprinkler taking 0.02 a second, with 4 s of fuel, which runs out at
///   exactly 10 s: 4 health, and the fire is gone;
/// - a player standing in a full fire for 3.5 s: 3.5 s of the data's burn damage, and the same
///   burn.
///
/// Seen red on the f32 per-frame fire (fire.rs before this change, 2026-10-20): "a weak fire,
/// a minute at 30 frames a second: burned 3.0762 s of its fuel; 3.0000 is its share". The
/// same run printed for every case: the weak fire's fuel went 3.08 s at 30 frames a second and
/// 3.52 s at 240 (the fuel's f32 step at 3,000 is 0.000244 and a frame's share at 240 is
/// 0.000208, so every frame took a whole step); the suppressed fire took 5.0167 health at 30
/// and 5.0018 at 240 (each frame burned at its starting intensity); the fire burning out its
/// fuel took 4.0033 and 4.0005 (it burned its whole last frame past its fuel). The player's
/// 3.5 s and burn were already exact at both rates (S8); with the fix every case is exact.
#[test]
fn the_same_seconds_in_a_fire_do_the_same_harm_at_30_and_240_frames_a_second() {
    use crate::ecs::components::FireSuppressor;
    let w = shipped();
    for fps in [30u32, 240] {
        let dt = 1.0 / fps as f32;
        // A fire beside a creature, with or without a sprinkler on it, for `secs`.
        let creature = |intensity: f32, fuel: f32, sprinkler: f32, secs: u32| {
            let mut fire = crate::systems::fire::FireSystem::new(data_dir());
            let data = store(None);
            let mut world = hecs::World::new();
            let hen = world.spawn((Transform::default(), Health { current: 100.0, max: 100.0 }));
            let f = world.spawn((Transform::default(), Fire { intensity, fuel_remaining: fuel }));
            if sprinkler > 0.0 {
                world.spawn((Transform::default(), FireSuppressor { range: 5.0, strength: sprinkler }));
            }
            for _ in 0..fps * secs {
                fire.tick(&mut world, dt, &data);
            }
            let fuel_left = world.get::<&Fire>(f).ok().map(|f| f64::from(f.fuel_remaining));
            (100.0 - health(&world, hen), fuel_left)
        };

        let (took, fuel_left) = creature(0.05, 3000.0, 0.0, 60);
        let burned = 3000.0 - fuel_left.expect("still burning");
        assert!((took - 3.0).abs() < 1e-3, "a weak fire, a minute at {fps} frames a second: took {took:.4} health; 3.0000 is its share");
        assert!((burned - 3.0).abs() < 1e-3, "a weak fire, a minute at {fps} frames a second: burned {burned:.4} s of its fuel; 3.0000 is its share");

        let (took, fuel_left) = creature(1.0, 1000.0, 0.1, 12);
        assert!((took - 5.0).abs() < 1e-3, "a suppressed fire, out at 10 s, at {fps} frames a second: took {took:.4} health; 5.0000 is its share");
        assert!(fuel_left.is_none(), "the sprinkler put it out at {fps} frames a second");

        let (took, fuel_left) = creature(0.5, 4.0, 0.02, 12);
        assert!((took - 4.0).abs() < 1e-3, "a fire burning out its 4 s of fuel at {fps} frames a second: took {took:.4} health; 4.0000 is its share");
        assert!(fuel_left.is_none(), "its fuel ran out at {fps} frames a second");

        // A player, through the food system that owns the body.
        let mut fire = crate::systems::fire::FireSystem::new(data_dir());
        let mut sys = FoodSystem::new(data_dir());
        let data = store(Some(Mode::Realistic));
        let mut world = hecs::World::new();
        let p = player(&mut world);
        world.spawn((Transform::default(), Fire { intensity: 1.0, fuel_remaining: 1.0e6 }));
        for _ in 0..(fps as f32 * 3.5).round() as u32 {
            fire.tick(&mut world, dt, &data);
            sys.tick(&mut world, dt, &data);
        }
        let took = 100.0 - health(&world, p);
        let want = 3.5 * f64::from(w.burns.health_per_s);
        assert!((took - want).abs() < 1e-3, "a player 3.5 s in a fire at {fps} frames a second: took {took:.4} health; {want:.4} is its share");
        let burns: Vec<String> = ids(&effects(&world, p)).into_iter().filter(|id| id.starts_with("burn_")).collect();
        assert_eq!(burns, vec!["burn_partial"], "the same burn at {fps} frames a second");
    }
}

// ── Going bad, and care ──

/// The shipped data with every wound that can go bad certain to when nobody cared for it and
/// never when someone did, so the rule can be seen without dice.
fn certain(sys: &mut FoodSystem) {
    for w in &mut sys.wounds.wounds {
        if let Some(b) = &mut w.goes_bad {
            b.uncared = 1.0;
            b.cared = 0.0;
        }
    }
}

/// AN UNCARED-FOR WOUND CAN GO BAD, A CLEANED AND COVERED ONE IS SAFER, AND ANTIBIOTICS END
/// THE INFECTION. With the chances made certain, a bite left alone has gone bad a day in (the
/// player is told, it takes health and stops healing), Antibiotics end it, and the same bite
/// bandaged in time has not.
///
/// Seen red with the care ignored when the dice are rolled: "the bandaged bite went bad too:
/// [\"bite_wound\", \"wound_care\", \"infected_wound\"]".
#[test]
fn an_uncared_wound_can_go_bad_and_antibiotics_end_it() {
    let w = shipped();
    let after_h = w.get("bite_wound").and_then(|b| b.goes_bad.as_ref()).expect("a bite wound can go bad").after_h as u32;
    // Left alone.
    let mut sys = FoodSystem::new(data_dir());
    certain(&mut sys);
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    injure(&mut world, p, Injury::Bite);
    run(&mut sys, &mut world, &data, after_h - 1, 3600.0);
    assert!(!effects(&world, p).has("infected_wound"), "not before its time");
    notices(&data);
    run(&mut sys, &mut world, &data, 2, 3600.0);
    assert!(effects(&world, p).has("infected_wound"), "the uncared bite went bad: {:?}", ids(&effects(&world, p)));
    let said = notices(&data).join(" ");
    assert!(said.contains("has gone bad"), "told it went bad: {said:?}");
    let before = health(&world, p);
    run(&mut sys, &mut world, &data, 10, 3600.0);
    assert!(health(&world, p) < before, "an infected wound takes health and stops healing");
    let said = use_item(&mut sys, &mut world, &data, p, "antibiotics_0");
    assert!(!effects(&world, p).has("infected_wound"), "antibiotics ended it: {said:?}");

    // Bandaged within the first hour.
    let mut sys = FoodSystem::new(data_dir());
    certain(&mut sys);
    let mut world = hecs::World::new();
    let q = player(&mut world);
    injure(&mut world, q, Injury::Bite);
    sys.tick(&mut world, 0.0, &data);
    use_item(&mut sys, &mut world, &data, q, "bandage_0");
    run(&mut sys, &mut world, &data, after_h + 2, 3600.0);
    assert!(!effects(&world, q).has("infected_wound"), "the bandaged bite went bad too: {:?}", ids(&effects(&world, q)));
}

/// A NEW WOUND IS NOT COVERED BY THE OLD DRESSING. Bandaged, then bitten again: the care comes
/// off, the player is told, and a bandage helps again.
///
/// Seen red with a new wound leaving the care on: "a second bite after the bandage: care still
/// on: [\"bite_wound\", \"wound_care\", \"bleeding\"]".
#[test]
fn a_new_wound_is_not_covered_by_the_old_dressing() {
    let w = shipped();
    let mut sys = FoodSystem::new(data_dir());
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    injure(&mut world, p, Injury::Bite);
    sys.tick(&mut world, 0.0, &data);
    use_item(&mut sys, &mut world, &data, p, "bandage_0");
    assert!(effects(&world, p).has(&w.care.effect), "the setup: cared for");
    injure(&mut world, p, Injury::Bite);
    sys.tick(&mut world, 0.0, &data);
    let fx = effects(&world, p);
    assert!(!fx.has(&w.care.effect), "a second bite after the bandage: care still on: {:?}", ids(&fx));
    assert!(notices(&data).join(" ").contains(&w.care.new_wound), "told the new wound is not covered");
    let said = use_item(&mut sys, &mut world, &data, p, "bandage_0");
    assert!(effects(&world, p).has(&w.care.effect) && said.contains(&w.care.given), "a bandage helps again: {said:?}");
}

/// CLEANING AND COVERING IS A USE. Antiseptic, which heals nothing and ends nothing, is used up
/// on a body with a wound that can go bad and nobody has cleaned, and kept by a body without one.
///
/// Seen red with `treatment::apply` ignoring `cares`: "antiseptic on a fresh bite: 2 left, the
/// wounds cared for: false".
#[test]
fn cleaning_and_covering_a_wound_is_a_use() {
    let w = shipped();
    let mut sys = FoodSystem::new(data_dir());
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = player(&mut world);
    world.get::<&mut Inventory>(p).unwrap().add_item("antiseptic_0", 1, 99);
    let said = use_item(&mut sys, &mut world, &data, p, "antiseptic_0");
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("antiseptic_0"), 2, "kept with no wound: {said:?}");
    injure(&mut world, p, Injury::Bite);
    sys.tick(&mut world, 0.0, &data);
    notices(&data);
    *data.get::<Mutex<Option<String>>>("use_item_request").unwrap().lock().unwrap() = Some("antiseptic_0".into());
    sys.tick(&mut world, 0.0, &data);
    let left = world.get::<&Inventory>(p).unwrap().count_item("antiseptic_0");
    let cared = effects(&world, p).has(&w.care.effect);
    assert!(left == 1 && cared, "antiseptic on a fresh bite: {left} left, the wounds cared for: {cared}");
}

// ── Saved with the game ──

/// WOUNDS COME BACK WITH A SAVE. They are status effects, so the body's save (`BodySave`)
/// carries each with the time it has left, the care with them; a load puts them back, and the
/// cared-for bite, its day about to pass, still does not go bad after the load.
///
/// Seen red with `restore_body` leaving out the saved effects: "after the load: bleeding None,
/// want Some(300.0)"; and, with the care ignored when the dice are rolled, "the saved care
/// still counts after the load".
#[test]
fn wounds_and_their_care_come_back_with_a_save() {
    use crate::save_load::{apply_save_to_world, extract_world_save};
    // A player as lib.rs spawns one, for the save.
    fn saved_player(world: &mut hecs::World) -> hecs::Entity {
        let p = player(world);
        world
            .insert(
                p,
                (
                    crate::systems::skills::PlayerSkills::new(),
                    crate::ecs::components::Name("Astra".into()),
                    crate::ecs::components::Appearance::default(),
                    crate::ecs::components::Outfit::default(),
                ),
            )
            .unwrap();
        p
    }
    // The bite 23 hours old, an hour before it can go bad.
    let bite_left = registry().duration("bite_wound") as f64 - 23.0 * 3600.0;
    let mut world = hecs::World::new();
    let p = saved_player(&mut world);
    {
        let mut fx = world.get::<&mut StatusEffects>(p).unwrap();
        fx.apply("bleeding", 300.0);
        fx.apply("bite_wound", bite_left);
        fx.apply("burn_partial", 1_000_000.0);
        fx.apply("wound_care", 1_000_000.0);
    }
    let save: crate::persistence::WorldSave =
        serde_json::from_str(&serde_json::to_string(&extract_world_save(&world)).unwrap()).unwrap();
    let mut fresh = hecs::World::new();
    let q = saved_player(&mut fresh);
    apply_save_to_world(&mut fresh, &save);
    let fx = effects(&fresh, q);
    for (id, want) in [("bleeding", 300.0), ("bite_wound", bite_left), ("burn_partial", 1_000_000.0), ("wound_care", 1_000_000.0)] {
        assert_eq!(fx.remaining(id), Some(want), "after the load: {id} {:?}, want Some({want:?})", fx.remaining(id));
    }
    // Its day passes in the next hour: cared for, it does not go bad.
    let mut sys = FoodSystem::new(data_dir());
    certain(&mut sys);
    let data = store(Some(Mode::Realistic));
    run(&mut sys, &mut fresh, &data, 3, 3600.0);
    assert!(!effects(&fresh, q).has("infected_wound"), "the saved care still counts after the load");
}

// ── The data ──

/// THE SHIPPED WOUND DATA IS WHOLE. Every wound is a `wound` row of status_effects.csv with a
/// course and no damage or healing columns of its own (what it does is here), every effect the
/// file names is a wound, a bite leaves at least one, the burn steps deepen in order, every
/// wound that can go bad does so less often cared for than not and becomes a wound Antibiotics
/// end, and Forgiving is milder by every share and heals faster.
///
/// Seen red with bleeding's row put back to its old type: "bleeding is a wound row: its type is
/// debuff".
#[test]
fn the_shipped_wound_data_is_whole() {
    let w = shipped();
    let reg = registry();
    assert!(!w.wounds.is_empty());
    for row in &w.wounds {
        let def = reg.get(&row.effect).unwrap_or_else(|| panic!("{} is not a status effect", row.effect));
        assert_eq!(def.kind, crate::systems::wounds::WOUND, "{} is a wound row: its type is {}", row.effect, def.kind);
        assert!(def.duration_s > 0.0, "{} has a course", row.effect);
        assert_eq!((def.damage_per_tick, def.healing_per_tick), (0.0, 0.0), "{}: what it does is in wounds.ron", row.effect);
        assert!(!row.onset.is_empty() && !row.helps.is_empty() && !row.healed.is_empty() && row.course.contains("{time}"));
        if row.health_per_h > 0.0 {
            assert!(!row.cause.is_empty(), "{} can kill, so it names a cause", row.effect);
        }
        if let Some(bad) = &row.goes_bad {
            assert!(bad.cared < bad.uncared, "{}: caring for it makes it less likely to go bad", row.effect);
            let becomes = w.get(&bad.becomes).unwrap_or_else(|| panic!("{} becomes {}, not a wound", row.effect, bad.becomes));
            assert!(becomes.stops_healing && reg.get(&becomes.effect).is_some_and(|d| d.has_tag("bacterial")), "Antibiotics end it");
        }
    }
    assert!(!w.bite.is_empty() && w.bite.iter().all(|id| w.get(id).is_some()), "a bite leaves wounds");
    assert!(w.burns.steps.windows(2).all(|s| s[0].from_s < s[1].from_s), "the burns deepen in order");
    assert!(w.burns.steps.iter().all(|s| w.get(&s.effect).is_some()), "every burn is a wound");
    assert!(reg.get(&w.care.effect).is_some(), "the care is a status effect");
    assert!(w.forgiving.course_share < 1.0 && w.forgiving.harm_share < 1.0 && w.forgiving.goes_bad_share < 1.0);
    assert!(w.healing.forgiving_days < w.healing.realistic_days && w.healing.asleep > 1.0);
}

/// Settings > Gameplay > Illness and injury's injury hint names the shipped numbers.
///
/// Seen red with the hint reading Forgiving's days in Realistic's place: "the hint names
/// \"about 21 days\": ... Realistic: from nearly dead to whole in about 3 days ...".
#[test]
fn the_injury_hint_reads_its_numbers_from_the_data() {
    let hint = crate::systems::wounds::mode_hint();
    for part in ["about 10 minutes", "about 21 days", "about 3 days", "1.8 times", "last 50%", "go bad 25%"] {
        assert!(hint.contains(part), "the hint names {part:?}: {hint}");
    }
}

/// A wound's time in the player's words: minutes below an hour, so ten minutes of bleeding is
/// not "about an hour".
///
/// Seen red with `time_words` always using the illness's hours-and-days words: "left: \"an
/// hour\" / right: \"10 minutes\"".
#[test]
fn times_read_in_plain_words() {
    assert_eq!(time_words(600.0), "10 minutes");
    assert_eq!(time_words(30.0), "a minute");
    assert_eq!(time_words(3600.0 * 24.0 * 14.0), "14 days");
}
