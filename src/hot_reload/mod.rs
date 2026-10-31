//! Hot-reload coordinator — watches data files and invalidates caches.
//!
//! On native: polls FileWatcher each frame for changed files,
//! invalidates AssetManager cache entries, systems reload on next access.
//! On WASM: hot-reload is not supported (data fetched from server).

pub mod data_store;
/// The data folder's file watcher (notify): here since the crate split's rung 0, beside the
/// coordinator that polls it (it was `assets::watcher`).
#[cfg(feature = "native")]
pub mod watcher;

#[cfg(feature = "native")]
use watcher::FileWatcher;

/// Coordinates hot-reload between file watcher and asset manager.
pub struct HotReloadCoordinator {
    #[cfg(feature = "native")]
    watcher: Option<FileWatcher>,
}

impl HotReloadCoordinator {
    /// Create a new coordinator. On native, starts watching the data directory.
    #[cfg(feature = "native")]
    pub fn new(data_dir: &std::path::Path) -> Self {
        let watcher = match FileWatcher::new(data_dir.to_path_buf()) {
            Ok(w) => Some(w),
            Err(e) => {
                log::warn!("Hot-reload disabled: {e}");
                None
            }
        };
        Self { watcher }
    }

    /// Create a no-op coordinator (WASM).
    #[cfg(feature = "wasm")]
    pub fn new() -> Self {
        Self {}
    }

    /// Poll for file changes. Call once per frame. Returns the changed file paths, relative to
    /// the data directory, for the caller to invalidate in the asset manager and to log
    /// (`poll_and_apply`). It used to take the asset manager and invalidate them itself; since the
    /// crate split's rung 0 it only reports, so this coordinator does not name the renderer's
    /// `AssetManager`.
    #[cfg(feature = "native")]
    pub fn poll(&self) -> Vec<String> {
        match self.watcher {
            Some(ref watcher) => watcher.poll_changes(),
            None => Vec::new(),
        }
    }

    /// No-op poll for WASM.
    #[cfg(feature = "wasm")]
    pub fn poll(&self) -> Vec<String> {
        Vec::new()
    }
}

/// One frame of hot-reload: poll the watcher, then apply every explicit
/// regeneration gate. Extracted whole from the lib.rs frame loop
/// (v0.1119, monolith ratchet) - this file owns WHICH data files trigger
/// WHAT rebuild, so a new gate is added here, not in lib.rs.
///
/// The two plant calls in the middle are not gates: the showcase reseed
/// and mesh rebuild run every frame (both internally sig/empty-gated, so
/// nearly free) and must stay AFTER the plants_visual gate that zeroes
/// the signature, preserving the original ordering exactly.
#[cfg(feature = "native")]
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
