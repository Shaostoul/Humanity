//! The ECS tick (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as it was. Since the headless sim runner's increment 2 (2026-11-15) its body is
//! `frame_sim::systems_slice`, one step of the systems on the simulation's parts (home storage
//! mirrored in, the systems ticked once, what they used and made settled back into home storage,
//! deaths and backpack overflow), the same function the soak and the no-GPU tests step; this stage
//! only hands it the windowed game's parts and where the player stands.
//!
//! Order against the body it replaced: the one move is the packs' per-frame body list
//! (`PackEngine::new_frame`, the first line of the old `death_pack::after_tick`), cleared now
//! before the tick instead of after it; nothing between the two reads that list (the celestial
//! loop fills it later in the frame and the draw list takes it after that). Where the player
//! stands (`FallSite`: the frame lock, the camera's feet, the planets' ground) is read before the
//! tick instead of inside the death step after it; the systems cannot move the camera, the
//! controller or the frame lock, so it reads the same values.

use crate::engine::death_pack::FallSite;
use crate::engine::frame_prelude::*;
use crate::engine::frame_sim::SimParts;
use crate::engine::planet_build::GroundSources;

/// Runs the game world's systems for this frame.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.ecs");
    state.death_packs.new_frame();
    let site = FallSite {
        lock_body: state.frame_lock_body.as_deref(),
        lock_anchor: state.frame_lock_anchor,
        ground: Some(GroundSources {
            defs: &state.planet_defs,
            heightmaps: &state.planet_heightmaps,
            tiles: &state.terrain_tiles,
            ocean: state.ocean_mask.as_ref(),
            chunk_states: &state.planet_chunk_states,
        }),
        // The camera in the ship's own metres (the home is drawn at home-local + station_off,
        // aboard and off the station alike: ipc.rs `camera_home_position`).
        feet: crate::engine::ipc::camera_home_position(state.camera.position, state.station_off)
            - Vec3::Y * state.controller.eye_height(),
        eye_height: state.controller.eye_height(),
        floor_under: state.controller.ground_floor(),
    };
    let parts = SimParts {
        runner: &mut state.system_runner,
        world: &mut state.game_world.world,
        data: &mut state.data_store,
        gui: &mut state.gui_state,
        world_loaded: state.world_loaded,
    };
    crate::engine::frame_sim::systems_slice(parts, &state.clock, &site);
}
