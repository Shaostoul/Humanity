//! What a changed data file rebuilds in the running game (the engine's half of hot reload).
//!
//! The file watcher and its coordinator are the core's (`crate::hot_reload`: they only report
//! which files changed). Which change rebuilds what is the engine's, because every rebuild here
//! works on the engine's state: the planet defs and their cached meshes, the procedural plants,
//! the showcase garden, the hull, the interior gravity. Here since the crate split's rung 0, part
//! B (2026-10-31): `poll_and_apply` was `hot_reload::poll_and_apply`, the one place the core
//! reached up into the engine (docs/design/crate-split.md).

/// One frame of hot-reload: poll the watcher, then apply every explicit
/// regeneration gate. Extracted whole from the lib.rs frame loop
/// (v0.1119, monolith ratchet) - this file owns WHICH data files trigger
/// WHAT rebuild, so a new gate is added here, not in lib.rs.
///
/// The two plant calls in the middle are not gates: the showcase reseed
/// and mesh rebuild run every frame (both internally sig/empty-gated, so
/// nearly free) and must stay AFTER the plants_visual gate that zeroes
/// the signature, preserving the original ordering exactly.
pub(crate) fn poll_and_apply(state: &mut crate::engine::state::EngineState) {
    let changes = state.hot_reload.poll();
    for changed in &changes {
        state.asset_manager.invalidate(changed);
    }
    for changed in &changes {
        log::info!("Hot-reload: {changed}");
    }
    // Planet-def hot-reload (v0.764): saving a data/planets/<id>.ron while
    // the game runs re-reads the defs and drops the cached surface meshes,
    // so palette / noise / sea-level tuning shows in the sky within a
    // frame - no relaunch during the visual tuning loop.
    if changes.iter().any(|c| {
        let n = c.replace('\\', "/");
        n.contains("planets/") && n.ends_with(".ron")
    }) {
        crate::engine::net_route::reload_planet_defs(state);
    }
    // Plant-visual hot-reload (v0.862): saving data/plants_visual.ron
    // regenerates every procedural plant next frame (the rebuild re-reads
    // the file; zeroing the signature forces it) - edit a leaf color,
    // alt-tab, and the garden already wears it.
    if changes.iter().any(|c| c.ends_with("plants_visual.ron")) {
        state.plant_mesh_sig = 0;
    }
    // Perpetual showcase (v0.863): an empty garden replants itself at
    // staggered stages while free resources are on (Creative and Dev; the
    // first-hour audit's Missing stakes 2), then the plant meshes regenerate
    // whenever growth actually moved. Only for a home whose start carries the
    // showcase garden (ship homes increment 6: a cabin has no garden, and the
    // grow machines it sees are the Commons').
    if crate::engine::save_slot::live_carries(crate::ship::starts::HomeContent::ShowcaseGarden) {
        crate::engine::ipc::auto_seed_showcase(state);
    }
    crate::engine::home_meshes::rebuild_plant_meshes(state);
    // Hull-profile hot-reload (v0.770): saving
    // data/blueprints/hull_profile.ron regrows the hull next frame -
    // silhouette tuning is save-file-see-hull, the same loop the planets
    // get. An invalid edit falls back to the embedded profile (warned in
    // the log), never a missing hull mid-session.
    if changes.iter().any(|c| {
        let n = c.replace('\\', "/");
        n.ends_with("blueprints/hull_profile.ron")
    }) {
        state.hull_profile = None; // force the re-read
        crate::engine::home_meshes::rebuild_hull(state);
    }
    // game.csv hot-reload (v0.1119): the interior-gravity knob
    // (gravity_m_s2) tunes live, like the file's own header always
    // promised. The watcher already dropped the stale cache entry, so
    // this re-reads the disk.
    if changes.iter().any(|c| c.ends_with("game.csv")) {
        if let Some(g) = state.asset_manager.game_setting_f64("gravity_m_s2") {
            state.controller.set_interior_gravity(g as f32);
            log::info!("game.csv: interior gravity now {g} m/s^2");
        }
    }
}
