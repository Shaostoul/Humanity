//! Sleeping in a bed (2026-09-27): the night passes and the body wakes rested.
//!
//! The body's sleep need is `Vitals::energy`: it drains over about sixteen
//! waking hours (`ENERGY_DECAY_PER_SEC` in `systems::food`) and leaves the
//! `fatigued` slowdown below 25. A night's sleep puts it back. How long a
//! night is: the American Academy of Sleep Medicine and Sleep Research
//! Society consensus (Watson et al. 2015, "Recommended Amount of Sleep for a
//! Healthy Adult", J Clin Sleep Med 11(6)) is that adults should sleep 7 or
//! more hours a night; the game sleeps 8.
//!
//! The night is LIVED, not skipped. The clock runs `SLEEP_TIME_SCALE` times
//! faster while you sleep, through the same request channel the time
//! scrubber uses, so every system that follows the clock goes through the
//! night by its own rules: crops grow and are held in the dark as on any
//! night, crafts and drones finish, the sun sets and rises. A clock JUMP
//! would do none of that (`save_load::catch_up_world` explains why the
//! clock is never jumped), which is why this is not one.
//!
//! Who asks: a built structure whose blueprint `provides: "rest"` (a bed),
//! through `construction::uses` and the E press in `engine::built_uses`.
//! The FoodSystem, which owns the vitals, runs `tick` every frame.

use crate::ecs::components::{Controllable, Dead, StatusEffects, Vitals};
use crate::hot_reload::data_store::DataStore;
use crate::systems::time::{GameTime, SECONDS_PER_DAY};
use std::sync::Mutex;

/// Game hours a night's sleep lasts.
pub const SLEEP_HOURS: f64 = 8.0;
/// How many times faster the clock runs while the player sleeps. Eight game
/// hours are 400 game seconds (a game day is `SECONDS_PER_DAY`, 1,200 s),
/// so the night goes by in about three and a half real seconds.
pub const SLEEP_TIME_SCALE: f32 = 120.0;
/// The DataStore slot a sleep request travels in: where the player lies
/// down ("Bed"), taken by the next tick.
pub const REQUEST_SLOT: &str = "sleep_request";
/// Game seconds past the wake time beyond which the clock must have been
/// set by something else (a save loaded mid-sleep), not run there.
const CLOCK_JUMP_SLACK_S: f64 = 60.0;

/// A sleep in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct Asleep {
    /// The game second the sleeper lay down at.
    pub started_at: f64,
    /// The game second the sleeper wakes at.
    pub wake_at: f64,
    /// The clock speed to put back on waking: whatever the player had.
    pub resume_scale: f32,
    /// What they sleep in, for the notices ("Bed").
    pub place: String,
}

/// Ask to sleep in `place`. The next `tick` takes it.
pub fn request(data: &DataStore, place: &str) {
    if let Some(slot) = data.get::<Mutex<Option<String>>>(REQUEST_SLOT) {
        if let Ok(mut s) = slot.lock() {
            *s = Some(place.to_string());
        }
    }
}

fn set_clock_scale(data: &DataStore, scale: f32) {
    if let Some(slot) = data.get::<Mutex<Option<f32>>>("time_set_scale_request") {
        if let Ok(mut s) = slot.lock() {
            *s = Some(scale);
        }
    }
}

fn notice(data: &DataStore, msg: String) {
    if let Some(slot) = data.get::<Mutex<Vec<String>>>("player_notices") {
        if let Ok(mut n) = slot.lock() {
            n.push(msg);
        }
    }
}

fn living_player(world: &hecs::World) -> bool {
    world
        .query::<(&Vitals, &Controllable, Option<&Dead>)>()
        .iter()
        .any(|(_e, (_v, _c, dead))| dead.is_none())
}

/// One frame of sleep. Starts a requested sleep (speeding the clock), or,
/// once the clock reaches the wake time, puts the clock back and wakes the
/// player with full energy, no fatigue and the `rested` buff for `rested_s`
/// seconds. A player who dies asleep wakes nobody: the clock is simply put
/// back. A request while already asleep is ignored.
pub fn tick(asleep: &mut Option<Asleep>, world: &mut hecs::World, data: &DataStore, rested_s: f32) {
    let requested = data
        .get::<Mutex<Option<String>>>(REQUEST_SLOT)
        .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
    let (now, scale, hour) = data
        .get::<Mutex<GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|g| (g.elapsed_seconds, g.time_scale, g.hour)))
        .unwrap_or((0.0, 1.0, 8.0));

    if let Some(place) = requested {
        if asleep.is_none() && living_player(world) {
            *asleep = Some(Asleep {
                started_at: now,
                wake_at: now + SLEEP_HOURS * SECONDS_PER_DAY / 24.0,
                resume_scale: scale,
                place: place.clone(),
            });
            set_clock_scale(data, SLEEP_TIME_SCALE);
            notice(data, format!("You lie down in the {place} and fall asleep."));
        }
        return;
    }

    let Some(a) = asleep.as_ref() else { return };
    // The clock left the night (2026-09-27, review of the sleep batch):
    // another save loaded mid-sleep (ESC > Play, or Characters) sets the
    // clock before the sleep began or past its end. That is no night slept:
    // put the clock back and wake nobody rested, rather than leave it at 120x
    // for the loaded character or refill someone who never lay down. A frame
    // at 120x moves at most 12 game seconds (dt is capped at 0.1 s), so the
    // slack below only ever catches a jump.
    if now < a.started_at || now > a.wake_at + CLOCK_JUMP_SLACK_S {
        set_clock_scale(data, a.resume_scale);
        *asleep = None;
        return;
    }
    let alive = living_player(world);
    if alive && now < a.wake_at {
        return;
    }
    set_clock_scale(data, a.resume_scale);
    if alive {
        for (_e, (vitals, effects, _c)) in world.query_mut::<(&mut Vitals, &mut StatusEffects, &Controllable)>() {
            vitals.energy = vitals.energy_max;
            effects.remove("fatigued");
            effects.apply("rested", rested_s);
            break;
        }
        let (h, m) = (hour.floor() as u32 % 24, ((hour.fract()) * 60.0).floor() as u32);
        notice(
            data,
            format!("You slept {SLEEP_HOURS:.0} hours in the {} and woke rested at {h:02}:{m:02}.", a.place),
        );
    }
    *asleep = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::Health;

    fn store(elapsed: f64) -> DataStore {
        let mut data = DataStore::new();
        let mut gt = GameTime::default();
        gt.set_elapsed(elapsed);
        data.insert("game_time", Mutex::new(gt));
        data.insert("time_set_scale_request", Mutex::new(None::<f32>));
        data.insert(REQUEST_SLOT, Mutex::new(None::<String>));
        data.insert("player_notices", Mutex::new(Vec::<String>::new()));
        data
    }

    fn tired_player(world: &mut hecs::World) -> hecs::Entity {
        let mut v = Vitals::default();
        v.energy = 10.0;
        let mut fx = StatusEffects::default();
        fx.apply("fatigued", 3.0);
        world.spawn((v, fx, Health::default(), Controllable))
    }

    fn set_clock(data: &DataStore, secs: f64) {
        data.get::<Mutex<GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(secs);
    }

    fn scale_asked(data: &DataStore) -> Option<f32> {
        data.get::<Mutex<Option<f32>>>("time_set_scale_request").unwrap().lock().unwrap().take()
    }

    /// Lying down speeds the clock, the night runs, and the player wakes at
    /// the end of it with full energy, no fatigue and the rested buff, and
    /// the clock back at the speed it had. Nothing happens early. Red check:
    /// with the wake test inverted the player wakes at midnight and the
    /// halfway assertion fails.
    #[test]
    fn a_night_in_bed_passes_and_the_player_wakes_rested() {
        let data = store(10_000.0);
        let mut world = hecs::World::new();
        let p = tired_player(&mut world);
        let mut asleep = None;

        request(&data, "Bed");
        tick(&mut asleep, &mut world, &data, 3600.0);
        let wake_at = asleep.as_ref().expect("asleep").wake_at;
        assert_eq!(wake_at, 10_000.0 + 400.0, "eight game hours of a 1,200 s day");
        assert_eq!(scale_asked(&data), Some(SLEEP_TIME_SCALE), "the clock runs fast through the night");

        // Halfway through the night: still asleep, still tired.
        set_clock(&data, 10_200.0);
        tick(&mut asleep, &mut world, &data, 3600.0);
        assert!(asleep.is_some());
        assert_eq!(world.get::<&Vitals>(p).unwrap().energy, 10.0);

        // Morning.
        set_clock(&data, 10_401.0);
        tick(&mut asleep, &mut world, &data, 3600.0);
        assert!(asleep.is_none(), "awake");
        assert_eq!(scale_asked(&data), Some(1.0), "the clock goes back to the speed it had");
        let v = world.get::<&Vitals>(p).unwrap();
        assert_eq!(v.energy, v.energy_max, "a night's sleep refills energy");
        let fx = world.get::<&StatusEffects>(p).unwrap();
        assert!(!fx.has("fatigued") && fx.has("rested"));
        let notices = data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone();
        assert!(notices.last().unwrap().contains("slept 8 hours in the Bed"), "{notices:?}");
    }

    /// A save loaded mid-sleep moves the clock outside the night: before the
    /// sleep began (an older save) or far past its end (another character).
    /// Either way the clock goes back to its speed and nobody wakes rested.
    /// Red check: without the jump test, the clock behind the start leaves
    /// the sleep running (still asleep, no scale request), and the clock far
    /// ahead refills the player who never slept.
    #[test]
    fn a_clock_that_jumps_out_of_the_night_ends_the_sleep_unrested() {
        for jump_to in [9_000.0, 50_000.0] {
            let data = store(10_000.0);
            let mut world = hecs::World::new();
            let p = tired_player(&mut world);
            let mut asleep = None;
            request(&data, "Bed");
            tick(&mut asleep, &mut world, &data, 3600.0);
            scale_asked(&data);
            set_clock(&data, jump_to);
            tick(&mut asleep, &mut world, &data, 3600.0);
            assert!(asleep.is_none(), "clock at {jump_to}: the sleep ended");
            assert_eq!(scale_asked(&data), Some(1.0), "clock at {jump_to}: speed put back");
            assert_eq!(world.get::<&Vitals>(p).unwrap().energy, 10.0, "clock at {jump_to}: nobody refilled");
        }
    }

    /// A player who dies in their sleep is not woken rested, and the clock
    /// is still put back rather than left running at 120x. Red check: with
    /// the refill not guarded by `alive`, the dead player is refilled.
    #[test]
    fn dying_asleep_puts_the_clock_back_and_restores_nothing() {
        let data = store(0.0);
        let mut world = hecs::World::new();
        let p = tired_player(&mut world);
        let mut asleep = None;
        request(&data, "Bed");
        tick(&mut asleep, &mut world, &data, 3600.0);
        scale_asked(&data);
        world.insert_one(p, Dead::default()).unwrap();
        tick(&mut asleep, &mut world, &data, 3600.0);
        assert!(asleep.is_none());
        assert_eq!(scale_asked(&data), Some(1.0));
        assert_eq!(world.get::<&Vitals>(p).unwrap().energy, 10.0);
    }
}
