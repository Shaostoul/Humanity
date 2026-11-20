//! The probe hold (the headless sim runner, increment 4, 2026-11-15), split out of
//! frame_particles.rs: a scripted camera park re-asserting its pose after the movement
//! integrator. It is part of the simulation's step (it moves the body), so it runs where it ran,
//! between the walk aboard and the house's power, while the ambient particles it shared a file
//! with are drawing and run after the step (tests/frame_split_lint.rs).

use crate::engine::frame_prelude::*;

/// Holds a probe's camera pose over what the movement integrator did this frame.
pub(crate) fn run(state: &mut EngineState) {
    let dt = state.clock.dt;
    humanity_core::frame_costs::span("cpu.span.probe_hold");
    // PROBE HOLD (environment program increment 2): a
    // scripted camera park re-asserts its pose every frame
    // so gravity/buoyancy cannot drag the probe off its
    // altitude during a long capture settle - every "high
    // eye fell to the sea" incident and the 2026-08-18
    // terrain-wedge artifact were this drift. Enforced
    // AFTER update_camera so whatever the integrator did
    // this frame is simply overwritten; the frame-lock
    // ride still moves the frame itself, so the ground
    // does not slide underneath. Any movement input
    // clears the hold (see the keyboard handler).
    // The locked body's turn, read before the hold is borrowed.
    let probe_body_rot = crate::engine::planet_build::frame_body_rot(state);
    // The grace is timed on the step's clock (the headless sim runner, increment 5, 2026-11-20:
    // the last wall-clock read in the step): the same 1.5 s at a steady frame rate, and a headless
    // run's park settles over the seconds of the world it steps, not of the wall clock.
    let now_s = state.clock.elapsed_s;
    if let Some((pos, at)) = state.probe_hold.as_mut() {
        if now_s - *at < 1.5 {
            // Grace: track through the park's basis and
            // surface-mode transitions (see the field doc
            // in engine::state - restoring pre-transition
            // coordinates mis-frames the probe).
            *pos = state.camera.position;
        } else {
            // Rig descent (v0.1245): sustained downward
            // motion along the local radial, so a parked
            // probe can reproduce what FLIGHT does to the
            // temporal cloud map (a continuous epipolar
            // history displacement - the radial smear
            // converging at the nadir the operator sees on
            // every arrival, which single-frame teleports
            // can never paint).
            if state.probe_descend_mps > 0.0 {
                let a = state.frame_lock_anchor;
                if a.length_squared() > 0.5 {
                    let down_w = probe_body_rot * (-a.normalize());
                    let dm = state.probe_descend_mps * dt;
                    *pos += Vec3::new(
                        (down_w.x as f32) * dm,
                        (down_w.y as f32) * dm,
                        (down_w.z as f32) * dm,
                    );
                    // The ANCHOR must sink too: in the
                    // surface-owned bands (below the
                    // co-rotate ceiling) the frame lock
                    // derives ship_world_pos from the
                    // anchor MINUS cam_local, so a pin
                    // moved down alone is exactly
                    // compensated (the same property the
                    // far_frame split exploits) and the
                    // camera freezes. Where the anchor is
                    // re-captured per frame instead, this
                    // shrink is harmlessly overwritten.
                    let r = state.frame_lock_anchor.length();
                    state.frame_lock_anchor *=
                        ((r - dm as f64) / r).max(0.0);
                }
            }
            state.camera.position = *pos;
        }
    }
}
