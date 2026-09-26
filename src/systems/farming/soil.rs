//! Plant nutrients as N, P2O5 and K2O (2026-09-26, gardening depth rung 3).
//!
//! Until this rung a grow area had ONE nutrient slider, which multiplied its
//! crops' growth speed (0.5x to 1.5x), and fertilizing a crop simply added 40
//! health. plants.csv's N, P and K columns were shown in the Garden table and
//! read by nothing. This file replaced a `Soil` scaffold that nothing used.
//!
//! THE LOOP, in grams of N, P2O5 and K2O (see `Npk`):
//!
//! 1. Every unit of growing space (a tower slot, a bed, tray or field unit, a
//!    hand-planted crop's pot) holds a store of plant-available nutrients, the
//!    `CropSoil` on the crop growing in it. A fresh unit starts with
//!    `FRESH_UNIT_SEASONS` of its crop's season need.
//! 2. A growing crop draws its season need (`season_need`: its expected
//!    harvest times the grams each kg of it removes, from the crop's own
//!    cited plants.csv removal columns, or, where those are blank, from its
//!    relative index scaled by the anchor crop's cited removal; see
//!    `removal_per_kg` and data/garden/nutrients.ron) in step with its growth
//!    clock, so it draws nothing in the dark and ten times as fast at the 10x
//!    growth setting.
//! 3. A crop whose store of ANY nutrient falls below `RESERVE_SEASONS` of its
//!    need runs short, and its health is capped by the SCARCEST one
//!    (`sufficiency`, Liebig's law of the minimum). The cap only ever pulls
//!    health down gently to `NUTRIENT_HEALTH_FLOOR`, never to death, and it
//!    lifts the moment the store is refilled. Health already feeds the season
//!    health record, so a short season is a smaller harvest.
//! 4. Fertilizing adds one item's plant-available grams to the unit
//!    (`FertilizerDef::available_per_item`), in the ratio of its analysis:
//!    compost gives a lot of phosphate and potash for its nitrogen.
//! 5. The old slider now drives an automatic FEEDER (`feed_target`): an area
//!    with its "nutrient" slider set is topped up from fertilizer in home
//!    storage. Half-way (the slider's default, and the old neutral point) is
//!    exactly enough; lower under-feeds; higher holds a bigger reserve but grows
//!    nothing faster, because past sufficiency more fertilizer does not help.
//! 6. When a crop is harvested or dies, what is left in its unit passes to the
//!    next crop sown there (`SoilMemory`), so cropping a unit mines it.
//!
//! CLOSING THE NITROGEN LOOP (2026-09-26, the next rung). Compost alone could
//! not feed the towers: at its real 7% first-season N a 50-slot lettuce tower
//! needed about 27 bags a season. The household's real nitrogen sources are:
//!
//! 7. Compost's SLOW release. The 93% of a bag's N that is not available in
//!    its first season is organic N, banked in the unit's soil
//!    (`SoilMemory::organic`) and released over the years after at the cited
//!    schedule (`release_organic`), so a unit composted season after season
//!    builds up, the way a real garden's soil does.
//! 8. URINE, the household's largest nitrogen stream: one stored person-day
//!    of it (`urine_stored_0`) carries 10.9 g of N, all of it available, and
//!    it may not go on a crop within a month of its harvest (the WHO rule, in
//!    `FertilizerDef::withhold_days`). It reaches the player through the
//!    Compost action in src/systems/food.rs.
//! 9. LEGUMES fix part of their own N from the air (plants.csv
//!    `n_fixed_pct`), so they draw only the rest from their unit
//!    (`soil_draw`), and leave the fixed N in their roots and haulm behind as
//!    a credit for the next crop (`legume_credit_n`).
//!
//! SOIL pH (2026-09-26) is its own rung, in soil_ph.rs. It landed as a
//! separate health ceiling rather than the multiplier on `sufficiency` this
//! note once planned: its cited losses are relative yields, so the cap is
//! that share of full health directly, where `health_ceiling`'s floor-to-100
//! mapping would have softened them (a 30% loss read as 24%); and a separate
//! ceiling combines with the nutrient and pest ones the way those combine
//! with each other, the lowest binding. Humidity is still the next rung.

use serde::Deserialize;

use crate::ecs::components::Npk;

use super::PlantDef;

/// The three nutrients, for naming the scarce one to the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nutrient {
    Nitrogen,
    Phosphorus,
    Potassium,
}

impl Nutrient {
    /// Plain word for a notice. Phosphorus and potassium, not phosphate and
    /// potash: the player is told which element is missing, even though the
    /// grams are counted as the oxides.
    pub fn word(self) -> &'static str {
        match self {
            Nutrient::Nitrogen => "nitrogen",
            Nutrient::Phosphorus => "phosphorus",
            Nutrient::Potassium => "potassium",
        }
    }
}

impl Npk {
    pub const ZERO: Npk = Npk { n: 0.0, p2o5: 0.0, k2o: 0.0 };

    pub fn new(n: f64, p2o5: f64, k2o: f64) -> Self {
        Self { n, p2o5, k2o }
    }

    /// Each nutrient times `k`.
    pub fn scaled(self, k: f64) -> Self {
        Self::new(self.n * k, self.p2o5 * k, self.k2o * k)
    }

    /// Nutrient by nutrient sum.
    pub fn plus(self, o: Npk) -> Self {
        Self::new(self.n + o.n, self.p2o5 + o.p2o5, self.k2o + o.k2o)
    }

    /// Nutrient by nutrient product: an index vector times a per-index scale.
    pub fn times(self, o: Npk) -> Self {
        Self::new(self.n * o.n, self.p2o5 * o.p2o5, self.k2o * o.k2o)
    }
}

// -- Tuning. Each is a game choice, said as such, not a measured number. -----

/// A fresh unit holds this many seasons of its crop's need (1.5).
///
/// A GAME ASSUMPTION, not a measurement: real soil's supply depends on the
/// soil, its organic matter and its history (WSU's compost guide puts the N a
/// soil's organic matter releases at 50 to 200 lb/acre a season), and the game
/// has no soil type per unit yet. 1.5 means the first crop in a newly prepared
/// unit grows unfed and unstressed, the second runs short about 40% of the way
/// through its season, and the third starts short: cropping mines a unit, and
/// fertilizing is how it is kept. Sized to the crop because units range from a
/// tower cup to a field patch and the game does not know their volume.
pub const FRESH_UNIT_SEASONS: f64 = 1.5;

/// A crop runs short of a nutrient once its unit holds less than this share
/// of the crop's season need of it (a tenth). Below it the crop's health cap
/// falls in proportion to what is left: the linear-plateau response the
/// fertilizer trials fit (yield rises in a straight line with available
/// nutrient up to a plateau, then stops rising). WHERE the plateau starts is
/// crop- and soil-specific and not in plants.csv, so a tenth of a season is a
/// game choice: it gives the player a warning band before the store is empty.
pub const RESERVE_SEASONS: f64 = 0.1;

/// The slider value at which the automatic feeder holds exactly
/// `RESERVE_SEASONS`: 0.5, the value the Garden edit modal starts a slider at
/// and the point the old slider treated as neutral (1.0x growth). See
/// `feed_target`.
pub const FEED_NEUTRAL: f32 = 0.5;

/// The lowest a nutrient shortage can take a crop's health (20 of 100). A
/// starved crop is stunted, not killed: it grows at a fifth of the pace
/// (growth is health-weighted) and yields about a fifth (the harvest follows
/// season health). Only thirst, RF and hard acceleration can kill a crop.
///
/// Why a fifth: Rothamsted's Broadbalk wheat experiment has grown wheat with
/// no fertilizer or manure on one strip since 1843, and it still yields.
/// Rothamsted's 2024 handout ("The Broadbalk Wheat Experiment", chart "mean
/// long-term yields of winter wheat 1852-2022", read off the graph) shows the
/// unfertilized plot holding about 1 t/ha throughout, against about 2.5 to 3
/// t/ha for fertilized continuous wheat before the 1960s and about 7 to 8.6
/// t/ha since: an unfed crop gives roughly 10 to 40% of a fed one. 20% sits
/// inside that range.
pub const NUTRIENT_HEALTH_FLOOR: f32 = 20.0;

/// Health a crop above its nutrient cap loses per second (0.1): 100 to the
/// floor in about 13 minutes of play. Against the shipped 10x growth, where a
/// tomato's 70-day season is about 2.3 hours, that is roughly a tenth of a
/// season: gradual, and back within a few minutes of fertilizing, because a
/// crop below its cap recovers at the well-watered rate. A game-feel choice.
pub const NUTRIENT_DECLINE_RATE: f32 = 0.1;

// -- Data: data/garden/nutrients.ron -------------------------------------------

/// The shipped copy, so a bare exe with no data folder still has the model.
pub const NUTRIENTS_RON: &str = include_str!("../../../data/garden/nutrients.ron");

/// What data/garden/nutrients.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct NutrientData {
    pub demand_anchor: DemandAnchor,
    #[serde(default)]
    pub fertilizers: Vec<FertilizerDef>,
    /// How banked organic N comes back, year by year (compost's slow
    /// release). Absent means it never does: organic N is banked and stays.
    #[serde(default)]
    pub organic_n_release: OrganicRelease,
    /// The share of a legume's whole-plant N, roots included, that leaves
    /// with its harvest (0.55 shipped; Salvagiotti et al. 2008 via the data
    /// file). Absent reads as 1.0: the harvest takes everything and a legume
    /// leaves no credit, the safe reading of a file that does not say.
    #[serde(default = "no_legume_credit")]
    pub legume_harvest_n_share: f64,
}

fn no_legume_credit() -> f64 {
    1.0
}

/// The share of the organic N still in the soil released in each year after
/// it went on, percent: the first year first, and the LAST entry holds for
/// every year after the list ends. See `release_organic`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrganicRelease {
    #[serde(default)]
    pub yearly_pct: Vec<f64>,
}

/// The crop whose published removal fixes the scale of the plants.csv
/// indices: grams of each nutrient removed per kg of its harvest.
#[derive(Debug, Clone, Deserialize)]
pub struct DemandAnchor {
    pub plant: String,
    pub n_g_per_kg: f64,
    pub p2o5_g_per_kg: f64,
    pub k2o_g_per_kg: f64,
}

/// One fertilizer item: its analysis as percent of the item's as-is mass,
/// and the share of each nutrient available in the season it is applied.
#[derive(Debug, Clone, Deserialize)]
pub struct FertilizerDef {
    pub item: String,
    pub n_pct: f64,
    pub p2o5_pct: f64,
    pub k2o_pct: f64,
    pub n_available: f64,
    #[serde(default = "all_available")]
    pub p2o5_available: f64,
    #[serde(default = "all_available")]
    pub k2o_available: f64,
    /// True when the N that is not available in the first season is ORGANIC
    /// N that releases over the years after (compost). It is banked in the
    /// unit's soil (`SoilMemory::organic`). False: that N is not counted.
    #[serde(default)]
    pub organic_rest: bool,
    /// The nutrients the automatic feeder doses this fertilizer by ("n",
    /// "p2o5", "k2o"). Empty means all three: it is dosed for whichever the
    /// unit lacks most, the way compost always was. Urine lists only "n".
    #[serde(default)]
    pub dose_for: Vec<String>,
    /// Garden days before a crop is ripe after which this fertilizer may no
    /// longer go on it (0 = no limit). Urine carries the WHO month.
    #[serde(default)]
    pub withhold_days: f64,
}

fn all_available() -> f64 {
    1.0
}

/// What one item of a fertilizer does to a unit, worked out once per tick
/// from its `FertilizerDef` and the item's items.csv mass.
#[derive(Debug, Clone, PartialEq)]
pub struct FertilizerDose {
    pub item: String,
    /// Plant-available grams this season.
    pub available: Npk,
    /// Grams of organic N banked in the unit's slow pool (0 unless the
    /// fertilizer's `organic_rest` is set).
    pub organic_n: f64,
    /// Which nutrients (N, P2O5, K2O) the feeder doses it by.
    pub dose_for: [bool; 3],
    /// See `FertilizerDef::withhold_days`.
    pub withhold_days: f64,
}

impl FertilizerDose {
    /// May this go on a crop with `days_left` garden days to ripening?
    pub fn allowed(&self, days_left: f64) -> bool {
        self.withhold_days <= 0.0 || days_left >= self.withhold_days
    }
}

impl FertilizerDef {
    /// Everything one item of `item_kg` does to a unit: its available grams,
    /// the organic N it banks, and the feeder's rules for it.
    pub fn dose(&self, item_kg: f64) -> FertilizerDose {
        let total_n = item_kg.max(0.0) * 1000.0 / 100.0 * self.n_pct;
        let organic_n = if self.organic_rest {
            total_n * (1.0 - self.n_available.clamp(0.0, 1.0))
        } else {
            0.0
        };
        let named = |n: &str| self.dose_for.is_empty() || self.dose_for.iter().any(|d| d.eq_ignore_ascii_case(n));
        FertilizerDose {
            item: self.item.clone(),
            available: self.available_per_item(item_kg),
            organic_n,
            dose_for: [named("n"), named("p2o5"), named("k2o")],
            withhold_days: self.withhold_days.max(0.0),
        }
    }

    /// Plant-available grams one item of `item_kg` adds to a unit. Percent of
    /// mass times the available share: a 2.0 kg bag at 0.94% N with 7%
    /// available is 2000 x 0.0094 x 0.07 = 1.316 g of N.
    pub fn available_per_item(&self, item_kg: f64) -> Npk {
        let g = item_kg.max(0.0) * 1000.0 / 100.0;
        Npk::new(
            g * self.n_pct * self.n_available,
            g * self.p2o5_pct * self.p2o5_available,
            g * self.k2o_pct * self.k2o_available,
        )
    }
}

impl NutrientData {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The data folder's copy first (so it can be modded), the shipped copy
    /// if that is missing or does not parse. The shipped copy always parses:
    /// a test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("nutrients.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::parse(NUTRIENTS_RON).expect("the shipped data/garden/nutrients.ron parses")
    }

    pub fn fertilizer(&self, item: &str) -> Option<&FertilizerDef> {
        self.fertilizers.iter().find(|f| f.item == item)
    }

    /// Grams of each nutrient a crop needs per kg of harvest per point of its
    /// plants.csv index: the anchor's removal divided by the anchor's index.
    /// Only a crop with a blank removal column uses it (`removal_per_kg`).
    /// None when the anchor is not a known plant, or has a zero index (there
    /// is then no scale, and the caller runs no nutrient model at all rather
    /// than invent one, even for crops with removal columns: a data folder
    /// that has lost its anchor is broken, and nothing going short is the
    /// safe way for it to fail).
    pub fn demand_scale(&self, anchor_def: Option<&PlantDef>) -> Option<Npk> {
        let def = anchor_def?;
        let (n, p, k) = (def.nutrient_n as f64, def.nutrient_p as f64, def.nutrient_k as f64);
        if n <= 0.0 || p <= 0.0 || k <= 0.0 {
            return None;
        }
        let a = &self.demand_anchor;
        Some(Npk::new(a.n_g_per_kg / n, a.p2o5_g_per_kg / p, a.k2o_g_per_kg / k))
    }

    /// `demand_scale` with the anchor looked up in `plants`.
    pub fn scale_for(&self, plants: &super::PlantRegistry) -> Option<Npk> {
        self.demand_scale(plants.get(&self.demand_anchor.plant))
    }
}

// -- The model ------------------------------------------------------------------

/// The harvest a crop is expected to give at full health, kg: the middle of
/// its plants.csv yield range times the items.csv mass of one harvested item.
/// Zero for a crop whose harvest has no mass (it then takes nothing out).
pub fn expected_harvest_kg(def: &PlantDef, item_kg: f64) -> f64 {
    let lo = def.yield_min.max(0.0) as f64;
    let hi = (def.yield_max as f64).max(lo);
    (lo + hi) / 2.0 * item_kg.max(0.0)
}

/// A crop's nutrient need over one growing season, grams: its expected
/// harvest times what each kg of it removes (`removal_per_kg`). For the
/// anchor tomato this is exactly the cited removal of its 1.0 kg harvest.
pub fn season_need(def: &PlantDef, harvest_kg: f64, scale: Npk) -> Npk {
    removal_per_kg(def, scale).scaled(harvest_kg.max(0.0))
}

/// Grams of N, P2O5 and K2O one kg of this crop's harvest carries away.
///
/// Per nutrient: the crop's own plants.csv removal column where it is
/// filled (2026-09-26; every value cited in data/garden/nutrients.ron under
/// REMOVAL COLUMNS), else its relative plants.csv index times the per-index
/// scale (`NutrientData::demand_scale`), the anchor-scaled reading every crop
/// used before the columns existed. The fallback is kept because the indices
/// are not kilograms of anything: they understate dense harvests (a wheat
/// unit read 7 g of N a season against the 146 g its cited column gives), so
/// a crop only uses one until its removal is sourced. Each nutrient falls back
/// on its own, so a crop with a cited N and no cited potash still gets its
/// real N.
pub fn removal_per_kg(def: &PlantDef, scale: Npk) -> Npk {
    let pick = |column: Option<f32>, index: f32, per_index: f64| -> f64 {
        match column {
            Some(g) if g.is_finite() && g > 0.0 => f64::from(g),
            _ => f64::from(index.max(0.0)) * per_index,
        }
    };
    Npk::new(
        pick(def.removal_n, def.nutrient_n, scale.n),
        pick(def.removal_p2o5, def.nutrient_p, scale.p2o5),
        pick(def.removal_k2o, def.nutrient_k, scale.k2o),
    )
}

/// What a freshly prepared unit holds for a crop with this season need.
pub fn fresh_store(need: Npk) -> Npk {
    need.scaled(FRESH_UNIT_SEASONS)
}

/// How well supplied a crop is, 0..1, and which nutrient is the scarcest.
///
/// Per nutrient: the store over the reserve (`RESERVE_SEASONS` of the need),
/// clamped to 0..1, so 1 while the unit holds the reserve and falling in a
/// straight line to 0 as it empties. The crop's sufficiency is the MINIMUM of
/// the three (Liebig's law of the minimum: growth is limited by the scarcest
/// resource, and plenty of the others does not make up for it). A nutrient
/// the crop does not need counts as fully supplied. Ties name nitrogen first.
pub fn sufficiency(store: &Npk, need: &Npk) -> (f32, Nutrient) {
    let one = |have: f64, want: f64| -> f32 {
        if want <= 0.0 {
            1.0
        } else {
            (have / (want * RESERVE_SEASONS)).clamp(0.0, 1.0) as f32
        }
    };
    let each = [
        (one(store.n, need.n), Nutrient::Nitrogen),
        (one(store.p2o5, need.p2o5), Nutrient::Phosphorus),
        (one(store.k2o, need.k2o), Nutrient::Potassium),
    ];
    let mut worst = each[0];
    for s in &each[1..] {
        if s.0 < worst.0 {
            worst = *s;
        }
    }
    worst
}

/// The highest health a crop with this sufficiency can hold: 100 when fully
/// supplied, down to `NUTRIENT_HEALTH_FLOOR` when the scarcest nutrient is
/// gone.
pub fn health_ceiling(sufficiency: f32) -> f32 {
    NUTRIENT_HEALTH_FLOOR + (100.0 - NUTRIENT_HEALTH_FLOOR) * sufficiency.clamp(0.0, 1.0)
}

/// What the automatic feeder keeps in a unit for slider value `v` (0..1):
/// `v / FEED_NEUTRAL` reserves. So 0.5 holds exactly the reserve (never
/// short), 1.0 holds two, 0.25 half of one (the crop settles at half
/// sufficiency), 0 feeds nothing.
pub fn feed_target(need: Npk, v: f32) -> Npk {
    let reserves = f64::from(v.clamp(0.0, 1.0) / FEED_NEUTRAL);
    need.scaled(RESERVE_SEASONS * reserves)
}

/// How much of one fertilizer item brings `store` up to `target` in the
/// nutrient it lacks most, as a fraction of an item (0 when nothing lacks).
/// The dose comes in the fertilizer's own ratio, so topping up the scarcest
/// nutrient over-supplies the others: compost bought for its nitrogen leaves
/// phosphate and potash piling up, as it does in real compost-fed beds (the
/// WSU guide warns that rates "may need to" be based on P for that reason).
/// A nutrient the fertilizer does not carry cannot be topped up by it.
pub fn feed_dose(store: &Npk, target: &Npk, per_item: &Npk) -> f64 {
    feed_dose_for(store, target, per_item, [true; 3])
}

/// `feed_dose`, counting only the nutrients marked in `dose_for` (N, P2O5,
/// K2O). Urine is dosed for its nitrogen alone, the way the source says to
/// apply it (at a nitrogen fertilizer's rate): topping up potash with urine
/// would pour in several times the nitrogen the crop can use.
pub fn feed_dose_for(store: &Npk, target: &Npk, per_item: &Npk, dose_for: [bool; 3]) -> f64 {
    let lack = |on: bool, have: f64, want: f64, per: f64| -> f64 {
        if !on || per <= 0.0 || have >= want {
            0.0
        } else {
            (want - have) / per
        }
    };
    lack(dose_for[0], store.n, target.n, per_item.n)
        .max(lack(dose_for[1], store.p2o5, target.p2o5, per_item.p2o5))
        .max(lack(dose_for[2], store.k2o, target.k2o, per_item.k2o))
}

// -- Garden time -------------------------------------------------------------------

/// The share of a crop's growth clock at which it ripens: it matures on
/// entering its LAST stage, (n - 1) / n of the way (see `uptake_fraction`).
fn mature_at(n_stages: usize) -> f64 {
    if n_stages <= 1 {
        1.0
    } else {
        (n_stages - 1) as f64 / n_stages as f64
    }
}

/// Garden days a crop's unit lived through while its uptake share rose by
/// `d_uptake`: the crop's days to ripening times that share. This is the
/// clock the slow organic pool runs on, the same one the crop draws on, so
/// it pauses in the dark and runs ten times as fast at 10x growth.
pub fn garden_days(growth_days: f64, n_stages: usize, d_uptake: f64) -> f64 {
    (growth_days.max(0.0) * mature_at(n_stages) * d_uptake.max(0.0)).max(0.0)
}

/// Garden days left before a crop with this uptake share ripens.
pub fn days_to_maturity(growth_days: f64, n_stages: usize, uptake: f32) -> f64 {
    let u = f64::from(uptake).clamp(0.0, 1.0);
    growth_days.max(0.0) * mature_at(n_stages) * (1.0 - u)
}

// -- Compost's slow release: the organic N pool ---------------------------------

/// A year of garden time, in garden days.
pub const GARDEN_YEAR_DAYS: f64 = 365.0;

/// Organic N put into a unit within this many garden days of its youngest
/// cohort joins that cohort instead of starting a new one. The feeder adds a
/// few milligrams every tick, and a cohort per tick would grow without end;
/// 30 days keeps each cohort's age right to within a month of a year-long
/// schedule, which is finer than the source's own resolution (whole years).
pub const COHORT_MERGE_DAYS: f64 = 30.0;

/// Bank `grams` of organic N in a unit's pool (`SoilMemory::organic`), as a
/// new cohort aged 0, or into the youngest cohort if it is under
/// `COHORT_MERGE_DAYS` old. The merged age is the mass-weighted mean, so a
/// big new application pulls the cohort's age toward zero.
pub fn bank_organic(pool: &mut Vec<crate::ecs::components::OrganicCohort>, grams: f64) {
    if !(grams > 0.0) {
        return;
    }
    if let Some(young) = pool.last_mut() {
        if young.age_days < COHORT_MERGE_DAYS {
            let total = young.n + grams;
            young.age_days = if total > 0.0 { young.age_days * young.n / total } else { 0.0 };
            young.n = total;
            return;
        }
    }
    pool.push(crate::ecs::components::OrganicCohort { age_days: 0.0, n: grams });
}

/// The yearly percent for organic N of this age: year 1 is `yearly_pct[0]`,
/// and the last entry holds for every year after the list.
fn yearly_rate(yearly_pct: &[f64], age_days: f64) -> f64 {
    if yearly_pct.is_empty() {
        return 0.0;
    }
    let year = (age_days.max(0.0) / GARDEN_YEAR_DAYS).floor() as usize;
    (yearly_pct[year.min(yearly_pct.len() - 1)] / 100.0).clamp(0.0, 1.0)
}

/// Age every cohort in a unit's pool by `days` garden days and return the
/// grams of N released, which become plant-available in the unit.
///
/// Each year's percent is a share of what is STILL in the soil at the start
/// of that year (the CDFA wording: "5% of the remaining organically-bound
/// nitrogen in the second year"). Within a year it comes out at an even
/// proportional pace, `1 - (1 - r)^(days / 365)`, so a full year releases
/// exactly r of what the cohort held at its start however the year is
/// sliced into ticks. A step that crosses a year boundary is split there, so
/// the offline catch-up's one big step gets the same answer as many small
/// ones. Cohorts old enough to sit on the last (flat) rate merge into one.
pub fn release_organic(
    pool: &mut Vec<crate::ecs::components::OrganicCohort>,
    days: f64,
    yearly_pct: &[f64],
) -> f64 {
    if !(days > 0.0) || pool.is_empty() {
        return 0.0;
    }
    let mut released = 0.0;
    for c in pool.iter_mut() {
        let mut left = days;
        while left > 0.0 && c.n > 0.0 {
            let rate = yearly_rate(yearly_pct, c.age_days);
            // Days to the end of this cohort's current year, where the rate
            // may change. Past the list's end the rate is flat for good.
            let year = (c.age_days / GARDEN_YEAR_DAYS).floor();
            let flat = yearly_pct.is_empty() || year as usize >= yearly_pct.len().saturating_sub(1);
            let step = if flat {
                left
            } else {
                ((year + 1.0) * GARDEN_YEAR_DAYS - c.age_days).max(1e-9).min(left)
            };
            let out = c.n * (1.0 - (1.0 - rate).powf(step / GARDEN_YEAR_DAYS));
            c.n -= out;
            released += out;
            c.age_days += step;
            left -= step;
        }
        if c.n <= 0.0 {
            c.age_days += left.max(0.0);
        }
    }
    // Cohorts on the flat tail release at the same rate, so one will do.
    // Cohorts are kept oldest first (new ones are pushed on the end), so the
    // settled ones are always at the front; only rebuild when two have
    // settled or one has run out, which is rare, since this runs per crop
    // per tick.
    let flat_from = GARDEN_YEAR_DAYS * yearly_pct.len().saturating_sub(1) as f64;
    let settled_count = pool.iter().filter(|c| c.age_days >= flat_from).count();
    if settled_count <= 1 && pool.iter().all(|c| c.n > 1e-12) {
        return released;
    }
    let (mut settled, mut rest): (Vec<_>, Vec<_>) = pool.drain(..).partition(|c| c.age_days >= flat_from);
    rest.retain(|c| c.n > 1e-12);
    settled.retain(|c| c.n > 1e-12);
    if !settled.is_empty() {
        let n: f64 = settled.iter().map(|c| c.n).sum();
        let age = settled.iter().map(|c| c.age_days).fold(0.0, f64::max);
        pool.push(crate::ecs::components::OrganicCohort { age_days: age, n });
    }
    pool.extend(rest);
    released
}

/// Grams of organic N still banked in a unit's pool.
pub fn organic_total(pool: &[crate::ecs::components::OrganicCohort]) -> f64 {
    pool.iter().map(|c| c.n).sum()
}

// -- Legumes: nitrogen from the air ---------------------------------------------

/// What a crop draws from its unit over a season: its season need, less the
/// share of its N it fixes from the air (plants.csv `n_fixed_pct`, as 0..1).
/// P and K are untouched: no plant fixes those.
pub fn soil_draw(need: Npk, fixed_share: f64) -> Npk {
    Npk::new(need.n * (1.0 - fixed_share.clamp(0.0, 1.0)), need.p2o5, need.k2o)
}

/// Grams of N a harvested legume leaves in its unit for the next crop: the
/// fixed share of the N in the roots, nodules and haulm that stay behind.
/// With `removal_n` the N its harvest carried away and `harvest_n_share`
/// the share of the whole plant's N that harvest was, the plant held
/// `removal_n / harvest_n_share`, the part left behind is that minus the
/// removal, and `fixed_share` of it came from the air, so it is new to the
/// unit. (The soil-derived rest of the residue is the unit's own N going
/// back, which the removal-only model already nets out for every crop.)
/// Zero for a non-legume or a nonsense share.
pub fn legume_credit_n(removal_n: f64, fixed_share: f64, harvest_n_share: f64) -> f64 {
    let f = fixed_share.clamp(0.0, 1.0);
    if f <= 0.0 || !(harvest_n_share > 0.0) || harvest_n_share >= 1.0 || !(removal_n > 0.0) {
        return 0.0;
    }
    f * removal_n * (1.0 / harvest_n_share - 1.0)
}

/// Share (0..1) of its season need a crop should have drawn by now, from its
/// growth clock: `clock_progress` is the crop's progress through its real
/// growth days, times the growth-speed setting and the outdoor climate
/// factor, but NOT its health (so a stunted crop still wants feeding, and
/// fertilizing it is never waiting on its own recovery). A crop matures when
/// it enters its LAST stage, at (n - 1) / n of the way, so the whole season's
/// need is drawn by then.
pub fn uptake_fraction(clock_progress: f32, n_stages: usize) -> f32 {
    if !clock_progress.is_finite() {
        return 0.0;
    }
    if n_stages <= 1 {
        return clock_progress.clamp(0.0, 1.0);
    }
    let mature_at = (n_stages - 1) as f32 / n_stages as f32;
    (clock_progress / mature_at).clamp(0.0, 1.0)
}

/// Take `want` out of `store`, as much of each nutrient as it has, and
/// return what was taken. Whatever the store cannot give is simply not
/// taken: the crop went without, and its health cap already says so.
pub fn draw(store: &mut Npk, want: Npk) -> Npk {
    let take = |have: &mut f64, w: f64| -> f64 {
        let t = w.max(0.0).min(*have).max(0.0);
        *have -= t;
        t
    };
    Npk::new(
        take(&mut store.n, want.n),
        take(&mut store.p2o5, want.p2o5),
        take(&mut store.k2o, want.k2o),
    )
}

// -- Soil memory: what an emptied unit still holds ----------------------------

/// The world's one `SoilMemory`, spawned the first time it is needed.
pub fn soil_memory_entity(world: &mut hecs::World) -> hecs::Entity {
    if let Some((e, _)) = world.query::<&crate::ecs::components::SoilMemory>().iter().next() {
        return e;
    }
    world.spawn((crate::ecs::components::SoilMemory::default(),))
}

/// Remember what is left in unit `slot` of grow area `area` once its crop is
/// gone (harvested, or cleared away dead), for the next crop sown there.
pub fn remember(world: &mut hecs::World, area: &str, slot: u32, store: Npk) {
    let e = soil_memory_entity(world);
    if let Ok(mut mem) = world.get::<&mut crate::ecs::components::SoilMemory>(e) {
        mem.units.entry(area.to_string()).or_default().insert(slot, store);
    }
}

/// Take what a unit was left with, if anything is remembered for it. Taken,
/// not copied: from here on the soil rides on the new crop.
pub fn recall(world: &mut hecs::World, area: &str, slot: u32) -> Option<Npk> {
    let e = world.query::<&crate::ecs::components::SoilMemory>().iter().next().map(|(e, _)| e)?;
    let mut mem = world.get::<&mut crate::ecs::components::SoilMemory>(e).ok()?;
    mem.units.get_mut(area).and_then(|slots| slots.remove(&slot))
}

/// Bank `grams` of organic N in unit `slot` of grow area `area`, where it
/// stays through every crop sown there and releases slowly (`release_organic`).
pub fn bank_organic_in(world: &mut hecs::World, area: &str, slot: u32, grams: f64) {
    if !(grams > 0.0) {
        return;
    }
    let e = soil_memory_entity(world);
    if let Ok(mut mem) = world.get::<&mut crate::ecs::components::SoilMemory>(e) {
        let pool = mem.organic.entry(area.to_string()).or_default().entry(slot).or_default();
        bank_organic(pool, grams);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::farming::PlantRegistry;
    use crate::systems::inventory::ItemRegistry;

    fn shipped() -> (NutrientData, PlantRegistry, ItemRegistry) {
        let data = NutrientData::parse(NUTRIENTS_RON).expect("shipped nutrients.ron parses");
        let plants = PlantRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/plants.csv"
        )))
        .expect("plants.csv");
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        (data, plants, items)
    }

    /// The shipped compost bag is what the WSU worksheet says a 2.0 kg bag of
    /// food and yard waste compost gives in its first season: 1.3 g of N
    /// (7% of 18.8), 6.4 g of P2O5 and 11.6 g of K2O. Seen red by changing
    /// n_available to 0.7 in the data (the UMN sheet's example figure).
    #[test]
    fn a_bag_of_compost_gives_the_wsu_first_season_nutrients() {
        let (data, _plants, items) = shipped();
        let compost = data.fertilizer("fertilizer_0").expect("compost is listed");
        let bag_kg = f64::from(items.mass_for("fertilizer_0"));
        assert!((bag_kg - 2.0).abs() < 1e-6, "the bag is 2.0 kg in items.csv, got {bag_kg}");
        let g = compost.available_per_item(bag_kg);
        assert!((g.n - 1.316).abs() < 1e-9, "available N {}", g.n);
        assert!((g.p2o5 - 6.4).abs() < 1e-9, "P2O5 {}", g.p2o5);
        assert!((g.k2o - 11.6).abs() < 1e-9, "K2O {}", g.k2o);
        // The feeder's order (2026-09-26): urine first for the nitrogen,
        // compost after it for the phosphorus and potassium.
        let order: Vec<&str> = data.fertilizers.iter().map(|f| f.item.as_str()).collect();
        assert_eq!(order, ["urine_stored_0", "fertilizer_0"], "the feeder's fertilizers, in order");
    }

    /// A bag's organic rest is the 93% of its 18.8 g of N that is not
    /// available in its first season: 17.484 g, banked in the unit. Seen red
    /// by dropping `organic_rest: true` from the compost entry in the data
    /// (nothing banked).
    #[test]
    fn a_bag_of_compost_banks_the_rest_of_its_nitrogen_as_organic_n() {
        let (data, _plants, items) = shipped();
        let dose = data.fertilizer("fertilizer_0").unwrap().dose(f64::from(items.mass_for("fertilizer_0")));
        assert!((dose.organic_n - 18.8 * 0.93).abs() < 1e-9, "organic N {}", dose.organic_n);
        assert!((dose.available.n + dose.organic_n - 18.8).abs() < 1e-9, "all 18.8 g accounted for");
        assert_eq!(dose.dose_for, [true; 3], "compost is dosed by whichever nutrient is short");
        assert_eq!(dose.withhold_days, 0.0, "compost has no withholding period");
    }

    /// Compost's organic N comes back at the CDFA schedule applied to the
    /// WSU first year: nothing more in its first year (that season's 7% was
    /// credited when it went on), 3.5% of what is left in its second, and 2%
    /// of what is left every year after. And a year is a year however it is
    /// sliced into ticks, including one tick that crosses two birthdays (the
    /// offline catch-up). Seen red two ways: `yearly_rate` reading the LAST
    /// entry for every age (year one released 0.35 g), and the year-boundary
    /// split removed from `release_organic` (one step over three years
    /// released 0 g against 0.95 g day by day).
    #[test]
    fn compost_releases_the_rest_of_its_nitrogen_in_later_years_at_the_cited_schedule() {
        let (data, _plants, _items) = shipped();
        let yearly = data.organic_n_release.yearly_pct.clone();
        assert_eq!(yearly, [0.0, 3.5, 2.0], "the shipped schedule");
        let banked = 18.8 * 0.93;
        let mut pool = Vec::new();
        bank_organic(&mut pool, banked);

        let year1 = release_organic(&mut pool, GARDEN_YEAR_DAYS, &yearly);
        assert!(year1.abs() < 1e-12, "the first year's share was given up front: {year1}");
        let year2 = release_organic(&mut pool, GARDEN_YEAR_DAYS, &yearly);
        assert!((year2 - banked * 0.035).abs() < 1e-9, "year two: 3.5% of {banked} = {}, got {year2}", banked * 0.035);
        let left = banked * 0.965;
        let year3 = release_organic(&mut pool, GARDEN_YEAR_DAYS, &yearly);
        assert!((year3 - left * 0.02).abs() < 1e-9, "year three: 2% of what is left, got {year3}");
        let year4 = release_organic(&mut pool, GARDEN_YEAR_DAYS, &yearly);
        assert!((year4 - left * 0.98 * 0.02).abs() < 1e-9, "year four: 2% again, got {year4}");
        assert!(
            (organic_total(&pool) + year2 + year3 + year4 - banked).abs() < 1e-9,
            "nothing made or lost: what is released plus what is left is what went in"
        );

        // Sliced into days, or in one step that crosses two birthdays: the same.
        let mut daily = Vec::new();
        bank_organic(&mut daily, banked);
        let by_day: f64 = (0..(3 * 365)).map(|_| release_organic(&mut daily, 1.0, &yearly)).sum();
        let mut once = Vec::new();
        bank_organic(&mut once, banked);
        let at_once = release_organic(&mut once, 3.0 * GARDEN_YEAR_DAYS, &yearly);
        assert!((by_day - (year2 + year3)).abs() < 1e-9, "day by day over three years: {by_day}");
        assert!((at_once - by_day).abs() < 1e-9, "one step over three years: {at_once} vs {by_day}");
    }

    /// Applying compost every year builds the soil up: the release grows
    /// year on year even though each year's bag is the same, which is what
    /// a real garden's organic matter does. And the feeder's milligram doses
    /// do not pile up a cohort per tick. Seen red by making `bank_organic`
    /// always push a new cohort (the pool grew to one entry per dose).
    #[test]
    fn composting_year_after_year_builds_the_soil_up() {
        let yearly = [0.0, 3.5, 2.0];
        let mut pool = Vec::new();
        let mut released = Vec::new();
        for _ in 0..10 {
            // A bag's organic N a year, as a feeder would give it: a
            // thousand small doses over the first month.
            for _ in 0..1000 {
                bank_organic(&mut pool, 17.484 / 1000.0);
                release_organic(&mut pool, 30.0 / 1000.0, &yearly);
            }
            released.push(release_organic(&mut pool, GARDEN_YEAR_DAYS - 30.0, &yearly));
        }
        for w in released.windows(2).skip(1) {
            assert!(w[1] > w[0], "each year gives more than the last: {released:?}");
        }
        assert!(pool.len() <= 3, "a few cohorts, not one per dose: {}", pool.len());
    }

    /// A person-day of stored urine is Jonsson et al.'s Swedish urine: 10.9
    /// g of N (0.727% of 1.5 kg), all of it available, with 2.28 g P2O5 and
    /// 3.6 g K2O, none of it banked, dosed by nitrogen alone, and never on a
    /// crop within 30 garden days of ripening. Seen red by setting the
    /// urine's n_available to 0.07 (compost's) in the data (1.3 g, not 10.9).
    #[test]
    fn stored_urine_adds_its_cited_nitrogen_all_available_and_keeps_the_month() {
        let (data, _plants, items) = shipped();
        let kg = f64::from(items.mass_for("urine_stored_0"));
        assert!((kg - 1.5).abs() < 1e-6, "one person-day is 1.5 kg in items.csv, got {kg}");
        let dose = data.fertilizer("urine_stored_0").expect("urine is listed").dose(kg);
        assert!((dose.available.n - 4000.0 / 550.0 * 1.5).abs() < 0.01, "N {} g", dose.available.n);
        assert!((dose.available.n - 10.905).abs() < 1e-9, "N {} g", dose.available.n);
        assert!((dose.available.p2o5 - 2.28).abs() < 1e-9, "P2O5 {}", dose.available.p2o5);
        assert!((dose.available.k2o - 3.6).abs() < 1e-9, "K2O {}", dose.available.k2o);
        assert_eq!(dose.organic_n, 0.0, "urine N is all available, nothing banked");
        assert_eq!(dose.dose_for, [true, false, false], "dosed by nitrogen alone");
        assert!(dose.allowed(30.0) && dose.allowed(90.0), "a month or more before harvest: allowed");
        assert!(!dose.allowed(29.9), "within the month: withheld");
        // Dosed for N only: a unit short of potash is not poured full of
        // urine to cover it.
        let store = Npk::new(1.0, 0.0, 0.0);
        let target = Npk::new(1.0, 1.0, 1.0);
        assert_eq!(feed_dose_for(&store, &target, &dose.available, dose.dose_for), 0.0);
        assert!(feed_dose(&store, &target, &dose.available) > 0.0, "unmasked it would have been");
    }

    /// A legume draws only the share of its N it does not fix, and leaves
    /// the fixed share of its roots' N behind; soybean at its own 55% ends
    /// the season where it started (Salvagiotti et al.'s near-neutral
    /// balance), a stronger fixer leaves the unit richer and a weaker one
    /// poorer. Seen red by crediting the whole residue N instead of its
    /// fixed share (soybean then left the unit richer).
    #[test]
    fn a_legume_draws_less_and_leaves_its_fixed_nitrogen_behind() {
        let (data, plants, _items) = shipped();
        let share = data.legume_harvest_n_share;
        assert!((share - 0.55).abs() < 1e-12, "shipped share {share}");
        let soy = f64::from(plants.get("soybean").unwrap().n_fixed_share);
        assert!((soy - 0.55).abs() < 1e-6, "soybean fixes 55%, got {soy}");
        assert_eq!(plants.get("tomato").unwrap().n_fixed_share, 0.0, "a tomato fixes nothing");
        assert_eq!(plants.get("peanut").unwrap().n_fixed_share, 0.0, "no figure sourced yet: none");

        // (1e-6, not 1e-9: plants.csv shares are f32, so 0.55 arrives as
        // 0.55000001.)
        let removal = Npk::new(10.0, 3.0, 4.0);
        let drawn = soil_draw(removal, soy);
        assert!((drawn.n - 4.5).abs() < 1e-6 && drawn.p2o5 == 3.0 && drawn.k2o == 4.0, "{drawn:?}");
        let credit = legume_credit_n(removal.n, soy, share);
        assert!((credit - drawn.n).abs() < 1e-6, "soybean: credit {credit} = what it drew {}", drawn.n);

        let net = |f: f64| legume_credit_n(removal.n, f, share) - soil_draw(removal, f).n;
        let fava = f64::from(plants.get("fava_bean").unwrap().n_fixed_share);
        let bean = f64::from(plants.get("bean").unwrap().n_fixed_share);
        assert!(net(fava) > 0.0, "fava bean (67%) leaves the unit richer: {}", net(fava));
        assert!(net(bean) < 0.0, "common bean (26%) leaves it poorer: {}", net(bean));
        assert!(net(0.0) == -removal.n, "a non-legume takes its whole removal");
        assert_eq!(legume_credit_n(removal.n, soy, 1.0), 0.0, "a harvest that takes everything leaves nothing");
    }

    /// The anchor tomato's season need is exactly the cited removal of its
    /// expected 1.0 kg harvest (2 to 8 fruit of 0.2 kg), which is the whole
    /// point of the anchor: the scale is a published number, not a guess.
    /// Seen red by setting the anchor's N to 1.6 g/kg in the data.
    #[test]
    fn the_anchor_tomato_needs_its_published_removal() {
        let (data, plants, items) = shipped();
        let tomato = plants.get("tomato").unwrap();
        let scale = data.demand_scale(Some(tomato)).expect("tomato anchors the scale");
        let kg = expected_harvest_kg(tomato, f64::from(items.mass_for("vegetable_tomato_0")));
        assert!((kg - 1.0).abs() < 1e-6, "tomato's expected harvest {kg} kg");
        let need = season_need(tomato, kg, scale);
        assert!((need.n - 1.5).abs() < 1e-6, "N {}", need.n);
        assert!((need.p2o5 - 0.9).abs() < 1e-6, "P2O5 {}", need.p2o5);
        assert!((need.k2o - 4.0).abs() < 1e-6, "K2O {}", need.k2o);
        // The tomato carries the anchor's figures in its own removal columns
        // too, so it reads the same by either route.
        assert_eq!((tomato.removal_n, tomato.removal_p2o5, tomato.removal_k2o), (Some(1.5), Some(0.9), Some(4.0)));
    }

    /// A crop with removal columns needs exactly its removal times its
    /// harvest, whatever its index says: a tomato twin whose index is tripled
    /// but whose columns read 10 / 5 / 20 g per kg needs those grams times its
    /// 2.5 kg. And wheat, the crop the indices understated most, now needs its
    /// cited 20.8 g N (NRCS Table 6-6), 8.33 g P2O5 and 5.83 g K2O (A2809
    /// Table 4.2) per kg times its 7 kg unit: 145.6 / 58.3 / 40.8 g a season,
    /// against the 7.0 / 5.0 / 8.4 g its index gave. Seen red by making
    /// `removal_per_kg` ignore the columns (the twin then needed 11.25 g of N,
    /// its tripled index, instead of 25).
    #[test]
    fn a_crop_with_removal_columns_needs_exactly_removal_times_harvest() {
        let (data, plants, items) = shipped();
        let scale = data.scale_for(&plants).expect("the anchor scales");
        let mut twin = plants.get("tomato").unwrap().clone();
        twin.nutrient_n *= 3.0;
        twin.nutrient_p *= 3.0;
        twin.nutrient_k *= 3.0;
        (twin.removal_n, twin.removal_p2o5, twin.removal_k2o) = (Some(10.0), Some(5.0), Some(20.0));
        let need = season_need(&twin, 2.5, scale);
        assert!((need.n - 25.0).abs() < 1e-9, "N {}", need.n);
        assert!((need.p2o5 - 12.5).abs() < 1e-9, "P2O5 {}", need.p2o5);
        assert!((need.k2o - 50.0).abs() < 1e-9, "K2O {}", need.k2o);

        let wheat = plants.get("wheat").unwrap();
        let kg = expected_harvest_kg(wheat, f64::from(items.mass_for("grain_wheat_0")));
        assert!((kg - 7.0).abs() < 1e-6, "a wheat unit is 8 to 20 items of 0.5 kg: {kg}");
        let need = season_need(wheat, kg, scale);
        assert!((need.n - 20.8 * 7.0).abs() < 1e-3, "wheat N {}", need.n);
        assert!((need.p2o5 - 8.33 * 7.0).abs() < 1e-3, "wheat P2O5 {}", need.p2o5);
        assert!((need.k2o - 5.83 * 7.0).abs() < 1e-3, "wheat K2O {}", need.k2o);
    }

    /// A crop whose removal columns are blank keeps the need it had before
    /// the columns existed: its index times the anchor's per-index scale times
    /// its harvest (the apple, unsourced, needs 0.10 x 10 g x 10 kg = 10 g of
    /// N). And each nutrient falls back on its own: a twin with only its N
    /// cited takes N from the column and P2O5 and K2O from its index. Seen red
    /// by reading a blank column as zero instead of falling back (the apple
    /// then needed nothing at all).
    #[test]
    fn a_crop_without_removal_columns_keeps_the_index_based_need() {
        let (data, plants, items) = shipped();
        let scale = data.scale_for(&plants).expect("the anchor scales");
        let apple = plants.get("apple").unwrap();
        assert_eq!((apple.removal_n, apple.removal_p2o5, apple.removal_k2o), (None, None, None), "apple is unsourced");
        let kg = expected_harvest_kg(apple, f64::from(items.mass_for("fruit_apple_0")));
        let need = season_need(apple, kg, scale);
        let by_index = Npk::new(
            f64::from(apple.nutrient_n) * scale.n,
            f64::from(apple.nutrient_p) * scale.p2o5,
            f64::from(apple.nutrient_k) * scale.k2o,
        )
        .scaled(kg);
        assert!(need.n > 0.0 && need.p2o5 > 0.0 && need.k2o > 0.0, "{need:?}");
        assert!((need.n - by_index.n).abs() < 1e-9, "N {} vs {}", need.n, by_index.n);
        assert!((need.p2o5 - by_index.p2o5).abs() < 1e-9, "P2O5 {} vs {}", need.p2o5, by_index.p2o5);
        assert!((need.k2o - by_index.k2o).abs() < 1e-9, "K2O {} vs {}", need.k2o, by_index.k2o);
        assert!((need.n - 10.0).abs() < 1e-3, "10 kg of apples at the tomato-anchored index: {}", need.n);

        let mut twin = apple.clone();
        twin.removal_n = Some(2.0);
        let mixed = season_need(&twin, kg, scale);
        assert!((mixed.n - 2.0 * kg).abs() < 1e-9, "N from the column: {}", mixed.n);
        assert!((mixed.p2o5 - by_index.p2o5).abs() < 1e-9 && (mixed.k2o - by_index.k2o).abs() < 1e-9, "{mixed:?}");
    }

    /// The shipped removal columns, as data. Every filled cell must be a
    /// positive number the loader actually read (a cell it cannot read
    /// silently falls back to the index, so a typo would hide), and in a
    /// believable proportion, so a unit slip cannot ship. The bounds, and why:
    ///
    /// - N 0.5 to 80 g per kg. The low end is a watery fruit (95% water at
    ///   1% N in its dry matter, the bottom of the NRCS handbook's "By weight,
    ///   nitrogen makes up from 1 to 4 percent of the plant's harvested
    ///   material"); the high end clears soybean seed, the richest harvest in
    ///   its Table 6-6 at 6.25%.
    /// - P2O5 0.3 to 25 and K2O 1 to 30 g per kg: the lowest and highest the
    ///   cited tables give (radish 0.46 and soybean 13.3 g P2O5; cucumber 1.8
    ///   and soybean 23.3 g K2O) with room either side. They catch a value
    ///   typed straight from lb per cwt, lb per bushel or lb per lb.
    /// - N against the crop's own protein (data/food/crop_nutrition.ron, from
    ///   USDA FoodData Central): USDA derives protein as measured N times a
    ///   factor of 5.3 to 6.25, so for the same food N over protein / 6.25 is
    ///   1.0 to 1.18. The removal tables weigh a slightly different product
    ///   (whole oats with the hull against groats: 0.72; paddy rice against
    ///   milled: 1.32), so 0.65 to 1.4 holds every shipped crop, and a lb per
    ///   ton figure typed as g per kg, which doubles it, lands above 1.4 for
    ///   all of them. (kg per tonne IS g per kg, so it is no slip.)
    /// - P2O5 and K2O against N: 0.08 to 1.8 and 0.1 to 6 (shipped: 0.11 to
    ///   1.5 and 0.17 to 4.5), so a grain's P or K typed per bushel, which is
    ///   dozens of times too small, fails. A doubled P or K value can pass:
    ///   nothing in the repo measures them independently.
    ///
    /// Seen red three ways: wheat N set to 41.6 (lb per ton read as g per kg;
    /// failed the protein band at 1.90), potato P2O5 set to 0.12 (the A2809
    /// lb per cwt typed raw; failed the range and the P2O5 to N band), and a
    /// cell set to "x" (not a number; the loader would have read it as blank
    /// and quietly fallen back to the index).
    #[test]
    fn shipped_removal_columns_are_read_and_plausible() {
        let csv = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/plants.csv"));
        let plants = PlantRegistry::from_csv(csv.as_bytes()).expect("plants.csv");
        let nutrition = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/food/crop_nutrition.ron"));
        let protein: std::collections::HashMap<&str, f64> = nutrition
            .lines()
            .filter_map(|l| {
                let id = l.split("plant_id: \"").nth(1)?.split('"').next()?;
                let p = l.split("protein_g:").nth(1)?.split(',').next()?.trim().parse().ok()?;
                Some((id, p))
            })
            .collect();
        assert!(protein.len() > 100, "crop_nutrition.ron read: {} rows", protein.len());

        let mut rows = csv.lines().filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty());
        let header: Vec<&str> = rows.next().expect("header").split(',').map(str::trim).collect();
        let col = |name: &str| header.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
        let cols = [col("removal_n_g_per_kg"), col("removal_p2o5_g_per_kg"), col("removal_k2o_g_per_kg")];
        let names = ["N", "P2O5", "K2O"];
        let ranges = [(0.5, 80.0), (0.3, 25.0), (1.0, 30.0)];

        let mut bad: Vec<String> = Vec::new();
        let mut filled = 0;
        for row in rows {
            let cells: Vec<&str> = row.split(',').map(str::trim).collect();
            let id = cells[0];
            let def = plants.get(id).unwrap_or_else(|| panic!("{id} is in plants.csv but not the registry"));
            let loaded = [def.removal_n, def.removal_p2o5, def.removal_k2o];
            let mut v = [None::<f64>; 3];
            for i in 0..3 {
                let text = cells.get(cols[i]).copied().unwrap_or("");
                if text.is_empty() {
                    if loaded[i].is_some() {
                        bad.push(format!("{id} {}: blank in the file but loaded as {:?}", names[i], loaded[i]));
                    }
                    continue;
                }
                match text.parse::<f64>() {
                    Ok(x) if x.is_finite() && x > 0.0 => {
                        v[i] = Some(x);
                        if loaded[i].map_or(true, |l| (f64::from(l) - x).abs() > 1e-5 * x) {
                            bad.push(format!("{id} {}: file says {x}, loader read {:?}", names[i], loaded[i]));
                        }
                        let (lo, hi) = ranges[i];
                        if !(lo..=hi).contains(&x) {
                            bad.push(format!("{id} {} = {x} g/kg is outside {lo} to {hi}", names[i]));
                        }
                    }
                    _ => bad.push(format!("{id} {}: {text:?} is not a positive number of g per kg", names[i])),
                }
            }
            if v.iter().any(Option::is_some) {
                filled += 1;
            }
            if let Some(n) = v[0] {
                if let Some(p) = protein.get(id).copied().filter(|p| *p > 0.0) {
                    let ratio = n / (p * 10.0 / 6.25);
                    if !(0.65..=1.4).contains(&ratio) {
                        bad.push(format!("{id} N {n} is {ratio:.2} x the N in its protein ({p} g/100 g)"));
                    }
                }
                if let Some(p) = v[1].filter(|p| !(0.08..=1.8).contains(&(p / n))) {
                    bad.push(format!("{id} P2O5 {p} is {:.3} x its N", p / n));
                }
                if let Some(k) = v[2].filter(|k| !(0.1..=6.0).contains(&(k / n))) {
                    bad.push(format!("{id} K2O {k} is {:.3} x its N", k / n));
                }
            }
        }
        assert!(bad.is_empty(), "removal columns:\n{}", bad.join("\n"));
        assert!(filled >= 60, "only {filled} crops carry removal columns");
        // The family home's staples all carry the three columns.
        for id in ["wheat", "rice", "potato", "bean", "soybean", "pea", "lentil", "chickpea", "tomato", "lettuce", "sunflower"] {
            let d = plants.get(id).unwrap();
            assert!(d.removal_n.is_some() && d.removal_p2o5.is_some() && d.removal_k2o.is_some(), "{id} is sourced");
        }
    }

    /// Liebig's law of the minimum: plenty of N and K does not make up for no
    /// P. Seen red by taking the MAXIMUM of the three instead.
    #[test]
    fn the_scarcest_nutrient_sets_the_sufficiency() {
        let need = Npk::new(1.0, 1.0, 1.0);
        let (s, which) = sufficiency(&Npk::new(10.0, 0.0, 10.0), &need);
        assert_eq!(s, 0.0, "no phosphorus at all");
        assert_eq!(which, Nutrient::Phosphorus);
        let (s, _) = sufficiency(&Npk::new(10.0, 0.05, 10.0), &need);
        assert!((s - 0.5).abs() < 1e-6, "half the reserve of P is half sufficiency, got {s}");
        let (s, _) = sufficiency(&Npk::new(0.1, 0.1, 0.1), &need);
        assert_eq!(s, 1.0, "the full reserve of each is enough");
        let (s, _) = sufficiency(&Npk::ZERO, &Npk::ZERO);
        assert_eq!(s, 1.0, "a crop that needs nothing is never short");
        assert_eq!(health_ceiling(0.0), NUTRIENT_HEALTH_FLOOR);
        assert_eq!(health_ceiling(1.0), 100.0);
    }

    /// The feeder at the slider's default holds exactly the reserve, more
    /// holds more, and its dose follows the nutrient the store lacks most.
    /// Seen red by making `feed_target` ignore the slider (one reserve always).
    #[test]
    fn the_feeder_tops_up_the_scarcest_nutrient_in_the_fertilizers_ratio() {
        let need = Npk::new(2.0, 1.0, 4.0);
        assert_eq!(feed_target(need, FEED_NEUTRAL), need.scaled(RESERVE_SEASONS));
        assert_eq!(feed_target(need, 1.0), need.scaled(2.0 * RESERVE_SEASONS));
        assert_eq!(feed_target(need, 0.0), Npk::ZERO);
        // A compost bag (N 1.316, P2O5 6.4, K2O 11.6) into an empty unit with
        // a target of 0.2 / 0.1 / 0.4: nitrogen needs the most of a bag.
        let bag = Npk::new(1.316, 6.4, 11.6);
        let dose = feed_dose(&Npk::ZERO, &feed_target(need, FEED_NEUTRAL), &bag);
        assert!((dose - 0.2 / 1.316).abs() < 1e-12, "dose {dose}");
        assert_eq!(feed_dose(&need, &feed_target(need, 1.0), &bag), 0.0, "already above target");
    }

    /// The whole season's need is drawn by the time the crop enters its last
    /// stage, and a draw never takes more than the unit holds. Seen red by
    /// dropping the maturity point (drawing on the raw clock).
    #[test]
    fn uptake_follows_the_clock_to_maturity_and_draw_takes_what_is_there() {
        assert_eq!(uptake_fraction(0.0, 6), 0.0);
        assert!((uptake_fraction(5.0 / 12.0, 6) - 0.5).abs() < 1e-6, "half way to the last stage");
        assert_eq!(uptake_fraction(5.0 / 6.0, 6), 1.0, "all drawn on entering the last stage");
        assert_eq!(uptake_fraction(3.0, 6), 1.0);
        assert_eq!(uptake_fraction(f32::NAN, 6), 0.0);
        let mut store = Npk::new(1.0, 0.1, 2.0);
        let took = draw(&mut store, Npk::new(0.5, 0.5, 0.5));
        assert_eq!(took, Npk::new(0.5, 0.1, 0.5), "P gives what it has");
        assert_eq!(store, Npk::new(0.5, 0.0, 1.5));
    }
}
