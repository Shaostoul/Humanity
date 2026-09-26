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
    /// By hand or with water: hand-picking, hosing off.
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
    #[serde(default)]
    pub note: String,
}

impl ControlDef {
    /// Does this control act on `pest` at all, at once or over days?
    pub fn works_on(&self, pest: &str) -> bool {
        self.removes.contains_key(pest) || self.removes_per_day.contains_key(pest)
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
}

fn met(cond: &Condition, c: &CropConditions) -> bool {
    match cond {
        Condition::Temperature { min_c, max_c } => c.temp_c >= *min_c && c.temp_c <= *max_c,
        Condition::ExcessNitrogen => c.excess_n,
        Condition::WaterStress => c.water_stressed,
        Condition::Damp => c.damp,
        Condition::Season(seasons) => c.season.is_empty() || seasons.iter().any(|s| s.eq_ignore_ascii_case(c.season)),
    }
}

/// How much one crop feeds `pest`'s growth in its area, 0..1: 0 if the pest
/// does not live there (indoors or out) or does not eat this plant, else 1
/// times `unfavoured` for every favouring condition the crop is not in.
pub fn favour(pest: &PestDef, plant: &str, c: &CropConditions, unfavoured: f64) -> f64 {
    let lives_here = if c.outdoors { pest.outdoors } else { pest.indoors };
    if !lives_here || !pest.hosts_plant(plant) {
        return 0.0;
    }
    pest.favoured_by
        .iter()
        .map(|cond| if met(cond, c) { 1.0 } else { unfavoured.clamp(0.0, 1.0) })
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
        st.level = if share > 0.0 {
            grow(st.level, pest.growth_rate() * share, pest.arrival_per_day * share, days)
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
    // Releases age, and end when their time is up or their prey is gone.
    let pressure = &area.pressure;
    area.releases.retain(|cid, left| {
        *left -= days;
        let Some(c) = data.control(cid) else { return false };
        let prey_left = c
            .removes_per_day
            .keys()
            .any(|p| pressure.get(p).map_or(false, |s| s.level >= PREY_GONE));
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
        } else if !control.removes_per_day.contains_key(&id) && st.level >= data.detect_at {
            out.untouched.push(id);
        }
    }
    if control.lasts_days > 0.0 && !control.removes_per_day.is_empty() {
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
pub fn appeared_notice(data: &PestData, pest: &PestDef, areas: &[String]) -> String {
    let wherever = match areas {
        [one] => format!("{} have appeared on {}.", pest.name, place(one)),
        many => format!("{} have appeared in {} grow areas.", pest.name, many.len()),
    };
    let controls: Vec<&str> = data.controls_for(&pest.id).iter().map(|c| c.name.as_str()).collect();
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
    if out.released {
        let prey: Vec<String> = control.removes_per_day.keys().map(|p| name_of(p)).collect();
        s.push_str(&format!(
            " They will work on the {} for up to {:.0} days, while there are any.",
            prey.join(" and "),
            control.lasts_days
        ));
    }
    if !out.untouched.is_empty() {
        let names: Vec<String> = out.untouched.iter().map(|p| name_of(p)).collect();
        s.push_str(&format!(" It does nothing to the {} there.", names.join(" or ")));
    }
    if !out.ended.is_empty() {
        let names: Vec<String> = out
            .ended
            .iter()
            .map(|e| data.control(e).map_or(e.clone(), |c| c.name.to_lowercase()))
            .collect();
        s.push_str(&format!(" It also killed what you released there ({}).", names.join(", ")));
    }
    s
}

// -- The control request, from the "pest_control_request" channel ------------------

/// Apply one player control request `(area, control id)` (2026-09-26): take
/// its items from the backpack (free in creative mode) and its water from the
/// home tanks, then act on the area's pests and tell the player what it did.
/// Refused, with a notice, when the area has no crops, the tanks are empty for
/// a water control, or the backpack lacks the items; nothing is spent then.
pub(super) fn handle_request(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    pests: &PestData,
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
    let plants = world
        .query::<&CropInstance>()
        .iter()
        .filter(|(_, c)| c.growth_stage != STAGE_DEAD && c.tower_id.as_deref().unwrap_or("") == area)
        .count();
    if plants == 0 {
        super::push_notice(data, format!("There are no crops to treat in {}.", area.replace('_', " ")));
        return;
    }
    let water_l = control.water_l_per_plant.max(0.0) * plants as f64;
    if water_l > 0.0 && !water_available {
        super::push_notice(data, format!("The water tanks are empty: nothing to {} with.", control.name.to_lowercase()));
        return;
    }
    let need = control.items_for(plants);
    if need > 0 && !creative {
        let item_name = data
            .get::<crate::systems::inventory::ItemRegistry>("item_registry")
            .and_then(|r| r.items.get(&control.item).map(|d| d.name.clone()))
            .unwrap_or_else(|| control.item.clone());
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
            super::push_notice(
                data,
                format!(
                    "{} needs {need} x {item_name} for the {plants} plants in {}; you have {have}.",
                    control.name,
                    area.replace('_', " ")
                ),
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
    let out = match world.get::<&mut crate::ecs::components::SoilMemory>(memory) {
        Ok(mut mem) => apply_control(mem.pests.entry(area.to_string()).or_default(), pests, control),
        Err(_) => return,
    };
    log::info!("[Farming] {} on {area}: {:?}", control.id, out);
    super::push_notice(data, outcome_notice(pests, control, area, plants, water_l, &out));
}

/// The pests in one grow area, for the Garden panel: (pest, pressure 0..1,
/// told), the worst first, and the releases still working there with their
/// garden days left. Empty when the area has none.
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
    let releases = a.releases.iter().filter_map(|(id, left)| data.control(id).map(|c| (c, *left))).collect();
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
        }
    }

    /// Every host is a real plants.csv id, every control's item a real
    /// items.csv id that the player can get (sold by the vendor or made by a
    /// recipe), every control names real pests, and every pest has at least
    /// one control. Seen red by misspelling "potato" as "potatoe" in the
    /// data (the host test named it).
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
        for p in &d.pests {
            for h in &p.hosts {
                assert!(plants.get(h).is_some(), "{}: host '{h}' is not in plants.csv", p.id);
            }
            assert!(p.fold > 1.0 && p.fold_days > 0.0, "{} grows", p.id);
            assert!(p.max_health_loss > 0.0 && p.max_health_loss <= 0.8, "{}: no wipe-outs", p.id);
            assert!(p.no_host_half_life_days > 0.0 && p.arrival_per_day > 0.0, "{}", p.id);
            assert!(!d.controls_for(&p.id).is_empty(), "{} has a control", p.id);
        }
        for c in &d.controls {
            for id in c.removes.keys().chain(c.removes_per_day.keys()) {
                assert!(d.pest(id).is_some(), "{}: '{id}' is not a pest", c.id);
            }
            for k in c.removes.values().chain(c.removes_per_day.values()) {
                assert!(*k > 0.0 && *k < 1.0, "{}: a control never removes all or none", c.id);
            }
            for e in &c.ends {
                assert!(d.control(e).is_some(), "{} ends unknown control {e}", c.id);
            }
            if !c.item.is_empty() {
                assert!(items.items.contains_key(&c.item), "{}: item {} not in items.csv", c.id, c.item);
                let sold = goods.contains(&format!("id: \"{}\"", c.item));
                assert!(sold || made(&c.item), "{}: nothing sells or makes {}", c.id, c.item);
                assert!(c.plants_per_item > 0, "{} says how many plants an item treats", c.id);
            }
        }
    }

    /// The IPM order: every pest's controls come gentlest first (mechanical,
    /// then biological, then the least-toxic sprays), whatever order the
    /// file lists them in. The shipped file already lists them in that
    /// order, so the first check reverses it: a first red run that dropped
    /// the sort in `controls_for` stayed green against the shipped order,
    /// which is why. Seen red, with the reversal, by dropping the sort
    /// (spider mites then listed soap first).
    #[test]
    fn controls_come_in_the_ipm_order() {
        let d = shipped();
        let mut reversed = d.clone();
        reversed.controls.reverse();
        let mites: Vec<&str> = reversed.controls_for("spider_mite").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(mites, ["hose_off", "predatory_mites", "soap_spray"]);
        let aphids: Vec<&str> = d.controls_for("aphid").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(aphids, ["hose_off", "soap_spray"]);
        let slugs: Vec<&str> = d.controls_for("slug").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(slugs, ["hand_pick", "iron_phosphate_bait"]);
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
}
