//! THE GROUND UNDER THE NEAR TREES, read out for the rig (BUG-156, 2026-10-05).
//!
//! The probe rig photographed full-geometry trees hanging in the air beside the
//! Silverdale waterfront. A picture cannot say why: a tree drawn above the
//! ground can be standing on the wrong surface, on the right surface at the
//! wrong level of detail, or on a surface that was right when it was harvested
//! and has since been refined under it. This readout measures, for every tree
//! in the near set, where its base is against each candidate ground:
//!
//! * `drawn_r`: the ground actually DRAWN under the tree this frame (the patch
//!   of the drawn leaf set that covers it, at that patch's own depth, with the
//!   region carve and DEM, exactly what `build_patch_mesh` built). The gap to
//!   this is what the eye sees: daylight under the trunk when positive.
//! * `fine_r`: the finest-detail ground (`frame_lock::ground_radius_m`), where
//!   the ground converges as the patches under the tree refine.
//! * `harvest_r` / `harvest_uncarved_r`: the surface the harvest itself
//!   sampled (the depth it anchored to), with and without the region carve.
//!
//! The same four for the eye, so a capture also says how far above the drawn
//! ground the player stands.
//!
//! Requested with the showcase verb `{"tree_ground":"1"}` (engine/ipc.rs);
//! written to `debug/tree_ground.json`. Dev tooling, permanent: the probe rig's
//! `ground_probe` vantage check reads it (scripts/probe-sweep.js).

use crate::engine::planet_build::PlayerView;
use crate::engine::state::EngineState;
use crate::terrain::drawn_surface::{drawn_leaf_containing, DrawnPatchSurface};
use crate::terrain::planet_chunks::{self as chunks, ElevationSource};
use glam::DVec3;
use std::collections::HashMap;

/// Where the readout lands, relative to the working directory like every
/// other dev IPC file.
pub(crate) const REPORT_PATH: &str = "debug/tree_ground.json";

/// A tree whose base is this far above the drawn ground shows daylight under
/// its trunk: the same threshold `tree_bases_sit_on_the_drawn_surface` holds.
pub(crate) const FLOAT_TOLERANCE_M: f64 = 0.15;

/// Per-tree numbers, metres of radius from the planet centre unless named.
fn tree_row(
    i: usize,
    t: &chunks::NearTree,
    eye: DVec3,
    view: &ViewBasis,
    drawn_r: Option<(u8, f64)>,
    harvest_r: f64,
    harvest_uncarved_r: f64,
    fine_r: f64,
) -> serde_json::Value {
    let (lat, lon) = crate::terrain::osm_region::dir_to_latlon_f64(t.dir);
    let base = t.dir * t.r_m;
    let ndc = view.ndc(base - eye);
    serde_json::json!({
        "i": i,
        "species": t.species,
        "h": t.height_m,
        "lat": lat,
        "lon": lon,
        "dist_m": (base - eye).length(),
        "ndc": ndc.map(|(x, y)| [x, y]),
        "on_screen": ndc.is_some_and(|(x, y)| x.abs() <= 1.0 && y.abs() <= 1.0),
        "base_r": t.r_m,
        "sink_m": chunks::tree_flare_radius_m(t.height_m) * chunks::TREE_GROUND_SINK_FLARE_FRAC,
        "leaf_depth": drawn_r.map(|(d, _)| d),
        "drawn_r": drawn_r.map(|(_, r)| r),
        "gap_drawn_m": drawn_r.map(|(_, r)| t.r_m - r),
        "harvest_r": harvest_r,
        "harvest_uncarved_r": harvest_uncarved_r,
        "fine_r": fine_r,
        "gap_fine_m": t.r_m - fine_r,
    })
}

/// The camera's view in the body's unrotated frame, enough to say where on
/// the screen a point lands (no roll: a player on foot never banks).
struct ViewBasis {
    fwd: DVec3,
    right: DVec3,
    up: DVec3,
    tan_half_v: f64,
    aspect: f64,
}

impl ViewBasis {
    /// Normalized device coordinates of a point `rel` metres from the eye, or
    /// None when it is behind the camera. On screen when both are within 1.
    fn ndc(&self, rel: DVec3) -> Option<(f64, f64)> {
        let z = rel.dot(self.fwd);
        if z <= 1e-6 {
            return None;
        }
        let x = rel.dot(self.right) / z / (self.tan_half_v * self.aspect);
        let y = rel.dot(self.up) / z / self.tan_half_v;
        Some((x, y))
    }
}

/// Order statistics of a list of gaps, metres.
fn gap_stats(mut v: Vec<f64>) -> serde_json::Value {
    if v.is_empty() {
        return serde_json::json!({ "n": 0 });
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    let pick = |q: f64| v[((n - 1) as f64 * q).round() as usize];
    serde_json::json!({
        "n": n,
        "min": v[0],
        "p05": pick(0.05),
        "median": pick(0.5),
        "p95": pick(0.95),
        "max": v[n - 1],
        "floating": v.iter().filter(|g| **g > FLOAT_TOLERANCE_M).count(),
    })
}

/// Build the readout for the body the player stands on. `None` when there is
/// no chunked body under the player (aboard, or in space).
pub(crate) fn report(state: &EngineState) -> Option<serde_json::Value> {
    let v = PlayerView::of(state);
    let body = v.body.clone()?;
    let def = state.planet_defs.get(&body)?;
    let hm: &crate::terrain::planet_heightmap::PlanetHeightmap = state.planet_heightmaps.get(&body)?;
    let cs = state.planet_chunk_states.get(&body)?;
    let earth = body == "earth";
    let tiles = (earth && state.terrain_tiles.tier_installed()).then_some(&state.terrain_tiles);
    let ocean = if earth { state.ocean_mask.as_ref() } else { None };
    let src = ElevationSource::Heightmap { hm, detail: &cs.detail, tiles, ocean };
    let carve = crate::terrain::water_carve::snapshot();
    let eye = v.anchor;
    let up = eye.normalize_or_zero();
    let fwd = (v.rot.inverse() * v.forward.as_dvec3()).normalize_or_zero();
    let right = fwd.cross(up).normalize_or_zero();
    let view = ViewBasis {
        fwd,
        right,
        up: right.cross(fwd),
        tan_half_v: ((state.camera.fov_degrees as f64).to_radians() * 0.5).tan(),
        aspect: state.camera.aspect.max(0.01) as f64,
    };
    // One surface per depth asked about, carved the way the drawn patches are.
    let mut drawn_at: HashMap<u8, DrawnPatchSurface> = HashMap::new();
    let mut drawn_r = |dir: DVec3| -> Option<(u8, f64)> {
        let leaf = drawn_leaf_containing(&cs.last_drawn, dir)?;
        let s = drawn_at
            .entry(leaf.depth)
            .or_insert_with(|| DrawnPatchSurface::new_single_shot(def, &src, leaf.depth).with_carve(carve.clone()));
        Some((leaf.depth, s.radius_at(dir)))
    };
    let harvest_depth = state.near_tree_depth;
    let mut harvest = DrawnPatchSurface::new(def, &src, harvest_depth.max(1)).with_carve(carve.clone());
    let mut harvest_uncarved = DrawnPatchSurface::new(def, &src, harvest_depth.max(1)).with_carve(None);
    let tiles_any = earth.then_some(&state.terrain_tiles);
    let fine_r = |dir: DVec3| crate::engine::frame_lock::ground_radius_m(Some(def), Some(hm), Some(&cs.detail), tiles_any, dir);

    let mut rows = Vec::with_capacity(state.near_trees.len());
    let (mut all_gaps, mut screen_gaps, mut screen_fine) = (Vec::new(), Vec::new(), Vec::new());
    for (i, t) in state.near_trees.iter().enumerate() {
        let d = drawn_r(t.dir);
        let fine = fine_r(t.dir);
        let row = tree_row(
            i,
            t,
            eye,
            &view,
            d,
            harvest.radius_at(t.dir),
            harvest_uncarved.radius_at(t.dir),
            fine,
        );
        let on_screen = row["on_screen"].as_bool() == Some(true);
        if let Some((_, r)) = d {
            all_gaps.push(t.r_m - r);
            if on_screen {
                screen_gaps.push(t.r_m - r);
            }
        }
        if on_screen {
            screen_fine.push(t.r_m - fine);
        }
        rows.push(row);
    }
    let (lat, lon) = crate::terrain::osm_region::dir_to_latlon_f64(up);
    let eye_drawn = drawn_r(up);
    let eye_walk = crate::engine::planet_build::Ground::for_body(state, &body).map(|g| g.radius(up));
    let max_drawn_depth = cs.last_drawn.iter().map(|p| p.depth).max().unwrap_or(0);
    let since_harvest_s = state.start_time.elapsed().as_secs_f32() - state.near_tree_born_s;
    Some(serde_json::json!({
        "body": body,
        "eye": {
            "lat": lat,
            "lon": lon,
            "r": eye.length(),
            "leaf_depth": eye_drawn.map(|(d, _)| d),
            "drawn_r": eye_drawn.map(|(_, r)| r),
            "above_drawn_m": eye_drawn.map(|(_, r)| eye.length() - r),
            "walk_ground_r": eye_walk,
            "above_walk_ground_m": eye_walk.map(|r| eye.length() - r),
            "fine_r": fine_r(up),
            "above_fine_m": eye.length() - fine_r(up),
        },
        "harvest": {
            "depth": harvest_depth,
            "max_drawn_depth": max_drawn_depth,
            "drawn_patches": cs.last_drawn.len(),
            "trees": state.near_trees.len(),
            "since_s": since_harvest_s,
            "moved_since_m": (state.near_trees_center - eye).length(),
        },
        "carve_regions": carve.as_ref().map(|c| c.len()).unwrap_or(0),
        "fov_deg": state.camera.fov_degrees,
        "aspect": state.camera.aspect,
        "gap_drawn_all": gap_stats(all_gaps),
        "gap_drawn_on_screen": gap_stats(screen_gaps),
        "gap_fine_on_screen": gap_stats(screen_fine),
        "trees": rows,
    }))
}

/// Write the readout to [`REPORT_PATH`] and return the line for the log.
pub(crate) fn write_report(state: &EngineState) -> String {
    let value = report(state).unwrap_or_else(|| serde_json::json!({ "error": "not standing on a chunked body" }));
    let summary = format!(
        "on-screen gap to the drawn ground {} ; eye above drawn ground {}",
        value["gap_drawn_on_screen"],
        value["eye"]["above_drawn_m"]
    );
    match serde_json::to_string_pretty(&value) {
        Ok(text) => {
            let _ = std::fs::create_dir_all("debug");
            if let Err(e) = std::fs::write(REPORT_PATH, text) {
                return format!("could not write {REPORT_PATH}: {e}");
            }
        }
        Err(e) => return format!("could not serialise the tree ground readout: {e}"),
    }
    summary
}
