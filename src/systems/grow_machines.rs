//! Grow machines (2026-09-26): which machines grow food, what each one grows,
//! and how much food that is a day, computed from its crops instead of typed
//! into `data/machines/home.ron`.
//!
//! Until this rung every grow machine carried a hand-typed food stat such as
//! `(kind: "food", value: "+120 kcal/d")`, and once the farming model began
//! weighing real harvests the two disagreed: the potato bed's "+120" is about
//! twice its cited yield, the 5.76 m2 grain field's "+3100" about sixty times.
//! The typed figures are gone; the card, the construction editor, the garden
//! overview and the Home page all show the figure computed here.
//!
//! **A grow machine** is one a grow medium in `data/garden/grow_media.ron`
//! matches: the same test the engine uses when it publishes the plots the
//! farming system grows in (`engine::home_meshes::publish_grow_plots`).
//!
//! **What it grows** is what the shipped home actually plants in it, the
//! perpetual showcase (`data/world/showcase.ron`, sown by
//! `engine::ipc::auto_seed_showcase`), so the card agrees with the harvest the
//! player gets:
//! - a tower (`aeroponic_tower_<config>`) fills every cup of its config in
//!   `data/towers/aeroponic_configs.ron`, cycling the config's plantings
//!   (`tower_cup_crops`, the one rule both use);
//! - a bed, tray, rack or field sows the showcase's crop for its type in every
//!   plot, or, for a type the showcase does not name, the medium's
//!   `default_crop` (what its Plant button sows). The showcase comes first
//!   because it names a crop per machine TYPE, where the medium's default is
//!   one crop for a whole medium (every `_field` would be wheat, the legume
//!   field included).
//!
//! The showcase's per-INSTANCE tower override (`ntower_0`, the strawberry hero
//! tower for the landing-page screenshot) is not a design choice, so the
//! figures here are per machine type, and every screen that shows one agrees.
//!
//! A machine with no crop the model can count (the aquaponic tank: fish are
//! not in plants.csv) has no computed figure; its typed figure stays, marked
//! as an estimate, and a test holds every other grow machine to having none.
//!
//! **The figure** is `self_sufficiency::food_supply_kcal_per_day` over the
//! machine's plots: each plot's plants at the crop's plants.csv spacing x the
//! cited per-plant yield x the harvest item's items.csv mass, once every
//! `growth_days`, cropped back to back. One plot's floor is the footprint over
//! the medium's `plots` (the whole footprint per shelf when `stacked`), the
//! same split the engine publishes.
//!
//! Feature-neutral (serde, ron and the farming, inventory and self-sufficiency
//! data), so `MachineHome::load` fills it in under every feature set.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Deserialize;

use crate::machines::{MachineHome, MachineStat};
use crate::systems::farming::PlantRegistry;
use crate::systems::inventory::ItemRegistry;
use crate::systems::self_sufficiency::{crop_is_tabulated, food_supply_kcal_per_day, CropNutrition};

// ─────────────────────────────────────────────────────────────────────────────
// Grow media (moved here from gui/loaders.rs so this model can use them under
// every feature set; `crate::gui` re-exports them unchanged).
// ─────────────────────────────────────────────────────────────────────────────

/// One control in a grow medium's edit form (rendered top-to-bottom in the modal).
#[derive(Debug, Clone, Deserialize)]
pub enum GrowControl {
    /// A 0..1 slider stored under `key` (water / nutrient / humidity / ...).
    Slider { key: String, label: String },
    /// A free-text field for the primary crop / species / fish.
    Crop { label: String, hint: String },
    /// A checkbox stored under `key`.
    Toggle { key: String, label: String },
}

/// A grow MEDIUM: a way crops are grown (aeroponic, soil bed, field, ...), matched to a
/// garden machine by id, with the controls its edit modal shows. Data-driven from
/// `data/garden/grow_media.ron` so plot-types are added without code (infinite-of-X).
#[derive(Debug, Clone, Deserialize)]
pub struct GrowMedium {
    pub id: String,
    #[serde(default)]
    pub match_prefix: Option<String>,
    #[serde(default)]
    pub match_suffix: Option<String>,
    #[serde(default)]
    pub match_exact: Option<String>,
    pub label: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub show_slots: bool,
    /// Plant id (data/plants.csv) the bed/tray/field Plant button sows when the
    /// user hasn't typed a crop into the edit modal (v0.738 grain loop).
    #[serde(default)]
    pub default_crop: Option<String>,
    /// How many plots a machine of this medium is divided into, one crop in
    /// each (2026-09-26; 0 or absent is 1). A plot's floor is the machine's
    /// footprint over this; towers ignore it (a cup is one plant).
    #[serde(default)]
    pub plots: u32,
    /// The plots are shelves one above another (a mushroom rack), each with
    /// the whole footprint.
    #[serde(default)]
    pub stacked: bool,
    /// Every machine of this medium grows inside its own enclosure (a
    /// mushroom rack's fruiting tent, 2026-09-26): its own small air, which
    /// farming::humidity keeps as a room of its own inside the room the
    /// machine stands in. None = the machine grows in the room's air.
    #[serde(default)]
    pub enclosure: Option<Enclosure>,
    #[serde(default)]
    pub controls: Vec<GrowControl>,
}

/// A grow machine's enclosure (`GrowMedium::enclosure`): a fruiting tent
/// wrapped around a mushroom rack. Its box, centred on the machine, and the
/// substrate inside it, whose breath sets how much fresh air it must be given
/// (data/garden/humidity.ron, THE FRUITING TENT).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Enclosure {
    /// Width (x), height (y) and depth (z), metres.
    pub size: (f32, f32, f32),
    /// Kilograms of fruiting substrate inside it.
    pub substrate_kg: f32,
}

impl GrowMedium {
    /// Does this medium apply to the given machine id? (exact, then prefix, then suffix.)
    pub fn matches(&self, machine_id: &str) -> bool {
        self.match_exact.as_deref() == Some(machine_id)
            || self.match_prefix.as_deref().is_some_and(|p| machine_id.starts_with(p))
            || self.match_suffix.as_deref().is_some_and(|s| machine_id.ends_with(s))
    }
}

/// Load the grow-media registry (data/garden/grow_media.ron). Empty on absence/parse error.
pub fn load_grow_media(data_dir: &Path) -> Vec<GrowMedium> {
    #[derive(Deserialize)]
    struct File {
        media: Vec<GrowMedium>,
    }
    match crate::embedded_data::read_data_or_embedded(data_dir, "garden/grow_media.ron") {
        Some(t) => match ron::from_str::<File>(&t) {
            Ok(f) => f.media,
            Err(e) => {
                log::warn!("grow_media parse failed: {e}");
                Vec::new()
            }
        },
        None => Vec::new(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// What the shipped home plants (the perpetual showcase).
// ─────────────────────────────────────────────────────────────────────────────

/// data/world/showcase.ron (v0.863 perpetual showcase; moved here from
/// engine::ipc so the food model reads the same crops the showcase sows).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ShowcaseCfg {
    pub enabled: bool,
    /// Machine TYPE id -> the plants.csv crop sown in every plot of it.
    #[serde(default)]
    pub bed_crops: HashMap<String, String>,
    /// Tower INSTANCE id -> one crop for all its cups (the screenshot hero).
    #[serde(default)]
    pub tower_overrides: HashMap<String, String>,
}

/// Read data/world/showcase.ron. `None` when it is absent or does not parse.
pub fn load_showcase(data_dir: &Path) -> Option<ShowcaseCfg> {
    let text = std::fs::read_to_string(data_dir.join("world").join("showcase.ron")).ok()?;
    ron::from_str(&text).ok()
}

/// The crop in each of a tower's `cups` cups: `plants` cycled until every cup
/// is full, the way the showcase fills a tower. Empty when `plants` is.
pub fn tower_cup_crops(plants: &[String], cups: u32) -> Vec<String> {
    if plants.is_empty() {
        return Vec::new();
    }
    (0..cups as usize).map(|i| plants[i % plants.len()].clone()).collect()
}

/// The part of one tower config (data/towers/aeroponic_configs.ron) this model
/// needs: its id, its cups and the crops it plants. The GUI's `TowerConfig`
/// reads the same entries whole; serde skips the fields not named here.
#[derive(Debug, Clone, Deserialize)]
struct TowerCups {
    #[serde(default)]
    id: String,
    #[serde(default)]
    slots: u32,
    #[serde(default)]
    plantings: Vec<TowerCupPlanting>,
}

#[derive(Debug, Clone, Deserialize)]
struct TowerCupPlanting {
    #[serde(default)]
    plant: String,
}

fn load_tower_cups(data_dir: &Path) -> Vec<TowerCups> {
    #[derive(Deserialize)]
    struct File {
        #[serde(default)]
        towers: Vec<TowerCups>,
    }
    crate::embedded_data::read_data_or_embedded(data_dir, "towers/aeroponic_configs.ron")
        .and_then(|t| ron::from_str::<File>(&t).ok())
        .map(|f| f.towers)
        .unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// The computed figure.
// ─────────────────────────────────────────────────────────────────────────────

/// What one grow machine type grows: `food_supply_kcal_per_day` plots
/// (crop, one plot's floor in m2 or `None` for a single plant, how many), and
/// a short name for the card ("potato", "50 cups").
#[derive(Debug, Clone, PartialEq)]
pub struct GrowPlan {
    pub plots: Vec<(String, Option<f32>, f32)>,
    pub grows: String,
}

/// A grow machine type's computed food: kcal a day from its crops, and what it
/// grows. Held on `MachineHome::grown` (never saved) and shown as the food
/// line of every card that shows the machine (`MachineHome::stats_for`).
#[derive(Debug, Clone, PartialEq)]
pub struct GrownFood {
    pub kcal_per_day: f32,
    pub grows: String,
}

impl GrownFood {
    /// The food line as a machine stat.
    pub fn stat(&self) -> MachineStat {
        MachineStat {
            kind: "food".to_string(),
            value: food_value(self.kcal_per_day, &self.grows),
            status: if self.kcal_per_day > 0.0 { "ok" } else { "off" }.to_string(),
        }
    }
}

/// The card text for a computed food figure: "+62 kcal/d (potato)". Whole
/// kcal from 10 up, one decimal below, so a herb tower's 3.4 does not read 3.
pub fn food_value(kcal_per_day: f32, grows: &str) -> String {
    let n = if kcal_per_day >= 10.0 { format!("{kcal_per_day:.0}") } else { format!("{kcal_per_day:.1}") };
    format!("+{n} kcal/d ({})", grows.replace('_', " "))
}

/// Everything the figure is computed from, loaded once per home load.
pub struct GrowFoodModel {
    pub media: Vec<GrowMedium>,
    pub showcase: ShowcaseCfg,
    towers: Vec<TowerCups>,
    nutrition: CropNutrition,
    plants: PlantRegistry,
    items: ItemRegistry,
}

impl GrowFoodModel {
    /// Load from a data directory (the disk copy first, the embedded one for
    /// the files that have it). `None` when the crop nutrition table or either
    /// registry is missing: then no figure can be computed at all.
    pub fn load(data_dir: &Path) -> Option<Self> {
        let nutrition = CropNutrition::load(&data_dir.join("food").join("crop_nutrition.ron")).ok()?;
        let read = |rel: &str| crate::embedded_data::read_data_or_embedded(data_dir, rel);
        let plants = PlantRegistry::from_csv(read("plants.csv")?.as_bytes()).ok()?;
        let items = ItemRegistry::from_csv(read("items.csv")?.as_bytes()).ok()?;
        Some(Self {
            media: load_grow_media(data_dir),
            showcase: load_showcase(data_dir).unwrap_or_default(),
            towers: load_tower_cups(data_dir),
            nutrition,
            plants,
            items,
        })
    }

    /// What a machine of type `machine` with footprint `size` (the catalog's
    /// w, h, d in m) grows. `None` for a machine no grow medium matches, or a
    /// grow machine with no tower config and no crop named for it.
    pub fn plan(&self, machine: &str, size: (f32, f32, f32)) -> Option<GrowPlan> {
        let medium = self.media.iter().find(|m| m.matches(machine))?;
        let tower = machine.strip_prefix("aeroponic_tower_").and_then(|k| self.towers.iter().find(|t| t.id == k));
        if let Some(t) = tower {
            let plants: Vec<String> = t.plantings.iter().map(|p| p.plant.clone()).collect();
            let cups = tower_cup_crops(&plants, t.slots.max(1));
            if cups.is_empty() {
                return None;
            }
            let mut per_crop: BTreeMap<String, f32> = BTreeMap::new();
            for c in &cups {
                *per_crop.entry(c.clone()).or_insert(0.0) += 1.0;
            }
            let grows = format!("{} cups", cups.len());
            return Some(GrowPlan { plots: per_crop.into_iter().map(|(c, n)| (c, None, n)).collect(), grows });
        }
        let crop = self.showcase.bed_crops.get(machine).cloned().or_else(|| medium.default_crop.clone())?;
        let n = medium.plots.max(1);
        let footprint = (size.0 * size.2).max(0.0);
        let per_plot = if medium.stacked { footprint } else { footprint / n as f32 };
        Some(GrowPlan { plots: vec![(crop.clone(), Some(per_plot), n as f32)], grows: crop })
    }

    /// The machine type's computed food, or `None` when it is not a grow
    /// machine, grows nothing named, or grows only crops the model cannot
    /// count (no crop_nutrition entry, no plants.csv row or no growth days).
    pub fn grown(&self, machine: &str, size: (f32, f32, f32)) -> Option<GrownFood> {
        let plan = self.plan(machine, size)?;
        if !plan.plots.iter().any(|(c, _, _)| crop_is_tabulated(c, &self.nutrition, &self.plants)) {
            return None;
        }
        let kcal_per_day = food_supply_kcal_per_day(&plan.plots, &self.nutrition, &self.plants, &self.items);
        Some(GrownFood { kcal_per_day, grows: plan.grows })
    }
}

/// Every grow machine type in `home`'s catalog with a computed figure, keyed
/// by type id. Empty when the model's data cannot be loaded from `data_dir`.
pub fn grown_food(home: &MachineHome, data_dir: &Path) -> BTreeMap<String, GrownFood> {
    let Some(model) = GrowFoodModel::load(data_dir) else {
        return BTreeMap::new();
    };
    home.catalog
        .iter()
        .filter_map(|(id, def)| model.grown(id, def.size).map(|g| (id.clone(), g)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    fn home(file: &str) -> MachineHome {
        MachineHome::load(&data_dir().join("machines").join(file)).unwrap_or_else(|| panic!("{file} loads"))
    }

    /// No grow machine in either shipped home carries a typed food figure the
    /// model can compute: the figure on its card is the computed one, so a
    /// typed one would only be a second, disagreeing number in the data.
    ///
    /// Seen red by putting `(kind: "food", value: "+120 kcal/d", status:
    /// "ok")` back on home_solo.ron's potato_grow_bed: "home_solo.ron:
    /// potato_grow_bed carries a typed food figure (+120 kcal/d)".
    #[test]
    fn no_grow_machine_carries_a_typed_food_figure_the_model_computes() {
        let model = GrowFoodModel::load(&data_dir()).expect("the food model loads");
        let mut bad = Vec::new();
        let mut computed = 0;
        for file in ["home.ron", "home_solo.ron"] {
            let h = home(file);
            for (id, def) in &h.catalog {
                if model.grown(id, def.size).is_none() {
                    continue;
                }
                computed += 1;
                for s in def.stats.iter().filter(|s| s.kind == "food") {
                    bad.push(format!("{file}: {id} carries a typed food figure ({})", s.value));
                }
            }
        }
        // 10 computable grow types in home.ron, 8 in home_solo.ron (the fish tank is not).
        assert!(computed >= 18, "the model computes the shipped grow machines: {computed}");
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// A grow machine the model cannot compute keeps a typed figure, and says
    /// it is an estimate, so no card passes off a guess as a computed number.
    ///
    /// Seen red by typing the solo fish tank back as plain "+75 kcal/d":
    /// "home_solo.ron: aquaponic_tank's typed food '+75 kcal/d' is not marked
    /// an estimate".
    #[test]
    fn an_uncomputable_grow_machine_marks_its_figure_an_estimate() {
        let model = GrowFoodModel::load(&data_dir()).expect("the food model loads");
        for file in ["home.ron", "home_solo.ron"] {
            let h = home(file);
            for (id, def) in &h.catalog {
                if !model.media.iter().any(|m| m.matches(id)) || model.grown(id, def.size).is_some() {
                    continue;
                }
                for s in def.stats.iter().filter(|s| s.kind == "food") {
                    assert!(s.value.contains("estimate"), "{file}: {id}'s typed food '{}' is not marked an estimate", s.value);
                }
            }
        }
    }

    /// The potato bed's card shows the self-sufficiency model's own figure for
    /// it: two 1 m2 plots of potato (grow_media.ron's soil bed has two plots,
    /// the bed is 2 x 1 m), 8 plants, about 62 kcal a day, not the 120 that was
    /// typed.
    ///
    /// Seen red by giving each plot the whole footprint in `GrowFoodModel::plan`
    /// (dropping the `/ n`): the card then counted 16 plants, "home.ron: card
    /// 122.97414 vs model 61.48707".
    #[test]
    fn the_potato_bed_card_is_the_self_sufficiency_figure() {
        let nutrition = CropNutrition::load(&data_dir().join("food").join("crop_nutrition.ron")).unwrap();
        let plants = PlantRegistry::from_csv(&std::fs::read(data_dir().join("plants.csv")).unwrap()).unwrap();
        let items = ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap();
        let model_kcal = food_supply_kcal_per_day(&[("potato".to_string(), Some(1.0), 2.0)], &nutrition, &plants, &items);
        assert!((55.0..=70.0).contains(&model_kcal), "the model's potato bed: {model_kcal}");
        for file in ["home.ron", "home_solo.ron"] {
            let h = home(file);
            let grown = h.grown.get("potato_grow_bed").unwrap_or_else(|| panic!("{file}: the potato bed has a figure"));
            assert!((grown.kcal_per_day - model_kcal).abs() < 1e-3, "{file}: card {} vs model {model_kcal}", grown.kcal_per_day);
            let food: Vec<_> = h.stats_for("potato_grow_bed").into_iter().filter(|s| s.kind == "food").collect();
            assert_eq!(food.len(), 1, "{file}: one food line on the card");
            assert_eq!(food[0].value, food_value(model_kcal, "potato"), "{file}");
        }
    }

    /// The solo home's total food (the Home page's grown line and the garden
    /// overview's total) is its placed grow machines at their card figures,
    /// counted from the solo design's bill of machines (its Food loop names
    /// them): 9 variety towers, 1 apothecary, 8 potato beds, 3 oilseed beds,
    /// 2 grain trays, 2 mushroom racks, a grain field and a legume field. The
    /// fish tank's estimate is not in it.
    ///
    /// Seen red by summing `catalog` types once each in `grown_kcal_per_day`
    /// instead of every placed instance: "solo total 822.70154 vs the bill
    /// 2728.4565".
    #[test]
    fn the_solo_home_food_total_is_its_placed_machines_summed() {
        let h = home("home_solo.ron");
        let bill = [
            ("aeroponic_tower_nutrition", 9.0),
            ("aeroponic_tower_apothecary", 1.0),
            ("potato_grow_bed", 8.0),
            ("oilseed_bed", 3.0),
            ("staple_grain_tray", 2.0),
            ("mushroom_rack", 2.0),
            ("grain_field", 1.0),
            ("legume_field", 1.0),
        ];
        let want: f32 = bill.iter().map(|(id, n)| n * h.grown[*id].kcal_per_day).sum();
        assert!((h.grown_kcal_per_day() - want).abs() < 1e-2, "solo total {} vs the bill {want}", h.grown_kcal_per_day());
    }

    /// The food loop's authored `closes` agrees with what the computed food
    /// says, so the RON never claims a loop closes that the Home page shows
    /// short (or the reverse).
    ///
    /// Seen red by the same break as the solo total test (each type counted
    /// once): "home.ron: authored closes vs 1074 kcal/day grown".
    #[test]
    fn the_food_loops_authored_closure_matches_the_computed_food() {
        for file in ["home.ron", "home_solo.ron"] {
            let h = home(file);
            let food: Vec<_> = h.loops.iter().filter(|l| l.food_demand_kcal.is_some()).collect();
            assert_eq!(food.len(), 1, "{file} marks exactly one food loop");
            let grown = h.grown_kcal_per_day();
            assert_eq!(food[0].closes, food[0].closes_given(grown), "{file}: authored closes vs {grown:.0} kcal/day grown");
        }
    }

    /// A tower fills every cup by cycling its plantings (the showcase rule).
    #[test]
    fn a_tower_cycles_its_plantings_through_every_cup() {
        let plants = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(tower_cup_crops(&plants, 5), vec!["a", "b", "c", "a", "b"]);
        assert!(tower_cup_crops(&[], 5).is_empty());
    }
}
