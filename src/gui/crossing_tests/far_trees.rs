//! From terrain/far_trees.rs's tests: the far trees' clamp at the shipped card default of the
//! settings (`config::AppConfig`).

use crate::terrain::far_trees::*;
use crate::terrain::far_trees::tests::forest_def;
use glam::DVec3;

/// A warning nobody can trigger is decoration; one that fires on a fresh
/// install is noise. This pins BOTH ends against the SHIPPED terrain LOD
/// defaults, the way the grass instance-cap warning is pinned in
/// `settings.rs`.
///
/// It also records the number that motivated the whole change. At the
/// shipped `terrain_split_px` / `terrain_patch_budget`, on this synthetic
/// world, the reach measures ~1.8 km at walking height and ~2.2 km at 1 km
/// up, and `budget_saturated` is TRUE in every one of those runs. The
/// second fact is the load-bearing one: raising or lowering `split_px`
/// across 4.0 and 9.579 does not move the reach AT ALL once the budget
/// binds, so the closed form everyone reaches for first - solve the split
/// threshold for distance - is answering a question the engine stopped
/// asking. Measure it.
///
/// Native-only because pinning against the SHIPPED defaults means reading
/// `AppConfig`, and `crate::config` is a native module (the relay has no
/// interactive settings file to default from). The rest of this module's
/// tests are pure maths and do run in the relay build.
#[cfg(feature = "native")]
#[test]
fn the_clamp_is_quiet_at_the_shipped_card_default_and_bites_at_the_ceiling() {
    use crate::terrain::planet_chunks::{
        select_patches, ChunkParams, RadialBand, MAX_BUILD_REQUESTS, TILE_MAX_PATCH_DEPTH,
    };
    let def = forest_def();
    let measured = RadialBand {
        min_r_m: def.radius - 2.0,
        max_r_m: def.radius + 2.0,
    };
    let cfg = crate::config::AppConfig::default();
    let params = ChunkParams {
        occluder_r_m: None,
        radius_m: def.radius,
        band: RadialBand {
            min_r_m: def.radius - 200.0,
            max_r_m: def.radius + 200.0,
        },
        max_depth: TILE_MAX_PATCH_DEPTH,
        split_px: cfg.terrain_split_px,
        px_per_rad: 1080.0 / 60f32.to_radians(),
        max_leaves: cfg.terrain_patch_budget as usize,
        max_build_requests: MAX_BUILD_REQUESTS,
    };
    // Walking height: the case a player is in essentially always.
    let cam = DVec3::new(def.radius + 2.0, 0.0, 0.0);
    let sel = select_patches(cam, None, &|_| Some(measured), &params);
    let reach = sel.stats.card_reach_m as f32;
    assert!(
        sel.stats.budget_saturated,
        "the shipped LOD defaults no longer saturate the leaf budget, so the \
         reason this reach is MEASURED rather than derived from split_px may \
         have gone away; re-read the module note before trusting either"
    );
    // QUIET where it ships: the default card distance fits inside the reach,
    // so a fresh install sees no warning and no clamp.
    assert_eq!(
        effective_card_far_m(cfg.veg_tree_card_m, reach),
        cfg.veg_tree_card_m,
        "the shipped card distance ({} m) no longer fits inside the measured \
         reach ({reach} m): a fresh install would open Settings to a warning",
        cfg.veg_tree_card_m
    );
    // REACHABLE: the slider's own top end does not fit, which is where the
    // operator had it when they reported the open-field-then-forest ring.
    assert!(
        effective_card_far_m(3_000.0, reach) < 3_000.0,
        "the slider ceiling (3000 m) now fits inside the measured reach \
         ({reach} m), so the warning can never fire and is dead code"
    );
}
