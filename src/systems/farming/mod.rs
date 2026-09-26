//! Farming system -- crop growth simulation driven by time, water, and plant data.
//!
//! Queries all entities with `CropInstance` and advances growth stages.
//! Plant definitions loaded from `data/plants.csv`.
//! Growth stages are data-driven: each plant species defines its own stage
//! names in plants.csv (colon-separated). Default stages are used when missing.

pub mod crops;
pub mod soil;
pub mod pests;
pub mod lighting;
pub mod soil_ph;
pub mod automation;
#[cfg(test)]
mod nutrient_tests;
#[cfg(test)]
mod pest_tests;
#[cfg(test)]
mod soil_ph_tests;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::components::{CropInstance, CropSoil, Npk, DEFAULT_GROWTH_STAGES, STAGE_DEAD};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// The crop growth multiplier the game ships with.
///
/// The operator, 2026-09-20, asked how fast a crop should grow in real time:
/// "I would like to have normal real growth speed but, with a custom option for
/// accelerating plant growth... it'd be nice for people to be like I want either
/// 1x speed or 10x or even 100x. For development purpose we could default to 10x
/// growth speed (not clock speed) just so we can actually test plant life cycles
/// without waiting days/weeks/months."
///
/// So this scales GROWTH PROGRESS only. The world clock, the day/night cycle,
/// weather and every other system keep running at real time: speeding the clock
/// instead would have dragged all of them along, which is not what was asked
/// for. `plants.csv` keeps its real agricultural `growth_days`, so the numbers
/// stay teachable and 1x remains a truthful mode rather than a handicap.
pub const DEFAULT_CROP_GROWTH_SPEED: f32 = 10.0;

/// The presets offered in Settings. 1x is real time, where the fastest crop in
/// `plants.csv` still takes hours; the faster rungs exist because a garden
/// nobody can watch change is a garden nobody learns from.
pub const CROP_GROWTH_SPEED_PRESETS: [f32; 3] = [1.0, 10.0, 100.0];

/// Lower bound: at zero, crops freeze forever and read as a BROKEN farm rather
/// than a slow one. Upper bound: past 1000x a crop ripens inside a single tick,
/// so the stage progression is never seen at all.
pub const MIN_CROP_GROWTH_SPEED: f32 = 0.01;
/// See [`MIN_CROP_GROWTH_SPEED`].
pub const MAX_CROP_GROWTH_SPEED: f32 = 1000.0;

/// Clamp a growth multiplier arriving from ANY source: the config file, the dev
/// IPC, a Settings slider. A NaN returns the default instead of propagating
/// into every crop's progress and stalling the whole garden at stage zero.
pub fn clamp_growth_speed(v: f32) -> f32 {
    if !v.is_finite() {
        return DEFAULT_CROP_GROWTH_SPEED;
    }
    v.clamp(MIN_CROP_GROWTH_SPEED, MAX_CROP_GROWTH_SPEED)
}


/// Plant definition loaded from plants.csv -- cached in DataStore as "plant_registry".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantDef {
    /// Unique plant ID (e.g., "tomato").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Total real-world days from seed to harvest.
    pub growth_days: f32,
    /// Water consumption in liters per day per plant.
    pub water_per_day: f32,
    /// Preferred growing seasons.
    pub seasons: Vec<String>,
    /// Ordered growth stage names for this plant species.
    /// Loaded from plants.csv `growth_stages` column (colon-separated).
    /// Falls back to DEFAULT_GROWTH_STAGES when empty.
    pub growth_stages: Vec<String>,
    /// Harvest yield range (units of produce per fully-grown plant). f32 because real
    /// crops can yield LESS than one unit per plant per harvest (saffron: 0.3 -- a few
    /// stigma threads); the harvest roll converts the continuous roll to whole inventory
    /// items by probabilistic rounding, preserving the expected value. Was u32, which made
    /// serde reject (and the registry silently drop) any plants.csv row with a fractional
    /// yield -- saffron was un-plantable for months before the 2026-07-01 fix.
    pub yield_min: f32,
    pub yield_max: f32,
    /// Relative nutrient demand fractions (N, P, K) from plants.csv. Shown per
    /// crop in the Garden table; feed the future shared-reservoir mix math.
    pub nutrient_n: f32,
    pub nutrient_p: f32,
    pub nutrient_k: f32,
    /// Preferred reservoir pH window (for the future tower compatibility check).
    pub ph_min: f32,
    pub ph_max: f32,
    /// Tolerated air/water temperature window, Celsius.
    pub temp_min_c: f32,
    pub temp_max_c: f32,
    /// Preferred relative-humidity window, 0..1.
    pub humidity_min: f32,
    pub humidity_max: f32,
    /// The items.csv id this plant harvests into (v0.749, ladder rung 6).
    /// Explicit per row (scripts/gen-harvest-items.js), so herbs, legumes,
    /// mushrooms, and fiber crops harvest to REAL items instead of a warning
    /// log. Empty = fall back to the legacy vegetable_/fruit_/grain_ prefix
    /// search (pre-column data still works).
    #[serde(default)]
    pub harvest_item: String,
    /// Does this species need light to grow? (2026-09-26, plants.csv
    /// `needs_light`.) True for every green plant: it grows only while its
    /// grow area is lit (see `light_growth_rate`). False for the fungi, which
    /// do not photosynthesise and grow on their substrate in light or dark.
    /// Defaults to true so a row without the column still needs light.
    #[serde(default = "default_needs_light")]
    pub needs_light: bool,
    /// Share of this crop's nitrogen it fixes from the air, 0..1 (plants.csv
    /// `n_fixed_pct` / 100, 2026-09-26). Nonzero only for the legumes with a
    /// cited figure; every other plant draws all its N from its unit. A
    /// legume draws only the rest (`soil::soil_draw`) and leaves the fixed N
    /// in its roots behind for the next crop (`soil::legume_credit_n`).
    #[serde(default)]
    pub n_fixed_share: f32,
    /// Grams of N, P2O5 and K2O one kg of this crop's harvest carries out of
    /// its unit (plants.csv `removal_n_g_per_kg`, `removal_p2o5_g_per_kg`,
    /// `removal_k2o_g_per_kg`, 2026-09-26), on the basis its harvest item
    /// represents. `None` where the column is blank: that nutrient's need then
    /// falls back to the relative `nutrient_*` index read against the anchor
    /// crop (`soil::removal_per_kg`). Each nutrient falls back on its own.
    #[serde(default)]
    pub removal_n: Option<f32>,
    #[serde(default)]
    pub removal_p2o5: Option<f32>,
    #[serde(default)]
    pub removal_k2o: Option<f32>,
}

fn default_needs_light() -> bool {
    true
}

/// Read the plants.csv `n_fixed_pct` cell as a 0..1 share. Blank, absent or
/// unparseable reads as 0 (no fixation), never as a dropped row: parsed from
/// text for the same reason as `parse_needs_light`.
fn parse_fixed_share(cell: &str) -> f32 {
    cell.trim().parse::<f32>().ok().filter(|v| v.is_finite()).map_or(0.0, |pct| (pct / 100.0).clamp(0.0, 1.0))
}

/// Read a plants.csv removal cell (grams per kg of harvest). Blank, absent,
/// unparseable, zero or negative reads as `None`, so the crop falls back to
/// its index for that nutrient and the row is never dropped: parsed from
/// text for the same reason as `parse_needs_light`. A filled cell that reads
/// as `None` is a data error, and the shipped-data lint in soil.rs fails on it
/// rather than let the fallback hide it.
fn parse_removal(cell: &str) -> Option<f32> {
    cell.trim().parse::<f32>().ok().filter(|v| v.is_finite() && *v > 0.0)
}

/// Read the plants.csv `needs_light` cell. Only an explicit no (`false`,
/// `no`, `0`) turns the light gate off; blank or absent means the species
/// needs light. Parsed from text rather than as a serde bool so a blank cell
/// can never make the row-resilient loader drop the whole plant.
fn parse_needs_light(cell: &str) -> bool {
    !matches!(cell.trim().to_ascii_lowercase().as_str(), "false" | "no" | "0")
}

impl PlantDef {
    /// Returns this plant's growth stages, falling back to defaults if empty.
    pub fn stages(&self) -> Vec<&str> {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES.iter().copied().collect()
        } else {
            self.growth_stages.iter().map(|s| s.as_str()).collect()
        }
    }

    /// Returns the first stage name (the initial stage when planted).
    pub fn first_stage(&self) -> &str {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES[0]
        } else {
            &self.growth_stages[0]
        }
    }

    /// Returns the last stage name (the harvest-ready stage).
    pub fn last_stage(&self) -> &str {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES[DEFAULT_GROWTH_STAGES.len() - 1]
        } else {
            &self.growth_stages[self.growth_stages.len() - 1]
        }
    }
}

/// Registry of all plant definitions, keyed by plant ID.
#[derive(Debug, Clone, Default)]
pub struct PlantRegistry {
    pub plants: HashMap<String, PlantDef>,
}

impl PlantRegistry {
    /// Look up a plant definition by ID.
    pub fn get(&self, id: &str) -> Option<&PlantDef> {
        self.plants.get(id)
    }

    /// Build the plant registry from raw `plants.csv` bytes.
    ///
    /// Uses the shared CSV loader (skips `#` comments, header-mapped, row-resilient).
    /// `growth_stages` and `seasons` are colon-separated lists. This is the
    /// constructor the runtime calls to populate `DataStore["plant_registry"]` —
    /// before v0.323 the CSV was loaded then discarded, so FarmingSystem ran on
    /// default growth stages with no species data.
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<PlantRow> = crate::assets::loader::parse_csv(data)?;
        let mut plants = HashMap::new();
        for row in rows {
            plants.insert(
                row.id.clone(),
                PlantDef {
                    id: row.id,
                    name: row.name,
                    growth_days: row.growth_days,
                    water_per_day: row.water_liters_per_day,
                    seasons: split_colon_list(&row.seasons),
                    growth_stages: split_colon_list(&row.growth_stages),
                    yield_min: row.yield_min,
                    yield_max: row.yield_max,
                    nutrient_n: row.nutrient_n,
                    nutrient_p: row.nutrient_p,
                    nutrient_k: row.nutrient_k,
                    ph_min: row.ph_min,
                    ph_max: row.ph_max,
                    temp_min_c: row.temp_min_c,
                    temp_max_c: row.temp_max_c,
                    humidity_min: row.humidity_min,
                    humidity_max: row.humidity_max,
                    harvest_item: row.harvest_item,
                    needs_light: parse_needs_light(&row.needs_light),
                    n_fixed_share: parse_fixed_share(&row.n_fixed_pct),
                    removal_n: parse_removal(&row.removal_n_g_per_kg),
                    removal_p2o5: parse_removal(&row.removal_p2o5_g_per_kg),
                    removal_k2o: parse_removal(&row.removal_k2o_g_per_kg),
                },
            );
        }
        Ok(Self { plants })
    }
}

/// One row of `plants.csv`. The columns `PlantRegistry` consumes; extra CSV
/// columns (value/skill/companions/adverse) are still ignored. Nutrient demand
/// (N/P/K), the pH window, the temperature window, and the humidity window are
/// now parsed so the Garden table can show per-crop needs and a future tower
/// compatibility check can compute the shared-reservoir window. Every numeric
/// field is `#[serde(default)]`, so older/leaner CSVs (and the test fixture)
/// that omit these columns simply default them to 0.
#[derive(Debug, Deserialize)]
struct PlantRow {
    id: String,
    name: String,
    #[serde(default)]
    growth_days: f32,
    #[serde(default)]
    water_liters_per_day: f32,
    #[serde(default)]
    nutrient_n: f32,
    #[serde(default)]
    nutrient_p: f32,
    #[serde(default)]
    nutrient_k: f32,
    #[serde(default)]
    ph_min: f32,
    #[serde(default)]
    ph_max: f32,
    #[serde(default)]
    temp_min_c: f32,
    #[serde(default)]
    temp_max_c: f32,
    #[serde(default)]
    humidity_min: f32,
    #[serde(default)]
    humidity_max: f32,
    #[serde(default)]
    growth_stages: String,
    #[serde(default)]
    seasons: String,
    #[serde(default)]
    yield_min: f32,
    #[serde(default)]
    yield_max: f32,
    #[serde(default)]
    harvest_item: String,
    /// Text, not bool: see `parse_needs_light`.
    #[serde(default)]
    needs_light: String,
    /// Text, not a number: see `parse_fixed_share`.
    #[serde(default)]
    n_fixed_pct: String,
    /// Text, not numbers: see `parse_removal`.
    #[serde(default)]
    removal_n_g_per_kg: String,
    #[serde(default)]
    removal_p2o5_g_per_kg: String,
    #[serde(default)]
    removal_k2o_g_per_kg: String,
}

/// Split a colon-separated list field into trimmed, non-empty entries.
fn split_colon_list(s: &str) -> Vec<String> {
    s.split(':')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

#[cfg(test)]
mod plant_registry_csv_tests {
    use super::*;

    #[test]
    fn from_csv_parses_plants_and_colon_lists() {
        let csv = b"id,name,type,growth_days,water_liters_per_day,growth_stages,seasons\n\
                    tomato,Tomato,fruit,80,0.5,seed:sprout:vegetative:mature,spring:summer\n";
        let reg = PlantRegistry::from_csv(csv).expect("parse");
        assert_eq!(reg.plants.len(), 1);
        let t = reg.get("tomato").expect("tomato present");
        assert!((t.growth_days - 80.0).abs() < 1e-6);
        assert!((t.water_per_day - 0.5).abs() < 1e-6);
        assert_eq!(t.growth_stages, vec!["seed", "sprout", "vegetative", "mature"]);
        assert_eq!(t.seasons, vec!["spring", "summer"]);
        assert_eq!(t.first_stage(), "seed");
        assert_eq!(t.last_stage(), "mature");
    }

    #[test]
    fn from_csv_parses_nutrient_temp_and_humidity_columns() {
        // A full-width row (mirroring data/plants.csv) must populate the N/P/K,
        // pH, temperature, and humidity fields the Garden table shows and the
        // future tower compatibility check will read. Locks the column names.
        let csv = b"id,name,description,type,growth_days,water_liters_per_day,nutrient_n,nutrient_p,nutrient_k,ph_min,ph_max,temp_min_c,temp_max_c,humidity_min,humidity_max,yield_min,yield_max,growth_stages,seasons\n\
                    tomato,Tomato,desc,fruit,70,1.5,0.15,0.05,0.20,6.0,6.8,18,30,0.50,0.80,2,8,seed:sprout:mature,spring:summer\n";
        let reg = PlantRegistry::from_csv(csv).expect("parse");
        let t = reg.get("tomato").expect("tomato present");
        assert!((t.nutrient_n - 0.15).abs() < 1e-6, "N");
        assert!((t.nutrient_p - 0.05).abs() < 1e-6, "P");
        assert!((t.nutrient_k - 0.20).abs() < 1e-6, "K");
        assert!((t.water_per_day - 1.5).abs() < 1e-6, "water/day");
        assert!((t.temp_min_c - 18.0).abs() < 1e-6, "temp_min");
        assert!((t.temp_max_c - 30.0).abs() < 1e-6, "temp_max");
        assert!((t.ph_min - 6.0).abs() < 1e-6, "ph_min");
        assert!((t.ph_max - 6.8).abs() < 1e-6, "ph_max");
        assert!((t.humidity_min - 0.50).abs() < 1e-6, "humidity_min");
        assert!((t.humidity_max - 0.80).abs() < 1e-6, "humidity_max");
        assert!((t.yield_max - 8.0).abs() < 1e-6, "yield still parses past the new columns");
    }

    /// ZERO-DROP GUARD: every data row in the shipped `data/plants.csv` must survive
    /// `PlantRegistry::from_csv` -- registry size == raw data-row count. The shared CSV
    /// loader is row-resilient (a row that fails serde is skipped with only a log::warn),
    /// which silently ate `saffron` for months: its fractional yield_min (0.3) failed the
    /// old `u32` yield field, so the registry never contained it and it was un-plantable
    /// in-game. Any future schema drift that starts eating rows fails HERE, loudly.
    #[test]
    fn shipped_plants_csv_parses_with_zero_dropped_rows() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("plants.csv");
        let text = std::fs::read_to_string(&path).expect("data/plants.csv reads");
        // Raw data-row count: non-comment, non-blank lines, minus the header line.
        let data_rows = text
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#')
            })
            .count()
            - 1;
        let reg = PlantRegistry::from_csv(text.as_bytes()).expect("data/plants.csv parses");
        assert_eq!(
            reg.plants.len(),
            data_rows,
            "PlantRegistry dropped {} of {} plants.csv rows (row-resilient parsing silently \
             ate them -- check the loader warn log for which rows failed serde)",
            data_rows - reg.plants.len().min(data_rows),
            data_rows
        );
        // The row that exposed the bug: saffron's fractional yield survives as-is.
        let saffron = reg.get("saffron").expect("saffron present (fractional-yield row)");
        assert!(
            (saffron.yield_min - 0.3).abs() < 1e-6,
            "saffron yield_min survives as 0.3, got {}",
            saffron.yield_min
        );
        assert!(
            (saffron.yield_max - 1.0).abs() < 1e-6,
            "saffron yield_max survives as 1.0, got {}",
            saffron.yield_max
        );
    }

    /// The shipped `data/plants.csv` carries real edible mushroom crops (added 2026-07-01 to
    /// back the `mushroom_rack` machine's "+50 kcal/d" claim -- see
    /// docs/design/homestead-solo-design.md gap #1). Guards the mushroom_rack's food-loop
    /// story against a silent regression to the old alien-only fungi.
    #[test]
    fn shipped_plants_csv_has_real_edible_mushrooms() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("plants.csv");
        let bytes = std::fs::read(&path).expect("data/plants.csv reads");
        let reg = PlantRegistry::from_csv(&bytes).expect("data/plants.csv parses");
        for id in ["oyster_mushroom", "shiitake", "button_mushroom"] {
            let def = reg.get(id).unwrap_or_else(|| panic!("{id} present in plants.csv"));
            assert!(def.growth_days > 0.0, "{id} has a real growth cycle");
            assert!(def.humidity_min > 0.5, "{id} is a high-humidity crop, not a desert plant");
        }
    }

    /// `needs_light` (2026-09-26): only an explicit no turns the light gate
    /// off, a blank cell or a CSV without the column still needs light (and
    /// never drops the row), and the shipped file marks the three real fungi
    /// as growing without light while every green crop needs it.
    #[test]
    fn needs_light_column_parses_and_the_fungi_grow_without_light() {
        let csv = b"id,name,growth_days,needs_light\n\
                    a,A,10,false\n\
                    b,B,10,true\n\
                    c,C,10,\n\
                    d,D,10,no\n";
        let reg = PlantRegistry::from_csv(csv).expect("parse");
        assert_eq!(reg.plants.len(), 4, "no row dropped over the light cell");
        assert!(!reg.get("a").unwrap().needs_light, "false turns the gate off");
        assert!(reg.get("b").unwrap().needs_light, "true keeps it");
        assert!(reg.get("c").unwrap().needs_light, "blank means it needs light");
        assert!(!reg.get("d").unwrap().needs_light, "no turns the gate off");
        let old = PlantRegistry::from_csv(b"id,name,growth_days\nx,X,10\n").expect("parse");
        assert!(old.get("x").unwrap().needs_light, "a CSV without the column needs light");

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("plants.csv");
        let shipped = PlantRegistry::from_csv(&std::fs::read(&path).expect("plants.csv reads"))
            .expect("plants.csv parses");
        for id in ["oyster_mushroom", "shiitake", "button_mushroom"] {
            assert!(!shipped.get(id).unwrap().needs_light, "{id} is a fungus and grows without light");
        }
        for id in ["tomato", "lettuce", "wheat", "potato", "strawberry"] {
            assert!(shipped.get(id).unwrap().needs_light, "{id} is a green plant and needs light");
        }
    }
}

/// Rate at which water_level decreases per second (base dehydration).
const DEHYDRATION_RATE: f32 = 0.002;

/// Water level below which crop health starts dropping.
const WATER_STRESS_THRESHOLD: f32 = 0.2;

/// Health recovery rate per second when well-watered.
const HEALTH_RECOVERY_RATE: f32 = 0.5;

/// Health decay rate per second when water-stressed.
const HEALTH_DECAY_RATE: f32 = 1.0;

/// Litres one hand watering uses (2026-09-25): a watering can, drawn from the
/// home tanks.
pub const HAND_WATER_L: f32 = 2.0;

/// Water level a steady full-intensity rain adds per second to an outdoor
/// field crop (2026-09-25). About twice the base dehydration, so rain keeps a
/// field watered and a drizzle slows the drying.
pub const RAIN_WATER_PER_S: f32 = 0.004;

/// One line for the player (the "player_notices" channel the main loop shows).
fn push_notice(data: &DataStore, msg: String) {
    if let Some(slot) = data.get::<std::sync::Mutex<Vec<String>>>("player_notices") {
        if let Ok(mut n) = slot.lock() {
            n.push(msg);
        }
    }
}

/// Is this grow-area tag an outdoor FIELD? True for the field machine type
/// ("grain_field") and for one physical field ("grain_field_1", how the
/// showcase garden tags its crops). Until 2026-09-25 only the first form
/// matched, so showcase field crops skipped season and weather entirely.
pub fn is_field_area(tid: &str) -> bool {
    let base = match tid.rsplit_once('_') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => head,
        _ => tid,
    };
    base.ends_with("_field")
}

/// Water level the home's automated irrigation holds a grow area at when
/// the player has not set that area's slider (2026-09-25). Well above the
/// 0.2 stress line, below saturation, the way a timed drip or aeroponic
/// pump keeps a root zone.
pub const DEFAULT_AUTO_IRRIGATION: f32 = 0.8;

/// Share of every game day the sun is up (2026-09-26, gardening depth rung
/// 2: light). The game's sun, the same one the solar panels run on
/// (`solar::sun_factor`), rises at 6:00 and sets at 18:00 every day, so half
/// of each day is lit. A test pins this to that curve, so a change to the
/// sun cannot quietly make every crop grow faster or slower than its data.
pub const DAYLIGHT_FRACTION: f64 = 0.5;

/// How fast a crop's growth clock runs this tick, as a multiple of the world
/// clock (2026-09-26, gardening depth rung 2: light). Before this, light did
/// nothing: crops grew at the same pace at midnight as at noon, and a grow
/// light was only a power draw.
///
/// A green plant grows on the light it catches, so it grows only while its
/// grow area is lit and not at all in the dark (`needs_light`). Darkness only
/// PAUSES it: nothing here touches health, water or yield.
///
/// Why lit time runs at 1 / DAYLIGHT_FRACTION (2x) and not 1x: plants.csv
/// `growth_days` are calendar days under a natural sky, nights included. A
/// day's growth follows the light that day delivered (the daily light
/// integral), so this puts the whole day's growth into its lit hours and
/// none into the dark ones, and a crop under the sun alone still matures in
/// exactly its `growth_days`. Real plants keep expanding at night on sugars
/// made by day; placing all of a day's growth in its lit hours changes WHEN
/// in the day a stage turns, not how many days the crop takes. It also keeps
/// the offline catch-up in save_load.rs right on average: that ages a crop
/// one day per day away, which is what a natural day gives.
///
/// Where the light comes from:
/// - An outdoor FIELD (`is_field_area`, the same test rain uses) has only
///   the sun.
/// - Every other grow area has the sun too, through the glass: both shipped
///   homes grow their beds and towers under a skylight (the home.ron design
///   note "Sun-lit (skylight) single canopy", every tower and bed card says
///   "sun-lit", and a machines.rs test pins the seed homes to no grow lights,
///   "sun-lit by design"). After dark, the powered grow lights near it light
///   it: `lamp_cover` is the share of the area they cover (0 to 1,
///   `lighting::light_cover`, 2026-09-26: a 100 W light covers about 0.58 m2,
///   nearest plots first), and it grows that share of a lit night's growth.
///   (The first light rung let one light light every indoor area.)
/// - A species that does not need light (the fungi) grows at the calendar
///   rate, lit or not.
///
/// A grow light left on through the night therefore gives up to twice the
/// growth of the sun alone. That is the first-order effect of doubling the
/// day's light, and an upper bound: real crops saturate at a species-specific
/// daily light (a lettuce in full sun gains little from more), and some are
/// harmed by light around the clock (tomato). plants.csv has no per-species
/// light amount yet, so that is the next rung, not a number invented here.
pub fn light_growth_rate(needs_light: bool, outdoors: bool, sun_up: bool, lamp_cover: f64) -> f64 {
    if !needs_light {
        return 1.0;
    }
    if sun_up {
        1.0 / DAYLIGHT_FRACTION
    } else if outdoors {
        0.0
    } else {
        lamp_cover.clamp(0.0, 1.0) / DAYLIGHT_FRACTION
    }
}

/// Home RF level above which crops start taking RF stress (v0.620). Any notable wireless emission.
const RF_HARM_THRESHOLD: f32 = 0.1;
/// Crop health lost per second per unit of home RF level. Scaled so one WiFi router (~0.6) outpaces the
/// well-watered recovery rate, so the grow visibly declines while RF is present + recovers once it stops.
const RF_HEALTH_PENALTY: f32 = 1.5;

/// Seconds per in-game day (must match time system).
const SECONDS_PER_DAY: f64 = 1200.0;

/// Determine growth stage from progress fraction (0.0 to 1.0+) using
/// a data-driven stage list. Stages are evenly distributed across the
/// 0.0-1.0 range unless custom thresholds are added later.
fn stage_from_progress<'a>(progress: f32, stages: &'a [&'a str]) -> &'a str {
    if stages.is_empty() {
        return DEFAULT_GROWTH_STAGES[0];
    }
    let n = stages.len();
    // Each stage occupies an equal fraction of the 0.0-1.0 range.
    // stage[i] starts at i/n and runs until (i+1)/n.
    let idx = ((progress * n as f32).floor() as usize).min(n - 1);
    stages[idx]
}

/// Returns the index of a stage name in the stage list, or None if not found.
fn stage_index(stage: &str, stages: &[&str]) -> Option<usize> {
    stages.iter().position(|s| *s == stage)
}

/// Map a seed item id (`seed_<plant>_0`) to its plant-definition id (`<plant>`).
/// Strips the `seed_` prefix and a trailing `_<n>` item-instance suffix.
fn plant_id_from_seed(seed_id: &str) -> Option<String> {
    let body = seed_id.strip_prefix("seed_")?;
    if let Some((base, suffix)) = body.rsplit_once('_') {
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return Some(base.to_string());
        }
    }
    Some(body.to_string())
}

/// Resolve a plant id to the produce item it yields, VALIDATED against the item
/// registry so a harvest only ever produces an item that actually exists. Tries
/// the `vegetable_/fruit_/grain_` naming convention. (A `harvest_item` column on
/// plants.csv would make this fully data-driven — tracked in gameplay-loops.md.)
fn harvest_item_for(
    plant_id: &str,
    plants: Option<&PlantRegistry>,
    items: Option<&crate::systems::inventory::ItemRegistry>,
) -> Option<String> {
    // The explicit plants.csv harvest_item column wins (v0.749) — validated
    // against items.csv so a typo degrades to the prefix search, not a
    // phantom item.
    if let Some(explicit) = plants
        .and_then(|p| p.get(plant_id))
        .map(|d| d.harvest_item.clone())
        .filter(|h| !h.is_empty())
    {
        if items.map(|r| r.items.contains_key(&explicit)).unwrap_or(false) {
            return Some(explicit);
        }
        log::warn!("plants.csv harvest_item '{explicit}' for {plant_id} is not in items.csv");
    }
    for prefix in ["vegetable", "fruit", "grain"] {
        let candidate = format!("{prefix}_{plant_id}_0");
        if items.map(|r| r.items.contains_key(&candidate)).unwrap_or(false) {
            return Some(candidate);
        }
    }
    None
}

/// A crop's SEASON health, 0..1 (2026-09-26): the time-average of its health
/// over every tick it spent growing, from `CropInstance::health_seconds` /
/// `growing_seconds`. This, not the health on harvest day, is what sets the
/// yield: current health recovers in a few minutes once water comes back and
/// is frozen once the crop matures (mature crops are skipped by the growth
/// loop), so reading it at harvest would forget a drought the plant came
/// through. A crop with no record (never ticked: a showcase crop spawned
/// mature, or one from an older save) falls back to its current health.
pub fn season_health(crop: &CropInstance) -> f32 {
    if crop.growing_seconds > 0.0 {
        (crop.health_seconds / crop.growing_seconds).clamp(0.0, 1.0) as f32
    } else {
        (crop.health / 100.0).clamp(0.0, 1.0)
    }
}

/// Season health below which a harvest tells the player the crop was
/// stressed (2026-09-26). A presentation threshold, not agronomy: small dips
/// stay quiet so a well-kept garden does not nag.
const STRESSED_HARVEST_NOTICE_BELOW: f32 = 0.9;

/// Whole items one harvest yields (2026-09-26, extracted so a test can drive
/// it with a seeded generator).
///
/// `roll` and `round` are two uniform draws in [0, 1). The continuous yield
/// is rolled in [ymin, ymax] and then SCALED BY SEASON HEALTH: a crop kept
/// well all season gets the full range, one that spent the season at half
/// health gets half, one that barely survived gets next to nothing. The
/// linear shape is the standard crop-water production function (FAO
/// Irrigation and Drainage Paper 33, Doorenbos and Kassam 1979: relative
/// yield loss is proportional to the relative water deficit over the
/// season, 1 - Ya/Ym = Ky (1 - ETa/ETm)). Season health stands in for the
/// deficit, since stress (thirst above all, and RF and hard acceleration)
/// is what drives crop health down here. Ky is taken as 1 because
/// plants.csv carries no per-crop response factor yet; FAO's Ky runs above
/// and below 1 by crop and by growth stage, so a per-crop column is the
/// natural next rung, not a number invented here.
///
/// The scaled yield becomes a whole count by PROBABILISTIC ROUNDING (floor +
/// Bernoulli on the fraction): a 0.3 yields 1 item 30% of the time and 0
/// items 70%, so the expected value equals the scaled roll. Fractional crops
/// average their real output over repeated harvests instead of being
/// silently rounded up to a full unit (3x inflation for saffron) or floored
/// to permanent zero, and a stressed crop's loss shows up the same way.
fn harvest_quantity(ymin: f32, ymax: f32, season_health: f32, roll: f32, round: f32) -> u32 {
    let rolled = (ymin + roll * (ymax - ymin)) * season_health.clamp(0.0, 1.0);
    rolled.floor() as u32 + u32::from(round < rolled.fract())
}

/// What a crop draws from its UNIT over one growing season, grams of N, P2O5
/// and K2O (2026-09-26, soil.rs): its harvest's removal (`crop_removal`),
/// less the nitrogen a legume fixes from the air (`soil::soil_draw`), so a
/// soybean asks its unit for 45% of its N and a tomato for all of it. This
/// is the need the tick draws, the feeder feeds and the shortage test reads.
/// Public so the Garden panel shows a unit's store against the same need the
/// tick uses.
pub fn crop_season_need(
    plant_id: &str,
    plants: Option<&PlantRegistry>,
    items: Option<&crate::systems::inventory::ItemRegistry>,
    scale: Option<Npk>,
) -> Npk {
    let fixed = plants.and_then(|r| r.get(plant_id)).map_or(0.0, |d| f64::from(d.n_fixed_share));
    soil::soil_draw(crop_removal(plant_id, plants, items, scale), fixed)
}

/// What a crop's harvest carries out of its unit in one season, grams: its
/// expected harvest (mid yield x the items.csv mass of what it harvests into)
/// times what each kg of it removes: the crop's own plants.csv removal
/// columns where filled, else its index read against the anchor's published
/// removal (`soil::removal_per_kg`). `scale` is `NutrientData::scale_for`;
/// None, or an unknown plant, removes nothing.
/// For a legume this is more than it draws (`crop_season_need`): the rest
/// of the N came from the air.
pub fn crop_removal(
    plant_id: &str,
    plants: Option<&PlantRegistry>,
    items: Option<&crate::systems::inventory::ItemRegistry>,
    scale: Option<Npk>,
) -> Npk {
    match (scale, plants.and_then(|r| r.get(plant_id))) {
        (Some(scale), Some(def)) => {
            let item_kg = harvest_item_for(plant_id, plants, items)
                .map_or(0.0, |i| items.map_or(0.0, |r| f64::from(r.mass_for(&i))));
            soil::season_need(def, soil::expected_harvest_kg(def, item_kg), scale)
        }
        _ => Npk::ZERO,
    }
}

/// Clear the dead crops found in a unit before it is replanted, keeping
/// what their soil still holds for the crop about to go in (2026-09-26).
fn clear_dead_keeping_soil(world: &mut hecs::World, dead: Vec<hecs::Entity>, area: &str, slot: u32) {
    for e in dead {
        let left = world.get::<&CropSoil>(e).ok().map(|s| s.store);
        if let Some(store) = left {
            soil::remember(world, area, slot, store);
        }
        let _ = world.despawn(e);
    }
}

/// Spawn a newly sown crop into its unit, on whatever soil that unit was
/// left with (soil::recall). A unit with nothing remembered, and a
/// hand-planted crop with no unit, get fresh soil on the crop's first tick.
fn spawn_in_remembered_soil(world: &mut hecs::World, crop: CropInstance) -> hecs::Entity {
    let remembered = match (&crop.tower_id, crop.tower_slot) {
        (Some(area), Some(slot)) => soil::recall(world, area, slot),
        _ => None,
    };
    match remembered {
        Some(store) => world.spawn((crop, CropSoil { store, uptake: 0.0 })),
        None => world.spawn((crop,)),
    }
}

/// The one-line notice for grow areas whose crops have just run short. An
/// area tag is a machine or tower id ("potato_grow_bed", "ntower_3"); the
/// empty tag is the hand-planted crops outside any grow area.
fn short_notice(newly_short: &[(String, soil::Nutrient)]) -> String {
    let place = |area: &str| {
        if area.is_empty() {
            "your hand-planted crops".to_string()
        } else {
            format!("the crops in {}", area.replace('_', " "))
        }
    };
    let fix = "Fertilize them: compost adds nitrogen, phosphorus and potassium, and stored urine adds nitrogen fast.";
    match newly_short {
        [(area, nutrient)] => format!("{} are short of {}. {fix}", capitalize(&place(area)), nutrient.word()),
        many => {
            let mut counts: HashMap<soil::Nutrient, usize> = HashMap::new();
            for (_, n) in many {
                *counts.entry(*n).or_default() += 1;
            }
            let most = counts
                .into_iter()
                .max_by_key(|(n, c)| (*c, matches!(n, soil::Nutrient::Nitrogen)))
                .map_or(soil::Nutrient::Nitrogen, |(n, _)| n);
            format!(
                "Crops in {} grow areas are short of nutrients, most often {}. {fix}",
                many.len(),
                most.word()
            )
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Simulates crop growth based on elapsed time and environmental factors.
pub struct FarmingSystem {
    _initialized: bool,
    /// data/garden/nutrients.ron, read on the first tick (see soil.rs). A
    /// "garden_nutrients" entry in the DataStore, if one is ever registered,
    /// wins over it.
    nutrients: Option<soil::NutrientData>,
    /// The automatic feeder's opened fertilizer, per grow area and item: the
    /// share of one item not yet dosed out (2026-09-26). The feeder doses a
    /// unit a few milligrams at a time, so it opens a whole bag (or a
    /// person-day of stored urine) from home storage and works through it.
    /// Not saved: a reload loses at most one part-used item of each kind per
    /// fed area.
    feed_open: HashMap<(String, String), f64>,
    /// True once the player has been told the feeder has no fertilizer, so
    /// the notice is said once, not every tick; cleared when a bag is opened.
    feeder_dry_told: bool,
    /// Grow areas whose crops the player has been told are short of a
    /// nutrient, so each shortage is announced once; an area leaves the set
    /// when its crops are supplied again.
    short_told: std::collections::HashSet<String>,
    /// data/garden/pests.ron, read on the first tick (see pests.rs). A
    /// "garden_pests" entry in the DataStore, if one is ever registered, wins
    /// over it.
    pests: Option<pests::PestData>,
    /// data/garden/soil_ph.ron, read on the first tick (see soil_ph.rs); a
    /// "garden_soil_ph" DataStore entry wins over it. And the pH notices told
    /// and the amendment bags opened, which are not saved.
    ph_data: Option<soil_ph::SoilPhData>,
    ph_rt: soil_ph::PhRuntime,
}

impl FarmingSystem {
    pub fn new() -> Self {
        Self {
            _initialized: false,
            nutrients: None,
            feed_open: HashMap::new(),
            feeder_dry_told: false,
            short_told: std::collections::HashSet::new(),
            pests: None,
            ph_data: None,
            ph_rt: soil_ph::PhRuntime::default(),
        }
    }
}

impl System for FarmingSystem {
    fn name(&self) -> &str {
        "FarmingSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let plant_registry = data.get::<PlantRegistry>("plant_registry");
        // Sustained acceleration, if the flight model is loaded. Resolved once
        // per tick rather than per crop: it is a ship-wide scalar, exactly like
        // `home_rf` below, and a burn reaches every planter at once.
        let g_harm_per_sec = data
            .get::<crate::systems::flight::FlightData>("flight_data")
            .and_then(|f| f.tolerance("plant"))
            .zip(
                data.get::<crate::systems::flight::FeltGravity>("felt_gravity")
                    .map(|f| f.felt_g),
            )
            .map(|(tol, g)| crate::systems::flight::harm_per_sec(tol, g))
            .unwrap_or(0.0);

        // Get current elapsed time from TimeSystem's GameTime if available
        let elapsed_seconds = data
            .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
            .and_then(|m| m.lock().ok())
            .map(|gt| gt.elapsed_seconds)
            .unwrap_or(0.0);
        // Light inputs (2026-09-26): the hour, for whether the sun is up, and
        // the clock's rate, so a crop's growth clock can be held in the dark
        // by exactly the game time that passed (see light_growth_rate).
        // Absent clock (headless tests, early boot) = noon at real time, the
        // same default SolarSystem uses, so nothing is dark by accident.
        let (hour, time_scale) = data
            .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
            .and_then(|m| m.lock().ok())
            .map(|gt| (gt.hour, gt.time_scale))
            .unwrap_or((12.0, 1.0));
        // Outdoor climate inputs (v0.749, ladder rung 6): the current season +
        // live weather temperature, applied to FIELD crops only below. Empty
        // season / absent weather = no penalty (headless tests, early boot).
        let season = data
            .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
            .and_then(|m| m.lock().ok())
            .map(|gt| format!("{:?}", gt.season).to_lowercase())
            .unwrap_or_default();
        // Deliberately the body-surface GLOBAL reference (w.temperature),
        // NOT w.temperature_at_player: field crops share the body's
        // climate and must not chill because the PLAYER climbed a
        // mountain or flew to altitude (review split; the home station
        // farms inside Earth's frame-lock envelope at ~400 km, where the
        // positional altitude term would read arctic).
        let weather_temp = data
            .get::<std::sync::Mutex<crate::systems::weather::Weather>>("weather")
            .and_then(|m| m.lock().ok().map(|w| w.temperature))
            .unwrap_or(20.0);

        // Global crop growth multiplier (operator, 2026-09-20). Published by
        // lib.rs from Settings as a plain f32 so the sim never imports a GUI
        // type, the same neutral-handle pattern as irrigation and nutrient.
        // Absent (headless tests, early boot) = the shipped default, so a test
        // that never publishes it still sees the behaviour players get.
        let growth_speed = clamp_growth_speed(
            data.get::<std::sync::Mutex<f32>>("crop_growth_speed")
                .and_then(|m| m.lock().ok().map(|v| *v))
                .unwrap_or(DEFAULT_CROP_GROWTH_SPEED),
        );

        // Build default stages vec once for plants without custom stages
        let default_stages: Vec<&str> = DEFAULT_GROWTH_STAGES.iter().copied().collect();

        let item_registry = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");

        // Per-area irrigation: a grow area the player has configured (in the garden
        // edit modal) tops its crops up to a target water level each tick. Keyed by
        // tower_id (e.g. "nutrition"). A neutral HashMap<String, f32> so the sim never
        // imports a GUI type; the GUI publishes it via lib.rs -> "garden_irrigation".
        // Empty/missing = no automated irrigation (crops dehydrate normally).
        let irrigation: std::collections::HashMap<String, f32> = data
            .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>("garden_irrigation")
            .and_then(|m| m.lock().ok())
            .map(|g| g.clone())
            .unwrap_or_default();
        // Per-area nutrient slider (garden edit modal), keyed by tower_id. Until
        // 2026-09-26 it multiplied growth speed (0.5x..1.5x); it now sets the
        // automatic FEEDER that tops each unit's N-P-K store up from home
        // storage (soil::feed_target). Same neutral-map pattern as irrigation.
        let nutrient: std::collections::HashMap<String, f32> = data
            .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>("garden_nutrient")
            .and_then(|m| m.lock().ok())
            .map(|g| g.clone())
            .unwrap_or_default();

        // Water -> FOOD coupling (v0.611): the downstream end of the power -> water -> food chain. If the
        // home has a real water system (a cistern) and it has run DRY, automated irrigation can no longer
        // top crops up -- so they dehydrate + wilt. (Cut the power -> the well pump sheds -> the cistern
        // drains -> days later the garden starts to die.) Read from PlumbingSystem's live WaterStatus.
        // Absent water_status (tests / a home with no plumbing) OR no cistern (capacity 0) = water
        // available, so existing gardening behaviour + un-plumbed homes are unchanged.
        // Is the home's irrigation running? (2026-09-25) A powered machine
        // with the Irrigator marker; with none (or none powered) crops get no
        // automatic water and must be watered by hand.
        let irrigation_on = world
            .query::<(
                &crate::ecs::components::Irrigator,
                Option<&crate::ecs::components::WaterConsumer>,
                Option<&crate::ecs::components::PowerConsumer>,
            )>()
            .iter()
            .any(|(_, (_, wc, pc))| {
                let needs_power = wc.map_or(false, |w| w.needs_power);
                !needs_power || pc.map_or(false, |p| p.enabled)
            });
        // Light (2026-09-26, see light_growth_rate). The sun is up when it
        // makes solar power: the same curve, so the fields and the panels
        // agree on when it is day. A grow light lights the plots near it
        // (lighting.rs), and only while its PowerConsumer is enabled, so a
        // light the electrical sim sheds (priority 5 goes first) or the player
        // switches off gives no light, and one with no power role at all is
        // never lit. Cover by grow-area id (instance or tower design id).
        let sun_up = crate::systems::solar::sun_factor(hour) > 0.0;
        let lamp_cover: HashMap<String, f64> = match (
            data.get::<Vec<lighting::GrowPlot>>("grow_plots"),
            data.get::<lighting::LightingData>("garden_lighting"),
        ) {
            (Some(plots), Some(ld)) if !sun_up => lighting::light_cover(&lighting::powered_lights(world), plots, ld),
            _ => HashMap::new(),
        };
        // Game seconds this tick, computed the way TimeSystem advances the
        // clock, so holding a crop in the dark holds it by exactly what
        // passed. A clock jump (the dev hour set, a save restore) is not a
        // tick and is not light-counted, as it was not before.
        let game_dt = f64::from(dt) * f64::from(time_scale);
        // Rain waters outdoor fields (2026-09-25): litres of water level per
        // second at full intensity; 0 when it is not raining.
        let rain = data
            .get::<std::sync::Mutex<crate::systems::weather::Weather>>("weather")
            .and_then(|m| m.lock().ok().map(|w| {
                use crate::systems::weather::WeatherCondition::*;
                match w.condition {
                    Rain | Storm => w.intensity.clamp(0.0, 1.0),
                    _ => 0.0,
                }
            }))
            .unwrap_or(0.0);
        // Damp weather for the pests that like it (2026-09-26, pests.rs):
        // UC IPM, "Snails and slugs are most active at night and on cloudy or
        // foggy days". Absent weather reads as dry; night counts as damp
        // below, where the pests are stepped.
        let damp_weather = data
            .get::<std::sync::Mutex<crate::systems::weather::Weather>>("weather")
            .and_then(|m| m.lock().ok().map(|w| {
                use crate::systems::weather::WeatherCondition::*;
                matches!(w.condition, Cloudy | Rain | Storm | Fog)
            }))
            .unwrap_or(false);
        let mut irrigation_l_per_day = 0.0_f32;
        let water_available = data
            .get::<std::sync::Mutex<crate::systems::plumbing::WaterStatus>>("water_status")
            .and_then(|m| m.lock().ok())
            .map(|ws| ws.capacity_l <= 0.0 || ws.stored_l > ws.capacity_l * 0.02)
            .unwrap_or(true);

        // RF -> FOOD coupling (v0.620): sum every POWERED RF emitter (a WiFi router) into a home RF
        // level. Sensitive crops lose health under RF -- the operator's "the user doesn't want a WiFi
        // router because it harms a plant they're growing." Run wired (Cat6/fibre, zero RF) to stay clean.
        let home_rf: f32 = {
            use crate::ecs::components::{PowerConsumer, RfEmitter};
            let mut rf = 0.0f32;
            for (_, (em, power)) in world.query::<(&RfEmitter, Option<&PowerConsumer>)>().iter() {
                let powered = !em.needs_power || power.map(|c| c.enabled).unwrap_or(false);
                if powered {
                    rf += em.strength;
                }
            }
            rf
        };

        // Creative mode (default ON in early dev): planting + fertilizing skip the
        // inventory requirement + consumption. Absent flag (tests) = survival =
        // consume, so the existing gardening tests still hold.
        let creative = data
            .get::<std::sync::Mutex<bool>>("creative_mode")
            .and_then(|m| m.lock().ok().map(|g| *g))
            .unwrap_or(false);

        // NUTRIENTS (2026-09-26, gardening depth rung 3; the model is in
        // soil.rs, the numbers and their sources in data/garden/nutrients.ron).
        // Worked out once per tick: the per-index scale from the anchor crop's
        // published removal, and what one item of each fertilizer puts into a
        // unit. No scale (the anchor is missing) means no nutrient model at
        // all: every crop's need reads zero and nothing is ever short.
        if self.nutrients.is_none() {
            self.nutrients = Some(soil::NutrientData::load());
        }
        // Also from the data (2026-09-26, closing the nitrogen loop): what each
        // fertilizer banks as slow organic N and the feeder's rules for it,
        // compost's year-by-year release schedule, and the share of a
        // legume's N its harvest carries away (for the credit it leaves).
        let (demand_scale, fertilizer_doses, organic_yearly, legume_harvest_share): (
            Option<Npk>,
            Vec<soil::FertilizerDose>,
            Vec<f64>,
            f64,
        ) = {
            let nd = data
                .get::<soil::NutrientData>("garden_nutrients")
                .or(self.nutrients.as_ref())
                .expect("nutrient data loaded above");
            let scale = plant_registry.and_then(|r| nd.scale_for(r));
            let doses = nd
                .fertilizers
                .iter()
                .map(|f| f.dose(item_registry.map_or(0.0, |r| f64::from(r.mass_for(&f.item)))))
                .collect();
            (scale, doses, nd.organic_n_release.yearly_pct.clone(), nd.legume_harvest_n_share)
        };
        // PESTS (2026-09-26, pests.rs; the pests, the controls and every
        // source in data/garden/pests.ron). Loaded once, like the nutrients.
        // The severity is the player's mode, published as a plain f32 the way
        // the growth speed is: 0 turns pests off, 0.5 (the default) halves
        // their damage, 1 is the cited damage. Absent = the gentle default.
        if self.pests.is_none() {
            self.pests = Some(pests::PestData::load());
        }
        let pest_data: &pests::PestData = data
            .get::<pests::PestData>("garden_pests")
            .or(self.pests.as_ref())
            .expect("pest data loaded above");
        let pest_severity = data
            .get::<std::sync::Mutex<f32>>("garden_pest_severity")
            .and_then(|m| m.lock().ok().map(|v| *v))
            .filter(|v| v.is_finite())
            .unwrap_or(pests::DEFAULT_PEST_SEVERITY)
            .clamp(0.0, 1.0);
        // SOIL pH (2026-09-26, soil_ph.rs; every number in soil_ph.ron).
        // Loaded once, like the pests; Off (Settings) freezes all of it.
        if self.ph_data.is_none() {
            self.ph_data = Some(soil_ph::SoilPhData::load());
        }
        let ph_data: &soil_ph::SoilPhData =
            data.get::<soil_ph::SoilPhData>("garden_soil_ph").or(self.ph_data.as_ref()).expect("loaded above");
        let ph_on = soil_ph::is_on(data);

        // A crop's season need, by plant id (soil::season_need). Cached for the
        // tick: a garden is a few dozen species and a few thousand crops.
        let mut need_cache: HashMap<String, Npk> = HashMap::new();
        let mut need_for = |plant_id: &str| -> Npk {
            if let Some(n) = need_cache.get(plant_id) {
                return *n;
            }
            let need = crop_season_need(plant_id, plant_registry, item_registry, demand_scale);
            need_cache.insert(plant_id.to_string(), need);
            need
        };

        // ── GUI / dev gardening commands (the inventory page writes these via the
        //    main-loop bridge): plant a seed, water a crop, dev-grow all, harvest. ──
        // PLANT: consume one matching seed from the player, spawn a CropInstance.
        let plant_seed = data
            .get::<std::sync::Mutex<Option<String>>>("plant_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(seed_id) = plant_seed {
            if let Some(plant_id) = plant_id_from_seed(&seed_id) {
                // Resolve the first growth stage now (immutable plant_registry borrow),
                // copied out before the &mut World pass below.
                let first_stage = plant_registry
                    .and_then(|reg| reg.get(&plant_id))
                    .map(|d| d.first_stage().to_string());
                if let Some(first_stage) = first_stage {
                    let mut planted = false;
                    for (_e, (inv, _ctrl)) in world.query_mut::<(
                        &mut crate::systems::inventory::Inventory,
                        &crate::ecs::components::Controllable,
                    )>() {
                        if creative || inv.has_item(&seed_id, 1) {
                            if !creative {
                                inv.remove_item(&seed_id, 1);
                            }
                            planted = true;
                            break;
                        }
                    }
                    if planted {
                        world.spawn((CropInstance {
                            crop_def_id: plant_id.clone(),
                            growth_stage: first_stage,
                            planted_at: elapsed_seconds,
                            water_level: 1.0,
                            health: 100.0,
                            tower_id: None,
                            tower_slot: None,
                            health_seconds: 0.0,
                            growing_seconds: 0.0,
                        },));
                        log::info!("[Farming] planted {plant_id} (from {seed_id})");
                    }
                } else {
                    log::debug!("[Farming] no plant def for seed {seed_id}; not planted");
                }
            }
        }

        // PLANT TOWER (v0.386): spawn a CropInstance for each plant id sent by the
        // GUI. A tower's curated varieties all become growing crops at once;
        // growth/water/harvest reuse the logic below. v0.398: in SURVIVAL mode each
        // variety consumes one seed_<plant>_0 from the player (a variety with no seed
        // is skipped); CREATIVE mode plants every variety free. The seed is
        // plot-agnostic — the same seed plants a crop in any plot type, so this
        // generalizes when non-aeroponic gardens (soil / sand / pots) arrive.
        let plant_tower = data
            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some((tower_id, plant_ids)) = plant_tower {
            let mut planted = 0u32;
            let mut skipped = 0u32;
            for (slot_idx, plant_id) in plant_ids.into_iter().enumerate() {
                let slot_idx = slot_idx as u32;
                let first_stage = plant_registry
                    .and_then(|reg| reg.get(&plant_id))
                    .map(|d| d.first_stage().to_string());
                let first_stage = match first_stage {
                    Some(s) => s,
                    None => continue,
                };
                // SLOT FILL (v0.410): a tower has fixed slots. Skip if a LIVE crop
                // already occupies this slot, so replanting is IDEMPOTENT (fills only
                // empty / harvested / dead slots) instead of stacking a new set every
                // time. Despawn any DEAD crop in the slot first so it gets refilled.
                let mut occupied = false;
                let mut dead_in_slot: Vec<hecs::Entity> = Vec::new();
                for (e, c) in world.query::<&CropInstance>().iter() {
                    if c.tower_id.as_deref() == Some(tower_id.as_str())
                        && c.tower_slot == Some(slot_idx)
                    {
                        if c.growth_stage.as_str() == crate::ecs::components::STAGE_DEAD {
                            dead_in_slot.push(e);
                        } else {
                            occupied = true;
                        }
                    }
                }
                if occupied {
                    continue;
                }
                clear_dead_keeping_soil(world, dead_in_slot, &tower_id, slot_idx);
                // Survival: consume one seed for this variety, skip if absent.
                if !creative {
                    let seed_id = format!("seed_{plant_id}_0");
                    let mut had = false;
                    for (_e, (inv, _ctrl)) in world.query_mut::<(
                        &mut crate::systems::inventory::Inventory,
                        &crate::ecs::components::Controllable,
                    )>() {
                        if inv.has_item(&seed_id, 1) {
                            inv.remove_item(&seed_id, 1);
                            had = true;
                        }
                        break;
                    }
                    if !had {
                        skipped += 1;
                        continue;
                    }
                }
                let crop = CropInstance {
                    crop_def_id: plant_id,
                    growth_stage: first_stage,
                    planted_at: elapsed_seconds,
                    water_level: 1.0,
                    health: 100.0,
                    tower_id: Some(tower_id.clone()),
                    tower_slot: Some(slot_idx),
                    health_seconds: 0.0,
                    growing_seconds: 0.0,
                };
                spawn_in_remembered_soil(world, crop);
                planted += 1;
            }
            if planted > 0 || skipped > 0 {
                log::info!("[Farming] planted tower: {planted} crops, {skipped} skipped (no seed)");
            }
        }

        // SHOWCASE TOWER (v0.862): fill a tower with ONE species at STAGGERED
        // ages so every growth stage is visible at once on the helix (operator:
        // "1 aeroponic tower full of strawberries at the various growth stages").
        // Replaces whatever was planted; demo/dev tool, creative-style (no seeds).
        let showcase = data
            .get::<std::sync::Mutex<Option<(String, String)>>>("showcase_tower_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some((tower_id, plant_id)) = showcase {
            if let Some(def) = plant_registry.and_then(|r| r.get(&plant_id)) {
                let doomed: Vec<hecs::Entity> = world
                    .query::<&CropInstance>()
                    .iter()
                    .filter(|(_, c)| c.tower_id.as_deref() == Some(tower_id.as_str()))
                    .map(|(e, _)| e)
                    .collect();
                for e in doomed {
                    let _ = world.despawn(e);
                }
                let slots = 50u32;
                let growth_seconds = def.growth_days as f64 * SECONDS_PER_DAY;
                let stages = def.stages();
                let n_stages = stages.len().max(1);
                for slot in 0..slots {
                    let frac = slot as f32 / (slots - 1) as f32;
                    let stage_i = ((frac * n_stages as f32).floor() as usize).min(n_stages - 1);
                    world.spawn((CropInstance {
                        crop_def_id: plant_id.clone(),
                        growth_stage: stages[stage_i].to_string(),
                        planted_at: elapsed_seconds - growth_seconds * frac as f64,
                        water_level: 1.0,
                        health: 100.0,
                        tower_id: Some(tower_id.clone()),
                        tower_slot: Some(slot),
                        health_seconds: 0.0,
                        growing_seconds: 0.0,
                    },));
                }
                log::info!("[Farming] showcase: {tower_id} filled with staggered {plant_id}");
            }
        }

        // PLANT BED (v0.738 grain loop): sow one crop per UNIT of a bed / tray /
        // field grow area. Same rules as tower planting — idempotent slot fill
        // (live crop keeps its unit, dead ones are replaced), one seed per unit
        // in survival, free in creative. The grow-area MACHINE id rides in
        // `tower_id` (it is the crop's grow-area tag; the Garden GUI groups by
        // it), with `tower_slot` as the unit index.
        let plant_bed = data
            .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some((area_id, plant_id, count)) = plant_bed {
            let first_stage = plant_registry
                .and_then(|reg| reg.get(&plant_id))
                .map(|d| d.first_stage().to_string());
            if let Some(first_stage) = first_stage {
                let mut planted = 0u32;
                // A machine TYPE id ("staple_grain_tray", what the Garden
                // panel's bed button sends) sows every plot of every machine
                // of that type, each crop tagged with the machine it stands
                // in (2026-09-26): the renderer, the grow lights and the pests
                // all go by the physical machine. The engine publishes the
                // machines ("grow_instances": type -> (instance id, plots)).
                // Anything else is one area of `count` units, as before.
                let targets: Vec<(String, u32)> = match data
                    .get::<HashMap<String, Vec<(String, u32)>>>("grow_instances")
                    .and_then(|m| m.get(&area_id))
                {
                    Some(insts) if !insts.is_empty() => insts
                        .iter()
                        .flat_map(|(id, plots)| (0..(*plots).max(1)).map(move |s| (id.clone(), s)))
                        .collect(),
                    _ => (0..count).map(|u| (area_id.clone(), u)).collect(),
                };
                for (area_id, unit) in targets {
                    let mut occupied = false;
                    let mut dead_in_slot: Vec<hecs::Entity> = Vec::new();
                    for (e, c) in world.query::<&CropInstance>().iter() {
                        if c.tower_id.as_deref() == Some(area_id.as_str())
                            && c.tower_slot == Some(unit)
                        {
                            if c.growth_stage.as_str() == crate::ecs::components::STAGE_DEAD {
                                dead_in_slot.push(e);
                            } else {
                                occupied = true;
                            }
                        }
                    }
                    if occupied {
                        continue;
                    }
                    clear_dead_keeping_soil(world, dead_in_slot, &area_id, unit);
                    if !creative {
                        let seed_id = format!("seed_{plant_id}_0");
                        let mut had = false;
                        for (_e, (inv, _ctrl)) in world.query_mut::<(
                            &mut crate::systems::inventory::Inventory,
                            &crate::ecs::components::Controllable,
                        )>() {
                            if inv.has_item(&seed_id, 1) {
                                inv.remove_item(&seed_id, 1);
                                had = true;
                            }
                            break;
                        }
                        if !had {
                            continue;
                        }
                    }
                    let crop = CropInstance {
                        crop_def_id: plant_id.clone(),
                        growth_stage: first_stage.clone(),
                        planted_at: elapsed_seconds,
                        water_level: 1.0,
                        health: 100.0,
                        tower_id: Some(area_id.clone()),
                        tower_slot: Some(unit),
                        health_seconds: 0.0,
                        growing_seconds: 0.0,
                    };
                    spawn_in_remembered_soil(world, crop);
                    planted += 1;
                }
                log::info!("[Farming] bed-planted {planted}x {plant_id} for {area_id}");
            } else {
                log::warn!("[Farming] no plant def '{plant_id}' for bed {area_id}; not planted");
            }
        }

        // DEV: stock one of each requested seed (the "one seed of each" starter set,
        // granted on demand so survival mode is testable now; the on-new-game grant
        // comes when the game is closer to ready). Grows the inventory to fit.
        let stock_seeds = data
            .get::<std::sync::Mutex<Option<Vec<String>>>>("stock_seeds_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(seed_ids) = stock_seeds {
            for (_e, (inv, _ctrl)) in world.query_mut::<(
                &mut crate::systems::inventory::Inventory,
                &crate::ecs::components::Controllable,
            )>() {
                let want = inv.max_slots + seed_ids.len();
                inv.ensure_slots(want);
                for seed_id in &seed_ids {
                    inv.add_item(seed_id, 1, 99);
                }
                break;
            }
            log::info!("[Farming] dev-stocked {} seed varieties", seed_ids.len());
        }

        // WATER: top up one crop's water + a little health.
        let water_bits = data
            .get::<std::sync::Mutex<Option<u64>>>("water_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(bits) = water_bits {
            if let Some(entity) = hecs::Entity::from_bits(bits) {
                // Hand watering is real water (2026-09-25): a can holds
                // HAND_WATER_L, drawn from the home tanks, and an empty
                // cistern means there is nothing to water with.
                if !water_available {
                    push_notice(data, "The water tanks are empty: nothing to water with.".to_string());
                } else if let Ok(mut crop) = world.get::<&mut CropInstance>(entity) {
                    crop.water_level = 1.0;
                    crop.health = (crop.health + 10.0).min(100.0);
                    if let Some(m) = data.get::<std::sync::Mutex<f32>>("hand_water_draw_l") {
                        if let Ok(mut v) = m.lock() {
                            *v += HAND_WATER_L;
                        }
                    }
                }
            }
        }

        // FERTILIZE: consume 1 fertilizer_0 from the player and put its
        // plant-available N, P2O5 and K2O into the crop's unit (2026-09-26,
        // soil.rs). Closes the food -> waste -> compost -> fertilizer -> crop
        // cycle (#7c sanitation). Until this rung it added 40 health and
        // watered the crop to half; now it feeds the soil, and the health
        // follows through the nutrient cap in the growth loop, so the old
        // boost is gone rather than counted twice.
        let fertilize_bits = data
            .get::<std::sync::Mutex<Option<u64>>>("fertilize_crop_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(bits) = fertilize_bits {
            // Only a crop that is there takes the fertilizer. Read what the
            // choice needs: its plant, its unit, and how far off ripe it is
            // (a fertilizer with a withholding period, urine, may not go on
            // within that many garden days of the harvest).
            let target = hecs::Entity::from_bits(bits).and_then(|e| {
                world.get::<&CropInstance>(e).ok().map(|c| {
                    let unit = c.tower_id.clone().zip(c.tower_slot);
                    (e, c.crop_def_id.clone(), unit)
                })
            });
            if let Some((entity, plant_id, unit)) = target {
                let uptake = world.get::<&CropSoil>(entity).map_or(0.0, |s| s.uptake);
                let days_left = plant_registry.and_then(|r| r.get(&plant_id)).map_or(f64::INFINITY, |d| {
                    soil::days_to_maturity(f64::from(d.growth_days), d.stages().len(), uptake)
                });
                // Compost first, as the button always used; then the other
                // listed fertilizers in order (stored urine), for a player
                // who has no compost.
                let order: Vec<&soil::FertilizerDose> = fertilizer_doses
                    .iter()
                    .filter(|d| d.item == "fertilizer_0")
                    .chain(fertilizer_doses.iter().filter(|d| d.item != "fertilizer_0"))
                    .collect();
                let mut used: Option<soil::FertilizerDose> = None;
                let mut withheld: Option<(String, f64)> = None;
                for (_e, (inv, _ctrl)) in world.query_mut::<(
                    &mut crate::systems::inventory::Inventory,
                    &crate::ecs::components::Controllable,
                )>() {
                    for dose in &order {
                        if !(creative || inv.has_item(&dose.item, 1)) {
                            continue;
                        }
                        if !dose.allowed(days_left) {
                            withheld.get_or_insert_with(|| (dose.item.clone(), dose.withhold_days));
                            continue;
                        }
                        if !creative {
                            inv.remove_item(&dose.item, 1);
                        }
                        used = Some((*dose).clone());
                        break;
                    }
                    break;
                }
                if let Some(dose) = used {
                    let fed = if let Ok(mut s) = world.get::<&mut CropSoil>(entity) {
                        s.store = s.store.plus(dose.available);
                        true
                    } else {
                        false
                    };
                    if !fed {
                        // Never ticked yet: fresh soil plus the fertilizer.
                        let store = soil::fresh_store(need_for(&plant_id)).plus(dose.available);
                        let _ = world.insert_one(entity, CropSoil { store, uptake: 0.0 });
                    }
                    // Compost's organic N goes into the unit's slow pool,
                    // to come back over the years. A hand-planted crop has
                    // no unit to remember it (the same as its soil), so
                    // for it only the first season's share counts.
                    if let Some((area, slot)) = &unit {
                        soil::bank_organic_in(world, area, *slot, dose.organic_n);
                        // Its ammonium acidifies the unit as it nitrifies (soil_ph.rs).
                        if ph_on {
                            let need_n = need_for(&plant_id).n;
                            let def = plant_registry.and_then(|r| r.get(&plant_id));
                            let plot_m2 = soil_ph::plot_area(data, area);
                            soil_ph::add_n_in(world, ph_data, area, *slot, &dose.item, dose.available.n, def, need_n, plot_m2);
                        }
                    }
                    log::info!(
                        "[Farming] fertilized {plant_id} with {}: +{:.2} g N, {:.2} g P2O5, {:.2} g K2O now, {:.2} g organic N banked",
                        dose.item,
                        dose.available.n,
                        dose.available.p2o5,
                        dose.available.k2o,
                        dose.organic_n
                    );
                } else if let Some((item, days)) = withheld {
                    // The withholding rule (for urine, the WHO's month), said
                    // where the player meets it.
                    let what = item_registry
                        .and_then(|r| r.items.get(&item))
                        .map_or(item.clone(), |d| d.name.to_lowercase());
                    push_notice(
                        data,
                        format!(
                            "No {what} on this crop: it ripens in under {days:.0} days, and {what} must go on \
                             at least that long before a harvest (the WHO advises a month between the last \
                             urine and the harvest). Compost is fine."
                        ),
                    );
                }
            }
        }

        // PEST CONTROL (2026-09-26, pests.rs): one control on one grow area,
        // `(area tag, control id from data/garden/pests.ron)`, from the
        // "pest_control_request" channel (the Garden panel's buttons, once
        // wired). Hosing off draws its water the way hand watering does, and
        // an item control takes its items from the backpack (free in creative
        // mode). With pests switched off there is nothing to control.
        let pest_control = data
            .get::<std::sync::Mutex<Option<(String, String)>>>("pest_control_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some((area, control_id)) = pest_control {
            if pest_severity > 0.0 {
                pests::handle_request(world, data, pest_data, &area, &control_id, creative, water_available);
            }
        }
        // SOIL pH AMENDMENT (2026-09-26, soil_ph.rs): Lime or Sulfur on one
        // grow area, `(area tag, amendment id)`, from "soil_ph_request".
        let ph_request = data
            .get::<std::sync::Mutex<Option<(String, String)>>>("soil_ph_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let (Some((area, amendment)), true) = (ph_request, ph_on) {
            let mut need_n = |p: &str| need_for(p).n;
            soil_ph::handle_request(world, data, ph_data, &mut self.ph_rt, &area, &amendment, creative, &mut need_n);
        }

        // DEV: instantly mature every living crop (a testing affordance, like
        // "Dev: stock all materials" — so the loop is verifiable without waiting
        // game-days for growth).
        let dev_grow = data
            .get::<std::sync::Mutex<bool>>("dev_grow_crops")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        if dev_grow {
            for (_e, crop) in world.query_mut::<&mut CropInstance>() {
                if crop.growth_stage == STAGE_DEAD {
                    continue;
                }
                if let Some(last) = plant_registry
                    .and_then(|reg| reg.get(&crop.crop_def_id))
                    .map(|d| d.last_stage().to_string())
                {
                    crop.growth_stage = last;
                }
            }
        }

        // HARVEST: a fully-grown crop -> produce items into the player + despawn it.
        // Volume-gate surplus that doesn't fit the pack routes into a compatible
        // home vessel (the grain silo) instead of vanishing — collected here,
        // applied after the inventory borrow ends. (v0.729)
        let mut vessel_routes: Vec<(String, u32)> = Vec::new();
        let mut harvest_list: Vec<u64> = Vec::new();
        if let Some(bits) = data
            .get::<std::sync::Mutex<Option<u64>>>("harvest_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()))
        {
            harvest_list.push(bits);
        }
        // Bulk harvest (v0.739): the Garden group's "Harvest N ready" button sends
        // every mature crop's bits at once — same code path, one loop.
        if let Some(m) = data.get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request") {
            if let Ok(mut s) = m.lock() {
                harvest_list.append(&mut s);
            }
        }
        // Stressed crops picked this tick, for ONE summary notice after the
        // loop (a bulk harvest of fifty would otherwise post fifty lines):
        // how many, and the sum of their season health for the average.
        let mut stressed_picked = 0u32;
        let mut stressed_health_sum = 0.0f32;
        for bits in harvest_list {
            if let Some(entity) = hecs::Entity::from_bits(bits) {
                // Read the crop (immutable, scoped) to confirm maturity + plant
                // id, and take its season health for the yield below.
                let picked = world.get::<&CropInstance>(entity).ok().and_then(|crop| {
                    let stages: Vec<&str> = plant_registry
                        .and_then(|reg| reg.get(&crop.crop_def_id))
                        .map(|d| d.stages())
                        .unwrap_or_else(|| default_stages.clone());
                    let mature = stages.last().map(|l| crop.growth_stage == *l).unwrap_or(false);
                    if mature {
                        Some((crop.crop_def_id.clone(), season_health(&crop)))
                    } else {
                        None
                    }
                });
                if let Some((plant_id, crop_season_health)) = picked {
                    if let Some(yield_item) = harvest_item_for(&plant_id, plant_registry, item_registry) {
                        // Yield range from the plant def. Yields are FRACTIONAL (f32):
                        // saffron's 0.3 means less than one unit per plant per harvest.
                        // Sanitize the window (min >= 0, max >= min); unknown plants
                        // fall back to exactly 1 unit as before.
                        let (ymin, ymax) = plant_registry
                            .and_then(|reg| reg.get(&plant_id))
                            .map(|d| {
                                let lo = d.yield_min.max(0.0);
                                (lo, d.yield_max.max(lo))
                            })
                            .unwrap_or((1.0, 1.0));
                        // Roll in [ymin, ymax], scale by the crop's season
                        // health, round probabilistically: see harvest_quantity
                        // for the model and its source (2026-09-26; before
                        // this the yield ignored how the crop was kept).
                        let qty = harvest_quantity(
                            ymin,
                            ymax,
                            crop_season_health,
                            rand::random::<f32>(),
                            rand::random::<f32>(),
                        );
                        if crop_season_health < STRESSED_HARVEST_NOTICE_BELOW {
                            stressed_picked += 1;
                            stressed_health_sum += crop_season_health;
                        }
                        let max_stack =
                            item_registry.map(|r| r.max_stack_for(&yield_item)).unwrap_or(99);
                        for (_e, (inv, _ctrl)) in world.query_mut::<(
                            &mut crate::systems::inventory::Inventory,
                            &crate::ecs::components::Controllable,
                        )>() {
                            // Volume-gated (Stage A slice 2): surplus that
                            // doesn't fit the pack routes to a home vessel
                            // after this borrow ends (v0.729).
                            let unit_vol =
                                item_registry.map(|r| r.volume_for(&yield_item)).unwrap_or(0.0);
                            let lost = inv.add_item_volume_gated(&yield_item, qty, max_stack, unit_vol);
                            if lost > 0 {
                                vessel_routes.push((yield_item.clone(), lost));
                            }
                            // Saved-seed loop (operator's "harvest yields seeds"):
                            // a SURVIVAL harvest returns a few seeds of this plant, so
                            // the garden is self-sustaining (plant 1 -> harvest -> get 2
                            // back -> replant + surplus). Creative needs no seeds, so it
                            // stays clean. Plot-agnostic: works for any plot type.
                            if !creative {
                                let seed_id = format!("seed_{plant_id}_0");
                                let seed_stack =
                                    item_registry.map(|r| r.max_stack_for(&seed_id)).unwrap_or(99);
                                let seed_vol =
                                    item_registry.map(|r| r.volume_for(&seed_id)).unwrap_or(0.0);
                                inv.add_item_volume_gated(&seed_id, 2, seed_stack, seed_vol);
                            }
                            log::info!("[Farming] harvested {qty}x {yield_item} from {plant_id}");
                            // Harvesting trains Farming (scales lightly with yield).
                            crate::systems::skills::award_skill_xp(data, "farming", 10 + qty * 2);
                            // Quest progress: a harvest of this crop (Harvest objectives).
                            crate::systems::quests::push_quest_event(
                                data,
                                format!("harvest_{}", plant_id),
                            );
                            break;
                        }
                    } else {
                        log::warn!(
                            "[Farming] {plant_id} has no produce item in items.csv; harvest yielded nothing"
                        );
                    }
                    // The unit keeps what this crop left in it, for the next
                    // crop sown there (2026-09-26, soil.rs). A legume also
                    // leaves the FIXED nitrogen in its roots, nodules and
                    // haulm (soil::legume_credit_n), in proportion to the
                    // share of its season it actually grew: a crop ripened
                    // by the dev button fixed nothing and leaves nothing.
                    let unit = world
                        .get::<&CropInstance>(entity)
                        .ok()
                        .and_then(|c| c.tower_id.clone().zip(c.tower_slot));
                    let left = world.get::<&CropSoil>(entity).ok().map(|s| (s.store, s.uptake));
                    if let (Some((area, slot)), Some((store, uptake))) = (unit, left) {
                        let fixed = plant_registry
                            .and_then(|r| r.get(&plant_id))
                            .map_or(0.0, |d| f64::from(d.n_fixed_share));
                        let removal = crop_removal(&plant_id, plant_registry, item_registry, demand_scale);
                        let credit = soil::legume_credit_n(removal.n, fixed, legume_harvest_share)
                            * f64::from(uptake.clamp(0.0, 1.0));
                        if credit > 0.0 {
                            log::info!("[Farming] {plant_id} left {credit:.3} g of fixed N in {area} unit {slot}");
                        }
                        soil::remember(world, &area, slot, Npk::new(store.n + credit, store.p2o5, store.k2o));
                    }
                    let _ = world.despawn(entity);
                }
            }
        }
        // Say why a harvest came in light (2026-09-26): the Garden panel
        // shows each crop's CURRENT health, which has usually recovered by
        // harvest day, so without this line a stressed season's small
        // harvest would look like bad luck.
        if stressed_picked > 0 {
            let pct = (stressed_health_sum / stressed_picked as f32 * 100.0).round();
            let what = if stressed_picked == 1 {
                "This crop was".to_string()
            } else {
                format!("{stressed_picked} of these crops were")
            };
            push_notice(
                data,
                format!(
                    "{what} stressed while growing (thirst, too few nutrients, pests, \
                     RF or hard acceleration) and gave about {pct}% of a full harvest."
                ),
            );
        }

        // Route harvest surplus into a compatible home vessel (v0.729): a
        // full backpack no longer LOSES the yield when the grain silo can
        // take it. Compatibility is PRE-CHECKED so surplus never dents a
        // wrong-class vessel (try_store on an incompatible container damages
        // it by design — we don't want grain denting the fuel drum).
        if !vessel_routes.is_empty() {
            use crate::systems::inventory::containers::{Container, ContainerRegistry, StoreOutcome};
            let containers_reg = data.get::<ContainerRegistry>("container_registry");
            for (item_id, qty) in vessel_routes {
                let class = item_registry
                    .map(|r| r.class_for(&item_id).to_string())
                    .unwrap_or_else(|| "solid".to_string());
                let unit_vol = item_registry.map(|r| r.volume_for(&item_id)).unwrap_or(0.0);
                let mut remaining = qty;
                if let Some(reg) = containers_reg {
                    for (_e, c) in world.query_mut::<&mut Container>() {
                        if remaining == 0 {
                            break;
                        }
                        if !reg.check(&c.container_type_id, &class).is_accepted() {
                            continue;
                        }
                        if let StoreOutcome::Stored { quantity } =
                            reg.try_store(c, &item_id, &class, unit_vol, remaining)
                        {
                            remaining -= quantity;
                            log::info!(
                                "[Farming] pack full: {quantity}x {item_id} routed into the {}",
                                c.container_type_id
                            );
                        }
                    }
                }
                // What neither the pack nor a vessel took goes to home storage
                // (the Barn, 2026-09-26), where it shows as crates and sacks;
                // it used to be thrown away with a log line.
                if remaining > 0 {
                    match data.get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs") {
                        Some(m) => {
                            if let Ok(mut out) = m.lock() {
                                out.push((item_id.clone(), remaining));
                            }
                        }
                        None => log::warn!(
                            "[Farming] pack + vessels full: {remaining}x {item_id} lost at harvest"
                        ),
                    }
                }
            }
        }

        // Collect entities to update (avoid borrow conflict with world)
        let mut updates: Vec<(hecs::Entity, CropInstance, CropSoil)> = Vec::new();

        // The automatic feeder's fertilizers (2026-09-26): every one listed in
        // data/garden/nutrients.ron, in order (stored urine for nitrogen, then
        // compost for the rest), drawn from home storage (the same
        // "home_stock" mirror of the Barn the automated machines use; lib.rs
        // takes what was used back out of the containers after the tick).
        // Absent (tests, early boot) = no stock, so survival feeding finds
        // nothing and says so; creative mode feeds for free.
        let home_stock = data
            .get::<std::sync::Mutex<std::collections::HashMap<String, u32>>>("home_stock");
        let mut home_stock = home_stock.and_then(|m| m.lock().ok());
        let mut feeder_ran_dry = false;
        // Grow areas with a crop short of a nutrient this tick, and which
        // nutrient is scarcest there (for one notice per shortage).
        let mut short_now: HashMap<String, soil::Nutrient> = HashMap::new();
        // Every unit's banked organic N (compost's slow release), taken out of
        // the world's SoilMemory for the loop below (the crop query holds the
        // world) and put back after it.
        let memory = soil::soil_memory_entity(world);
        let mut organic = world
            .get::<&mut crate::ecs::components::SoilMemory>(memory)
            .map(|mut m| std::mem::take(&mut m.organic))
            .unwrap_or_default();
        // Every area's pests, taken out the same way and put back after.
        let mut area_pests = world
            .get::<&mut crate::ecs::components::SoilMemory>(memory)
            .map(|mut m| std::mem::take(&mut m.pests))
            .unwrap_or_default();
        // Every soil unit's pH (soil_ph.rs), taken out the same way. Its lime,
        // sulfur and nitrifying ammonium react first, on garden days (the
        // crops' clock, not paused at night: soil chemistry goes on).
        let mut ph_units = world
            .get::<&mut crate::ecs::components::SoilMemory>(memory)
            .map(|mut m| std::mem::take(&mut m.ph))
            .unwrap_or_default();
        if ph_on {
            soil_ph::step_all(&mut ph_units, ph_data, game_dt * f64::from(growth_speed) / SECONDS_PER_DAY);
        }
        let mut ph_out: HashMap<String, soil_ph::OutOfWindow> = HashMap::new();

        // PESTS: step every grow area's pressure on the garden clock
        // (2026-09-26, pests.rs). First a tally of each area's living crops
        // (mature ones too: a ripe plant still feeds a pest until it is
        // picked) and, per pest, how much of the area feeds it in the
        // conditions it likes; then each area with crops or pests steps.
        if pest_severity > 0.0 {
            // Garden days this tick: game time at the growth speed, the clock
            // the crops ripen on, so a pest's life cycle keeps its real length
            // against the season. Continuous, not light-gated: pests feed in
            // the dark too (slugs mostly at night).
            let pest_days = game_dt * f64::from(growth_speed) / SECONDS_PER_DAY;
            // Per area: living crops, and per pest (by its index in
            // pests.ron) the summed favour. Indexed so this pass over every
            // crop allocates nothing per crop.
            let n_pests = pest_data.pests.len();
            let mut tally: HashMap<String, (usize, Vec<f64>)> = HashMap::new();
            for (_e, (c, s)) in world.query::<(&CropInstance, Option<&CropSoil>)>().iter() {
                if c.growth_stage == STAGE_DEAD {
                    continue;
                }
                let area = c.tower_id.as_deref().unwrap_or("");
                let outdoors = is_field_area(area);
                let need_n = need_for(&c.crop_def_id).n;
                let cond = pests::CropConditions {
                    outdoors,
                    temp_c: if outdoors { f64::from(weather_temp) } else { pest_data.indoor_temp_c },
                    water_stressed: c.water_level < WATER_STRESS_THRESHOLD,
                    excess_n: s.map_or(false, |s| need_n > 0.0 && s.store.n > pest_data.excess_n_seasons * need_n),
                    damp: outdoors && (damp_weather || !sun_up),
                    season: &season,
                };
                if !tally.contains_key(area) {
                    tally.insert(area.to_string(), (0, vec![0.0; n_pests]));
                }
                let entry = tally.get_mut(area).expect("inserted above");
                entry.0 += 1;
                for (i, pest) in pest_data.pests.iter().enumerate() {
                    entry.1[i] += pests::favour(pest, &c.crop_def_id, &cond, pest_data.unfavoured_rate);
                }
            }
            let mut areas: Vec<String> = tally.keys().chain(area_pests.keys()).cloned().collect();
            areas.sort();
            areas.dedup();
            // Pests that just became noticeable, and where: one notice per
            // pest per tick, however many areas it turned up in.
            let mut appeared: Vec<(String, Vec<String>)> = Vec::new();
            for area in areas {
                let (n, fav): (usize, &[f64]) = tally.get(&area).map_or((0, &[]), |(n, f)| (*n, f.as_slice()));
                let st = area_pests.entry(area.clone()).or_default();
                for pest_id in pests::step_area(st, pest_data, fav, n, pest_days) {
                    match appeared.iter_mut().find(|(p, _)| *p == pest_id) {
                        Some((_, list)) => list.push(area.clone()),
                        None => appeared.push((pest_id, vec![area.clone()])),
                    }
                }
                if st.pressure.is_empty() && st.releases.is_empty() {
                    area_pests.remove(&area);
                }
            }
            for (pest_id, list) in appeared {
                if let Some(pest) = pest_data.pest(&pest_id) {
                    push_notice(data, pests::appeared_notice(pest_data, pest, &list));
                }
            }
        }

        for (entity, (crop, crop_soil)) in world.query_mut::<(&CropInstance, Option<&CropSoil>)>() {
            // Skip dead crops
            if crop.growth_stage == STAGE_DEAD {
                continue;
            }

            // Resolve this plant's stage list
            let plant_stages: Vec<&str> = plant_registry
                .as_ref()
                .and_then(|reg| reg.get(&crop.crop_def_id))
                .map(|def| def.stages())
                .unwrap_or_else(|| default_stages.clone());

            // Skip crops already at their final stage (they sit until harvested)
            if let Some(last) = plant_stages.last() {
                if crop.growth_stage == *last {
                    continue;
                }
            }

            let mut crop = crop.clone();

            // NUTRIENTS (2026-09-26, soil.rs). The crop's unit: what it had,
            // or fresh soil if it never ticked (newly sown in an unremembered
            // unit, spawned by the showcase, or loaded from a save, which does
            // not carry soil yet).
            let need = need_for(&crop.crop_def_id);
            let mut crop_soil = crop_soil.cloned().unwrap_or_else(|| CropSoil {
                store: soil::fresh_store(need),
                uptake: 0.0,
            });
            // The unit's garden clock: this crop's days to ripening, and how
            // far it has come (for urine's withholding period below).
            let growth_days = plant_registry
                .and_then(|reg| reg.get(&crop.crop_def_id))
                .map_or(0.0, |d| f64::from(d.growth_days));
            // The feeder, where this area's "nutrient" slider is set: top the
            // unit up to the slider's target, one fertilizer at a time in the
            // listed order, each for the nutrients it is dosed by, from that
            // area's opened item of it, opening another from home storage
            // when it runs out. Urine goes first, for the nitrogen; compost
            // then covers the phosphorus and potassium, and any nitrogen urine
            // could not (none in stock, or the crop is within urine's month
            // of harvest).
            if let Some(tid) = &crop.tower_id {
                if let Some(v) = nutrient.get(tid).copied() {
                    let target = soil::feed_target(need, v);
                    let days_left = soil::days_to_maturity(growth_days, plant_stages.len(), crop_soil.uptake);
                    let mut could_supply = [false; 3];
                    for dose_def in fertilizer_doses.iter().filter(|d| d.allowed(days_left)) {
                        let per_item = dose_def.available;
                        for (i, per) in [per_item.n, per_item.p2o5, per_item.k2o].into_iter().enumerate() {
                            could_supply[i] |= dose_def.dose_for[i] && per > 0.0;
                        }
                        let dose = soil::feed_dose_for(&crop_soil.store, &target, &per_item, dose_def.dose_for);
                        if dose <= 0.0 {
                            continue;
                        }
                        let given = if creative {
                            dose
                        } else {
                            let open = self.feed_open.entry((tid.clone(), dose_def.item.clone())).or_insert(0.0);
                            while *open < dose {
                                let took = home_stock
                                    .as_mut()
                                    .and_then(|s| s.get_mut(&dose_def.item))
                                    .map_or(false, |q| {
                                        if *q > 0 {
                                            *q -= 1;
                                            true
                                        } else {
                                            false
                                        }
                                    });
                                if !took {
                                    break;
                                }
                                *open += 1.0;
                                self.feeder_dry_told = false;
                            }
                            let given = dose.min(*open);
                            *open -= given;
                            given
                        };
                        crop_soil.store = crop_soil.store.plus(per_item.scaled(given));
                        // Compost's organic N banks in the unit's slow pool.
                        if let Some(slot) = crop.tower_slot {
                            let pool = organic.entry(tid.clone()).or_default().entry(slot).or_default();
                            soil::bank_organic(pool, dose_def.organic_n * given);
                            // Its ammonium acidifies the unit as it nitrifies.
                            if ph_on {
                                let def = plant_registry.and_then(|r| r.get(&crop.crop_def_id));
                                soil_ph::add_n(&mut ph_units, ph_data, tid, slot, &dose_def.item, per_item.n * given, def, need.n, soil_ph::plot_area(data, tid));
                            }
                        }
                    }
                    // Out of stock: after every fertilizer had its turn, the
                    // unit still lacks a nutrient one of them could have
                    // supplied. (A tiny tolerance for the float sums.)
                    let short = |have: f64, want: f64| have < want - 1e-9 * want.abs().max(1.0);
                    if !creative
                        && ((could_supply[0] && short(crop_soil.store.n, target.n))
                            || (could_supply[1] && short(crop_soil.store.p2o5, target.p2o5))
                            || (could_supply[2] && short(crop_soil.store.k2o, target.k2o)))
                    {
                        feeder_ran_dry = true;
                    }
                }
            }
            // Liebig: the scarcest nutrient caps the crop's health (soil.rs).
            let (supplied, scarcest) = soil::sufficiency(&crop_soil.store, &need);
            let nutrient_ceiling = soil::health_ceiling(supplied);
            if supplied < 1.0 {
                short_now
                    .entry(crop.tower_id.clone().unwrap_or_default())
                    .or_insert(scarcest);
            }

            // Dehydration: water level drops over time
            crop.water_level = (crop.water_level - DEHYDRATION_RATE * dt).max(0.0);

            // Per-area irrigation: if the crop's grow area is configured with a water
            // target, automated irrigation keeps it topped up to that level. A high
            // setting holds crops well-watered (healthy, fast growth); a low setting
            // lets them dehydrate and wilt -- so the garden edit slider is meaningful.
            // GATED on the home's water sim (v0.611): a dry cistern can't feed the
            // irrigation, so the top-up is suppressed and the crops dehydrate.
            //
            // A grow area with NO slider set is watered by the home's automated
            // irrigation at DEFAULT_AUTO_IRRIGATION (2026-09-25). Before this an
            // un-configured area got nothing, so every crop still growing died of
            // thirst about eight minutes into a session (0.002/s down to the 0.2
            // stress line, then 100 s of health): the operator's save held 1,575
            // of 1,976 crops dead of thirst, and a new player's showcase garden
            // went the same way. A slider still overrides it (set it low to let an
            // area dry out), a dry cistern still cuts it off, and a crop planted
            // outside any grow area still needs watering by hand.
            if water_available && irrigation_on {
                if let Some(tid) = &crop.tower_id {
                    let target = irrigation.get(tid).copied().unwrap_or(DEFAULT_AUTO_IRRIGATION);
                    crop.water_level = crop.water_level.max(target);
                    // The water is real: this crop's daily need, from
                    // plants.csv, counts toward what the irrigation draws.
                    if target > 0.0 {
                        irrigation_l_per_day += plant_registry
                            .as_ref()
                            .and_then(|r| r.get(&crop.crop_def_id))
                            .map_or(0.0, |d| d.water_per_day);
                    }
                }
            }
            // Rain on an outdoor field (2026-09-25).
            if rain > 0.0 && crop.tower_id.as_deref().map_or(false, is_field_area) {
                crop.water_level = (crop.water_level + RAIN_WATER_PER_S * rain * dt).min(1.0);
            }

            // Pests (2026-09-26, pests.rs): each pest in this crop's area that
            // eats it caps its health by its pressure there, the same way a
            // nutrient shortage does, and never below pests::PEST_HEALTH_FLOOR.
            // The lower of the two caps is the one that binds.
            let pest_ceiling = if pest_severity > 0.0 {
                pests::health_ceiling(
                    pest_data,
                    area_pests.get(crop.tower_id.as_deref().unwrap_or("")),
                    &crop.crop_def_id,
                    pest_severity,
                )
            } else {
                100.0
            };
            // Soil pH (soil_ph.rs): outside its window a crop's nutrients are
            // less available, capped the same way; said once per area below.
            let def = plant_registry.and_then(|r| r.get(&crop.crop_def_id));
            let area = crop.tower_id.as_deref().unwrap_or("");
            let (ph_ceiling, off) = soil_ph::check_crop(&ph_units, ph_data, ph_on, area, crop.tower_slot, def);
            if let Some(o) = off {
                ph_out.entry(area.to_string()).or_insert(o);
            }
            let ceiling = nutrient_ceiling.min(pest_ceiling).min(ph_ceiling);

            // Health effects from water level, capped by nutrients and pests.
            if crop.water_level < WATER_STRESS_THRESHOLD {
                // Water stress -- health decays
                crop.health = (crop.health - HEALTH_DECAY_RATE * dt).max(0.0);
            } else if crop.health < ceiling {
                // Well watered -- health recovers, as far as its nutrients
                // and pests allow (100 when fed and pest-free, so unchanged
                // for a healthy crop).
                crop.health = (crop.health + HEALTH_RECOVERY_RATE * dt).min(ceiling);
            } else {
                // Watered but short of a nutrient or eaten by a pest: health
                // eases down to what the cap allows, slowly, and never below
                // the floors (soil::NUTRIENT_HEALTH_FLOOR,
                // pests::PEST_HEALTH_FLOOR). A shortage or an infestation
                // stunts; it does not kill. Fertilize, or knock the pest
                // back, and the cap lifts at once.
                crop.health = (crop.health - soil::NUTRIENT_DECLINE_RATE * dt).max(ceiling);
            }

            // RF stress (v0.620): a powered wireless emitter (WiFi router) bathes the grow in RF; crops
            // lose health proportional to the home RF level. Run wired / Li-Fi or remove the emitter to
            // protect the grow (the operator's "tradeoffs bite"). Outpaces recovery at one router's worth.
            if home_rf > RF_HARM_THRESHOLD {
                crop.health = (crop.health - RF_HEALTH_PENALTY * home_rf * dt).max(0.0);
            }

            // Sustained acceleration snaps stems and collapses trellises. Same
            // shape as the RF drain above: a ship-wide scalar eating crop health
            // until something gives. Silent at cruise, lethal during an evasion
            // burn -- which makes "the farm dies if you run from the missile" a
            // consequence of the flight plan rather than a scripted event.
            if g_harm_per_sec > 0.0 {
                crop.health = (crop.health - g_harm_per_sec * dt).max(0.0);
            }

            // Season health record (2026-09-26): every tick of growth adds
            // this tick's health to the running total the harvest reads (see
            // season_health). Taken after every stress above so a tick spent
            // thirsty, in RF or under a burn counts at the health it left the
            // crop with. Mature crops never reach this line (skipped above),
            // so the record covers the growing season and nothing after it.
            crop.health_seconds += f64::from((crop.health / 100.0).clamp(0.0, 1.0)) * f64::from(dt);
            crop.growing_seconds += f64::from(dt);

            // If health hits zero, crop dies
            if crop.health <= 0.0 {
                crop.growth_stage = STAGE_DEAD.to_string();
                updates.push((entity, crop, crop_soil));
                continue;
            }

            // LIGHT (2026-09-26): run this crop's growth clock at the rate
            // its light allows (see light_growth_rate). Growth below is read
            // from the crop's age, elapsed minus planted_at, so the clock is
            // run by moving planted_at: forward by this tick's game time in
            // the dark (the age holds still), back by it in the light (the
            // age gains two seconds a second). The same device the offline
            // catch-up in save_load.rs uses. Clamped so a crop is never
            // planted in the future. Health, water and the season record
            // above are untouched: the dark pauses a crop, it does not harm
            // it.
            let outdoors = crop.tower_id.as_deref().map_or(false, is_field_area);
            let needs_light = plant_registry
                .and_then(|reg| reg.get(&crop.crop_def_id))
                .map_or(true, |d| d.needs_light);
            let cover = crop.tower_id.as_deref().and_then(|t| lamp_cover.get(t)).copied().unwrap_or(0.0);
            let light_rate = light_growth_rate(needs_light, outdoors, sun_up, cover);
            crop.planted_at = (crop.planted_at + game_dt * (1.0 - light_rate)).min(elapsed_seconds);

            // Calculate growth progress based on elapsed time since planting
            if let Some(registry) = plant_registry {
                if let Some(plant_def) = registry.get(&crop.crop_def_id) {
                    // Total growth time in game seconds
                    let growth_seconds = plant_def.growth_days as f64 * SECONDS_PER_DAY;

                    if growth_seconds > 0.0 {
                        let age = elapsed_seconds - crop.planted_at;
                        let progress = (age / growth_seconds) as f32;

                        // Health-weighted progress: unhealthy crops grow slower.
                        // Nutrients act through this (and through the season
                        // health the harvest reads): a crop short of its
                        // scarcest nutrient has its health capped above, so it
                        // grows slower and yields less. The "nutrient" slider's
                        // old direct growth multiplier (0.5x..1.5x, until
                        // 2026-09-26) is gone: the slider now runs the feeder,
                        // and past sufficiency more feed grows nothing faster.
                        let health_factor = (crop.health / 100.0).max(0.1);
                        // Outdoor climate (v0.749, ladder rung 6): FIELD crops
                        // face the season + live weather; indoor grows (towers,
                        // beds, trays, racks) are climate-controlled and skip
                        // this — the tradeoff that justifies indoor farming.
                        // Off-season fields crawl at 25%; temperature outside
                        // the plant's window slows growth toward a 20% floor
                        // (5% per degree) — slow, not lethal (death stays
                        // water/RF-driven).
                        let climate_factor = if crop
                            .tower_id
                            .as_deref()
                            .map_or(false, is_field_area)
                        {
                            let season_ok = season.is_empty()
                                || plant_def.seasons.is_empty()
                                || plant_def
                                    .seasons
                                    .iter()
                                    .any(|s| s.eq_ignore_ascii_case(&season));
                            let season_f: f32 = if season_ok { 1.0 } else { 0.25 };
                            let temp_f: f32 = if weather_temp < plant_def.temp_min_c {
                                (1.0 - (plant_def.temp_min_c - weather_temp) * 0.05)
                                    .clamp(0.2, 1.0)
                            } else if weather_temp > plant_def.temp_max_c {
                                (1.0 - (weather_temp - plant_def.temp_max_c) * 0.05)
                                    .clamp(0.2, 1.0)
                            } else {
                                1.0
                            };
                            season_f * temp_f
                        } else {
                            1.0
                        };
                        // growth_speed is the player/dev multiplier; it multiplies
                        // PROGRESS, so 10x reaches harvest in a tenth of the real
                        // growth_days while the world clock is untouched.
                        let clock_progress = progress * climate_factor * growth_speed;
                        let effective_progress = clock_progress * health_factor;

                        // The crop draws its season need in step with its
                        // growth clock (soil::uptake_fraction): nothing in the
                        // dark, ten times as fast at 10x, and after the offline
                        // catch-up the whole time away in one go, so growth
                        // made while the player was away is paid for from the
                        // unit on return. What the unit cannot give the crop
                        // goes without; the cap above already says so.
                        let drawn_by_now =
                            soil::uptake_fraction(clock_progress, plant_stages.len());
                        if drawn_by_now > crop_soil.uptake {
                            let step = f64::from(drawn_by_now - crop_soil.uptake);
                            // Compost's slow release first (2026-09-26): the
                            // unit's banked organic N ages by the garden days
                            // this crop just grew through, and what it gives
                            // up joins the store in time for this draw.
                            if let (Some(tid), Some(slot)) = (&crop.tower_id, crop.tower_slot) {
                                if let Some(pool) = organic.get_mut(tid).and_then(|m| m.get_mut(&slot)) {
                                    let days = soil::garden_days(growth_days, plant_stages.len(), step);
                                    crop_soil.store.n += soil::release_organic(pool, days, &organic_yearly);
                                }
                            }
                            let want = need.scaled(step);
                            soil::draw(&mut crop_soil.store, want);
                            crop_soil.uptake = drawn_by_now;
                        }

                        let new_stage =
                            stage_from_progress(effective_progress, &plant_stages);

                        // Only advance forward, never regress (except to Dead)
                        let current_idx = stage_index(&crop.growth_stage, &plant_stages);
                        let new_idx = stage_index(new_stage, &plant_stages);

                        if let (Some(cur), Some(nxt)) = (current_idx, new_idx) {
                            if nxt > cur {
                                crop.growth_stage = new_stage.to_string();
                                log::debug!(
                                    "Crop {} advanced to {}",
                                    crop.crop_def_id,
                                    crop.growth_stage
                                );
                            }
                        }
                    }
                }
            }

            updates.push((entity, crop, crop_soil));
        }
        drop(home_stock);
        // The banked organic N and the pests go back where they live.
        if let Ok(mut m) = world.get::<&mut crate::ecs::components::SoilMemory>(memory) {
            m.organic = organic;
            m.pests = area_pests;
            m.ph = ph_units;
        }
        self.ph_rt.tell(data, ph_data, &ph_out);

        // Say so when crops run short (2026-09-26): the Garden panel does not
        // show a unit's N-P-K yet, so without this a starving bed only shows
        // as falling health. Once per shortage per grow area, as one line.
        let newly_short: Vec<(String, soil::Nutrient)> = short_now
            .iter()
            .filter(|(area, _)| !self.short_told.contains(*area))
            .map(|(a, n)| (a.clone(), *n))
            .collect();
        self.short_told.retain(|area| short_now.contains_key(area));
        if !newly_short.is_empty() {
            for (area, _) in &newly_short {
                self.short_told.insert(area.clone());
            }
            push_notice(data, short_notice(&newly_short));
        }
        if feeder_ran_dry && !self.feeder_dry_told {
            self.feeder_dry_told = true;
            // Name what it feeds with ("stored urine and fertilizer").
            let names: Vec<String> = fertilizer_doses
                .iter()
                .map(|d| {
                    item_registry
                        .and_then(|r| r.items.get(&d.item))
                        .map_or(d.item.clone(), |i| i.name.to_lowercase())
                })
                .collect();
            let name = if names.is_empty() { "fertilizer".to_string() } else { names.join(" and ") };
            push_notice(
                data,
                format!(
                    "The garden feeder has run out of {name} in home storage: fed crops will go \
                     short until you compost more."
                ),
            );
        }

        // What the irrigation draws, in litres per minute of real time: the
        // plants' daily need is per real day, and the plumbing sim runs on
        // real minutes like the rest of the home's machines.
        if let Some(m) = data.get::<std::sync::Mutex<f32>>("irrigation_demand_lpm") {
            if let Ok(mut v) = m.lock() {
                *v = irrigation_l_per_day / 1440.0;
            }
        }

        // Apply updates back to the world. A crop ticking for the first time
        // gets its soil component here.
        for (entity, crop, crop_soil) in updates {
            if let Ok(mut existing) = world.get::<&mut CropInstance>(entity) {
                *existing = crop;
            }
            let had_soil = match world.get::<&mut CropSoil>(entity) {
                Ok(mut s) => {
                    *s = crop_soil.clone();
                    true
                }
                Err(_) => false,
            };
            if !had_soil {
                let _ = world.insert_one(entity, crop_soil);
            }
        }

        self._initialized = true;
    }
}

#[cfg(test)]
mod gardening_tests {
    use super::*;
    use crate::ecs::components::{Controllable, CropInstance};
    use crate::ecs::systems::System;
    use crate::hot_reload::data_store::DataStore;
    use crate::systems::inventory::{Inventory, ItemRegistry};

    /// DataStore with plant + item registries and the four gardening channels,
    /// mirroring the runtime wiring in lib.rs. Shared with nutrient_tests.
    pub(super) fn make_store() -> DataStore {
        let mut data = DataStore::new();
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
        data.insert("plant_registry", plants);
        data.insert("item_registry", items);
        data.insert(
            "game_time",
            std::sync::Mutex::new(crate::systems::time::GameTime::default()),
        );
        data.insert("plant_request", std::sync::Mutex::new(Option::<String>::None));
        data.insert(
            "plant_tower_request",
            std::sync::Mutex::new(Option::<(String, Vec<String>)>::None),
        );
        data.insert("water_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("harvest_request", std::sync::Mutex::new(Option::<u64>::None));
        data.insert("dev_grow_crops", std::sync::Mutex::new(false));
        data.insert(
            "fertilize_crop_request",
            std::sync::Mutex::new(Option::<u64>::None),
        );
        data.insert(
            "stock_seeds_request",
            std::sync::Mutex::new(Option::<Vec<String>>::None),
        );
        data.insert(
            "plant_bed_request",
            std::sync::Mutex::new(Option::<(String, String, u32)>::None),
        );
        data.insert("harvest_many_request", std::sync::Mutex::new(Vec::<u64>::new()));
        // One tower the light tests stand a grow light over (lighting.rs).
        data.insert("garden_lighting", lighting::LightingData::parse(lighting::LIGHTING_RON).unwrap());
        data.insert(
            "grow_plots",
            vec![lighting::GrowPlot { id: "ntower_3".into(), cups: 12, ..Default::default() }],
        );
        data
    }

    fn set_string(data: &DataStore, key: &str, v: &str) {
        *data
            .get::<std::sync::Mutex<Option<String>>>(key)
            .unwrap()
            .lock()
            .unwrap() = Some(v.to_string());
    }

    /// Full gardening loop: plant a seed (consumed → crop spawned) → dev-grow to
    /// maturity → harvest (produce yielded into the player + crop despawned).
    #[test]
    fn plant_grow_harvest_full_loop() {
        let data = make_store();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("seed_tomato_0", 1, 99);
        let player = world.spawn((inv, Controllable));

        // PLANT.
        set_string(&data, "plant_request", "seed_tomato_0");
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("seed_tomato_0"),
            0,
            "seed consumed by planting"
        );
        let crops: Vec<hecs::Entity> =
            world.query::<&CropInstance>().iter().map(|(e, _)| e).collect();
        assert_eq!(crops.len(), 1, "exactly one crop planted");
        let crop_entity = crops[0];
        assert_eq!(
            world.get::<&CropInstance>(crop_entity).unwrap().crop_def_id,
            "tomato",
            "seed mapped to the tomato plant def"
        );

        // DEV-GROW to maturity (tomato's last stage is `ripe`).
        *data.get::<std::sync::Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.get::<&CropInstance>(crop_entity).unwrap().growth_stage,
            "ripe",
            "dev-grow matured the crop"
        );

        // HARVEST: yield produce + despawn the crop.
        *data
            .get::<std::sync::Mutex<Option<u64>>>("harvest_request")
            .unwrap()
            .lock()
            .unwrap() = Some(crop_entity.to_bits().into());
        sys.tick(&mut world, 1.0, &data);
        assert!(
            world.get::<&CropInstance>(crop_entity).is_err(),
            "harvested crop was despawned"
        );
        let tomatoes = world
            .get::<&Inventory>(player)
            .unwrap()
            .count_item("vegetable_tomato_0");
        assert!(
            tomatoes >= 2,
            "harvest yielded produce (>= yield_min 2 tomatoes), got {tomatoes}"
        );
        // Saved-seed loop: this survival harvest returned seeds (planted 1 -> 0, then
        // the harvest granted 2 back), so the garden is self-sustaining.
        let seeds = world.get::<&Inventory>(player).unwrap().count_item("seed_tomato_0");
        assert_eq!(seeds, 2, "survival harvest yielded 2 seeds, got {seeds}");
    }

    /// The bed Plant button sends a machine TYPE; it sows one crop in every
    /// plot of every machine of that type, each tagged with the machine it
    /// stands in, and a second press fills only the empty plots (2026-09-26).
    /// Seen red by ignoring "grow_instances": the crops were then tagged with
    /// the type id, which the renderer and the grow lights cannot place.
    #[test]
    fn planting_a_bed_type_sows_every_plot_of_every_machine() {
        let data = {
            let mut d = make_store();
            let mut m: HashMap<String, Vec<(String, u32)>> = HashMap::new();
            m.insert("staple_grain_tray".into(), vec![("graintray_0".into(), 2), ("graintray_1".into(), 2)]);
            d.insert("grow_instances", m);
            d
        };
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(24);
        inv.add_item("seed_wheat_0", 10, 50);
        world.spawn((inv, Controllable));
        let sow = |sys: &mut FarmingSystem, world: &mut hecs::World| {
            *data
                .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
                .unwrap()
                .lock()
                .unwrap() = Some(("staple_grain_tray".to_string(), "wheat".to_string(), 2));
            sys.tick(world, 1.0, &data);
        };
        sow(&mut sys, &mut world);
        let mut spots: Vec<(String, u32)> = world
            .query::<&CropInstance>()
            .iter()
            .map(|(_, c)| (c.tower_id.clone().unwrap_or_default(), c.tower_slot.unwrap_or(99)))
            .collect();
        spots.sort();
        let want: Vec<(String, u32)> =
            [("graintray_0", 0), ("graintray_0", 1), ("graintray_1", 0), ("graintray_1", 1)].iter().map(|(a, b)| (a.to_string(), *b)).collect();
        assert_eq!(spots, want, "one crop per plot, tagged with its machine");
        sow(&mut sys, &mut world);
        assert_eq!(world.query::<&CropInstance>().iter().count(), 4, "a second press finds every plot full");
    }

    /// A harvest the pack cannot take, with no vessel for it, goes to home
    /// storage (the Barn) instead of being thrown away (2026-09-26).
    #[test]
    fn a_harvest_the_pack_cannot_take_goes_to_home_storage() {
        let mut data = make_store();
        data.insert("home_stock_outputs", std::sync::Mutex::new(Vec::<(String, u32)>::new()));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(24);
        inv.add_item("seed_wheat_0", 3, 50);
        let player = world.spawn((inv, Controllable));
        *data
            .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("staple_grain_tray".to_string(), "wheat".to_string(), 3));
        sys.tick(&mut world, 1.0, &data);
        let crops: Vec<hecs::Entity> = world.query::<&CropInstance>().iter().map(|(e, _)| e).collect();
        *data.get::<std::sync::Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        // The pack is full to its volume, and there is no vessel.
        {
            let mut inv = world.get::<&mut Inventory>(player).unwrap();
            inv.volume_current_l = inv.volume_capacity_l;
        }
        *data.get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() =
            crops.iter().map(|e| e.to_bits().into()).collect();
        sys.tick(&mut world, 1.0, &data);
        let filed: u32 = data
            .get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs")
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id == "grain_wheat_0")
            .map(|(_, q)| *q)
            .sum();
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("grain_wheat_0"), 0, "the pack is full");
        assert!(filed >= 24, "the harvest went to home storage, not away: {filed}");
    }

    /// v0.739 BULK HARVEST: the "Harvest N ready" button sends every mature
    /// crop's bits through harvest_many_request; one tick picks them all
    /// (immature crops in the list are left standing).
    #[test]
    fn bulk_harvest_picks_every_ready_crop_in_one_tick() {
        let data = make_store();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(24);
        inv.add_item("seed_wheat_0", 3, 50);
        let player = world.spawn((inv, Controllable));

        *data
            .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("staple_grain_tray".to_string(), "wheat".to_string(), 3));
        sys.tick(&mut world, 1.0, &data);
        let crops: Vec<hecs::Entity> =
            world.query::<&CropInstance>().iter().map(|(e, _)| e).collect();
        assert_eq!(crops.len(), 3);

        // Mature them all, then bulk-harvest the whole set in ONE tick.
        *data.get::<std::sync::Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        *data
            .get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request")
            .unwrap()
            .lock()
            .unwrap() = crops.iter().map(|e| e.to_bits().into()).collect();
        sys.tick(&mut world, 1.0, &data);

        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            0,
            "all three crops harvested + despawned in one tick"
        );
        let grain = world.get::<&Inventory>(player).unwrap().count_item("grain_wheat_0");
        assert!(
            grain >= 24,
            "three harvests of wheat (yield_min 8 each) landed, got {grain}"
        );
    }

    /// v0.738 GRAIN LOOP: bed-planting a grain tray sows one wheat crop per unit
    /// (consuming one seed each in survival), grows, and harvest yields REAL
    /// grain_wheat_0 — the item the grain silo accepts (dry_goods). Replanting is
    /// idempotent: live units are skipped, so no crop stacking.
    #[test]
    fn bed_plant_grow_harvest_yields_grain() {
        let data = make_store();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(24);
        inv.add_item("seed_wheat_0", 8, 50);
        let player = world.spawn((inv, Controllable));

        // PLANT the 8-unit tray.
        *data
            .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("staple_grain_tray".to_string(), "wheat".to_string(), 8));
        sys.tick(&mut world, 1.0, &data);
        let crops: Vec<hecs::Entity> = world
            .query::<&CropInstance>()
            .iter()
            .filter(|(_, c)| c.tower_id.as_deref() == Some("staple_grain_tray"))
            .map(|(e, _)| e)
            .collect();
        assert_eq!(crops.len(), 8, "one wheat crop per tray unit");
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("seed_wheat_0"),
            0,
            "survival planting consumed one seed per unit"
        );

        // REPLANT while everything is alive: idempotent, nothing stacks.
        *data
            .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("staple_grain_tray".to_string(), "wheat".to_string(), 8));
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            8,
            "replant filled nothing (all units occupied)"
        );

        // GROW + HARVEST one unit: real grain lands in the pack.
        *data.get::<std::sync::Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        *data
            .get::<std::sync::Mutex<Option<u64>>>("harvest_request")
            .unwrap()
            .lock()
            .unwrap() = Some(crops[0].to_bits().into());
        sys.tick(&mut world, 1.0, &data);
        let grain = world.get::<&Inventory>(player).unwrap().count_item("grain_wheat_0");
        assert!(
            grain >= 8,
            "harvest yielded real wheat grain (>= yield_min 8), got {grain}"
        );
    }

    /// Creative mode: planting spawns a crop WITHOUT needing or consuming a seed,
    /// so the seed economy can be built out before it bites. (Survival mode, the
    /// absent-flag default, still consumes — proven by plant_grow_harvest_full_loop.)
    #[test]
    fn creative_mode_plants_without_consuming_seed() {
        let mut data = make_store();
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let mut sys = FarmingSystem::new();

        // Player holds NO seeds — creative mode plants anyway.
        let mut world = hecs::World::new();
        let _player = world.spawn((Inventory::new(16), Controllable));
        set_string(&data, "plant_request", "seed_tomato_0");
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            1,
            "creative mode planted a crop with no seed in inventory"
        );

        // And a held seed is NOT consumed in creative mode.
        let mut world2 = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("seed_tomato_0", 1, 99);
        let p2 = world2.spawn((inv, Controllable));
        set_string(&data, "plant_request", "seed_tomato_0");
        sys.tick(&mut world2, 1.0, &data);
        assert_eq!(
            world2.get::<&Inventory>(p2).unwrap().count_item("seed_tomato_0"),
            1,
            "creative mode did not consume the held seed"
        );
    }

    /// Survival mode: planting a tower consumes one seed per variety and skips
    /// varieties the player has no seed for.
    #[test]
    fn survival_tower_planting_consumes_seeds() {
        let data = make_store(); // no creative flag = survival = consume
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("seed_tomato_0", 1, 99); // has tomato, lacks lettuce
        let player = world.spawn((inv, Controllable));

        *data
            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("nutrition".to_string(), vec!["tomato".to_string(), "lettuce".to_string()]));
        sys.tick(&mut world, 1.0, &data);

        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            1,
            "only the seeded variety (tomato) was planted; lettuce skipped"
        );
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("seed_tomato_0"),
            0,
            "the tomato seed was consumed"
        );
    }

    /// Creative mode: planting a tower spawns every variety free, no seeds needed.
    #[test]
    fn creative_tower_planting_is_free() {
        let mut data = make_store();
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let _player = world.spawn((Inventory::new(16), Controllable)); // no seeds
        *data
            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
            .unwrap()
            .lock()
            .unwrap() = Some(("nutrition".to_string(), vec!["tomato".to_string(), "lettuce".to_string()]));
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            2,
            "creative planted both varieties free"
        );
    }

    /// Replanting a tower FILLS its fixed slots idempotently — it must NOT stack a
    /// fresh set of crops each time (the v0.410 fix for the 33 -> 66 -> 99 bug).
    #[test]
    fn tower_replant_fills_slots_idempotently() {
        let mut data = make_store();
        data.insert("creative_mode", std::sync::Mutex::new(true));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let _player = world.spawn((Inventory::new(16), Controllable));
        let set_req = |data: &DataStore| {
            *data
                .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
                .unwrap()
                .lock()
                .unwrap() = Some(("nutrition".to_string(), vec!["tomato".to_string(), "lettuce".to_string()]));
        };
        set_req(&data);
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(world.query::<&CropInstance>().iter().count(), 2, "first plant fills 2 slots");
        // Plant AGAIN: slots already occupied -> still 2 crops, not 4.
        set_req(&data);
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(
            world.query::<&CropInstance>().iter().count(),
            2,
            "replant is idempotent (no stacking)"
        );
        let slots: Vec<Option<u32>> =
            world.query::<&CropInstance>().iter().map(|(_, c)| c.tower_slot).collect();
        assert!(slots.contains(&Some(0)) && slots.contains(&Some(1)), "slots 0 and 1 recorded");
    }

    #[test]
    fn seed_and_harvest_id_mapping() {
        assert_eq!(plant_id_from_seed("seed_tomato_0").as_deref(), Some("tomato"));
        assert_eq!(plant_id_from_seed("seed_sweet_potato_0").as_deref(), Some("sweet_potato"));
        assert_eq!(plant_id_from_seed("iron_ore_0"), None);
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        assert_eq!(
            harvest_item_for("tomato", None, Some(&items)).as_deref(),
            Some("vegetable_tomato_0")
        );
        // A plant with no produce item in items.csv yields nothing (no crash).
        assert_eq!(harvest_item_for("void_orchid", None, Some(&items)), None);
    }

    /// v0.749 (ladder rung 6): EVERY shipped plant harvests into a REAL item.
    /// The harvest_item column (scripts/gen-harvest-items.js) covers herbs,
    /// legumes, mushrooms, fiber, and the apothecary garden — before this,
    /// 113 of 132 plants harvested to a warning log.
    #[test]
    fn every_shipped_plant_resolves_a_harvest_item() {
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
        let mut missing: Vec<String> = Vec::new();
        for id in plants.plants.keys() {
            if harvest_item_for(id, Some(&plants), Some(&items)).is_none() {
                missing.push(id.clone());
            }
        }
        missing.sort();
        assert!(
            missing.is_empty(),
            "{} plants harvest to nothing: {missing:?}",
            missing.len()
        );
    }

    // Fertilizing (fertilize_consumes_fertilizer_and_boosts_crop_health until
    // 2026-09-26) and the nutrient slider (per_area_nutrient_speeds_growth)
    // are tested in nutrient_tests.rs, against the N-P-K model that replaced
    // the +40 health and the growth multiplier those two tests pinned.

    /// Per-area irrigation: a crop whose grow area is configured with a water target
    /// (the garden edit modal's water slider) stays topped up and holds health, while
    /// an area whose slider is turned down to zero dehydrates and loses health. Proves
    /// the slider is wired through to the sim -- editing a grow area actually changes
    /// crop survival. (Until 2026-09-25 the dry case was an UN-configured area; that
    /// is now watered by default, see `unconfigured_grow_areas_are_watered_by_default`.)
    #[test]
    fn per_area_irrigation_keeps_configured_crops_watered() {
        use crate::ecs::components::CropInstance;
        let mut data = make_store();
        // Configure the "nutrition" tower for full irrigation (water slider = 1.0),
        // and turn the "apothecary" tower's slider right down.
        let mut irr = std::collections::HashMap::new();
        irr.insert("nutrition".to_string(), 1.0_f32);
        irr.insert("apothecary".to_string(), 0.0_f32);
        data.insert("garden_irrigation", std::sync::Mutex::new(irr));

        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,)); // the home's irrigation, running
        let dry = |tower: &str| CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 0.15, // below WATER_STRESS_THRESHOLD
            health: 80.0,
            tower_id: Some(tower.to_string()),
            tower_slot: Some(0),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        // Irrigated crop lives in the configured "nutrition" tower.
        let irrigated = world.spawn((dry("nutrition"),));
        // Parched crop lives in the tower whose water slider is turned down to zero.
        let parched = world.spawn((dry("apothecary"),));

        for _ in 0..5 {
            sys.tick(&mut world, 1.0, &data);
        }

        let irr_c = world.get::<&CropInstance>(irrigated).unwrap();
        let dry_c = world.get::<&CropInstance>(parched).unwrap();
        assert!(
            irr_c.water_level > 0.9,
            "irrigated crop stays topped up, got {}",
            irr_c.water_level
        );
        assert!(
            irr_c.health >= 80.0,
            "irrigated crop held/recovered health, got {}",
            irr_c.health
        );
        assert!(
            dry_c.water_level < irr_c.water_level,
            "un-irrigated crop is drier ({} vs {})",
            dry_c.water_level,
            irr_c.water_level
        );
        assert!(
            dry_c.health < irr_c.health,
            "un-irrigated crop lost health vs the irrigated one ({} vs {})",
            dry_c.health,
            irr_c.health
        );
    }

    #[test]
    fn numbered_fields_are_fields_and_towers_are_not() {
        assert!(is_field_area("grain_field"));
        assert!(is_field_area("grain_field_1"));
        assert!(is_field_area("legume_field_12"));
        assert!(!is_field_area("ntower_3"));
        assert!(!is_field_area("nutrition"));
        assert!(!is_field_area("oilseed_bed_4"));
    }

    /// The default (2026-09-25): a grow area nobody configured is watered by the
    /// home's automated irrigation, so a showcase garden does not die of thirst
    /// eight minutes into a session (the operator's save held 1,575 of 1,976 crops
    /// dead that way). A crop planted outside any grow area still dries out and
    /// needs watering by hand.
    #[test]
    fn unconfigured_grow_areas_are_watered_by_default() {
        use crate::ecs::components::CropInstance;
        let data = make_store();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,)); // the home's irrigation, running
        let crop = |tower: Option<&str>| CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: tower.map(str::to_string),
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let in_area = world.spawn((crop(Some("ntower_3")),));
        let by_hand = world.spawn((crop(None),));
        // Ten minutes: past the eight it used to take to die.
        for _ in 0..600 {
            sys.tick(&mut world, 1.0, &data);
        }
        let a = world.get::<&CropInstance>(in_area).unwrap();
        assert!(a.water_level >= DEFAULT_AUTO_IRRIGATION - 1e-4, "grow area watered by default, got {}", a.water_level);
        assert!(a.health >= 99.0, "and healthy, got {}", a.health);
        assert_ne!(a.growth_stage, STAGE_DEAD);
        let h = world.get::<&CropInstance>(by_hand).unwrap();
        assert!(
            h.water_level < 0.2 && h.health < 100.0,
            "a hand-planted crop still dries out ({} water, {} health)",
            h.water_level,
            h.health
        );
    }

    /// Real water (2026-09-25). With no irrigation machine running, a grow
    /// area is NOT watered (the machine is what waters it); with one running,
    /// the crops it waters publish their real daily need as litres per minute
    /// for the plumbing to draw; rain waters an outdoor field; hand watering
    /// queues litres and is refused when the tanks are empty.
    #[test]
    fn the_garden_water_is_real() {
        use crate::ecs::components::{CropInstance, Irrigator};
        use crate::systems::plumbing::WaterStatus;
        let crop = |tower: &str, water: f32| CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: water,
            health: 100.0,
            tower_id: Some(tower.to_string()),
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let demand = |d: &DataStore| *d.get::<std::sync::Mutex<f32>>("irrigation_demand_lpm").unwrap().lock().unwrap();
        let mut data = make_store();
        data.insert("irrigation_demand_lpm", std::sync::Mutex::new(0.0_f32));
        data.insert("hand_water_draw_l", std::sync::Mutex::new(0.0_f32));
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        let tomato_l_per_day = data
            .get::<PlantRegistry>("plant_registry")
            .unwrap()
            .get("tomato")
            .unwrap()
            .water_per_day;
        let mut sys = FarmingSystem::new();

        // No irrigation machine: the tower crop dries.
        let mut world = hecs::World::new();
        let c = world.spawn((crop("ntower_1", 0.5),));
        for _ in 0..60 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(world.get::<&CropInstance>(c).unwrap().water_level < 0.5, "nothing waters it");
        assert_eq!(demand(&data), 0.0, "and nothing is drawn");

        // Irrigation running: topped up, and its real need is published.
        world.spawn((Irrigator,));
        sys.tick(&mut world, 1.0, &data);
        assert!(world.get::<&CropInstance>(c).unwrap().water_level >= DEFAULT_AUTO_IRRIGATION - 1e-4);
        assert!((demand(&data) - tomato_l_per_day / 1440.0).abs() < 1e-6, "tomato's daily litres as L/min: {}", demand(&data));

        // Rain on an outdoor field, none on a tower.
        let mut rainy = make_store();
        rainy.insert(
            "weather",
            std::sync::Mutex::new(crate::systems::weather::Weather {
                condition: crate::systems::weather::WeatherCondition::Rain,
                intensity: 1.0,
                ..Default::default()
            }),
        );
        let mut w2 = hecs::World::new();
        let field = w2.spawn((crop("grain_field_1", 0.3),));
        let tower = w2.spawn((crop("ntower_2", 0.3),));
        for _ in 0..10 {
            sys.tick(&mut w2, 1.0, &rainy);
        }
        assert!(w2.get::<&CropInstance>(field).unwrap().water_level > 0.3, "rain wets the field");
        assert!(w2.get::<&CropInstance>(tower).unwrap().water_level < 0.3, "not the indoor tower");

        // Hand watering: litres queued; refused with a notice when the tanks are empty.
        let bits = world.query::<&CropInstance>().iter().next().unwrap().0.to_bits().get();
        *data.get::<std::sync::Mutex<Option<u64>>>("water_request").unwrap().lock().unwrap() = Some(bits);
        sys.tick(&mut world, 0.016, &data);
        assert_eq!(*data.get::<std::sync::Mutex<f32>>("hand_water_draw_l").unwrap().lock().unwrap(), HAND_WATER_L);
        data.insert("water_status", std::sync::Mutex::new(WaterStatus { stored_l: 0.0, capacity_l: 8000.0, ..Default::default() }));
        *data.get::<std::sync::Mutex<Option<u64>>>("water_request").unwrap().lock().unwrap() = Some(bits);
        sys.tick(&mut world, 0.016, &data);
        let notes = data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone();
        assert_eq!(notes.len(), 1, "told the tanks are empty: {notes:?}");
    }

    /// Water -> FOOD coupling (v0.611): the SAME configured irrigation that keeps a crop topped up with
    /// a full cistern FAILS when the cistern is dry, so the crop dehydrates + loses health. This is the
    /// downstream end of power -> water -> food (a power cut drains the cistern, then the garden wilts).
    #[test]
    fn dry_cistern_stops_irrigation_and_wilts_crops() {
        use crate::ecs::components::CropInstance;
        use crate::systems::plumbing::WaterStatus;

        let configured_crop = || CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 0.15, // below WATER_STRESS_THRESHOLD
            health: 80.0,
            tower_id: Some("nutrition".to_string()),
            tower_slot: Some(0),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };

        // Control: a FULL cistern -> irrigation works -> the crop stays topped up.
        let mut full = make_store();
        let mut irr = std::collections::HashMap::new();
        irr.insert("nutrition".to_string(), 1.0_f32);
        full.insert("garden_irrigation", std::sync::Mutex::new(irr.clone()));
        full.insert("water_status", std::sync::Mutex::new(WaterStatus { stored_l: 7000.0, capacity_l: 8000.0, ..Default::default() }));
        let mut sys = FarmingSystem::new();
        let mut w_full = hecs::World::new();
        w_full.spawn((crate::ecs::components::Irrigator,)); // the home's irrigation, running
        let c_full = w_full.spawn((configured_crop(),));
        for _ in 0..5 { sys.tick(&mut w_full, 1.0, &full); }
        let wet = w_full.get::<&CropInstance>(c_full).unwrap().water_level;
        assert!(wet > 0.9, "full cistern -> irrigation tops the crop up, got {wet}");

        // A DRY cistern (same capacity) -> irrigation can't deliver -> the crop dehydrates.
        let mut empty = make_store();
        empty.insert("garden_irrigation", std::sync::Mutex::new(irr));
        empty.insert("water_status", std::sync::Mutex::new(WaterStatus { stored_l: 0.0, capacity_l: 8000.0, ..Default::default() }));
        let mut w_dry = hecs::World::new();
        w_dry.spawn((crate::ecs::components::Irrigator,));
        let c_dry = w_dry.spawn((configured_crop(),));
        for _ in 0..5 { sys.tick(&mut w_dry, 1.0, &empty); }
        let dry = w_dry.get::<&CropInstance>(c_dry).unwrap();
        assert!(dry.water_level < 0.15, "dry cistern -> the crop dehydrates (not topped up), got {}", dry.water_level);
        assert!(dry.health < 80.0, "dry cistern -> the water-stressed crop loses health, got {}", dry.health);
    }

    /// The global crop growth multiplier (operator, 2026-09-20) reaches the sim,
    /// and it multiplies GROWTH rather than the clock: at the SAME elapsed game
    /// time and the same real growth_days, a 10x garden is further along than a
    /// 1x one. Written red first: without the multiplier applied, both gardens
    /// land on the same stage and the assert fails.
    #[test]
    fn crop_growth_speed_multiplies_growth_not_the_clock() {
        use crate::ecs::components::CropInstance;

        // 5% of the way through tomato's real window. At 1x that is stage 0; at
        // 10x it is half-grown. Deliberately a fraction where the two answers
        // cannot be the same stage, so the test cannot pass by accident.
        let elapsed_fraction = 0.05_f64;

        let run = |speed: f32| -> usize {
            let mut data = make_store();
            data.insert("crop_growth_speed", std::sync::Mutex::new(speed));
            let (growth_seconds, stages): (f64, Vec<&str>) = {
                let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
                let def = reg.get("tomato").unwrap();
                (def.growth_days as f64 * SECONDS_PER_DAY, def.stages())
            };
            {
                let gt = data
                    .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
                    .unwrap();
                gt.lock().unwrap().elapsed_seconds = growth_seconds * elapsed_fraction;
            }
            let mut sys = FarmingSystem::new();
            let mut world = hecs::World::new();
            let e = world.spawn((CropInstance {
                crop_def_id: "tomato".to_string(),
                growth_stage: stages[0].to_string(),
                planted_at: 0.0,
                water_level: 1.0,
                health: 100.0,
                tower_id: None,
                tower_slot: None,
                health_seconds: 0.0,
                growing_seconds: 0.0,
            },));
            sys.tick(&mut world, 1.0, &data);
            let c = world.get::<&CropInstance>(e).unwrap();
            stage_index(&c.growth_stage, &stages).unwrap()
        };

        let slow = run(1.0);
        let fast = run(10.0);
        assert!(
            fast > slow,
            "10x must outgrow 1x at the same game time (1x stage {slow}, 10x stage {fast})",
        );
        assert_eq!(slow, 0, "at 1x, 5% of the window is still the first stage");
    }

    /// Offline progression end to end (2026-09-25): a garden saved, then loaded
    /// after time away, must be further along when the toggle is on and exactly
    /// where it was when it is off. Goes through the real catch-up and the real
    /// FarmingSystem tick, so it proves the shifted planted_at actually turns
    /// into growth, not just that a number moved. Native-gated because
    /// save_load is (the relay build has no offline home).
    #[cfg(feature = "native")]
    #[test]
    fn time_away_grows_the_garden_only_with_offline_progression_on() {
        use crate::ecs::components::CropInstance;

        let run = |offline_on: bool| -> usize {
            let mut data = make_store();
            data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
            let (growth_seconds, stages): (f64, Vec<&str>) = {
                let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
                let def = reg.get("tomato").unwrap();
                (def.growth_days as f64 * SECONDS_PER_DAY, def.stages())
            };
            // Saved one minute after planting, at game second 1000.
            let mut save = crate::persistence::WorldSave::new_offline("t", "fibonacci");
            save.game_time = 1060.0;
            save.timestamp = 1_000_000;
            save.crops = vec![CropInstance {
                crop_def_id: "tomato".to_string(),
                growth_stage: stages[0].to_string(),
                planted_at: 1000.0,
                water_level: 1.0,
                health: 100.0,
                tower_id: None,
                tower_slot: None,
                health_seconds: 0.0,
                growing_seconds: 0.0,
            }];
            let mut world = hecs::World::new();
            crate::save_load::apply_save_to_world(&mut world, &save);
            // Away for 60% of the tomato's whole real growth window.
            let now = save.timestamp + (growth_seconds * 0.6) as u64;
            let r = crate::save_load::catch_up_world(&mut world, &save, offline_on, now);
            assert!((r.clock - 1060.0).abs() < 1e-9, "the clock resumes where it was saved");
            data.get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
                .unwrap()
                .lock()
                .unwrap()
                .set_elapsed(r.clock);
            let mut sys = FarmingSystem::new();
            sys.tick(&mut world, 1.0, &data);
            let (_e, c) = world.query_mut::<&CropInstance>().into_iter().next().unwrap();
            stage_index(&c.growth_stage, &stages).unwrap()
        };

        let off = run(false);
        let on = run(true);
        assert_eq!(off, 0, "toggle off: one minute of growth is still the first stage");
        assert!(on > off, "toggle on: 60% of the window away must grow the crop (on {on}, off {off})");
    }

    /// A multiplier arriving from a hand-edited config or the dev IPC is clamped
    /// rather than trusted. Zero would freeze the whole garden forever and read as
    /// broken; NaN would poison every crop's progress.
    #[test]
    fn growth_speed_is_clamped_from_any_source() {
        assert_eq!(clamp_growth_speed(0.0), MIN_CROP_GROWTH_SPEED);
        assert_eq!(clamp_growth_speed(-5.0), MIN_CROP_GROWTH_SPEED);
        assert_eq!(clamp_growth_speed(1.0e9), MAX_CROP_GROWTH_SPEED);
        assert_eq!(clamp_growth_speed(f32::NAN), DEFAULT_CROP_GROWTH_SPEED);
        // The offered presets must all survive the clamp untouched, or a radio
        // button in Settings would silently not be the value it claims.
        for preset in CROP_GROWTH_SPEED_PRESETS {
            assert_eq!(clamp_growth_speed(preset), preset, "preset {preset} clamped");
        }
    }
    /// RF -> FOOD coupling (v0.620): a POWERED WiFi router (RF emitter) harms a well-watered crop (RF
    /// stress outpaces recovery); with NO emitter the same crop holds/recovers. The operator's tradeoff.
    #[test]
    fn powered_rf_emitter_harms_crops() {
        use crate::ecs::components::{CropInstance, PowerConsumer, RfEmitter};
        let well_watered = || CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: "sprout".to_string(),
            planted_at: 0.0,
            water_level: 1.0, // not water-stressed, so we isolate RF
            health: 80.0,
            tower_id: None,
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let data = make_store();
        let mut sys = FarmingSystem::new();

        // A powered WiFi router (RF 0.6) bathes the grow -> the crop loses health.
        let mut world = hecs::World::new();
        let c = world.spawn((well_watered(),));
        world.spawn((RfEmitter { strength: 0.6, needs_power: true }, PowerConsumer { draw_watts: 8.0, priority: 4, enabled: true }));
        for _ in 0..5 { sys.tick(&mut world, 1.0, &data); }
        let harmed = world.get::<&CropInstance>(c).unwrap().health;
        assert!(harmed < 80.0, "powered RF harms the crop, got {harmed}");

        // No emitter -> the same well-watered crop holds or recovers.
        let mut world2 = hecs::World::new();
        let c2 = world2.spawn((well_watered(),));
        for _ in 0..5 { sys.tick(&mut world2, 1.0, &data); }
        let safe = world2.get::<&CropInstance>(c2).unwrap().health;
        assert!(safe >= 80.0, "no RF -> the crop holds/recovers, got {safe}");
    }

    /// Yield follows the crop's season health (2026-09-26): forty wheat
    /// crops kept well all season out-yield forty that spent it at half
    /// health, by about half, through the real harvest path. Every crop's
    /// CURRENT health is 100, the way a stressed crop's health has usually
    /// recovered by harvest day, so this also proves the harvest reads the
    /// season record and not today's health. Averaged over forty harvests
    /// each (wheat rolls 8 to 20), so the ratio sits near 0.5 with a spread
    /// far inside the bounds asserted: not flaky.
    #[test]
    fn healthy_crop_out_yields_a_stressed_one() {
        let mut data = make_store();
        // Creative: no seeds come back with the grain, so the count below is
        // produce only.
        data.insert("creative_mode", std::sync::Mutex::new(true));
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        let last_stage = data
            .get::<PlantRegistry>("plant_registry")
            .unwrap()
            .get("wheat")
            .unwrap()
            .last_stage()
            .to_string();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        // A pack big enough that no grain overflows (grain is ~0.93 L a unit).
        let mut inv = Inventory::new(64);
        inv.volume_capacity_l = 1.0e6;
        let player = world.spawn((inv, Controllable));
        let mature = |season: f64| CropInstance {
            crop_def_id: "wheat".to_string(),
            growth_stage: last_stage.clone(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: None,
            tower_slot: None,
            health_seconds: 1000.0 * season,
            growing_seconds: 1000.0,
        };
        let healthy: Vec<u64> =
            (0..40).map(|_| world.spawn((mature(1.0),)).to_bits().into()).collect();
        let stressed: Vec<u64> =
            (0..40).map(|_| world.spawn((mature(0.5),)).to_bits().into()).collect();
        let grain = |world: &hecs::World| {
            world.get::<&Inventory>(player).unwrap().count_item("grain_wheat_0")
        };
        let notices = |data: &DataStore| {
            std::mem::take(
                &mut *data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap(),
            )
        };

        *data.get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() =
            healthy;
        sys.tick(&mut world, 1.0, &data);
        let healthy_total = grain(&world);
        assert!(
            notices(&data).is_empty(),
            "a well-kept harvest posts no stress notice"
        );

        *data.get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() =
            stressed;
        sys.tick(&mut world, 1.0, &data);
        let stressed_total = grain(&world) - healthy_total;
        assert_eq!(world.query::<&CropInstance>().iter().count(), 0, "all eighty harvested");

        assert!(
            healthy_total >= 40 * 8,
            "full season health gives the full range (>= yield_min 8 each), got {healthy_total}"
        );
        assert!(
            stressed_total < healthy_total,
            "a stressed season yields less ({stressed_total} vs {healthy_total})"
        );
        let ratio = stressed_total as f32 / healthy_total as f32;
        assert!(
            (0.35..=0.65).contains(&ratio),
            "half the season health gives about half the harvest, got {ratio:.2} \
             ({stressed_total} vs {healthy_total})"
        );
        let said = notices(&data);
        assert_eq!(said.len(), 1, "one summary notice for the whole bulk harvest: {said:?}");
        assert!(
            said[0].contains("40 of these crops") && said[0].contains("50%"),
            "the notice says how many and how much: {}",
            said[0]
        );
    }

    /// The yield model on its own, seeded so it is exact run to run
    /// (2026-09-26): the average harvest is proportional to season health
    /// for a whole-unit crop and a fractional one alike (saffron rolls 0.3
    /// to 1.0, so probabilistic rounding carries the loss), and a crop at
    /// zero season health yields nothing at all.
    #[test]
    fn harvest_quantity_scales_the_average_with_season_health() {
        use rand::{Rng, SeedableRng};
        let mean = |ymin: f32, ymax: f32, health: f32| {
            let mut rng = rand::rngs::StdRng::seed_from_u64(0x5eed);
            let n = 20_000;
            let total: u32 = (0..n)
                .map(|_| harvest_quantity(ymin, ymax, health, rng.random(), rng.random()))
                .sum();
            total as f32 / n as f32
        };
        for (ymin, ymax) in [(8.0, 20.0), (0.3, 1.0)] {
            let full = mean(ymin, ymax, 1.0);
            let half = mean(ymin, ymax, 0.5);
            let expect = (ymin + ymax) / 2.0;
            assert!(
                (full - expect).abs() < expect * 0.03,
                "full health averages the plant's range ({ymin}..{ymax}): {full} vs {expect}"
            );
            assert!(
                (half / full - 0.5).abs() < 0.03,
                "half the season health averages half the harvest ({ymin}..{ymax}): {half} vs {full}"
            );
            assert_eq!(mean(ymin, ymax, 0.0), 0.0, "zero season health yields nothing");
        }
    }

    /// A drought the crop recovered from still counts (2026-09-26). Two wheat
    /// crops grow side by side; one goes thirty seconds without water, is
    /// watered by hand, and climbs back to full health. By the end both show
    /// health 100, but only the one that never went thirsty carries a full
    /// season record, which is what its harvest will read.
    #[test]
    fn season_health_remembers_a_drought_the_crop_recovered_from() {
        let data = make_store();
        let first_stage = data
            .get::<PlantRegistry>("plant_registry")
            .unwrap()
            .get("wheat")
            .unwrap()
            .first_stage()
            .to_string();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        // Hand-planted (no grow area, so no automated irrigation): only the
        // water level set here, and the hand watering below, reach them.
        let young = |water: f32| CropInstance {
            crop_def_id: "wheat".to_string(),
            growth_stage: first_stage.clone(),
            planted_at: 0.0,
            water_level: water,
            health: 100.0,
            tower_id: None,
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let kept = world.spawn((young(1.0),));
        let dry = world.spawn((young(0.0),));

        // Thirty seconds of drought for one of them.
        for _ in 0..30 {
            sys.tick(&mut world, 1.0, &data);
        }
        let low = world.get::<&CropInstance>(dry).unwrap().health;
        assert!(low < 75.0, "the thirsty crop lost health, got {low}");

        // Water it by hand, then let it recover.
        *data.get::<std::sync::Mutex<Option<u64>>>("water_request").unwrap().lock().unwrap() =
            Some(dry.to_bits().into());
        for _ in 0..80 {
            sys.tick(&mut world, 1.0, &data);
        }

        let k = (*world.get::<&CropInstance>(kept).unwrap()).clone();
        let d = (*world.get::<&CropInstance>(dry).unwrap()).clone();
        assert!(
            k.health >= 99.9 && d.health >= 99.9,
            "both crops look fully healthy today ({} and {})",
            k.health,
            d.health
        );
        assert_eq!(d.growth_stage, first_stage, "still growing, not harvested or dead");
        assert!(
            season_health(&k) > 0.999,
            "the crop that never went thirsty has a full season record, got {}",
            season_health(&k)
        );
        assert!(
            season_health(&d) < 0.95,
            "the drought stays in the record after the plant recovers, got {}",
            season_health(&d)
        );
    }

    // ── Light (2026-09-26, gardening depth rung 2) ──────────────────────

    /// Put the world clock at `hour` on game day `day`, through the same
    /// set_elapsed every clock writer uses, so the derived hour follows.
    fn set_clock(data: &DataStore, day: u32, hour: f64) {
        data.get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
            .unwrap()
            .lock()
            .unwrap()
            .set_elapsed(f64::from(day) * SECONDS_PER_DAY + hour / 24.0 * SECONDS_PER_DAY);
    }

    /// Advance the clock one second at a time the way TimeSystem does (the
    /// clock moves first, then the farming tick sees the new hour), `n` times.
    fn run_seconds(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, n: usize) {
        for _ in 0..n {
            {
                let slot = data
                    .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
                    .unwrap();
                let mut gt = slot.lock().unwrap();
                let next = gt.elapsed_seconds + 1.0;
                gt.set_elapsed(next);
            }
            sys.tick(world, 1.0, data);
        }
    }

    /// A crop's growth age right now: what its stage is read from.
    fn growth_age(world: &hecs::World, e: hecs::Entity, data: &DataStore) -> f64 {
        crate::systems::time::elapsed_now(data) - world.get::<&CropInstance>(e).unwrap().planted_at
    }

    /// A fresh, well-kept crop planted right now in `area`.
    fn fresh_crop(data: &DataStore, plant: &str, area: Option<&str>) -> CropInstance {
        let stage = data
            .get::<PlantRegistry>("plant_registry")
            .unwrap()
            .get(plant)
            .unwrap()
            .first_stage()
            .to_string();
        CropInstance {
            crop_def_id: plant.to_string(),
            growth_stage: stage,
            planted_at: crate::systems::time::elapsed_now(data),
            water_level: 1.0,
            health: 100.0,
            tower_id: area.map(str::to_string),
            tower_slot: Some(0),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }

    /// A grow light as the home spawns one: the marker plus its power role,
    /// switched on or shed, hung 2 m over the test tower (make_store).
    fn grow_light(
        powered: bool,
    ) -> (crate::ecs::components::GrowLight, crate::ecs::components::PowerConsumer, crate::ecs::components::Transform) {
        (
            crate::ecs::components::GrowLight,
            crate::ecs::components::PowerConsumer { draw_watts: 100.0, priority: 5, enabled: powered },
            crate::ecs::components::Transform {
                position: glam::Vec3::new(0.0, 2.0, 0.0),
                rotation: glam::Quat::IDENTITY,
                scale: glam::Vec3::ONE,
            },
        )
    }

    /// The sun the crops follow is the sun the solar panels follow, and it
    /// is up exactly DAYLIGHT_FRACTION of the day. If the sun's arc ever
    /// changes, this fails before every crop quietly starts growing faster or
    /// slower than its plants.csv days.
    #[test]
    fn daylight_fraction_is_the_share_of_the_day_the_sun_is_up() {
        let samples = 24_000;
        let lit = (0..samples)
            .filter(|i| {
                let hour = (*i as f32 + 0.5) * 24.0 / samples as f32;
                crate::systems::solar::sun_factor(hour) > 0.0
            })
            .count();
        let measured = lit as f64 / samples as f64;
        assert!(
            (measured - DAYLIGHT_FRACTION).abs() < 1e-3,
            "the sun is up {measured:.4} of the day, DAYLIGHT_FRACTION says {DAYLIGHT_FRACTION}"
        );
    }

    /// An outdoor field crop does not grow at night and does by day. At
    /// 100x growth so a day visibly moves it through its stages; its age
    /// shows the mechanism: frozen through the night, two seconds a second
    /// in the sun.
    #[test]
    fn an_outdoor_crop_grows_by_day_and_not_at_night() {
        let mut data = make_store();
        data.insert("crop_growth_speed", std::sync::Mutex::new(100.0_f32));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,)); // watered, so only light differs
        set_clock(&data, 0, 18.5);
        let field = world.spawn((fresh_crop(&data, "tomato", Some("grain_field_1")),));
        let first = world.get::<&CropInstance>(field).unwrap().growth_stage.clone();

        // 18:30 to 05:30: eleven hours of night.
        run_seconds(&mut sys, &mut world, &data, 550);
        assert!(
            growth_age(&world, field, &data).abs() < 1e-6,
            "no growth in the dark, age {}",
            growth_age(&world, field, &data)
        );
        assert_eq!(world.get::<&CropInstance>(field).unwrap().growth_stage, first, "same stage after a night");

        // On to 06:30, then the first full hour of sun.
        run_seconds(&mut sys, &mut world, &data, 50);
        let dawn = growth_age(&world, field, &data);
        run_seconds(&mut sys, &mut world, &data, 50);
        let gained = growth_age(&world, field, &data) - dawn;
        assert!((gained - 100.0).abs() < 1e-6, "an hour of sun (50 s) grows it 100 s, got {gained}");

        // The rest of the day: it moves on.
        run_seconds(&mut sys, &mut world, &data, 500);
        assert_ne!(
            world.get::<&CropInstance>(field).unwrap().growth_stage,
            first,
            "a day of sun moved the crop on a stage"
        );
    }

    /// Why lit time counts double: over a whole natural day, a crop under
    /// the sun gains exactly one day of growth, so plants.csv growth_days
    /// (calendar days, nights included) stay true. Holds for a field and
    /// for an indoor tower with no grow light (lit by the skylight); within
    /// a tick either side, since the sun is sampled once a tick and is down
    /// on the exact second of sunrise and of sunset.
    #[test]
    fn a_natural_day_grows_a_crop_exactly_one_day() {
        let mut data = make_store();
        data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,));
        set_clock(&data, 0, 0.0);
        let crops = [
            world.spawn((fresh_crop(&data, "tomato", Some("grain_field_1")),)),
            world.spawn((fresh_crop(&data, "tomato", Some("ntower_3")),)),
        ];
        run_seconds(&mut sys, &mut world, &data, SECONDS_PER_DAY as usize);
        for e in crops {
            let age = growth_age(&world, e, &data);
            assert!(
                (age - SECONDS_PER_DAY).abs() <= 2.0,
                "a day of sun and night is one day of growth, got {age} s of {SECONDS_PER_DAY}"
            );
        }
    }

    /// Indoors after dark, a crop grows only while a grow light is powered:
    /// none, or one the electrical sim has shed, leaves it paused until
    /// sunrise. By day the skylight lights it with no grow light at all, and
    /// a grow light never reaches an outdoor field.
    #[test]
    fn an_indoor_crop_grows_at_night_only_under_a_powered_grow_light() {
        let night_growth = |light: Option<bool>| -> (f64, f64) {
            let mut data = make_store();
            data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
            let mut sys = FarmingSystem::new();
            let mut world = hecs::World::new();
            world.spawn((crate::ecs::components::Irrigator,));
            if let Some(powered) = light {
                world.spawn(grow_light(powered));
            }
            set_clock(&data, 0, 19.0);
            let tower = world.spawn((fresh_crop(&data, "lettuce", Some("ntower_3")),));
            let field = world.spawn((fresh_crop(&data, "lettuce", Some("grain_field_1")),));
            run_seconds(&mut sys, &mut world, &data, 450); // 19:00 to 04:00
            (growth_age(&world, tower, &data), growth_age(&world, field, &data))
        };
        let (dark_tower, _) = night_growth(None);
        let (lit_tower, lit_field) = night_growth(Some(true));
        let (shed_tower, _) = night_growth(Some(false));
        assert!(dark_tower.abs() < 1e-6, "no grow light: paused at night, age {dark_tower}");
        assert!(shed_tower.abs() < 1e-6, "a shed grow light gives no light, age {shed_tower}");
        assert!((lit_tower - 900.0).abs() < 1e-6, "a powered grow light lights it, age {lit_tower}");
        assert!(lit_field.abs() < 1e-6, "a grow light does not reach an outdoor field, age {lit_field}");

        // By day, no grow light needed.
        let mut data = make_store();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,));
        set_clock(&data, 0, 9.0);
        let tower = world.spawn((fresh_crop(&data, "lettuce", Some("ntower_3")),));
        run_seconds(&mut sys, &mut world, &data, 100);
        let age = growth_age(&world, tower, &data);
        assert!((age - 200.0).abs() < 1e-6, "the skylight lights it by day, age {age}");
    }

    /// A grow light lights the tower under it, not the one across the room:
    /// with the same powered light, the test tower (make_store, under the
    /// light) grows through the night and a tower 6 m away stays paused
    /// (2026-09-26, lighting.rs). Seen red by giving every plot in the home
    /// the light (the first light rung's rule): the far tower then grew too.
    #[test]
    fn a_grow_light_lights_the_tower_under_it_not_the_one_across_the_room() {
        let mut data = make_store();
        data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
        data.insert(
            "grow_plots",
            vec![
                lighting::GrowPlot { id: "ntower_3".into(), cups: 12, ..Default::default() },
                lighting::GrowPlot { id: "ntower_9".into(), cups: 12, pos: [6.0, 0.0, 0.0], ..Default::default() },
            ],
        );
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,));
        world.spawn(grow_light(true));
        set_clock(&data, 0, 19.0);
        let near = world.spawn((fresh_crop(&data, "lettuce", Some("ntower_3")),));
        let far = world.spawn((fresh_crop(&data, "lettuce", Some("ntower_9")),));
        run_seconds(&mut sys, &mut world, &data, 450); // 19:00 to 04:00
        let (n, f) = (growth_age(&world, near, &data), growth_age(&world, far, &data));
        assert!((n - 900.0).abs() < 1e-6, "the tower under the light grew all night, age {n}");
        assert!(f.abs() < 1e-6, "the tower across the room did not, age {f}");
    }

    /// Darkness pauses a crop; it does not harm it. Two identical crops
    /// start a night at 80 health, one under a powered grow light and one in
    /// the dark: by morning both have recovered the same, neither lost any
    /// water to the dark, and both carry the same season health record, so
    /// the harvest will not punish the one that sat in the dark.
    #[test]
    fn darkness_pauses_a_crop_but_does_it_no_harm() {
        let night = |lit: bool| -> CropInstance {
            let mut data = make_store();
            data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
            let mut sys = FarmingSystem::new();
            let mut world = hecs::World::new();
            world.spawn((crate::ecs::components::Irrigator,));
            if lit {
                world.spawn(grow_light(true));
            }
            set_clock(&data, 0, 18.5);
            let mut c = fresh_crop(&data, "tomato", Some("ntower_3"));
            c.health = 80.0;
            let e = world.spawn((c,));
            run_seconds(&mut sys, &mut world, &data, 550);
            let c = world.get::<&CropInstance>(e).unwrap();
            (*c).clone()
        };
        let dark = night(false);
        let lit = night(true);
        assert_ne!(dark.growth_stage, STAGE_DEAD);
        assert_eq!(dark.health, lit.health, "the dark cost no health ({} vs {})", dark.health, lit.health);
        assert!(dark.health >= 99.9, "and it recovered overnight like any watered crop, got {}", dark.health);
        assert_eq!(dark.water_level, lit.water_level, "the dark changed nothing about water");
        assert!(
            (season_health(&dark) - season_health(&lit)).abs() < 1e-9,
            "the same season record ({} vs {})",
            season_health(&dark),
            season_health(&lit)
        );
    }

    /// Fungi grow without light (plants.csv `needs_light` false): an oyster
    /// mushroom in the dark mushroom rack grows at the calendar rate through
    /// the night, where a green crop beside it would be paused.
    #[test]
    fn mushrooms_grow_in_the_dark() {
        let mut data = make_store();
        data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((crate::ecs::components::Irrigator,));
        set_clock(&data, 0, 19.0);
        let mushroom = world.spawn((fresh_crop(&data, "oyster_mushroom", Some("mushroom_rack")),));
        let lettuce = world.spawn((fresh_crop(&data, "lettuce", Some("mushroom_rack")),));
        run_seconds(&mut sys, &mut world, &data, 450);
        let m = growth_age(&world, mushroom, &data);
        assert!((m - 450.0).abs() < 1e-6, "the mushroom grew through the night, age {m}");
        assert!(growth_age(&world, lettuce, &data).abs() < 1e-6, "the green crop beside it did not");
    }
}
