//! Plants per grow unit (2026-09-26, units.rs), through the pure functions,
//! the real FarmingSystem tick and the shipped data. Each test was seen red
//! first; its doc comment says what was broken to see it.

use std::collections::HashMap;
use std::sync::Mutex;

use super::gardening_tests::make_store;
use super::soil;
use super::units::{self, plants_in_plot, plot_harvest_kg, PLOT_AREA_KEY};
use super::*;
use crate::ecs::components::{Controllable, CropInstance, CropSoil, Irrigator};
use crate::ecs::systems::System;
use crate::systems::inventory::{Inventory, ItemRegistry};

fn shipped() -> (PlantRegistry, ItemRegistry) {
    let plants = PlantRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/plants.csv")))
        .expect("plants.csv");
    let items =
        ItemRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/items.csv"))).expect("items.csv");
    (plants, items)
}

/// A crop of `plant` in grow area `area` (unit 0), at `stage`.
fn crop(plant: &str, area: Option<&str>, stage: &str) -> CropInstance {
    CropInstance {
        crop_def_id: plant.to_string(),
        growth_stage: stage.to_string(),
        planted_at: 0.0,
        water_level: 1.0,
        health: 100.0,
        tower_id: area.map(str::to_string),
        tower_slot: area.map(|_| 0),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    }
}

fn scale(plants: &PlantRegistry) -> Option<Npk> {
    soil::NutrientData::parse(soil::NUTRIENTS_RON).unwrap().scale_for(plants)
}

/// A plot holds floor(area / spacing) plants and never fewer than one. A 2
/// m2 tray of wheat at 0.003 m2 is 666; 0.5 m2 of plants at 0.1 m2 is exactly
/// 5 (in f32 the division is 4.9999..., which a bare floor makes 4); a plot
/// smaller than one plant's spacing, or an unknown plot, or a crop with no
/// spacing, is 1. Seen red by removing the tolerance in `plants_in_plot` (the
/// 0.5 m2 plot then held 4).
#[test]
fn plants_in_plot_fills_the_plot_and_never_less_than_one() {
    let (plants, _) = shipped();
    let wheat = plants.get("wheat").unwrap();
    assert_eq!(plants_in_plot(wheat, Some(2.0)), 666);
    assert_eq!(plants_in_plot(wheat, Some(5.76)), 1920, "a 2.4 x 2.4 m field");
    let mut tenth = wheat.clone();
    tenth.area_per_plant_m2 = Some(0.1);
    assert_eq!(plants_in_plot(&tenth, Some(0.5)), 5, "an exact multiple is not floored short");
    let tomato = plants.get("tomato").unwrap();
    assert_eq!(plants_in_plot(tomato, Some(0.1)), 1, "a plot smaller than one plant still holds one");
    assert_eq!(plants_in_plot(tomato, None), 1, "no plot area: one plant");
    assert_eq!(plants_in_plot(tomato, Some(f32::NAN)), 1, "a broken area: one plant");
}

/// THE CASE THIS RUNG EXISTS FOR, through the tick. A 2 x 1 m wheat tray
/// (a plot the engine publishes at 2.0 m2) holds 666 plants at the cited
/// 0.003 m2 (UMN's 30 to 32 plants a square foot), each giving 0.93 to 1.06 g
/// of grain (NASS 46.0 to 52.5 bu/acre), so a ripe tray harvests 1.32 items of
/// 0.5 kg on average (0.66 kg, 0.33 kg/m2) where it gave 8 to 20 items (7 kg)
/// before. Its season need is its harvest's removal: 20.8 g of N a kg x 0.66
/// kg = 13.8 g (it was 146 g), and a fresh tray's soil holds 1.5 seasons of
/// that. Four hundred trays are picked so the mean is tight (standard error
/// about 0.02 items). Seen red twice: harvesting one plant's yield per unit
/// (the trays gave 0.003 items each), and `crop_season_need` ignoring its
/// plants (the fresh tray held 1.5 x 0.02 g of N).
#[test]
fn a_two_square_metre_wheat_tray_yields_and_needs_what_its_plants_give() {
    const TRAYS: usize = 400;
    let mut data = make_store();
    data.insert("creative_mode", Mutex::new(true));
    data.insert("home_stock_outputs", Mutex::new(Vec::<(String, u32)>::new()));
    let mut areas: HashMap<String, f32> = (0..TRAYS).map(|i| (format!("graintray_{i}"), 2.0)).collect();
    areas.insert("graintray_young".to_string(), 2.0);
    data.insert(PLOT_AREA_KEY, areas);
    let (first, last) = {
        let w = data.get::<PlantRegistry>("plant_registry").unwrap().get("wheat").unwrap();
        (w.first_stage().to_string(), w.last_stage().to_string())
    };
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(64);
    inv.volume_capacity_l = 1.0e9;
    let player = world.spawn((inv, Controllable));

    // The need, and a fresh tray's soil, are the 666 plants'.
    let plants = data.get::<PlantRegistry>("plant_registry");
    let items = data.get::<ItemRegistry>("item_registry");
    let tray_plants = units::crop_plants(&crop("wheat", Some("graintray_young"), &first), plants, data.get(PLOT_AREA_KEY));
    assert_eq!(tray_plants, 666);
    let need = crop_season_need("wheat", tray_plants, plants, items, scale(plants.unwrap()));
    let kg = plot_harvest_kg("wheat", Some(2.0), plants, items);
    assert!((kg - 0.6617).abs() < 1e-3, "a tray's expected harvest {kg} kg");
    assert!((need.n - 20.8 * kg).abs() < 1e-3, "tray N need {} g", need.n);
    let young = world.spawn((crop("wheat", Some("graintray_young"), &first),));
    sys.tick(&mut world, 1.0, &data);
    let store = world.get::<&CropSoil>(young).expect("the tray got its soil").store;
    assert!((store.n / (soil::FRESH_UNIT_SEASONS * need.n) - 1.0).abs() < 1e-3, "fresh tray N {} vs 1.5 x {}", store.n, need.n);

    // Four hundred ripe trays, picked in one go.
    let ripe: Vec<u64> = (0..TRAYS)
        .map(|i| world.spawn((crop("wheat", Some(&format!("graintray_{i}")), &last),)).to_bits().into())
        .collect();
    *data.get::<Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() = ripe;
    sys.tick(&mut world, 1.0, &data);
    let in_pack = world.get::<&Inventory>(player).unwrap().count_item("grain_wheat_0");
    let in_barn: u32 = data
        .get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs")
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .filter(|(i, _)| i == "grain_wheat_0")
        .map(|(_, q)| q)
        .sum();
    let per_tray = f64::from(in_pack + in_barn) / TRAYS as f64;
    let wheat = data.get::<PlantRegistry>("plant_registry").unwrap().get("wheat").unwrap();
    let expected = 666.0 * f64::from(wheat.yield_min + wheat.yield_max) / 2.0;
    assert!((expected - 1.3234).abs() < 1e-3, "{expected}");
    assert!((per_tray - expected).abs() < 0.12, "a wheat tray gave {per_tray} items on average, expected {expected}");
}

/// A tower cup is one plant, whatever plot areas the engine publishes: a crop
/// tagged with a tower config id ("nutrition") or a tower instance id
/// ("ntower_3", "apoth_c1"), none of which are grow plots, and a hand-planted
/// crop, each hold one. A tomato cup therefore needs one plant's harvest's
/// removal (1.5 g N a kg x 6.35 kg), while the same tomato sown in a 2 m2
/// plot holds floor(2 / 0.418) = 4. Seen red by making `units::plot_area`
/// fall back to 1 m2 for an unknown tag (every cup then held 2 tomatoes).
#[test]
fn a_tower_cup_is_one_plant() {
    let (plants, items) = shipped();
    let areas: HashMap<String, f32> =
        [("graintray_1".to_string(), 2.0), ("staple_grain_tray".to_string(), 2.0)].into_iter().collect();
    for tag in [Some("nutrition"), Some("ntower_3"), Some("apoth_c1"), None] {
        let c = crop("tomato", tag, "ripe");
        assert_eq!(units::crop_plants(&c, Some(&plants), Some(&areas)), 1, "a tomato in {tag:?}");
    }
    let need = crop_season_need("tomato", 1, Some(&plants), Some(&items), scale(&plants));
    assert!((need.n - 1.5 * 6.350).abs() < 1e-2, "a tomato cup needs {} g of N", need.n);
    assert_eq!(units::crop_plants(&crop("tomato", Some("graintray_1"), "ripe"), Some(&plants), Some(&areas)), 4);
}

/// A crop whose area_per_plant_m2 is blank keeps the behaviour every crop
/// had before: one plant a unit, harvesting its plants.csv range, however
/// big the plot. Mint (planted as runners, no per-plant spacing sourced) in a
/// 5.76 m2 field is one plant harvesting 6 to 15 items of 0.05 kg. A cell the
/// loader cannot read is blank too, never a dropped row. Seen red by giving a
/// blank spacing a default of 0.1 m2 in `plants_in_plot` (the mint field then
/// held 57 plants).
#[test]
fn a_blank_spacing_keeps_one_plant_per_unit() {
    let (plants, items) = shipped();
    let mint = plants.get("mint").unwrap();
    assert_eq!(mint.area_per_plant_m2, None, "mint has no sourced spacing");
    assert_eq!(plants_in_plot(mint, Some(5.76)), 1);
    let kg = plot_harvest_kg("mint", Some(5.76), Some(&plants), Some(&items));
    assert!((kg - (6.0 + 15.0) / 2.0 * 0.05).abs() < 1e-6, "one mint plant's old range: {kg} kg");

    let csv = "id,name,yield_min,yield_max,area_per_plant_m2\na,A,1,2,\nb,B,1,2,x\nc,C,1,2,0.25\nd,D,1,2,-1\n";
    let reg = PlantRegistry::from_csv(csv.as_bytes()).expect("parses");
    assert_eq!(reg.plants.len(), 4, "no row dropped");
    assert_eq!(reg.get("a").unwrap().area_per_plant_m2, None);
    assert_eq!(reg.get("b").unwrap().area_per_plant_m2, None);
    assert_eq!(reg.get("c").unwrap().area_per_plant_m2, Some(0.25));
    assert_eq!(reg.get("d").unwrap().area_per_plant_m2, None);
}

/// The irrigation draw is per plant times plants. A potato bed plot the
/// engine publishes at 2.0 m2 holds 8 potatoes, and the irrigation it runs
/// draws 8 potatoes' water (plants.csv litres per plant a day, published as
/// litres a minute); a potato in a tower cup draws one's. Seen red by
/// dropping the plants from the irrigation sum (the bed drew one potato's
/// water).
#[test]
fn the_irrigation_draw_scales_with_plants() {
    let demand_for = |area: &str| -> (f32, f32) {
        let mut data = make_store();
        data.insert("irrigation_demand_lpm", Mutex::new(0.0_f32));
        data.insert(PLOT_AREA_KEY, [("potato_1".to_string(), 2.0_f32)].into_iter().collect::<HashMap<_, _>>());
        let (first, per_plant) = {
            let p = data.get::<PlantRegistry>("plant_registry").unwrap().get("potato").unwrap();
            (p.first_stage().to_string(), p.water_per_day)
        };
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        world.spawn((crop("potato", Some(area), &first),));
        sys.tick(&mut world, 1.0, &data);
        let drawn = *data.get::<Mutex<f32>>("irrigation_demand_lpm").unwrap().lock().unwrap();
        (drawn, per_plant)
    };
    let (bed, per_plant) = demand_for("potato_1");
    assert!(per_plant > 0.0);
    assert!((bed - 8.0 * per_plant / 1440.0).abs() < 1e-7, "a potato bed draws 8 potatoes' water: {bed} L/min");
    let (cup, _) = demand_for("ntower_1");
    assert!((cup - per_plant / 1440.0).abs() < 1e-7, "a potato cup draws one: {cup} L/min");
}

// -- The shipped data ------------------------------------------------------------

#[derive(serde::Deserialize)]
struct YieldFile {
    crops: Vec<YieldRecord>,
}

#[derive(serde::Deserialize)]
struct YieldRecord {
    plant: String,
    basis: Basis,
    per_plant_kg: (f64, f64),
}

#[derive(serde::Deserialize, Clone, Copy, Debug)]
enum Basis {
    DrySeed,
    FreshVeg,
    FreshFruit,
    FreshHerb,
    Spice,
    Fiber,
}

impl Basis {
    /// kg of harvest per m2 per planting the basis allows. Each spans the
    /// cited values in data/garden/yields.ron with about 2x room either side
    /// (the file's "band" section says which crop sets each end).
    fn band(self) -> (f64, f64) {
        match self {
            Basis::DrySeed => (0.05, 1.5),
            Basis::FreshVeg => (0.2, 15.0),
            Basis::FreshFruit => (0.5, 40.0),
            Basis::FreshHerb => (0.1, 20.0),
            Basis::Spice => (0.0005, 0.05),
            Basis::Fiber => (0.05, 0.5),
        }
    }
}

/// Crops the home plants that have no sourced spacing yet, each with its
/// reason in data/garden/yields.ron ("Not sourced yet"). A crop leaves this
/// list by gaining a row there.
const NOT_SOURCED_YET: [&str; 5] = ["mint", "rosemary", "aloe_vera", "st_johns_wort", "oyster_mushroom"];

/// The shipped spacing and yields, as data. For every plants.csv row with an
/// area_per_plant_m2:
///
/// - the cell is a positive number the loader read (a cell it cannot read
///   falls back to one plant, so a typo would hide), under 10 m2 a plant (the
///   widest shipped is a hop hill, 5.06);
/// - data/garden/yields.ron has its record, and every record has its row, so
///   no number is unsourced;
/// - yield_min/yield_max x the harvest item's items.csv weight_kg is the
///   record's per-plant kg (within 1%), so plants.csv and the cited kg cannot
///   drift apart, and neither can the item's weight;
/// - the harvest per m2 (per-plant kg / area) sits inside its basis's band.
///   A per-m2 figure typed as per-plant multiplies it by the plants per m2,
///   which throws every dense crop far out of its band.
///
/// And every crop the home plants (data/world/showcase.ron bed_crops and every
/// planting in data/towers/aeroponic_configs.ron) is sourced or named in
/// NOT_SOURCED_YET.
///
/// Seen red three ways: wheat's yield typed per m2 (0.66 and 0.71 items, 0.33
/// kg/m2 read as per plant: 110 kg/m2, outside 0.05 to 1.5, and off its
/// record), a yields.ron record deleted (lentil unsourced), and soybean's item
/// weight changed to 0.5 kg in items.csv (off its record by 67%).
#[test]
fn shipped_spacing_and_yields_are_sourced_and_physical() {
    let (plants, items) = shipped();
    let csv = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/plants.csv"));
    let ron_text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/garden/yields.ron"));
    let file: YieldFile = ron::from_str(ron_text).expect("data/garden/yields.ron parses");
    let records: HashMap<&str, &YieldRecord> = file.crops.iter().map(|r| (r.plant.as_str(), r)).collect();
    assert_eq!(records.len(), file.crops.len(), "one record a crop");

    let mut rows = csv.lines().filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty());
    let header: Vec<&str> = rows.next().expect("header").split(',').map(str::trim).collect();
    let col = header.iter().position(|h| *h == "area_per_plant_m2").expect("an area_per_plant_m2 column");

    let mut bad: Vec<String> = Vec::new();
    let mut filled: Vec<String> = Vec::new();
    for row in rows {
        let cells: Vec<&str> = row.split(',').map(str::trim).collect();
        let id = cells[0];
        let def = plants.get(id).unwrap_or_else(|| panic!("{id} is in plants.csv but not the registry"));
        let text = cells.get(col).copied().unwrap_or("");
        if text.is_empty() {
            if def.area_per_plant_m2.is_some() {
                bad.push(format!("{id}: blank in the file but loaded as {:?}", def.area_per_plant_m2));
            }
            if records.contains_key(id) {
                bad.push(format!("{id}: has a yields.ron record but no area_per_plant_m2"));
            }
            continue;
        }
        filled.push(id.to_string());
        let area = match text.parse::<f64>() {
            Ok(a) if a.is_finite() && a > 0.0 && a <= 10.0 => a,
            _ => {
                bad.push(format!("{id}: area_per_plant_m2 {text:?} is not a plant's area in m2"));
                continue;
            }
        };
        if def.area_per_plant_m2.map_or(true, |l| (f64::from(l) - area).abs() > 1e-6 * area) {
            bad.push(format!("{id}: file says {area}, loader read {:?}", def.area_per_plant_m2));
        }
        let Some(rec) = records.get(id) else {
            bad.push(format!("{id}: area_per_plant_m2 is filled but data/garden/yields.ron has no record"));
            continue;
        };
        let item_kg = units::harvest_item_kg(id, Some(&plants), Some(&items));
        let csv_kg = (f64::from(def.yield_min) * item_kg, f64::from(def.yield_max) * item_kg);
        let (lo, hi) = rec.per_plant_kg;
        if !(lo > 0.0 && hi >= lo) {
            bad.push(format!("{id}: per_plant_kg {:?} is not a range", rec.per_plant_kg));
        }
        for (name, got, want) in [("min", csv_kg.0, lo), ("max", csv_kg.1, hi)] {
            if (got - want).abs() > 0.01 * want {
                bad.push(format!("{id}: yield_{name} x {item_kg} kg = {got} kg, its record says {want}"));
            }
        }
        // The band reads what the game harvests (plants.csv), not the record,
        // so a slip in plants.csv fails it even before the record check.
        let (b_lo, b_hi) = rec.basis.band();
        for (name, kg) in [("min", csv_kg.0), ("max", csv_kg.1)] {
            let per_m2 = kg / area;
            if !(b_lo..=b_hi).contains(&per_m2) {
                bad.push(format!(
                    "{id}: {name} {kg} kg a plant over {area} m2 is {per_m2:.4} kg/m2, outside {:?} {b_lo} to {b_hi}",
                    rec.basis
                ));
            }
        }
    }
    for plant in records.keys() {
        if !filled.iter().any(|f| f == plant) && plants.get(plant).is_none() {
            bad.push(format!("yields.ron record {plant} is not a plant in plants.csv"));
        }
    }
    assert!(bad.is_empty(), "spacing and yields:\n{}", bad.join("\n"));
    assert!(filled.len() >= 50, "only {} crops carry a spacing", filled.len());

    // Every crop the home actually plants is sourced, or named as not yet.
    let showcase = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/world/showcase.ron"));
    let towers = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/towers/aeroponic_configs.ron"));
    let bed_block = showcase.split("bed_crops:").nth(1).and_then(|s| s.split('}').next()).expect("bed_crops");
    let quoted_second = |l: &str| l.split('"').nth(3).map(str::to_string);
    let mut home: Vec<String> = bed_block.lines().filter_map(quoted_second).collect();
    home.extend(towers.lines().filter_map(|l| l.split("(plant: \"").nth(1)?.split('"').next().map(str::to_string)));
    home.sort();
    home.dedup();
    assert!(home.len() > 50, "read {} home crops", home.len());
    let missing: Vec<&String> = home
        .iter()
        .filter(|p| !filled.contains(p) && !NOT_SOURCED_YET.contains(&p.as_str()))
        .collect();
    assert!(missing.is_empty(), "home crops with no sourced spacing and no reason: {missing:?}");
    for p in NOT_SOURCED_YET {
        assert!(!filled.iter().any(|f| f == p), "{p} is sourced now: take it off NOT_SOURCED_YET");
    }
}
