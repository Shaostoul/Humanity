//! The one "turn off all reuse" switch (CLAUDE.md, the reuse rule, refined 2026-10-30).
//!
//! The operator's rule: nothing kept across runs during development, and data kept in memory
//! across frames only when it cannot go stale (rebuilt on a schedule, or tagged with a version its
//! source bumps), "and every one of them obeys a single 'turn off all reuse' switch in the Dev page.
//! When a bug smells like a cache, one click rules caches in or out." (the proposal he took, "I like
//! this idea, let's do it."). Every reuse in the game asks `enabled()` and, when it is false, works
//! everything out again instead: the draw lists start empty (`engine::state::DrawBufs`), the dev
//! request reader checks every file every frame (`engine::ipc::RequestWatch`), the Garden page data
//! builds every grow area's rows every build instead of keeping them by area while the farm's
//! change numbers, the data's stamps and the clock say nothing they read moved
//! (`engine::garden_mirror`, `systems::farming::versions`, 2026-10-31), and a page lays out every
//! block of a long list, on screen or not (`gui::widgets::offscreen`, 2026-10-31: the Inventory
//! page's Garden groups and rows of slot tiles). The farm itself (systems/farming, farm-clock.md
//! rung 2): an area not yet advanced to now is state kept stale on purpose, so with the switch off
//! every grow area and the air advance every frame (`clock.rs`); and what the farm's body keeps
//! for a round of its rotation is worked out again in every run: which pests eat each plant
//! (`FarmingSystem::pest_hosts`), the grow rooms' layout for the air (`humidity::AirLayout`) and
//! the hives' and devices' reach (`pollination`). Since stutter survey 2 (2026-10-31): every egui
//! context's fonts borrow one copy of the font files read at startup (`gui::fonts`), and a page
//! screen's first sight comes over its first turns (`gui::screen_surface::turn_plan`); with the
//! switch off each context builds its own fonts and a first sight runs twice and paints on one
//! frame, as before. The live music (2026-11-07, BUG-267: `engine::music`): while the switch is
//! on, the score plays from the data and the recordings it was built from (its samples decoded once
//! through `instrument_packs::samples::SampleBank`, which keeps them between builds); while it is
//! off, a new score is built from the files read again and every recording decoded again (the bank
//! keeping nothing) once every phrase, taking over at the next phrase boundary, and the music's
//! data for the hum and Settings' lists is read again every few seconds. The whole-Earth map packs
//! (2026-11-09, `humanity_terrain::map_ground`, driven by `engine::map_meshes`): the tiles decoded
//! from them and each pack's shore field are kept while patches need them, tagged by the pack's
//! build; while the switch is off a fresh set is published every frame, so every tile and shore
//! field is decoded again. The clouds and the rooms' bounced light (2026-11-13): the clouds'
//! accumulated picture from earlier frames is dropped on every frame
//! (`renderer::cloud_resolve::drops_history`), the sun-shadow atlas is re-anchored and baked
//! whole on every frame instead of an eighth at a time (`cloud_temporal::CloudLightState::plan`),
//! and the far rung's profile, which cannot be built in one frame, steps aside
//! (`cloud_temporal::cloud_fr_knob_with_reuse`); the room probes forget which rooms hold them,
//! so every resident room is seeded from this frame's ambient colour, and every traced probe takes
//! its fresh estimate whole (`renderer::room_probes`, "THE REUSE SWITCH"). The atmosphere's
//! lookup tables and the shared grass mesh, kept while their inputs match, are built again on
//! every frame (`keeps`, the helper for a reuse tagged with a version).
//! The pulley designer (2026-11-13, docs/design/pulleys.md): the
//! Calculator's Pulleys mode keeps its last answer under a key of its inputs, and
//! `humanity_systems::pulley::ShapeStore` keeps the last block-and-tackle shapes under a key of
//! every number they are built from; with the switch off both are worked out again every time.
//! A new reuse that does not ask is a bug.
//!
//! THE ENGINE AND THE HOME, part A of the second wave (2026-11-13). Where every frame would be far
//! over a frame, the switch puts the reuse on a short SCHEDULE instead, so nothing is older than
//! the schedule: the rule's other allowed form.
//! - `src/engine/frame_water.rs`: the sea's two shells. The kept selection is walked again every
//!   frame (`selection_kept`), and every drawn patch of both shells is built again on its turn, a
//!   slice a frame, once every 30 frames (`patch_rebuild_due`; every frame would be about a
//!   thousand patch builds a frame). Their meshes depend on nothing that moves since the backstop's
//!   drop went to the vertex shader (the real bug this wave found: the drop was baked into each
//!   kept patch from the crest at build time, so a sea that rose after left the backstop in the
//!   troughs). The patches themselves are `ChunkState`'s (`planet_chunks.rs`), shared with the
//!   terrain, which is not answered yet (tests/reuse_switch_lint.rs, the backlog).
//! - `src/engine/frame_near_trees.rs`: `decoration_mesh_cache`, and the near-tree harvest
//!   (`near_tree_inputs`): with the switch off the set is harvested every frame
//!   (`humanity_terrain::near_tree_gate`, `ReuseOff`); with it on, the gate now also watches the
//!   tree model distance and the near-tree budget (the second real bug: neither slider showed
//!   anything until the player walked 12 m). The near-tree models (`engine::near_tree_models`): a
//!   model that failed to load is tried again every 3 s (`failed_model_retry_due`); the procedural
//!   meshes, sprites and atlas are a pure function of the code and the tree registry, which is read
//!   once per run, and the photoscans are read once per run the same way: not reuses that can go
//!   stale, so they stay.
//! - `src/engine/state.rs`: `decoration_mesh_cache` (above; world decorations' models are read once
//!   per run the same way), `stock_pile_cache` (the stock piles' layout, laid out every frame with
//!   the switch off; keyed by the stored volume AND the home's edits since the third real bug: an
//!   edit that moved a storage zone and kept the volume left the piles where it used to be),
//!   `hero_plant_missing` (the garden's models found missing, forgotten on each full rebuild),
//!   `plant_mesh_sig` (the garden's signature, zeroed for each full rebuild),
//!   `machine_model_materials` (a model's picture uploaded again with the switch off,
//!   `home_reuse::model_material_kept`), `solar_orbit_paths` (the orbit rings, worked out again
//!   from the clock every second, `world_load::orbit_paths_due`).
//! - `src/engine/home_reuse.rs`: the home and the garden. The home is rebuilt every 5 s while
//!   aboard (with every machine drawn afresh a rebuild measured 301 to 637 ms, far over a frame;
//!   the doors keep their state across it):
//!   the shell, the editor's storey cut-away, the hull, the machines with their Furniture detail
//!   bodies, the pipes and their markings, all through `rebuild_homestead`, past the edit flags
//!   (lib.rs), `furniture_detail::needs_rebuild` and `pipe_markers::rebuild_if_mode_changed`.
//!   The whole garden is built again every 2 s once the last full rebuild has finished (about
//!   400 ms of the worker).
//! - `src/engine/home_meshes.rs`: `reuse_off_at` and `full_at` (when the schedules last ran);
//!   `tent_meshes` and `tent_materials` (a fruiting tent's cover, 2026-11-13: built again when
//!   the tent or its door's roll step changes, and every frame with the switch off, its materials
//!   then written again from what they are made of).
//! - `src/engine/map_view.rs`: `last_key` (the Maps world layer, rendered every frame with the
//!   switch off, `world_layer_due`).
//! - `src/engine/map_tiles.rs`: `tiles` (the plan tiles: with the switch off, once every wanted
//!   tile is baked, the one baked longest ago is baked again, one a frame, `tile_to_bake`).
//! - `src/engine/screens/video.rs`: `FrameDraw::Keep` (a quiet tick keeps nothing with the switch
//!   off; it composes and uploads the last frame again, `plan_frame`).
//! - `src/renderer/stars.rs`: `constellation_rgba` (the line buffer rebuilt every frame with the
//!   switch off, `constellation_kept`).
//! - `src/engine/screens/camera.rs`: `last_render` (camera screens). Already a schedule: each
//!   renders at most every 100 ms and at most one a frame (`CAMERA_INTERVAL`, `pick_due`), so a
//!   picture is never older than its turn; it stays as it is.
//! - `src/gui/pages/settings.rs`: `CACHE` (Settings > Media's "Found ffmpeg" line, in
//!   `media_status_line`: worked out at most every two seconds, and on every frame with the switch
//!   off). The converted videos once kept on disk in `<data dir>/media/cache` were not put behind
//!   the switch: they are gone, a film plays through the person's ffmpeg as it goes
//!   (`media::stream`, 2026-11-13).
//! - `src/renderer/taa.rs`: temporal anti-aliasing (2026-11-13, renderer roadmap rung 1a). The
//!   history and the per-object motion book are discarded every frame with the switch off
//!   (`history_kept`, `MotionBook::turn`), so the picture aliases exactly as Off does.
//! - `src/renderer/skinning.rs`: `last_staging`, `last_keys` (the bodies' previous palette for the
//!   motion target: with the switch off `skin_upload_prev` uploads this frame's rows, so nobody moves
//!   on their own).
//!
//! The switch is the Dev tab's Performance section ("Turn off all reuse"), for this run only, and
//! `HUMANITY_NO_REUSE=1` for a rig or a scripted run.

use std::sync::atomic::{AtomicU8, Ordering};

const UNREAD: u8 = 0;
const ON: u8 = 1;
const OFF: u8 = 2;

/// Unread until the first question, when `HUMANITY_NO_REUSE` is read once.
static STATE: AtomicU8 = AtomicU8::new(UNREAD);

/// May a reuse keep what it worked out earlier? True unless the switch is off.
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        _ => {
            let off = std::env::var_os("HUMANITY_NO_REUSE").is_some_and(|v| v == "1");
            if off {
                log::info!("Reuse: OFF for this run (HUMANITY_NO_REUSE=1): everything is worked out again every frame");
            }
            STATE.store(if off { OFF } else { ON }, Ordering::Relaxed);
            !off
        }
    }
}

/// May a reuse TAGGED WITH A VERSION keep what it built for `have` now that its source says
/// `want`? Only while the two match exactly and reuse is on. `reuse` is `enabled()` at the call
/// site (`keeps(self.key, key, reuse::enabled())`); it is a parameter so a test can drive both
/// arms without flipping the switch for every test in the process.
pub fn keeps<K: PartialEq>(have: Option<K>, want: K, reuse: bool) -> bool {
    reuse && have == Some(want)
}

/// Turn every reuse on or off, from the Dev page.
pub fn set_enabled(on: bool) {
    STATE.store(if on { ON } else { OFF }, Ordering::Relaxed);
    log::info!(
        "Reuse: {} (Dev > Performance)",
        if on { "ON: reused data is kept between frames again" } else { "OFF: everything is worked out again every frame" }
    );
}

#[cfg(test)]
mod tests {
    /// The switch flips both ways and every reader sees it at once.
    #[test]
    fn the_switch_turns_every_reuse_off_and_on() {
        super::set_enabled(false);
        assert!(!super::enabled());
        super::set_enabled(true);
        assert!(super::enabled());
    }

    /// A version-tagged reuse keeps what it built only for the same version and only with reuse
    /// on; with reuse off it builds again even for the version it has (so the check can fail).
    /// Seen red 2026-11-13 with `keeps` ignoring `reuse`: "reuse off: built again for the same
    /// version".
    #[test]
    fn with_reuse_off_a_versioned_reuse_builds_again_for_the_same_version() {
        assert!(!super::keeps(Some(7u32), 7, false), "reuse off: built again for the same version");
        assert!(super::keeps(Some(7u32), 7, true), "reuse on: kept for the same version");
        assert!(!super::keeps(Some(6u32), 7, true), "a new version: built again");
        assert!(!super::keeps(None, 7u32, true), "nothing built yet: built");
    }
}
