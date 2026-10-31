//! The farm on its own clock (2026-10-31, docs/design/farm-clock.md rung 2).
//!
//! The operator, 2026-10-30, on FarmingSystem running every frame: "What if we only update plant
//! data once every hour or 6 or 24 or whatever we configure? I imagine this would help MMO
//! servers." Until this rung the farm advanced every crop every frame: 3.6 ms of every frame on
//! average with the showcase's 2,000 crops (sim-frame-cost.md, Survey 1), though a plant at real
//! time changes by nothing anyone could see in a second. Rung 1 made a step of any length exact,
//! so the farm can now take its steps on a clock of its own:
//!
//! - **The interval** is the player's (Settings > Gameplay, "Garden updates": every frame, every
//!   second, the default, every minute or every hour), in game seconds
//!   ([`INTERVAL_KEY`]). Each grow AREA (a crop's `tower_id`: a tower, a bed, a field) advances
//!   once an interval, by exactly the game time since its own last advance ([`AreaClock::owed`]).
//! - **Time-sliced:** the interval is cut into [`TURNS`] turns and each area has one
//!   ([`AreaClock::turn`], the turn with the fewest crops when the area is first seen), so a
//!   frame advances only the areas whose turn came up and the cost of an interval is spread over
//!   its frames instead of landing on one.
//! - **The air** (every grow room's and tent's, the home's own, their fans, heaters and hives) is
//!   one coupled whole: each room's air exchanges with the home's, and the household breathes in
//!   it. It is cheap and the people in the home breathe it, so it takes its own turn, slot 0 of
//!   the rotation, and never waits longer than [`AIR_MAX_S`] whatever the interval.
//! - **What reads the farm brings it up to now first.** A player's command on a crop or an area
//!   (a harvest, a watering, a sowing, a pest control) advances the areas it touches to now before
//!   it acts (the tick, `FarmingSystem::tick`); the save brings every area and the air to now
//!   (`farming::settle(.., 0.0)`); the Garden page and the plant meshes keep what they show within
//!   [`LOOK_FRESH_S`] of now, and while they look, the rotation runs at no more than that.
//! - **The reuse switch** (crate::reuse, the Dev page's "Turn off all reuse"): an area not yet
//!   advanced to now is state kept stale on purpose, so with reuse off every area and the air
//!   advance every frame, as before this rung.
//!
//! The coupling, worked out from the tick (farm-clock.md, "What an area's advance carries"): an
//! area's advance carries its crops with their soil, its pests, its weeds, its soil pH units, its
//! banked organic nitrogen, its feeder's opened bags, its crops' pollination and picking records.
//! None of those is read by another area. What is shared is read where it stands: the room's air
//! (the air's own turn), the home's fertilizer stock and the tanks (drawn as each area advances),
//! the grow lights' share-out (worked out from every area's living crops when an area needs it).
//! Each crossing of an area's slice is first order in the interval, and step_tests.rs and
//! clock_tests.rs measure it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use crate::ecs::components::CropInstance;
use crate::hot_reload::data_store::DataStore;

/// The DataStore channel the Settings choice reaches the farm by (`Mutex<f64>`): game seconds
/// between two advances of each area, 0 for every frame. Absent (a headless test, a store nobody
/// set up): every frame, the farm as it ran before this rung.
pub const INTERVAL_KEY: &str = "garden_update_s";

/// The Settings default: every game second, which at the normal (real-time) speed is every real
/// second. A second's change to a crop cannot be seen (its water moves 0.003% and its health at
/// most 0.014 of 100), and at 30 frames a second it is 1/30 of the farm's work a frame.
pub const DEFAULT_INTERVAL_S: f64 = 1.0;

/// The longest interval the farm takes (the channel holds no more): a game day, for the server's
/// lazy homes of rung 4 and for measuring.
pub const MAX_INTERVAL_S: f64 = 86_400.0;

/// The longest interval a player can choose: a game hour. A hand-edited config past it is held
/// there. The operator also named 6 hours and a day; measured 2026-10-31 (clock_tests.rs
/// `six_hours_and_a_day_measured`, farm-clock.md) they do not hold up yet: inside a step that long
/// what rung 1 still reads at a step's start (the feeder's dose for the step, a new organic bank's
/// age, a field's weather) moves a unit's nitrogen by 0.9% at six hours and 2.8% at a day and a
/// crop's health by 0.05, and with the air a grow room's CO2 by 88% at six hours. Rung 3's events
/// are what makes them exact.
pub const LONGEST_CHOICE_S: f64 = 3_600.0;

/// The intervals Settings > Gameplay offers, game seconds, each with the key of its words in
/// data/i18n/en.json.
pub const CHOICES: [(f64, &str); 4] = [
    (0.0, "settings.garden_updates.every_frame"),
    (1.0, "settings.garden_updates.every_second"),
    (60.0, "settings.garden_updates.every_minute"),
    (3_600.0, "settings.garden_updates.every_hour"),
];

/// An interval as the farm takes it: not negative, at most a day, and every frame for a number
/// that is not one.
pub fn clamp_interval(s: f64) -> f64 {
    if s.is_finite() {
        s.clamp(0.0, MAX_INTERVAL_S)
    } else {
        0.0
    }
}

/// A player's choice as Settings keeps it (the config on its way in): `clamp_interval`, and no
/// longer than `LONGEST_CHOICE_S`.
pub fn clamp_choice(s: f64) -> f64 {
    clamp_interval(s).min(LONGEST_CHOICE_S)
}

/// The turns an interval is cut into: slot 0 is the air's, slots 1 to 15 the areas'. A frame's
/// worst is a turn's share of the farm plus a turn's own fixed cost (reading the clock, the
/// light, the water, the rooms, picking out its crops); its average is the whole farm once an
/// interval plus that fixed cost once a turn. Measured in the game with the showcase's 1,981 crops
/// (2026-10-31, sim-frame-cost.md): at 8 turns a turn's frame reached 0.75 to 1.4 ms, over the
/// 0.5 ms budget, so the interval is cut finer, a fifteenth of the crops a turn.
pub const TURNS: u32 = 16;

/// The longest the air waits between two steps, game seconds, whatever the interval: the people
/// in the home breathe it, the atmosphere system judges it and the HUD shows it every frame.
pub const AIR_MAX_S: f64 = 1.0;

/// What a reader that shows the garden to a person (the Garden page, the plant meshes) keeps it
/// within, game seconds: in a second nothing about a crop can be seen to change (see
/// `DEFAULT_INTERVAL_S`).
pub const LOOK_FRESH_S: f64 = 1.0;

/// Frames a reader's wish for a fresh garden lasts after it last asked: the rotation runs at no
/// more than `LOOK_FRESH_S` until it lapses. About a second at 30 frames a second, so a page
/// drawn every few frames (a wall screen's turn) keeps it.
pub const LOOK_KEEP_FRAMES: u32 = 30;

/// The DataStore channel by which a save just loaded into the world tells the farm
/// (`Mutex<bool>`): its crops are as the save left them, at the clock the save restored, so every
/// area starts again from now (`save_load::resume_home`).
pub const RESUME_KEY: &str = "garden_clock_resume";

/// Register the farm clock's channels (the main loop, at boot): the interval at the default until
/// the Settings value reaches it, and the resume flag.
pub fn register(data: &mut DataStore) {
    data.insert(INTERVAL_KEY, Mutex::new(DEFAULT_INTERVAL_S));
    data.insert(RESUME_KEY, Mutex::new(false));
}

/// Carry this frame's Settings choice to the farm (lib.rs, every frame).
pub fn publish(data: &DataStore, interval_s: f64) {
    if let Some(Ok(mut v)) = data.get::<Mutex<f64>>(INTERVAL_KEY).map(|m| m.lock()) {
        *v = clamp_interval(interval_s);
    }
}

/// The interval the DataStore holds, or None (every frame) without one.
pub fn interval(data: &DataStore) -> Option<f64> {
    data.get::<Mutex<f64>>(INTERVAL_KEY).and_then(|m| m.lock().ok().map(|v| clamp_interval(*v)))
}

/// A save was just loaded into the world (`save_load::resume_home`): the farm takes it at its next
/// tick and starts every area's clock again from then.
pub fn hand_over_resume(data: &DataStore) {
    if let Some(Ok(mut v)) = data.get::<Mutex<bool>>(RESUME_KEY).map(|m| m.lock()) {
        *v = true;
    }
}

/// Take the resume flag, leaving it down (the farming tick).
pub fn take_resume(data: &DataStore) -> bool {
    data.get::<Mutex<bool>>(RESUME_KEY).and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v))).unwrap_or(false)
}

/// Which grow area of which farm clock a crop belongs to: the index of its `tower_id` in that
/// clock's table, put on the crop the first time the farm sees it (`FarmClock::mark_new`) so a
/// turn picks out its crops by a number, not by comparing names. Not saved: a loaded crop gets it
/// again. A crop's `tower_id` never changes after it is sown, so the mark cannot go stale; and
/// with the reuse switch off the farm advances every crop every frame without reading it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FarmArea {
    /// The clock that put it there (`FarmClock::id`): a crop marked by another clock is not one of
    /// this clock's areas.
    pub clock: u32,
    pub index: u32,
}

/// Each FarmClock's own number, so two farms (two tests on one thread) never read each other's
/// marks.
static NEXT_CLOCK: AtomicU32 = AtomicU32::new(1);

/// One grow area's place in the rotation.
#[derive(Debug, Clone)]
pub struct AreaClock {
    /// Its tag: a machine instance id, a tower's design id, "" for the hand-planted crops.
    pub name: String,
    /// Game seconds since it last advanced: the step its next advance takes.
    pub owed: f64,
    /// Its slot in the rotation, 1 to TURNS - 1.
    pub turn: u32,
    /// Its crops when it last advanced (or when it was first seen): what the next new area is
    /// balanced against.
    pub crops: usize,
    /// Litres a day its irrigation draws, as its last advance worked it out: the tanks are drawn
    /// at this rate until its next (`FarmClock::irrigation_l_day`).
    pub irrigation_l_day: f32,
    /// Not advanced yet (new, or a save was just loaded): it advances on the frame it is seen,
    /// whoever's turn it is, so the air steps on its breath and the tanks see its draw from the
    /// first frame, not from its first turn. Seen in clock_tests.rs (2026-10-31): without it a
    /// minute's interval left the rooms' air stepping without the crops' breath for up to a
    /// minute after every load, their CO2 and water 4 to 14% off at the end.
    pub fresh: bool,
}

/// What is due on one frame: the areas whose turn came up, every area, the air.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Due {
    /// Every area (the interval is every frame, the reuse switch is off, or the frame spanned a
    /// whole interval).
    pub all: bool,
    /// The turns that came up, by slot (slot 0, the air's, holds no areas).
    pub turns: [bool; TURNS as usize],
    pub air: bool,
}

/// Which turns of the rotation a frame brought up that moved the farm clock from `t0` to `t`,
/// with the areas advancing every `interval_s` and the air every `air_s`. Pure, so the rule is
/// tested (clock_tests.rs). `every` (the interval is every frame, the reuse switch is off, a burn
/// is harming the crops a frame at a time): everything.
///
/// A turn comes up when the frame crosses its boundary: the interval is cut into TURNS equal
/// parts, part k of every interval belongs to slot k mod TURNS. A frame that spans a whole
/// interval (a clock faster than the interval, a long hitch) brings up every turn, so nothing is
/// ever skipped; a frame of no game time (the clock held) brings up nothing. Both ends are the
/// clock's own values, never one worked back from the other (`t - dt` can round below a
/// boundary the frame before already crossed, and bring a turn up twice).
pub fn plan(t0: f64, t: f64, interval_s: f64, air_s: f64, every: bool) -> Due {
    let mut due = Due { all: false, turns: [false; TURNS as usize], air: false };
    if every || !(interval_s > 0.0) {
        due.all = true;
        due.air = true;
        return due;
    }
    let n = TURNS as f64;
    let (k0, k1) = ((t0 * n / interval_s).floor(), (t * n / interval_s).floor());
    if k1 - k0 >= n {
        due.all = true;
    } else {
        let mut k = k0 + 1.0;
        while k <= k1 {
            due.turns[(k.rem_euclid(n)) as usize] = true;
            k += 1.0;
        }
    }
    due.air = due.all || !(air_s > 0.0) || (t / air_s).floor() > (t0 / air_s).floor();
    due
}

/// The farm's own clock: every grow area's place in the rotation and its time owed, the air's,
/// and how far the rotation has turned.
#[derive(Debug)]
pub struct FarmClock {
    /// This clock's number, carried on its marks (`FarmArea::clock`).
    pub id: u32,
    pub areas: Vec<AreaClock>,
    by_name: HashMap<String, u32>,
    /// Game seconds since the air last stepped.
    pub air_owed: f64,
    /// Game seconds this clock has run (the frames' steps summed, not the world clock, so a jump
    /// of the clock, a dev hour set or a save restored, is not a step, as it never was): where the
    /// rotation stands.
    pub farm_t: f64,
    /// Frames left while a reader wants the garden fresh (`LOOK_FRESH_S`).
    pub look_frames: u32,
    /// Litres a day the grow rooms' humidifiers draw, as the air's last step worked out.
    pub humidifier_l_day: f64,
}

impl Default for FarmClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FarmClock {
    pub fn new() -> Self {
        Self {
            id: NEXT_CLOCK.fetch_add(1, Ordering::Relaxed),
            areas: Vec::new(),
            by_name: HashMap::new(),
            air_owed: 0.0,
            farm_t: 0.0,
            look_frames: 0,
            humidifier_l_day: 0.0,
        }
    }

    /// The area `name`'s index, its clock started with `owed` game seconds if it is new, on the
    /// turn holding the fewest crops (the lowest such slot on a tie), so the turns stay even as
    /// areas are planted.
    pub fn index_of(&mut self, name: &str, owed: f64, crops: usize) -> u32 {
        if let Some(i) = self.by_name.get(name) {
            return *i;
        }
        let mut load = [0usize; TURNS as usize];
        for a in &self.areas {
            load[a.turn as usize] += a.crops.max(1);
        }
        let turn = (1..TURNS).min_by_key(|t| load[*t as usize]).unwrap_or(1);
        let i = self.areas.len() as u32;
        self.areas.push(AreaClock { name: name.to_string(), owed, turn, crops, irrigation_l_day: 0.0, fresh: true });
        self.by_name.insert(name.to_string(), i);
        i
    }

    /// The index of a known area.
    pub fn find(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// A frame of `game_dt` game seconds went by: every area and the air owe it.
    pub fn pass(&mut self, game_dt: f64) {
        let dt = game_dt.max(0.0);
        for a in &mut self.areas {
            a.owed += dt;
        }
        self.air_owed += dt;
        self.farm_t += dt;
    }

    /// A save was loaded: every area and the air are at now, and every area advances on the next
    /// frame (`AreaClock::fresh`), so what it draws and breathes is the loaded crops'.
    pub fn restart(&mut self) {
        for a in &mut self.areas {
            a.owed = 0.0;
            a.fresh = true;
            a.irrigation_l_day = 0.0;
        }
        self.air_owed = 0.0;
    }

    /// Mark every crop that has no mark (sown, spawned or loaded since the last tick), starting
    /// the clock of any area not seen before with `owed` (this frame's step: it was there for it,
    /// the way every crop's first tick always stepped it). With `recheck` (this clock's first
    /// tick, or a save just loaded) a crop marked by another clock is marked again too: only one
    /// farm advances a world. Every frame the query finds nothing to do in a few microseconds
    /// (hecs skips the marked crops by their component set, not one by one).
    pub fn mark_new(&mut self, world: &mut hecs::World, owed: f64, recheck: bool) {
        let mut fresh: Vec<(hecs::Entity, String)> = world
            .query::<hecs::Without<&CropInstance, &FarmArea>>()
            .iter()
            .map(|(e, c)| (e, c.tower_id.clone().unwrap_or_default()))
            .collect();
        let id = self.id;
        if recheck {
            fresh.extend(
                world
                    .query::<(&CropInstance, &FarmArea)>()
                    .iter()
                    .filter(|(_, (_, m))| m.clock != id)
                    .map(|(e, (c, _))| (e, c.tower_id.clone().unwrap_or_default())),
            );
        }
        if fresh.is_empty() {
            return;
        }
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, name) in &fresh {
            *count.entry(name.as_str()).or_default() += 1;
        }
        let new: Vec<(String, usize)> = count.into_iter().map(|(n, c)| (n.to_string(), c)).collect();
        for (name, crops) in new {
            self.index_of(&name, owed, crops);
        }
        for (e, name) in fresh {
            let index = self.by_name[&name];
            let _ = world.insert_one(e, FarmArea { clock: id, index });
        }
    }

    /// The areas a frame's `due` brings up, by index, and every area not advanced yet
    /// (`AreaClock::fresh`), leaving out `skip` (advanced already this tick, by a command).
    pub fn due_areas(&self, due: &Due, skip: &[bool]) -> Vec<u32> {
        (0..self.areas.len() as u32)
            .filter(|i| !skip.get(*i as usize).copied().unwrap_or(false))
            .filter(|i| {
                let a = &self.areas[*i as usize];
                due.all || due.turns[a.turn as usize] || a.fresh
            })
            .collect()
    }

    /// The areas more than `within_s` behind now (a reader's ask).
    pub fn stale(&self, within_s: f64) -> Vec<u32> {
        (0..self.areas.len() as u32).filter(|i| self.areas[*i as usize].owed > within_s).collect()
    }

    /// `areas` grouped by the step each owes, longest first: one run of the farm's body takes one
    /// step for all of its areas. Areas that advanced together owe the same to the bit (the same
    /// frames added to the same zero), so a turn is one group.
    pub fn groups(&self, areas: &[u32]) -> Vec<(f64, Vec<u32>)> {
        let mut by: BTreeMap<u64, Vec<u32>> = BTreeMap::new();
        for i in areas {
            by.entry(self.areas[*i as usize].owed.to_bits()).or_default().push(*i);
        }
        let mut out: Vec<(f64, Vec<u32>)> = by.into_iter().map(|(b, v)| (f64::from_bits(b), v)).collect();
        out.sort_by(|a, b| b.0.total_cmp(&a.0));
        out
    }

    /// What every area's irrigation draws together, litres a day, with the humidifiers'.
    pub fn irrigation_l_day(&self) -> f64 {
        self.areas.iter().map(|a| f64::from(a.irrigation_l_day)).sum::<f64>() + self.humidifier_l_day
    }
}

/// What one run of the farm's body covers (FarmingSystem::run): which crops and areas it advances
/// and by how much, whether the air steps with it, whether it takes the player's commands.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Game seconds each of its areas advances.
    pub step_s: f64,
    /// Game seconds the air steps, when it steps in this run.
    pub air_s: Option<f64>,
    /// The commands run: the player's requests, and no area advanced.
    pub commands: bool,
    /// None: every crop and every area, the whole farm (as every tick before rung 2). Some: only
    /// these areas, by index (this clock's marks) and by name (the per-area maps).
    only: Option<Only>,
}

#[derive(Debug, Clone)]
struct Only {
    clock: u32,
    by_index: Vec<bool>,
    names: HashSet<String>,
}

impl Scope {
    /// The whole farm, `step_s` game seconds, the air `air_s` if it steps.
    pub fn all(step_s: f64, air_s: Option<f64>) -> Self {
        Self { step_s, air_s, commands: false, only: None }
    }

    /// The player's commands, nothing advanced.
    pub fn commands(clock: &FarmClock) -> Self {
        Self { step_s: 0.0, air_s: None, commands: true, only: Some(Only { clock: clock.id, by_index: Vec::new(), names: HashSet::new() }) }
    }

    /// `areas` of `clock`, `step_s` game seconds, the air `air_s` if it steps.
    pub fn areas(clock: &FarmClock, areas: &[u32], step_s: f64, air_s: Option<f64>) -> Self {
        let mut by_index = vec![false; clock.areas.len()];
        let mut names = HashSet::new();
        for i in areas {
            by_index[*i as usize] = true;
            names.insert(clock.areas[*i as usize].name.clone());
        }
        Self { step_s, air_s, commands: false, only: Some(Only { clock: clock.id, by_index, names }) }
    }

    /// The whole farm?
    pub fn every(&self) -> bool {
        self.only.is_none()
    }

    /// Does this run advance any area?
    pub fn advances(&self) -> bool {
        self.only.as_ref().map_or(true, |o| !o.names.is_empty())
    }

    /// Does this run advance a crop with this mark? Every crop in the whole farm; else only one
    /// this clock marked in one of its areas (a crop not marked yet, sown or spawned since the
    /// tick began, waits for its mark).
    pub fn crop(&self, mark: Option<&FarmArea>) -> bool {
        match &self.only {
            None => true,
            Some(o) => mark.is_some_and(|m| m.clock == o.clock && o.by_index.get(m.index as usize).copied().unwrap_or(false)),
        }
    }

    /// Does this run advance the area named `name` (its pests, weeds, soil pH, notices)?
    pub fn area(&self, name: &str) -> bool {
        match &self.only {
            None => true,
            Some(o) => o.names.contains(name),
        }
    }
}

/// The crops' breath and gas exchange by area, as each area's last advance worked out (litres a
/// day of water; grams a day of CO2 taken up and O2 given out; grams an hour of a fungus's CO2):
/// what the air's own turn steps the rooms on, so the air between two advances of an area runs on
/// that area's last step's average, as the air of one tick always ran on the tick's.
#[derive(Debug, Clone, Default)]
pub struct Held {
    pub breathed: HashMap<String, f64>,
    pub photo: HashMap<String, (f64, f64)>,
    pub fungi: HashMap<String, f64>,
}

impl Held {
    /// Keep what a run of `scope` worked out: the whole farm's replaces everything; a slice's
    /// replaces only its own areas' (an area of the slice with no watered crop now has none).
    pub fn update(&mut self, scope: &Scope, breathed: &HashMap<String, f64>, photo: &HashMap<String, (f64, f64)>, fungi: &HashMap<String, f64>) {
        if scope.every() {
            self.breathed = breathed.clone();
            self.photo = photo.clone();
            self.fungi = fungi.clone();
            return;
        }
        if !scope.advances() {
            return;
        }
        self.breathed.retain(|a, _| !scope.area(a));
        self.photo.retain(|a, _| !scope.area(a));
        self.fungi.retain(|a, _| !scope.area(a));
        self.breathed.extend(breathed.iter().map(|(k, v)| (k.clone(), *v)));
        self.photo.extend(photo.iter().map(|(k, v)| (k.clone(), *v)));
        self.fungi.extend(fungi.iter().map(|(k, v)| (k.clone(), *v)));
    }
}
