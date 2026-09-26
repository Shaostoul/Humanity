//! Weeds in the garden's soil (2026-09-26, gardening depth).
//!
//! The garden had pests, diseases, nutrients, pH, light, pollination, humidity
//! and picking windows, and no weeds, though in a real bed or field they are the
//! main labour of growing. Every number and its source live in
//! data/garden/weeds.ron; this file is the model. It follows the pests' shape
//! (pests.rs): the state lives on the world's `SoilMemory`, the player acts
//! through a request channel, and the Settings pest severity scales it.
//!
//! THE LOOP:
//!
//! 1. Each SOIL grow area (a bed, tray or field, and the hand-planted crops;
//!    soil_ph.ron decides, the same test lime and sulfur use: towers and the
//!    mushroom racks have no soil and no weeds) holds an `AreaWeeds` in
//!    `SoilMemory::weeds`: a weed COVER, 0 to 1, and a SEED BANK, 1.0 being a
//!    typical field's. It belongs to the place: it stays in the soil between
//!    crops, and grows on an empty bed too.
//! 2. Every tick, on the garden clock, the cover grows from the bank
//!    (`step_area`: dW/dt = (r W + a)(1 - W), a = emergence x bank, solved
//!    exactly by `pests::grow`), a mulch cuts both r and a while it lasts, and
//!    the bank runs down at the cited rate and fills with the seed of weeds left
//!    standing. An outdoor field starts with a typical field's bank, an indoor
//!    bed with a quarter of it: weeds come up faster outdoors.
//! 3. Each crop in the area has its health CAPPED (`health_ceiling`) by the
//!    cover and its cited loss from weeds, hardest in its CRITICAL PERIOD
//!    (`phase_share`), never below `WEED_HEALTH_FLOOR`. The farming tick takes
//!    the lowest of the nutrient, pest, pH, air and weed caps.
//! 4. The player is told once per area when weeds come up (`detect_at`), and
//!    controls them in the order growers are taught: HOE (with the backpack's
//!    hoe, which wears), then MULCH (sawdust or bark from the sawmill), through
//!    the "weed_control_request" channel (`Weeds::handle_request`). Sawdust and
//!    bark take nitrogen from the soil as they rot (`crop_tick`), which is
//!    banked as slow organic N in the unit.
//!
//! TWO MODES: the Settings "Garden pests, diseases and weeds" severity
//! ("garden_pest_severity", Off / Gentle / Realistic) scales the weeds too.
//! Weeds are the same shape as a pest (a pressure per area that caps a crop's
//! health, knocked back by controls), so one switch reads as one idea: 0 turns
//! weeds off (nothing grows, nothing is capped, the buttons do nothing), 0.5
//! halves the loss, 1 is the cited loss. A separate switch would also have
//! needed a new field on the Settings state in gui/mod.rs, which is at its
//! line budget.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;

use crate::ecs::components::{AreaWeeds, CropInstance, Npk, OrganicCohort, SoilMemory, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;

use super::soil_ph::SoilPhData;
use super::PlantDef;

/// The shipped copy, so a bare exe with no data folder still has weeds.
pub const WEEDS_RON: &str = include_str!("../../../data/garden/weeds.ron");

/// DataStore key of the loaded `WeedData` (lib.rs registers it at boot).
pub const DATA_KEY: &str = "garden_weeds";

/// The Garden panel's Hoe / Mulch request: `Mutex<Option<(area, control id)>>`.
pub const REQUEST_KEY: &str = "weed_control_request";

/// The lowest weeds can take a crop's health: 20 of 100, the floor a nutrient
/// shortage, a pest and a pH out of window share (soil::NUTRIENT_HEALTH_FLOOR),
/// so weeds stunt a crop and never kill it.
pub const WEED_HEALTH_FLOOR: f32 = 20.0;

// -- Data: data/garden/weeds.ron ------------------------------------------------

/// The critical period for a crop with no window of its own, as shares of its
/// growth_days (FAO: "approximately centred on the first one-third").
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Window {
    pub start: f64,
    pub end: f64,
}

/// One weeds.ron `crops` row: plants.csv ids, their critical period in garden
/// days after sowing or transplanting, and their loss from weeds left all
/// season.
#[derive(Debug, Clone, Deserialize)]
pub struct CropWeeds {
    pub plants: Vec<String>,
    #[serde(default)]
    pub start_days: Option<f64>,
    #[serde(default)]
    pub end_days: Option<f64>,
    #[serde(default)]
    pub max_loss: Option<f64>,
}

/// What a control does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum WeedControlKind {
    /// Cut the weeds out with the backpack's `tool`, which wears.
    Hoe,
    /// Lay `item` over the soil: smothers some, stops most new ones for a time.
    Mulch,
}

/// One weeds.ron `controls` row (the file's comments say what each field is).
#[derive(Debug, Clone, Deserialize)]
pub struct WeedControl {
    pub id: String,
    pub name: String,
    pub kind: WeedControlKind,
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub m2_per_use: f64,
    #[serde(default)]
    pub removes: f64,
    #[serde(default)]
    pub removes_when_large: Option<f64>,
    #[serde(default)]
    pub large_above: f64,
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub kg_per_m2: Option<f64>,
    #[serde(default)]
    pub depth_cm: Option<f64>,
    #[serde(default)]
    pub lasts_days: f64,
    #[serde(default)]
    pub blocks: f64,
    #[serde(default)]
    pub smothers: f64,
    #[serde(default)]
    pub n_tie_g_per_kg: f64,
    #[serde(default)]
    pub note: String,
}

impl WeedControl {
    /// The share of the cover one hoeing takes: `removes` while the weeds are
    /// small, `removes_when_large` once the cover is past `large_above`.
    pub fn hoe_share(&self, cover: f64) -> f64 {
        let k = match self.removes_when_large {
            Some(large) if cover > self.large_above => large,
            _ => self.removes,
        };
        k.clamp(0.0, 1.0)
    }

    /// Kilograms of the mulch a square metre takes: `kg_per_m2`, else
    /// `depth_cm` of it at its items.csv volume per kg (bark: 6.35 cm at 3.3
    /// L a kg is 19.2 kg). 0 for a hoe, or a depth with no item volume known.
    pub fn mulch_kg_per_m2(&self, items: Option<&crate::systems::inventory::ItemRegistry>) -> f64 {
        if let Some(kg) = self.kg_per_m2 {
            return kg.max(0.0);
        }
        let (Some(cm), Some(r)) = (self.depth_cm, items) else { return 0.0 };
        let (vol_l, kg) = (f64::from(r.volume_for(&self.item)), f64::from(r.mass_for(&self.item)));
        if vol_l > 0.0 && kg > 0.0 {
            // cm of depth over a m2 is cm x 10 litres.
            cm.max(0.0) * 10.0 / (vol_l / kg)
        } else {
            0.0
        }
    }

    /// Grams of N a m2 under this mulch gives up per garden day while it
    /// lasts: its whole tie-up spread evenly over its life.
    pub fn n_tie_g_per_m2_day(&self, items: Option<&crate::systems::inventory::ItemRegistry>) -> f64 {
        if self.kind != WeedControlKind::Mulch || !(self.lasts_days > 0.0) {
            return 0.0;
        }
        self.mulch_kg_per_m2(items) * self.n_tie_g_per_kg.max(0.0) / self.lasts_days
    }
}

/// What data/garden/weeds.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct WeedData {
    pub growth_per_day: f64,
    pub emergence_per_day: f64,
    pub detect_at: f64,
    pub field_start_bank: f64,
    pub indoor_start_bank: f64,
    pub bank_to_5pct_days: f64,
    pub seed_set_per_day: f64,
    pub bank_max: f64,
    pub default_period: Window,
    pub before_period_share: f64,
    pub after_period_share: f64,
    pub default_max_loss: f64,
    /// WSSA's nine season-long losses, of which `default_max_loss` is the
    /// median (a test checks).
    #[serde(default)]
    pub wssa_losses: Vec<f64>,
    pub default_unit_area_m2: f64,
    #[serde(default)]
    pub crops: Vec<CropWeeds>,
    #[serde(default)]
    pub controls: Vec<WeedControl>,
}

impl WeedData {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The data folder's copy first (so it can be modded), the shipped copy if
    /// that is missing or does not parse. The shipped copy always parses: a
    /// test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("weeds.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::shipped()
    }

    pub fn shipped() -> Self {
        Self::parse(WEEDS_RON).expect("the shipped data/garden/weeds.ron parses")
    }

    pub fn control(&self, id: &str) -> Option<&WeedControl> {
        self.controls.iter().find(|c| c.id == id)
    }

    fn row(&self, plant: &str) -> Option<&CropWeeds> {
        self.crops.iter().find(|c| c.plants.iter().any(|p| p == plant))
    }

    /// The share of its health weeds left all season take from `plant`.
    pub fn max_loss(&self, plant: &str) -> f64 {
        self.row(plant).and_then(|r| r.max_loss).unwrap_or(self.default_max_loss).clamp(0.0, 1.0)
    }

    /// `plant`'s critical period, garden days after sowing: its own row's, else
    /// `default_period` of its `growth_days`.
    pub fn window(&self, plant: &str, growth_days: f64) -> (f64, f64) {
        match self.row(plant).map(|r| (r.start_days, r.end_days)) {
            Some((Some(s), Some(e))) => (s, e),
            _ => (self.default_period.start * growth_days.max(0.0), self.default_period.end * growth_days.max(0.0)),
        }
    }

    /// How much of the weed cap applies `crop_days` into the crop's season:
    /// 1 inside its critical period, `before_period_share` before it,
    /// `after_period_share` after.
    pub fn phase_share(&self, plant: &str, growth_days: f64, crop_days: f64) -> f64 {
        let (start, end) = self.window(plant, growth_days);
        if crop_days < start {
            self.before_period_share
        } else if crop_days <= end {
            1.0
        } else {
            self.after_period_share
        }
        .clamp(0.0, 1.0)
    }

    /// A soil area's weeds the first time they are needed: no cover yet, and
    /// the seed bank of a field outdoors or of a filled indoor bed.
    pub fn new_area(&self, outdoors: bool) -> AreaWeeds {
        let bank = if outdoors { self.field_start_bank } else { self.indoor_start_bank };
        AreaWeeds { bank: bank.clamp(0.0, self.bank_max), ..Default::default() }
    }
}

// -- The model ------------------------------------------------------------------

/// The share of emergence and growth the area's mulches stop (the best one
/// working there; 0 with none).
pub fn mulch_block(area: &AreaWeeds, data: &WeedData) -> f64 {
    area.mulch
        .keys()
        .filter_map(|id| data.control(id))
        .map(|c| c.blocks.clamp(0.0, 1.0))
        .fold(0.0, f64::max)
}

/// Step one soil area `days` garden days: the cover grows from the bank (less
/// under a mulch), the bank runs down and fills with the seed of the weeds
/// standing, and mulches age. True when the weeds have just become
/// noticeable (`detect_at`), for the one notice.
pub fn step_area(area: &mut AreaWeeds, data: &WeedData, days: f64) -> bool {
    if !(days > 0.0) {
        return false;
    }
    let open = 1.0 - mulch_block(area, data);
    let before = area.level;
    let a = data.emergence_per_day.max(0.0) * area.bank.max(0.0) * open;
    area.level = super::pests::grow(area.level, data.growth_per_day.max(0.0) * open, a, days);
    // The bank: first-order decline to 5% in bank_to_5pct_days (Burnside),
    // plus the seed the stand sets, taken at the step's mean cover. Solved as
    // a linear equation, so a long step cannot overshoot.
    let k = if data.bank_to_5pct_days > 0.0 { 20f64.ln() / data.bank_to_5pct_days } else { 0.0 };
    let seed = data.seed_set_per_day.max(0.0) * 0.5 * (before + area.level);
    area.bank = if k > 0.0 {
        let keep = (-k * days).exp();
        area.bank * keep + seed * (1.0 - keep) / k
    } else {
        area.bank + seed * days
    }
    .clamp(0.0, data.bank_max.max(0.0));
    area.mulch.retain(|_, left| {
        *left -= days;
        *left > 0.0
    });
    if area.level >= data.detect_at && !area.told {
        area.told = true;
        return true;
    }
    if area.level < data.detect_at / 2.0 {
        area.told = false;
    }
    false
}

/// The highest health a crop of `plant` can hold at weed `cover`, `crop_days`
/// into a season of `growth_days`: its cited loss x the cover x how much of
/// the cap its phase takes (`WeedData::phase_share`), scaled by `severity`
/// (the mode), never below `WEED_HEALTH_FLOOR`.
pub fn health_ceiling(data: &WeedData, cover: f64, plant: &str, growth_days: f64, crop_days: f64, severity: f32) -> f32 {
    let sev = f64::from(severity).clamp(0.0, 1.0);
    let loss = data.max_loss(plant) * sev * cover.clamp(0.0, 1.0) * data.phase_share(plant, growth_days, crop_days);
    ((100.0 * (1.0 - loss)) as f32).clamp(WEED_HEALTH_FLOOR, 100.0)
}

/// The ground one unit of `area` covers, m2: its plot's floor area, else the
/// ground one plant of its crop stands on, else `default_unit_area_m2`.
pub fn unit_m2(data: &WeedData, store: &DataStore, area: &str, def: Option<&PlantDef>) -> f64 {
    super::soil_ph::plot_area(store, area)
        .filter(|a| *a > 0.0)
        .or_else(|| def.and_then(|d| d.area_per_plant_m2).map(f64::from).filter(|a| *a > 0.0))
        .unwrap_or(data.default_unit_area_m2.max(1e-6))
}

/// How many garden days into its season a crop is: its growth age on the
/// garden clock (game seconds at the growth speed).
pub fn crop_days(crop: &CropInstance, elapsed_seconds: f64, growth_speed: f32) -> f64 {
    (elapsed_seconds - crop.planted_at).max(0.0) / super::SECONDS_PER_DAY * f64::from(growth_speed.max(0.0))
}

/// The farming system's weeds: the data, read on first use (a "garden_weeds"
/// DataStore entry wins over it).
#[derive(Debug, Default)]
pub struct Weeds {
    own: Option<WeedData>,
}

impl Weeds {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure(&mut self) {
        if self.own.is_none() {
            self.own = Some(WeedData::load());
        }
    }

    /// The data in use (the DataStore's, else this system's own).
    pub fn data<'a>(&'a self, store: &'a DataStore) -> Option<&'a WeedData> {
        store.get::<WeedData>(DATA_KEY).or(self.own.as_ref())
    }

    /// Take every soil area's weeds out of the world's `SoilMemory` for the
    /// crop loop (put them back with `SoilMemory::weeds = ...` after it), and
    /// step them `days` garden days. A soil area with a living crop and no
    /// weeds yet starts from its seed bank; an area that is no longer soil is
    /// forgotten. Nothing steps with weeds switched off (`severity` 0).
    pub fn step(
        &mut self,
        world: &mut hecs::World,
        store: &DataStore,
        soil: &SoilPhData,
        severity: f32,
        days: f64,
    ) -> HashMap<String, AreaWeeds> {
        let e = super::soil::soil_memory_entity(world);
        let mut map = world.get::<&mut SoilMemory>(e).map(|mut m| std::mem::take(&mut m.weeds)).unwrap_or_default();
        if !(severity > 0.0) || !(days > 0.0) {
            return map;
        }
        self.ensure();
        let Some(data) = self.data(store) else { return map };
        // A soil area with a living crop and no weeds yet starts from its bank.
        // Each area is looked at once a tick, and only a new one allocates.
        {
            let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            let mut q = world.query::<&CropInstance>();
            for (_, c) in q.iter() {
                let a = c.tower_id.as_deref().unwrap_or("");
                if c.growth_stage != STAGE_DEAD && seen.insert(a) && !map.contains_key(a) && soil.soil_for(a).is_some() {
                    map.insert(a.to_string(), data.new_area(super::is_field_area(a)));
                }
            }
        }
        map.retain(|a, _| soil.soil_for(a).is_some());
        let mut appeared: Vec<String> = map
            .iter_mut()
            .filter_map(|(a, st)| step_area(st, data, days).then(|| a.clone()))
            .collect();
        appeared.sort();
        for a in appeared {
            super::push_notice(store, appeared_notice(&a));
        }
        map
    }

    /// One crop's tick: its weed cap, and the nitrogen the mulch over its unit
    /// takes this tick (`days` garden days), drawn from its `soil` store and
    /// banked in the unit's slow organic pool (it comes back as the mulch
    /// rots). 100 and nothing drawn with weeds off or no soil state.
    #[allow(clippy::too_many_arguments)]
    pub fn crop_tick(
        &self,
        store: &DataStore,
        areas: &HashMap<String, AreaWeeds>,
        crop: &CropInstance,
        def: Option<&PlantDef>,
        severity: f32,
        elapsed_seconds: f64,
        growth_speed: f32,
        days: f64,
        soil: &mut Npk,
        organic: &mut HashMap<String, HashMap<u32, Vec<OrganicCohort>>>,
    ) -> f32 {
        let area = crop.tower_id.as_deref().unwrap_or("");
        let (Some(data), Some(st), true) = (self.data(store), areas.get(area), severity > 0.0) else {
            return 100.0;
        };
        let items = store.get::<crate::systems::inventory::ItemRegistry>("item_registry");
        let rate: f64 = st.mulch.keys().filter_map(|id| data.control(id)).map(|c| c.n_tie_g_per_m2_day(items)).sum();
        if rate > 0.0 && days > 0.0 {
            let take = (rate * unit_m2(data, store, area, def) * days).min(soil.n.max(0.0));
            soil.n -= take;
            if let Some(slot) = crop.tower_slot {
                super::soil::bank_organic(organic.entry(area.to_string()).or_default().entry(slot).or_default(), take);
            }
        }
        let growth = def.map_or(0.0, |d| f64::from(d.growth_days));
        health_ceiling(data, st.level, &crop.crop_def_id, growth, crop_days(crop, elapsed_seconds, growth_speed), severity)
    }

    /// Apply one player request `(area, control id)` from the
    /// "weed_control_request" channel: hoe with the backpack's hoe (worn by one
    /// use per `m2_per_use` of ground; free in creative mode, where no hoe is
    /// needed), or lay a mulch (its items from the backpack, free in creative
    /// mode), and say what it did. Refused, with a notice and nothing spent,
    /// when the area is not soil, has nothing planted, or the backpack lacks
    /// the hoe or the mulch. Ignored with weeds switched off.
    pub fn handle_request(&mut self, world: &mut hecs::World, store: &DataStore, soil: &SoilPhData, severity: f32, creative: bool) {
        let req = store
            .get::<Mutex<Option<(String, String)>>>(REQUEST_KEY)
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        let Some((area, id)) = req else { return };
        if !(severity > 0.0) {
            return;
        }
        self.ensure();
        let Some(data) = self.data(store) else { return };
        let Some(control) = data.control(&id) else {
            log::warn!("[Farming] no weed control '{id}' in data/garden/weeds.ron");
            return;
        };
        let name = area.replace('_', " ");
        if soil.soil_for(&area).is_none() {
            super::push_notice(
                store,
                format!("{name} has no soil, so no weeds: a tower grows in a nutrient mist and a mushroom rack in its own substrate."),
            );
            return;
        }
        let plants = store.get::<super::PlantRegistry>("plant_registry");
        let crops: Vec<String> = world
            .query::<&CropInstance>()
            .iter()
            .filter(|(_, c)| c.growth_stage != STAGE_DEAD && c.tower_id.as_deref().unwrap_or("") == area)
            .map(|(_, c)| c.crop_def_id.clone())
            .collect();
        if crops.is_empty() {
            super::push_notice(store, format!("There is nothing planted to weed in {name}."));
            return;
        }
        let m2: f64 = crops.iter().map(|p| unit_m2(data, store, &area, plants.and_then(|r| r.get(p)))).sum();
        let items = store.get::<crate::systems::inventory::ItemRegistry>("item_registry");
        let item_name = |id: &str| items.and_then(|r| r.items.get(id).map(|d| d.name.clone())).unwrap_or_else(|| id.to_string());
        // What the backpack gives up: the hoe's wear, or the mulch.
        let mut spent = String::new();
        if !creative {
            use crate::ecs::components::Controllable;
            use crate::systems::inventory::Inventory;
            match control.kind {
                WeedControlKind::Hoe => {
                    let uses = (m2 / control.m2_per_use.max(1e-6)).ceil().max(1.0) as u32;
                    let base = items.map_or(0, |r| r.durability_for(&control.tool));
                    let levels = store.get::<crate::systems::crafting::quality::QualityLevels>("quality_levels");
                    let mut have = false;
                    let mut broke = 0;
                    for (_e, (inv, _c)) in world.query_mut::<(&mut Inventory, &Controllable)>() {
                        have = inv.has_item(&control.tool, 1);
                        for _ in 0..uses {
                            if !inv.has_item(&control.tool, 1) {
                                break;
                            }
                            if inv.wear_item(&control.tool, |q| levels.map_or(base, |l| l.durability(base, q))) {
                                broke += 1;
                            }
                        }
                        break;
                    }
                    if !have {
                        super::push_notice(store, format!("{} needs a {} in your backpack.", control.name, item_name(&control.tool).to_lowercase()));
                        return;
                    }
                    if broke > 0 {
                        spent = format!(" Your {} wore out.", item_name(&control.tool).to_lowercase());
                    }
                }
                WeedControlKind::Mulch => {
                    let each = items.map_or(1.0, |r| f64::from(r.mass_for(&control.item))).max(1e-6);
                    let need = (m2 * control.mulch_kg_per_m2(items) / each).ceil().max(1.0) as u32;
                    let mut have = 0;
                    let mut took = false;
                    for (_e, (inv, _c)) in world.query_mut::<(&mut Inventory, &Controllable)>() {
                        have = inv.count_item(&control.item);
                        if have >= need {
                            inv.remove_item(&control.item, need);
                            took = true;
                        }
                        break;
                    }
                    if !took {
                        super::push_notice(
                            store,
                            format!("{} for {name} ({m2:.1} m2) takes {need} x {}; you have {have}.", control.name, item_name(&control.item)),
                        );
                        return;
                    }
                    spent = format!(" It took {need} x {}.", item_name(&control.item));
                }
            }
        }
        let e = super::soil::soil_memory_entity(world);
        let Ok(mut mem) = world.get::<&mut SoilMemory>(e) else { return };
        let st = mem.weeds.entry(area.clone()).or_insert_with(|| data.new_area(super::is_field_area(&area)));
        let before = st.level;
        let mut s = match control.kind {
            WeedControlKind::Hoe => {
                st.level *= 1.0 - control.hoe_share(before);
                let big = if control.removes_when_large.is_some() && before > control.large_above {
                    " They were big: a hoe gets far fewer once weeds are established, so hoe while they are small."
                } else {
                    ""
                };
                format!("Hoed {} ({m2:.1} m2): weeds {} to {}.{big}", soil_of(&area), pct(before), pct(st.level))
            }
            WeedControlKind::Mulch => {
                st.level *= 1.0 - control.smothers.clamp(0.0, 1.0);
                let left = st.mulch.entry(control.id.clone()).or_insert(0.0);
                *left = left.max(control.lasts_days);
                let tie = control.mulch_kg_per_m2(items) * control.n_tie_g_per_kg;
                let n = if tie > 0.0 {
                    format!(" As it rots it takes about {tie:.0} g of nitrogen a square metre from the soil over that time: feed the bed.")
                } else {
                    String::new()
                };
                format!(
                    "{} on {} ({m2:.1} m2, {:.1} kg a square metre): weeds {} to {}, and most new ones are stopped for about {:.0} garden days.{n}",
                    control.name,
                    soil_of(&area),
                    control.mulch_kg_per_m2(items),
                    pct(before),
                    pct(st.level),
                    control.lasts_days
                )
            }
        };
        drop(mem);
        s.push_str(&spent);
        log::info!("[Farming] {} on {area}: {before:.3} ({m2:.2} m2)", control.id);
        super::push_notice(store, s);
    }
}

// -- What the player is told ----------------------------------------------------

fn soil_of(area: &str) -> String {
    if area.is_empty() {
        "the soil your hand-planted crops grow in".to_string()
    } else {
        format!("the soil in {}", area.replace('_', " "))
    }
}

fn pct(level: f64) -> String {
    format!("{:.0}%", (level * 100.0).clamp(0.0, 100.0))
}

/// The one line said when weeds first become noticeable in `area`.
pub fn appeared_notice(area: &str) -> String {
    format!(
        "Weeds are coming up in {}. Hoe them while they are small: a crop loses most to weeds in its critical period, \
         about the first third of its growth, and weeds left to flower fill the soil with seed for years. Mulch stops most new ones.",
        soil_of(area)
    )
}

// -- The request channel, and the Garden panel's view -----------------------------

/// Put the weeds' data and the Garden panel's request channel in the
/// DataStore (lib.rs, at boot).
pub fn register(store: &mut DataStore) {
    store.insert(DATA_KEY, WeedData::load());
    store.insert(REQUEST_KEY, Mutex::new(Option::<(String, String)>::None));
}

/// Carry the panel's chosen control `(area, control id)` to the farming
/// system (lib.rs, every frame).
pub fn publish(store: &DataStore, request: Option<(String, String)>) {
    if let (Some(req), Some(m)) = (request, store.get::<Mutex<Option<(String, String)>>>(REQUEST_KEY)) {
        if let Ok(mut s) = m.lock() {
            *s = Some(req);
        }
    }
}

/// One soil grow area's weed row for the Garden panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeedAreaRow {
    /// The grow area's tag (a crop's `tower_id`; "" for hand-planted crops).
    pub area: String,
    /// Weed cover, 0..1.
    pub level: f32,
    /// True once the weeds are noticeable (`detect_at`), for the colour.
    pub noticed: bool,
    /// "Weeds 12% · sawdust mulch, 80 garden days left · seed in the soil 1.4x
    /// a typical field's".
    pub line: String,
    /// The controls as (id, button label, note), in the file's order (hoe,
    /// then the mulches).
    pub controls: Vec<(String, String, String)>,
}

/// The Garden panel's view of the weeds for one frame (lib.rs crop bridge).
pub struct GuiView {
    data: Option<WeedData>,
    weeds: HashMap<String, AreaWeeds>,
    severity: f32,
    elapsed: f64,
    speed: f32,
    /// A row per soil area with a living crop, sorted by area.
    pub areas: Vec<WeedAreaRow>,
}

impl GuiView {
    pub fn new(world: &hecs::World, store: &DataStore) -> Self {
        let data = store.get::<WeedData>(DATA_KEY).cloned();
        let weeds = world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.weeds.clone()).unwrap_or_default();
        let severity = store
            .get::<Mutex<f32>>("garden_pest_severity")
            .and_then(|m| m.lock().ok().map(|v| *v))
            .unwrap_or(super::pests::DEFAULT_PEST_SEVERITY);
        let speed = super::clamp_growth_speed(
            store
                .get::<Mutex<f32>>("crop_growth_speed")
                .and_then(|m| m.lock().ok().map(|v| *v))
                .unwrap_or(super::DEFAULT_CROP_GROWTH_SPEED),
        );
        let mut view = Self { data, weeds, severity, elapsed: crate::systems::time::elapsed_now(store), speed, areas: Vec::new() };
        let (Some(data), Some(soil), true) = (view.data.as_ref(), store.get::<SoilPhData>("garden_soil_ph"), severity > 0.0) else {
            return view;
        };
        let items = store.get::<crate::systems::inventory::ItemRegistry>("item_registry");
        // Sorted by area, each once.
        let areas: Vec<String> = {
            let mut set: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
            let mut q = world.query::<&CropInstance>();
            for (_, c) in q.iter() {
                if c.growth_stage != STAGE_DEAD {
                    set.insert(c.tower_id.as_deref().unwrap_or(""));
                }
            }
            set.into_iter().filter(|a| soil.soil_for(a).is_some()).map(str::to_string).collect()
        };
        let controls: Vec<(String, String, String)> =
            data.controls.iter().map(|c| (c.id.clone(), c.name.clone(), c.note.clone())).collect();
        view.areas = areas
            .into_iter()
            .map(|area| {
                let st = view.weeds.get(&area).cloned().unwrap_or_else(|| data.new_area(super::is_field_area(&area)));
                let mut line = format!("Weeds {}", pct(st.level));
                let mut mulch: Vec<(&String, &f64)> = st.mulch.iter().collect();
                mulch.sort_by(|a, b| a.0.cmp(b.0));
                for (id, left) in mulch {
                    let what = data
                        .control(id)
                        .map(|c| items.and_then(|r| r.items.get(&c.item).map(|d| d.name.to_lowercase())).unwrap_or_else(|| c.item.clone()))
                        .unwrap_or_else(|| id.clone());
                    line.push_str(&format!(" · {what} mulch, {left:.0} garden days left"));
                }
                line.push_str(&format!(" · seed in the soil {:.1}x a typical field's", st.bank));
                WeedAreaRow { area, level: st.level as f32, noticed: st.level >= data.detect_at, line, controls: controls.clone() }
            })
            .collect();
        view
    }

    /// The crop card's "Weeds" row: the cover, where the crop is in its
    /// critical period and the health the weeds hold it to; "" when weeds are
    /// off, the area has none, or they are not capping it.
    pub fn crop_row(&self, crop: &CropInstance, def: Option<&PlantDef>) -> String {
        let area = crop.tower_id.as_deref().unwrap_or("");
        let (Some(data), Some(st), true) = (self.data.as_ref(), self.weeds.get(area), self.severity > 0.0) else {
            return String::new();
        };
        let growth = def.map_or(0.0, |d| f64::from(d.growth_days));
        let days = crop_days(crop, self.elapsed, self.speed);
        let cap = health_ceiling(data, st.level, &crop.crop_def_id, growth, days, self.severity);
        if cap >= 99.5 {
            return String::new();
        }
        let (start, end) = data.window(&crop.crop_def_id, growth);
        let phase = if days < start {
            format!("before its critical period (days {start:.0} to {end:.0})")
        } else if days <= end {
            format!("in its critical period (days {start:.0} to {end:.0})")
        } else {
            "past its critical period".to_string()
        };
        format!("{} cover, {phase}: health held to {cap:.0}", pct(st.level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> WeedData {
        WeedData::shipped()
    }

    /// The critical period caps a crop hardest: at full cover a tomato (UMass:
    /// days 23 to 41) is held to 100 x (1 - 0.47) = 53 inside it, less hard
    /// after it (half the loss) and least before it (a quarter); gentle mode
    /// halves the loss; no cover, no cap; and worse data stops at the floor.
    /// Seen red by making `phase_share` answer 1 for every day (the tomato was
    /// then held to 53 before and after its critical period too).
    #[test]
    fn weeds_cap_a_crop_hardest_in_its_critical_period() {
        let d = shipped();
        let before = health_ceiling(&d, 1.0, "tomato", 70.0, 10.0, 1.0);
        let during = health_ceiling(&d, 1.0, "tomato", 70.0, 30.0, 1.0);
        let after = health_ceiling(&d, 1.0, "tomato", 70.0, 60.0, 1.0);
        assert!((during - 53.0).abs() < 1e-3, "in its critical period: {during}");
        assert!(during < after && after < before, "hardest in the period: {before} / {during} / {after}");
        assert!((after - 76.5).abs() < 1e-3 && (before - 88.25).abs() < 1e-3, "{before} / {after}");
        assert!((health_ceiling(&d, 1.0, "tomato", 70.0, 30.0, 0.5) - 76.5).abs() < 1e-3, "gentle: half");
        assert_eq!(health_ceiling(&d, 0.0, "tomato", 70.0, 30.0, 1.0), 100.0);
        assert_eq!(health_ceiling(&d, 1.0, "tomato", 70.0, 30.0, 0.0), 100.0, "off");
        // The grain: wheat's cited window is days 12 to 24 (Agostinetto 2008)
        // and its loss spring wheat's 19.5% (WSSA).
        assert!((health_ceiling(&d, 1.0, "wheat", 120.0, 18.0, 1.0) - 80.5).abs() < 1e-3);
        assert!(health_ceiling(&d, 1.0, "wheat", 120.0, 60.0, 1.0) > 80.5, "past the grain's period");
        // A crop with no row takes FAO's first third of its growth.
        assert_eq!(d.window("lettuce", 45.0), (0.21 * 45.0, 0.33 * 45.0));
        let mut worse = d.clone();
        worse.default_max_loss = 1.0;
        assert_eq!(health_ceiling(&worse, 1.0, "lettuce", 45.0, 12.0, 1.0), WEED_HEALTH_FLOOR, "never below the floor");
    }

    /// The seed bank and the cover follow their anchors: a bare bed with a
    /// typical field's bank is at half cover in about three weeks (the start of
    /// the cited critical periods), an indoor bed's quarter bank is slower; the
    /// bank runs down to 5% in five clean years and one uncontrolled 150-day
    /// season takes it from 5% back to about 90% (Burnside, via eOrganic).
    /// Seen red by using ln(10) for ln(20) in `step_area` (five clean years
    /// then left 10%).
    #[test]
    fn the_seed_bank_and_cover_follow_their_anchors() {
        let d = shipped();
        let days_to_half = |bank: f64| {
            let mut a = AreaWeeds { bank, ..Default::default() };
            (1..=200).find(|_| {
                step_area(&mut a, &d, 1.0);
                a.level >= 0.5
            })
        };
        let field = days_to_half(d.field_start_bank).unwrap();
        assert!((18..=24).contains(&field), "half cover in about three weeks: {field}");
        assert!(days_to_half(d.indoor_start_bank).unwrap() > field + 4, "slower indoors");
        let mut clean = AreaWeeds { bank: 1.0, ..Default::default() };
        for _ in 0..1825 {
            clean.level = 0.0;
            step_area(&mut clean, &d, 1.0);
        }
        assert!((clean.bank - 0.05).abs() < 0.005, "five clean years: {}", clean.bank);
        let mut wild = AreaWeeds { bank: 0.05, ..Default::default() };
        for _ in 0..150 {
            step_area(&mut wild, &d, 1.0);
        }
        assert!((0.75..=1.05).contains(&wild.bank), "one uncontrolled season: {}", wild.bank);
    }
}
