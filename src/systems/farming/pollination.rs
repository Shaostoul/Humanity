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
//! 2. Indoors a flower is pollinated while a bumblebee hive WITH A WORKING
//!    COLONY reaches its grow area (`hive_cover`; bees do nothing for a
//!    wind-pollinated crop), or while the player's last hand pollination of
//!    that area still covers it: the "pollinate_request" channel (the Garden
//!    panel's Hand-pollinate button, `Request::Hand`) gives every crop there
//!    that needs help its `hand_every_days`. A pollination device (a powered
//!    circulating fan, pollination.ron `devices`) at the grow machine
//!    pollinates the crops it serves by their cited share (`Help`).
//! 3. The player is told once, when an indoor area's crops start flowering
//!    with nothing to pollinate them, what they will set left alone and how
//!    to pollinate them by hand (`flowering_notice`).
//! 4. At harvest the crop's fruit set (`fruit_set`) is ONE multiplier on its
//!    yield (`Pollination::harvest_set`, the single hook in the harvest path):
//!    its unhelped share plus the rest in proportion to the pollinated days.
//!
//! THE COLONY (2026-09-27). A placed hive holds no colony until the player
//! introduces one (the Garden panel's Introduce colony button,
//! `Request::Colony`, taking one bought `colony_item`). The colony works for
//! `colony_life_days` garden days, on the same garden clock as the flowering
//! days, and is then spent: its bees stop pollinating and the player is told
//! once. Each hive's colony is kept in `SoilMemory::hives` by the hive's
//! machine instance id, so it is saved, and a hive taken away and placed again
//! keeps the colony its id had (a new id has none).
//!
//! TWO MODES (the house rule for a deep system): the "garden_pollination_on"
//! channel, from Settings. Off, every crop sets fully and nothing is shown or
//! said, and no colony ages; On (the default, and the absent case) is the
//! cited model.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Deserialize;

use super::lighting::GrowPlot;
use crate::ecs::components::{CropInstance, CropPollination, HiveColony, SoilMemory, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;

/// The shipped copy, so a bare exe with no data folder still has it.
pub const POLLINATION_RON: &str = include_str!("../../../data/garden/pollination.ron");

/// DataStore key of the loaded data (registered by the main loop).
pub const DATA_KEY: &str = "garden_pollination";
/// DataStore key of the mode, a `Mutex<bool>`: true is On. Absent = On.
pub const MODE_KEY: &str = "garden_pollination_on";
/// DataStore key of the Garden panel's pollination request, a
/// `Mutex<Option<Request>>`.
pub const REQUEST_KEY: &str = "pollinate_request";

/// What the player asked of pollination in the Garden panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Hand-pollinate a grow area: a crop's `tower_id` ("" is the
    /// hand-planted crops).
    Hand(String),
    /// Introduce a new bumblebee colony in a hive, by the hive's machine
    /// instance id.
    Colony(String),
}

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

/// A pollination device (pollination.ron `devices`): a catalog machine that,
/// placed at a grow machine and powered, pollinates some of its crops.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceDef {
    /// Its data/machines catalog key; a placed one carries it as its
    /// `MachineType`.
    pub machine: String,
    /// What to call it, for the player ("circulating fan").
    pub name: String,
    /// The share of a full crop each crop it serves sets under it, 0..1, by
    /// plants.csv id. A crop not listed is not helped by it.
    pub sets: HashMap<String, f64>,
}

impl DeviceDef {
    /// How much of a pollinated day one flowering day under it is for this
    /// crop, 0..1: the part of what the crop cannot set alone that the device
    /// sets, so a day under it gives exactly its `sets` (0 for a crop it does
    /// not serve).
    pub fn help_for(&self, crop: &CropPollinationDef) -> f64 {
        let Some(sets) = self.sets.get(&crop.id) else { return 0.0 };
        let unhelped = crop.no_help.clamp(0.0, 1.0);
        if unhelped >= 1.0 {
            return 1.0;
        }
        ((sets - unhelped) / (1.0 - unhelped)).clamp(0.0, 1.0)
    }
}

/// What data/garden/pollination.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct PollinationData {
    /// The floor one bumblebee colony serves, m2.
    pub hive_cover_m2: f64,
    /// Garden days one colony works after it is introduced.
    pub colony_life_days: f64,
    /// The bought item (data/items.csv) one colony is.
    pub colony_item: String,
    /// Pollination devices.
    #[serde(default)]
    pub devices: Vec<DeviceDef>,
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
        if !(d.colony_life_days.is_finite() && d.colony_life_days > 0.0) {
            return Err("colony_life_days must be positive".to_string());
        }
        if d.colony_item.trim().is_empty() {
            return Err("colony_item names the bought colony item".to_string());
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
        for dev in &d.devices {
            if dev.machine.trim().is_empty() || dev.name.trim().is_empty() {
                return Err("a device names its catalog machine and what to call it".to_string());
            }
            for (crop, sets) in &dev.sets {
                if !(0.0..=1.0).contains(sets) {
                    return Err(format!("{}: sets for {crop} must be 0 to 1", dev.machine));
                }
                match d.index.get(crop).map(|i| &d.crops[*i]) {
                    Some(c) if c.needs_help() => {}
                    _ => return Err(format!("{}: {crop} is not a crop here that needs help", dev.machine)),
                }
            }
        }
        Ok(d)
    }

    /// The device placed as this catalog machine, if it is one.
    pub fn device(&self, machine: &str) -> Option<(usize, &DeviceDef)> {
        self.devices.iter().enumerate().find(|(_, dv)| dv.machine == machine)
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
/// counts as a flowering day, and as a pollinated one while a hand
/// pollination still covers it; the days it does not cover count as `help`
/// of a pollinated day (1 under a working hive, a device's share under a
/// device, 0 with nothing). The hand pollination wears off either way
/// (flowers opened after it need their own).
pub fn record_step(rec: &mut CropPollination, flowering: bool, help: f64, days: f64) {
    if !(days > 0.0) {
        return;
    }
    if flowering {
        rec.flowering_days += days;
        let hand = rec.hand_days_left.clamp(0.0, days);
        rec.pollinated_days += hand + (days - hand) * help.clamp(0.0, 1.0);
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

/// Every bumblebee hive: its machine instance id ("" for one without) and
/// where it stands, sorted by id.
pub fn hives(world: &hecs::World) -> Vec<(String, [f32; 3])> {
    use crate::ecs::components::{MachineInstanceId, PollinatorHive, Transform};
    let mut v: Vec<(String, [f32; 3])> = world
        .query::<(&PollinatorHive, &Transform, Option<&MachineInstanceId>)>()
        .iter()
        .map(|(_, (_, t, id))| (id.map(|i| i.0.clone()).unwrap_or_default(), t.position.to_array()))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// A hive's colony, as the Garden panel and the tick see it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Colony {
    /// No colony was ever introduced in it.
    None,
    /// Working, with this many garden days left.
    Working { left_days: f64 },
    /// Past its working life: it pollinates nothing.
    Spent,
}

impl Colony {
    pub fn of(rec: Option<&HiveColony>, life_days: f64) -> Self {
        match rec {
            None => Colony::None,
            Some(c) if c.age_days < life_days => Colony::Working { left_days: life_days - c.age_days },
            Some(_) => Colony::Spent,
        }
    }

    pub fn working(self) -> bool {
        matches!(self, Colony::Working { .. })
    }
}

/// The colonies kept for the hives, by hive id (empty before any was
/// introduced).
fn colonies(world: &hecs::World) -> HashMap<String, HiveColony> {
    world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.hives.clone()).unwrap_or_default()
}

/// Where the hives with a working colony stand.
fn working_hive_positions(world: &hecs::World, d: &PollinationData) -> Vec<[f32; 3]> {
    let kept = colonies(world);
    hives(world)
        .into_iter()
        .filter(|(id, _)| Colony::of(kept.get(id), d.colony_life_days).working())
        .map(|(_, p)| p)
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

/// The indoor grow machine nearest `pos` across the floor, by its id and
/// aliases: the one a device standing there serves.
pub fn nearest_plot(pos: [f32; 3], plots: &[GrowPlot]) -> Option<&GrowPlot> {
    plots.iter().filter(|p| !p.outdoors).min_by(|a, b| {
        let da = (a.pos[0] - pos[0]).powi(2) + (a.pos[2] - pos[2]).powi(2);
        let db = (b.pos[0] - pos[0]).powi(2) + (b.pos[2] - pos[2]).powi(2);
        da.total_cmp(&db)
    })
}

/// What pollinates each indoor grow area now: the areas a hive with a
/// working colony reaches, and the powered devices at each grow machine.
#[derive(Debug, Default)]
pub struct Help {
    hive_areas: HashSet<String>,
    /// Grow-area tag -> indexes into `PollinationData::devices`.
    devices: HashMap<String, Vec<usize>>,
}

impl Help {
    /// The home's hives and devices now (the engine publishes "grow_plots").
    /// An area under a row cover gets neither: the fabric keeps the bees out
    /// and a fan's draught off the flowers (critic review, 2026-09-27: a
    /// covered greenhouse bed was still pollinated by its hive).
    pub fn now(world: &hecs::World, data: &DataStore, d: &PollinationData) -> Self {
        let mut help = Self::uncovered(world, data, d);
        let covered = covered_fields(world, data);
        if !covered.is_empty() {
            help.hive_areas.retain(|a| !covered.contains(a));
            help.devices.retain(|a, _| !covered.contains(a));
        }
        help
    }

    /// The hives and devices before any cover is counted.
    fn uncovered(world: &hecs::World, data: &DataStore, d: &PollinationData) -> Self {
        let Some(plots) = data.get::<Vec<GrowPlot>>("grow_plots") else { return Self::default() };
        let working = working_hive_positions(world, d);
        let mut help = Help {
            hive_areas: if working.is_empty() { HashSet::new() } else { hive_cover(&working, plots, d.hive_reach_m()) },
            devices: HashMap::new(),
        };
        if d.devices.is_empty() {
            return help;
        }
        use crate::ecs::components::{MachineType, PowerConsumer, Transform};
        for (_, (kind, t, power)) in world.query::<(&MachineType, &Transform, Option<&PowerConsumer>)>().iter() {
            let Some((i, _)) = d.device(&kind.0) else { continue };
            // A device works only while it has power: one the electrical sim
            // sheds, or the player switches off, does nothing.
            if !power.map_or(false, |p| p.enabled) {
                continue;
            }
            let Some(plot) = nearest_plot(t.position.to_array(), plots) else { continue };
            for tag in std::iter::once(&plot.id).chain(plot.aliases.iter()) {
                let v = help.devices.entry(tag.clone()).or_default();
                if !v.contains(&i) {
                    v.push(i);
                }
            }
        }
        help
    }

    /// Does a hive with a working colony reach this area?
    pub fn hive(&self, area: &str) -> bool {
        self.hive_areas.contains(area)
    }

    /// The device at this area that helps this crop most, and its help
    /// (`DeviceDef::help_for`); None when no device there serves it.
    pub fn device<'a>(&self, d: &'a PollinationData, def: &CropPollinationDef, area: &str) -> Option<(&'a DeviceDef, f64)> {
        self.devices
            .get(area)?
            .iter()
            .filter_map(|i| d.devices.get(*i))
            .map(|dv| (dv, dv.help_for(def)))
            .filter(|(_, h)| *h > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// How much of a pollinated day each flowering day here is for this crop
    /// without a hand: 1 under a working hive (for a crop bees serve), a
    /// device's share, else 0.
    pub fn for_crop(&self, d: &PollinationData, def: &CropPollinationDef, area: &str) -> f64 {
        if def.by.hive_helps() && self.hive(area) {
            return 1.0;
        }
        self.device(d, def, area).map_or(0.0, |(_, h)| h)
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
    data_store.insert(REQUEST_KEY, std::sync::Mutex::new(Option::<Request>::None));
}

/// The Garden panel's side, for the main loop each frame: the Settings mode
/// and what the player asked (hand-pollinate an area, introduce a colony).
pub fn publish(data: &DataStore, on: bool, pending: Option<Request>) {
    if let Some(Ok(mut v)) = data.get::<std::sync::Mutex<bool>>(MODE_KEY).map(|m| m.lock()) {
        *v = on;
    }
    if let (Some(req), Some(m)) = (pending, data.get::<std::sync::Mutex<Option<Request>>>(REQUEST_KEY)) {
        if let Ok(mut v) = m.lock() {
            *v = Some(req);
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

    /// One farming tick of `days` garden days: the Garden panel's request
    /// (Hand-pollinate, or Introduce colony), the hives' colonies, then every
    /// indoor crop's record, then the notices.
    pub fn tick(&mut self, world: &mut hecs::World, data: &DataStore, days: f64) {
        let request = data
            .get::<std::sync::Mutex<Option<Request>>>(REQUEST_KEY)
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if !mode_on(data) {
            self.told.clear();
            return;
        }
        if self.data.is_none() && data.get::<PollinationData>(DATA_KEY).is_none() {
            self.data = Some(PollinationData::load());
        }
        let Some(d) = data.get::<PollinationData>(DATA_KEY).or(self.data.as_ref()) else { return };
        match request {
            Some(Request::Hand(area)) => hand_pollinate(world, data, d, &area),
            Some(Request::Colony(hive)) => introduce_colony(world, data, d, &hive),
            None => {}
        }
        age_colonies(world, data, d, days);
        let help = Help::now(world, data, d);
        let covered = covered_fields(world, data);
        let (waiting, flowering) = step_crops(world, d, &help, days, &covered);
        // Told once per area while it flowers: a hand pollination wearing
        // off is not news, the Garden panel's button says it.
        self.told.retain(|a| flowering.contains(a));
        for (area, plants) in &waiting {
            if self.told.insert(area.clone()) {
                super::push_notice(data, flowering_notice(data, d, area, plants, covered.contains(area)));
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
        let covered = covered_fields(world, data);
        let rec = world.get::<&CropPollination>(entity).ok();
        // A field crop with a record flowered under a cover: its record
        // decides, whether or not the cover is still on (critic review,
        // 2026-09-27: taking it off a click before harvest reset the set).
        let outdoors = rec.is_none() && crop.tower_id.as_deref().map_or(false, |a| open_field(a, &covered));
        fruit_set(def, rec.as_deref(), outdoors) as f32
    }
}

/// The outdoor fields under a floating row cover now (`farming::pests`,
/// 2026-09-27). A cover keeps the wild insects off as well as the pests, so
/// a crop there that needs insects is pollinated the way an indoor one is,
/// by hand, until the cover comes off; the University of Minnesota guides
/// the pests data cites say to take covers off cucurbits when they flower.
/// Bees from a hive never reach a covered field (hives serve indoor plots).
///
/// Named "fields" for the case that matters most, but it is every covered
/// grow area, indoor soil beds included (a cover on a greenhouse bed shuts
/// its hive out too, `Help::now`). With pests switched Off the covers are
/// out of play, as they are for warming (`pests::cover_warming`): the
/// Garden panel shows no cover then and none can be taken off, so one left
/// on must not keep cutting the fruit set (critic review, 2026-09-27).
fn covered_fields(world: &hecs::World, data: &DataStore) -> HashSet<String> {
    let severity = data
        .get::<std::sync::Mutex<f32>>("garden_pest_severity")
        .and_then(|m| m.lock().ok().map(|v| *v))
        .unwrap_or(super::pests::DEFAULT_PEST_SEVERITY);
    if !(severity > 0.0) {
        return HashSet::new();
    }
    data.get::<super::pests::PestData>("garden_pests")
        .map(|pd| super::pests::covered_areas(world, pd))
        .unwrap_or_default()
}

/// An outdoor field open to the wind and wild insects: a field not under a
/// row cover.
fn open_field(area: &str, covered: &HashSet<String>) -> bool {
    super::is_field_area(area) && !covered.contains(area)
}

/// Step every living indoor crop that needs help `days` garden days, giving
/// a record to any that has none. Returns, by area, the plants flowering with
/// nothing pollinating them, and every area with such a crop in flower.
fn step_crops(
    world: &mut hecs::World,
    d: &PollinationData,
    help: &Help,
    days: f64,
    covered: &HashSet<String>,
) -> (BTreeMap<String, BTreeSet<String>>, HashSet<String>) {
    let mut waiting: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut in_flower: HashSet<String> = HashSet::new();
    let mut fresh: Vec<(hecs::Entity, CropPollination)> = Vec::new();
    for (e, (crop, rec)) in world.query_mut::<(&CropInstance, Option<&mut CropPollination>)>() {
        let Some(def) = d.crop(&crop.crop_def_id) else { continue };
        let area = crop.tower_id.as_deref().unwrap_or("");
        // An open field needs no record; one that flowered under a cover
        // keeps its record, and from then on its open flowers count as
        // pollinated (the wind and wild insects), so taking the cover off
        // just before the harvest cannot erase what it cost.
        let open = open_field(area, covered);
        if !def.needs_help() || crop.growth_stage == STAGE_DEAD || (open && rec.is_none()) {
            continue;
        }
        let flowering = def.is_flowering(&crop.growth_stage);
        let helped = if open { 1.0 } else { help.for_crop(d, def, area) };
        let mut new = CropPollination::default();
        let had_record = rec.is_some();
        let r = match rec {
            Some(r) => r,
            None => &mut new,
        };
        if flowering && !open {
            in_flower.insert(area.to_string());
            if helped <= 0.0 && r.hand_days_left <= 0.0 {
                waiting.entry(area.to_string()).or_default().insert(crop.crop_def_id.clone());
            }
        }
        record_step(r, flowering, helped, days);
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
    if open_field(area, &covered_fields(world, data)) {
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

/// What to call a hive in a notice or a row: "bumblebee hive 1" for
/// "bumblebee_hive_1".
fn hive_name(id: &str) -> String {
    if id.is_empty() {
        "the bumblebee hive".to_string()
    } else {
        id.replace('_', " ")
    }
}

/// Age every placed hive's colony `days` garden days, and tell the player
/// once when one is spent. A hive with no colony has nothing to age.
fn age_colonies(world: &mut hecs::World, data: &DataStore, d: &PollinationData, days: f64) {
    let ids: Vec<String> = hives(world).into_iter().map(|(id, _)| id).collect();
    if ids.is_empty() {
        return;
    }
    let mut spent: Vec<String> = Vec::new();
    for (_, mem) in world.query_mut::<&mut SoilMemory>() {
        for id in &ids {
            let Some(c) = mem.hives.get_mut(id) else { continue };
            if days > 0.0 {
                c.age_days += days;
            }
            if c.age_days >= d.colony_life_days && !c.told {
                c.told = true;
                spent.push(id.clone());
            }
        }
        break;
    }
    for id in spent {
        super::push_notice(
            data,
            format!(
                "The bumblebee colony in {} has come to the end of its working life after {}: its bees no longer \
                 pollinate. Koppert: \"Remove bumblebee hives the latest 10 weeks after introduction\". Introduce \
                 a new colony from the Garden panel (a {} from the vendor), or pollinate by hand.",
                hive_name(&id),
                days_words(d.colony_life_days),
                item_name(data, &d.colony_item).to_lowercase()
            ),
        );
    }
}

/// A bought item's display name ("Bumblebee Colony"), from items.csv.
fn item_name(data: &DataStore, id: &str) -> String {
    data.get::<crate::systems::inventory::ItemRegistry>("item_registry")
        .and_then(|r| r.items.get(id).map(|i| i.name.clone()))
        .unwrap_or_else(|| id.trim_end_matches("_0").replace('_', " "))
}

/// The Introduce colony action on one hive: a new colony takes one bought
/// `colony_item` from the player's pack (nothing in creative mode) and works
/// for `colony_life_days`. Refused while the hive's colony is still working,
/// which a new one would waste.
fn introduce_colony(world: &mut hecs::World, data: &DataStore, d: &PollinationData, hive: &str) {
    if !hives(world).iter().any(|(id, _)| id == hive) {
        super::push_notice(data, format!("There is no bumblebee hive called {} in the home.", hive_name(hive)));
        return;
    }
    let kept = colonies(world);
    if let Colony::Working { left_days } = Colony::of(kept.get(hive), d.colony_life_days) {
        super::push_notice(
            data,
            format!(
                "The colony in {} is still working, with {} left; a new one now would waste it.",
                hive_name(hive),
                days_words(left_days)
            ),
        );
        return;
    }
    let creative = data
        .get::<std::sync::Mutex<bool>>("creative_mode")
        .and_then(|m| m.lock().ok().map(|g| *g))
        .unwrap_or(false);
    let name = item_name(data, &d.colony_item);
    if !creative {
        let mut took = false;
        for (_e, (inv, _ctrl)) in world
            .query_mut::<(&mut crate::systems::inventory::Inventory, &crate::ecs::components::Controllable)>()
        {
            if inv.count_item(&d.colony_item) >= 1 {
                inv.remove_item(&d.colony_item, 1);
                took = true;
            }
            break;
        }
        if !took {
            super::push_notice(
                data,
                format!(
                    "Introducing a colony in {} takes a {} (bought from the vendor), and you have none.",
                    hive_name(hive),
                    name.to_lowercase()
                ),
            );
            return;
        }
    }
    let e = super::soil::soil_memory_entity(world);
    if let Ok(mut mem) = world.get::<&mut SoilMemory>(e) {
        mem.hives.insert(hive.to_string(), HiveColony::default());
    }
    super::push_notice(
        data,
        format!(
            "You introduced a {} in {}. Its bees work the flowers of the indoor crops within {:.0} m for {}, and \
             then the colony is spent and must be replaced.",
            name.to_lowercase(),
            hive_name(hive),
            d.hive_reach_m(),
            days_words(d.colony_life_days)
        ),
    );
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
pub fn flowering_notice(data: &DataStore, d: &PollinationData, area: &str, plants: &BTreeSet<String>, covered: bool) -> String {
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
    // Under a row cover the fabric keeps the insects, the wind, a hive's bees
    // and a fan's draught off the flowers, so only two things help: taking
    // the cover off, or pollinating by hand.
    if covered {
        return format!(
            "The {} plants in {where_} are flowering under a row cover, which keeps insects and the wind off \
             them. Left alone, {}. Take the cover off, or hand-pollinate them from the Garden panel while they \
             flower ({}).",
            names.join(" and "),
            alone.join("; "),
            how.join("; ")
        );
    }
    let mut s = format!(
        "The {} plants in {where_} are flowering, and indoors nothing carries their pollen. Left alone, {}. \
         Hand-pollinate them from the Garden panel while they flower ({})",
        names.join(" and "),
        alone.join("; "),
        how.join("; ")
    );
    if defs.iter().any(|c| c.by.hive_helps()) {
        s.push_str(", or keep a bumblebee colony in a hive near them");
    }
    // A device that serves any of them (pollination.ron `devices`).
    for dv in &d.devices {
        let served: Vec<String> = defs
            .iter()
            .filter_map(|c| dv.sets.get(&c.id).map(|v| format!("{} sets {}", plant_name(data, &c.id), share_words(*v))))
            .collect();
        if !served.is_empty() {
            s.push_str(&format!(", or run a {} over them ({} under one)", dv.name, served.join("; ")));
        }
    }
    s.push('.');
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

/// The crop card's "Pollination" row. `hive_here`: a hive with a working
/// colony reaches it. `device`: the device at its grow machine that serves it
/// best, as (its name, the share of a full crop it sets there).
pub fn card_row(
    def: &CropPollinationDef,
    rec: Option<&CropPollination>,
    outdoors: bool,
    hive_here: bool,
    device: Option<(&str, f64)>,
    phase: Phase,
) -> String {
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
        Phase::Before => match device {
            Some((name, sets)) => format!("the {name} will pollinate it when it flowers ({})", share_words(sets)),
            None => {
                let who = if def.by.hive_helps() { "a hand or a bumblebee hive" } else { "a hand" };
                format!("indoors: will need {who} when it flowers ({} without)", share_words(def.no_help))
            }
        },
        Phase::Flowering if hive => "a bumblebee hive is working the flowers".to_string(),
        Phase::Flowering if hand > 0.0 => format!("hand-pollinated, {} left", days_words(hand)),
        Phase::Flowering => match device {
            Some((name, sets)) => format!("a {name} is working the flowers: {} under it", share_words(sets)),
            None => {
                let expect = match rec {
                    Some(r) if r.flowering_days > 0.0 => fruit_set(def, rec, false),
                    _ => def.no_help,
                };
                format!("not pollinated: expect {}", share_words(expect))
            }
        },
        Phase::After => format!("set {}", share_words(fruit_set(def, rec, false))),
    }
}

/// One bumblebee hive, as the Garden panel shows it under a grow area it
/// reaches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HiveRow {
    /// The hive's machine instance id (what `Request::Colony` names).
    pub id: String,
    /// Its colony: "bumblebee hive 1: colony working, 52 of 70 garden days
    /// left", "...: no colony ...", "...: colony spent ...".
    pub line: String,
    /// It has no working colony: the Introduce colony button shows.
    pub needs_colony: bool,
}

/// One indoor grow area's pollination, for the Garden panel: whether flowers
/// there are short of a pollinator (the Hand-pollinate button shows under
/// `waiting`) and the hives that reach it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AreaRow {
    /// The grow area's tag (a crop's `tower_id`; "" for hand-planted crops).
    pub area: String,
    /// "Flowering, nothing to pollinate it", or "Flowering, only partly
    /// pollinated" when a device sets some of it; "" when every flower there
    /// is pollinated (or none is open).
    pub waiting: String,
    /// The hives that reach it, when it grows a crop bees pollinate.
    pub hives: Vec<HiveRow>,
}

/// The line for a hive with this colony.
pub fn hive_line(id: &str, colony: Colony, d: &PollinationData, item: &str) -> String {
    let name = hive_name(id);
    let mut name_cap: String = name.chars().take(1).flat_map(char::to_uppercase).collect();
    name_cap.push_str(&name.chars().skip(1).collect::<String>());
    match colony {
        Colony::None => format!(
            "{name_cap}: no colony (a {} is bought, and works {})",
            item.to_lowercase(),
            days_words(d.colony_life_days)
        ),
        Colony::Working { left_days } => format!(
            "{name_cap}: colony working, {} of {} left",
            trim(left_days),
            days_words(d.colony_life_days)
        ),
        Colony::Spent => format!("{name_cap}: colony spent after {}; its bees no longer pollinate", days_words(d.colony_life_days)),
    }
}

/// Every crop's "Pollination" row and each indoor grow area's pollination
/// line (`AreaRow`: flowers waiting for a pollinator, where the Garden panel
/// shows its Hand-pollinate button, and the hives reaching it, with their
/// Introduce colony buttons), built once a frame by the main loop's crop
/// bridge. Empty with the mode Off.
#[derive(Debug, Default)]
pub struct GuiView {
    rows: HashMap<hecs::Entity, String>,
    pub areas: Vec<AreaRow>,
}

impl GuiView {
    pub fn new(world: &hecs::World, data: &DataStore) -> Self {
        let mut v = Self::default();
        let Some(d) = data.get::<PollinationData>(DATA_KEY).filter(|_| mode_on(data)) else { return v };
        let plants = data.get::<super::PlantRegistry>("plant_registry");
        let help = Help::now(world, data, d);
        let covered = covered_fields(world, data);
        // Per area: (flowers with no help at all, flowers only partly helped,
        // grows a crop bees pollinate).
        let mut areas: BTreeMap<String, (bool, bool, bool)> = BTreeMap::new();
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
            let outdoors = rec.is_none() && open_field(area, &covered);
            let hive_here = help.hive(area);
            let device = help.device(d, def, area);
            let sets = device.map(|(dv, _)| (dv.name.as_str(), dv.sets.get(&def.id).copied().unwrap_or(0.0)));
            let row = card_row(def, rec, outdoors, hive_here, sets, phase);
            if !outdoors && def.needs_help() {
                let entry = areas.entry(area.to_string()).or_default();
                entry.2 |= def.by.hive_helps();
                let helped = help.for_crop(d, def, area);
                if phase == Phase::Flowering && rec.map_or(0.0, |r| r.hand_days_left) <= 0.0 && helped < 1.0 {
                    if helped <= 0.0 {
                        entry.0 = true;
                    } else {
                        entry.1 = true;
                    }
                }
            }
            v.rows.insert(e, row);
        }
        // The hives, under each area they reach that grows a crop bees serve.
        let kept = colonies(world);
        let item = item_name(data, &d.colony_item);
        let reach: Vec<(String, HashSet<String>)> = match data.get::<Vec<GrowPlot>>("grow_plots") {
            Some(plots) => hives(world).into_iter().map(|(id, p)| (id, hive_cover(&[p], plots, d.hive_reach_m()))).collect(),
            None => Vec::new(),
        };
        for (area, (none, partly, bees)) in areas {
            let waiting = if none {
                "Flowering, nothing to pollinate it".to_string()
            } else if partly {
                "Flowering, only partly pollinated".to_string()
            } else {
                String::new()
            };
            let hives: Vec<HiveRow> = if bees {
                reach
                    .iter()
                    .filter(|(_, cover)| cover.contains(&area))
                    .map(|(id, _)| {
                        let colony = Colony::of(kept.get(id), d.colony_life_days);
                        HiveRow { id: id.clone(), line: hive_line(id, colony, d, &item), needs_colony: !colony.working() }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            if !waiting.is_empty() || !hives.is_empty() {
                v.areas.push(AreaRow { area, waiting, hives });
            }
        }
        v
    }

    /// This crop's row ("" for none: a crop the data does not list).
    pub fn row(&self, e: hecs::Entity) -> String {
        self.rows.get(&e).cloned().unwrap_or_default()
    }

    /// The pollination line of one grow area, if it has one.
    pub fn area(&self, area: &str) -> Option<&AreaRow> {
        self.areas.iter().find(|a| a.area == area)
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

    /// A world with the home's irrigation in it (the marker, unpowered: always
    /// on), so the crops stay watered at any garden speed. Water runs on the
    /// game clock (2026-09-27), and these tests are about flowers, not thirst.
    fn watered_world() -> hecs::World {
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,));
        world
    }

    fn shipped() -> PollinationData {
        PollinationData::parse(POLLINATION_RON).expect("the shipped pollination.ron parses")
    }

    /// A store at `speed` growth with the channels pollination uses and the
    /// notices, with the mode On (as absent) unless `on` is false.
    fn store(speed: f32, on: bool) -> DataStore {
        let mut data = make_store();
        crate::systems::farming::gardening_tests::set_garden_speed(&data, speed);
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        data.insert(REQUEST_KEY, std::sync::Mutex::new(Option::<Request>::None));
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

    /// A placed bumblebee hive `id` at `pos`, as home_spawn spawns one, with a
    /// colony of `colony` garden days' age kept for it (None: no colony).
    fn hive(world: &mut hecs::World, id: &str, pos: [f32; 3], colony: Option<f64>) -> hecs::Entity {
        let e = world.spawn((
            PollinatorHive,
            Transform { position: glam::Vec3::from_array(pos), ..Default::default() },
            crate::ecs::components::MachineInstanceId(id.to_string()),
        ));
        if let Some(age_days) = colony {
            let m = crate::systems::farming::soil::soil_memory_entity(world);
            world.get::<&mut SoilMemory>(m).unwrap().hives.insert(id.to_string(), HiveColony { age_days, told: false });
        }
        e
    }

    fn colony(world: &hecs::World, id: &str) -> Option<HiveColony> {
        world.query::<&SoilMemory>().iter().next().and_then(|(_, m)| m.hives.get(id).cloned())
    }

    fn rec(world: &hecs::World, e: hecs::Entity) -> CropPollination {
        world.get::<&CropPollination>(e).map(|r| (*r).clone()).unwrap_or_default()
    }

    fn notices(data: &DataStore) -> Vec<String> {
        std::mem::take(&mut *data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
    }

    fn request(data: &DataStore, area: &str) {
        *data.get::<std::sync::Mutex<Option<Request>>>(REQUEST_KEY).unwrap().lock().unwrap() = Some(Request::Hand(area.to_string()));
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
        record_step(&mut a, true, 0.0, 10.0);
        let mut b = CropPollination { hand_days_left: 2.5, ..Default::default() };
        for _ in 0..1000 {
            record_step(&mut b, true, 0.0, 0.01);
        }
        assert!((a.pollinated_days - 2.5).abs() < 1e-9 && (b.pollinated_days - 2.5).abs() < 1e-6, "{a:?} {b:?}");
        assert!((a.flowering_days - 10.0).abs() < 1e-9 && (b.flowering_days - 10.0).abs() < 1e-6);
        assert_eq!(a.hand_days_left, 0.0);
        // A quarter of the flowering helped: 0.49 + 0.51 x 0.25.
        assert!((fruit_set(tomato, Some(&a), false) - (0.49 + 0.51 * 0.25)).abs() < 1e-9);
        // Not in flower: the cover wears off, nothing is counted.
        let mut c = CropPollination { hand_days_left: 2.5, ..Default::default() };
        record_step(&mut c, false, 1.0, 1.0);
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
        let mut world = watered_world();
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
        let mut world = watered_world();
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
        let mut world = watered_world();
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
        let mut world = watered_world();
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
        assert_eq!(card_row(d.crop("corn").unwrap(), None, true, false, None, Phase::Flowering), "outdoors: the wind");
    }

    /// A field under a floating row cover (farming::pests) is shut to wild
    /// insects, so a crop there that needs them records its flowering days
    /// like an indoor one, sets less fruit unhelped, and can be
    /// hand-pollinated. Taking the cover off does not erase what the covered
    /// flowering cost: the record decides at harvest, and the flowers that
    /// open after it are pollinated by the wind and wild insects. Found by the
    /// pests work (2026-09-27): a covered zucchini field set a full crop; then
    /// by the critic review: removing the cover a click before harvest reset
    /// the set to 100%. Seen red with `open_field` ignoring the cover (no
    /// record was made), and with `harvest_set` reading the cover at harvest
    /// again (the set jumped to 1.0 when the cover came off).
    #[test]
    fn a_covered_field_is_pollinated_like_an_indoor_area() {
        let mut data = store(100.0, true);
        data.insert("garden_pests", crate::systems::farming::pests::PestData::load());
        let mut sys = FarmingSystem::new();
        let mut world = watered_world();
        let field = world.spawn((crop("zucchini", "grain_field_1", "flower"),));
        let m = crate::systems::farming::soil::soil_memory_entity(&mut world);
        world
            .get::<&mut SoilMemory>(m)
            .unwrap()
            .pests
            .entry("grain_field_1".to_string())
            .or_default()
            .releases
            .insert("row_cover".to_string(), 300.0);
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        let r = rec(&world, field);
        assert!(r.flowering_days > 0.0, "under the cover the flowering is recorded: {r:?}");
        let mut p = Pollination::new();
        step(&mut p, &mut world, &data, 0.0);
        assert!(p.harvest_set(&world, &data, field) < 1.0, "unhelped under a cover, it sets less");

        request(&data, "grain_field_1");
        step(&mut p, &mut world, &data, 0.1);
        assert!(
            !notices(&data).iter().any(|n| n.contains("pollinated by the wind")),
            "hand pollination is not refused under a cover"
        );
        assert!(rec(&world, field).hand_days_left > 0.0, "the hand pollination took");

        // Hand pollination aside, what the covered flowering cost stays.
        world.get::<&mut CropPollination>(field).unwrap().hand_days_left = 0.0;
        let before = p.harvest_set(&world, &data, field);
        world.get::<&mut SoilMemory>(m).unwrap().pests.get_mut("grain_field_1").unwrap().releases.clear();
        assert!(
            (p.harvest_set(&world, &data, field) - before).abs() < 1e-6,
            "taking the cover off does not reset the set: {before}"
        );
        // Open again, its later flowers are pollinated.
        let r0 = rec(&world, field);
        step(&mut p, &mut world, &data, 2.0);
        let r1 = rec(&world, field);
        assert!(
            r1.flowering_days > r0.flowering_days && r1.pollinated_days - r0.pollinated_days > 1.99,
            "uncovered flowers count as pollinated: {r0:?} then {r1:?}"
        );
        assert!(p.harvest_set(&world, &data, field) > before, "so the set rises");
    }

    /// With pests switched Off the covers are out of play, as they are for
    /// warming: the panel shows none and none can be taken off, so a cover
    /// left on must not cut the fruit set (critic review, 2026-09-27). Seen
    /// red with `covered_fields` ignoring the pest setting: the field kept a
    /// record and set less.
    #[test]
    fn with_pests_off_a_cover_left_on_does_not_cut_the_set() {
        let mut data = store(100.0, true);
        data.insert("garden_pests", crate::systems::farming::pests::PestData::load());
        data.insert("garden_pest_severity", std::sync::Mutex::new(0.0_f32));
        let mut sys = FarmingSystem::new();
        let mut world = watered_world();
        let field = world.spawn((crop("zucchini", "grain_field_1", "flower"),));
        let m = crate::systems::farming::soil::soil_memory_entity(&mut world);
        world.get::<&mut SoilMemory>(m).unwrap().pests.entry("grain_field_1".to_string()).or_default().releases.insert("row_cover".to_string(), 300.0);
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(world.get::<&CropPollination>(field).is_err(), "no record: the field is open with pests off");
        assert_eq!(Pollination::new().harvest_set(&world, &data, field), 1.0);
    }

    /// A row cover on an indoor bed shuts its hive out: the fabric keeps the
    /// bees off the flowers (critic review, 2026-09-27: a covered greenhouse
    /// bed was still pollinated fully). Seen red with `Help::now` not
    /// removing covered areas: the covered tomatoes were pollinated.
    #[test]
    fn a_covered_indoor_bed_is_not_reached_by_its_hive() {
        let mut data = store(100.0, true);
        data.insert(DATA_KEY, shipped());
        data.insert("garden_pests", crate::systems::farming::pests::PestData::load());
        data.insert("grow_plots", vec![GrowPlot { id: "bed_1".into(), footprint_m2: 2.0, ..Default::default() }]);
        let mut sys = FarmingSystem::new();
        let mut world = watered_world();
        hive(&mut world, "hive_1", [1.0, 0.0, 0.0], Some(0.0));
        let open = world.spawn((crop("tomato", "bed_1", "flower"),));
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        let r = rec(&world, open);
        assert!(r.pollinated_days > 0.0 && (r.pollinated_days - r.flowering_days).abs() < 1e-9, "uncovered, the hive works it: {r:?}");
        let m = crate::systems::farming::soil::soil_memory_entity(&mut world);
        world.get::<&mut SoilMemory>(m).unwrap().pests.entry("bed_1".to_string()).or_default().releases.insert("row_cover".to_string(), 300.0);
        for _ in 0..24 {
            sys.tick(&mut world, 1.0, &data);
        }
        let r2 = rec(&world, open);
        assert!(r2.flowering_days > r.flowering_days, "it kept flowering: {r2:?}");
        assert!((r2.pollinated_days - r.pollinated_days).abs() < 1e-9, "covered, the hive no longer reaches it: {r:?} then {r2:?}");
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
        let mut world = watered_world();
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
    ///
    /// CHANGED 2026-09-27: a hive pollinates only with a working colony, so
    /// this one has its machine id and a colony introduced just before.
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
        let mut world = watered_world();
        hive(&mut world, "hive_1", [1.0, 0.0, 0.0], Some(0.0));
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
        let waiting: Vec<&str> = view.areas.iter().filter(|a| !a.waiting.is_empty()).map(|a| a.area.as_str()).collect();
        assert_eq!(waiting, vec!["ntower_3", "ntower_9"], "the corn still waits beside the hive");
        let near_hives = &view.area("ntower_3").unwrap().hives;
        assert_eq!(near_hives.len(), 1, "the hive is listed where it reaches");
        assert!(!near_hives[0].needs_colony && near_hives[0].line.starts_with("Hive 1: colony working"), "{:?}", near_hives[0]);
        assert!(view.area("ntower_9").unwrap().hives.is_empty(), "and not where it does not");
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
        let mut world = watered_world();
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
        assert_eq!(card_row(tomato, Some(&hand), false, false, None, Phase::Flowering), "hand-pollinated, 1.5 garden days left");
        assert_eq!(card_row(cuke, None, false, false, None, Phase::Flowering), "not pollinated: expect no fruit");
        assert_eq!(
            card_row(tomato, None, false, false, None, Phase::Before),
            "indoors: will need a hand or a bumblebee hive when it flowers (about 49% of a full crop without)"
        );
        assert_eq!(card_row(corn, None, false, true, None, Phase::Before), "indoors: will need a hand when it flowers (about 50% of a full crop without)");
        let half = CropPollination { flowering_days: 4.0, pollinated_days: 2.0, hand_days_left: 0.0 };
        assert_eq!(card_row(cuke, Some(&half), false, false, None, Phase::After), "set about 50% of a full crop");
    }

    // -- The colony's life and the devices (2026-09-27) --------------------------

    /// The player's pack, holding `colonies` bought colonies.
    fn player_with(world: &mut hecs::World, colonies: u32) -> hecs::Entity {
        let mut inv = Inventory::new(16);
        inv.volume_capacity_l = 1.0e9;
        if colonies > 0 {
            inv.add_item("bumblebee_colony_0", colonies, 10);
        }
        world.spawn((inv, Controllable))
    }

    fn colonies_held(world: &hecs::World, who: hecs::Entity) -> u32 {
        world.get::<&Inventory>(who).unwrap().count_item("bumblebee_colony_0")
    }

    fn ask(data: &DataStore, r: Request) {
        *data.get::<std::sync::Mutex<Option<Request>>>(REQUEST_KEY).unwrap().lock().unwrap() = Some(r);
    }

    /// The shipped colony and device data point at real things: a colony
    /// works Koppert's 70 garden days (10 weeks); the colony is a real
    /// items.csv item the vendor sells (the rule tests/recipe_sources_lint.rs
    /// holds recipe inputs to: whatever the player must spend has a source),
    /// at the Arbico price in trade_goods.ron's scale; and every device is a
    /// machine in both shipped home catalogs with a Consumer power role (so
    /// it is spawned, and works only while powered) and serves crops that
    /// need help. Seen red by renaming the colony's trade_goods.ron row to
    /// another id (the vendor then sold nothing to introduce).
    #[test]
    fn the_colony_and_the_devices_are_real_things_to_buy_and_place() {
        let d = shipped();
        assert_eq!(d.colony_life_days, 70.0, "Koppert: remove hives the latest 10 weeks after introduction");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let items = crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(root.join("data/items.csv")).unwrap()).unwrap();
        assert!(items.items.contains_key(&d.colony_item), "{} is an items.csv item", d.colony_item);
        let goods = crate::systems::economy::TradeGoodsRegistry::from_ron(&std::fs::read(root.join("data/trade_goods.ron")).unwrap()).unwrap();
        let good = goods.get(&d.colony_item).unwrap_or_else(|| panic!("the vendor sells {}", d.colony_item));
        assert_eq!(good.base_value, 2483, "Arbico's $300 at the $7.25 federal minimum wage, a credit a minute");
        assert!(goods.vendor_sell_price(&d.colony_item).is_some());
        assert!(!d.devices.is_empty(), "a device is listed");
        for file in ["home.ron", "home_solo.ron"] {
            let home = crate::machines::MachineHome::load(&root.join("data/machines").join(file)).unwrap();
            for dv in &d.devices {
                let def = home.catalog.get(&dv.machine).unwrap_or_else(|| panic!("{file}: no catalog machine '{}'", dv.machine));
                assert!(
                    matches!(def.power, Some(crate::machines::MachinePower::Consumer { .. })),
                    "{file}: {} has a Consumer power role",
                    dv.machine
                );
                assert!(!def.pollinates_crops, "{file}: a device is not a hive");
            }
        }
        let fan = d.device("circulation_fan").expect("the circulating fan").1;
        assert_eq!(fan.sets.get("strawberry"), Some(&0.79), "Allen and Gaede: 77 of the 97 a daily brush sets");
        assert!(fan.sets.get("tomato").is_none(), "no source has a fan pollinate tomatoes indoors");
    }

    /// A colony works its 70 garden days and is then spent. A hive whose
    /// colony was just introduced pollinates a flowering tomato beside it
    /// fully; once the colony passes 70 garden days its bees stop, the tomato
    /// is pollinated no more, the player is told once, and the Garden panel
    /// lists the hive as spent with the Introduce colony button. A hive
    /// placed with no colony never pollinates. Seen red by never ageing a
    /// colony (`age_colonies` not called from the tick): after 69 garden
    /// days of work the hive still read "70 of 70 garden days left".
    #[test]
    fn a_colony_works_its_life_then_is_spent() {
        let mut data = store(100.0, true);
        data.insert(DATA_KEY, shipped());
        let mut world = watered_world();
        hive(&mut world, "hive_1", [1.0, 0.0, 0.0], Some(0.0));
        let t = world.spawn((crop("tomato", "ntower_3", "flower"),));
        let mut p = Pollination::new();
        step(&mut p, &mut world, &data, 60.0);
        step(&mut p, &mut world, &data, 9.0);
        let r = rec(&world, t);
        assert_eq!((r.flowering_days, r.pollinated_days), (69.0, 69.0), "working: every flowering day pollinated");
        assert!(!notices(&data).iter().any(|n| n.contains("end of its working life")), "not spent yet");
        let view = GuiView::new(&world, &data);
        let row = &view.area("ntower_3").unwrap().hives[0];
        assert_eq!(row.line, "Hive 1: colony working, 1 of 70 garden days left");
        assert!(!row.needs_colony);
        // Past 70: spent.
        step(&mut p, &mut world, &data, 2.0);
        step(&mut p, &mut world, &data, 5.0);
        let r = rec(&world, t);
        assert_eq!((r.flowering_days, r.pollinated_days), (76.0, 69.0), "spent: no more pollination");
        let said = notices(&data);
        let told: Vec<&String> = said.iter().filter(|n| n.contains("end of its working life")).collect();
        assert_eq!(told.len(), 1, "told once: {said:?}");
        assert!(told[0].contains("hive 1") && told[0].contains("70 garden days") && told[0].contains("bumblebee colony"), "{}", told[0]);
        assert!(colony(&world, "hive_1").unwrap().told);
        let view = GuiView::new(&world, &data);
        let row = &view.area("ntower_3").unwrap().hives[0];
        assert!(row.needs_colony && row.line.contains("colony spent"), "{row:?}");
        assert!(view.row(t).starts_with("not pollinated"), "{}", view.row(t));
        // A hive with no colony pollinates nothing and says so.
        let mut world = watered_world();
        hive(&mut world, "hive_2", [1.0, 0.0, 0.0], None);
        let t = world.spawn((crop("tomato", "ntower_3", "flower"),));
        step(&mut p, &mut world, &data, 3.0);
        assert_eq!(rec(&world, t).pollinated_days, 0.0, "no colony, no bees");
        let view = GuiView::new(&world, &data);
        let row = &view.area("ntower_3").unwrap().hives[0];
        assert!(row.needs_colony && row.line.starts_with("Hive 2: no colony (a bumblebee colony is bought"), "{row:?}");
    }

    /// Introducing a colony is a real action with a real cost: it takes one
    /// bought colony from the pack, refused when the pack has none; it is
    /// refused while the hive's colony still works (a new one would waste
    /// it, and nothing is taken); a spent colony is replaced for another
    /// bought one; in creative mode nothing is taken. The colony is kept in
    /// the saved SoilMemory: it survives a serde round trip, and a save from
    /// before it loads with none. Seen red by skipping the pack check (the
    /// first request, with no colony in the pack, introduced one anyway).
    #[test]
    fn introducing_a_colony_takes_a_bought_one() {
        let mut data = store(100.0, true);
        data.insert(DATA_KEY, shipped());
        let mut world = watered_world();
        hive(&mut world, "hive_1", [1.0, 0.0, 0.0], None);
        let who = player_with(&mut world, 0);
        let mut p = Pollination::new();
        ask(&data, Request::Colony("hive_1".into()));
        step(&mut p, &mut world, &data, 0.0);
        assert!(colony(&world, "hive_1").is_none(), "none in the pack: no colony");
        assert!(notices(&data).iter().any(|n| n.contains("takes a bumblebee colony") && n.contains("you have none")));
        // Two bought: one goes in.
        world.get::<&mut Inventory>(who).unwrap().add_item("bumblebee_colony_0", 2, 10);
        ask(&data, Request::Colony("hive_1".into()));
        step(&mut p, &mut world, &data, 0.0);
        assert_eq!(colony(&world, "hive_1"), Some(HiveColony::default()), "a new colony");
        assert_eq!(colonies_held(&world, who), 1, "one taken");
        assert!(notices(&data).iter().any(|n| n.contains("You introduced a bumblebee colony in hive 1") && n.contains("70 garden days")));
        // Working: refused, nothing taken.
        step(&mut p, &mut world, &data, 30.0);
        ask(&data, Request::Colony("hive_1".into()));
        step(&mut p, &mut world, &data, 0.0);
        assert_eq!(colony(&world, "hive_1").unwrap().age_days, 30.0, "the working colony stays");
        assert_eq!(colonies_held(&world, who), 1, "nothing taken");
        assert!(notices(&data).iter().any(|n| n.contains("still working, with 40 garden days left")));
        // Spent: replaced.
        step(&mut p, &mut world, &data, 45.0);
        ask(&data, Request::Colony("hive_1".into()));
        step(&mut p, &mut world, &data, 0.0);
        assert_eq!(colony(&world, "hive_1"), Some(HiveColony::default()), "replaced");
        assert_eq!(colonies_held(&world, who), 0);
        // Creative: nothing needed.
        data.insert("creative_mode", std::sync::Mutex::new(true));
        hive(&mut world, "hive_2", [40.0, 0.0, 0.0], None);
        ask(&data, Request::Colony("hive_2".into()));
        step(&mut p, &mut world, &data, 0.0);
        assert!(colony(&world, "hive_2").is_some(), "creative: introduced with none in the pack");
        // Saved with SoilMemory, and an older save has none.
        let mem = world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap();
        let back: SoilMemory = serde_json::from_str(&serde_json::to_string(&mem).unwrap()).unwrap();
        assert_eq!(back.hives, mem.hives);
        let older: SoilMemory = serde_json::from_str("{}").unwrap();
        assert!(older.hives.is_empty());
    }

    /// A powered circulating fan over strawberries sets them the cited 79%
    /// (Allen and Gaede via McGregor: 77 of the 97 a daily brush sets), with
    /// no clicking at any growth speed. It serves the grow machine nearest
    /// it and no other, does nothing for a tomato (no source), and nothing
    /// unpowered. The card, the Garden panel line and the flowering notice
    /// say so. Seen red by leaving the devices out of `Help::now` (the
    /// strawberries under the fan then set 21%).
    #[test]
    fn a_fan_over_strawberries_sets_their_cited_share() {
        let mut data = store(100.0, true);
        data.insert(DATA_KEY, shipped());
        data.insert(
            "grow_plots",
            vec![
                GrowPlot { id: "ntower_0".into(), cups: 12, ..Default::default() },
                GrowPlot { id: "ntower_5".into(), cups: 12, pos: [6.0, 0.0, 0.0], ..Default::default() },
            ],
        );
        let mut world = watered_world();
        let fan = world.spawn((
            crate::ecs::components::MachineType("circulation_fan".into()),
            Transform { position: glam::Vec3::new(0.5, 0.0, 0.0), ..Default::default() },
            crate::ecs::components::PowerConsumer { draw_watts: 11.0, priority: 4, enabled: true },
        ));
        let under = world.spawn((crop("strawberry", "ntower_0", "flower"),));
        let beyond = world.spawn((crop("strawberry", "ntower_5", "flower"),));
        let tomato = world.spawn((crop("tomato", "ntower_0", "flower"),));
        let mut p = Pollination::new();
        step(&mut p, &mut world, &data, 10.0);
        assert!((p.harvest_set(&world, &data, under) - 0.79).abs() < 1e-6, "{}", p.harvest_set(&world, &data, under));
        assert!((p.harvest_set(&world, &data, beyond) - 0.21).abs() < 1e-6, "the next machine is not under it");
        assert!((p.harvest_set(&world, &data, tomato) - 0.49).abs() < 1e-6, "a fan does not pollinate a tomato");
        let view = GuiView::new(&world, &data);
        assert_eq!(view.row(under), "a circulating fan is working the flowers: about 79% of a full crop under it");
        assert_eq!(view.area("ntower_0").unwrap().waiting, "Flowering, nothing to pollinate it", "the tomato has nothing");
        let said = notices(&data);
        assert!(
            said.iter().any(|n| n.contains("ntower 5") && n.contains("run a circulating fan over them (strawberry sets about 79% of a full crop under one)")),
            "{said:?}"
        );
        // Unpowered: nothing. Ten more days at no help halve what it set.
        world.get::<&mut crate::ecs::components::PowerConsumer>(fan).unwrap().enabled = false;
        step(&mut p, &mut world, &data, 10.0);
        let half = 0.21 + 0.79 * ((0.79 - 0.21) / 0.79) / 2.0;
        assert!((p.harvest_set(&world, &data, under) - half).abs() < 1e-6, "{}", p.harvest_set(&world, &data, under));
        // Only strawberries in the area: partly pollinated, the Hand-pollinate
        // button still offered for the rest.
        world.despawn(tomato).unwrap();
        world.get::<&mut crate::ecs::components::PowerConsumer>(fan).unwrap().enabled = true;
        let view = GuiView::new(&world, &data);
        assert_eq!(view.area("ntower_0").unwrap().waiting, "Flowering, only partly pollinated");
        let d = shipped();
        assert_eq!(
            card_row(d.crop("strawberry").unwrap(), None, false, false, Some(("circulating fan", 0.79)), Phase::Before),
            "the circulating fan will pollinate it when it flowers (about 79% of a full crop)"
        );
    }
}
