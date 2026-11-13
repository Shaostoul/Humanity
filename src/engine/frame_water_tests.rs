//! The water shells' tests (moved out of frame_water.rs, 2026-11-13, for its line budget). Each
//! was seen red first; what broke it and what it said is in its comment.

use super::*;

/// THE BACKSTOP REAL BUG (2026-11-13): the backstop's depth was baked into each patch's mesh
/// from the crest at the frame it was built, and the patch was kept. A sea that rose after
/// (wind, a storm, an ocean event folded into the crest) left the kept patches at the calm
/// day's shallow drop, standing in the new troughs. Built at a 0.3 m crest and drawn under a
/// 3 m one, the backstop must still sit below the troughs. Seen red 2026-11-13 with the drop
/// baked into the mesh: "the backstop sits 0.80 m down under a 3.0 m crest".
#[test]
fn a_backstop_built_on_a_calm_day_clears_a_storms_troughs() {
    // Built on a calm day (0.3 m crest: the mesh no longer takes the crest at all) and drawn
    // under a 3 m one.
    let storm = 3.0_f32;
    let depth = -backstop_build_lift_m() + backstop_shader_drop_m(storm);
    assert!(depth >= storm as f64 + 0.4, "the backstop sits {depth:.2} m down under a {storm:.1} m crest");
    // And on a glassy day it is still tight (the v0.1051 reason the drop tracks the crest).
    let glassy = -backstop_build_lift_m() + backstop_shader_drop_m(0.1);
    assert!(glassy <= 0.1 + BACKSTOP_CLEARANCE_M + 1e-6, "a glassy sea keeps a tight backstop, got {glassy:.2} m");
}

/// The CPU twin above is only worth anything if the shader really lowers the backstop by the
/// live crest plus the same clearance. Seen red 2026-11-13 before the shader had the branch:
/// "the shader's clearance is not const BACKSTOP_CLEARANCE_M: f32 = 0.5;".
#[test]
fn the_backstop_drop_is_the_shaders() {
    let src = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/pbr/00-bindings-vertex.wgsl"))
        .expect("the vertex shader");
    let want_const = format!("const BACKSTOP_CLEARANCE_M: f32 = {:.1};", BACKSTOP_CLEARANCE_M);
    assert!(src.contains(&want_const), "the shader's clearance is not {want_const}");
    assert!(
        src.contains("disp = disp - (camera.light5_cone_inner.x + BACKSTOP_CLEARANCE_M);"),
        "the shader's backstop branch does not drop by the live crest"
    );
}

/// With the reuse switch off the stored water selection is never used again: the walk runs
/// every frame. Seen red 2026-11-13 with the parked test alone: "reuse off kept the selection".
#[test]
fn with_reuse_off_the_water_selection_runs_every_frame() {
    assert!(selection_kept(true, true), "reuse on, parked: kept");
    assert!(!selection_kept(true, false), "moved: walked");
    assert!(!selection_kept(false, true), "reuse off kept the selection");
}

/// With the reuse switch off every drawn patch is built again within the period, each once;
/// with it on, never. Seen red 2026-11-13 with no rebuild at all: "patch never rebuilt with
/// reuse off (or more than once)".
#[test]
fn with_reuse_off_every_water_patch_is_built_again_within_the_period() {
    let ids: Vec<chunks::PatchId> = (0..6u8)
        .flat_map(|face| (0..40u64).map(move |path| chunks::PatchId { face, depth: 9, path: path * 7 }))
        .collect();
    for id in &ids {
        let turns = (0..WATER_REUSE_OFF_PERIOD).filter(|f| patch_rebuild_due(id, 1000 + f, false)).count();
        assert_eq!(turns, 1, "patch never rebuilt with reuse off (or more than once): {id:?}");
        assert!((0..WATER_REUSE_OFF_PERIOD).all(|f| !patch_rebuild_due(id, 1000 + f, true)), "reuse on rebuilt {id:?}");
    }
    // A slice per frame, not all at once: no frame takes more than a few times its share.
    let worst = (0..WATER_REUSE_OFF_PERIOD)
        .map(|f| ids.iter().filter(|id| patch_rebuild_due(id, f, false)).count())
        .max()
        .unwrap_or(0);
    let share = ids.len() as u64 / WATER_REUSE_OFF_PERIOD;
    assert!((worst as u64) <= share * 3 + 2, "one frame rebuilt {worst} of {} patches", ids.len());
}
