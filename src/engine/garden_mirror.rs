//! The Garden data the Inventory page shows: every crop with its card (N-P-K, light, pH, pollination,
//! picking, the air, weeds, the outlook), the pests and row covers per area, weeds, pollination
//! and the rooms' air (docs/design/sim-frame-cost.md, S1, 2026-10-30).
//!
//! lib.rs built it from the ECS world on EVERY world frame (about 15 small allocations, 6 to 8
//! formats and 20 lookups per crop, five views that each walk every crop: estimated 4 to 9 ms a
//! frame with the showcase's 1,981 crops), though its only reader is the Inventory page
//! (gui/pages/inventory.rs). Since 2026-10-30 it is built only on a frame that draws that page: the
//! main screen's open page (lib.rs, before the main UI runs) or an in-world screen showing it
//! (engine/screens.rs `frame_surfaces`), and at most once a frame.
//!
//! **BY AREA, KEPT UNTIL IT CHANGES (2026-10-31, sim-frame-cost.md, "The Garden views by area").**
//! The operator: "We want to avoid stuttering", and on the farm, "only calculate the value of the
//! viewed objects along its timeline". On the Inventory wall screen's turn the views alone were
//! about 4.5 ms (busy machine) with every crop's rows rebuilt in slices, and all of it grew with the
//! crops. Since farm-clock rung 2 a grow area's data changes only when something writes it, so each
//! area's rows (its crops' cards and tiles, and its pests, row cover, weeds and pollination lines)
//! are kept and built again only when one of the things they read moved:
//!
//! - **its own version** (systems/farming/versions.rs): the farm's body bumps it in every run that
//!   covers the area (its turn, the run after a command, a reader's settle) and bumps the epoch
//!   for every area at once (a whole-farm run, the commands, the clock's rebase, a load);
//! - **its crops**: the list of the area's crops changed (a sowing, a harvest, a load, the dev
//!   showcase): the area's rows are new rows;
//! - **the data and the settings** (the global key): the DataStore stamp of every entry the rows are
//!   worked out from (a hot reload; a machine edit, which publishes the grow plots again), the pH,
//!   pollination, picking and pest-severity settings, the clock's speed, the language, the soil
//!   memory itself (a load replaces it), the tanks holding water and the irrigation running;
//! - **what the area reads of the home now**: its irrigation (`WaterSupply::holds`), the
//!   pollinators reaching it (`pollination::GuiContext::area_key`);
//! - **the clock** where a row's words read it: "ripens in" and the tile's count step by whole days,
//!   hours or minutes and the weeds' row by the edges of the crop's critical period, so each row
//!   says until which game second its words hold (`outlook::words_hold_until`,
//!   `weeds::GuiView::row_holds_until`) and the area is built again when the clock leaves the
//!   window it was built in (also backwards).
//!
//! Two words of each row read what moves every game second for the whole home at once, so they
//! are kept apart: the light (the sun, the grow lights' cover) and the air (each room's humidity
//! and CO2 step on the air's own turn). Building every area's rows again whenever the air stepped
//! would put all of them on one frame each second, the very spike to avoid. Both are worked out
//! for each area every build, from per-area readings (cheap: no crop walk), and words are made only
//! for an area whose reading moved, once per species there, and written only into the rows whose
//! words changed. What the air lines and the home's air say, and the hives' lines (their colonies
//! age on the air's turn), are made every build: a line per area or per hive, never per crop.
//!
//! Each crop's slot tile key for the page's skipping of rows off screen (gui/widgets/offscreen.rs)
//! is worked out with its row (`GuiCrop::tile_key`), so the page no longer hashes every tile of
//! every open group every frame.
//!
//! The reuse rule (CLAUDE.md): everything kept here is tagged with the versions and stamps above,
//! and with the Dev page's "Turn off all reuse" (crate::reuse) every area is built again every
//! build, exactly as before. Without the farm's versions channel (a store nobody registered it in)
//! nothing is kept either.
//!
//! A wall screen's build rebuilds at most an eighth of the crops (`SLICES`, the oldest-built areas
//! first): a screen coming back into view after a while keeps its other rows until their turn, the
//! way its picture is older still until its own turn to be drawn (2026-10-31, the stutter survey:
//! that return rebuilt every row in one frame, about 10 ms). The main screen's page never shows a
//! kept row whose key moved: it builds every such area.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ecs::components::{CropInstance, CropSoil, SoilMemory};
use crate::engine::state::EngineState;
use crate::gui::widgets::offscreen::KeyHasher;
use crate::gui::{GardenPests, GuiAreaPests, GuiCrop};
use crate::hot_reload::data_store::DataStore;
use crate::systems::farming::{self, pests::CoverRow, pollination::AreaFlags, weeds::WeedAreaRow};
use crate::systems::inventory::ItemRegistry;

/// The main egui context's pass number when the Garden data was last built. The pass number goes
/// up once a frame, when the main UI runs; both callers run before it, so they see the same number
/// within a frame and a new one the next frame.
static BUILT_AT_PASS: AtomicU64 = AtomicU64::new(u64::MAX);

/// A wall screen's build rebuilds at most 1/SLICES of the crops' rows (and at least one area),
/// the areas built longest ago first; the rest keep theirs until a later build reaches them.
pub(crate) const SLICES: usize = 8;

/// The DataStore entries the rows are worked out from, kept while each one's change stamp
/// (`DataStore::stamp`) holds: a hot reload of any, or a machine edit (which publishes the grow
/// plots, their instances and their floor areas again), builds every area again. KEEP IN STEP with
/// what the rows read: an entry read below and missing here keeps rows from before it changed.
const STAMPED: [&str; 14] = [
    "plant_registry",
    "item_registry",
    "garden_nutrients",
    farming::units::PLOT_AREA_KEY,
    "garden_lighting",
    "garden_soil_ph",
    farming::pollination::DATA_KEY,
    farming::picking::DATA_KEY,
    farming::humidity::DATA_KEY,
    farming::weeds::DATA_KEY,
    "garden_pests",
    crate::systems::life_support::DATA_KEY,
    "grow_plots",
    "grow_instances",
];

/// What is kept between builds (the main thread's, `ROWS`; a test keeps its own).
#[derive(Default)]
pub(crate) struct Rows {
    /// The grow areas, in the order their rows stand in `GuiState::crops`: the areas with crops in
    /// the order the world first holds one of their crops, then the emptied beds that still show a
    /// weeds or row-cover line, by name.
    areas: Vec<Area>,
    /// `areas` by name, for the lists the page shows sorted by area.
    sorted: Vec<usize>,
    /// Builds so far: when each area was built, for the wall screen's oldest-first.
    builds: u64,
}

/// One grow area and what was built for it.
#[derive(Default)]
struct Area {
    /// Its crops' `tower_id`.
    tower: Option<String>,
    /// Its tag as the farm names it (`tower_id`, "" for none).
    name: String,
    /// Its crops, in the world's order: its rows, in `range`, are theirs one for one.
    crops: Vec<hecs::Entity>,
    range: Range<usize>,
    /// The key its rows were built under; 0: not built (new, or its crops changed).
    key: u64,
    /// The clock's span (game seconds, from inclusive, to exclusive) its rows' words hold for.
    valid: (f64, f64),
    /// The build that last built it.
    built_at: u64,
    /// It has a living crop, and those crops as (plant id, stage) for its row cover's line.
    living: bool,
    living_crops: Vec<(String, String)>,
    /// Its own lines.
    pests: Option<GuiAreaPests>,
    cover: Option<CoverRow>,
    weeds: Option<WeedAreaRow>,
    flags: Option<AreaFlags>,
    /// For each of its crops: the index of its species in `species`, and whether it needs light.
    kinds: Vec<(usize, bool)>,
    species: Vec<String>,
    /// The light's words for a crop that needs none and one that does, and the key they were made under.
    light: Option<(u64, [String; 2])>,
    /// The air's words for each species, and the key they were made under.
    air: Option<(u64, Vec<String>)>,
}

thread_local! {
    static ROWS: RefCell<Rows> = RefCell::new(Rows::default());
}

/// Build the Garden data for the main screen's Inventory page, unless it was built already this
/// frame.
pub(crate) fn build_for_this_frame(state: &mut EngineState) {
    build_once(state, false);
}

/// Build the Garden data for a wall screen showing the Inventory page (engine/screens.rs), unless it
/// was built already this frame; at most an eighth of the crops' rows are built again (`SLICES`).
pub(crate) fn build_for_wall_screen(state: &mut EngineState) {
    build_once(state, true);
}

fn build_once(state: &mut EngineState, wall: bool) {
    let pass = state.egui_ctx.cumulative_pass_nr();
    if BUILT_AT_PASS.swap(pass, Ordering::Relaxed) == pass {
        return;
    }
    // The farm advances each grow area on its own turn (2026-10-31, docs/design/farm-clock.md
    // rung 2): what the page shows is brought within a second of now first, and while the page
    // keeps being drawn the farm's turns keep it there. Inside a home-storage window of its own,
    // so what the feeder draws while it catches up leaves the containers (stock_piles::settle_farm,
    // the seam review's A).
    crate::engine::stock_piles::settle_farm(
        &mut state.game_world.world,
        &mut state.data_store,
        &mut state.gui_state.placed_items,
        &mut state.gui_state.home_stock,
        crate::systems::farming::clock::LOOK_FRESH_S,
    );
    let reuse = crate::reuse::enabled();
    ROWS.with(|r| {
        build(
            &state.game_world.world,
            &state.data_store,
            &mut state.gui_state.crops,
            &mut state.gui_state.garden_pests,
            &mut r.borrow_mut(),
            reuse,
            wall,
        )
    });
}

/// Which of the areas that need building a build builds (pure, so the rule is tested): every one
/// that MUST be built (never built, or its crops changed: there are no rows to keep) and, with a
/// `budget` of crops (a wall screen's build), the others oldest-built first while the budget lasts,
/// at least one; with none (the main screen's page), every one. Each entry is (area, its crops,
/// the build that last built it, must).
pub(crate) fn pick(dirty: &[(usize, usize, u64, bool)], budget: Option<usize>) -> Vec<usize> {
    let Some(budget) = budget else { return dirty.iter().map(|d| d.0).collect() };
    let mut out: Vec<usize> = dirty.iter().filter(|d| d.3).map(|d| d.0).collect();
    let mut spent: usize = dirty.iter().filter(|d| d.3).map(|d| d.1).sum();
    let mut rest: Vec<&(usize, usize, u64, bool)> = dirty.iter().filter(|d| !d.3).collect();
    rest.sort_by_key(|d| (d.2, d.0));
    for d in rest {
        if spent >= budget && !out.is_empty() {
            break;
        }
        out.push(d.0);
        spent += d.1;
    }
    out.sort_unstable();
    out
}

/// One build of the Garden data into the page's crops and lists, keeping each area's rows while
/// nothing they read has moved (see the module notes). `reuse` false (the Dev page's switch):
/// every area is built again. `wall`: a wall screen's build, with its budget (`SLICES`).
pub(crate) fn build(
    world: &hecs::World,
    data: &DataStore,
    out_crops: &mut Vec<GuiCrop>,
    out: &mut GardenPests,
    rows: &mut Rows,
    reuse: bool,
    wall: bool,
) {
    use crate::systems::farming::{humidity, lighting, outlook, picking, pollination, soil as sl, soil_ph, weeds};
    // Its cost, on the Performance page's "Data for the open pages" row (budget_systems.ron).
    let _cost = crate::frame_costs::stage("cpu.page_data.garden");
    // Its parts (cpu.garden.*, children of cpu.page_data.garden, never summed with it): each
    // assignment closes the part before it.
    let mut _part = crate::frame_costs::stage("cpu.garden.setup");
    rows.builds += 1;
    let builds = rows.builds;

    // ── Every crop's area, once over the crops: each area's crops in the world's order. ──
    let mut walk: Vec<(Option<String>, Vec<hecs::Entity>)> = Vec::new();
    {
        let mut q = world.query::<&CropInstance>();
        let mut named: HashMap<&str, usize> = HashMap::new();
        let mut none: Option<usize> = None;
        let mut last: Option<usize> = None;
        for (e, c) in q.iter() {
            let tid = c.tower_id.as_deref();
            let i = match last {
                // A crop next to one of its own area's (the usual order: an area is sown at once).
                Some(i) if walk[i].0.as_deref() == tid => i,
                _ => {
                    let found = match tid {
                        Some(t) => named.get(t).copied(),
                        None => none,
                    };
                    found.unwrap_or_else(|| {
                        let i = walk.len();
                        walk.push((tid.map(str::to_string), Vec::new()));
                        match tid {
                            Some(t) => {
                                named.insert(t, i);
                            }
                            None => none = Some(i),
                        }
                        i
                    })
                }
            };
            walk[i].1.push(e);
            last = Some(i);
        }
    }
    // The soil memory: the emptied beds' weeds and row covers, and each area's pests.
    let mut memory_q = world.query::<&SoilMemory>();
    let memory: Option<(hecs::Entity, &SoilMemory)> = memory_q.iter().next();
    let pest_data = data.get::<farming::pests::PestData>("garden_pests");
    {
        let have: HashSet<&str> = walk.iter().map(|(t, _)| t.as_deref().unwrap_or("")).collect();
        let mut extra: Vec<String> = Vec::new();
        if let Some((_, m)) = memory {
            for (a, st) in &m.weeds {
                if (st.level >= 0.01 || !st.mulch.is_empty()) && !have.contains(a.as_str()) {
                    extra.push(a.clone());
                }
            }
            if let Some(pd) = pest_data {
                for (a, st) in &m.pests {
                    if pd.controls.iter().any(|c| c.is_cover() && st.releases.contains_key(&c.id)) && !have.contains(a.as_str()) {
                        extra.push(a.clone());
                    }
                }
            }
        }
        extra.sort();
        extra.dedup();
        walk.extend(extra.into_iter().map(|a| (Some(a), Vec::new())));
    }

    // ── The layout: the same areas holding the same crops keep their rows where they stand. ──
    let total: usize = walk.iter().map(|(_, c)| c.len()).sum();
    let same_layout = out_crops.len() == total
        && rows.areas.len() == walk.len()
        && rows.areas.iter().zip(&walk).all(|(a, (t, c))| a.tower == *t && a.crops == *c);
    let relaid = !same_layout;
    if relaid {
        let mut old: HashMap<Option<String>, Area> = rows.areas.drain(..).map(|a| (a.tower.clone(), a)).collect();
        let mut old_rows = std::mem::take(out_crops);
        let mut new_rows: Vec<GuiCrop> = Vec::with_capacity(total);
        for (tower, crops) in walk {
            let start = new_rows.len();
            let kept = old.remove(&tower).filter(|o| o.crops == crops && o.range.end <= old_rows.len() && o.range.len() == crops.len());
            let area = match kept {
                Some(o) => {
                    new_rows.extend(old_rows[o.range.clone()].iter_mut().map(std::mem::take));
                    Area { range: start..new_rows.len(), ..o }
                }
                None => {
                    new_rows.resize_with(start + crops.len(), GuiCrop::default);
                    let name = tower.clone().unwrap_or_default();
                    Area { tower, name, crops, range: start..new_rows.len(), ..Default::default() }
                }
            };
            rows.areas.push(area);
        }
        *out_crops = new_rows;
        rows.sorted = (0..rows.areas.len()).collect();
        let areas = &rows.areas;
        rows.sorted.sort_by(|a, b| areas[*a].name.cmp(&areas[*b].name).then(a.cmp(b)));
        // Only the crops still there keep an outlook.
        let present: HashSet<u64> = rows.areas.iter().flat_map(|a| a.crops.iter().map(|e| e.to_bits().get())).collect();
        out.outlook.retain(|bits, _| present.contains(bits));
    }

    // ── What every area's rows read besides their own area (the global key). ──
    let versions = farming::versions::read(data, |v| (v.epoch, rows.areas.iter().map(|a| v.area(&a.name)).collect::<Vec<u64>>()));
    let reuse = reuse && versions.is_some();
    let (epoch, area_versions) = versions.unwrap_or_default();
    let (elapsed, time_scale) = outlook::clock(data);
    let supply = outlook::WaterSupply::now(world, data);
    let severity = data
        .get::<std::sync::Mutex<f32>>("garden_pest_severity")
        .and_then(|m| m.lock().ok().map(|v| *v))
        .unwrap_or(farming::pests::DEFAULT_PEST_SEVERITY);
    let global = {
        let mut h = KeyHasher::default();
        h.write_u64(epoch);
        for key in STAMPED {
            h.write_u64(data.stamp(key));
        }
        h.write_u8(soil_ph::is_on(data) as u8);
        h.write_u8(pollination::mode_on(data) as u8);
        h.write_u8(picking::realistic(data) as u8);
        h.write_u32(severity.to_bits());
        h.write_u32(time_scale.to_bits());
        h.write(crate::i18n::language().as_bytes());
        h.write_u64(memory.map_or(0, |(e, _)| e.to_bits().get()));
        h.write_u8(supply.water_available as u8);
        h.write_u8(supply.irrigation_on as u8);
        h.finish()
    };
    let pol_ctx = pollination::GuiContext::now(world, data);

    // ── Which areas need building. ──
    let mut dirty: Vec<(usize, usize, u64, bool)> = Vec::new();
    let mut keys: Vec<u64> = Vec::with_capacity(rows.areas.len());
    for (i, a) in rows.areas.iter().enumerate() {
        let mut h = KeyHasher::default();
        h.write_u64(global);
        h.write_u64(area_versions.get(i).copied().unwrap_or(0));
        h.write_u32(supply.holds(a.tower.as_deref()).map_or(u32::MAX, f32::to_bits));
        if let Some(ctx) = &pol_ctx {
            ctx.area_key(&a.name, &mut h);
        }
        let key = h.finish().max(1);
        keys.push(key);
        // Its rows hold while its key does and the clock is still where they were built (they are
        // exact there by construction: a crop's words can stand on a step of their count, true now
        // and wrong the moment the clock moves) or inside the span its words hold for.
        let held = a.key == key && (elapsed == a.valid.0 || (a.valid.0 <= elapsed && elapsed < a.valid.1));
        if !reuse || !held {
            dirty.push((i, a.crops.len(), a.built_at, a.key == 0 || !reuse));
        }
    }
    let budget = (wall && reuse).then(|| total.div_ceil(SLICES).max(1));
    let todo = pick(&dirty, budget);
    let rebuilt: HashSet<usize> = todo.iter().copied().collect();

    // ── The chosen areas' rows. ──
    if !todo.is_empty() {
        let plant_reg = data.get::<farming::PlantRegistry>("plant_registry");
        // N-P-K for the crop card (2026-09-26, farming::soil).
        let item_reg = data.get::<ItemRegistry>("item_registry");
        let nutrient_scale = data.get::<sl::NutrientData>("garden_nutrients").zip(plant_reg).and_then(|(nd, reg)| nd.scale_for(reg));
        let light_data = data.get::<lighting::LightingData>("garden_lighting");
        let plot_areas = data.get(farming::units::PLOT_AREA_KEY);
        let ents: Vec<hecs::Entity> = todo.iter().flat_map(|i| rows.areas[*i].crops.iter().copied()).collect();
        _part = crate::frame_costs::stage("cpu.garden.ph_view");
        let ph_view = soil_ph::GardenView::new(data, world);
        _part = crate::frame_costs::stage("cpu.garden.pollination_view");
        let pollination = pol_ctx.as_ref().map(|c| pollination::GuiView::for_crops(world, c, &ents)).unwrap_or_default();
        _part = crate::frame_costs::stage("cpu.garden.picking_view");
        let picking = picking::GuiView::for_crops(world, data, &ents);
        _part = crate::frame_costs::stage("cpu.garden.weed_view");
        let weed_view = weeds::GuiView::context(world, data);
        let kept_weeds: HashSet<&str> = weed_view.kept_areas().collect();
        // Each crop's wait and water, for the Garden card (first-hour F11, systems/farming/outlook.rs).
        _part = crate::frame_costs::stage("cpu.garden.outlook_view");
        let outlook_view = outlook::OutlookView::new(world, data);
        _part = crate::frame_costs::stage("cpu.garden.crop_rows");
        for &i in &todo {
            let a = &mut rows.areas[i];
            let mut valid_until = f64::INFINITY;
            a.kinds.clear();
            a.species.clear();
            a.living_crops.clear();
            a.light = None;
            a.air = None;
            for (k, &entity) in a.crops.iter().enumerate() {
                let Ok(mut q) = world.query_one::<(&CropInstance, Option<&CropSoil>)>(entity) else { continue };
                let Some((crop, soil)) = q.get() else { continue };
                let bits: u64 = entity.to_bits().into();
                let plants = farming::units::crop_plants(crop, plant_reg, plot_areas);
                let need = farming::crop_season_need(&crop.crop_def_id, plants, plant_reg, item_reg, nutrient_scale);
                let store = soil.map_or_else(|| sl::fresh_store(need), |s| s.store);
                let (supply_share, scarce) = sl::sufficiency(&store, &need);
                let def = plant_reg.and_then(|r| r.get(&crop.crop_def_id));
                let per_kg = def.zip(nutrient_scale).map_or(crate::ecs::components::Npk::default(), |(d, s)| sl::removal_per_kg(d, s));
                let phc = ph_view.crop(crop, def); // soil pH for the crop card
                let name = def.map(|d| d.name.clone()).unwrap_or_else(|| crop.crop_def_id.clone());
                let stages: Vec<&str> =
                    def.map(|d| d.stages()).unwrap_or_else(|| crate::ecs::components::DEFAULT_GROWTH_STAGES.iter().copied().collect());
                let dead = crop.growth_stage.as_str() == crate::ecs::components::STAGE_DEAD;
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
                let species = match a.species.iter().position(|s| *s == crop.crop_def_id) {
                    Some(s) => s,
                    None => {
                        a.species.push(crop.crop_def_id.clone());
                        a.species.len() - 1
                    }
                };
                a.kinds.push((species, def.map_or(true, |d| d.needs_light)));
                if !dead {
                    a.living_crops.push((crop.crop_def_id.clone(), crop.growth_stage.clone()));
                }
                let outlook_row = outlook_view.row(crop, def);
                valid_until = valid_until.min(outlook_view.hold_until(crop, def)).min(weed_view.row_holds_until(crop, def));
                let mut row = GuiCrop {
                    entity_bits: bits,
                    name,
                    stage: crop.growth_stage.clone(),
                    progress,
                    water: crop.water_level,
                    health: crop.health,
                    season_health: farming::season_health(crop),
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
                    // The light's and the air's words come below, from the area's own readings.
                    light: String::new(),
                    light_need: light_data.map(|d| lighting::light_need_word(def, d)).unwrap_or_default(),
                    soil: [store.n as f32, store.p2o5 as f32, store.k2o as f32],
                    need: [need.n as f32, need.p2o5 as f32, need.k2o as f32],
                    short_of: (supply_share < 1.0).then(|| scarce.word().to_string()),
                    ph: phc.ph,
                    ph_window: phc.window,
                    ph_cap: phc.cap,
                    ph_held: phc.held,
                    pollination: pollination.row(entity),
                    picking: picking.row(entity),
                    humidity: String::new(),
                    weeds: weed_view.crop_row(crop, def),
                    tile_key: 0,
                };
                row.tile_key = crate::gui::pages::inventory::crop_tile_key(&row, &outlook_row.tile);
                out.outlook.insert(bits, outlook_row);
                if let Some(slot) = out_crops.get_mut(a.range.start + k) {
                    *slot = row;
                }
            }
            a.living = !a.living_crops.is_empty();
            // Its own lines.
            a.pests = match (pest_data, a.crops.is_empty()) {
                (Some(pd), false) => {
                    let (pests, rel) = farming::pests::area_report(world, pd, &a.name);
                    let pests: Vec<(String, f32, Vec<(String, String, String)>)> = pests
                        .into_iter()
                        .filter(|(_, lvl, _)| *lvl >= 0.01)
                        .map(|(p, lvl, _)| {
                            let controls = pd.controls_for(&p.id).into_iter().filter(|c| !c.is_cover()).map(|c| (c.id.clone(), c.name.clone(), c.note.clone())).collect();
                            (p.name.clone(), lvl as f32, controls)
                        })
                        .collect();
                    let releases: Vec<(String, f32)> = rel.into_iter().map(|(c, d)| (c.name.clone(), d as f32)).collect();
                    (!pests.is_empty() || !releases.is_empty()).then(|| GuiAreaPests { area: a.name.clone(), pests, releases })
                }
                _ => None,
            };
            a.cover = pest_data.and_then(|pd| {
                farming::pests::cover_row(data, pd, &a.name, memory.and_then(|(_, m)| m.pests.get(&a.name)), &a.living_crops)
            });
            a.weeds = if a.living || kept_weeds.contains(a.name.as_str()) { weed_view.area_row(data, &a.name) } else { None };
            a.flags = pollination.flags.get(&a.name).copied();
            a.key = keys[i];
            a.valid = (elapsed, valid_until);
            a.built_at = builds;
        }
    }

    // ── The light's and the air's words, for every area (see the module notes). ──
    _part = crate::frame_costs::stage("cpu.garden.air_view");
    let plant_reg = data.get::<farming::PlantRegistry>("plant_registry");
    let lit = lighting::light_now(world, data);
    let tags: Vec<String> = {
        let mut t: Vec<String> = rows.areas.iter().filter(|a| a.living).map(|a| a.name.clone()).collect();
        t.sort();
        t.dedup();
        t
    };
    let air = humidity::GuiView::with_areas(world, data, tags);
    let air_severity = air.severity();
    for (i, a) in rows.areas.iter_mut().enumerate() {
        if a.crops.is_empty() {
            continue;
        }
        let fresh = rebuilt.contains(&i);
        let rows_here = &mut out_crops[a.range.clone()];
        // The light: the sun, and this area's share of the powered grow lights.
        let outdoors = a.tower.as_deref().map_or(false, farming::is_field_area);
        let cover = a.tower.as_deref().and_then(|t| lit.cover.get(t)).copied().unwrap_or(0.0);
        let light_key = {
            let mut h = KeyHasher::default();
            h.write_u8(lit.sun_up as u8);
            h.write_u64(cover.to_bits());
            h.write_u8(outdoors as u8);
            h.finish()
        };
        if fresh || !reuse || a.light.as_ref().map_or(true, |(k, _)| *k != light_key) {
            let words = [false, true].map(|needs| lighting::light_word(needs, outdoors, lit.sun_up, cover));
            for (row, (_, needs)) in rows_here.iter_mut().zip(&a.kinds) {
                let w = &words[*needs as usize];
                if row.light != *w {
                    row.light = w.clone();
                }
            }
            a.light = Some((light_key, words));
        }
        // The air: this area's humidity and CO2 as its room's air stands.
        let air_key = {
            let mut h = KeyHasher::default();
            let [rh, co2] = air.reading(&a.name);
            h.write_u64(rh);
            h.write_u64(co2);
            h.write_u32(air_severity.to_bits());
            h.write_u64(global);
            h.finish()
        };
        if fresh || !reuse || a.air.as_ref().map_or(true, |(k, _)| *k != air_key) {
            let words: Vec<String> = a.species.iter().map(|s| air.row_in(&a.name, plant_reg.and_then(|r| r.get(s)))).collect();
            for (row, (species, _)) in rows_here.iter_mut().zip(&a.kinds) {
                let w = &words[*species];
                if row.humidity != *w {
                    row.humidity = w.clone();
                }
            }
            a.air = Some((air_key, words));
        }
    }

    // ── The lists the page shows by area, sorted by area. ──
    _part = crate::frame_costs::stage("cpu.garden.areas");
    if relaid || !todo.is_empty() || !reuse {
        let sorted = || rows.sorted.iter().map(|i| &rows.areas[*i]);
        out.areas = sorted().filter_map(|a| a.pests.clone()).collect();
        out.covers = sorted().filter_map(|a| a.cover.clone()).collect();
        out.weeds = sorted().filter_map(|a| a.weeds.clone()).collect();
    }
    let hives = pollination::hive_rows(world, data);
    out.pollinate_areas = rows.sorted.iter().map(|i| &rows.areas[*i]).filter_map(|a| a.flags.and_then(|f| pollination::area_row(&a.name, f, &hives))).collect();
    out.air = air.areas;
    out.home_air = air.home; // ship life support: the home's air
    out.ph_amendments = soil_ph::amendments(data);
}

#[cfg(test)]
#[path = "garden_mirror_tests.rs"]
mod tests;
