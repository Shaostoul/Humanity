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
//! 2. A growing crop draws its season need (`season_need`, scaled from the
//!    plants.csv indices by a cited removal figure, see
//!    data/garden/nutrients.ron) in step with its growth clock, so it draws
//!    nothing in the dark and ten times as fast at the 10x growth setting.
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
//! pH and humidity stay out of scope: they are the next rungs (pH decides how
//! much of the store is available at all, so it slots in as a multiplier on
//! `sufficiency`).

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
}

fn all_available() -> f64 {
    1.0
}

impl FertilizerDef {
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
    /// None when the anchor is not a known plant, or has a zero index (there
    /// is then no scale, and the caller runs no nutrient model at all rather
    /// than invent one).
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
/// harvest times its plants.csv index times the per-index scale
/// (`NutrientData::demand_scale`). For the anchor tomato this is exactly the
/// cited removal of its 1.0 kg harvest; everything else is read against it.
pub fn season_need(def: &PlantDef, harvest_kg: f64, scale: Npk) -> Npk {
    let index = Npk::new(
        def.nutrient_n.max(0.0) as f64,
        def.nutrient_p.max(0.0) as f64,
        def.nutrient_k.max(0.0) as f64,
    );
    index.times(scale).scaled(harvest_kg.max(0.0))
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
    let lack = |have: f64, want: f64, per: f64| -> f64 {
        if per <= 0.0 || have >= want {
            0.0
        } else {
            (want - have) / per
        }
    };
    lack(store.n, target.n, per_item.n)
        .max(lack(store.p2o5, target.p2o5, per_item.p2o5))
        .max(lack(store.k2o, target.k2o, per_item.k2o))
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
fn soil_memory_entity(world: &mut hecs::World) -> hecs::Entity {
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
        assert_eq!(data.fertilizers[0].item, "fertilizer_0", "compost is the feeder's fertilizer");
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
