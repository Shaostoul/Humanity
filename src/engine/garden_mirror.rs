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
//! frame. The code below is lib.rs's, moved, with the crop rows time-sliced since the same evening
//! (`SLICES`: measured, the build was 8.9 ms on a quiet machine, 5.9 of it the rows).

use std::sync::atomic::{AtomicU64, Ordering};

use crate::engine::state::EngineState;
use crate::systems::inventory::ItemRegistry;

/// The main egui context's pass number when the Garden data was last built. The pass number goes
/// up once a frame, when the main UI runs; both callers run before it, so they see the same number
/// within a frame and a new one the next frame.
static BUILT_AT_PASS: AtomicU64 = AtomicU64::new(u64::MAX);

/// How many frames a full refresh of the crop rows is spread over (time-slicing, 2026-10-30): each
/// frame that draws the page rebuilds the rows of every SLICES-th crop, in rotation, and keeps the
/// rest from the frames before. The rows were 5.9 ms of the 8.9 ms build with the showcase's 1,981
/// crops on a quiet machine; spread over 8 frames a row is at most 8 frames old (about 0.13 s at 60
/// fps), which nobody reading a page can see, and no single frame pays for all of them.
const SLICES: u64 = 8;

/// A page drawn again after this long (closed and reopened, or out of view) rebuilds every row at
/// once, so it never opens on rows from minutes ago.
const ROWS_FRESH_FOR: std::time::Duration = std::time::Duration::from_millis(500);

/// The rotation's state between builds (the main thread's).
#[derive(Clone, Copy, Default)]
struct Rotation {
    /// Builds so far: which slice is next.
    count: u64,
    /// When the rows were last built.
    last: Option<std::time::Instant>,
    /// The build count from which every row is again younger than a rotation: set when a wall
    /// screen came back into view and kept its old rows ([`plan`]).
    all_fresh_at: u64,
}

thread_local! {
    static SLICE: std::cell::Cell<Rotation> = const { std::cell::Cell::new(Rotation { count: 0, last: None, all_fresh_at: 0 }) };
}

/// Build the Garden data for the main screen's Inventory page, unless it was built already this
/// frame.
pub(crate) fn build_for_this_frame(state: &mut EngineState) {
    build_once(state, false);
}

/// Build the Garden data for a wall screen showing the Inventory page (engine/screens.rs), unless it
/// was built already this frame. Coming back into view after a while it does not rebuild every row
/// at once (about 10 ms with the showcase's crops, measured 2026-10-31): the screen's own picture is
/// older still, so its kept rows stand in until the rotation reaches them, the way its picture does
/// until its turn to be drawn.
pub(crate) fn build_for_wall_screen(state: &mut EngineState) {
    build_once(state, true);
}

fn build_once(state: &mut EngineState, wall: bool) {
    let pass = state.egui_ctx.cumulative_pass_nr();
    if BUILT_AT_PASS.swap(pass, Ordering::Relaxed) == pass {
        return;
    }
    build(state, wall);
}

/// Which rows this build rebuilds: (every row, the next rotation state). Pure, so the rule is tested.
///
/// - The Dev page's "turn off all reuse" switch off (`reuse` false): every row, every build.
/// - The main screen's page after a gap (`stale`), or while rows a wall screen kept are still in the
///   rotation: every row, so the page a person opens never shows rows from minutes ago.
/// - A wall screen after a gap: one slice, as always, and every row counts as fresh again only
///   after a full rotation (`all_fresh_at`), so a main-screen page opened meanwhile rebuilds them.
///   With no rows kept at all (`had_rows` false) every row is built anyway and fresh.
fn plan(rot: Rotation, now: std::time::Instant, reuse: bool, wall: bool, had_rows: bool) -> (bool, Rotation) {
    let stale = rot.last.map_or(true, |t| now.duration_since(t) > ROWS_FRESH_FOR);
    let every_row = !reuse || (!wall && (stale || rot.count < rot.all_fresh_at));
    let mut next = Rotation { count: rot.count.wrapping_add(1), last: Some(now), all_fresh_at: rot.all_fresh_at };
    if every_row || !had_rows {
        next.all_fresh_at = 0;
    } else if stale {
        next.all_fresh_at = rot.count.wrapping_add(SLICES);
    }
    (every_row, next)
}

fn build(state: &mut EngineState, wall: bool) {
    // Its cost, on the Performance page's "Data for the open pages" row (budget_systems.ron).
    let _cost = crate::frame_costs::stage("cpu.page_data.garden");
    // Its parts (cpu.garden.*, children of cpu.page_data.garden, never summed with it): each
    // assignment closes the part before it (2026-10-30, to find which part of the 16-20 ms is which).
    let mut _part = crate::frame_costs::stage("cpu.garden.setup");
    let plant_reg = state
        .data_store
        .get::<crate::systems::farming::PlantRegistry>("plant_registry");
    // N-P-K for the crop card (2026-09-26, farming::soil).
    let item_reg = state.data_store.get::<ItemRegistry>("item_registry");
    let nutrient_scale = state
        .data_store
        .get::<crate::systems::farming::soil::NutrientData>("garden_nutrients")
        .zip(plant_reg)
        .and_then(|(nd, reg)| nd.scale_for(reg));
    let lit = crate::systems::farming::lighting::light_now(&state.game_world.world, &state.data_store);
    let light_data = state.data_store.get::<crate::systems::farming::lighting::LightingData>("garden_lighting");
    _part = crate::frame_costs::stage("cpu.garden.ph_view");
    let ph_view = crate::systems::farming::soil_ph::GardenView::new(&state.data_store, &state.game_world.world);
    state.gui_state.garden_pests.ph_amendments = ph_view.amendments();
    _part = crate::frame_costs::stage("cpu.garden.pollination_view");
    let pollination = crate::systems::farming::pollination::GuiView::new(&state.game_world.world, &state.data_store);
    _part = crate::frame_costs::stage("cpu.garden.picking_view");
    let picking = crate::systems::farming::picking::GuiView::new(&state.game_world.world, &state.data_store);
    _part = crate::frame_costs::stage("cpu.garden.air_view");
    let air_view = crate::systems::farming::humidity::GuiView::new(&state.game_world.world, &state.data_store); // humidity rows
    _part = crate::frame_costs::stage("cpu.garden.weed_view");
    let weed_view = crate::systems::farming::weeds::GuiView::new(&state.game_world.world, &state.data_store); // weed rows
    // Each crop's wait and water, for the Garden card (first-hour F11, systems/farming/outlook.rs).
    _part = crate::frame_costs::stage("cpu.garden.outlook_view");
    let outlook_view = crate::systems::farming::outlook::OutlookView::new(&state.game_world.world, &state.data_store);
    _part = crate::frame_costs::stage("cpu.garden.crop_rows");
    // Time-sliced (SLICES): this frame rebuilds every SLICES-th crop's row and keeps the others, a new
    // crop's row is built at once and a gone crop's is dropped. Every row is rebuilt when the main
    // screen's page was not drawn just before (ROWS_FRESH_FOR) and while the Dev page's "turn off all
    // reuse" switch is off (crate::reuse): the kept rows are the one reuse here ([`plan`]).
    let now = std::time::Instant::now();
    let rot = SLICE.with(|c| c.get());
    let (every_row, next) = plan(rot, now, crate::reuse::enabled(), wall, !state.gui_state.crops.is_empty());
    SLICE.with(|c| c.set(next));
    let phase = rot.count % SLICES;
    let mut kept: std::collections::HashMap<u64, crate::gui::GuiCrop> =
        std::mem::take(&mut state.gui_state.crops).into_iter().map(|c| (c.entity_bits, c)).collect();
    let mut rows: Vec<crate::gui::GuiCrop> = Vec::with_capacity(kept.len());
    let mut present: std::collections::HashSet<u64> = std::collections::HashSet::with_capacity(kept.len());
    for (pos, (entity, (crop, soil))) in state
        .game_world
        .world
        .query::<(&crate::ecs::components::CropInstance, Option<&crate::ecs::components::CropSoil>)>()
        .iter()
        .enumerate()
    {
        let bits: u64 = entity.to_bits().into();
        present.insert(bits);
        if !every_row && pos as u64 % SLICES != phase {
            if let Some(row) = kept.remove(&bits) {
                rows.push(row); // its outlook entry is kept too
                continue;
            }
        }
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
        rows.push(crate::gui::GuiCrop {
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
    state.gui_state.crops = rows;
    state.gui_state.garden_pests.outlook.retain(|bits, _| present.contains(bits));
    _part = crate::frame_costs::stage("cpu.garden.areas");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// The rotation's rule (2026-10-31, the stutter survey: an Inventory wall screen coming back into
    /// view rebuilt every row in one frame, about 10 ms with the showcase's crops). A wall screen
    /// keeps rotating through its old rows, since its own picture is older still; the main screen's
    /// page never opens on old rows, also not while a wall screen's old rows are still rotating out;
    /// with reuse turned off every build rebuilds every row.
    ///
    /// Seen red 2026-10-31 against the rule of 2026-10-30 (a gap rebuilt every row from either
    /// caller): "a wall screen back in view rebuilds one slice, not every row".
    #[test]
    fn a_wall_screen_back_in_view_rebuilds_one_slice() {
        let t0 = Instant::now();
        let soon = t0 + Duration::from_millis(20);
        let later = t0 + Duration::from_secs(30);
        let built = Rotation { count: 40, last: Some(t0), all_fresh_at: 0 };

        // Drawn just before: one slice, from either caller.
        assert!(!plan(built, soon, true, false, true).0, "the main page drawn just before: one slice");
        assert!(!plan(built, soon, true, true, true).0, "a wall screen drawn just before: one slice");

        // After a gap: the main page rebuilds every row; a wall screen one slice, and its rows are
        // old until a full rotation has passed.
        assert!(plan(built, later, true, false, true).0, "the main page after a gap: every row");
        let (every, after_wall) = plan(built, later, true, true, true);
        assert!(!every, "a wall screen back in view rebuilds one slice, not every row");
        assert_eq!(after_wall.all_fresh_at, 40 + SLICES);

        // The main page opened during that rotation, though drawn just before: every row.
        let (every, after_main) = plan(after_wall, later + Duration::from_millis(20), true, false, true);
        assert!(every, "rows kept by a wall screen are not the main page's");
        assert_eq!(after_main.all_fresh_at, 0, "and after that every row is fresh");

        // The wall screen's rotation completed: the main page goes back to slices.
        let mut rot = after_wall;
        for i in 0..SLICES {
            rot = plan(rot, later + Duration::from_millis(20 * (i + 1)), true, true, true).1;
        }
        assert!(!plan(rot, later + Duration::from_millis(400), true, false, true).0, "a full rotation later: one slice");

        // No rows kept at all: every row is built anyway (nothing to keep), so they are fresh.
        assert_eq!(plan(built, later, true, true, false).1.all_fresh_at, 0);

        // Reuse turned off (the Dev page's switch): every row, every build, from either caller.
        assert!(plan(built, soon, false, true, true).0 && plan(built, soon, false, false, true).0);
    }
}
