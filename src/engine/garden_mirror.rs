//! The Garden data the Inventory page shows: every crop with its card (N-P-K, light, pH, pollination,
//! picking, the air, weeds, the outlook), the pests and row covers per area, weeds, pollination
//! and the rooms' air (docs/design/sim-frame-cost.md, S1, 2026-10-30).
//!
//! lib.rs built it from the ECS world on EVERY world frame (about 15 small allocations, 6 to 8
//! formats and 20 lookups per crop, five views that each walk every crop: estimated 4 to 9 ms a
//! frame with the showcase's 1,981 crops), though its only reader is the Inventory page
//! (gui/pages/inventory.rs; every other use of `garden_pests` is the settings flags it also holds,
//! which this does not touch). Now it is built only on a frame that draws that page: the main
//! screen's open page (lib.rs, before the main UI runs) or an in-world screen showing it
//! (engine/screens.rs `frame_surfaces`, among the screens framed this tick), and at most once a
//! frame. The code below is lib.rs's, moved unchanged.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::engine::state::EngineState;
use crate::systems::inventory::ItemRegistry;

/// The main egui context's pass number when the Garden data was last built. The pass number goes
/// up once a frame, when the main UI runs; both callers run before it, so they see the same number
/// within a frame and a new one the next frame.
static BUILT_AT_PASS: AtomicU64 = AtomicU64::new(u64::MAX);

/// Build the Garden data unless it was built already this frame.
pub(crate) fn build_for_this_frame(state: &mut EngineState) {
    let pass = state.egui_ctx.cumulative_pass_nr();
    if BUILT_AT_PASS.swap(pass, Ordering::Relaxed) == pass {
        return;
    }
    build(state);
}

fn build(state: &mut EngineState) {
    // Its cost, on the Performance page's "Data for the open pages" row (budget_systems.ron).
    let _cost = crate::renderer::frame_costs::stage("cpu.page_data.garden");
    let plant_reg = state
        .data_store
        .get::<crate::systems::farming::PlantRegistry>("plant_registry");
    state.gui_state.crops.clear();
    // N-P-K for the crop card (2026-09-26, farming::soil).
    let item_reg = state.data_store.get::<ItemRegistry>("item_registry");
    let nutrient_scale = state
        .data_store
        .get::<crate::systems::farming::soil::NutrientData>("garden_nutrients")
        .zip(plant_reg)
        .and_then(|(nd, reg)| nd.scale_for(reg));
    let lit = crate::systems::farming::lighting::light_now(&state.game_world.world, &state.data_store);
    let light_data = state.data_store.get::<crate::systems::farming::lighting::LightingData>("garden_lighting");
    let ph_view = crate::systems::farming::soil_ph::GardenView::new(&state.data_store, &state.game_world.world);
    state.gui_state.garden_pests.ph_amendments = ph_view.amendments();
    let pollination = crate::systems::farming::pollination::GuiView::new(&state.game_world.world, &state.data_store);
    let picking = crate::systems::farming::picking::GuiView::new(&state.game_world.world, &state.data_store);
    let air_view = crate::systems::farming::humidity::GuiView::new(&state.game_world.world, &state.data_store); // humidity rows
    let weed_view = crate::systems::farming::weeds::GuiView::new(&state.game_world.world, &state.data_store); // weed rows
    // Each crop's wait and water, for the Garden card (first-hour F11, systems/farming/outlook.rs).
    let outlook_view = crate::systems::farming::outlook::OutlookView::new(&state.game_world.world, &state.data_store);
    state.gui_state.garden_pests.outlook.clear();
    for (entity, (crop, soil)) in state
        .game_world
        .world
        .query::<(&crate::ecs::components::CropInstance, Option<&crate::ecs::components::CropSoil>)>()
        .iter()
    {
        use crate::systems::farming::soil as sl;
        let plants = crate::systems::farming::units::crop_plants(crop, plant_reg, state.data_store.get(crate::systems::farming::units::PLOT_AREA_KEY));
        let need = crate::systems::farming::crop_season_need(&crop.crop_def_id, plants, plant_reg, item_reg, nutrient_scale);
        let store = soil.map_or_else(|| sl::fresh_store(need), |s| s.store);
        let (supply, scarce) = sl::sufficiency(&store, &need);
        let def = plant_reg.and_then(|r| r.get(&crop.crop_def_id));
        let per_kg = def.zip(nutrient_scale).map_or(crate::ecs::components::Npk::default(), |(d, s)| sl::removal_per_kg(d, s));
        let phc = ph_view.crop(crop, def); // soil pH for the crop card
        let name = def
            .map(|d| d.name.clone())
            .unwrap_or_else(|| crop.crop_def_id.clone());
        let stages: Vec<&str> = def.map(|d| d.stages()).unwrap_or_else(|| {
            crate::ecs::components::DEFAULT_GROWTH_STAGES
                .iter()
                .copied()
                .collect()
        });
        let dead =
            crop.growth_stage.as_str() == crate::ecs::components::STAGE_DEAD;
        let last = stages.last().copied().unwrap_or("");
        let mature = !dead && crop.growth_stage.as_str() == last;
        let progress = if dead {
            0.0
        } else {
            stages
                .iter()
                .position(|s| *s == crop.growth_stage.as_str())
                .map(|i| (i as f32 + 1.0) / stages.len().max(1) as f32)
                .unwrap_or(0.0)
        };
        state.gui_state.crops.push(crate::gui::GuiCrop {
            entity_bits: entity.to_bits().into(),
            name,
            stage: crop.growth_stage.clone(),
            progress,
            water: crop.water_level,
            health: crop.health,
            season_health: crate::systems::farming::season_health(crop),
            mature: picking.ready(entity, mature),
            dead,
            tower_id: crop.tower_id.clone(),
            tower_slot: crop.tower_slot,
            n: per_kg.n as f32,
            p: per_kg.p2o5 as f32,
            k: per_kg.k2o as f32,
            water_per_day: def.map(|d| d.water_per_day * plants as f32).unwrap_or(0.0),
            plants,
            temp_min: def.map(|d| d.temp_min_c).unwrap_or(0.0),
            temp_max: def.map(|d| d.temp_max_c).unwrap_or(0.0),
            light: crate::systems::farming::lighting::light_word(def.map_or(true, |d| d.needs_light), crop.tower_id.as_deref().map_or(false, crate::systems::farming::is_field_area), lit.sun_up, crop.tower_id.as_deref().and_then(|t| lit.cover.get(t)).copied().unwrap_or(0.0)),
            light_need: light_data.map(|d| crate::systems::farming::lighting::light_need_word(def, d)).unwrap_or_default(),
            soil: [store.n as f32, store.p2o5 as f32, store.k2o as f32],
            need: [need.n as f32, need.p2o5 as f32, need.k2o as f32],
            short_of: (supply < 1.0).then(|| scarce.word().to_string()),
            ph: phc.ph,
            ph_window: phc.window,
            ph_cap: phc.cap,
            ph_held: phc.held,
            pollination: pollination.row(entity),
            picking: picking.row(entity),
            humidity: air_view.crop_row(crop, def),
            weeds: weed_view.crop_row(crop, def),
        });
        state.gui_state.garden_pests.outlook.insert(entity.to_bits().into(), outlook_view.row(crop, def));
    }
    state.gui_state.garden_pests.weeds = weed_view.areas;
    state.gui_state.garden_pests.pollinate_areas = pollination.areas;
    state.gui_state.garden_pests.home_air = air_view.home; // ship life support: the home's air
    state.gui_state.garden_pests.air = air_view.areas;
    // Pests per grow area for the Garden panel (2026-09-26).
    if let Some(pd) = state.data_store.get::<crate::systems::farming::pests::PestData>("garden_pests") {
        let mut areas: Vec<String> =
            state.gui_state.crops.iter().map(|c| c.tower_id.clone().unwrap_or_default()).collect();
        areas.sort();
        areas.dedup();
        let world = &state.game_world.world;
        // Row covers per soil area (2026-09-27, farming::pests::cover_rows):
        // the cover gets its own row with Row cover / Take off buttons.
        state.gui_state.garden_pests.covers = crate::systems::farming::pests::cover_rows(world, &state.data_store, pd);
        state.gui_state.garden_pests.areas = areas
            .into_iter()
            .filter_map(|area| {
                let (pests, rel) = crate::systems::farming::pests::area_report(world, pd, &area);
                let pests: Vec<(String, f32, Vec<(String, String, String)>)> = pests
                    .into_iter()
                    .filter(|(_, lvl, _)| *lvl >= 0.01)
                    .map(|(p, lvl, _)| {
                        let controls = pd.controls_for(&p.id).into_iter().filter(|c| !c.is_cover()).map(|c| (c.id.clone(), c.name.clone(), c.note.clone())).collect();
                        (p.name.clone(), lvl as f32, controls)
                    })
                    .collect();
                let releases: Vec<(String, f32)> = rel.into_iter().map(|(c, d)| (c.name.clone(), d as f32)).collect();
                (!pests.is_empty() || !releases.is_empty()).then(|| crate::gui::GuiAreaPests { area, pests, releases })
            })
            .collect();
    }
}
