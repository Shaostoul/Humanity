//! Self-sufficiency data layer (v0.663) -- the per-crop nutrition bridge + the editable
//! component-output / location tables that turn the homestead design from prose into a
//! computable score. Closes gaps #3 and #4 in `docs/design/homestead-solo-design.md`
//! section 7, implementing the "What data we'd add" section of
//! `docs/design/self-sufficiency.md`.
//!
//! This slice is **data + loaders + pure math** -- deliberately NOT UI. It is
//! feature-neutral (ron + serde + std only, no GUI/renderer/persistence imports), so it
//! compiles under both `native` and `relay` with no cfg gate. Wiring these numbers into
//! the Home-page loop summary (so the food loop is computed instead of trusting the
//! hand-typed catalog strings) is the next, deferred increment.
//!
//! Data files (all hot-reloadable, edited by hand or eventually the GUI):
//!   - `data/food/crop_nutrition.ron`            -- gap #3: per-crop calories/macros + a
//!     grams-per-yield-unit bridge for every FOOD crop in `data/plants.csv`.
//!   - `data/self_sufficiency/component_outputs.ron` -- gap #4: per generation/collection/
//!     recycling machine, an output figure + assumptions.
//!   - `data/self_sufficiency/location.ron`      -- gap #4: the reference location the design
//!     is scored for (sun-hours, rainfall, degree-days).

use serde::{Deserialize, Serialize};
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Gap #3 -- per-crop nutrition bridge.
// ─────────────────────────────────────────────────────────────────────────────

/// One crop's nutrition + yield-to-grams bridge. All macro fields are per 100 g of edible
/// portion (USDA magnitude); `grams_per_yield_unit` is the grams of edible harvest that ONE
/// `data/plants.csv` yield unit represents (the normalization that makes the CSV's abstract,
/// per-crop-inconsistent yield numbers computable). See `crop_nutrition.ron`'s header for the
/// per-class estimation basis and the potato calibration anchor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropNutritionEntry {
    /// MUST match a row id in `data/plants.csv` (a typo would be a dead entry; a unit test
    /// cross-checks every id against the CSV).
    pub plant_id: String,
    pub calories_per_100g: f32,
    pub protein_g: f32,
    pub fat_g: f32,
    pub carbs_g: f32,
    /// Grams of edible harvest per one `data/plants.csv` yield unit.
    pub grams_per_yield_unit: f32,
}

/// The whole crop-nutrition table (`data/food/crop_nutrition.ron`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropNutrition {
    pub crops: Vec<CropNutritionEntry>,
}

impl CropNutrition {
    /// Parse from a RON string. Returns the parse error as a `String` so callers stay
    /// renderer/log-framework agnostic.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str::<CropNutrition>(text).map_err(|e| e.to_string())
    }

    /// Load + parse from a `.ron` file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_ron(&text)
    }

    /// Look a crop up by its `plant_id`.
    pub fn get(&self, plant_id: &str) -> Option<&CropNutritionEntry> {
        self.crops.iter().find(|c| c.plant_id == plant_id)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Gap #4 -- editable component-output table + reference location.
// ─────────────────────────────────────────────────────────────────────────────

/// One generation/collection/recycling machine's output figure + its (editable) assumptions.
/// `unit` is heterogeneous across entries (kWh/day, L/day, kg/day, kW, ...): each per-loop
/// score sums only the entries carrying its own unit. See `component_outputs.ron`'s header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentOutput {
    /// A machine catalog id in `data/machines/home.ron` (a unit test enforces existence).
    pub id: String,
    pub output_value: f32,
    pub unit: String,
    pub assumptions: String,
}

/// The whole component-output table (`data/self_sufficiency/component_outputs.ron`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentOutputs {
    pub components: Vec<ComponentOutput>,
}

impl ComponentOutputs {
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str::<ComponentOutputs>(text).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_ron(&text)
    }

    pub fn get(&self, id: &str) -> Option<&ComponentOutput> {
        self.components.iter().find(|c| c.id == id)
    }
}

/// The reference location the homestead design is scored for
/// (`data/self_sufficiency/location.ron`). Self-sufficiency is gated by WHERE you are, so a
/// design is scored for a place + a household size, never in the abstract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    /// Peak-sun-hours/day, mid-summer.
    pub sun_hours_summer: f32,
    /// Peak-sun-hours/day, mid-winter (the worst-stretch value energy is sized on).
    pub sun_hours_winter: f32,
    pub annual_rainfall_mm: f32,
    /// Heating degree-days (base 18.3 C / 65 F).
    pub heating_degree_days: f32,
    /// Cooling degree-days (base 18.3 C / 65 F).
    pub cooling_degree_days: f32,
}

impl Location {
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str::<Location>(text).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_ron(&text)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure math (the computed sketch -- small + honest, no UI).
// ─────────────────────────────────────────────────────────────────────────────

/// Computed food supply in kcal/day from a list of `(plant_id, yield_units_per_day)`:
/// `sum over crops of units/day * grams_per_yield_unit * calories_per_100g / 100`.
///
/// This is the gap #3 bridge in action: the food loop is computed from crop data, not read
/// off the hand-typed "+120 kcal/d" catalog strings. A crop with no `crop_nutrition.ron`
/// entry contributes 0 (honest -- an un-tabulated crop cannot be counted, rather than guessed).
pub fn food_supply_kcal_per_day(counts: &[(String, f32)], nutrition: &CropNutrition) -> f32 {
    counts
        .iter()
        .map(|(id, units)| {
            nutrition
                .get(id)
                .map(|c| units * c.grams_per_yield_unit * c.calories_per_100g / 100.0)
                .unwrap_or(0.0)
        })
        .sum()
}

/// `(supply, demand)` daily household ENERGY balance in kWh/day.
///
/// `supply` sums every placed component whose `unit` is exactly `"kWh/day"` times its count
/// (so a runtime-gated backup rated in `"kW"`, or a water/air row in other units, is correctly
/// excluded from the passive-supply figure). `demand` is a **placeholder (0.0)**: the load side
/// lives on the CONSUMER machines, not this generation/collection table, and wiring it in is
/// the next slice. The tuple shape is returned now so that follow-up is a drop-in.
pub fn household_energy_balance(components: &[(String, u32)], outputs: &ComponentOutputs) -> (f32, f32) {
    let supply: f32 = components
        .iter()
        .map(|(id, n)| {
            outputs
                .get(id)
                .filter(|c| c.unit == "kWh/day")
                .map(|c| c.output_value * *n as f32)
                .unwrap_or(0.0)
        })
        .sum();
    let demand_placeholder = 0.0;
    (supply, demand_placeholder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    fn load_nutrition() -> CropNutrition {
        CropNutrition::load(&data_dir().join("food").join("crop_nutrition.ron"))
            .expect("data/food/crop_nutrition.ron parses")
    }

    fn load_outputs() -> ComponentOutputs {
        ComponentOutputs::load(&data_dir().join("self_sufficiency").join("component_outputs.ron"))
            .expect("data/self_sufficiency/component_outputs.ron parses")
    }

    /// All three shipped data files parse cleanly.
    #[test]
    fn all_three_files_parse() {
        let nutrition = load_nutrition();
        assert!(nutrition.crops.len() > 50, "expected the full food-crop table, got {}", nutrition.crops.len());
        let outputs = load_outputs();
        assert!(!outputs.components.is_empty(), "component_outputs must list machines");
        let loc = Location::load(&data_dir().join("self_sufficiency").join("location.ron"))
            .expect("data/self_sufficiency/location.ron parses");
        assert_eq!(loc.name, "Silverdale, WA");
        assert!(loc.sun_hours_summer > loc.sun_hours_winter, "PNW summer sun-hours exceed winter");
    }

    /// Every `crop_nutrition` plant_id is a REAL row in `data/plants.csv` (a typo = a dead
    /// entry). Reads the CSV's id column directly, cross-checking the SOURCE data rather
    /// than a derived registry. (Historical note: `PlantRegistry` used to silently drop
    /// fractional-yield rows like saffron's 0.3 because its yield fields were `u32`; fixed
    /// 2026-07-01 -- yields are f32 now and farming's zero-drop test guards the registry.)
    #[test]
    fn every_crop_nutrition_id_exists_in_plants_csv() {
        let text = std::fs::read_to_string(data_dir().join("plants.csv")).expect("data/plants.csv reads");
        // The set of id-column values from every non-comment, non-blank data row.
        let ids: std::collections::HashSet<&str> = text
            .lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| l.split(',').next())
            .collect();
        let nutrition = load_nutrition();
        for c in &nutrition.crops {
            assert!(
                ids.contains(c.plant_id.as_str()),
                "crop_nutrition plant_id '{}' has no data/plants.csv row (dead entry)",
                c.plant_id
            );
            assert!(
                c.calories_per_100g >= 0.0 && c.grams_per_yield_unit > 0.0,
                "{} has sane bridge numbers (cal>=0, grams/unit>0)",
                c.plant_id
            );
        }
    }

    /// Every `component_outputs` id is a machine in `data/machines/home.ron`'s catalog (parsed
    /// with the real loader), so no output figure floats free of an actual machine.
    #[test]
    fn every_component_output_id_exists_in_home_ron_catalog() {
        let home = crate::machines::MachineHome::load(&data_dir().join("machines").join("home.ron"))
            .expect("data/machines/home.ron loads");
        let outputs = load_outputs();
        for c in &outputs.components {
            assert!(
                home.catalog.contains_key(&c.id),
                "component_outputs id '{}' is not a machine in home.ron catalog",
                c.id
            );
        }
    }

    /// Calibration sanity check for `grams_per_yield_unit` (gap #3). home.ron's
    /// `potato_grow_bed` asserts "+120 kcal/d". Modeling a bed as ~1.0 harvested tuber-unit/day
    /// (a 2 m^2 intensive aeroponic bed at ~150 g/day, NASA/CIP magnitude), 8 beds through the
    /// crop-nutrition bridge = 8 * 150 g * 77 kcal/100g = 924 kcal/day, which must land within
    /// 2x of the 8 * 120 = 960 kcal claim. It does (0.96x), so no re-tuning of the potato
    /// grams_per_yield_unit was needed.
    #[test]
    fn potato_grams_calibration_matches_home_ron_kcal_claim() {
        let nutrition = load_nutrition();
        let counts = vec![("potato".to_string(), 8.0_f32)]; // 8 beds * ~1.0 tuber-unit/day
        let kcal = food_supply_kcal_per_day(&counts, &nutrition);
        let claim = 8.0 * 120.0; // home.ron potato_grow_bed: +120 kcal/d per bed
        assert!(
            kcal > claim * 0.5 && kcal < claim * 2.0,
            "8-bed potato supply {kcal:.0} kcal/day not within 2x of the home.ron claim {claim:.0}"
        );
    }

    /// An un-tabulated crop id contributes 0 to the computed supply (honest, not a guess).
    #[test]
    fn unknown_crop_contributes_zero() {
        let nutrition = load_nutrition();
        let kcal = food_supply_kcal_per_day(&[("not_a_real_crop".to_string(), 5.0)], &nutrition);
        assert_eq!(kcal, 0.0);
    }

    /// The energy-balance helper sums only the kWh/day generation rows. 4 solar panels at 1.44
    /// kWh/day each = 5.76 kWh/day (the solo design's supply figure), and the kW-rated backup
    /// generator does NOT inflate the passive supply.
    #[test]
    fn household_energy_supply_sums_kwh_per_day_components() {
        let outputs = load_outputs();
        let (supply, demand) = household_energy_balance(
            &[("solar_panel".to_string(), 4), ("generator_portable".to_string(), 1)],
            &outputs,
        );
        assert!((supply - 5.76).abs() < 0.5, "4 solar panels ~ 5.76 kWh/day, got {supply}");
        assert_eq!(demand, 0.0, "demand is a documented placeholder for now");
    }
}
