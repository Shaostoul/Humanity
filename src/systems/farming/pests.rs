//! Garden pests and integrated pest management (2026-09-26, gardening depth).
//!
//! Until this rung the garden had no pests at all (the 2026-09-25 gap survey:
//! "No pests, disease, weeds, pollination or rotation"). The pests, the
//! controls, every number and its source live in data/garden/pests.ron; this
//! file is the model.
//!
//! THE LOOP:
//!
//! 1. Each grow area (a tower, bed, tray or field; the hand-planted crops are
//!    one more, tagged "") holds a PRESSURE per pest, 0 to 1, in
//!    `SoilMemory::pests`. It belongs to the place, not the crop: it stays in
//!    the area between crops and meets whatever is sown there next.
//! 2. Every tick, on the garden clock (the crops' own, so 10x by default),
//!    each area's pressure grows logistically from a trickle of arrivals
//!    (`grow`), at the pest's cited rate scaled by the area's HOST SHARE and
//!    by the conditions each host crop is in (`favour`): a monoculture of a
//!    pest's host in the conditions it likes builds it fastest. With no host
//!    in the area it dies out or leaves (`decay`), which is why ROTATION works.
//! 3. The player is told ONCE, when a pest crosses `detect_at` in an area,
//!    what it is, what favours it and which controls to try, gentlest first
//!    (`appeared_notice`). That is the monitoring step of IPM.
//! 4. On each host crop the pressure caps health (`health_ceiling`), the way
//!    a nutrient shortage does, so health eases down gradually, never to death,
//!    and the season health the harvest reads carries the loss.
//! 5. The player controls it in the IPM order: by hand or with water
//!    (Mechanical), natural enemies (Biological), a least-toxic spray last
//!    (LeastToxic): `apply_control`, driven from the "pest_control_request"
//!    channel. A release of natural enemies keeps working for days while its
//!    prey lasts; a soap spray kills the predatory mites as well as the pests.
//!
//! TWO MODES (the house rule for a deep system): the "garden_pest_severity"
//! channel scales the damage. 0 turns pests off entirely, 0.5 (the default,
//! `DEFAULT_PEST_SEVERITY`) is gentle, 1 is the cited damage.
//!
//! DISEASES (2026-09-26) are rows of this same model, not a second system:
//! gray mold, powdery mildew and downy mildew, each favoured by a `Humidity`
//! window read from the air the crop grows in (farming::humidity), each with
//! its own `unfavoured_rate` because the sources say they infect only in
//! those conditions. Their controls follow the IPM order too: ventilate
//! (Cultural, acting on the grow room's air), remove infected leaves
//! (Mechanical), then the least-toxic sprays, which may `protect` the leaves
//! for days rather than kill what is there. The severity setting covers them.
//!
//! GREENHOUSE PESTS AND ROW COVERS (2026-09-27). Greenhouse whitefly and
//! western flower thrips are two more rows, with their traps (sticky cards,
//! which keep working with nothing to catch: `stays_without_prey`) and the
//! natural enemies growers release for them (Encarsia wasps, cucumeris
//! mites), which are for greenhouses only (`indoors_only`). A ROW COVER is a
//! lasting control of its own kind (`ControlDef::is_cover`): laid over a
//! soil area from pieces in the backpack, sized by the ground it spans, it
//! keeps a share of each pest's ARRIVALS out (`excludes`) but not the ones
//! already under it, which go on breeding (the guides' reason to lay it
//! first and rotate); it warms the crops under it (`cover_warming`, read by
//! the crop climate of a field and the pests' Temperature); and bees cannot
//! reach flowers under it, so the player is told when it goes over a crop
//! that needs insects to pollinate it and the Garden panel flags one in
//! flower (`cover_rows`). It is saved as the area's release, its days being
//! the fabric's life in the weather, and taking it off gives the pieces
//! back. Pests are not advanced during the offline catch-up
//! (docs/design/offline-progression.md), and neither is a cover's wear.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::ecs::components::AreaPests;

/// The shipped copy, so a bare exe with no data folder still has pests.
pub const PESTS_RON: &str = include_str!("../../../data/garden/pests.ron");

/// The lowest pests can take a crop's health: 20 of 100, the same floor a
/// nutrient shortage has (soil::NUTRIENT_HEALTH_FLOOR, from Rothamsted's
/// unfertilised Broadbalk wheat), so even two pests at their worst stunt a
/// crop and never kill it. Only thirst, RF and hard acceleration kill.
pub const PEST_HEALTH_FLOOR: f32 = 20.0;

/// The damage scale when nothing has set "garden_pest_severity": gentle, half
/// the cited damage. The house rule (CLAUDE.md, 2026-09-24): every deep
/// system has a full-realism mode and a simplified one, and general play
/// defaults to the softened one. 1.0 is the realistic mode, 0 turns pests off.
pub const DEFAULT_PEST_SEVERITY: f32 = 0.5;

/// Below this a release of natural enemies has run out of prey: UC IPM,
/// "If spider mite prey are not present, these predators will disperse or
/// starve". A thousandth of a full infestation.
pub const PREY_GONE: f64 = 1e-3;

/// A pest this rare that the player has not been told about is dropped from
/// the area, so the saved map does not fill with near-zero entries.
const FORGET_BELOW: f64 = 1e-6;

// -- Data: data/garden/pests.ron --------------------------------------------------

/// The IPM order: the gentlest first. Declared in that order, so sorting by
/// kind sorts by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum ControlKind {
    /// Changing the conditions: ventilating a humid grow room (2026-09-26).
    Cultural,
    /// By hand or with water: hand-picking, hosing off, removing leaves.
    Mechanical,
    /// Natural enemies and the microbes that sicken pests.
    Biological,
    /// The least-toxic sprays and baits, last.
    LeastToxic,
}

/// A condition a pest thrives in (pests.ron `favoured_by`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Condition {
    /// The area's temperature is inside this window, Celsius.
    Temperature { min_c: f64, max_c: f64 },
    /// The crop's unit holds more than `excess_n_seasons` of its N need.
    ExcessNitrogen,
    /// The crop is thirsty (below the farming water-stress line).
    WaterStress,
    /// Outdoors in cloudy, foggy or wet weather, or at night.
    Damp,
    /// The game season is one of these (lower case).
    Season(Vec<String>),
    /// The air's relative humidity, 0..1, is inside this window: the grow
    /// room's indoors (farming::humidity), the weather's outdoors.
    Humidity { min: f64, max: f64 },
}

/// One pest (pests.ron `pests`).
#[derive(Debug, Clone, Deserialize)]
pub struct PestDef {
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub indoors: bool,
    #[serde(default = "yes")]
    pub outdoors: bool,
    pub hosts: Vec<String>,
    #[serde(default)]
    pub favoured_by: Vec<Condition>,
    /// A disease rather than a pest (2026-09-26): said as "has appeared".
    #[serde(default)]
    pub disease: bool,
    /// This pest's own factor for a favouring condition not met, when its
    /// sources say it spreads ONLY in those conditions (the diseases); None
    /// uses the file's `unfavoured_rate`.
    #[serde(default)]
    pub unfavoured_rate: Option<f64>,
    pub fold: f64,
    pub fold_days: f64,
    pub arrival_per_day: f64,
    pub no_host_half_life_days: f64,
    pub max_health_loss: f64,
    #[serde(default)]
    pub advice: String,
    /// `hosts` as a set, built by `PestData::parse`.
    #[serde(skip)]
    host_set: HashSet<String>,
}

fn yes() -> bool {
    true
}

impl PestDef {
    /// Does this pest feed on `plant` (a plants.csv id)?
    pub fn hosts_plant(&self, plant: &str) -> bool {
        self.host_set.contains(plant)
    }

    /// Its intrinsic rate of increase, per garden day, when every favouring
    /// condition holds: `fold` times as many in `fold_days`, so ln(fold) /
    /// fold_days (the standard r = ln R0 / T). Aphids: ln 80 / 11 = 0.40.
    pub fn growth_rate(&self) -> f64 {
        if self.fold > 1.0 && self.fold_days > 0.0 {
            self.fold.ln() / self.fold_days
        } else {
            0.0
        }
    }
}

/// One control (pests.ron `controls`).
#[derive(Debug, Clone, Deserialize)]
pub struct ControlDef {
    pub id: String,
    pub name: String,
    pub kind: ControlKind,
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub plants_per_item: u32,
    #[serde(default)]
    pub water_l_per_plant: f64,
    #[serde(default)]
    pub removes: HashMap<String, f64>,
    #[serde(default)]
    pub removes_per_day: HashMap<String, f64>,
    #[serde(default)]
    pub lasts_days: f64,
    #[serde(default)]
    pub ends: Vec<String>,
    /// Share of each pest's growth and arrivals it stops while it lasts
    /// (`lasts_days`): a protectant spray, which keeps new infection from
    /// starting and cures nothing (2026-09-26).
    #[serde(default)]
    pub protects: HashMap<String, f64>,
    /// Air changes it gives the area's grow room at once, with the home's air
    /// (the heat-and-vent, farming::humidity::ventilate). 0 for the rest.
    #[serde(default)]
    pub air_changes: f64,
    /// Pests whose conditions it takes away, listed with them though it
    /// removes none (ventilating a humid room).
    #[serde(default)]
    pub prevents: Vec<String>,
    /// plants.csv ids it must not be used on (sulfur burns cucurbits): it is
    /// refused on an area where one grows.
    #[serde(default)]
    pub not_on: Vec<String>,
    /// Share of each pest's ARRIVALS it keeps out while it is on (a row
    /// cover, 2026-09-27). Not of the pests already under it: they go on
    /// breeding there, which is why the guides say to lay it first.
    #[serde(default)]
    pub excludes: HashMap<String, f64>,
    /// Degrees C it warms the crops under it while it is on: in sun, and
    /// otherwise (at night, and indoors where no sun falls on it). Read by
    /// `cover_warming` for the crop climate and the pests' Temperature.
    #[serde(default)]
    pub warms_day_c: f64,
    #[serde(default)]
    pub warms_night_c: f64,
    /// Square metres of ground one item covers: a row cover is sized by the
    /// bed, not by the plants in it. 0 sizes it by `plants_per_item`.
    #[serde(default)]
    pub m2_per_item: f64,
    /// It lies over soil: refused on a tower or a mushroom rack.
    #[serde(default)]
    pub needs_soil: bool,
    /// For greenhouses: refused on an outdoor field (Encarsia wasps and
    /// cucumeris mites, which UC IPM does not recommend outdoors).
    #[serde(default)]
    pub indoors_only: bool,
    /// A lasting release that keeps working when its prey is gone: sticky
    /// cards, and predators that bring their own food or eat pollen.
    #[serde(default)]
    pub stays_without_prey: bool,
    /// The cover (a control id) this takes off, giving its items back.
    #[serde(default)]
    pub takes_off: String,
    #[serde(default)]
    pub note: String,
}

impl ControlDef {
    /// Does this control act on `pest` at all: at once, over days, by
    /// guarding against it, by keeping it out, or by taking away its
    /// conditions?
    pub fn works_on(&self, pest: &str) -> bool {
        self.removes.contains_key(pest)
            || self.removes_per_day.contains_key(pest)
            || self.protects.contains_key(pest)
            || self.excludes.contains_key(pest)
            || self.prevents.iter().any(|p| p == pest)
    }

    /// Does it keep working over the days after it is applied?
    pub fn lasts(&self) -> bool {
        self.lasts_days > 0.0
            && (!self.removes_per_day.is_empty() || !self.protects.is_empty() || !self.excludes.is_empty())
    }

    /// Is it a cover laid over the crops (a row cover), which stays until it
    /// is taken off or worn out?
    pub fn is_cover(&self) -> bool {
        !self.excludes.is_empty()
    }

    /// Items it takes to cover `m2` of ground (a row cover): whole pieces,
    /// at least one; 0 when it uses no item or is not sized by the ground.
    pub fn items_for_ground(&self, m2: f64) -> u32 {
        if self.item.is_empty() || !(self.m2_per_item > 0.0) {
            return 0;
        }
        let pieces = (m2.max(0.0) / self.m2_per_item - 1e-9).ceil().max(1.0);
        pieces.min(f64::from(u32::MAX)) as u32
    }

    /// Items it takes to treat `plants` plants (0 when it uses none).
    pub fn items_for(&self, plants: usize) -> u32 {
        if self.item.is_empty() || plants == 0 {
            0
        } else {
            let per = self.plants_per_item.max(1) as usize;
            ((plants + per - 1) / per) as u32
        }
    }
}

/// What data/garden/pests.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct PestData {
    pub unfavoured_rate: f64,
    pub detect_at: f64,
    pub indoor_temp_c: f64,
    pub excess_n_seasons: f64,
    pub pests: Vec<PestDef>,
    #[serde(default)]
    pub controls: Vec<ControlDef>,
}

impl PestData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d: PestData = ron::from_str(text).map_err(|e| e.to_string())?;
        for p in &mut d.pests {
            p.host_set = p.hosts.iter().cloned().collect();
        }
        Ok(d)
    }

    /// The data folder's copy first (so it can be modded), the shipped copy
    /// if that is missing or does not parse. The shipped copy always parses:
    /// a test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("pests.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::parse(PESTS_RON).expect("the shipped data/garden/pests.ron parses")
    }

    pub fn pest(&self, id: &str) -> Option<&PestDef> {
        self.pests.iter().find(|p| p.id == id)
    }

    pub fn control(&self, id: &str) -> Option<&ControlDef> {
        self.controls.iter().find(|c| c.id == id)
    }

    /// The controls that work on `pest`, in the IPM order (gentlest first;
    /// within a kind, the order of the file).
    pub fn controls_for(&self, pest: &str) -> Vec<&ControlDef> {
        let mut v: Vec<&ControlDef> = self.controls.iter().filter(|c| c.works_on(pest)).collect();
        v.sort_by_key(|c| c.kind);
        v
    }
}

// -- The model ------------------------------------------------------------------

/// What a crop is growing in, for the favouring conditions.
#[derive(Debug, Clone, Copy)]
pub struct CropConditions<'a> {
    /// An outdoor field (`farming::is_field_area`).
    pub outdoors: bool,
    /// Outdoors the weather's; indoors `PestData::indoor_temp_c`.
    pub temp_c: f64,
    pub water_stressed: bool,
    pub excess_n: bool,
    /// Outdoors, and cloudy, foggy, wet or night.
    pub damp: bool,
    /// The game season, lower case ("" when there is no clock: every season
    /// condition then holds, the way the crop climate factor treats it).
    pub season: &'a str,
    /// The relative humidity, 0..1, of the air the crop grows in: its grow
    /// room's indoors, the weather's outdoors (farming::humidity::AirMap).
    pub humidity: f64,
}

fn met(cond: &Condition, c: &CropConditions) -> bool {
    match cond {
        Condition::Temperature { min_c, max_c } => c.temp_c >= *min_c && c.temp_c <= *max_c,
        Condition::ExcessNitrogen => c.excess_n,
        Condition::WaterStress => c.water_stressed,
        Condition::Damp => c.damp,
        Condition::Season(seasons) => c.season.is_empty() || seasons.iter().any(|s| s.eq_ignore_ascii_case(c.season)),
        Condition::Humidity { min, max } => c.humidity >= *min && c.humidity <= *max,
    }
}

/// How much one crop feeds `pest`'s growth in its area, 0..1: 0 if the pest
/// does not live there (indoors or out) or does not eat this plant, else 1
/// times `unfavoured` (or the pest's own `unfavoured_rate`, for a disease
/// that spreads only in its conditions) for every favouring condition the
/// crop is not in.
pub fn favour(pest: &PestDef, plant: &str, c: &CropConditions, unfavoured: f64) -> f64 {
    let lives_here = if c.outdoors { pest.outdoors } else { pest.indoors };
    if !lives_here || !pest.hosts_plant(plant) {
        return 0.0;
    }
    let unfavoured = pest.unfavoured_rate.unwrap_or(unfavoured).clamp(0.0, 1.0);
    pest.favoured_by
        .iter()
        .map(|cond| if met(cond, c) { 1.0 } else { unfavoured })
        .product()
}

/// Grow a pressure `p0` for `days` garden days at rate `r` per day, with
/// `a` per day arriving from outside: dP/dt = (r P + a)(1 - P), logistic
/// growth toward a full infestation plus a trickle of newcomers that keeps it
/// from ever being exactly zero on a host.
///
/// Solved exactly rather than stepped, so a tick of any length (a frame, a
/// dev clock jump, a catch-up after time away) gives the same answer as many
/// small ones. With Q = (r P + a) / (1 - P), dQ/dt = (r + a) Q, so Q grows
/// exponentially and P = (Q - a) / (Q + r).
pub fn grow(p0: f64, r: f64, a: f64, days: f64) -> f64 {
    let p0 = if p0.is_finite() { p0.clamp(0.0, 1.0) } else { 0.0 };
    let (r, a) = (r.max(0.0), a.max(0.0));
    if !(days > 0.0) || p0 >= 1.0 || r + a <= 0.0 {
        return p0;
    }
    let q0 = (r * p0 + a) / (1.0 - p0);
    if q0 <= 0.0 {
        return p0;
    }
    let ln_q = q0.ln() + (r + a) * days;
    if ln_q > 700.0 {
        return 1.0;
    }
    let q = ln_q.exp();
    ((q - a) / (q + r)).clamp(0.0, 1.0)
}

/// With no host in the area the pressure halves every `half_life` garden
/// days: the pests starve, leave or die over the winter in the ground.
pub fn decay(p: f64, half_life: f64, days: f64) -> f64 {
    if !(days > 0.0) {
        return p;
    }
    if !(half_life > 0.0) {
        return 0.0;
    }
    p * 0.5f64.powf(days / half_life)
}

/// Step one area's pests `days` garden days. `favour_sum[i]` is, for
/// `data.pests[i]`, the sum of `favour` over the area's `n_crops` living
/// crops (indexed, not keyed by id, because the tally runs over every crop
/// every tick), so `favour_sum / n_crops` is the host share weighted by
/// conditions: a pure stand of a host in the conditions it likes is 1, half
/// the area in another crop is 0.5, and it is 0 with no host (the pressure
/// then decays). A missing entry reads as 0. Returns the ids of the pests
/// that have just become noticeable (`detect_at`), for the one notice each.
pub fn step_area(
    area: &mut AreaPests,
    data: &PestData,
    favour_sum: &[f64],
    n_crops: usize,
    days: f64,
) -> Vec<String> {
    let mut appeared = Vec::new();
    if !(days > 0.0) {
        return appeared;
    }
    // A pest a mod has since removed from pests.ron is forgotten.
    area.pressure.retain(|id, _| data.pest(id).is_some());
    for (i, pest) in data.pests.iter().enumerate() {
        let share = if n_crops > 0 {
            (favour_sum.get(i).copied().unwrap_or(0.0) / n_crops as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let existing = area.pressure.get(&pest.id).copied();
        if share <= 0.0 && existing.is_none() {
            continue;
        }
        let mut st = existing.unwrap_or_default();
        // A protectant spray still on the leaves (sulfur, potassium
        // bicarbonate) stops its share of new infection: of the growth and
        // the arrivals alike.
        let guard: f64 = area
            .releases
            .keys()
            .filter_map(|cid| data.control(cid).and_then(|c| c.protects.get(&pest.id)))
            .map(|k| 1.0 - k.clamp(0.0, 1.0))
            .product();
        // A row cover over the area keeps its share of newcomers out, and
        // nothing more: what is already under it goes on breeding (UMD, UMN:
        // lay it before the pests come, and rotate first).
        let shut: f64 = area
            .releases
            .keys()
            .filter_map(|cid| data.control(cid).and_then(|c| c.excludes.get(&pest.id)))
            .map(|k| 1.0 - k.clamp(0.0, 1.0))
            .product();
        st.level = if share > 0.0 {
            grow(st.level, pest.growth_rate() * share * guard, pest.arrival_per_day * share * guard * shut, days)
        } else {
            decay(st.level, pest.no_host_half_life_days, days)
        };
        // Natural enemies released here keep eating while they last.
        for cid in area.releases.keys() {
            if let Some(k) = data.control(cid).and_then(|c| c.removes_per_day.get(&pest.id)) {
                st.level *= (1.0 - k.clamp(0.0, 1.0)).powf(days);
            }
        }
        if st.level >= data.detect_at && !st.told {
            st.told = true;
            appeared.push(pest.id.clone());
        } else if st.level < data.detect_at / 2.0 {
            st.told = false;
        }
        if st.level < FORGET_BELOW && !st.told {
            area.pressure.remove(&pest.id);
        } else {
            area.pressure.insert(pest.id.clone(), st);
        }
    }
    // Releases age, and end when their time is up or, for natural enemies,
    // their prey is gone. A protectant spray has no prey: it lasts its time,
    // as do sticky cards, predators with their own food, and a row cover
    // (its time is the fabric's life in the weather).
    let pressure = &area.pressure;
    area.releases.retain(|cid, left| {
        *left -= days;
        let Some(c) = data.control(cid) else { return false };
        let prey_left = c.removes_per_day.is_empty()
            || c.stays_without_prey
            || c.removes_per_day.keys().any(|p| pressure.get(p).map_or(false, |s| s.level >= PREY_GONE));
        *left > 0.0 && prey_left
    });
    appeared
}

/// The highest health a crop of `plant` can hold under the pests in its area:
/// each pest that eats it takes `max_health_loss x pressure` of what is left,
/// scaled by `severity` (the mode), and never below `PEST_HEALTH_FLOOR`.
pub fn health_ceiling(data: &PestData, area: Option<&AreaPests>, plant: &str, severity: f32) -> f32 {
    let Some(area) = area else { return 100.0 };
    let sev = f64::from(severity).clamp(0.0, 1.0);
    let mut keep = 1.0f64;
    for pest in &data.pests {
        if !pest.hosts_plant(plant) {
            continue;
        }
        let p = area.pressure.get(&pest.id).map_or(0.0, |s| s.level.clamp(0.0, 1.0));
        keep *= 1.0 - (pest.max_health_loss * sev).clamp(0.0, 1.0) * p;
    }
    ((100.0 * keep) as f32).clamp(PEST_HEALTH_FLOOR, 100.0)
}

/// Degrees C the covers on an area warm the crops under them (a row cover,
/// 2026-09-27): each one's `warms_day_c` while the sun is on it (`sunlit`:
/// an outdoor field by day), its `warms_night_c` otherwise, at night and
/// indoors. 0 with no cover, and with pests off (`severity` 0), when the pest
/// controls are out of play. The farming tick adds it to the temperature the
/// crop climate of a field reads and to the pests' Temperature conditions.
pub fn cover_warming(data: &PestData, area: Option<&AreaPests>, sunlit: bool, severity: f32) -> f64 {
    let Some(area) = area else { return 0.0 };
    if !(severity > 0.0) {
        return 0.0;
    }
    area.releases
        .keys()
        .filter_map(|cid| data.control(cid))
        .filter(|c| c.is_cover())
        .map(|c| if sunlit { c.warms_day_c } else { c.warms_night_c })
        .sum()
}

/// What one application of a control did in an area.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ControlOutcome {
    /// Pests it knocked back: id, pressure before, pressure after.
    pub changed: Vec<(String, f64, f64)>,
    /// Noticeable pests in the area it does nothing to.
    pub untouched: Vec<String>,
    /// Releases it killed (a soap spray and the predatory mites).
    pub ended: Vec<String>,
    /// True when it is a release that will work over the days to come.
    pub released: bool,
}

/// Apply `control` once to an area: take its share off each pest it works
/// on at once, start its release if it lasts, and kill the releases it ends.
pub fn apply_control(area: &mut AreaPests, data: &PestData, control: &ControlDef) -> ControlOutcome {
    let mut out = ControlOutcome::default();
    let mut ids: Vec<String> = area.pressure.keys().cloned().collect();
    ids.sort();
    for id in ids {
        let Some(st) = area.pressure.get_mut(&id) else { continue };
        if let Some(k) = control.removes.get(&id) {
            let before = st.level;
            st.level *= 1.0 - k.clamp(0.0, 1.0);
            out.changed.push((id, before, st.level));
        } else if !control.works_on(&id) && st.level >= data.detect_at {
            out.untouched.push(id);
        }
    }
    if control.lasts() {
        let left = area.releases.entry(control.id.clone()).or_insert(0.0);
        *left = left.max(control.lasts_days);
        out.released = true;
    }
    for e in &control.ends {
        if area.releases.remove(e).is_some() {
            out.ended.push(e.clone());
        }
    }
    out
}

// -- What the player is told ------------------------------------------------------

/// "the crops in ntower 3", or "your hand-planted crops" for the "" area.
pub fn place(area: &str) -> String {
    if area.is_empty() {
        "your hand-planted crops".to_string()
    } else {
        format!("the crops in {}", area.replace('_', " "))
    }
}

fn pct(level: f64) -> String {
    format!("{:.0}%", (level * 100.0).clamp(0.0, 100.0))
}

/// The one line said when `pest` first becomes noticeable in `areas`: where,
/// what favours it (its advice), and its controls, gentlest first.
///
/// `soil` and `indoors` say whether any of `areas` grows in soil and
/// whether any is indoors, so the list offers only controls that can be used
/// there: a row cover needs soil, and the greenhouse natural enemies need a
/// greenhouse (critic review, 2026-09-27: the notice suggested a row cover
/// first for aphids on a tower, where laying one is refused).
pub fn appeared_notice(data: &PestData, pest: &PestDef, areas: &[String], soil: bool, indoors: bool) -> String {
    // "Aphids have", but "Gray mold has".
    let verb = if pest.disease { "has" } else { "have" };
    let wherever = match areas {
        [one] => format!("{} {verb} appeared on {}.", pest.name, place(one)),
        many => format!("{} {verb} appeared in {} grow areas.", pest.name, many.len()),
    };
    let controls: Vec<&str> = data
        .controls_for(&pest.id)
        .iter()
        .filter(|c| (soil || !c.needs_soil) && (indoors || !c.indoors_only))
        .map(|c| c.name.as_str())
        .collect();
    let ladder = if controls.is_empty() {
        String::new()
    } else {
        format!(" Controls, gentlest first: {}.", controls.join(", then "))
    };
    let advice = if pest.advice.is_empty() { String::new() } else { format!(" {}", pest.advice) };
    format!("{wherever}{advice}{ladder}")
}

/// The line said after a control is applied.
pub fn outcome_notice(
    data: &PestData,
    control: &ControlDef,
    area: &str,
    plants: usize,
    water_l: f64,
    out: &ControlOutcome,
) -> String {
    let name_of = |id: &str| data.pest(id).map_or(id.to_string(), |p| p.name.to_lowercase());
    let water = if water_l > 0.0 { format!(", {water_l:.0} L of water") } else { String::new() };
    let mut s = format!("{} on {} ({plants} plants{water})", control.name, place(area));
    let moved: Vec<String> = out
        .changed
        .iter()
        .filter(|(_, b, _)| *b >= FORGET_BELOW)
        .map(|(id, b, a)| format!("{} {} to {}", name_of(id), pct(*b), pct(*a)))
        .collect();
    if !moved.is_empty() {
        s.push_str(&format!(": {}.", moved.join(", ")));
    } else if out.released {
        s.push('.');
    } else {
        s.push_str(&format!(": nothing it works on is there. {}", control.note));
        return s;
    }
    if out.released && !control.removes_per_day.is_empty() {
        let mut prey: Vec<String> = control.removes_per_day.keys().map(|p| name_of(p)).collect();
        prey.sort();
        if control.stays_without_prey {
            s.push_str(&format!(" They will work on the {} for {:.0} garden days.", prey.join(" and "), control.lasts_days));
        } else {
            s.push_str(&format!(
                " They will work on the {} for up to {:.0} days, while there are any.",
                prey.join(" and "),
                control.lasts_days
            ));
        }
    }
    if out.released && !control.protects.is_empty() {
        let mut guarded: Vec<String> = control.protects.keys().map(|p| name_of(p)).collect();
        guarded.sort();
        s.push_str(&format!(
            " It guards the leaves against {} for about {:.0} garden days.",
            guarded.join(" and "),
            control.lasts_days
        ));
    }
    if !out.untouched.is_empty() {
        let names: Vec<String> = out.untouched.iter().map(|p| name_of(p)).collect();
        s.push_str(&format!(" It does nothing to the {} there.", names.join(" or ")));
    }
    if !out.ended.is_empty() {
        // A spray kills the natural enemies released there; a release of
        // wasps takes the sticky cards down first (2026-09-27), so they are
        // not caught.
        let (killed, taken_down): (Vec<&String>, Vec<&String>) = out
            .ended
            .iter()
            .partition(|e| data.control(e).map_or(true, |c| c.kind == ControlKind::Biological));
        let names = |v: &[&String]| -> String {
            v.iter().map(|e| data.control(e).map_or((*e).clone(), |c| c.name.to_lowercase())).collect::<Vec<_>>().join(", ")
        };
        if !killed.is_empty() {
            s.push_str(&format!(" It also killed what you released there ({}).", names(killed.as_slice())));
        }
        if !taken_down.is_empty() {
            s.push_str(&format!(" You took down the {} there first.", names(taken_down.as_slice())));
        }
    }
    s
}

// -- The control request, from the "pest_control_request" channel ------------------

/// Apply one player control request `(area, control id)` (2026-09-26): take
/// its items from the backpack (free in creative mode) and its water from the
/// home tanks, then act on the area's pests and tell the player what it did.
/// Refused, with a notice, when the area has no crops, the tanks are empty for
/// a water control, or the backpack lacks the items; nothing is spent then.
/// Since 2026-09-27 also refused: a greenhouse-only release on an outdoor
/// field, a cover where there is no soil to lay it on, and a second cover
/// over one already on. Taking a cover off needs no crops (after the harvest
/// is when UMN says to) and gives its pieces back.
pub(super) fn handle_request(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    pests: &PestData,
    air: &super::humidity::HumidityData,
    area: &str,
    control_id: &str,
    creative: bool,
    water_available: bool,
) {
    use crate::ecs::components::{CropInstance, STAGE_DEAD};
    let Some(control) = pests.control(control_id) else {
        log::warn!("[Farming] no pest control '{control_id}' in data/garden/pests.ron");
        return;
    };
    if !control.takes_off.is_empty() {
        take_off_cover(world, data, pests, control, area, creative);
        return;
    }
    // Every plant in the area counts, not every crop entity: since the
    // per-plot harvest (units.rs) one bed plot is many plants, and a soap
    // spray or a hosing is per plant (2026-09-26 review: a 128-plant bean
    // field was charged one item and 2 L).
    let registry = data.get::<super::PlantRegistry>("plant_registry");
    let plot_areas = data.get::<std::collections::HashMap<String, f32>>(super::units::PLOT_AREA_KEY);
    let mut plants = 0usize;
    let here: Vec<String> = world
        .query::<&CropInstance>()
        .iter()
        .filter(|(_, c)| c.growth_stage != STAGE_DEAD && c.tower_id.as_deref().unwrap_or("") == area)
        .map(|(_, c)| {
            plants += super::units::crop_plants(c, registry, plot_areas) as usize;
            c.crop_def_id.clone()
        })
        .collect();
    if plants == 0 {
        super::push_notice(data, format!("There are no crops to treat in {}.", area.replace('_', " ")));
        return;
    }
    // Encarsia wasps and cucumeris mites are for greenhouses (UC IPM: not
    // recommended outdoors), so an outdoor field is refused before anything
    // is spent.
    if control.indoors_only && super::is_field_area(area) {
        super::push_notice(
            data,
            format!("{} is for greenhouses, not an outdoor field like {}. {}", control.name, area.replace('_', " "), control.note),
        );
        return;
    }
    // A row cover lies over soil: a tower grows in a mist, a rack in its
    // own substrate, and there is nothing to lay it on.
    if control.needs_soil && !has_soil(data, area) {
        super::push_notice(
            data,
            format!(
                "A {} lies over a bed or a field, and {} has no soil to lay it on: a tower grows in a nutrient mist and a mushroom rack in its own substrate.",
                control.name.to_lowercase(),
                area.replace('_', " ")
            ),
        );
        return;
    }
    // One cover at a time.
    if control.is_cover() && cover_on(world, area, control) {
        super::push_notice(data, format!("There is already a {} over {}.", control.name.to_lowercase(), place(area)));
        return;
    }
    // A control whose label forbids a crop growing here (sulfur burns the
    // cucurbits) is refused, with the reason, before anything is spent.
    if let Some(plant) = here.iter().find(|p| control.not_on.iter().any(|n| n == *p)) {
        let name = data
            .get::<super::PlantRegistry>("plant_registry")
            .and_then(|r| r.get(plant).map(|d| d.name.to_lowercase()))
            .unwrap_or_else(|| plant.replace('_', " "));
        super::push_notice(
            data,
            format!("{} is not for the {name} in {}. {}", control.name, area.replace('_', " "), control.note),
        );
        return;
    }
    // Ventilating acts on the air of the area's grow room, not on its pests.
    if control.air_changes > 0.0 {
        super::push_notice(data, super::humidity::ventilate(world, data, air, area, control.air_changes));
        return;
    }
    let water_l = control.water_l_per_plant.max(0.0) * plants as f64;
    if water_l > 0.0 && !water_available {
        super::push_notice(data, format!("The water tanks are empty: nothing to {} with.", control.name.to_lowercase()));
        return;
    }
    // A cover is sized by the ground it spans, everything else by the plants.
    let ground_m2 = if control.m2_per_item > 0.0 { cover_ground_m2(world, data, area) } else { 0.0 };
    let need = if control.m2_per_item > 0.0 { control.items_for_ground(ground_m2) } else { control.items_for(plants) };
    let item_name = data
        .get::<crate::systems::inventory::ItemRegistry>("item_registry")
        .and_then(|r| r.items.get(&control.item).map(|d| d.name.clone()))
        .unwrap_or_else(|| control.item.clone());
    if need > 0 && !creative {
        let mut took = false;
        let mut have = 0;
        for (_e, (inv, _ctrl)) in world.query_mut::<(
            &mut crate::systems::inventory::Inventory,
            &crate::ecs::components::Controllable,
        )>() {
            have = inv.count_item(&control.item);
            if have >= need {
                inv.remove_item(&control.item, need);
                took = true;
            }
            break;
        }
        if !took {
            let what = if control.m2_per_item > 0.0 {
                format!("the {ground_m2:.0} m2 of ground")
            } else {
                format!("the {plants} plants")
            };
            super::push_notice(
                data,
                format!("{} needs {need} x {item_name} for {what} in {}; you have {have}.", control.name, area.replace('_', " ")),
            );
            return;
        }
    }
    if water_l > 0.0 {
        if let Some(m) = data.get::<std::sync::Mutex<f32>>("hand_water_draw_l") {
            if let Ok(mut v) = m.lock() {
                *v += water_l as f32;
            }
        }
    }
    let memory = super::soil::soil_memory_entity(world);
    let (out, state) = match world.get::<&mut crate::ecs::components::SoilMemory>(memory) {
        Ok(mut mem) => {
            let st = mem.pests.entry(area.to_string()).or_default();
            if control.is_cover() {
                st.cover_pieces.insert(control.id.clone(), if creative { 0 } else { need });
            }
            let out = apply_control(st, pests, control);
            (out, st.clone())
        }
        Err(_) => return,
    };
    log::info!("[Farming] {} on {area}: {:?}", control.id, out);
    let notice = if control.is_cover() {
        let bees = insect_pollinated(data, &here);
        cover_notice(pests, control, area, need, &item_name, ground_m2, &state, &bees)
    } else {
        outcome_notice(pests, control, area, plants, water_l, &out)
    };
    super::push_notice(data, notice);
}

// -- Row covers (2026-09-27) --------------------------------------------------------

/// Does `area` grow in soil (data/garden/soil_ph.ron: a bed, tray or field,
/// or the hand-planted crops), rather than a tower's mist or a rack's
/// substrate? The loaded copy from the DataStore, else the file.
pub fn has_soil(data: &crate::hot_reload::data_store::DataStore, area: &str) -> bool {
    match data.get::<super::soil_ph::SoilPhData>("garden_soil_ph") {
        Some(d) => d.soil_for(area).is_some(),
        None => super::soil_ph::SoilPhData::load().soil_for(area).is_some(),
    }
}

/// Every grow area with a cover on it now: the areas whose flowers bees
/// cannot reach (UMN: "row covers prevent pollinators from reaching the
/// flowers"), for the pollination model to ask. Empty with no covers.
pub fn covered_areas(world: &hecs::World, data: &PestData) -> HashSet<String> {
    let mut q = world.query::<&crate::ecs::components::SoilMemory>();
    let areas = q
        .iter()
        .next()
        .map(|(_, m)| {
            m.pests
                .iter()
                .filter(|(_, a)| a.releases.keys().any(|cid| data.control(cid).map_or(false, |c| c.is_cover())))
                .map(|(area, _)| area.clone())
                .collect()
        })
        .unwrap_or_default();
    areas
}

/// Is `cover` on `area` now?
fn cover_on(world: &hecs::World, area: &str, cover: &ControlDef) -> bool {
    let mut q = world.query::<&crate::ecs::components::SoilMemory>();
    let on = q.iter().next().map_or(false, |(_, m)| m.pests.get(area).map_or(false, |a| a.releases.contains_key(&cover.id)));
    on
}

/// The ground a cover over `area` spans, m2: every plot of its machine (the
/// engine's published plot area times its plots, the ground the weeds count
/// too), so laying it and taking it off agree on its pieces whatever grows
/// there. For an area the engine does not publish (the hand-planted crops),
/// each living crop's own ground: its plants.csv `area_per_plant_m2` times
/// its plants, a quarter square metre a plant where that is blank (a game
/// estimate, the one the sulfur note in pests.ron uses).
fn cover_ground_m2(world: &hecs::World, data: &crate::hot_reload::data_store::DataStore, area: &str) -> f64 {
    use crate::ecs::components::{CropInstance, STAGE_DEAD};
    if let Some(plot) = super::soil_ph::plot_area(data, area).filter(|a| *a > 0.0) {
        return plot * f64::from(super::weeds::area_plots(data, area));
    }
    let registry = data.get::<super::PlantRegistry>("plant_registry");
    let plot_areas = data.get::<std::collections::HashMap<String, f32>>(super::units::PLOT_AREA_KEY);
    let mut q = world.query::<&CropInstance>();
    let m2 = q
        .iter()
        .filter(|(_, c)| c.growth_stage != STAGE_DEAD && c.tower_id.as_deref().unwrap_or("") == area)
        .map(|(_, c)| {
            let each = registry
                .and_then(|r| r.get(&c.crop_def_id))
                .and_then(|d| d.area_per_plant_m2)
                .map(f64::from)
                .filter(|a| *a > 0.0)
                .unwrap_or(0.25);
            each * f64::from(super::units::crop_plants(c, registry, plot_areas))
        })
        .sum();
    m2
}

/// The crops among `plants` (plants.csv ids) that need insects to carry
/// their pollen (data/garden/pollination.ron, `by: Insects`), by display
/// name, once each: a cover keeps the bees off their flowers.
fn insect_pollinated(data: &crate::hot_reload::data_store::DataStore, plants: &[String]) -> Vec<String> {
    let owned;
    let pol = match data.get::<super::pollination::PollinationData>(super::pollination::DATA_KEY) {
        Some(p) => p,
        None => {
            owned = super::pollination::PollinationData::load();
            &owned
        }
    };
    let registry = data.get::<super::PlantRegistry>("plant_registry");
    let mut names: Vec<String> = plants
        .iter()
        .filter(|p| pol.crop(p).map_or(false, |c| c.by == super::pollination::PollinatedBy::Insects))
        .map(|p| registry.and_then(|r| r.get(p).map(|d| d.name.to_lowercase())).unwrap_or_else(|| p.replace('_', " ")))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// "a, b and c".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The line said when a cover goes on: what it took, what it keeps out and
/// what it cannot (the pests already under it), how much it warms the
/// crops, and the crops there that need bees at flowering.
#[allow(clippy::too_many_arguments)]
fn cover_notice(
    data: &PestData,
    cover: &ControlDef,
    area: &str,
    pieces: u32,
    item_name: &str,
    ground_m2: f64,
    state: &AreaPests,
    bees: &[String],
) -> String {
    let what = if pieces > 0 { format!(" ({pieces} x {item_name}, {ground_m2:.0} m2 of ground)") } else { String::new() };
    let mut s = format!("{} laid over {}{what}.", cover.name, place(area));
    let mut kept: Vec<String> = cover.excludes.keys().filter_map(|p| data.pest(p).map(|d| d.name.to_lowercase())).collect();
    kept.sort();
    s.push_str(&format!(" It keeps most new {} off", and_list(&kept)));
    let mut inside: Vec<(String, f64)> = state
        .pressure
        .iter()
        .filter(|(id, st)| cover.excludes.contains_key(*id) && st.level >= data.detect_at)
        .filter_map(|(id, st)| data.pest(id).map(|p| (p.name.to_lowercase(), st.level)))
        .collect();
    inside.sort_by(|a, b| a.0.cmp(&b.0));
    if inside.is_empty() {
        s.push('.');
    } else {
        let list: Vec<String> = inside.iter().map(|(n, l)| format!("{n} {}", pct(*l))).collect();
        s.push_str(&format!(
            ", but it cannot shut out what is already under it: {}, which go on breeding there.",
            and_list(&list)
        ));
    }
    s.push_str(&format!(
        " It warms the crops about {:.0} C in sun and {:.0} C at night.",
        cover.warms_day_c, cover.warms_night_c
    ));
    if !bees.is_empty() {
        s.push_str(&format!(
            " The {} here need bees to carry their pollen, and bees cannot get under it: take it off when they flower.",
            and_list(bees)
        ));
    }
    s
}

/// Take `off.takes_off` off `area` and put its pieces back in the backpack
/// (none in creative mode, where laying it took none). Refused, with the
/// cover left on, when there is none there or the pack has no room.
fn take_off_cover(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    pests: &PestData,
    off: &ControlDef,
    area: &str,
    creative: bool,
) {
    let Some(cover) = pests.control(&off.takes_off) else {
        log::warn!("[Farming] {} takes off an unknown control '{}'", off.id, off.takes_off);
        return;
    };
    if !cover_on(world, area, cover) {
        super::push_notice(data, format!("There is no {} over {}.", cover.name.to_lowercase(), place(area)));
        return;
    }
    // The pieces this cover took when it was laid, not a recount of the
    // ground under it now: the crops under a hand-planted cover can change.
    let memory = super::soil::soil_memory_entity(world);
    let pieces = world
        .get::<&crate::ecs::components::SoilMemory>(memory)
        .ok()
        .and_then(|m| m.pests.get(area).and_then(|a| a.cover_pieces.get(&cover.id).copied()))
        .unwrap_or(0);
    let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");
    let item_name = items.and_then(|r| r.items.get(&cover.item).map(|d| d.name.clone())).unwrap_or_else(|| cover.item.clone());
    let mut back = String::new();
    if pieces > 0 && !creative {
        let stack = items.map_or(99, |r| r.max_stack_for(&cover.item)).max(1);
        let mut fitted = false;
        for (_e, (inv, _c)) in world.query_mut::<(&mut crate::systems::inventory::Inventory, &crate::ecs::components::Controllable)>() {
            let left = inv.add_item(&cover.item, pieces, stack);
            if left == 0 {
                fitted = true;
            } else {
                inv.remove_item(&cover.item, pieces - left);
            }
            break;
        }
        if !fitted {
            super::push_notice(
                data,
                format!("Your pack has no room for the {pieces} x {item_name} of the {}: make room and take it off again.", cover.name.to_lowercase()),
            );
            return;
        }
        back = format!(" and folded {pieces} x {item_name} back into your pack");
    }
    let memory = super::soil::soil_memory_entity(world);
    if let Ok(mut mem) = world.get::<&mut crate::ecs::components::SoilMemory>(memory) {
        if let Some(a) = mem.pests.get_mut(area) {
            a.releases.remove(&cover.id);
            a.cover_pieces.remove(&cover.id);
        }
    }
    log::info!("[Farming] {} off {area}: {pieces} pieces back", cover.id);
    super::push_notice(data, format!("You took the {} off {}{back}.", cover.name.to_lowercase(), place(area)));
}

/// One grow area's row-cover line for the Garden panel (2026-09-27).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoverRow {
    /// The grow area's tag (a crop's `tower_id`; "" for hand-planted crops).
    pub area: String,
    /// "No row cover", or "Row cover on · 350 garden days of wear left ·
    /// about 6 C warmer in sun", and a warning when a crop that needs bees is
    /// flowering under it.
    pub line: String,
    /// A crop that needs bees is in flower under the cover: take it off.
    pub warn: bool,
    /// Its buttons as (control id, label, note): the covers that can go on,
    /// or what takes off the one that is on.
    pub controls: Vec<(String, String, String)>,
}

/// The Garden panel's row-cover rows: one per soil area with a living crop,
/// and per area with a cover still on (an emptied bed, so it can be taken
/// off), sorted by area. None with pests off, or with no cover in the data.
pub fn cover_rows(
    world: &hecs::World,
    store: &crate::hot_reload::data_store::DataStore,
    data: &PestData,
) -> Vec<CoverRow> {
    use crate::ecs::components::{CropInstance, SoilMemory, STAGE_DEAD};
    let severity = store
        .get::<std::sync::Mutex<f32>>("garden_pest_severity")
        .and_then(|m| m.lock().ok().map(|v| *v))
        .unwrap_or(DEFAULT_PEST_SEVERITY);
    let covers: Vec<&ControlDef> = data.controls.iter().filter(|c| c.is_cover()).collect();
    let Some(soil) = store.get::<super::soil_ph::SoilPhData>("garden_soil_ph") else { return Vec::new() };
    if !(severity > 0.0) || covers.is_empty() {
        return Vec::new();
    }
    let pol = store.get::<super::pollination::PollinationData>(super::pollination::DATA_KEY);
    let registry = store.get::<super::PlantRegistry>("plant_registry");
    // Each area's living crops: (plant id, stage).
    let mut crops: std::collections::BTreeMap<String, Vec<(String, String)>> = std::collections::BTreeMap::new();
    {
        let mut q = world.query::<&CropInstance>();
        for (_, c) in q.iter() {
            if c.growth_stage != STAGE_DEAD {
                crops
                    .entry(c.tower_id.clone().unwrap_or_default())
                    .or_default()
                    .push((c.crop_def_id.clone(), c.growth_stage.clone()));
            }
        }
    }
    let on: HashMap<String, AreaPests> = {
        let mut q = world.query::<&SoilMemory>();
        let found = q.iter().next().map(|(_, m)| m.pests.clone()).unwrap_or_default();
        found
    };
    let mut areas: std::collections::BTreeSet<String> =
        crops.keys().filter(|a| soil.soil_for(a).is_some()).cloned().collect();
    for (a, st) in &on {
        if covers.iter().any(|c| st.releases.contains_key(&c.id)) {
            areas.insert(a.clone());
        }
    }
    areas
        .into_iter()
        .map(|area| {
            let st = on.get(&area);
            let laid = covers.iter().find_map(|c| st.and_then(|s| s.releases.get(&c.id)).map(|left| (*c, *left)));
            match laid {
                Some((c, left)) => {
                    let mut line = format!(
                        "{} on · {left:.0} garden days of wear left · about {:.0} C warmer in sun",
                        c.name, c.warms_day_c
                    );
                    // Flowering under it, with bees shut out.
                    let mut flowering: Vec<String> = crops
                        .get(&area)
                        .into_iter()
                        .flatten()
                        .filter(|(p, stage)| {
                            pol.and_then(|d| d.crop(p))
                                .map_or(false, |d| d.by == super::pollination::PollinatedBy::Insects && d.is_flowering(stage))
                        })
                        .map(|(p, _)| registry.and_then(|r| r.get(p).map(|d| d.name.to_lowercase())).unwrap_or_else(|| p.replace('_', " ")))
                        .collect();
                    flowering.sort();
                    flowering.dedup();
                    let warn = !flowering.is_empty();
                    if warn {
                        line.push_str(&format!(" · {} in flower under it: take it off so bees can reach them", and_list(&flowering)));
                    }
                    let controls = data
                        .controls
                        .iter()
                        .filter(|t| t.takes_off == c.id)
                        .map(|t| (t.id.clone(), t.name.clone(), t.note.clone()))
                        .collect();
                    CoverRow { area, line, warn, controls }
                }
                None => {
                    let line = format!("No {}", covers.first().map_or("cover".to_string(), |c| c.name.to_lowercase()));
                    let controls = covers.iter().map(|c| (c.id.clone(), c.name.clone(), c.note.clone())).collect();
                    CoverRow { area, line, warn: false, controls }
                }
            }
        })
        .collect()
}

/// The pests in one grow area, for the Garden panel: (pest, pressure 0..1,
/// told), the worst first, and the releases still working there with their
/// garden days left (a cover is not among them: it has its own row,
/// `cover_rows`). Empty when the area has none.
pub fn area_report<'a>(
    world: &hecs::World,
    data: &'a PestData,
    area: &str,
) -> (Vec<(&'a PestDef, f64, bool)>, Vec<(&'a ControlDef, f64)>) {
    let a = {
        let mut q = world.query::<&crate::ecs::components::SoilMemory>();
        let found = q.iter().next().and_then(|(_, m)| m.pests.get(area).cloned());
        found
    };
    let Some(a) = a else {
        return (Vec::new(), Vec::new());
    };
    let mut pests: Vec<(&PestDef, f64, bool)> = a
        .pressure
        .iter()
        .filter_map(|(id, s)| data.pest(id).map(|p| (p, s.level, s.told)))
        .collect();
    pests.sort_by(|x, y| y.1.total_cmp(&x.1));
    let releases = a
        .releases
        .iter()
        .filter_map(|(id, left)| data.control(id).filter(|c| !c.is_cover()).map(|c| (c, *left)))
        .collect();
    (pests, releases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::PestPressure;

    fn shipped() -> PestData {
        PestData::parse(PESTS_RON).expect("the shipped pests.ron parses")
    }

    fn indoors() -> CropConditions<'static> {
        CropConditions {
            outdoors: false,
            temp_c: 21.0,
            water_stressed: false,
            excess_n: false,
            damp: false,
            season: "spring",
            humidity: 0.6,
        }
    }

    /// Every host is a real plants.csv id, every control's item a real
    /// items.csv id that the player can get (sold by the vendor or made by a
    /// recipe), every control names real pests, and every pest has at least
    /// one control. Seen red by misspelling "potato" as "potatoe" in the
    /// data (the host test named it), and again for the diseases (2026-09-26)
    /// by misspelling gray mold's "blueberry" as "bluebery". It also checks
    /// the diseases are there with their humidity windows, and that what a
    /// control guards against, takes away the conditions of, or must not go
    /// on, is a real pest or plant. Since 2026-09-27 also: the whitefly and
    /// the thrips are there, what a cover keeps out is a real pest, a cover
    /// is sized by the ground and has something that takes it off, and a
    /// take-off control names a real cover. Seen red for those by misspelling
    /// the row cover's "whitefly" as "whitefy" (the excludes check named it)
    /// and its item as "floating_row_cover_1" (the items.csv check named it).
    #[test]
    fn the_shipped_pests_name_real_plants_items_and_pests() {
        let d = shipped();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let plants = crate::systems::farming::PlantRegistry::from_csv(
            &std::fs::read(root.join("data/plants.csv")).unwrap(),
        )
        .unwrap();
        let items = crate::systems::inventory::ItemRegistry::from_csv(
            &std::fs::read(root.join("data/items.csv")).unwrap(),
        )
        .unwrap();
        let goods = std::fs::read_to_string(root.join("data/trade_goods.ron")).unwrap();
        let recipes = std::fs::read_to_string(root.join("data/recipes.csv")).unwrap();
        let made = |item: &str| {
            recipes.lines().filter(|l| !l.starts_with('#')).any(|l| {
                l.split(',').nth(4).map_or(false, |outs| outs.split('|').any(|o| o.split(':').next() == Some(item)))
            })
        };
        assert!(d.pests.len() >= 5, "a handful of pests");
        // The diseases (2026-09-26): gray mold, powdery mildew and downy
        // mildew, each spreading in a humidity window.
        for id in ["gray_mold", "powdery_mildew", "downy_mildew"] {
            let p = d.pest(id).unwrap_or_else(|| panic!("{id} is in pests.ron"));
            assert!(p.disease, "{id} is a disease");
            assert!(p.favoured_by.iter().any(|c| matches!(c, Condition::Humidity { .. })), "{id} has a Humidity window");
        }
        for p in &d.pests {
            for c in &p.favoured_by {
                if let Condition::Humidity { min, max } = c {
                    assert!(0.0 <= *min && min < max && *max <= 1.0, "{}: a humidity window inside 0..1", p.id);
                }
            }
            if let Some(u) = p.unfavoured_rate {
                assert!(u > 0.0 && u < 1.0, "{}: its own unfavoured rate slows, never stops", p.id);
            }
            for h in &p.hosts {
                assert!(plants.get(h).is_some(), "{}: host '{h}' is not in plants.csv", p.id);
            }
            assert!(p.fold > 1.0 && p.fold_days > 0.0, "{} grows", p.id);
            assert!(p.max_health_loss > 0.0 && p.max_health_loss <= 0.8, "{}: no wipe-outs", p.id);
            assert!(p.no_host_half_life_days > 0.0 && p.arrival_per_day > 0.0, "{}", p.id);
            assert!(!d.controls_for(&p.id).is_empty(), "{} has a control", p.id);
        }
        // The greenhouse pests (2026-09-27).
        for id in ["whitefly", "thrips"] {
            let p = d.pest(id).unwrap_or_else(|| panic!("{id} is in pests.ron"));
            assert!(p.indoors && !p.disease, "{id} is an insect of the greenhouse");
        }
        let covers: Vec<&ControlDef> = d.controls.iter().filter(|c| c.is_cover()).collect();
        assert!(!covers.is_empty(), "a row cover is in pests.ron");
        for c in &covers {
            assert!(c.m2_per_item > 0.0 && !c.item.is_empty(), "{}: sized by the ground, from an item", c.id);
            assert!(c.needs_soil && c.lasts_days > 0.0, "{}: lies over soil and wears out", c.id);
            assert!(c.warms_day_c > c.warms_night_c && c.warms_night_c > 0.0, "{}: warmer in sun", c.id);
            assert!(d.controls.iter().any(|t| t.takes_off == c.id), "{}: something takes it off", c.id);
        }
        for c in &d.controls {
            for id in c
                .removes
                .keys()
                .chain(c.removes_per_day.keys())
                .chain(c.protects.keys())
                .chain(c.excludes.keys())
                .chain(c.prevents.iter())
            {
                assert!(d.pest(id).is_some(), "{}: '{id}' is not a pest", c.id);
            }
            for k in c.removes.values().chain(c.removes_per_day.values()).chain(c.protects.values()).chain(c.excludes.values()) {
                assert!(*k > 0.0 && *k < 1.0, "{}: a control never removes all or none", c.id);
            }
            if !c.takes_off.is_empty() {
                assert!(d.control(&c.takes_off).map_or(false, |t| t.is_cover()), "{} takes off a cover", c.id);
            }
            for p in &c.not_on {
                assert!(plants.get(p).is_some(), "{}: not_on '{p}' is not in plants.csv", c.id);
            }
            if !c.protects.is_empty() || !c.removes_per_day.is_empty() {
                assert!(c.lasts_days > 0.0, "{} works over days, so it says for how many", c.id);
            }
            for e in &c.ends {
                assert!(d.control(e).is_some(), "{} ends unknown control {e}", c.id);
            }
            if !c.item.is_empty() {
                assert!(items.items.contains_key(&c.item), "{}: item {} not in items.csv", c.id, c.item);
                let sold = goods.contains(&format!("id: \"{}\"", c.item));
                assert!(sold || made(&c.item), "{}: nothing sells or makes {}", c.id, c.item);
                assert!(
                    c.plants_per_item > 0 || c.m2_per_item > 0.0,
                    "{} says how many plants or square metres an item treats",
                    c.id
                );
            }
        }
    }

    /// The IPM order: every pest's controls come gentlest first (mechanical,
    /// then biological, then the least-toxic sprays), whatever order the
    /// file lists them in. The shipped file already lists them in that
    /// order, so the first check reverses it: a first red run that dropped
    /// the sort in `controls_for` stayed green against the shipped order,
    /// which is why. Seen red, with the reversal, by dropping the sort
    /// (spider mites then listed soap first). Since 2026-09-27 the row cover
    /// (cultural: keeping them out) leads the insects' ladders, and the
    /// whitefly's and the thrips' are pinned whole; seen red by declaring the
    /// row cover `Mechanical` (it then came after hosing off).
    #[test]
    fn controls_come_in_the_ipm_order() {
        let d = shipped();
        let mut reversed = d.clone();
        reversed.controls.reverse();
        let mites: Vec<&str> = reversed.controls_for("spider_mite").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(mites, ["row_cover", "hose_off", "predatory_mites", "soap_spray"]);
        let aphids: Vec<&str> = d.controls_for("aphid").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(aphids, ["row_cover", "hose_off", "soap_spray"]);
        let slugs: Vec<&str> = d.controls_for("slug").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(slugs, ["hand_pick", "iron_phosphate_bait"]);
        let whitefly: Vec<&str> = d.controls_for("whitefly").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(whitefly, ["row_cover", "hose_off", "yellow_sticky_cards", "encarsia", "soap_spray"]);
        let thrips: Vec<&str> = d.controls_for("thrips").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(thrips, ["row_cover", "yellow_sticky_cards", "blue_sticky_cards", "cucumeris_mites", "soap_spray"]);
        assert!(d.controls_for("gray_mold").iter().all(|c| !c.is_cover()), "a cover keeps insects off, not a mould");
        for p in &d.pests {
            let kinds: Vec<ControlKind> = d.controls_for(&p.id).iter().map(|c| c.kind).collect();
            assert!(kinds.windows(2).all(|w| w[0] <= w[1]), "{}: {kinds:?}", p.id);
        }
    }

    /// The cited growth rates: aphids 80-fold in 11 days (UC IPM), spider
    /// mites 70-fold in 6 (Iowa State). Seen red by using log10 in
    /// `growth_rate`.
    #[test]
    fn growth_rates_follow_the_cited_folds() {
        let d = shipped();
        let aphid = d.pest("aphid").unwrap();
        assert!((aphid.growth_rate() - 80f64.ln() / 11.0).abs() < 1e-12);
        assert!((aphid.growth_rate() - 0.398).abs() < 0.001, "{}", aphid.growth_rate());
        let mite = d.pest("spider_mite").unwrap();
        // Unchecked, with no arrivals, a 1% infestation is 70 times as bad
        // six days later (while still far from full).
        let p = grow(1e-4, mite.growth_rate(), 0.0, 6.0);
        assert!((p / 1e-4 - 70.0).abs() < 0.5, "70-fold in 6 days, got {}", p / 1e-4);
    }

    /// The exact logistic solution: slicing a stretch of time into ticks,
    /// or one tick crossing all of it, gives the same pressure, which is
    /// what lets a frame, a clock jump and a catch-up agree; it never passes
    /// 1; and with no arrivals nothing comes from nothing. Seen red by
    /// replacing it with one Euler step (a 30-day step overshot to 1).
    #[test]
    fn growth_is_exact_however_the_time_is_sliced() {
        let (r, a) = (0.4, 0.002);
        let once = grow(0.0, r, a, 30.0);
        let mut sliced = 0.0;
        for _ in 0..3000 {
            sliced = grow(sliced, r, a, 0.01);
        }
        assert!((once - sliced).abs() < 1e-9, "{once} vs {sliced}");
        assert!(once > 0.0 && once < 1.0, "{once}");
        assert_eq!(grow(0.0, r, 0.0, 30.0), 0.0, "no arrivals, no pest");
        assert!(grow(0.5, r, a, 1e6) <= 1.0);
        assert_eq!(grow(0.3, r, a, 0.0), 0.3);
        assert!((decay(0.8, 30.0, 30.0) - 0.4).abs() < 1e-12, "a half-life halves it");
    }

    /// Conditions: aphids indoors at 21 C are favoured by the warmth and
    /// held back without excess nitrogen; a caterpillar never lives indoors;
    /// a non-host feeds nothing. Seen red by making `favour` ignore the
    /// indoors / outdoors flags (the caterpillar then fed on an indoor kale).
    #[test]
    fn favour_follows_host_place_and_conditions() {
        let d = shipped();
        let u = d.unfavoured_rate;
        let aphid = d.pest("aphid").unwrap();
        assert_eq!(favour(aphid, "lettuce", &indoors(), u), u, "warm enough, not over-fed");
        let fed = CropConditions { excess_n: true, ..indoors() };
        assert_eq!(favour(aphid, "lettuce", &fed, u), 1.0, "over-fed: full speed");
        let cold = CropConditions { temp_c: 10.0, ..indoors() };
        assert_eq!(favour(aphid, "lettuce", &cold, u), u * u, "cold and not over-fed");
        assert_eq!(favour(aphid, "wheat", &fed, u), 0.0, "wheat is no aphid host here");
        let cat = d.pest("cabbage_caterpillar").unwrap();
        assert_eq!(favour(cat, "kale", &indoors(), u), 0.0, "no moths indoors");
        let out = CropConditions { outdoors: true, ..indoors() };
        assert_eq!(favour(cat, "kale", &out, u), 1.0, "outdoors in the growing season");
        let winter = CropConditions { season: "winter", ..out };
        assert_eq!(favour(cat, "kale", &winter, u), u);
    }

    /// A control takes its share off the pests it works on, names the ones
    /// it does not, and a soap spray kills a release of predatory mites.
    /// Seen red by leaving an ended release in place (`get` for `remove` in
    /// `apply_control`).
    #[test]
    fn a_control_removes_its_share_and_soap_ends_the_predators() {
        let d = shipped();
        let mut area = AreaPests::default();
        area.pressure.insert("aphid".into(), PestPressure { level: 0.4, told: true });
        area.pressure.insert("cabbage_caterpillar".into(), PestPressure { level: 0.2, told: true });
        let out = apply_control(&mut area, &d, d.control("predatory_mites").unwrap());
        assert!(out.released && area.releases.contains_key("predatory_mites"));
        let out = apply_control(&mut area, &d, d.control("soap_spray").unwrap());
        assert_eq!(out.changed.len(), 1);
        assert!((area.pressure["aphid"].level - 0.4 * 0.2).abs() < 1e-12, "soap takes 0.8 of the aphids");
        assert_eq!(out.untouched, ["cabbage_caterpillar"], "caterpillars are immune to soap");
        assert_eq!(out.ended, ["predatory_mites"], "CSU: beneficial mites are affected too");
        assert!(area.releases.is_empty());
        assert!((area.pressure["cabbage_caterpillar"].level - 0.2).abs() < 1e-12);
    }

    /// Pests stunt, never kill: the ceiling is 100 with none, falls with
    /// pressure on a host only, halves the damage in the gentle mode, and
    /// never goes below the floor. Seen red by dropping the floor clamp (the
    /// worse pests then capped a bean at 0.8).
    #[test]
    fn the_health_ceiling_follows_pressure_on_hosts_only() {
        let d = shipped();
        let mut area = AreaPests::default();
        assert_eq!(health_ceiling(&d, Some(&area), "tomato", 1.0), 100.0);
        area.pressure.insert("aphid".into(), PestPressure { level: 1.0, told: true });
        assert!((health_ceiling(&d, Some(&area), "tomato", 1.0) - 70.0).abs() < 1e-4, "aphids at their worst take 0.3");
        assert!((health_ceiling(&d, Some(&area), "tomato", 0.5) - 85.0).abs() < 1e-4, "gentle: half");
        assert_eq!(health_ceiling(&d, Some(&area), "wheat", 1.0), 100.0, "not a host");
        area.pressure.insert("spider_mite".into(), PestPressure { level: 1.0, told: true });
        area.pressure.insert("slug".into(), PestPressure { level: 1.0, told: true });
        // Three pests on a bean multiply: 0.7 x 0.5 x 0.7 of full health.
        assert!((health_ceiling(&d, Some(&area), "bean", 1.0) - 24.5).abs() < 1e-3);
        // Worse pests still stop at the floor.
        let mut worse = d.clone();
        for p in &mut worse.pests {
            p.max_health_loss = 0.8;
        }
        assert_eq!(health_ceiling(&worse, Some(&area), "bean", 1.0), PEST_HEALTH_FLOOR, "0.2 x 0.2 x 0.2 floors at 20");
        assert_eq!(health_ceiling(&d, None, "bean", 1.0), 100.0);
    }

    /// The greenhouse pests (2026-09-27) grow at their cited rates: the
    /// whitefly 75 daughters (half UNH's 150 eggs) a 29-day generation, the
    /// thrips 75 (half the low end of UMass's 150-300) a 21-day one; a
    /// whitefly thrives at the home's 21 C and is held back in the cold, a
    /// thrips needs over-feeding to run at full speed, and neither lives on
    /// wheat. Seen red by setting the whitefly's `fold` to 150 in pests.ron
    /// (the full egg count, sons and all).
    #[test]
    fn whitefly_and_thrips_grow_at_their_cited_rates() {
        let d = shipped();
        let u = d.unfavoured_rate;
        let wf = d.pest("whitefly").unwrap();
        assert!((wf.growth_rate() - 75f64.ln() / 29.0).abs() < 1e-12, "{}", wf.growth_rate());
        assert!((wf.growth_rate() - 0.149).abs() < 0.001, "{}", wf.growth_rate());
        let th = d.pest("thrips").unwrap();
        assert!((th.growth_rate() - 75f64.ln() / 21.0).abs() < 1e-12, "{}", th.growth_rate());
        // Unchecked, with no arrivals, a whitefly generation later there are
        // 75 times as many (far from full).
        let p = grow(1e-5, wf.growth_rate(), 0.0, 29.0);
        assert!((p / 1e-5 - 75.0).abs() < 0.5, "75-fold in a generation, got {}", p / 1e-5);
        assert_eq!(favour(wf, "tomato", &indoors(), u), 1.0, "a greenhouse at 21 C suits them");
        let cold = CropConditions { temp_c: 12.0, ..indoors() };
        assert_eq!(favour(wf, "tomato", &cold, u), u, "and the cold holds them back");
        assert_eq!(favour(th, "pepper", &indoors(), u), u, "thrips at a quarter speed on a well-fed pepper");
        let fed = CropConditions { excess_n: true, ..indoors() };
        assert_eq!(favour(th, "pepper", &fed, u), 1.0, "and at full speed over-fed (UC IPM)");
        assert_eq!(favour(wf, "wheat", &fed, u), 0.0);
        assert_eq!(favour(th, "wheat", &fed, u), 0.0);
    }

    /// A row cover keeps newcomers off and nothing more (UMD, UMN): over five
    /// days a bare stand of a host collects aphids from arrivals and a
    /// covered one about a tenth as many, while aphids already under a cover
    /// breed at their full rate, at least as fast as breeding alone makes
    /// them. Seen red twice: by leaving `shut` out of the arrivals in
    /// `step_area` (the covered stand then matched the bare one), and by
    /// applying it to the growth as well (the aphids already under the cover
    /// then barely grew).
    #[test]
    fn a_row_cover_keeps_newcomers_off_but_not_what_is_already_under_it() {
        let d = shipped();
        let aphid = d.pest("aphid").unwrap();
        let i = d.pests.iter().position(|p| p.id == "aphid").unwrap();
        let mut fav = vec![0.0; d.pests.len()];
        fav[i] = 1.0;
        let covered = || {
            let mut a = AreaPests::default();
            a.releases.insert("row_cover".into(), 365.0);
            a
        };
        let (mut bare, mut under) = (AreaPests::default(), covered());
        let mut seeded_under = covered();
        seeded_under.pressure.insert("aphid".into(), PestPressure { level: 0.1, told: true });
        for _ in 0..5 {
            for a in [&mut bare, &mut under, &mut seeded_under] {
                step_area(a, &d, &fav, 1, 1.0);
            }
        }
        let lvl = |a: &AreaPests| a.pressure.get("aphid").map_or(0.0, |s| s.level);
        // Already under it: they breed as if it were not there.
        let breeding_alone = grow(0.1, aphid.growth_rate(), 0.0, 5.0);
        assert!(lvl(&seeded_under) > 0.3, "they go on breeding under it: {}", lvl(&seeded_under));
        assert!(lvl(&seeded_under) >= breeding_alone, "{} vs {breeding_alone}", lvl(&seeded_under));
        // Newcomers: a tenth get in.
        assert!(lvl(&bare) > 0.01 && lvl(&bare) < 0.1, "a few percent from arrivals: {}", lvl(&bare));
        let ratio = lvl(&under) / lvl(&bare);
        assert!((ratio - 0.1).abs() < 0.01, "a tenth of the newcomers get in: {ratio}");
        assert!(under.releases.contains_key("row_cover"), "a cover stays on with no pests under it");
        assert!((under.releases["row_cover"] - 360.0).abs() < 1e-9, "and wears a day a day");
    }

    /// Under a row cover the crops are warmer: UMD's 10 F (the middle of 5
    /// to 15) in sun, the lightweight grade's 2 F otherwise, nothing with no
    /// cover or with pests off. Seen red by returning the day figure at night.
    #[test]
    fn a_row_cover_warms_the_crops_by_day_more_than_by_night() {
        let d = shipped();
        let mut a = AreaPests::default();
        assert_eq!(cover_warming(&d, Some(&a), true, 1.0), 0.0);
        assert_eq!(cover_warming(&d, None, true, 1.0), 0.0);
        a.releases.insert("row_cover".into(), 365.0);
        // A release that is not a cover warms nothing.
        a.releases.insert("predatory_mites".into(), 21.0);
        // 10 F and 2 F in Celsius, to the tenth pests.ron gives them.
        assert!((cover_warming(&d, Some(&a), true, 0.5) - 10.0 * 5.0 / 9.0).abs() < 0.05);
        assert!((cover_warming(&d, Some(&a), false, 0.5) - 2.0 * 5.0 / 9.0).abs() < 0.05);
        assert_eq!(cover_warming(&d, Some(&a), true, 0.0), 0.0, "pests off: no controls in play");
        let cover = d.control("row_cover").unwrap();
        assert_eq!(cover.items_for_ground(12.0), 1);
        assert_eq!(cover.items_for_ground(12.5), 2);
        assert_eq!(cover.items_for_ground(0.0), 1, "the smallest bed still takes a piece");
    }
}
