//! How many plants a grow unit holds, and so what it harvests (2026-09-26).
//!
//! A garden "unit" (one `CropInstance`) is one of three things:
//!
//! - a TOWER CUP (`tower_id` = the tower config or tower instance id,
//!   `tower_slot` = the cup): one plant;
//! - a hand-planted crop (`tower_id` None): one plant;
//! - a PLOT of a bed, tray or field machine: as many plants as its floor area
//!   fits at the crop's spacing.
//!
//! Until this rung plants.csv's per-plant yield was applied to every unit
//! alike, so a 2 m2 wheat tray harvested what one "plant" was listed as (7 kg,
//! several times what good wheat gives from 2 m2) and the nutrient model,
//! which multiplies the harvest, billed the unit's soil for all of it.
//!
//! The plot's floor area comes from the engine, which publishes it under
//! `PLOT_AREA_KEY`: every grow machine instance id and type id, mapped to the
//! floor area of ONE plot in it (the machine footprint, size.x x size.z in
//! data/machines/home.ron, over the plots it is sown in). A tag that is not in
//! the map (a tower, a hand-planted crop, a test area) is one plant. The
//! crop's spacing is plants.csv `area_per_plant_m2`, cited per crop in
//! data/garden/yields.ron; blank means one plant whatever the unit.
//!
//! Everything a unit does scales with its plants: the harvest roll, the season
//! nutrient need (and so the fresh unit's store and the feeder's target), the
//! legume credit, and the water the irrigation draws.

use std::collections::HashMap;

use crate::ecs::components::CropInstance;
use crate::systems::inventory::ItemRegistry;

use super::{PlantDef, PlantRegistry};

/// DataStore key of the plot areas the engine publishes:
/// `HashMap<String, f32>`, grow machine instance id and type id -> the floor
/// area of one plot in it, m2. Tower cups are not in it.
pub const PLOT_AREA_KEY: &str = "grow_plot_area_m2";

/// Plants one plot of `plot_area_m2` holds of this crop:
/// `max(1, floor(area / area_per_plant_m2))`. One when the plot area is
/// unknown (a tower cup, a hand-planted crop) or the crop has no spacing
/// sourced yet, which is the behaviour every crop had before 2026-09-26.
///
/// Worked in f64 with a whisker of tolerance, so a plot that is an exact
/// multiple of the spacing is not floored one plant short by f32 rounding
/// (0.5 / 0.1 is 4.9999... in binary).
pub fn plants_in_plot(def: &PlantDef, plot_area_m2: Option<f32>) -> u32 {
    let (Some(area), Some(each)) = (plot_area_m2, def.area_per_plant_m2) else {
        return 1;
    };
    let (area, each) = (f64::from(area), f64::from(each));
    if !(area.is_finite() && each.is_finite() && area > 0.0 && each > 0.0) {
        return 1;
    }
    let n = (area / each * (1.0 + 1e-6)).floor();
    // A plot of a square kilometre of flax would overflow nothing, but the
    // clamp keeps a corrupt area from wrapping the count.
    n.clamp(1.0, f64::from(u32::MAX)) as u32
}

/// The floor area of the plot a crop tagged `tag` grows in, m2, from the
/// engine's published map. `None` for a tower cup, a hand-planted crop, or a
/// tag the map does not know.
pub fn plot_area(plot_areas: Option<&HashMap<String, f32>>, tag: Option<&str>) -> Option<f32> {
    plot_areas.zip(tag).and_then(|(m, t)| m.get(t).copied())
}

/// Plants in the unit a crop grows in (see the module doc).
pub fn crop_plants(
    crop: &CropInstance,
    plants: Option<&PlantRegistry>,
    plot_areas: Option<&HashMap<String, f32>>,
) -> u32 {
    plants
        .and_then(|r| r.get(&crop.crop_def_id))
        .map_or(1, |def| plants_in_plot(def, plot_area(plot_areas, crop.tower_id.as_deref())))
}

/// What one harvested item of this plant weighs, kg: items.csv `weight_kg` of
/// the item it harvests into (`harvest_item_for`). Zero when either registry
/// is missing or the plant has no harvest item. The single source of truth
/// for what a harvest weighs: the harvest, the nutrient need and the food
/// model (`self_sufficiency`) all read it.
pub fn harvest_item_kg(plant_id: &str, plants: Option<&PlantRegistry>, items: Option<&ItemRegistry>) -> f64 {
    super::harvest_item_for(plant_id, plants, items)
        .map_or(0.0, |i| items.map_or(0.0, |r| f64::from(r.mass_for(&i))))
}

/// Expected kg one plot of `plot_area_m2` of this crop harvests at full
/// health: the plants it holds x the middle of the per-plant yield x the
/// harvest item's mass (`soil::expected_harvest_kg`). `None` for the plot
/// area means one plant (a tower cup). Zero for an unknown plant.
pub fn plot_harvest_kg(
    plant_id: &str,
    plot_area_m2: Option<f32>,
    plants: Option<&PlantRegistry>,
    items: Option<&ItemRegistry>,
) -> f64 {
    plants.and_then(|r| r.get(plant_id)).map_or(0.0, |def| {
        super::soil::expected_harvest_kg(def, harvest_item_kg(plant_id, plants, items), plants_in_plot(def, plot_area_m2))
    })
}
