//! Pollination decides how much fruit and seed a crop sets (2026-09-26,
//! gardening depth).
//!
//! Until this rung every crop yielded the same whether or not anything had
//! pollinated it (the 2026-09-25 gap survey: "No pests, disease, weeds,
//! pollination or rotation"). The crops, every number and its source live in
//! data/garden/pollination.ron; this file is the model.
//!
//! THE LOOP:
//!
//! 1. A crop listed in pollination.ron that cannot set fruit on its own
//!    (`CropPollinationDef::needs_help`) and grows indoors keeps a record of
//!    its season on its entity, a `CropPollination`: the garden days it has
//!    spent in its flowering stages and how many of them its flowers were
//!    pollinated (`record_step`). An outdoor field (`farming::is_field_area`)
//!    needs none: the wind and wild insects pollinate it.
//! 2. Indoors a flower is pollinated while a bumblebee hive reaches its grow
//!    area (`hive_cover`; bees do nothing for a wind-pollinated crop) or while
//!    the player's last hand pollination of that area still covers it: the
//!    "pollinate_request" channel (the Garden panel's Hand-pollinate button)
//!    gives every crop there that needs help its `hand_every_days`.
//! 3. The player is told once, when an indoor area's crops start flowering
//!    with nothing to pollinate them, what they will set left alone and how
//!    to pollinate them by hand (`flowering_notice`).
//! 4. At harvest the crop's fruit set (`fruit_set`) is ONE multiplier on its
//!    yield (`Pollination::harvest_set`, the single hook in the harvest path):
//!    its unhelped share plus the rest in proportion to the pollinated days.
//!
//! TWO MODES (the house rule for a deep system): the "garden_pollination_on"
//! channel, from Settings. Off, every crop sets fully and nothing is shown or
//! said; On (the default, and the absent case) is the cited model.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Deserialize;

use super::lighting::GrowPlot;
use crate::ecs::components::{CropInstance, CropPollination, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;

/// The shipped copy, so a bare exe with no data folder still has it.
pub const POLLINATION_RON: &str = include_str!("../../../data/garden/pollination.ron");

/// DataStore key of the loaded data (registered by the main loop).
pub const DATA_KEY: &str = "garden_pollination";
/// DataStore key of the mode, a `Mutex<bool>`: true is On. Absent = On.
pub const MODE_KEY: &str = "garden_pollination_on";
/// DataStore key of the Hand-pollinate request, a `Mutex<Option<String>>`
/// holding a grow-area tag (a crop's `tower_id`; "" is the hand-planted crops).
pub const REQUEST_KEY: &str = "pollinate_request";

// -- Data: data/garden/pollination.ron ----------------------------------------------

/// How a crop's flowers are pollinated (pollination.ron `by`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PollinatedBy {
    /// Inside the flower, before or as it opens: nothing is needed.
    SelfPollinating,
    /// Self-fertile, but the pollen only falls when the flower is shaken:
    /// the wind outdoors, a bumblebee's buzz or a hand vibrator indoors.
    Vibration,
    /// The pollen must be carried to another flower.
    Insects,
    /// Pollen blown from the tassel to the silks.
    Wind,
}

impl PollinatedBy {
    /// Do bees pollinate it? Not a wind-pollinated crop.
    pub fn hive_helps(self) -> bool {
        matches!(self, Self::Vibration | Self::Insects)
    }
}

/// One crop (pollination.ron `crops`).
#[derive(Debug, Clone, Deserialize)]
pub struct CropPollinationDef {
    pub id: String,
    pub by: PollinatedBy,
    /// Its plants.csv growth stages while its flowers are open.
    #[serde(default)]
    pub flowering: Vec<String>,
    /// The share of a full crop it sets indoors with no help, 0..1.
    pub no_help: f64,
    /// Garden days one hand pollination covers.
    #[serde(default)]
    pub hand_every_days: f64,
    /// How to pollinate it by hand, told to the player.
    #[serde(default)]
    pub how: String,
}

impl CropPollinationDef {
    /// Does it set less than a full crop indoors without help?
    pub fn needs_help(&self) -> bool {
        self.by != PollinatedBy::SelfPollinating && self.no_help < 1.0
    }

    pub fn is_flowering(&self, stage: &str) -> bool {
        self.flowering.iter().any(|s| s == stage)
    }
}

/// What data/garden/pollination.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct PollinationData {
    /// The floor one bumblebee colony serves, m2.
    pub hive_cover_m2: f64,
    pub crops: Vec<CropPollinationDef>,
    /// `crops` by id, built by `parse` (the tick looks every crop up).
    #[serde(skip)]
    index: HashMap<String, usize>,
}

impl PollinationData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d: PollinationData = ron::from_str(text).map_err(|e| e.to_string())?;
        if !(d.hive_cover_m2.is_finite() && d.hive_cover_m2 > 0.0) {
            return Err("hive_cover_m2 must be positive".to_string());
        }
        for (i, c) in d.crops.iter().enumerate() {
            if !(0.0..=1.0).contains(&c.no_help) {
                return Err(format!("{}: no_help must be 0 to 1", c.id));
            }
            if c.needs_help() && (c.flowering.is_empty() || !(c.hand_every_days > 0.0)) {
                return Err(format!("{}: a crop that needs help names its flowering stages and hand_every_days", c.id));
            }
            if d.index.insert(c.id.clone(), i).is_some() {
                return Err(format!("{} is listed twice", c.id));
            }
        }
        Ok(d)
    }

    /// The data folder's copy first (so it can be modded), the shipped copy
    /// if that is missing or does not parse. The shipped copy always parses:
    /// a test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("pollination.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::parse(POLLINATION_RON).expect("the shipped data/garden/pollination.ron parses")
    }

    pub fn crop(&self, id: &str) -> Option<&CropPollinationDef> {
        self.index.get(id).and_then(|i| self.crops.get(*i))
    }

    /// How far a hive's bees reach across the floor, m: the radius of a
    /// circle of `hive_cover_m2` (17.8 m for 1,000 m2).
    pub fn hive_reach_m(&self) -> f64 {
        (self.hive_cover_m2.max(0.0) / std::f64::consts::PI).sqrt()
    }
}

// -- The model ---------------------------------------------------------------------

/// Step one crop's record `days` garden days: while it flowers, every day
/// counts as a flowering day, and as a pollinated one while a hive reaches
/// it (`hive`) or a hand pollination still covers it. The hand pollination
/// wears off either way (flowers opened after it need their own).
pub fn record_step(rec: &mut CropPollination, flowering: bool, hive: bool, days: f64) {
    if !(days > 0.0) {
        return;
    }
    if flowering {
        rec.flowering_days += days;
        rec.pollinated_days += if hive { days } else { rec.hand_days_left.clamp(0.0, days) };
    }
    rec.hand_days_left = (rec.hand_days_left - days).max(0.0);
}

/// The share of a full crop this crop sets, 0..1: all of it outdoors or for
/// a crop that needs no help, else its unhelped share plus the rest in
/// proportion to the days its flowers were pollinated. A crop with no
/// flowering on its record never flowered on the clock (spawned ripe, the
/// dev button, the offline catch-up) and is not charged for flowers it
/// never had.
pub fn fruit_set(def: &CropPollinationDef, rec: Option<&CropPollination>, outdoors: bool) -> f64 {
    if outdoors || !def.needs_help() {
        return 1.0;
    }
    let unhelped = def.no_help.clamp(0.0, 1.0);
    match rec {
        Some(r) if r.flowering_days > 0.0 => {
            let helped = (r.pollinated_days / r.flowering_days).clamp(0.0, 1.0);
            unhelped + (1.0 - unhelped) * helped
        }
        _ => 1.0,
    }
}

/// Where the bumblebee hives stand: every `PollinatorHive` at its `Transform`.
pub fn hive_positions(world: &hecs::World) -> Vec<[f32; 3]> {
    world
        .query::<(&crate::ecs::components::PollinatorHive, &crate::ecs::components::Transform)>()
        .iter()
        .map(|(_, (_, t))| t.position.to_array())
        .collect()
}

/// The grow areas the hives reach: every indoor plot whose centre is within
/// `reach_m` of a hive across the floor, by instance id and alias (a
/// tower's design id, which its crops carry). Outdoor fields have wild
/// pollinators; hand-planted crops have no place, so no hive reaches them.
pub fn hive_cover(hives: &[[f32; 3]], plots: &[GrowPlot], reach_m: f64) -> HashSet<String> {
    let mut out = HashSet::new();
    for p in plots.iter().filter(|p| !p.outdoors) {
        let reached = hives.iter().any(|h| {
            let (dx, dz) = (f64::from(p.pos[0] - h[0]), f64::from(p.pos[2] - h[2]));
            (dx * dx + dz * dz).sqrt() <= reach_m
        });
        if reached {
            out.insert(p.id.clone());
            out.extend(p.aliases.iter().cloned());
        }
    }
    out
}

/// The areas the home's hives reach now (the engine publishes "grow_plots").
fn covered_now(world: &hecs::World, data: &DataStore, d: &PollinationData) -> HashSet<String> {
    let hives = hive_positions(world);
    match data.get::<Vec<GrowPlot>>("grow_plots") {
        Some(plots) if !hives.is_empty() => hive_cover(&hives, plots, d.hive_reach_m()),
        _ => HashSet::new(),
    }
}

/// Is pollination On? Absent = On.
pub fn mode_on(data: &DataStore) -> bool {
    data.get::<std::sync::Mutex<bool>>(MODE_KEY).and_then(|m| m.lock().ok().map(|v| *v)).unwrap_or(true)
}

/// Register the data and the two channels (the main loop, at boot).
pub fn register(data_store: &mut DataStore) {
    data_store.insert(DATA_KEY, PollinationData::load());
    data_store.insert(MODE_KEY, std::sync::Mutex::new(true));
    data_store.insert(REQUEST_KEY, std::sync::Mutex::new(Option::<String>::None));
}

/// The Garden panel's side, for the main loop each frame: the Settings mode
/// and the area the player chose to hand-pollinate.
pub fn publish(data: &DataStore, on: bool, pending: Option<String>) {
    if let Some(Ok(mut v)) = data.get::<std::sync::Mutex<bool>>(MODE_KEY).map(|m| m.lock()) {
        *v = on;
    }
    if let (Some(area), Some(m)) = (pending, data.get::<std::sync::Mutex<Option<String>>>(REQUEST_KEY)) {
        if let Ok(mut v) = m.lock() {
            *v = Some(area);
        }
    }
}

// -- The farming tick's part ---------------------------------------------------------

/// FarmingSystem's pollination state: the data (read on the first tick; a
/// "garden_pollination" entry in the DataStore wins over it) and the areas
/// already told their flowers wait for a pollinator.
#[derive(Debug, Default)]
pub struct Pollination {
    data: Option<PollinationData>,
    told: HashSet<String>,
}

impl Pollination {
    pub fn new() -> Self {
        Self::default()
    }

    /// One farming tick of `days` garden days: the Hand-pollinate request,
    /// then every indoor crop's record, then the notices.
    pub fn tick(&mut self, world: &mut hecs::World, data: &DataStore, days: f64) {
        let request = data
            .get::<std::sync::Mutex<Option<String>>>(REQUEST_KEY)
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if !mode_on(data) {
            self.told.clear();
            return;
        }
        if self.data.is_none() && data.get::<PollinationData>(DATA_KEY).is_none() {
            self.data = Some(PollinationData::load());
        }
        let Some(d) = data.get::<PollinationData>(DATA_KEY).or(self.data.as_ref()) else { return };
        if let Some(area) = request {
            hand_pollinate(world, data, d, &area);
        }
        let covered = covered_now(world, data, d);
        let (waiting, flowering) = step_crops(world, d, &covered, days);
        // Told once per area while it flowers: a hand pollination wearing
        // off is not news, the Garden panel's button says it.
        self.told.retain(|a| flowering.contains(a));
        for (area, plants) in &waiting {
            if self.told.insert(area.clone()) {
                super::push_notice(data, flowering_notice(data, d, area, plants));
            }
        }
    }

    /// The fruit set this crop's harvest is multiplied by (1 with the mode
    /// Off, for a crop not in the data, or one that set fully).
    pub fn harvest_set(&self, world: &hecs::World, data: &DataStore, entity: hecs::Entity) -> f32 {
        if !mode_on(data) {
            return 1.0;
        }
        let Some(d) = data.get::<PollinationData>(DATA_KEY).or(self.data.as_ref()) else { return 1.0 };
        let Ok(crop) = world.get::<&CropInstance>(entity) else { return 1.0 };
        let Some(def) = d.crop(&crop.crop_def_id) else { return 1.0 };
        let outdoors = crop.tower_id.as_deref().map_or(false, super::is_field_area);
        let rec = world.get::<&CropPollination>(entity).ok();
        fruit_set(def, rec.as_deref(), outdoors) as f32
    }
}

/// Step every living indoor crop that needs help `days` garden days, giving
/// a record to any that has none. Returns, by area, the plants flowering with
/// nothing pollinating them, and every area with such a crop in flower.
fn step_crops(
    world: &mut hecs::World,
    d: &PollinationData,
    covered: &HashSet<String>,
    days: f64,
) -> (BTreeMap<String, BTreeSet<String>>, HashSet<String>) {
    let mut waiting: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut in_flower: HashSet<String> = HashSet::new();
    let mut fresh: Vec<(hecs::Entity, CropPollination)> = Vec::new();
    for (e, (crop, rec)) in world.query_mut::<(&CropInstance, Option<&mut CropPollination>)>() {
        let Some(def) = d.crop(&crop.crop_def_id) else { continue };
        let area = crop.tower_id.as_deref().unwrap_or("");
        if !def.needs_help() || crop.growth_stage == STAGE_DEAD || super::is_field_area(area) {
            continue;
        }
        let flowering = def.is_flowering(&crop.growth_stage);
        let hive = def.by.hive_helps() && covered.contains(area);
        let mut new = CropPollination::default();
        let had_record = rec.is_some();
        let r = match rec {
            Some(r) => r,
            None => &mut new,
        };
        if flowering {
            in_flower.insert(area.to_string());
            if !hive && r.hand_days_left <= 0.0 {
                waiting.entry(area.to_string()).or_default().insert(crop.crop_def_id.clone());
            }
        }
        record_step(r, flowering, hive, days);
        if !had_record {
            fresh.push((e, new));
        }
    }
    for (e, rec) in fresh {
        let _ = world.insert_one(e, rec);
    }
    (waiting, in_flower)
}

/// The Hand-pollinate action on one grow area: every living crop there that
/// needs help is covered for its `hand_every_days` (a crop not yet in flower
/// gains nothing: the cover wears off before its flowers open).
fn hand_pollinate(world: &mut hecs::World, data: &DataStore, d: &PollinationData, area: &str) {
    if super::is_field_area(area) {
        super::push_notice(data, "Outdoor fields are pollinated by the wind and wild insects.".to_string());
        return;
    }
    let mut done: BTreeMap<String, bool> = BTreeMap::new();
    let mut fresh: Vec<(hecs::Entity, CropPollination)> = Vec::new();
    for (e, (crop, rec)) in world.query_mut::<(&CropInstance, Option<&mut CropPollination>)>() {
        if crop.tower_id.as_deref().unwrap_or("") != area || crop.growth_stage == STAGE_DEAD {
            continue;
        }
        let Some(def) = d.crop(&crop.crop_def_id).filter(|c| c.needs_help()) else { continue };
        let cover = def.hand_every_days.max(0.0);
        match rec {
            Some(r) => r.hand_days_left = r.hand_days_left.max(cover),
            None => fresh.push((e, CropPollination { hand_days_left: cover, ..Default::default() })),
        }
        *done.entry(crop.crop_def_id.clone()).or_insert(false) |= def.is_flowering(&crop.growth_stage);
    }
    for (e, rec) in fresh {
        let _ = world.insert_one(e, rec);
    }
    let place = super::pests::place(area);
    if done.is_empty() {
        super::push_notice(data, format!("Nothing among {place} needs pollinating by hand."));
        return;
    }
    let parts: Vec<String> = done
        .keys()
        .filter_map(|id| d.crop(id))
        .map(|c| format!("{} for {} ({})", plant_name(data, &c.id), days_words(c.hand_every_days), c.how))
        .collect();
    let mut s = format!("You hand-pollinated {place}: {}. Repeat while they flower.", parts.join("; "));
    if !done.values().any(|f| *f) {
        s.push_str(" Nothing there is in flower yet, so this does nothing until it is.");
    }
    super::push_notice(data, s);
}

/// "2.5 garden days", "1 garden day".
fn days_words(days: f64) -> String {
    match trim(days).as_str() {
        "1" => "1 garden day".to_string(),
        t => format!("{t} garden days"),
    }
}

/// 2.5 -> "2.5", 3 -> "3".
fn trim(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// A plant's display name, lower case ("bell pepper"), from plants.csv.
fn plant_name(data: &DataStore, id: &str) -> String {
    data.get::<super::PlantRegistry>("plant_registry")
        .and_then(|r| r.get(id).map(|p| p.name.to_lowercase()))
        .unwrap_or_else(|| id.replace('_', " "))
}

/// "about 49% of a full crop", "no fruit".
fn share_words(share: f64) -> String {
    if share < 0.005 {
        "no fruit".to_string()
    } else {
        format!("about {:.0}% of a full crop", share * 100.0)
    }
}

/// The one line said when an indoor area's crops start flowering with
/// nothing to pollinate them: what they set left alone, and what to do.
pub fn flowering_notice(data: &DataStore, d: &PollinationData, area: &str, plants: &BTreeSet<String>) -> String {
    let defs: Vec<&CropPollinationDef> = plants.iter().filter_map(|p| d.crop(p)).collect();
    let names: Vec<String> = defs.iter().map(|c| plant_name(data, &c.id)).collect();
    let alone: Vec<String> = defs
        .iter()
        .map(|c| format!("{} sets {}", plant_name(data, &c.id), share_words(c.no_help)))
        .collect();
    let how: Vec<String> = defs
        .iter()
        .map(|c| format!("{}: {}, every {}", plant_name(data, &c.id), c.how, days_words(c.hand_every_days)))
        .collect();
    let where_ = if area.is_empty() { "your hand-planted crops".to_string() } else { area.replace('_', " ") };
    let mut s = format!(
        "The {} plants in {where_} are flowering, and indoors nothing carries their pollen. Left alone, {}. \
         Hand-pollinate them from the Garden panel while they flower ({})",
        names.join(" and "),
        alone.join("; "),
        how.join("; ")
    );
    if defs.iter().any(|c| c.by.hive_helps()) {
        s.push_str(", or place a bumblebee hive near them.");
    } else {
        s.push('.');
    }
    s
}

// -- What the Garden panel shows ----------------------------------------------------

/// Where a crop is in its season, for its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Before,
    Flowering,
    After,
}

/// The crop card's "Pollination" row.
pub fn card_row(def: &CropPollinationDef, rec: Option<&CropPollination>, outdoors: bool, hive_here: bool, phase: Phase) -> String {
    if !def.needs_help() {
        return "pollinates itself: needs no help".to_string();
    }
    if outdoors {
        let who = if def.by == PollinatedBy::Wind { "the wind" } else { "wild bees and other insects" };
        return format!("outdoors: {who}");
    }
    let hive = hive_here && def.by.hive_helps();
    let hand = rec.map_or(0.0, |r| r.hand_days_left);
    match phase {
        Phase::Before if hive => "the bumblebee hive will pollinate it when it flowers".to_string(),
        Phase::Before => {
            let who = if def.by.hive_helps() { "a hand or a bumblebee hive" } else { "a hand" };
            format!("indoors: will need {who} when it flowers ({} without)", share_words(def.no_help))
        }
        Phase::Flowering if hive => "a bumblebee hive is working the flowers".to_string(),
        Phase::Flowering if hand > 0.0 => format!("hand-pollinated, {} left", days_words(hand)),
        Phase::Flowering => {
            let expect = match rec {
                Some(r) if r.flowering_days > 0.0 => fruit_set(def, rec, false),
                _ => def.no_help,
            };
            format!("not pollinated: expect {}", share_words(expect))
        }
        Phase::After => format!("set {}", share_words(fruit_set(def, rec, false))),
    }
}

/// Every crop's "Pollination" row and the indoor areas with flowers waiting
/// for a pollinator (where the Garden panel shows its Hand-pollinate
/// button), built once a frame by the main loop's crop bridge. Empty with
/// the mode Off.
#[derive(Debug, Default)]
pub struct GuiView {
    rows: HashMap<hecs::Entity, String>,
    pub areas: Vec<String>,
}

impl GuiView {
    pub fn new(world: &hecs::World, data: &DataStore) -> Self {
        let mut v = Self::default();
        let Some(d) = data.get::<PollinationData>(DATA_KEY).filter(|_| mode_on(data)) else { return v };
        let plants = data.get::<super::PlantRegistry>("plant_registry");
        let covered = covered_now(world, data, d);
        let mut areas = BTreeSet::new();
        for (e, (crop, rec)) in world.query::<(&CropInstance, Option<&CropPollination>)>().iter() {
            let Some(def) = d.crop(&crop.crop_def_id) else { continue };
            if crop.growth_stage == STAGE_DEAD {
                continue;
            }
            let area = crop.tower_id.as_deref().unwrap_or("");
            let phase = if def.is_flowering(&crop.growth_stage) {
                Phase::Flowering
            } else {
                let stages = plants.and_then(|r| r.get(&crop.crop_def_id)).map(|p| p.stages()).unwrap_or_default();
                let at = stages.iter().position(|s| *s == crop.growth_stage);
                let opens = stages.iter().position(|s| def.is_flowering(s));
                if matches!((at, opens), (Some(a), Some(o)) if a < o) { Phase::Before } else { Phase::After }
            };
            let outdoors = super::is_field_area(area);
            let hive_here = covered.contains(area);
            let row = card_row(def, rec, outdoors, hive_here, phase);
            if phase == Phase::Flowering && !outdoors && def.needs_help() && !(hive_here && def.by.hive_helps()) && rec.map_or(0.0, |r| r.hand_days_left) <= 0.0 {
                areas.insert(area.to_string());
            }
            v.rows.insert(e, row);
        }
        v.areas = areas.into_iter().collect();
        v
    }

    /// This crop's row ("" for none: a crop the data does not list).
    pub fn row(&self, e: hecs::Entity) -> String {
        self.rows.get(&e).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Controllable, PollinatorHive, Transform};
    use crate::ecs::systems::System;
    use crate::systems::farming::gardening_tests::make_store;
    use crate::systems::farming::{FarmingSystem, PlantRegistry};
    use crate::systems::inventory::Inventory;

    fn shipped() -> PollinationData {
        PollinationData::parse(POLLINATION_RON).expect("the shipped pollination.ron parses")
    }

    /// A store at `speed` growth with the channels pollination uses and the
    /// notices, with the mode On (as absent) unless `on` is false.
    fn store(speed: f32, on: bool) -> DataStore {
        let mut data = make_store();
        data.insert("crop_growth_speed", std::sync::Mutex::new(speed));
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        data.insert(REQUEST_KEY, std::sync::Mutex::new(Option::<String>::None));
        data.insert(MODE_KEY, std::sync::Mutex::new(on));
        data
    }

    /// A crop of `plant` in `area` at `stage`, planted at game second 0.
    fn crop(plant: &str, area: &str, stage: &str) -> CropInstance {
        CropInstance {
            crop_def_id: plant.to_string(),
            growth_stage: stage.to_string(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(area.to_string()),
            tower_slot: Some(0),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }

    fn rec(world: &hecs::World, e: hecs::Entity) -> CropPollination {
        world.get::<&CropPollination>(e).map(|r| (*r).clone()).unwrap_or_default()
    }

    fn notices(data: &DataStore) -> Vec<String> {
        std::mem::take(&mut *data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
    }

    fn request(data: &DataStore, area: &str) {
        *data.get::<std::sync::Mutex<Option<String>>>(REQUEST_KEY).unwrap().lock().unwrap() = Some(area.to_string());
    }

    /// The Pollination part of the tick alone, `days` garden days.
    fn step(p: &mut Pollination, world: &mut hecs::World, data: &DataStore, days: f64) {
        p.tick(world, data, days);
    }

    /// The shipped file parses; every crop is a real plants.csv id; every
    /// flowering stage is one of that crop's own plants.csv stages (so a
    /// renamed stage cannot silently make a crop never flower); and it
    /// covers the home's fruit and seed crops (the towers' and the
    /// showcase's) and the grains. Seen red by misspelling "tomato" as
    /// "tomatoe" in the data (the id check named it), and again by naming
    /// corn's flowering stage "silking".
    #[test]
    fn the_shipped_data_names_real_plants_and_their_stages() {
        let d = shipped();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let plants = PlantRegistry::from_csv(&std::fs::read(root.join("data/plants.csv")).unwrap()).unwrap();
        for c in &d.crops {
            let p = plants.get(&c.id).unwrap_or_else(|| panic!("'{}' is not in plants.csv", c.id));
            let stages = p.stages();
            for s in &c.flowering {
                assert!(stages.contains(&s.as_str()), "{}: '{s}' is not one of its stages {stages:?}", c.id);
            }
            assert!(!c.how.is_empty() || !c.needs_help(), "{} says how to pollinate it by hand", c.id);
        }
        for id in [
            "tomato", "pepper", "eggplant", "cucumber", "zucchini", "squash", "pumpkin", "watermelon",
            "strawberry", "bean", "pea", "corn", "sunflower", "chickpea", "lentil", "soybean", "okra",
            "wheat", "rice", "barley", "oat",
        ] {
            assert!(d.crop(id).is_some(), "{id} is covered");
        }
        assert!(d.crop("lettuce").is_none(), "a leaf crop is not listed");
        assert!((d.hive_reach_m() - 17.84).abs() < 0.01, "1,000 m2 reaches 17.8 m: {}", d.hive_reach_m());
    }

    /// The model: a record steps exactly, however the days are sliced; a
    /// hand pollination covers what is left of it and then wears off; and
    /// the fruit set blends the unhelped share with the pollinated days.
    /// Seen red by letting a hand pollination cover a whole tick regardless
    /// of what was left of it (`clamp(0, days)` dropped: one 10-day step
    /// then counted 10 pollinated days from 2.5 of cover).
    #[test]
    fn the_record_counts_pollinated_flowering_days() {
        let d = shipped();
        let tomato = d.crop("tomato").unwrap();
        let mut a = CropPollination { hand_days_left: 2.5, ..Default::default() };
        record_step(&mut a, true, false, 10.0);
        let mut b = CropPollination { hand_days_left: 2.5, ..Default::default() };
        for _ in 0..1000 {
            record_step(&mut b, true, false, 0.01);
        }
        assert!((a.pollinated_days - 2.5).abs() < 1e-9 && (b.pollinated_days - 2.5).abs() < 1e-6, "{a:?} {b:?}");
        assert!((a.flowering_days - 10.0).abs() < 1e-9 && (b.flowering_days - 10.0).abs() < 1e-6);
        assert_eq!(a.hand_days_left, 0.0);
        // A quarter of the flowering helped: 0.49 + 0.51 x 0.25.
        assert!((fruit_set(tomato, Some(&a), false) - (0.49 + 0.51 * 0.25)).abs() < 1e-9);
        // Not in flower: the cover wears off, nothing is counted.
        let mut c = CropPollination { hand_days_left: 2.5, ..Default::default() };
        record_step(&mut c, false, true, 1.0);
        assert_eq!((c.flowering_days, c.pollinated_days, c.hand_days_left), (0.0, 0.0, 1.5));
        assert_eq!(fruit_set(tomato, Some(&c), false), 1.0, "never flowered: not charged");
        assert_eq!(fruit_set(tomato, None, false), 1.0);
    }

    /// An indoor tomato with no help sets its cited share, 0.49 (McGregor,
    /// Moore 1968: 4.3 against 8.8 pounds), through the real farming tick:
    /// it keeps a record while it flowers, the player is told once, and the
    /// harvest multiplier reads the record. Seen red by skipping the
    /// record step in `step_crops` (no flowering days: the harvest then
    /// read 1).
    #[test]
    fn an_indoor_tomato_with_no_help_sets_its_cited_share() {
        let data = store(100.0, true);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let e = world.spawn((crop("tomato", "ntower_3", "flower"),));
        // 100x growth, 1 s ticks: a garden day is 12 ticks.
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        let r = rec(&world, e);
        assert!(r.flowering_days > 1.0 && r.pollinated_days == 0.0, "{r:?}");
        let said = notices(&data);
        let told: Vec<&String> = said.iter().filter(|n| n.contains("indoors nothing carries their pollen")).collect();
        assert_eq!(told.len(), 1, "told once: {said:?}");
        assert!(told[0].contains("ntower 3") && told[0].contains("about 49%"), "{}", told[0]);
        let mut p = Pollination::new();
        step(&mut p, &mut world, &data, 0.0);
        assert!((p.harvest_set(&world, &data, e) - 0.49).abs() < 1e-6, "{}", p.harvest_set(&world, &data, e));
    }

    /// The harvest applies the fruit set: 600 ripe tomatoes that flowered
    /// with no help give about 0.49 of what 600 fully pollinated ones give,
    /// through the real harvest path. Averaged over 600 harvests each, so
    /// the ratio sits near 0.49 well inside the bounds whatever plants.csv
    /// says a tomato plant yields. Seen red by removing the multiplication from the harvest
    /// path in farming/mod.rs (the ratio was then 1.0).
    ///
    /// CHANGED 2026-09-26 (picking.rs): a tomato is picked over a season, so
    /// each one here has waited out its whole window (Forgiving picking) and
    /// one press takes its season, as one harvest did before.
    #[test]
    fn the_harvest_carries_the_fruit_set() {
        let mut data = store(10.0, true);
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let ripe = data.get::<PlantRegistry>("plant_registry").unwrap().get("tomato").unwrap().last_stage().to_string();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(64);
        inv.volume_capacity_l = 1.0e9;
        let player = world.spawn((inv, Controllable));
        let flowered = |pollinated: f64| CropPollination { flowering_days: 10.0, pollinated_days: pollinated, hand_days_left: 0.0 };
        let mut pick = |world: &mut hecs::World, pollinated: f64| -> u32 {
            let bits: Vec<u64> = (0..600)
                .map(|_| {
                    let waited = crate::ecs::components::CropPicking { days_ripe: 1000.0, ..Default::default() };
                    world.spawn((crop("tomato", "ntower_3", &ripe), flowered(pollinated), waited)).to_bits().into()
                })
                .collect();
            let before = world.get::<&Inventory>(player).unwrap().count_item("vegetable_tomato_0");
            *data.get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() = bits;
            sys.tick(world, 1.0, &data);
            world.get::<&Inventory>(player).unwrap().count_item("vegetable_tomato_0") - before
        };
        let full = pick(&mut world, 10.0);
        let alone = pick(&mut world, 0.0);
        assert_eq!(world.query::<&CropInstance>().iter().count(), 0, "all harvested");
        assert!(full > 300, "a full set gives a real harvest: {full}");
        let ratio = alone as f64 / full as f64;
        assert!((0.42..=0.56).contains(&ratio), "unpollinated tomatoes give about 0.49: {ratio:.3} ({alone} vs {full})");
    }

    /// Hand-pollinating an area restores a full set while it lasts, and
    /// only while: a flowering tomato hand-pollinated once is covered for
    /// its 2.5 garden days, then goes back to unhelped; pressing again
    /// covers it again. Crops in another tower are untouched, the player is
    /// told how, and the flowering notice is not repeated when the cover
    /// wears off (once per area while it flowers). Seen red by not giving
    /// the hand days to crops
    /// that already had a record (`r.hand_days_left = ...` removed).
    #[test]
    fn hand_pollinating_restores_full_set_while_it_lasts() {
        let data = store(100.0, true);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let e = world.spawn((crop("tomato", "ntower_3", "flower"),));
        let other = world.spawn((crop("tomato", "ntower_4", "flower"),));
        sys.tick(&mut world, 1.0, &data); // both get a record
        let _ = notices(&data);
        request(&data, "ntower_3");
        // Two garden days, inside the 2.5 the pollination covers.
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        let r = rec(&world, e);
        assert!(r.hand_days_left > 0.0, "{r:?}");
        let covered = r.pollinated_days / r.flowering_days;
        assert!(covered > 0.9, "covered while it lasts: {r:?}");
        assert_eq!(rec(&world, other).pollinated_days, 0.0, "the other tower was not touched");
        let said = notices(&data);
        assert!(said.iter().any(|n| n.contains("You hand-pollinated the crops in ntower 3") && n.contains("2.5 garden days")), "{said:?}");
        // Four more days: the cover has worn off and the share falls.
        for _ in 0..48 {
            sys.tick(&mut world, 1.0, &data);
        }
        let r = rec(&world, e);
        assert_eq!(r.hand_days_left, 0.0);
        let later = r.pollinated_days / r.flowering_days;
        assert!(later < 0.6, "it wore off: {r:?}");
        let said = notices(&data);
        assert!(!said.iter().any(|n| n.contains("nothing carries their pollen")), "told once while it flowers: {said:?}");
        // Pressing again covers it again.
        request(&data, "ntower_3");
        sys.tick(&mut world, 1.0, &data);
        assert!(rec(&world, e).hand_days_left > 2.0);
    }

    /// An outdoor field crop needs no help: the wind and wild insects
    /// pollinate it, so it keeps no record and sets fully, and the
    /// Hand-pollinate action there only says so. Seen red by dropping the
    /// `outdoors` test in `fruit_set` (the field tomato with a record of no
    /// pollinated days then set 0.49).
    #[test]
    fn an_outdoor_field_crop_needs_no_help() {
        let data = store(100.0, true);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let field = world.spawn((crop("zucchini", "grain_field_1", "flower"),));
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(world.get::<&CropPollination>(field).is_err(), "no record outdoors");
        let d = shipped();
        let none = CropPollination { flowering_days: 5.0, ..Default::default() };
        assert_eq!(fruit_set(d.crop("zucchini").unwrap(), Some(&none), true), 1.0);
        assert_eq!(fruit_set(d.crop("tomato").unwrap(), Some(&none), true), 1.0);
        let mut p = Pollination::new();
        step(&mut p, &mut world, &data, 0.0);
        assert_eq!(p.harvest_set(&world, &data, field), 1.0);
        assert!(!notices(&data).iter().any(|n| n.contains("nothing carries")), "no flowering notice outdoors");
        assert_eq!(card_row(d.crop("corn").unwrap(), None, true, false, Phase::Flowering), "outdoors: the wind");
    }

    /// Lettuce is harvested as leaves, so pollination does not touch it: no
    /// record, no card row, a harvest multiplier of 1. A self-pollinating
    /// bean needs no help either. Seen red by making `PollinationData::crop`
    /// fall back to the first listed crop (the tomato) for an id it does not
    /// list: the lettuce then got a record.
    #[test]
    fn lettuce_is_unaffected() {
        let data = store(100.0, true);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let lettuce = world.spawn((crop("lettuce", "ntower_3", "vegetative"),));
        let bean = world.spawn((crop("bean", "ntower_3", "flower"),));
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(world.get::<&CropPollination>(lettuce).is_err() && world.get::<&CropPollination>(bean).is_err());
        let mut p = Pollination::new();
        step(&mut p, &mut world, &data, 0.0);
        assert_eq!(p.harvest_set(&world, &data, lettuce), 1.0);
        assert_eq!(p.harvest_set(&world, &data, bean), 1.0);
        let mut with_data = store(100.0, true);
        with_data.insert(DATA_KEY, shipped());
        let view = GuiView::new(&world, &with_data);
        assert_eq!(view.row(lettuce), "", "no row for lettuce");
        assert_eq!(view.row(bean), "pollinates itself: needs no help");
        assert!(view.areas.is_empty(), "nothing to hand-pollinate");
    }

    /// A bumblebee hive pollinates the indoor areas it reaches and not the
    /// ones beyond: two towers, one beside the hive and one 30 m away (past
    /// its 17.8 m), each with a flowering tomato and a corn in silk. The
    /// near tomato is fully pollinated; the far one and both corns are not
    /// (bees do nothing for a wind-pollinated crop), the Garden panel
    /// offers Hand-pollinate only where flowers still wait, and a shipped
    /// hive spawns the marker the tick looks for (home_spawn's test). Seen
    /// red by making `hive_cover` ignore the reach (the far tower was then
    /// covered too).
    #[test]
    fn a_hive_covers_the_areas_it_reaches() {
        let mut data = store(100.0, true);
        data.insert(DATA_KEY, shipped());
        data.insert(
            "grow_plots",
            vec![
                GrowPlot { id: "ntower_3".into(), cups: 12, ..Default::default() },
                GrowPlot { id: "ntower_9".into(), cups: 12, pos: [30.0, 0.0, 0.0], ..Default::default() },
            ],
        );
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((PollinatorHive, Transform { position: glam::Vec3::new(1.0, 0.0, 0.0), ..Default::default() }));
        let near = world.spawn((crop("tomato", "ntower_3", "flower"),));
        let far = world.spawn((crop("tomato", "ntower_9", "flower"),));
        let near_corn = world.spawn((crop("corn", "ntower_3", "silk"),));
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        let (n, f, c) = (rec(&world, near), rec(&world, far), rec(&world, near_corn));
        assert!(n.flowering_days > 1.0 && (n.pollinated_days - n.flowering_days).abs() < 1e-9, "near: {n:?}");
        assert_eq!(f.pollinated_days, 0.0, "far: {f:?}");
        assert_eq!(c.pollinated_days, 0.0, "corn: {c:?}");
        let p = Pollination::new();
        assert_eq!(p.harvest_set(&world, &data, near), 1.0);
        assert!((p.harvest_set(&world, &data, far) - 0.49).abs() < 1e-6);
        let view = GuiView::new(&world, &data);
        assert_eq!(view.row(near), "a bumblebee hive is working the flowers");
        assert!(view.row(far).starts_with("not pollinated: expect about 49%"), "{}", view.row(far));
        assert_eq!(view.areas, vec!["ntower_3".to_string(), "ntower_9".to_string()], "the corn still waits beside the hive");
    }

    /// Off mode: every crop sets fully, nothing is recorded, nothing is
    /// said and nothing is shown. Seen red by not reading the mode in
    /// `harvest_set` (the zucchini that never saw a bee then set nothing
    /// with the mode Off). A first version of this test stayed green through
    /// that break because it asked before any data was loaded, so the answer
    /// was 1 for the wrong reason; the data is now in the store from the start.
    #[test]
    fn off_mode_sets_fully() {
        let mut data = store(100.0, false);
        data.insert(DATA_KEY, shipped());
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let fresh = world.spawn((crop("tomato", "ntower_3", "flower"),));
        let old = world.spawn((
            crop("zucchini", "ntower_3", "flower"),
            CropPollination { flowering_days: 5.0, ..Default::default() },
        ));
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(world.get::<&CropPollination>(fresh).is_err(), "nothing recorded");
        assert!(notices(&data).iter().all(|n| !n.contains("pollen")), "nothing said");
        let p = Pollination::new();
        assert_eq!(p.harvest_set(&world, &data, old), 1.0, "a zucchini that never saw a bee sets fully");
        let view = GuiView::new(&world, &data);
        assert_eq!((view.row(fresh), view.row(old), view.areas.len()), (String::new(), String::new(), 0));
        // And On again, the same zucchini sets nothing.
        *data.get::<std::sync::Mutex<bool>>(MODE_KEY).unwrap().lock().unwrap() = true;
        assert_eq!(p.harvest_set(&world, &data, old), 0.0);
    }

    /// The card's words for each case. Seen red by requiring more than two
    /// garden days of cover before the card says "hand-pollinated" (the
    /// covered tomato then read "not pollinated").
    #[test]
    fn the_card_row_says_where_the_crop_stands() {
        let d = shipped();
        let tomato = d.crop("tomato").unwrap();
        let cuke = d.crop("cucumber").unwrap();
        let corn = d.crop("corn").unwrap();
        let hand = CropPollination { flowering_days: 1.0, pollinated_days: 1.0, hand_days_left: 1.5 };
        assert_eq!(card_row(tomato, Some(&hand), false, false, Phase::Flowering), "hand-pollinated, 1.5 garden days left");
        assert_eq!(card_row(cuke, None, false, false, Phase::Flowering), "not pollinated: expect no fruit");
        assert_eq!(
            card_row(tomato, None, false, false, Phase::Before),
            "indoors: will need a hand or a bumblebee hive when it flowers (about 49% of a full crop without)"
        );
        assert_eq!(card_row(corn, None, false, true, Phase::Before), "indoors: will need a hand when it flowers (about 50% of a full crop without)");
        let half = CropPollination { flowering_days: 4.0, pollinated_days: 2.0, hand_days_left: 0.0 };
        assert_eq!(card_row(cuke, Some(&half), false, false, Phase::After), "set about 50% of a full crop");
    }
}
