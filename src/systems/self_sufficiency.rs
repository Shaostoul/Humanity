//! Self-sufficiency data layer (v0.663) -- the per-crop nutrition bridge + the editable
//! component-output / location tables that turn the homestead design from prose into a
//! computable score. Closes gaps #3 and #4 in `docs/design/homestead-solo-design.md`
//! section 7, implementing the "What data we'd add" section of
//! `docs/design/self-sufficiency.md`.
//!
//! This slice is **data + loaders + pure math** -- deliberately NOT UI. It is
//! feature-neutral (ron + serde + std, plus the farming and inventory registries, no
//! GUI/renderer/persistence imports), so it compiles under both `native` and `relay` with
//! no cfg gate. Every grow machine's food line, and the Home page's food total, come from
//! `food_supply_kcal_per_day` through `systems::grow_machines` (2026-09-26); the hand-typed
//! catalog strings they replaced are gone from data/machines/.
//!
//! Data files (all hot-reloadable, edited by hand or eventually the GUI):
//!   - `data/food/crop_nutrition.ron`            -- gap #3: per-crop calories/macros for
//!     every FOOD crop in `data/plants.csv`. What a harvest WEIGHS is not here (it carried
//!     its own `grams_per_yield_unit` until 2026-09-26, which disagreed with items.csv by up
//!     to 10x): it is plants.csv's per-plant yield x the harvest item's items.csv mass x the
//!     plants in the plot (`farming::units::plot_harvest_kg`), the same figure the harvest
//!     and the nutrient model use.
//!   - `data/self_sufficiency/component_outputs.ron` -- gap #4: per generation/collection/
//!     recycling machine, an output figure + assumptions.
//!   - `data/self_sufficiency/location.ron`      -- gap #4: the reference location the design
//!     is scored for (sun-hours, rainfall, degree-days).

use serde::{Deserialize, Serialize};
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Gap #3 -- per-crop nutrition bridge.
// ─────────────────────────────────────────────────────────────────────────────

/// One crop's nutrition. All macro fields are per 100 g of the harvest as its items.csv
/// item describes it (USDA magnitude). How much a plot harvests is not here: see
/// `food_supply_kcal_per_day`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropNutritionEntry {
    /// MUST match a row id in `data/plants.csv` (a typo would be a dead entry; a unit test
    /// cross-checks every id against the CSV).
    pub plant_id: String,
    pub calories_per_100g: f32,
    pub protein_g: f32,
    pub fat_g: f32,
    pub carbs_g: f32,
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

/// Computed food supply in kcal/day from grow plots cropped back to back. Each entry is
/// `(plant_id, plot floor area in m2, number of such plots)`; a `None` area is a single
/// plant (a tower cup). A plot harvests `farming::units::plot_harvest_kg` (the plants its
/// area holds at the crop's plants.csv spacing x the per-plant yield x the harvest item's
/// items.csv mass) once a season, so it supplies
/// `harvest kg x 10 x calories_per_100g / season days` kcal a day. A season is
/// `growth_days` for a crop harvested once, and `growth_days` plus its picking
/// window for a crop picked over weeks (`farming::picking::HarvestWindows::season_days`;
/// until 2026-09-26 every crop counted a whole season per `growth_days`).
///
/// This is the gap #3 bridge in action: the food loop is computed from crop data (the
/// hand-typed "+120 kcal/d" catalog strings are gone, `grow_machines`), on the same harvest figure the game
/// hands the player and the nutrient model bills the soil for. A crop with no
/// `crop_nutrition.ron` entry, no plants.csv row or no growth days contributes 0 (honest --
/// an un-tabulated crop cannot be counted, rather than guessed; see `crop_is_tabulated`).
pub fn food_supply_kcal_per_day(
    plots: &[(String, Option<f32>, f32)],
    nutrition: &CropNutrition,
    plants: &crate::systems::farming::PlantRegistry,
    items: &crate::systems::inventory::ItemRegistry,
) -> f32 {
    plots
        .iter()
        .map(|(id, area, count)| {
            let (Some(n), Some(def)) = (nutrition.get(id), plants.get(id)) else {
                return 0.0;
            };
            if !crop_is_tabulated(id, nutrition, plants) {
                return 0.0;
            }
            let kg = crate::systems::farming::units::plot_harvest_kg(id, *area, Some(plants), Some(items));
            // A crop picked over a season holds its plot for its growth days
            // AND its picking window, and gives one season's harvest in that
            // time (2026-09-26, farming::picking, data/garden/harvest_windows.ron):
            // a tower basil is one season per 30 + 266 days, not per 30.
            let season_days = crate::systems::farming::picking::HarvestWindows::shipped()
                .season_days(id, f64::from(def.growth_days));
            (kg * 10.0 * f64::from(n.calories_per_100g) / season_days) as f32 * count
        })
        .sum()
}

/// Can the food model count this crop at all: it has a `crop_nutrition.ron` entry, a
/// plants.csv row and a positive growth period. A crop that fails this adds 0 kcal, so a
/// machine growing only such crops has no computed figure (`grow_machines`).
pub fn crop_is_tabulated(
    plant_id: &str,
    nutrition: &CropNutrition,
    plants: &crate::systems::farming::PlantRegistry,
) -> bool {
    nutrition.get(plant_id).is_some() && plants.get(plant_id).is_some_and(|d| d.growth_days > 0.0)
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
            assert!(c.calories_per_100g >= 0.0, "{} has a sane calorie figure (>= 0)", c.plant_id);
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

    fn load_registries() -> (crate::systems::farming::PlantRegistry, crate::systems::inventory::ItemRegistry) {
        let plants = crate::systems::farming::PlantRegistry::from_csv(
            &std::fs::read(data_dir().join("plants.csv")).expect("data/plants.csv reads"),
        )
        .expect("plants.csv parses");
        let items = crate::systems::inventory::ItemRegistry::from_csv(
            &std::fs::read(data_dir().join("items.csv")).expect("data/items.csv reads"),
        )
        .expect("items.csv parses");
        (plants, items)
    }

    /// The food loop's potato bed, from cited data (2026-09-26). A 2 x 1 m
    /// `potato_grow_bed` (its home.ron footprint) holds floor(2.0 / 0.234) = 8
    /// plants at UMN's 11 x 33 in spacing, each giving 0.88 to 0.92 kg (NASS
    /// North Dakota's 335 to 350 cwt/acre), so a bed harvests about 7.2 kg every
    /// 90 growth days: 80 g a day, 61.5 kcal a day at 77 kcal/100 g. Held to 55
    /// to 70, the span of the two cited years with a little room.
    ///
    /// It replaced `potato_grams_calibration_matches_home_ron_kcal_claim`, which
    /// passed by modelling a bed as "~1.0 harvested tuber-unit/day" to land
    /// within 2x of home.ron's "+120 kcal/d" per bed. From the cited yields that
    /// claim is 2x high even cropping back to back all year (61.5 against 120),
    /// and data/home_outline.json's own cross-check, at two crops a year, puts a
    /// bed at 38. The finding is reported, not tuned away here. (Since 2026-09-26
    /// the typed "+120" is gone: the bed's card shows this figure, computed by
    /// `grow_machines`, and its test holds the two equal.)
    ///
    /// Seen red by making `units::plants_in_plot` return 1 (the bed then gave
    /// the calories of one plant, 7.7 kcal a day).
    #[test]
    fn potato_bed_calories_come_from_the_cited_yield() {
        let nutrition = load_nutrition();
        let (plants, items) = load_registries();
        let home = crate::machines::MachineHome::load(&data_dir().join("machines").join("home.ron"))
            .expect("data/machines/home.ron loads");
        let bed = home.catalog.get("potato_grow_bed").expect("potato_grow_bed is in the catalog");
        let area = bed.size.0 * bed.size.2;
        assert!((area - 2.0).abs() < 1e-6, "a potato bed is 2 m2: {area}");
        let one_bed = food_supply_kcal_per_day(&[("potato".to_string(), Some(area), 1.0)], &nutrition, &plants, &items);
        assert!((55.0..=70.0).contains(&one_bed), "a potato bed supplies {one_bed:.1} kcal/day");
        let eight = food_supply_kcal_per_day(&[("potato".to_string(), Some(area), 8.0)], &nutrition, &plants, &items);
        assert!((eight - 8.0 * one_bed).abs() < 1e-3, "plots add up: {eight} vs 8 x {one_bed}");
        // A tower cup of potato is one plant.
        let cup = food_supply_kcal_per_day(&[("potato".to_string(), None, 1.0)], &nutrition, &plants, &items);
        assert!((cup * 8.0 - one_bed).abs() < 1e-3, "one plant is an eighth of the bed: {cup} vs {one_bed}");
    }

    /// A harvest weighs what its items.csv item weighs, nothing else
    /// (2026-09-26: crop_nutrition.ron's own grams per yield unit is gone).
    /// A 2 m2 wheat tray's 666 plants at 0.001987 items of 0.5 kg each is
    /// 0.662 kg of grain per 120-day crop, 5.5 g a day, 18.8 kcal a day at
    /// 340 kcal/100 g; it was 8 to 20 items of 0.5 kg in the harvest and 8 to
    /// 20 units of 50 g in this model, a 10x disagreement inside one crop.
    /// Seen red by weighing the harvest at a flat 50 g an item (the old wheat
    /// grams_per_yield_unit): the tray then gave a tenth of the calories.
    #[test]
    fn a_harvest_weighs_what_its_item_weighs() {
        let nutrition = load_nutrition();
        let (plants, items) = load_registries();
        let tray = food_supply_kcal_per_day(&[("wheat".to_string(), Some(2.0), 1.0)], &nutrition, &plants, &items);
        let wheat = plants.get("wheat").unwrap();
        let per_plant_kg = f64::from(wheat.yield_min + wheat.yield_max) / 2.0 * f64::from(items.mass_for("grain_wheat_0"));
        let want = 666.0 * per_plant_kg * 10.0 * 340.0 / 120.0;
        assert!((f64::from(tray) - want).abs() < 1e-3, "wheat tray {tray} kcal/day vs {want}");
        assert!((18.0..=20.0).contains(&tray), "{tray}");
    }

    /// An un-tabulated crop id contributes 0 to the computed supply (honest, not a guess).
    #[test]
    fn unknown_crop_contributes_zero() {
        let nutrition = load_nutrition();
        let (plants, items) = load_registries();
        let kcal = food_supply_kcal_per_day(&[("not_a_real_crop".to_string(), Some(2.0), 5.0)], &nutrition, &plants, &items);
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
