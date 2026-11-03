//! From terrain/plant_mesh.rs's tests: the plant models against the simulation's plant list
//! (`PlantRegistry`).

use humanity_terrain::plant_mesh::*;

/// The three mushroom crops are drawn as fungi (2026-09-27), each is a
/// plant in data/plants.csv, none also has a plant recipe (which would
/// never be used), and the substrate unit agrees with the unit the farm
/// counts: a bed unit's area is the crop's `area_per_plant_m2` (a square
/// foot for button), and a block fits the half shelf a block is given.
#[test]
fn fungi_in_the_shipped_ron_match_the_crops_they_draw() {
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let text = std::fs::read_to_string(root.join("data/plants_visual.ron")).expect("plants_visual.ron");
    let reg = PlantVisualRegistry::from_ron(&text).expect("plants_visual.ron parses");
    let csv = std::fs::read(root.join("data/plants.csv")).expect("data/plants.csv exists");
    let plants = crate::farming::PlantRegistry::from_csv(&csv).expect("plants.csv parses");
    for id in ["oyster_mushroom", "shiitake", "button_mushroom"] {
        assert!(reg.fungus(id).is_some(), "{id} is not drawn as a fungus");
    }
    for (id, f) in &reg.fungi {
        let def = plants.get(id).unwrap_or_else(|| panic!("fungi names '{id}', not a plant in plants.csv"));
        assert!(reg.get(id).is_none(), "{id} has a plant recipe as well, which is never drawn");
        let each = def.area_per_plant_m2.unwrap_or_else(|| panic!("{id} has no area_per_plant_m2"));
        let fr = &f.fruit;
        assert!(fr.cap_diameter_m[0] > 0.0 && fr.cap_diameter_m[0] <= fr.cap_diameter_m[1], "{id}: cap sizes");
        assert!((0.0..1.0).contains(&fr.pin_at) && fr.per_unit > 0, "{id}: pins and caps");
        match &f.substrate {
            FungusSubstrate::Bed { unit_side_m, .. } => {
                let area = unit_side_m * unit_side_m;
                assert!((area / each - 1.0).abs() < 0.01, "{id}: a bed unit of {area} m2, the farm counts {each}");
            }
            FungusSubstrate::Block { size_m, colonised_at, .. } => {
                assert!(size_m[0] * size_m[2] <= each, "{id}: a block wider than the shelf it is given");
                assert!(*colonised_at < fr.pin_at, "{id}: pins before the block is colonised");
            }
        }
    }
}

/// Every stage-model line in the shipped `data/plants_visual.ron` names a
/// plant that exists in `data/plants.csv` and a model set whose four
/// stage files exist on disk, down to the buffer and palette each glTF
/// points at (2026-09-26). A typo on either side would not error at run
/// time: the loader treats a missing model as "this species has none"
/// and quietly draws the procedural stand-in, which is how six model
/// sets sat unused for two months.
#[test]
fn every_stage_model_mapping_names_a_real_plant_and_a_model_on_disk() {
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let text = std::fs::read_to_string(root.join("data/plants_visual.ron"))
        .expect("data/plants_visual.ron exists");
    let reg = PlantVisualRegistry::from_ron(&text).expect("plants_visual.ron parses");
    let csv = std::fs::read(root.join("data/plants.csv")).expect("data/plants.csv exists");
    let plants =
        crate::farming::PlantRegistry::from_csv(&csv).expect("plants.csv parses");
    assert!(!reg.stage_models.is_empty(), "the shipped file maps stage models");
    for (plant, set) in &reg.stage_models {
        assert!(
            plants.get(plant).is_some(),
            "stage_models names '{plant}', which is not a plant in data/plants.csv"
        );
        for q in 1..=4 {
            let dir = root.join(format!("assets/models/plants/{set}_{q}"));
            let gltf = dir.join(format!("{set}_{q}.gltf"));
            let body = std::fs::read_to_string(&gltf).unwrap_or_else(|_| {
                panic!("'{plant}' maps to set '{set}', but {} is missing", gltf.display())
            });
            let doc: serde_json::Value = serde_json::from_str(&body)
                .unwrap_or_else(|e| panic!("{} is not valid glTF JSON: {e}", gltf.display()));
            for list in ["buffers", "images"] {
                for entry in doc[list].as_array().into_iter().flatten() {
                    if let Some(uri) = entry["uri"].as_str() {
                        assert!(
                            dir.join(uri).is_file(),
                            "{} points at {uri}, which is missing",
                            gltf.display()
                        );
                    }
                }
            }
        }
    }
}
