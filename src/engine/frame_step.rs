//! The simulation's step on the engine (the headless sim runner, increment 4, 2026-11-15;
//! docs/design/headless-sim-runner.md, section 2): every stage of the frame that moves the world, in
//! the frame's order, run once. Both drivers run this one function after advancing the frame's
//! clock (`state.clock`): the windowed frame (lib.rs, `WindowEvent::RedrawRequested`, after
//! frame_clock.rs reads the wall clock) and a headless run (engine/sim_walk.rs, its clock advanced
//! by its script, with no GPU). Nothing here draws; the view's stages run after it, windowed only.
//! tests/frame_split_lint.rs reads the stage list from here.

use crate::engine::frame_prelude::*;

/// One step of the world on the engine, by the frame's clock (advanced already by the driver).
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    // ONE spin for the whole frame (see the field docs), from the clock just advanced.
    state.current_spin = current_planet_spin(state);
    crate::engine::frame_station::run(state);
    crate::engine::frame_move_setup::run(state);
    crate::engine::frame_walk_aboard::run(state, f);
    crate::engine::frame_probe_hold::run(state);
    crate::engine::frame_house_power::run(state);
    crate::engine::frame_planet_walk::run(state);
    crate::engine::frame_collide_drive::run(state, f);
    crate::engine::frame_look_env::run(state);
    crate::engine::frame_bridges::run(state);
    crate::engine::frame_bridges_npc::run(state);
    crate::engine::frame_ecs::run(state);
    crate::engine::frame_settles::run(state);
    crate::engine::doors::tick(state);
}
