//! Soil pH (2026-09-26, gardening depth: the next rung of the soil model).
//!
//! plants.csv has always carried each crop's `ph_min`/`ph_max` window, and
//! nothing used it. The media, the rates, every number and its source live in
//! data/garden/soil_ph.ron; this file is the model. It sits beside the N-P-K
//! model in soil.rs and follows the pests' shape (pests.rs): the state lives
//! on the world's `SoilMemory`, the player acts through a request channel.
//!
//! THE LOOP:
//!
//! 1. Each unit of a SOIL grow area (a bed, tray or field unit) has a pH,
//!    `SoilMemory::ph`, that belongs to the unit, not the crop: it is
//!    remembered between crops and saved. A unit nobody has touched reads its
//!    medium's starting pH (6.5, a bed prepared the way the guides say). A
//!    tower's nutrient solution is HELD at its setpoint by the tower's dosing
//!    and never drifts; a mushroom rack's substrate is not modelled.
//! 2. Everything that moves a pH is counted as grams of calcium carbonate
//!    equivalent and turned into pH units by the medium's buffer and the
//!    unit's area (`ph_shift`, `unit_area_m2`). It does not act at once: it
//!    joins a pending pool (`UnitPh::pending`) that reacts first-order on
//!    garden days, each pool at its own half-life (`step_unit`).
//! 3. NITROGEN acidifies: each gram of plant-available N the feeder or the
//!    Fertilize button puts in brings the cited CaCO3 equivalent of acidity
//!    for its fertilizer (stored urine as urea, 1.8 g per g of N; compost 0)
//!    into the "nitrification" pool (`add_n`), so a unit fed urine season after
//!    season slowly turns acid, the way a real bed does.
//! 4. The player corrects it with LIME (raises) or SULFUR (lowers), per grow
//!    area, through the "soil_ph_request" channel (`handle_request`). Each
//!    button brings every planted unit toward the middle of its crop's window,
//!    counting what is still reacting, so a second press does not double the
//!    dose; sulfur is capped per application, as the guides advise.
//! 5. A crop whose unit is outside its window has its health CAPPED
//!    (`health_ceiling`): 30% of the harvest per pH unit outside, the median of
//!    the NRCS relative-yield table, never below the same floor a nutrient
//!    shortage and a pest have. The farming tick takes the lowest of the
//!    nutrient, pest and pH ceilings. The player is told once per grow area
//!    when its soil leaves a crop's window, and which amendment fixes it.
//!
//! TWO MODES (the house rule for a deep system): the "garden_soil_ph_on"
//! channel, from Settings "Soil pH: Off / On". Off freezes all of it: no
//! drift, no cap, no amendments, and the Garden panel does not show pH.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::Deserialize;

use crate::ecs::components::{CropInstance, SoilMemory, UnitPh, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;

use super::PlantDef;

/// The shipped copy, so a bare exe with no data folder still has the model.
pub const SOIL_PH_RON: &str = include_str!("../../../data/garden/soil_ph.ron");

/// Soil pH is on unless Settings turns it off. Absent channel (headless
/// tests, early boot) reads as this, so a test that never publishes it sees
/// what players get.
pub const DEFAULT_SOIL_PH_ON: bool = true;

/// The pending pool for the acidity of newly added ammonium.
pub const NITRIFICATION: &str = "nitrification";

/// A pool this small has finished reacting and is dropped.
const FORGET_BELOW: f64 = 1e-9;

/// Every unit's pH, keyed by grow-area tag then unit index, as in
/// `SoilMemory::ph`.
pub type PhUnits = HashMap<String, HashMap<u32, UnitPh>>;

// -- Data: data/garden/soil_ph.ron -------------------------------------------------

/// What data/garden/soil_ph.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct SoilPhData {
    pub media: Vec<PhMedium>,
    pub default_medium: String,
    pub ph_floor: f64,
    pub ph_ceiling: f64,
    #[serde(default)]
    pub fertilizer_acidity: Vec<FertilizerAcidity>,
    pub nitrification_half_life_days: f64,
    pub reference_n_removal_g_per_m2: f64,
    pub default_unit_area_m2: f64,
    #[serde(default)]
    pub amendments: Vec<Amendment>,
    pub loss_per_ph_unit: f64,
    pub health_floor: f32,
    /// The cited table `loss_per_ph_unit` is the median of (a test checks).
    #[serde(default)]
    pub yield_table_ph: Vec<f64>,
    #[serde(default)]
    pub yield_table: Vec<YieldRow>,
}

/// One kind of grow area (soil_ph.ron `media`).
#[derive(Debug, Clone, Deserialize)]
pub struct PhMedium {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub match_exact: Vec<String>,
    #[serde(default)]
    pub match_prefix: Vec<String>,
    /// Held at this pH by the grower: never drifts, no amendments, no cap.
    #[serde(default)]
    pub held_ph: Option<f64>,
    /// A soil: its starting pH and its buffer.
    #[serde(default)]
    pub soil: Option<SoilDef>,
}

/// A soil medium's pH behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct SoilDef {
    pub start_ph: f64,
    /// Grams of calcium carbonate per m2 that move the root zone one pH unit.
    pub buffer_g_caco3_per_m2: f64,
}

/// The acidity one fertilizer's nitrogen brings (soil_ph.ron
/// `fertilizer_acidity`), grams of CaCO3 per gram of plant-available N.
#[derive(Debug, Clone, Deserialize)]
pub struct FertilizerAcidity {
    pub item: String,
    pub caco3_g_per_g_n: f64,
}

/// A soil amendment the player applies (soil_ph.ron `amendments`).
#[derive(Debug, Clone, Deserialize)]
pub struct Amendment {
    pub id: String,
    pub name: String,
    pub item: String,
    pub caco3_g_per_g_pure: f64,
    #[serde(default = "pure")]
    pub purity: f64,
    pub half_life_days: f64,
    /// Most of the PURE material per m2 in one application (None: no cap).
    #[serde(default)]
    pub max_g_per_m2: Option<f64>,
}

fn pure() -> f64 {
    1.0
}

impl Amendment {
    /// Grams of CaCO3 equivalent per gram of the item: positive raises the
    /// pH (lime, 1.0), negative lowers it (90% sulfur, -2.81).
    pub fn caco3_per_g(&self) -> f64 {
        self.caco3_g_per_g_pure * self.purity.clamp(0.0, 1.0)
    }

    pub fn raises(&self) -> bool {
        self.caco3_per_g() > 0.0
    }
}

/// One crop's row of the cited relative-yield table.
#[derive(Debug, Clone, Deserialize)]
pub struct YieldRow {
    pub plant: String,
    pub relative_yield: Vec<f64>,
}

impl SoilPhData {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The data folder's copy first (so it can be modded), the shipped copy
    /// if that is missing or does not parse. The shipped copy always parses:
    /// a test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("soil_ph.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::parse(SOIL_PH_RON).expect("the shipped data/garden/soil_ph.ron parses")
    }

    /// The medium a grow-area tag grows in: an exact name first, then a
    /// prefix, then `default_medium` (beds, trays, fields and "", the
    /// hand-planted crops).
    pub fn medium_for(&self, area: &str) -> Option<&PhMedium> {
        self.media
            .iter()
            .find(|m| m.match_exact.iter().any(|e| e == area))
            .or_else(|| {
                self.media
                    .iter()
                    .find(|m| m.match_prefix.iter().any(|p| !p.is_empty() && area.starts_with(p.as_str())))
            })
            .or_else(|| self.media.iter().find(|m| m.id == self.default_medium))
    }

    /// The soil an area grows in, if it is a soil.
    pub fn soil_for(&self, area: &str) -> Option<SoilDef> {
        self.medium_for(area).and_then(|m| m.soil)
    }

    pub fn amendment(&self, id: &str) -> Option<&Amendment> {
        self.amendments.iter().find(|a| a.id == id)
    }

    /// Grams of CaCO3 of acidity per gram of N from fertilizer `item` (0 for
    /// one the file does not list).
    pub fn acidity_per_g_n(&self, item: &str) -> f64 {
        self.fertilizer_acidity
            .iter()
            .find(|f| f.item == item)
            .map_or(0.0, |f| f.caco3_g_per_g_n.max(0.0))
    }

    /// The half-life of a pending pool, garden days (0: reacts at once).
    fn half_life(&self, pool: &str) -> f64 {
        if pool == NITRIFICATION {
            self.nitrification_half_life_days
        } else {
            self.amendment(pool).map_or(0.0, |a| a.half_life_days)
        }
    }
}

// -- The model ------------------------------------------------------------------------

/// The soil area, m2, a unit's pH changes are spread over, for a crop of
/// `def` whose season N need (what it draws from the unit,
/// `farming::crop_season_need`) is `need_n` grams.
///
/// `plot_m2` is the plot's real floor area when the unit is a plot of a
/// placed bed, tray or field (2026-09-26: the engine publishes it as
/// "grow_plot_area_m2", see `plot_area`), and is used as it is. Without one
/// (a test, or a unit with no machine) the area is INFERRED from the N the
/// crop's harvest removes, taking every crop to remove about what the
/// nutrient model's anchor tomato does per m2 (soil_ph.ron
/// `reference_n_removal_g_per_m2`, a game estimate for the rest). A legume
/// draws only the share of its N it does not fix, so its removal is its draw
/// over that share. A crop with no N need takes `default_unit_area_m2`.
pub fn unit_area_m2(data: &SoilPhData, plot_m2: Option<f64>, def: Option<&PlantDef>, need_n: f64) -> f64 {
    if let Some(a) = plot_m2.filter(|a| a.is_finite() && *a > 0.0) {
        return a;
    }
    let fixed = def.map_or(0.0, |d| f64::from(d.n_fixed_share).clamp(0.0, 1.0));
    let removal = if fixed < 1.0 { need_n / (1.0 - fixed) } else { 0.0 };
    if removal > 0.0 && data.reference_n_removal_g_per_m2 > 0.0 {
        removal / data.reference_n_removal_g_per_m2
    } else {
        data.default_unit_area_m2.max(1e-6)
    }
}

/// The floor area of one plot of grow area `area` (a machine instance or
/// type id), m2, from the map the engine publishes ("grow_plot_area_m2");
/// None for towers, hand-planted crops and anything unplaced.
pub fn plot_area(store: &DataStore, area: &str) -> Option<f64> {
    store
        .get::<HashMap<String, f32>>("grow_plot_area_m2")
        .and_then(|m| m.get(area))
        .map(|a| f64::from(*a))
}

/// pH units that `caco3_g` grams of calcium carbonate equivalent (negative:
/// acidity) move a unit of `area_m2` in this soil. Linear, the way the lime
/// requirement tables are read (a rate per pH unit).
pub fn ph_shift(soil: &SoilDef, area_m2: f64, caco3_g: f64) -> f64 {
    let per_unit = soil.buffer_g_caco3_per_m2 * area_m2.max(1e-6);
    if per_unit > 0.0 {
        caco3_g / per_unit
    } else {
        0.0
    }
}

/// Step one unit `days` garden days: each pending pool reacts first-order at
/// its half-life, so a stretch of time gives the same pH however it is sliced
/// into ticks. The pH stays inside the file's floor and ceiling (lime past the
/// ceiling stays in the soil as undissolved carbonate).
pub fn step_unit(u: &mut UnitPh, data: &SoilPhData, days: f64) {
    if !(days > 0.0) || u.pending.is_empty() {
        return;
    }
    for (pool, left) in u.pending.iter_mut() {
        let h = data.half_life(pool);
        let reacted = if h > 0.0 { *left * (1.0 - 0.5f64.powf(days / h)) } else { *left };
        u.ph += reacted;
        *left -= reacted;
    }
    u.ph = u.ph.clamp(data.ph_floor, data.ph_ceiling);
    u.pending.retain(|_, v| v.abs() > FORGET_BELOW);
}

/// Step every remembered unit `days` garden days.
pub fn step_all(units: &mut PhUnits, data: &SoilPhData, days: f64) {
    for slots in units.values_mut() {
        for u in slots.values_mut() {
            step_unit(u, data, days);
        }
    }
}

/// A soil unit's state, created at its medium's starting pH the first time it
/// is needed. None when the area is not soil (a held tower, a mushroom rack).
pub fn unit_mut<'a>(units: &'a mut PhUnits, data: &SoilPhData, area: &str, slot: u32) -> Option<&'a mut UnitPh> {
    let soil = data.soil_for(area)?;
    Some(
        units
            .entry(area.to_string())
            .or_default()
            .entry(slot)
            .or_insert_with(|| UnitPh { ph: soil.start_ph, pending: HashMap::new() }),
    )
}

/// Add the acidity of `n_g` grams of plant-available N from fertilizer
/// `item` to unit `slot` of `area`, whose crop of `def` needs `need_n` grams
/// of N a season (for its area, `unit_area_m2`). It joins the nitrification
/// pool and reaches the pH as the ammonium nitrifies. Nothing happens for a
/// fertilizer with no acidity (compost) or an area that is not soil.
#[allow(clippy::too_many_arguments)]
pub fn add_n(
    units: &mut PhUnits,
    data: &SoilPhData,
    area: &str,
    slot: u32,
    item: &str,
    n_g: f64,
    def: Option<&PlantDef>,
    need_n: f64,
    plot_m2: Option<f64>,
) {
    let per = data.acidity_per_g_n(item);
    if !(per > 0.0) || !(n_g > 0.0) {
        return;
    }
    let Some(soil) = data.soil_for(area) else { return };
    let shift = -ph_shift(&soil, unit_area_m2(data, plot_m2, def, need_n), per * n_g);
    if let Some(u) = unit_mut(units, data, area, slot) {
        *u.pending.entry(NITRIFICATION.to_string()).or_default() += shift;
    }
}

/// `add_n` straight into the world's `SoilMemory` (the Fertilize button,
/// which runs outside the crop loop).
#[allow(clippy::too_many_arguments)]
pub fn add_n_in(
    world: &mut hecs::World,
    data: &SoilPhData,
    area: &str,
    slot: u32,
    item: &str,
    n_g: f64,
    def: Option<&PlantDef>,
    need_n: f64,
    plot_m2: Option<f64>,
) {
    let e = super::soil::soil_memory_entity(world);
    if let Ok(mut mem) = world.get::<&mut SoilMemory>(e) {
        add_n(&mut mem.ph, data, area, slot, item, n_g, def, need_n, plot_m2);
    }
}

/// How far `ph` lies outside `lo..=hi`, pH units (0 inside).
pub fn outside(ph: f64, lo: f64, hi: f64) -> f64 {
    if ph < lo {
        lo - ph
    } else if ph > hi {
        ph - hi
    } else {
        0.0
    }
}

/// The highest health a crop with window `lo..=hi` can hold at `ph`: 100
/// inside it, `loss_per_ph_unit` less per pH unit outside, never below
/// `health_floor`. A crop with no window (both 0, or reversed) is not capped.
pub fn health_ceiling(data: &SoilPhData, ph: f64, lo: f64, hi: f64) -> f32 {
    if !(hi > lo) || hi <= 0.0 {
        return 100.0;
    }
    let keep = 1.0 - data.loss_per_ph_unit.max(0.0) * outside(ph, lo, hi);
    ((100.0 * keep) as f32).clamp(data.health_floor.clamp(0.0, 100.0), 100.0)
}

/// The pH a crop in `area` (unit `slot`) sees, and whether it is held: None
/// when the medium is not modelled; the setpoint for a held medium; else the
/// unit's remembered pH, or the soil's starting pH when nothing is remembered
/// (and always for a hand-planted crop, which has no unit to remember it).
pub fn crop_ph(units: &PhUnits, data: &SoilPhData, area: &str, slot: Option<u32>) -> Option<(f64, bool)> {
    let medium = data.medium_for(area)?;
    if let Some(held) = medium.held_ph {
        return Some((held, true));
    }
    let soil = medium.soil?;
    let remembered = slot.and_then(|s| units.get(area).and_then(|m| m.get(&s))).map(|u| u.ph);
    Some((remembered.unwrap_or(soil.start_ph), false))
}

/// The pH health ceiling for one crop (100 when pH is off, held, not
/// modelled, or the plant is unknown).
pub fn crop_ceiling(
    units: &PhUnits,
    data: &SoilPhData,
    on: bool,
    area: &str,
    slot: Option<u32>,
    def: Option<&PlantDef>,
) -> f32 {
    if !on {
        return 100.0;
    }
    match (def, crop_ph(units, data, area, slot)) {
        (Some(d), Some((ph, false))) => health_ceiling(data, ph, f64::from(d.ph_min), f64::from(d.ph_max)),
        _ => 100.0,
    }
}

/// `crop_ceiling`, and the crop's pH and window when it is outside it (for
/// the notice; None inside, or when the ceiling does not apply).
pub fn check_crop(
    units: &PhUnits,
    data: &SoilPhData,
    on: bool,
    area: &str,
    slot: Option<u32>,
    def: Option<&PlantDef>,
) -> (f32, Option<OutOfWindow>) {
    let cap = crop_ceiling(units, data, on, area, slot, def);
    if cap >= 100.0 {
        return (cap, None);
    }
    let out = match (def, crop_ph(units, data, area, slot)) {
        (Some(d), Some((ph, _))) => Some(OutOfWindow {
            plant: d.name.clone(),
            ph,
            lo: f64::from(d.ph_min),
            hi: f64::from(d.ph_max),
        }),
        _ => None,
    };
    (cap, out)
}

/// Is soil pH on (the "garden_soil_ph_on" channel, from Settings)?
pub fn is_on(data: &DataStore) -> bool {
    data.get::<Mutex<bool>>("garden_soil_ph_on")
        .and_then(|m| m.lock().ok().map(|v| *v))
        .unwrap_or(DEFAULT_SOIL_PH_ON)
}

// -- What the player is told, and the request channel -------------------------------

/// The farming system's pH bookkeeping that is not saved: the grow areas the
/// player has been told are out of their crops' window (so it is said once),
/// and the grams left in each opened bag of an amendment.
#[derive(Debug, Default)]
pub struct PhRuntime {
    told: HashSet<String>,
    open_g: HashMap<String, f64>,
}

/// One crop outside its window this tick, for the notice.
#[derive(Debug, Clone)]
pub struct OutOfWindow {
    pub plant: String,
    pub ph: f64,
    pub lo: f64,
    pub hi: f64,
}

fn soil_of(area: &str) -> String {
    if area.is_empty() {
        "the soil your hand-planted crops grow in".to_string()
    } else {
        format!("the soil in {}", area.replace('_', " "))
    }
}

impl PhRuntime {
    /// Say once per grow area when its soil has left a crop's window, and
    /// which amendment fixes it; an area leaves the told set when all its
    /// crops are back inside. `out_now` is this tick's first out-of-window
    /// crop per area.
    pub fn tell(&mut self, data: &DataStore, ph: &SoilPhData, out_now: &HashMap<String, OutOfWindow>) {
        self.told.retain(|a| out_now.contains_key(a));
        let mut areas: Vec<&String> = out_now.keys().filter(|a| !self.told.contains(*a)).collect();
        areas.sort();
        for area in areas {
            let o = &out_now[area];
            let acid = o.ph < o.lo;
            let fix = ph
                .amendments
                .iter()
                .find(|a| a.raises() == acid)
                .map_or(String::new(), |a| format!(" Use {} in the Garden panel.", a.name));
            let word = if acid { "acid" } else { "alkaline" };
            let why = if acid {
                " Nitrogen fertilizer, stored urine too, acidifies soil as it turns to nitrate.".to_string()
            } else {
                String::new()
            };
            super::push_notice(
                data,
                format!(
                    "{} is too {word} for its {}: pH {:.1}, and it grows best at {:.1} to {:.1}.{fix}{why}",
                    capitalize(&soil_of(area)),
                    o.plant.to_lowercase(),
                    o.ph,
                    o.lo,
                    o.hi
                ),
            );
            self.told.insert(area.clone());
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Apply one player request `(area, amendment id)` (2026-09-26): bring every
/// planted unit of the area toward the middle of its crop's window with the
/// amendment, counting what is still reacting, take the bags from the
/// backpack (free in creative mode), and say what was done and how long it
/// takes. Refused, with a notice and nothing spent, when the area is not
/// soil, has no planted units, needs none of it, or the backpack lacks the
/// bags. `need_n` gives a plant's season N need (for its unit's area).
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_request(
    world: &mut hecs::World,
    data: &DataStore,
    ph: &SoilPhData,
    rt: &mut PhRuntime,
    area: &str,
    amendment_id: &str,
    creative: bool,
    need_n: &mut dyn FnMut(&str) -> f64,
) {
    let Some(am) = ph.amendment(amendment_id) else {
        log::warn!("[Farming] no soil amendment '{amendment_id}' in data/garden/soil_ph.ron");
        return;
    };
    let place = soil_of(area);
    let Some(medium) = ph.medium_for(area) else { return };
    let Some(soil) = medium.soil else {
        let why = match medium.held_ph {
            Some(p) => format!("its pH is held at {p:.1} by the tower's dosing"),
            None => "it is not soil".to_string(),
        };
        super::push_notice(data, format!("{} is for soil, and {} is {}: {why}.", am.name, area.replace('_', " "), medium.name.to_lowercase()));
        return;
    };
    let plants = data.get::<super::PlantRegistry>("plant_registry");
    let crops: Vec<(u32, String)> = world
        .query::<&CropInstance>()
        .iter()
        .filter(|(_, c)| c.growth_stage != STAGE_DEAD && c.tower_id.as_deref().unwrap_or("") == area)
        .filter_map(|(_, c)| c.tower_slot.map(|s| (s, c.crop_def_id.clone())))
        .collect();
    if crops.is_empty() {
        super::push_notice(data, format!("There are no planted units to treat in {}.", area.replace('_', " ")));
        return;
    }
    let units = world
        .query::<&SoilMemory>()
        .iter()
        .next()
        .map(|(_, m)| m.ph.get(area).cloned().unwrap_or_default())
        .unwrap_or_default();
    // The plan: per unit, grams of the item and the pH it will move.
    let per_g = am.caco3_per_g();
    let mut plan: Vec<(u32, f64, f64, f64)> = Vec::new(); // (slot, grams, shift, pH after)
    let mut capped = false;
    for (slot, plant) in &crops {
        let Some(def) = plants.and_then(|r| r.get(plant)) else { continue };
        let (lo, hi) = (f64::from(def.ph_min), f64::from(def.ph_max));
        if !(hi > lo) || per_g == 0.0 {
            continue;
        }
        let target = ((lo + hi) / 2.0).clamp(ph.ph_floor, ph.ph_ceiling);
        let now = units.get(slot).map_or(soil.start_ph, |u| u.ph + u.pending.values().sum::<f64>());
        let want = if am.raises() { target - now } else { now - target };
        if want <= 0.005 {
            continue;
        }
        let a = unit_area_m2(ph, plot_area(data, area), Some(def), need_n(plant));
        let mut grams = want * soil.buffer_g_caco3_per_m2 * a / per_g.abs();
        if let Some(max) = am.max_g_per_m2 {
            let limit = max * a / am.purity.clamp(1e-6, 1.0);
            if grams > limit {
                grams = limit;
                capped = true;
            }
        }
        let shift = ph_shift(&soil, a, grams * per_g);
        plan.push((*slot, grams, shift, now + shift));
    }
    if plan.is_empty() {
        let dir = if am.raises() { "up to" } else { "down to" };
        super::push_notice(
            data,
            format!(
                "{} is already {dir} the pH its crops want (counting what is still reacting): {} would take it past their window.",
                capitalize(&place),
                am.name.to_lowercase()
            ),
        );
        return;
    }
    let total_g: f64 = plan.iter().map(|p| p.1).sum();
    let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");
    let item_name = items.and_then(|r| r.items.get(&am.item).map(|d| d.name.clone())).unwrap_or_else(|| am.item.clone());
    if !creative {
        let bag_g = items.map_or(0.0, |r| f64::from(r.mass_for(&am.item)) * 1000.0).max(1.0);
        let open = rt.open_g.get(&am.item).copied().unwrap_or(0.0);
        let bags = ((total_g - open).max(0.0) / bag_g).ceil() as u32;
        let mut have = 0;
        let mut took = bags == 0;
        if bags > 0 {
            for (_e, (inv, _ctrl)) in world.query_mut::<(
                &mut crate::systems::inventory::Inventory,
                &crate::ecs::components::Controllable,
            )>() {
                have = inv.count_item(&am.item);
                if have >= bags {
                    inv.remove_item(&am.item, bags);
                    took = true;
                }
                break;
            }
        }
        if !took {
            super::push_notice(
                data,
                format!(
                    "{} for {} takes {:.2} kg, {bags} x {item_name}; you have {have}.",
                    am.name,
                    place,
                    total_g / 1000.0
                ),
            );
            return;
        }
        rt.open_g.insert(am.item.clone(), open + f64::from(bags) * bag_g - total_g);
    }
    let e = super::soil::soil_memory_entity(world);
    if let Ok(mut mem) = world.get::<&mut SoilMemory>(e) {
        for (slot, _g, shift, _) in &plan {
            if let Some(u) = unit_mut(&mut mem.ph, ph, area, *slot) {
                *u.pending.entry(am.id.clone()).or_default() += shift;
            }
        }
    }
    let lo = plan.iter().map(|p| p.3).fold(f64::INFINITY, f64::min);
    let hi = plan.iter().map(|p| p.3).fold(f64::NEG_INFINITY, f64::max);
    let toward = if hi - lo < 0.05 { format!("pH {lo:.1}") } else { format!("pH {lo:.1} to {hi:.1}") };
    let mut s = format!(
        "{} on {place}: {:.2} kg of {} for {} unit{}, toward {toward}. It works slowly: about half of it has reacted after {:.0} garden days.",
        am.name,
        total_g / 1000.0,
        item_name.to_lowercase(),
        plan.len(),
        if plan.len() == 1 { "" } else { "s" },
        am.half_life_days
    );
    if capped {
        s.push_str(&format!(
            " That is as much {} as one application should give (Oregon State: two small applications a year apart are better than one large one); give it again once this has worked.",
            am.name.to_lowercase()
        ));
    }
    log::info!("[Farming] {} on {area}: {:.1} g over {} units", am.id, total_g, plan.len());
    super::push_notice(data, s);
}

// -- The request and Settings channels, and the Garden panel's view ------------------

/// Put the pH channels in the DataStore (lib.rs, at boot): the data, the
/// Settings switch, and the Garden panel's Lime / Sulfur request.
pub fn register(store: &mut DataStore) {
    store.insert("garden_soil_ph", SoilPhData::load());
    store.insert("garden_soil_ph_on", Mutex::new(DEFAULT_SOIL_PH_ON));
    store.insert("soil_ph_request", Mutex::new(Option::<(String, String)>::None));
}

/// Carry this frame's Settings switch and the panel's chosen amendment
/// `(area, amendment id)` to the farming system (lib.rs, every frame).
pub fn bridge(store: &DataStore, on: bool, request: Option<(String, String)>) {
    if let Some(m) = store.get::<Mutex<bool>>("garden_soil_ph_on") {
        if let Ok(mut v) = m.lock() {
            *v = on;
        }
    }
    if let Some(req) = request {
        if let Some(m) = store.get::<Mutex<Option<(String, String)>>>("soil_ph_request") {
            if let Ok(mut s) = m.lock() {
                *s = Some(req);
            }
        }
    }
}

/// What the Garden panel shows about one crop's pH.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CropPhView {
    /// The unit's pH; None when soil pH is off or the medium is not modelled.
    pub ph: Option<f32>,
    /// The crop's plants.csv window.
    pub window: [f32; 2],
    /// The health cap pH sets (100 inside the window).
    pub cap: f32,
    /// Held by a tower's dosing: lime and sulfur do not apply.
    pub held: bool,
}

/// The Garden panel's view of pH for one frame (lib.rs crop bridge).
pub struct GardenView<'a> {
    data: Option<&'a SoilPhData>,
    units: PhUnits,
    on: bool,
}

impl<'a> GardenView<'a> {
    pub fn new(store: &'a DataStore, world: &hecs::World) -> Self {
        let units = world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.ph.clone()).unwrap_or_default();
        Self { data: store.get::<SoilPhData>("garden_soil_ph"), units, on: is_on(store) }
    }

    pub fn crop(&self, crop: &CropInstance, def: Option<&PlantDef>) -> CropPhView {
        let window = def.map_or([0.0; 2], |d| [d.ph_min, d.ph_max]);
        let (Some(data), true) = (self.data, self.on) else {
            return CropPhView { window, cap: 100.0, ..Default::default() };
        };
        let area = crop.tower_id.as_deref().unwrap_or("");
        match crop_ph(&self.units, data, area, crop.tower_slot) {
            Some((ph, held)) => CropPhView {
                ph: Some(ph as f32),
                window,
                cap: crop_ceiling(&self.units, data, true, area, crop.tower_slot, def),
                held,
            },
            None => CropPhView { window, cap: 100.0, ..Default::default() },
        }
    }

    /// The amendments as (id, button label), in the file's order; empty when
    /// soil pH is off.
    pub fn amendments(&self) -> Vec<(String, String)> {
        match (self.data, self.on) {
            (Some(d), true) => d.amendments.iter().map(|a| (a.id.clone(), a.name.clone())).collect(),
            _ => Vec::new(),
        }
    }
}
