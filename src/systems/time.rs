//! Time system — day/night cycle, seasons, and sun position.
//!
//! Stores `GameTime` in DataStore under key "game_time".
//! Computes sun direction and color for the renderer.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// In-game season derived from day count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    /// Determine season from day count: four equal quarters of a year of
    /// `days_per_year` days (the setting), Spring first.
    pub fn from_day(day: u32, days_per_year: u32) -> Self {
        let year = days_per_year.max(4);
        match (day % year) * 4 / year {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }
}

/// Complete game time state — serializable for save/load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTime {
    /// Total elapsed game seconds since world creation.
    pub elapsed_seconds: f64,
    /// Number of full days completed.
    pub day_count: u32,
    /// Current hour of the day (0.0 .. 24.0).
    pub hour: f32,
    /// Current season.
    pub season: Season,
    /// The clock's speed this frame, game seconds per real second: the
    /// time-speed setting, or a hold over it (`speed_hold`).
    pub time_scale: f32,
    /// Hours in a day (the Settings "Hours in a day", default 24). An hour
    /// is always `SECONDS_PER_HOUR`; this only says how many make a day.
    pub hours_per_day: u32,
    /// Days in a year (the Settings "Days in a year", default 365).
    pub days_per_year: u32,
    /// A temporary speed over the setting: the F11 panel's "Hold the clock
    /// still" (0), a night's sleep, a probe rig's freeze. None = the setting.
    pub speed_hold: Option<f32>,
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            day_count: 0,
            hour: 8.0,
            season: Season::Spring,
            time_scale: DEFAULT_TIME_SPEED,
            hours_per_day: DEFAULT_HOURS_PER_DAY,
            days_per_year: DEFAULT_DAYS_PER_YEAR,
            speed_hold: None,
        }
    }
}

impl GameTime {
    /// Game seconds in one day of this calendar: its hours times an hour.
    pub fn seconds_per_day(&self) -> f64 {
        f64::from(clamp_hours_per_day(self.hours_per_day)) * SECONDS_PER_HOUR
    }

    /// How far through the day, 0 to 1 (0 = midnight, 0.5 = noon).
    pub fn day_fraction(&self) -> f64 {
        let day = self.seconds_per_day();
        self.elapsed_seconds.rem_euclid(day) / day
    }

    /// Where the sun is, as the hour it would be on a 24-hour dial: noon is
    /// 12 however many hours the day has. Everything that follows the SUN
    /// (solar power, the grow lights' timer, the drawn sky, the planet's
    /// spin, the day's warmth) reads this; the clock a person reads is `hour`.
    pub fn solar_hour(&self) -> f32 {
        self.hour * 24.0 / clamp_hours_per_day(self.hours_per_day) as f32
    }

    /// `solar_hour` where the sun crosses longitude `lon_deg` (east
    /// positive) rather than longitude 0: noon is the moment the sun stands
    /// over that meridian. The game clock is longitude 0's time by
    /// construction (`dev_travel::planet_spin_from_time`), so a place 122
    /// degrees west has its noon at about 20:09 on it. The home's own systems
    /// (its solar panels, the grow lights' timer, the crops' sunlight) read
    /// this at the home's longitude, `home_longitude_deg` (BUG-090).
    pub fn solar_hour_at(&self, lon_deg: f64) -> f32 {
        (f64::from(self.solar_hour()) + lon_deg / 15.0).rem_euclid(24.0) as f32
    }

    /// Recompute hour, day and season from `elapsed_seconds`. Every writer of
    /// the clock goes through this, so the derived fields can never disagree
    /// with the total.
    pub fn recompute_derived(&mut self) {
        let day = self.seconds_per_day();
        self.hour = (self.elapsed_seconds.rem_euclid(day) / SECONDS_PER_HOUR) as f32;
        self.day_count = (self.elapsed_seconds.max(0.0) / day) as u32;
        self.season = Season::from_day(self.day_count, clamp_days_per_year(self.days_per_year));
    }

    /// Put the clock at an absolute game time (clamped to >= 0).
    pub fn set_elapsed(&mut self, secs: f64) {
        self.elapsed_seconds = secs.max(0.0);
        self.recompute_derived();
    }
}

/// Put the world clock back where a save left it (offline progression,
/// 2026-09-25). Before this, the clock started at zero on every launch while
/// the restored crops kept their `planted_at` from the previous session's
/// clock, so a garden REWOUND on restart: a crop planted at game second 5,000
/// sat frozen until the new session's clock caught up to 5,000 again.
///
/// Two writes, both needed. The request channel reaches the TimeSystem's own
/// accumulator, which is authoritative and re-exports over the DataStore copy
/// every tick (a direct write alone would be erased). The direct write makes
/// anything that reads the clock BEFORE the first tick (the HUD, an early
/// save) see the restored value rather than zero.
pub fn request_restore_elapsed(data: &DataStore, secs: f64) {
    if let Some(req) = data.get::<std::sync::Mutex<Option<f64>>>("time_restore_elapsed_request") {
        if let Ok(mut r) = req.lock() {
            *r = Some(secs);
        }
    }
    if let Some(slot) = data.get::<std::sync::Mutex<GameTime>>("game_time") {
        if let Ok(mut g) = slot.lock() {
            g.set_elapsed(secs);
        }
    }
}

/// The current game clock in seconds, from the DataStore copy the TimeSystem
/// exports every tick. 0 when the slot is absent (unit tests).
pub fn elapsed_now(data: &DataStore) -> f64 {
    data.get::<std::sync::Mutex<GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|g| g.elapsed_seconds))
        .unwrap_or(0.0)
}

// ── ONE GAME CLOCK (operator, 2026-09-27, decision-briefs.md Brief 6) ──────
// "The default day length should be 24 hours. Though we want it to be
// configurable. Like, maybe prefer a 20 hour day or 36 hour day. However that
// shouldn't change how long an hour is unless they change the setting that
// makes stuff happen faster/slower."
//
// So an hour is always 3,600 game seconds, a day is `hours_per_day` of them,
// and at time speed 1 a game second is a real second. The time speed is the
// ONLY thing that makes the world run faster: the garden's separate growth
// multiplier (2026-09-20) and the 20-minute day (1,200 s) are gone, and
// crops, tanks, batteries, the body's daily needs, the weather, the sun and
// the planet's spin all read this one clock.

/// Game seconds in an hour, whatever the day length.
pub const SECONDS_PER_HOUR: f64 = 3600.0;
/// Game seconds in an Earth day, 24 hours: the unit every per-day number in
/// the data is written in (plants.csv `growth_days`, litres a day, a year of
/// urine). A crop takes its `growth_days` of these whatever the calendar
/// says, because a plant grows by the hour, not by the name of the day.
pub const EARTH_DAY_S: f64 = 24.0 * SECONDS_PER_HOUR;

/// Hours in a day unless the player sets another (the operator's default).
pub const DEFAULT_HOURS_PER_DAY: u32 = 24;
/// The Settings range for hours in a day: whole hours, half to twice Earth's.
pub const MIN_HOURS_PER_DAY: u32 = 12;
/// See [`MIN_HOURS_PER_DAY`].
pub const MAX_HOURS_PER_DAY: u32 = 48;
/// Days in a year unless the player sets another. Earth's 365, to match
/// real hours; the operator did not decide this one (it is the agent's
/// choice of 2026-09-27, changeable in Settings).
pub const DEFAULT_DAYS_PER_YEAR: u32 = 365;
/// The Settings range for days in a year: four seasons of a week or more,
/// up to about a Mars year and a half.
pub const MIN_DAYS_PER_YEAR: u32 = 28;
/// See [`MIN_DAYS_PER_YEAR`].
pub const MAX_DAYS_PER_YEAR: u32 = 1000;

/// Time speed 1: a game second is a real second (the realistic mode).
pub const REALISTIC_TIME_SPEED: f32 = 1.0;
/// The time speed a new game starts at: realistic. The operator's words
/// above fix an hour at an hour "unless they change the setting".
pub const DEFAULT_TIME_SPEED: f32 = REALISTIC_TIME_SPEED;
/// The simplified mode's speed, the agent's proposal (2026-09-27): a day in
/// 20 real minutes, the pace the game had before the one clock, so a
/// lettuce (45 days) ripens in 15 hours of play and a 12-hour night passes
/// in 10 minutes.
pub const SIMPLIFIED_TIME_SPEED: f32 = 72.0;
/// The time speeds Settings offers as buttons, with their names.
pub const TIME_SPEED_PRESETS: [(f32, &str); 4] = [
    (REALISTIC_TIME_SPEED, "Realistic"),
    (24.0, "A day an hour"),
    (SIMPLIFIED_TIME_SPEED, "Simplified"),
    (720.0, "Garden testing"),
];
/// Slowest and fastest the setting goes. Below 1 the world crawls behind
/// real time; past 1,000 a whole day goes by in under 90 seconds.
pub const MIN_TIME_SPEED: f32 = 1.0;
/// See [`MIN_TIME_SPEED`].
pub const MAX_TIME_SPEED: f32 = 1000.0;

/// Clamp a time speed from any source (config file, Settings slider). A NaN
/// gives the default rather than a clock that never moves.
pub fn clamp_time_speed(v: f32) -> f32 {
    if !v.is_finite() {
        return DEFAULT_TIME_SPEED;
    }
    v.clamp(MIN_TIME_SPEED, MAX_TIME_SPEED)
}
/// Clamp hours in a day to the Settings range.
pub fn clamp_hours_per_day(v: u32) -> u32 {
    v.clamp(MIN_HOURS_PER_DAY, MAX_HOURS_PER_DAY)
}
/// Clamp days in a year to the Settings range.
pub fn clamp_days_per_year(v: u32) -> u32 {
    v.clamp(MIN_DAYS_PER_YEAR, MAX_DAYS_PER_YEAR)
}

/// DataStore slot: (hours per day, days per year) from Settings.
pub const CALENDAR_SLOT: &str = "time_calendar";
/// DataStore slot: the time-speed setting.
pub const SPEED_SLOT: &str = "time_speed_setting";
/// DataStore slot: a request to hold the clock's speed (Some(Some(x))) or
/// let it go back to the setting (Some(None)).
pub const HOLD_SLOT: &str = "time_speed_hold_request";

/// Put the clock's slots in a fresh DataStore (world init, and tests).
pub fn insert_slots(data: &mut DataStore) {
    data.insert(CALENDAR_SLOT, std::sync::Mutex::new((DEFAULT_HOURS_PER_DAY, DEFAULT_DAYS_PER_YEAR)));
    data.insert(SPEED_SLOT, std::sync::Mutex::new(DEFAULT_TIME_SPEED));
    data.insert(HOLD_SLOT, std::sync::Mutex::new(None::<Option<f32>>));
    // In a shared world the host's clock wins (2026-09-29): the host's clock
    // as it arrives, the summed jumps, and whether it is in charge.
    data.insert(HOST_CLOCK_SLOT, std::sync::Mutex::new(None::<f64>));
    data.insert(REBASE_SLOT, std::sync::Mutex::new(0.0_f64));
    data.insert(HOST_ACTIVE_SLOT, std::sync::Mutex::new(false));
}

/// Settings to the clock, every frame (lib.rs): hours in a day, days in a
/// year and the time speed. The TimeSystem applies them on its next tick.
pub fn publish_settings(data: &DataStore, hours_per_day: u32, days_per_year: u32, time_speed: f32) {
    if let Some(m) = data.get::<std::sync::Mutex<(u32, u32)>>(CALENDAR_SLOT) {
        if let Ok(mut c) = m.lock() {
            *c = (clamp_hours_per_day(hours_per_day), clamp_days_per_year(days_per_year));
        }
    }
    if let Some(m) = data.get::<std::sync::Mutex<f32>>(SPEED_SLOT) {
        if let Ok(mut s) = m.lock() {
            *s = clamp_time_speed(time_speed);
        }
    }
}

/// Hold the clock at `hold` game seconds a second (Some), or let it go back
/// to the time-speed setting (None). The F11 freeze, sleep and the probe rig.
pub fn request_speed_hold(data: &DataStore, hold: Option<f32>) {
    if let Some(m) = data.get::<std::sync::Mutex<Option<Option<f32>>>>(HOLD_SLOT) {
        if let Ok(mut r) = m.lock() {
            *r = Some(hold.map(|v| v.max(0.0)));
        }
    }
}

/// DataStore key: the longitude (degrees east) the player's home hangs over,
/// an `f64`. The engine publishes it at world load from the home station's
/// orbit (`station::orbit::hang_longitude_deg`). Absent (the relay, unit
/// tests) means longitude 0, where the game clock already is local time.
pub const HOME_LONGITUDE_KEY: &str = "home_longitude_deg";

/// The home's longitude, degrees east: 0 when nobody published one.
pub fn home_longitude_deg(data: &DataStore) -> f64 {
    data.get::<f64>(HOME_LONGITUDE_KEY).copied().unwrap_or(0.0)
}

/// The local hour at longitude `lon_deg` when it is `global_hour` at
/// longitude 0, on a day of `hours_per_day`: the day's hours are spread
/// round the planet, so each degree east is `hours_per_day / 360` later.
pub fn local_hour(global_hour: f64, lon_deg: f64, hours_per_day: u32) -> f64 {
    let h = f64::from(clamp_hours_per_day(hours_per_day));
    (global_hour + lon_deg / 360.0 * h).rem_euclid(h)
}

impl GameTime {
    /// How far through the game year, 0 to 1, continuously (the hour of the
    /// day included). The seasonal clock environment Layer 1 runs on
    /// (`systems::env_layer1`): a smooth annual cycle, where `season` is the
    /// four-step label.
    pub fn year_fraction(&self) -> f64 {
        let years = self.elapsed_seconds / self.seconds_per_day() / f64::from(clamp_days_per_year(self.days_per_year));
        years - years.floor()
    }
}

/// DataStore slot (`Mutex<Option<f64>>`): the shared world's clock, host game
/// seconds, put there each time the host (the relay) sends it while this
/// player is IN the shared world (`engine::net_route`, "game_time_sync").
/// Operator decision 2026-09-29: in a shared world the host's clock wins.
pub const HOST_CLOCK_SLOT: &str = "host_clock_sync";
/// DataStore slot (`Mutex<f64>`): every jump the clock has taken to follow
/// the host, summed, game seconds. A system that keeps ABSOLUTE game-time
/// stamps (a crop's `planted_at`) shifts them by what changed since it last
/// looked, so a crop's age never jumps with the clock: joining a world whose
/// day is 90 days behind yours neither ripens your garden nor erases it.
pub const REBASE_SLOT: &str = "clock_rebase_total";
/// DataStore slot (`Mutex<bool>`): true while the host's clock is in charge.
pub const HOST_ACTIVE_SLOT: &str = "host_clock_active";
/// The host's clock runs one game second per real second (the relay's world
/// tick), so while it is in charge that is everyone's speed.
pub const HOST_TIME_SPEED: f32 = 1.0;
/// Real seconds with no word from the host after which the clock is the
/// player's own again (the relay sends it every 5 s, so this is three missed
/// in a row: the player left the shared world or lost the connection). The
/// clock does not jump back: the host's date simply becomes the player's.
pub const HOST_RELEASE_S: f64 = 20.0;
/// The host's calendar: the relay counts 86,400-second days
/// (`relay/mod.rs`, `secs_per_day`), so a shared world has 24-hour days and
/// the default year, whatever a player's own Settings say. Without it two
/// players on one host clock saw different hours (review of 2026-09-29: 43,200
/// s is noon on a 24-hour day and 09:36 on a 30-hour one).
pub const HOST_HOURS_PER_DAY: u32 = 24;
/// The host's year, in days (the default).
pub const HOST_DAYS_PER_YEAR: u32 = DEFAULT_DAYS_PER_YEAR;

/// True while the host's clock is in charge (the player is in a shared world).
pub fn host_clock_active(data: &DataStore) -> bool {
    data.get::<std::sync::Mutex<bool>>(HOST_ACTIVE_SLOT).and_then(|m| m.lock().ok().map(|b| *b)).unwrap_or(false)
}

/// The summed clock jumps (see `REBASE_SLOT`); 0 before any.
pub fn rebase_total(data: &DataStore) -> f64 {
    data.get::<std::sync::Mutex<f64>>(REBASE_SLOT).and_then(|m| m.lock().ok().map(|v| *v)).unwrap_or(0.0)
}

/// Drives the day/night cycle and writes sun parameters to DataStore.
pub struct TimeSystem {
    game_time: GameTime,
    initialized: bool,
    /// Some(real seconds since the host last sent its clock) while the
    /// host's clock is in charge; None when the clock is the player's own.
    host_silence_s: Option<f64>,
    /// Every jump taken to follow the host, summed (`REBASE_SLOT`).
    rebase_total: f64,
}

impl TimeSystem {
    pub fn new() -> Self {
        Self {
            game_time: GameTime::default(),
            initialized: false,
            host_silence_s: None,
            rebase_total: 0.0,
        }
    }

    /// Compute sun direction from hour of day.
    /// Sun rises at 6, peaks at 12, sets at 18, below horizon at night.
    /// `pub` (not private): reused directly by the construction editor's manual
    /// sun-override (`GuiState::construction_sun_override`, src/lib.rs) since
    /// the real astronomical sun direction is tied to a ship position that
    /// never rotates -- this hour-based model is the only way to get a
    /// deliberately different lighting angle for editing.
    pub fn sun_direction(hour: f32) -> Vec3 {
        // Map hour to angle: 6h = 0 (horizon), 12h = PI/2 (zenith), 18h = PI (horizon)
        let day_fraction = (hour - 6.0) / 12.0; // 0 at sunrise, 1 at sunset
        let angle = day_fraction * std::f32::consts::PI;

        if hour >= 6.0 && hour <= 18.0 {
            // Daytime: sun arcs from east (+X) to west (-X), peaking at Y=1
            let y = angle.sin();
            let x = angle.cos();
            Vec3::new(x, y.max(0.01), -0.3).normalize()
        } else {
            // Nighttime: sun below horizon, provide faint moonlight direction
            Vec3::new(0.0, -0.5, -0.5).normalize()
        }
    }

    /// Compute sun color based on hour — warm at dawn/dusk, white at noon, dark at night.
    /// `pub` for the same reason as `sun_direction` above.
    pub fn sun_color(hour: f32) -> [f32; 3] {
        if hour < 5.0 || hour > 19.5 {
            // Deep night — faint blue moonlight
            [0.05, 0.05, 0.1]
        } else if hour < 6.5 {
            // Dawn — orange/pink
            let t = (hour - 5.0) / 1.5;
            [0.8 * t + 0.1, 0.3 * t + 0.05, 0.1 * t + 0.05]
        } else if hour < 8.0 {
            // Morning — warming to white
            let t = (hour - 6.5) / 1.5;
            let r = 0.9 + 0.1 * t;
            let g = 0.35 + 0.55 * t;
            let b = 0.15 + 0.75 * t;
            [r, g, b]
        } else if hour < 16.0 {
            // Full daylight — warm white
            [1.0, 0.95, 0.9]
        } else if hour < 18.0 {
            // Evening — cooling toward golden hour
            let t = (hour - 16.0) / 2.0;
            [1.0, 0.95 - 0.5 * t, 0.9 - 0.7 * t]
        } else {
            // Dusk — fading orange to night
            let t = (hour - 18.0) / 1.5;
            let r = (0.9 * (1.0 - t)).max(0.05);
            let g = (0.4 * (1.0 - t)).max(0.05);
            let b = (0.2 * (1.0 - t)).max(0.1);
            [r, g, b]
        }
    }
}

/// `dt` scaled by the CURRENT game `time_scale`, read from the DataStore's
/// `game_time` slot (TimeSystem registers first and exports it every frame, so
/// downstream systems see this frame's value). Timer-based economy systems
/// (crafting, drones, manufacturing) call this so "accelerated for testing"
/// speeds the WHOLE economy, not just the wall clock (v0.663 -- previously
/// those systems ran on raw dt and ignored the time scale entirely). Do NOT
/// pre-multiply dt at the SystemRunner call site instead: TimeSystem scales
/// internally and would double-scale. Absent slot (unit tests) = raw dt.
pub fn scaled_dt(dt: f32, data: &DataStore) -> f32 {
    data.get::<std::sync::Mutex<GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|g| dt * g.time_scale))
        .unwrap_or(dt)
}

impl System for TimeSystem {
    fn name(&self) -> &str {
        "TimeSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, dt: f32, data: &DataStore) {
        // The host's clock, if it spoke this tick (see HOST_CLOCK_SLOT below).
        let host = data
            .get::<std::sync::Mutex<Option<f64>>>(HOST_CLOCK_SLOT)
            .and_then(|m| m.lock().ok().and_then(|mut r| r.take()));
        // The calendar: the host's in a shared world (HOST_HOURS_PER_DAY),
        // otherwise the player's from Settings (publish_settings), read every
        // tick. Absent (tests that never publish) = the defaults.
        if host.is_some() || self.host_silence_s.is_some() {
            self.game_time.hours_per_day = HOST_HOURS_PER_DAY;
            self.game_time.days_per_year = HOST_DAYS_PER_YEAR;
        } else if let Some((h, d)) = data.get::<std::sync::Mutex<(u32, u32)>>(CALENDAR_SLOT).and_then(|m| m.lock().ok().map(|c| *c)) {
            self.game_time.hours_per_day = clamp_hours_per_day(h);
            self.game_time.days_per_year = clamp_days_per_year(d);
        }
        // Dev/screenshot clock control (v0.871): an external hour request
        // jumps the clock. The TimeSystem's own accumulator is authoritative
        // (it re-exports over the DataStore copy every tick), so outside
        // writers go through this channel instead of poking the mutex.
        if let Some(req) = data.get::<std::sync::Mutex<Option<f32>>>("time_set_hour_request") {
            if let Ok(mut r) = req.lock() {
                if let Some(h) = r.take() {
                    self.set_hour(h);
                }
            }
        }
        let setting = data
            .get::<std::sync::Mutex<f32>>(SPEED_SLOT)
            .and_then(|m| m.lock().ok().map(|s| clamp_time_speed(*s)))
            .unwrap_or(DEFAULT_TIME_SPEED);
        // A hold over the setting (request_speed_hold): the F11 panel's
        // freeze (v0.1224, 0 is what makes a lighting comparison repeatable),
        // a night's sleep, the probe rig. It goes through this channel
        // because the DataStore copy is overwritten from this system's own
        // GameTime at the end of every tick.
        if let Some(req) = data.get::<std::sync::Mutex<Option<Option<f32>>>>(HOLD_SLOT) {
            if let Ok(mut r) = req.lock() {
                if let Some(hold) = r.take() {
                    self.game_time.speed_hold = hold.map(|v| v.max(0.0));
                }
            }
        }
        // The host's clock (HOST_CLOCK_SLOT): in a shared world it wins
        // (operator, 2026-09-29). Each word from the host sets the clock to
        // it, and the jump is added to the rebase total so absolute stamps
        // follow; with no word for HOST_RELEASE_S the clock is the player's.
        if let Some(h) = host {
            let jump = h - self.game_time.elapsed_seconds;
            if jump != 0.0 {
                self.game_time.set_elapsed(h);
                self.rebase_total += jump;
            }
            self.host_silence_s = Some(0.0);
        } else if let Some(s) = self.host_silence_s.as_mut() {
            *s += f64::from(dt);
            if *s > HOST_RELEASE_S {
                self.host_silence_s = None;
            }
        }
        self.game_time.time_scale = if self.host_silence_s.is_some() {
            HOST_TIME_SPEED
        } else {
            self.game_time.speed_hold.unwrap_or(setting)
        };
        // Absolute clock restore from a save (see request_restore_elapsed).
        // Same channel shape as the two above, f64 because it carries the
        // whole clock, not an hour.
        if let Some(req) = data.get::<std::sync::Mutex<Option<f64>>>("time_restore_elapsed_request") {
            if let Ok(mut r) = req.lock() {
                if let Some(secs) = r.take() {
                    self.game_time.set_elapsed(secs);
                }
            }
        }
        let scaled_dt = dt as f64 * self.game_time.time_scale as f64;
        self.game_time.elapsed_seconds += scaled_dt;

        // Hour, day count and season from the total.
        self.game_time.recompute_derived();

        self.initialized = true;

        // Export the freshly-advanced time to the DataStore so downstream
        // time-dependent systems (farming / ecology / weather / hydrology) and the
        // GUI HUD read the CURRENT value. TimeSystem is registered first, so this
        // write lands before those systems tick in the same frame. Uses a Mutex
        // (the interaction_prompt pattern) because System::tick only gets
        // &DataStore — interior mutability is how a system writes back. The slot is
        // pre-seeded at world init; if it's somehow absent we skip and readers fall
        // back to their defaults. BEFORE v0.324 this export never happened (despite
        // the module doc claiming it), so every time-dependent system ran on
        // Spring / elapsed=0 — the audit's "crops freeze" finding.
        if let Some(slot) = data.get::<std::sync::Mutex<GameTime>>("game_time") {
            if let Ok(mut g) = slot.lock() {
                *g = self.game_time.clone();
            }
        }
        if let Some(slot) = data.get::<std::sync::Mutex<f64>>(REBASE_SLOT) {
            if let Ok(mut v) = slot.lock() {
                *v = self.rebase_total;
            }
        }
        if let Some(slot) = data.get::<std::sync::Mutex<bool>>(HOST_ACTIVE_SLOT) {
            if let Ok(mut v) = slot.lock() {
                *v = self.host_silence_s.is_some();
            }
        }
    }
}

impl TimeSystem {
    /// Get current game time (for systems that need to read it directly).
    pub fn game_time(&self) -> &GameTime {
        &self.game_time
    }

    /// Jump to an hour of the current day, on the clock a person reads
    /// (0 to `hours_per_day`).
    pub fn set_hour(&mut self, hour: f32) {
        let hours = f64::from(clamp_hours_per_day(self.game_time.hours_per_day));
        let clamped = f64::from(hour).rem_euclid(hours);
        let current_day_start = f64::from(self.game_time.day_count) * self.game_time.seconds_per_day();
        self.game_time.set_elapsed(current_day_start + clamped * SECONDS_PER_HOUR);
    }

    // (2026-09-29) current_sun_direction, current_sun_color and is_daytime
    // were removed unused: they read the game clock alone, longitude 0's
    // time, where the day and night a player sees depend on where they are
    // (`solar_hour_at`, `weather::local_solar_hour`).
}

#[cfg(test)]
mod game_time_export_tests {
    use super::*;
    use crate::ecs::systems::System;
    use crate::hot_reload::data_store::DataStore;

    /// THE HOST'S CLOCK WINS IN A SHARED WORLD (operator, 2026-09-29). A
    /// player on day 100 at time speed 72 joins a world whose host is on day
    /// 3: the clock goes to the host's, runs at the host's speed, and the jump
    /// is recorded for absolute stamps to follow. After 20 s with no word from
    /// the host the speed is the player's own again, and the date stays the
    /// host's. Red check, run: ignoring HOST_CLOCK_SLOT fails the first
    /// assertion.
    /// THE HOST'S CALENDAR COMES WITH ITS CLOCK (review of 2026-09-29). A
    /// player who set 30-hour days joins: while the host is in charge the day
    /// is the host's 24 hours, so 43,200 s is noon for everyone; after the
    /// host falls silent the player's own calendar is back. Red check, run:
    /// reading the Settings calendar while hosted fails the first assertion
    /// (the hour reads 9.6).
    #[test]
    fn the_host_calendar_comes_with_the_host_clock() {
        let mut data = DataStore::new();
        insert_slots(&mut data);
        data.insert("game_time", std::sync::Mutex::new(GameTime::default()));
        data.insert("time_restore_elapsed_request", std::sync::Mutex::new(None::<f64>));
        publish_settings(&data, 30, 365, 1.0);
        *data.get::<std::sync::Mutex<Option<f64>>>(HOST_CLOCK_SLOT).unwrap().lock().unwrap() = Some(43_200.0);
        let mut sys = TimeSystem::new();
        let mut world = hecs::World::new();
        sys.tick(&mut world, 0.0, &data);
        let g = data.get::<std::sync::Mutex<GameTime>>("game_time").unwrap().lock().unwrap().clone();
        assert_eq!(g.hours_per_day, HOST_HOURS_PER_DAY);
        assert!((g.solar_hour() - 12.0).abs() < 1e-3, "noon on the host's clock: {}", g.solar_hour());
        // 21 s of silence releases the host; the calendar is chosen at the
        // start of a tick, so the player's own is back from the next one.
        for _ in 0..22 {
            sys.tick(&mut world, 1.0, &data);
        }
        let g = data.get::<std::sync::Mutex<GameTime>>("game_time").unwrap().lock().unwrap().clone();
        assert_eq!(g.hours_per_day, 30, "the player's own calendar back after the host falls silent");
    }

    #[test]
    fn the_host_clock_wins_while_joined() {
        let mut data = DataStore::new();
        let mut gt = GameTime::default();
        gt.set_elapsed(100.0 * EARTH_DAY_S);
        data.insert("game_time", std::sync::Mutex::new(gt.clone()));
        data.insert(SPEED_SLOT, std::sync::Mutex::new(72.0_f32));
        data.insert(HOST_CLOCK_SLOT, std::sync::Mutex::new(None::<f64>));
        data.insert(REBASE_SLOT, std::sync::Mutex::new(0.0_f64));
        data.insert(HOST_ACTIVE_SLOT, std::sync::Mutex::new(false));
        data.insert("time_restore_elapsed_request", std::sync::Mutex::new(Some(100.0 * EARTH_DAY_S)));
        let mut sys = TimeSystem::new();
        sys.tick(&mut hecs::World::new(), 0.0, &data); // take the saved clock first
        *data.get::<std::sync::Mutex<Option<f64>>>(HOST_CLOCK_SLOT).unwrap().lock().unwrap() = Some(3.0 * EARTH_DAY_S);
        let mut world = hecs::World::new();
        sys.tick(&mut world, 1.0, &data);
        let now = elapsed_now(&data);
        assert!((now - (3.0 * EARTH_DAY_S + 1.0)).abs() < 1e-6, "the host's day 3, one host second on: {now}");
        assert!((rebase_total(&data) - (-97.0 * EARTH_DAY_S)).abs() < 1e-6, "the jump is recorded");
        assert!(host_clock_active(&data));
        assert_eq!(data.get::<std::sync::Mutex<GameTime>>("game_time").unwrap().lock().unwrap().time_scale, HOST_TIME_SPEED);
        for _ in 0..21 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(!host_clock_active(&data), "20 s of silence: the clock is the player's again");
        let g = data.get::<std::sync::Mutex<GameTime>>("game_time").unwrap().lock().unwrap().clone();
        assert_eq!(g.time_scale, 72.0, "their own speed back");
        assert!(g.elapsed_seconds < 4.0 * EARTH_DAY_S, "the date stays the host's, no jump back");
    }

    #[test]
    fn time_system_exports_advanced_game_time_to_datastore() {
        // Pre-seed the slot exactly as world init does, tick the system, and confirm
        // the advanced time is visible in the DataStore — the export that never
        // happened before v0.324 (so farming/HUD/seasons all saw Spring / t=0).
        let mut data = DataStore::new();
        data.insert("game_time", std::sync::Mutex::new(GameTime::default()));
        let mut world = hecs::World::new();
        let mut sys = TimeSystem::new();
        for _ in 0..10 {
            sys.tick(&mut world, 1.0, &data);
        }
        let exported = data
            .get::<std::sync::Mutex<GameTime>>("game_time")
            .expect("game_time slot present")
            .lock()
            .unwrap()
            .clone();
        assert!(
            exported.elapsed_seconds > 0.0,
            "TimeSystem did not export advanced time to the DataStore"
        );
        // Exported value must equal the system's internal state.
        assert!((exported.elapsed_seconds - sys.game_time().elapsed_seconds).abs() < 1e-9);
    }

    #[test]
    fn absent_game_time_slot_is_a_safe_noop() {
        // If the slot was never seeded, the export silently skips (no panic) and the
        // system still advances its own clock.
        let data = DataStore::new();
        let mut world = hecs::World::new();
        let mut sys = TimeSystem::new();
        sys.tick(&mut world, 1.0, &data);
        assert!(sys.game_time().elapsed_seconds > 0.0);
    }

    #[test]
    fn restore_request_puts_the_clock_back_where_the_save_left_it() {
        // The restart bug: the clock started at zero every launch. A restore
        // must land in the SYSTEM's accumulator (which re-exports every tick),
        // and be visible in the DataStore copy before the first tick too.
        let mut data = DataStore::new();
        data.insert("game_time", std::sync::Mutex::new(GameTime::default()));
        data.insert(
            "time_restore_elapsed_request",
            std::sync::Mutex::new(Option::<f64>::None),
        );
        let saved = 3.0 * EARTH_DAY_S + 12.0 * SECONDS_PER_HOUR; // day 3, noon
        request_restore_elapsed(&data, saved);
        assert!((elapsed_now(&data) - saved).abs() < 1e-9, "visible before the first tick");

        let mut world = hecs::World::new();
        let mut sys = TimeSystem::new();
        sys.tick(&mut world, 1.0, &data);
        let gt = sys.game_time().clone();
        assert!((gt.elapsed_seconds - (saved + 1.0)).abs() < 1e-9, "resumed from the save, then advanced");
        assert_eq!(gt.day_count, 3);
        assert!((gt.hour - 12.0).abs() < 0.05, "derived hour follows the restored total, got {}", gt.hour);
        assert!((elapsed_now(&data) - gt.elapsed_seconds).abs() < 1e-9, "exported copy agrees");

        // Consumed once: the next tick just advances.
        sys.tick(&mut world, 1.0, &data);
        assert!((sys.game_time().elapsed_seconds - (saved + 2.0)).abs() < 1e-9);
    }
}

#[cfg(test)]
mod scaled_dt_tests {
    use super::*;

    /// scaled_dt multiplies by the exported time_scale, and falls back to raw dt
    /// when no game_time slot exists (unit tests / headless contexts).
    #[test]
    fn scaled_dt_tracks_time_scale_and_defaults_to_raw() {
        let empty = DataStore::new();
        assert_eq!(scaled_dt(1.0, &empty), 1.0, "no slot = raw dt");

        let mut data = DataStore::new();
        let mut gt = GameTime::default();
        gt.time_scale = 5.0;
        data.insert("game_time", std::sync::Mutex::new(gt));
        assert!((scaled_dt(2.0, &data) - 10.0).abs() < 1e-6, "dt x time_scale");
    }
}

#[cfg(test)]
mod sun_override_tests {
    // `sun_direction`/`sun_color` were made `pub` (v0.652.0) specifically so the
    // construction editor's manual sun-angle override (GuiState::
    // construction_sun_override) can drive lighting independent of a real
    // TimeSystem instance -- these tests pin the values that override control
    // actually relies on.
    use super::*;

    #[test]
    fn noon_gives_a_high_elevation_sun() {
        let dir = TimeSystem::sun_direction(12.0);
        assert!(dir.y > 0.9, "noon sun should be nearly overhead, got y={}", dir.y);
    }

    #[test]
    fn sunrise_and_sunset_give_a_low_elevation_sun() {
        let sunrise = TimeSystem::sun_direction(6.0);
        let sunset = TimeSystem::sun_direction(18.0);
        assert!(sunrise.y < 0.2, "sunrise should be near the horizon, got y={}", sunrise.y);
        assert!(sunset.y < 0.2, "sunset should be near the horizon, got y={}", sunset.y);
    }

    #[test]
    fn midnight_falls_back_to_a_below_horizon_moonlight_direction() {
        let dir = TimeSystem::sun_direction(0.0);
        assert!(dir.y < 0.0, "midnight should be below the horizon, got y={}", dir.y);
    }

    #[test]
    fn noon_color_is_near_white_midnight_is_dim_blue() {
        let noon = TimeSystem::sun_color(12.0);
        let midnight = TimeSystem::sun_color(0.0);
        assert!(noon[0] > 0.9 && noon[1] > 0.9, "noon should read as near-white, got {noon:?}");
        assert!(
            midnight[0] < 0.2 && midnight[2] > midnight[0],
            "midnight should read as dim and blue-shifted, got {midnight:?}"
        );
    }
}

/// The one clock of 2026-09-27 (decision-briefs.md Brief 6).
#[cfg(test)]
mod one_clock_tests {
    use super::*;

    fn store() -> DataStore {
        let mut data = DataStore::new();
        data.insert("game_time", std::sync::Mutex::new(GameTime::default()));
        data.insert("time_set_hour_request", std::sync::Mutex::new(None::<f32>));
        insert_slots(&mut data);
        data
    }

    /// The default day is 24 hours of 3,600 s, 86,400 game seconds, and a
    /// 36-hour day is 129,600; the hour is the same length in both, so the
    /// hour a person reads moves by exactly one every 3,600 game seconds.
    /// Red check, run: `seconds_per_day` returning the old fixed 1,200 fails
    /// the first assertion, and deriving `hour` as a 24th of the day (the old
    /// `day_seconds / SECONDS_PER_DAY * 24`) fails the 36-hour hour step.
    #[test]
    fn the_day_is_its_hours_and_an_hour_is_always_3600_seconds() {
        let day24 = GameTime::default();
        assert_eq!(day24.seconds_per_day(), 86_400.0);
        let day36 = GameTime { hours_per_day: 36, ..Default::default() };
        assert_eq!(day36.seconds_per_day(), 129_600.0);
        for hours in [24, 36, 20] {
            let mut g = GameTime { hours_per_day: hours, ..Default::default() };
            g.set_elapsed(5.0 * SECONDS_PER_HOUR);
            let before = g.hour;
            g.set_elapsed(6.0 * SECONDS_PER_HOUR);
            assert!((g.hour - before - 1.0).abs() < 1e-4, "{hours}-hour day: an hour moved the clock {}", g.hour - before);
        }
        // Through the system: Settings publish 36 hours, the day follows.
        let data = store();
        publish_settings(&data, 36, 365, 1.0);
        let mut sys = TimeSystem::new();
        let mut world = hecs::World::new();
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(sys.game_time().seconds_per_day(), 129_600.0);
        // Noon of a 36-hour day is hour 18, and the sun is overhead there.
        let mut noon = GameTime { hours_per_day: 36, ..Default::default() };
        noon.set_elapsed(18.0 * SECONDS_PER_HOUR);
        assert!((noon.solar_hour() - 12.0).abs() < 1e-4);
        assert!((noon.day_fraction() - 0.5).abs() < 1e-9);
    }

    /// The time speed is the only speed-up: at 72 the clock runs 72 game
    /// seconds a real second (and the sun with it), a hold (the F11 freeze,
    /// sleep) overrides it, and letting go puts the setting back. Red check,
    /// run: dropping `self.game_time.time_scale = ...` leaves the clock at
    /// the default speed 1 and the first assertion fails (72 s wanted, 1 got).
    #[test]
    fn the_time_speed_setting_drives_the_clock_and_a_hold_overrides_it() {
        let data = store();
        let mut sys = TimeSystem::new();
        let mut world = hecs::World::new();
        publish_settings(&data, 24, 365, 72.0);
        sys.tick(&mut world, 1.0, &data);
        assert!((sys.game_time().elapsed_seconds - 72.0).abs() < 1e-6, "{}", sys.game_time().elapsed_seconds);
        assert!((scaled_dt(0.5, &data) - 36.0).abs() < 1e-5, "every system's scaled_dt follows the same speed");
        // Held still.
        request_speed_hold(&data, Some(0.0));
        sys.tick(&mut world, 1.0, &data);
        assert!((sys.game_time().elapsed_seconds - 72.0).abs() < 1e-6, "held");
        // Let go: back to the setting.
        request_speed_hold(&data, None);
        sys.tick(&mut world, 1.0, &data);
        assert!((sys.game_time().elapsed_seconds - 144.0).abs() < 1e-6);
        // Out-of-range settings are clamped, never a frozen clock.
        assert_eq!(clamp_time_speed(0.0), MIN_TIME_SPEED);
        assert_eq!(clamp_time_speed(f32::NAN), DEFAULT_TIME_SPEED);
        assert_eq!(clamp_hours_per_day(3), MIN_HOURS_PER_DAY);
    }

    /// Seasons follow the year the player set: a quarter of it each, Spring
    /// first. Red check, run: `from_day` on the old fixed 120-day year puts
    /// day 100 of a 365-day year in Winter, not Spring.
    #[test]
    fn seasons_follow_the_configured_year() {
        assert_eq!(Season::from_day(0, 365), Season::Spring);
        assert_eq!(Season::from_day(100, 365), Season::Summer);
        assert_eq!(Season::from_day(90, 365), Season::Spring);
        assert_eq!(Season::from_day(200, 365), Season::Autumn);
        assert_eq!(Season::from_day(300, 365), Season::Winter);
        assert_eq!(Season::from_day(365, 365), Season::Spring, "a new year");
        assert_eq!(Season::from_day(30, 120), Season::Summer, "a 120-day year");
        let mut g = GameTime { days_per_year: 365, ..Default::default() };
        g.set_elapsed(182.5 * EARTH_DAY_S);
        assert!((g.year_fraction() - 0.5).abs() < 1e-9, "{}", g.year_fraction());
        let mut short = GameTime { days_per_year: 120, ..Default::default() };
        short.set_elapsed(60.0 * EARTH_DAY_S);
        assert!((short.year_fraction() - 0.5).abs() < 1e-9);
        assert_eq!(short.season, Season::Autumn);
    }

    /// The local hour spreads the day's hours round the planet: 15 degrees
    /// an hour on a 24-hour day, 10 on a 36-hour one.
    #[test]
    fn local_hour_spreads_the_day_round_the_planet() {
        assert!((local_hour(12.0, 15.0, 24) - 13.0).abs() < 1e-9);
        assert!((local_hour(12.0, -90.0, 36) - 3.0).abs() < 1e-9);
        assert!((local_hour(1.0, -30.0, 24) - 23.0).abs() < 1e-9);
    }
}
