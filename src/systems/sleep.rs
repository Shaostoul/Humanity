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
//! through `construction::uses` and the E press in `engine::built_uses`, and,
//! the same way, a home machine whose def `provides: Some("rest")` (the
//! bedroom's bed, 2026-10-04, `built_uses::use_machine`).
//! The FoodSystem, which owns the vitals, runs `tick` every frame.

use crate::ecs::components::{Controllable, Dead, StatusEffects, Vitals};
use crate::hot_reload::data_store::DataStore;
use crate::systems::time::{GameTime, SECONDS_PER_HOUR};
use std::sync::Mutex;

/// Game hours a night's sleep lasts.
pub const SLEEP_HOURS: f64 = 8.0;
/// How many times faster the clock runs while the player sleeps. Eight game
/// hours are 28,800 game seconds (an hour is always 3,600, the one clock of
/// 2026-09-27), so at 7,200 the night goes by in four real seconds, whatever
/// the time-speed setting (the fastest it offers is 1,000).
pub const SLEEP_TIME_SCALE: f32 = 7200.0;
/// The DataStore slot a sleep request travels in: where the player lies
/// down ("Bed"), taken by the next tick.
pub const REQUEST_SLOT: &str = "sleep_request";
/// Game seconds past the wake time beyond which the clock must have been
/// set by something else (a save loaded mid-sleep), not run there: twice
/// the most one frame can move it asleep (dt is capped at 0.1 s, so 720 s
/// at `SLEEP_TIME_SCALE`).
const CLOCK_JUMP_SLACK_S: f64 = 2.0 * 0.1 * SLEEP_TIME_SCALE as f64;

/// A sleep in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct Asleep {
    /// The game second the sleeper lay down at.
    pub started_at: f64,
    /// The game second the sleeper wakes at.
    pub wake_at: f64,
    /// The hold to put back on waking: whatever the player had (None = the
    /// time-speed setting, Some(0) = the F11 panel held the clock still).
    pub resume_hold: Option<f32>,
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

fn set_clock_hold(data: &DataStore, hold: Option<f32>) {
    crate::systems::time::request_speed_hold(data, hold);
}

// -- The short rest (the Inventory page's Rest button, 2026-09-27) --------------------
//
// Until the built bed existed the Rest button refilled energy completely,
// anywhere, for free, which the bed would have been pointless beside. It is
// now what a sit-down break really is: a ten-minute nap. Brooks and Lack 2006,
// "A brief afternoon nap following nocturnal sleep restriction: which nap
// duration is most recuperative?", Sleep 29(6):831-840, compared naps of 5,
// 10, 20 and 30 minutes: the 10-minute nap was the most effective, with
// "immediate improvements in all outcome measures (including sleep latency,
// subjective sleepiness, fatigue, vigor, and cognitive performance), with some
// of these benefits maintained for as long as 155 minutes." So the nap lifts
// the fatigue slowdown for 155 minutes, and it repays only the sleep it is:
// ten minutes at a night's rate. A nap does not pay back a night, so it does
// nothing for someone past exhausted, and a second nap inside the first one's
// window adds nothing.

/// The status effect a short rest gives (data/status_effects.csv).
pub const REFRESHED: &str = "refreshed";
/// How long the nap is, seconds (Brooks and Lack 2006: 10 minutes).
pub const NAP_S: f32 = 10.0 * 60.0;
/// How long its lift lasts, seconds (Brooks and Lack 2006: up to 155 minutes).
pub const NAP_BENEFIT_S: f32 = 155.0 * 60.0;
/// A GAME CHOICE: below this energy a person is exhausted and a nap does not
/// lift the fatigue slowdown; only a bed does.
pub const NAP_FLOOR_ENERGY: f32 = 10.0;

/// The Rest button. Returns the notice to show.
pub fn short_rest(vitals: &mut Vitals, effects: &mut StatusEffects, energy_decay_per_waking_s: f32) -> String {
    if effects.has(REFRESHED) {
        return "You rested a little while ago. A nap's lift lasts about two and a half hours; \
                only a night in a bed puts the sleep back."
            .to_string();
    }
    // A night of SLEEP_HOURS repays the waking day around it, so each second
    // asleep repays (24 - SLEEP_HOURS) / SLEEP_HOURS seconds awake.
    let waking_per_sleeping = ((24.0 - SLEEP_HOURS) / SLEEP_HOURS) as f32;
    let gain = NAP_S * waking_per_sleeping * energy_decay_per_waking_s;
    vitals.energy = (vitals.energy + gain).min(vitals.energy_max);
    effects.apply(REFRESHED, NAP_BENEFIT_S);
    if vitals.energy < NAP_FLOOR_ENERGY {
        "You doze for ten minutes, but you are too exhausted for a nap to help. You need a night in a bed.".to_string()
    } else {
        "You sit down and doze for ten minutes: refreshed for about two and a half hours. \
         Only a night in a bed restores your energy."
            .to_string()
    }
}

/// Is a short rest still holding off the fatigue slowdown? Only for a tired
/// person, not an exhausted one.
pub fn nap_holds_off_fatigue(energy: f32, effects: &StatusEffects) -> bool {
    effects.has(REFRESHED) && energy >= NAP_FLOOR_ENERGY
}

/// Show the player a one-line notice (the "player_notices" channel).
pub fn notice(data: &DataStore, msg: String) {
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
    let (now, hold, hour) = data
        .get::<Mutex<GameTime>>("game_time")
        // The hour the wake notice reads: the home's own time, the one the
        // HUD shows aboard (BUG-090).
        .and_then(|m| {
            m.lock().ok().map(|g| {
                let lon = crate::systems::time::home_longitude_deg(data);
                let local = crate::systems::time::local_hour(f64::from(g.hour), lon, g.hours_per_day) as f32;
                (g.elapsed_seconds, g.speed_hold, local)
            })
        })
        .unwrap_or((0.0, None, 8.0));

    if let Some(place) = requested {
        // In a shared world the host's clock wins (2026-09-29): it runs at
        // the server's speed for everyone (72x unless its admin set another,
        // 2026-10-04), so nobody can sleep the night away there.
        if crate::systems::time::host_clock_active(data) {
            notice(data, format!("In a shared world everyone keeps the host's time, so the night can't be slept away here. The {place} is still yours to rest in."));
            return;
        }
        if asleep.is_none() && living_player(world) {
            *asleep = Some(Asleep {
                started_at: now,
                wake_at: now + SLEEP_HOURS * SECONDS_PER_HOUR,
                resume_hold: hold,
                place: place.clone(),
            });
            set_clock_hold(data, Some(SLEEP_TIME_SCALE));
            notice(data, format!("You lie down in the {place} and fall asleep."));
        }
        return;
    }

    let Some(a) = asleep.as_ref() else { return };
    // The clock left the night (2026-09-27, review of the sleep batch):
    // another save loaded mid-sleep (ESC > Play, or Characters) sets the
    // clock before the sleep began or past its end. That is no night slept:
    // put the clock back and wake nobody rested, rather than leave it racing
    // for the loaded character or refill someone who never lay down. A frame
    // asleep moves at most 720 game seconds (dt is capped at 0.1 s), so the
    // slack (twice that) only ever catches a jump.
    if now < a.started_at || now > a.wake_at + CLOCK_JUMP_SLACK_S {
        set_clock_hold(data, a.resume_hold);
        *asleep = None;
        return;
    }
    let alive = living_player(world);
    if alive && now < a.wake_at {
        return;
    }
    set_clock_hold(data, a.resume_hold);
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
        crate::systems::time::insert_slots(&mut data);
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

    /// The hold the sleep asked the clock for, if any: Some(Some(x)) held
    /// at x, Some(None) let go back to the time-speed setting.
    fn scale_asked(data: &DataStore) -> Option<Option<f32>> {
        data.get::<Mutex<Option<Option<f32>>>>(crate::systems::time::HOLD_SLOT).unwrap().lock().unwrap().take()
    }

    /// NO SLEEPING THE NIGHT AWAY IN A SHARED WORLD (2026-09-29). The host's
    /// clock wins there and runs at the server's speed for everyone (72x
    /// unless its admin set another, 2026-10-04), so the bed is refused with
    /// a notice and the clock is not sped up. Red check,
    /// run: dropping the host_clock_active check in tick lets the player fall
    /// asleep and fails the first assertion.
    #[test]
    fn no_sleeping_the_night_away_in_a_shared_world() {
        let data = store(10_000.0);
        *data.get::<Mutex<bool>>(crate::systems::time::HOST_ACTIVE_SLOT).unwrap().lock().unwrap() = true;
        let mut world = hecs::World::new();
        tired_player(&mut world);
        let mut asleep = None;
        request(&data, "Bed");
        tick(&mut asleep, &mut world, &data, 3600.0);
        assert!(asleep.is_none(), "no sleep in a shared world");
        assert_eq!(scale_asked(&data), None, "the clock was not sped up");
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
        assert_eq!(wake_at, 10_000.0 + 28_800.0, "eight hours of 3,600 s");
        assert_eq!(scale_asked(&data), Some(Some(SLEEP_TIME_SCALE)), "the clock runs fast through the night");
        // A few real seconds, not a real night: the eight hours at the
        // sleep speed. Red check, run: the old 120x makes this 240 s.
        let real_s = 28_800.0 / f64::from(SLEEP_TIME_SCALE);
        assert!((2.0..=6.0).contains(&real_s), "a night takes {real_s} real seconds");

        // Halfway through the night: still asleep, still tired.
        set_clock(&data, 24_400.0);
        tick(&mut asleep, &mut world, &data, 3600.0);
        assert!(asleep.is_some());
        assert_eq!(world.get::<&Vitals>(p).unwrap().energy, 10.0);

        // Morning.
        set_clock(&data, 38_801.0);
        tick(&mut asleep, &mut world, &data, 3600.0);
        assert!(asleep.is_none(), "awake");
        assert_eq!(scale_asked(&data), Some(None), "the clock goes back to the speed it had");
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
            assert_eq!(scale_asked(&data), Some(None), "clock at {jump_to}: speed put back");
            assert_eq!(world.get::<&Vitals>(p).unwrap().energy, 10.0, "clock at {jump_to}: nobody refilled");
        }
    }

    /// The Rest button is a ten-minute nap, not a night (Brooks and Lack 2006):
    /// it repays ten minutes of sleep (1.6 energy points, where the old button
    /// gave all 90 back), lifts the fatigue slowdown for 155 minutes for a
    /// tired person but not an exhausted one, and a second nap inside that
    /// window adds nothing. Red check: make `short_rest` set energy to
    /// `energy_max`, as the old Rest button did, and `energy < 15` fails.
    #[test]
    fn a_short_rest_is_a_nap_not_a_night() {
        // The food system's waking energy drain: 75 points over 16 hours.
        let decay = 75.0 / 57_600.0;
        let mut v = Vitals::default();
        v.energy = 12.0;
        let mut fx = StatusEffects::default();
        let msg = short_rest(&mut v, &mut fx, decay);
        let gain = v.energy - 12.0;
        assert!((gain - 600.0 * 2.0 * decay).abs() < 1e-4, "ten minutes of sleep at a night's rate: {gain}");
        assert!(v.energy < 15.0, "a nap is not a night: {}", v.energy);
        assert!(msg.contains("ten minutes"), "{msg}");
        assert!(fx.has(REFRESHED));
        assert!(nap_holds_off_fatigue(v.energy, &fx), "a tired person is kept alert");
        assert!(!nap_holds_off_fatigue(5.0, &fx), "an exhausted one is not");
        // A second nap inside the window adds nothing.
        let before = v.energy;
        let again = short_rest(&mut v, &mut fx, decay);
        assert_eq!(v.energy, before);
        assert!(again.contains("a little while ago"), "{again}");
        // The lift wears off after 155 minutes.
        fx.tick(NAP_BENEFIT_S + 1.0);
        assert!(!fx.has(REFRESHED) && !nap_holds_off_fatigue(v.energy, &fx));
    }

    /// A player who dies in their sleep is not woken rested, and the clock
    /// is still put back rather than left racing. Red check: with
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
        assert_eq!(scale_asked(&data), Some(None));
        assert_eq!(world.get::<&Vitals>(p).unwrap().energy, 10.0);
    }
}
