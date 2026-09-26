//! Crops picked over a season, not harvested once (2026-09-26, gardening
//! depth). The crops, every number and its source live in
//! data/garden/harvest_windows.ron; this file is the model.
//!
//! plants.csv `growth_days` is the days to a crop's FIRST ripe harvest and
//! its yield is the whole season's, per plant (data/garden/yields.ron). Until
//! this rung every crop was harvested once at growth_days and the unit
//! emptied, so a tomato, a pepper or a cut herb delivered its whole season at
//! its first ripe fruit, and a unit replanted back to back yielded a season
//! every growth_days (a tower basil about twelve a year).
//!
//! THE LOOP:
//!
//! 1. A crop the data lists as `Picked` gets a `CropPicking` the first time
//!    it is seen ripe (`Picking::step`), and from then its garden clock counts
//!    `days_ripe`. Its season is split into `Plan::picks` equal shares; share
//!    `k` comes ripe `k x every_days` after the plant ripened, so the first is
//!    ready the day it ripens.
//! 2. Harvest on a ripe picked plant takes the shares that are ready
//!    (`take`); pressing it again before the next comes ripe gives nothing.
//!    Each share is `1 / picks` of the plant's season yield, rolled ONCE for
//!    the plant (`CropPicking::roll`) and scaled by season health and fruit set
//!    exactly as a once harvest is, and whole items come out with the fraction
//!    carried (`pick_items`), so a plant picked every time gives its season
//!    yield and never more.
//! 3. The plant is spent after its last share (`spent`), and leaves its unit
//!    empty the way a once harvest does (farming/mod.rs `vacate_unit`). The
//!    player can also clear a plant early (the crop card's Clear button, the
//!    "clear_crop_request" channel).
//! 4. A crop harvested `Once` behaves as before: one roll, one harvest, the
//!    unit emptied.
//!
//! TWO MODES (the house rule for a deep system): "garden_picking_realistic",
//! from Settings. Forgiving (false, the default and the absent case): ripe
//! produce waits on the plant, a harvest takes every share that has come ripe
//! since the last one, and the plant is spent only when its last share has
//! been picked. Realistic: a share is at its best for one interval and passes
//! over if it is not picked before the next comes ripe, and the plant is spent
//! when its window ends. The sources for why are in harvest_windows.ron.
//!
//! SEED. A survival harvest returns seed in proportion to what it harvested:
//! `SEEDS_PER_FULL_HARVEST` for a full season (every share, full health, full
//! fruit set), less for a smaller one, none from a crop that set no fruit
//! (`seed_count`). Until this rung every harvest returned 2 seeds whatever it
//! gave, so a zucchini nothing pollinated still gave seed.
//!
//! The window runs on the farming tick's garden days (the growth speed
//! setting applies, light does not: a plant bears through the night) and not
//! during the offline catch-up (save_load.rs), so produce the player could not
//! have picked while away does not pass over.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::ecs::components::{CropInstance, CropPicking, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;

/// The shipped copy, so a bare exe with no data folder still has it.
pub const HARVEST_WINDOWS_RON: &str = include_str!("../../../data/garden/harvest_windows.ron");

/// DataStore key of the loaded data (registered by the main loop).
pub const DATA_KEY: &str = "garden_harvest_windows";
/// DataStore key of the mode, a `Mutex<bool>`: true is Realistic. Absent =
/// Forgiving.
pub const MODE_KEY: &str = "garden_picking_realistic";
/// DataStore key of the crop card's Clear button, a `Mutex<Option<u64>>`
/// holding a crop entity's bits.
pub const CLEAR_KEY: &str = "clear_crop_request";

/// Seed items a whole season's harvest returns in survival (the saved-seed
/// loop, "plant one, get two back"). A GAME RULE, not a seed yield: one seed
/// item sows a whole unit, and a real tomato's fruit holds hundreds of seeds.
/// What is cited is the proportion: seed forms only in flowers that were
/// pollinated and fruit that set (data/garden/pollination.ron, "A crop whose
/// harvest is a fruit or a seed only makes it from a pollinated flower"), so a
/// harvest returns seed in step with what it harvested.
pub const SEEDS_PER_FULL_HARVEST: f64 = 2.0;

/// A whisker of garden days, so a pick due at exactly `k x every_days` is not
/// missed by the float sum of many ticks.
const DUE_EPS: f64 = 1e-6;

// -- Data: data/garden/harvest_windows.ron -------------------------------------------

/// How a crop is harvested (harvest_windows.ron `harvest`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Harvest {
    /// Harvested whole when ripe; the unit is then empty.
    Once,
    /// Once ripe, picked every `every_days` for `window_days` garden days.
    Picked { window_days: f64, every_days: f64 },
}

/// One crop's row.
#[derive(Debug, Clone, Deserialize)]
pub struct CropHarvest {
    pub plant: String,
    pub harvest: Harvest,
}

/// What data/garden/harvest_windows.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct HarvestWindows {
    pub crops: Vec<CropHarvest>,
    /// `crops` by plant id, built by `parse`.
    #[serde(skip)]
    index: HashMap<String, Harvest>,
}

/// A picked crop's season: how many picks, and how far apart (garden days).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub picks: u32,
    pub every_days: f64,
}

impl Plan {
    /// The window the picks span, garden days.
    pub fn window_days(&self) -> f64 {
        f64::from(self.picks) * self.every_days
    }
}

impl HarvestWindows {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d: HarvestWindows = ron::from_str(text).map_err(|e| e.to_string())?;
        for c in &d.crops {
            if let Harvest::Picked { window_days, every_days } = c.harvest {
                if !(every_days.is_finite() && every_days > 0.0 && window_days.is_finite() && window_days >= every_days) {
                    return Err(format!("{}: a picked crop needs 0 < every_days <= window_days", c.plant));
                }
            }
            if d.index.insert(c.plant.clone(), c.harvest).is_some() {
                return Err(format!("{} is listed twice", c.plant));
            }
        }
        Ok(d)
    }

    /// The data folder's copy first (so it can be modded), the shipped copy
    /// if that is missing or does not parse. The shipped copy always parses:
    /// a test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("harvest_windows.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::shipped().clone()
    }

    /// The shipped copy, parsed once (for the pure models, such as
    /// self_sufficiency's food supply, that have no DataStore).
    pub fn shipped() -> &'static Self {
        static SHIPPED: OnceLock<HarvestWindows> = OnceLock::new();
        SHIPPED.get_or_init(|| Self::parse(HARVEST_WINDOWS_RON).expect("the shipped data/garden/harvest_windows.ron parses"))
    }

    /// How this plant is harvested; a plant not listed is harvested once.
    pub fn harvest(&self, plant: &str) -> Harvest {
        self.index.get(plant).copied().unwrap_or(Harvest::Once)
    }

    /// A picked plant's season plan; None for one harvested once.
    pub fn plan(&self, plant: &str) -> Option<Plan> {
        match self.harvest(plant) {
            Harvest::Once => None,
            Harvest::Picked { window_days, every_days } => {
                Some(Plan { picks: ((window_days / every_days).round() as u32).max(1), every_days })
            }
        }
    }

    /// Garden days one season of this plant holds its unit: its growth days,
    /// plus its picking window if it is picked. A food model that crops a
    /// unit back to back gets one season's harvest per this many days, not
    /// per `growth_days`.
    pub fn season_days(&self, plant: &str, growth_days: f64) -> f64 {
        growth_days.max(0.0) + self.plan(plant).map_or(0.0, |p| p.window_days())
    }
}

// -- The model (pure) ------------------------------------------------------------------

/// The share that is ripe now, 0-based: `floor(days_ripe / every_days)`;
/// `plan.picks` or more means the window is over.
pub fn due(rec: &CropPicking, plan: &Plan) -> u32 {
    let k = (rec.days_ripe.max(0.0) / plan.every_days + DUE_EPS).floor();
    if k >= f64::from(plan.picks) {
        plan.picks
    } else {
        k as u32
    }
}

/// How many shares a harvest would take now (0: nothing is ready). Forgiving:
/// every share from the first not yet taken to the one ripe now. Realistic:
/// only the one ripe now, if it has not been taken.
pub fn ready(rec: &CropPicking, plan: &Plan, realistic: bool) -> u32 {
    let d = due(rec, plan);
    if realistic {
        u32::from(d < plan.picks && rec.next_pick <= d)
    } else {
        let last = d.min(plan.picks.saturating_sub(1));
        if rec.next_pick <= last {
            last - rec.next_pick + 1
        } else {
            0
        }
    }
}

/// Take what is ready: returns the shares taken and moves the plant on.
pub fn take(rec: &mut CropPicking, plan: &Plan, realistic: bool) -> u32 {
    let n = ready(rec, plan, realistic);
    if n > 0 {
        rec.next_pick = due(rec, plan).min(plan.picks.saturating_sub(1)) + 1;
        rec.taken += n;
    }
    n
}

/// The first share not taken and not passed over: in Realistic mode the
/// shares before the ripe one that were never picked have passed.
fn frontier(rec: &CropPicking, plan: &Plan, realistic: bool) -> u32 {
    if realistic {
        rec.next_pick.max(due(rec, plan))
    } else {
        rec.next_pick
    }
    .min(plan.picks)
}

/// Shares still to come or ready, of `plan.picks`.
pub fn left(rec: &CropPicking, plan: &Plan, realistic: bool) -> u32 {
    plan.picks - frontier(rec, plan, realistic)
}

/// Shares that passed over unpicked (Realistic only; Forgiving loses none).
pub fn passed_over(rec: &CropPicking, plan: &Plan, realistic: bool) -> u32 {
    frontier(rec, plan, realistic).saturating_sub(rec.taken)
}

/// Is the plant done bearing? Every share taken or passed over; in Realistic
/// mode also when its window is over.
pub fn spent(rec: &CropPicking, plan: &Plan, realistic: bool) -> bool {
    rec.next_pick >= plan.picks || (realistic && due(rec, plan) >= plan.picks)
}

/// Garden days until the next share comes ripe (0 when one is ready).
pub fn next_in(rec: &CropPicking, plan: &Plan, realistic: bool) -> f64 {
    if ready(rec, plan, realistic) > 0 {
        return 0.0;
    }
    let next = rec.next_pick.max(due(rec, plan) + 1);
    (f64::from(next) * plan.every_days - rec.days_ripe).max(0.0)
}

/// A plant's season, in (fractional) items: its roll in the plants.csv
/// yield range, scaled by season health x fruit set. Per plant; the caller
/// passes the range already times the plants in the unit.
pub fn season_items(ymin: f32, ymax: f32, roll: f32, scale: f32) -> f64 {
    let (lo, hi) = (f64::from(ymin), f64::from(ymax));
    (lo + f64::from(roll) * (hi - lo)) * f64::from(scale.clamp(0.0, 1.0))
}

/// Whole items one take hands over: `shares` of `picks` of the season, on top
/// of what the plant was carrying. Whole items come out and the fraction is
/// carried to the next pick, so small picks (a strawberry plant's daily
/// berries are 0.01 of an item) add up instead of rounding to nothing. The
/// plant's LAST take (`ends`) rounds the remainder probabilistically (+1 with
/// the fraction's probability, `round` a uniform draw in [0, 1)), so the
/// season's expected total is exactly `season` and its total is at most one
/// item over: a crop harvested once is the one-share case, the same rounding
/// `farming::harvest_quantity` does.
pub fn pick_items(carry: &mut f64, season: f64, shares: u32, picks: u32, ends: bool, round: f64) -> u32 {
    if picks == 0 {
        return 0;
    }
    let got = (*carry + season.max(0.0) * f64::from(shares) / f64::from(picks)).max(0.0);
    let whole = got.floor();
    let frac = got - whole;
    if ends {
        *carry = 0.0;
        whole as u32 + u32::from(round < frac)
    } else {
        *carry = frac;
        whole as u32
    }
}

/// Seed items a survival harvest returns: `SEEDS_PER_FULL_HARVEST` times the
/// share of a full season this take harvested (`harvested` of `full`, both in
/// items, the fractional amounts before rounding), rounded probabilistically.
/// A pick of one share in 36 returns a seed now and then; a crop that set no
/// fruit returns none.
pub fn seed_count(harvested: f64, full: f64, round: f64) -> u32 {
    if !(full > 0.0) {
        return 0;
    }
    let s = (SEEDS_PER_FULL_HARVEST * harvested / full).max(0.0);
    s.floor() as u32 + u32::from(round < s.fract())
}

// -- The farming tick's part -------------------------------------------------------------

/// What one press of Harvest took from a ripe crop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Taken {
    /// Whole items of its harvest item.
    pub items: u32,
    /// Seed items to return in survival.
    pub seeds: u32,
    /// The plant is done: harvested once, or its last share picked. The
    /// caller empties the unit.
    pub ends_plant: bool,
    /// The first take from this plant (a crop harvested once is always its
    /// first): the harvest's stressed-crop notice counts a plant once.
    pub first: bool,
    /// It is a picked crop (for the "finished bearing" notice).
    pub picked: bool,
}

/// Why a crop leaves its unit without a harvest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaving {
    /// Realistic mode: its window ended.
    Spent,
    /// The player cleared it.
    Cleared,
}

/// Is the mode Realistic? Absent = Forgiving.
pub fn realistic(data: &DataStore) -> bool {
    data.get::<std::sync::Mutex<bool>>(MODE_KEY).and_then(|m| m.lock().ok().map(|v| *v)).unwrap_or(false)
}

/// Register the data and the two channels (the main loop, at boot).
pub fn register(data_store: &mut DataStore) {
    data_store.insert(DATA_KEY, HarvestWindows::load());
    data_store.insert(MODE_KEY, std::sync::Mutex::new(false));
    data_store.insert(CLEAR_KEY, std::sync::Mutex::new(Option::<u64>::None));
}

/// The Garden panel's side, for the main loop each frame: the Settings mode
/// and the crop the player chose to clear.
pub fn publish(data: &DataStore, realistic: bool, clear: Option<u64>) {
    if let Some(Ok(mut v)) = data.get::<std::sync::Mutex<bool>>(MODE_KEY).map(|m| m.lock()) {
        *v = realistic;
    }
    if let (Some(bits), Some(m)) = (clear, data.get::<std::sync::Mutex<Option<u64>>>(CLEAR_KEY)) {
        if let Ok(mut v) = m.lock() {
            *v = Some(bits);
        }
    }
}

/// Is this crop at its plant's last (ripe) stage?
fn ripe(crop: &CropInstance, plants: Option<&super::PlantRegistry>) -> bool {
    crop.growth_stage != STAGE_DEAD
        && plants.and_then(|r| r.get(&crop.crop_def_id)).map_or(false, |d| crop.growth_stage == d.last_stage())
}

/// FarmingSystem's picking state: the data (read on the first tick; a
/// "garden_harvest_windows" entry in the DataStore wins over it) and the
/// grow areas told that produce passed over.
#[derive(Debug, Default)]
pub struct Picking {
    data: Option<HarvestWindows>,
    told_over: HashSet<String>,
}

impl Picking {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the data file on first use, unless the DataStore has it.
    fn ensure_loaded(&mut self, data: &DataStore) {
        if data.get::<HarvestWindows>(DATA_KEY).is_none() && self.data.is_none() {
            self.data = Some(HarvestWindows::load());
        }
    }

    /// The DataStore's data, else this system's own copy (after `ensure_loaded`).
    fn windows<'a>(own: &'a Option<HarvestWindows>, data: &'a DataStore) -> &'a HarvestWindows {
        data.get::<HarvestWindows>(DATA_KEY).or(own.as_ref()).unwrap_or_else(|| HarvestWindows::shipped())
    }

    /// One farming tick of `days` garden days: the Clear request, then every
    /// ripe picked plant's clock (a first-seen one gets its record), then, in
    /// Realistic mode, the one notice per area whose produce passed over.
    /// Returns the crops that leave their unit now; the caller empties each
    /// unit (farming/mod.rs `vacate_unit`).
    pub fn step(&mut self, world: &mut hecs::World, data: &DataStore, days: f64) -> Vec<(hecs::Entity, Leaving)> {
        let mut out = Vec::new();
        let clear = data
            .get::<std::sync::Mutex<Option<u64>>>(CLEAR_KEY)
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()))
            .and_then(hecs::Entity::from_bits)
            .filter(|e| world.get::<&CropInstance>(*e).is_ok());
        if let Some(e) = clear {
            out.push((e, Leaving::Cleared));
        }
        let realistic = realistic(data);
        if !realistic {
            self.told_over.clear();
        }
        let plants = data.get::<super::PlantRegistry>("plant_registry");
        self.ensure_loaded(data);
        let windows = Self::windows(&self.data, data);
        let mut fresh: Vec<hecs::Entity> = Vec::new();
        let mut over_areas: Vec<String> = Vec::new();
        for (e, (crop, rec)) in world.query_mut::<(&CropInstance, Option<&mut CropPicking>)>() {
            let Some(plan) = windows.plan(&crop.crop_def_id) else { continue };
            if !ripe(crop, plants) || clear == Some(e) {
                continue;
            }
            let Some(rec) = rec else {
                fresh.push(e);
                continue;
            };
            let before = passed_over(rec, &plan, realistic);
            if days > 0.0 {
                rec.days_ripe += days;
            }
            if realistic && passed_over(rec, &plan, realistic) > before {
                over_areas.push(crop.tower_id.clone().unwrap_or_default());
            }
            if realistic && spent(rec, &plan, realistic) {
                out.push((e, Leaving::Spent));
            }
        }
        for e in fresh {
            let _ = world.insert_one(e, CropPicking::default());
        }
        over_areas.sort();
        over_areas.dedup();
        let new: Vec<String> = over_areas.into_iter().filter(|a| self.told_over.insert(a.clone())).collect();
        if !new.is_empty() {
            super::push_notice(data, over_notice(&new));
        }
        out
    }

    /// One press of Harvest on a ripe crop whose plant yields `per_plant`
    /// items (plants.csv yield_min, yield_max) in a unit of `plants` plants, at
    /// `scale` = season health x fruit set. None when a picked plant has
    /// nothing ready (the press gives nothing). A crop harvested once gives
    /// its season (`farming::harvest_quantity`, as before this rung) and ends.
    #[allow(clippy::too_many_arguments)]
    pub fn harvest(
        &mut self,
        world: &mut hecs::World,
        data: &DataStore,
        entity: hecs::Entity,
        plant_id: &str,
        per_plant: (f32, f32),
        plants: u32,
        scale: f32,
    ) -> Option<Taken> {
        let n = plants.max(1) as f32;
        let (ymin, ymax) = (per_plant.0 * n, per_plant.1 * n);
        let full = (f64::from(ymin) + f64::from(ymax)) / 2.0;
        let realistic = realistic(data);
        self.ensure_loaded(data);
        let Some(plan) = Self::windows(&self.data, data).plan(plant_id) else {
            let roll = rand::random::<f32>();
            let items = super::harvest_quantity(ymin, ymax, scale, roll, rand::random::<f32>());
            let seeds = seed_count(season_items(ymin, ymax, roll, scale), full, rand::random::<f64>());
            return Some(Taken { items, seeds, ends_plant: true, first: true, picked: false });
        };
        let mut rec = world.get::<&CropPicking>(entity).map(|r| (*r).clone()).unwrap_or_default();
        let first = rec.taken == 0;
        let shares = take(&mut rec, &plan, realistic);
        if shares == 0 {
            return None;
        }
        let roll = *rec.roll.get_or_insert_with(rand::random::<f32>);
        let season = season_items(ymin, ymax, roll, scale);
        let ends = spent(&rec, &plan, realistic);
        let items = pick_items(&mut rec.carry, season, shares, plan.picks, ends, rand::random::<f64>());
        let harvested = season * f64::from(shares) / f64::from(plan.picks);
        let seeds = seed_count(harvested, full, rand::random::<f64>());
        let had = match world.get::<&mut CropPicking>(entity) {
            Ok(mut r) => {
                *r = rec.clone();
                true
            }
            Err(_) => false,
        };
        if !had {
            let _ = world.insert_one(entity, rec);
        }
        Some(Taken { items, seeds, ends_plant: ends, first, picked: true })
    }
}

/// The one line said the first time ripe produce passes over in an area
/// (Realistic mode).
fn over_notice(areas: &[String]) -> String {
    let place = match areas {
        [one] if one.is_empty() => "your hand-planted crops".to_string(),
        [one] => one.replace('_', " "),
        many => format!("{} grow areas", many.len()),
    };
    format!(
        "Ripe produce went past its best unpicked in {place}. Picked crops (tomatoes, peppers, \
         cut herbs...) give a share of their season at each pick, and in Realistic picking a \
         pick not taken before the next comes ripe is lost. Harvest them often, or choose \
         Forgiving picking in Settings."
    )
}

/// "2.5 garden days", "1 garden day", "less than a garden day".
fn days_words(days: f64) -> String {
    if days < 0.95 {
        return "less than a garden day".to_string();
    }
    let s = format!("{days:.1}");
    match s.strip_suffix(".0").unwrap_or(&s) {
        "1" => "1 garden day".to_string(),
        t => format!("{t} garden days"),
    }
}

/// The crop card's "Picking" row for a picked crop: before it ripens, how it
/// will be picked; once ripe, what is ready, what is left and when the next
/// share comes, and in Realistic mode what passed over.
pub fn card_row(plan: &Plan, rec: Option<&CropPicking>, ripe: bool, realistic: bool) -> String {
    if !ripe {
        return format!(
            "picked once ripe: {} picks, one every {}",
            plan.picks,
            days_words(plan.every_days)
        );
    }
    let fresh = CropPicking::default();
    let rec = rec.unwrap_or(&fresh);
    let (now, left, total) = (ready(rec, plan, realistic), left(rec, plan, realistic), plan.picks);
    let mut s = match now {
        0 => format!("{left} of {total} picks left, next in {}", days_words(next_in(rec, plan, realistic))),
        1 => format!("a pick is ready now, {left} of {total} left"),
        n => format!("{n} picks are ready now, {left} of {total} left"),
    };
    let over = passed_over(rec, plan, realistic);
    if over > 0 {
        s.push_str(&format!(", {over} went past their best unpicked"));
    }
    s
}

/// The Garden panel's view, built once a frame by the main loop's crop
/// bridge: each picked crop's card row, and whether a pick is ready (which is
/// what "ready" and the bulk "Harvest N ready" button mean for a picked crop).
#[derive(Debug, Default)]
pub struct GuiView {
    rows: HashMap<hecs::Entity, (String, Option<bool>)>,
}

impl GuiView {
    pub fn new(world: &hecs::World, data: &DataStore) -> Self {
        let mut v = Self::default();
        let windows = data.get::<HarvestWindows>(DATA_KEY).unwrap_or_else(|| HarvestWindows::shipped());
        let plants = data.get::<super::PlantRegistry>("plant_registry");
        let realistic = realistic(data);
        for (e, (crop, rec)) in world.query::<(&CropInstance, Option<&CropPicking>)>().iter() {
            let Some(plan) = windows.plan(&crop.crop_def_id) else { continue };
            if crop.growth_stage == STAGE_DEAD {
                continue;
            }
            let is_ripe = ripe(crop, plants);
            let fresh = CropPicking::default();
            let now = is_ripe.then(|| ready(rec.unwrap_or(&fresh), &plan, realistic) > 0);
            v.rows.insert(e, (card_row(&plan, rec, is_ripe, realistic), now));
        }
        v
    }

    /// This crop's "Picking" row ("" for a crop harvested once).
    pub fn row(&self, e: hecs::Entity) -> String {
        self.rows.get(&e).map(|(r, _)| r.clone()).unwrap_or_default()
    }

    /// Is this crop ready to harvest? For a ripe picked crop, whether a pick
    /// is ready now; for any other crop, `mature` as the bridge found it.
    pub fn ready(&self, e: hecs::Entity, mature: bool) -> bool {
        self.rows.get(&e).and_then(|(_, r)| *r).unwrap_or(mature)
    }
}
