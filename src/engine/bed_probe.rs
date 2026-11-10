//! THE BED PROBE: a road's cross-section and a building's footing, read out for the rig
//! (whole-Earth maps, the rung between 2 and 3, 2026-11-09; docs/design/whole-earth-maps.md 6.1
//! and 6.2).
//!
//! A picture shows a road ribbon floating or half buried, or a house sunk into a hill; it cannot
//! say by how much. This readout answers from the game's own numbers:
//!
//! * **A road** (`{"bed_probe":"road;lat,lon"}`): the road bed whose centreline passes nearest the
//!   point (`map_ground::MapSet::road_near`), and a cross-section square to it every 0.25 m, out to
//!   8 m past each side of its bed: `drawn_m` (the patch drawn over that point this frame, what the
//!   eye sees), `ground_m` (the walk oracle), `raw_m` (the land layer, no bed) and `ribbon_m` (where
//!   the district mesher put the ribbon's surface there, by `osm_region::road_edge_radius`, the one
//!   rule it draws by), all metres above sea level.
//! * **A building** (`{"bed_probe":"building;lat,lon"}`): the pad holding or nearest the point,
//!   its floor (`map_beds`), the mesher's wall bottom and floor (`osm_region::building_levels`), and
//!   at every corner and half an apron outside it the drawn ground, the walk oracle and the land
//!   layer.
//!
//! Written to `debug/bed_probe.json` (engine/ipc.rs routes the verb); judged by
//! scripts/lib/bed-probe-check.js for the vantages' `bed_probe` (scripts/probe-sweep.js). Dev
//! tooling, permanent. Cost: one request, a few hundred ground samples on the main thread, never
//! per frame.

use crate::engine::map_meshes::DistrictGround;
use crate::engine::planet_build::{Ground, PlayerView};
use crate::engine::state::EngineState;
use glam::DVec3;
use humanity_terrain::drawn_surface::{drawn_leaf_containing, DrawnPatchSurface};
use humanity_terrain::map_ground;
use humanity_terrain::osm_region::{building_levels, dir_to_latlon_f64, latlon_to_dir_f64, road_edge_radius, MeshGround};
use humanity_terrain::planet_chunks::ElevationSource;
use std::collections::HashMap;

/// Where the readout lands, relative to the working directory like every other dev IPC file.
pub(crate) const REPORT_PATH: &str = "debug/bed_probe.json";

/// The cross-section's sample spacing, metres.
const STEP_M: f64 = 0.25;
/// How far past each side of the bed the cross-section runs, metres.
const PAST_BED_M: f64 = 8.0;

/// Parse `"road;lat,lon"` or `"building;lat,lon"`.
pub(crate) fn parse(spec: &str) -> Option<(String, (f64, f64))> {
    let (kind, at) = spec.split_once(';')?;
    let mut it = at.split(',').map(|v| v.trim().parse::<f64>());
    let (lat, lon) = match (it.next(), it.next(), it.next()) {
        (Some(Ok(lat)), Some(Ok(lon)), None) if lat.abs() <= 90.0 && lon.abs() <= 360.0 => (lat, lon),
        _ => return None,
    };
    matches!(kind.trim(), "road" | "building").then(|| (kind.trim().to_string(), (lat, lon)))
}

/// The point `off_m` metres from `c` along the unit tangent `t`, on a sphere of `radius_m` (f64).
fn offset(c: DVec3, t: DVec3, off_m: f64, radius_m: f64) -> DVec3 {
    (c + t * (off_m / radius_m)).normalize()
}

/// Build the readout for one road or building near `at`.
pub(crate) fn report(state: &EngineState, kind: &str, at: (f64, f64)) -> Result<serde_json::Value, String> {
    let view = PlayerView::of(state);
    let body = view.body.clone().ok_or("not locked to a body")?;
    if body != "earth" {
        return Err(format!("the map packs are Earth's; the player is at {body}"));
    }
    let def = state.planet_defs.get(&body).ok_or("no planet definition for Earth")?;
    let hm: &humanity_terrain::planet_heightmap::PlanetHeightmap = state.planet_heightmaps.get(&body).ok_or("Earth has no heightmap")?;
    let cs = state.planet_chunk_states.get(&body).ok_or("Earth has no chunked terrain")?;
    let ground = Ground::for_body(state, &body).ok_or("no ground for Earth")?;
    let tiles = state.terrain_tiles.tier_installed().then_some(&state.terrain_tiles);
    let src = ElevationSource::Heightmap { hm, detail: &cs.detail, tiles, ocean: state.ocean_mask.as_ref() };
    let map = map_ground::snapshot().ok_or("no map pack is installed")?;
    let dg = DistrictGround::new(def, hm, &map);
    let sea_r = dg.sea_r;
    let (elev, natural) = (|d: DVec3| dg.elev(d), |d: DVec3| dg.natural(d));
    let mg = MeshGround { elev: &elev, natural: &natural, beds: map_ground::beds_enabled() };
    let mut surfaces: HashMap<u8, DrawnPatchSurface> = HashMap::new();
    let mut drawn = |d: DVec3| -> (Option<u8>, Option<f64>) {
        match drawn_leaf_containing(&cs.last_drawn, d) {
            Some(l) => (Some(l.depth), Some(surfaces.entry(l.depth).or_insert_with(|| DrawnPatchSurface::new(def, &src, l.depth).with_map(Some(map.clone()))).radius_at(d) - sea_r)),
            None => (None, None),
        }
    };
    let raw = |d: DVec3| map.raw_land_m(d).map(|(h, _)| h);
    let d = latlon_to_dir_f64(at.0, at.1);
    let head = serde_json::json!({
        "at": [at.0, at.1],
        "beds": mg.beds,
        "walk_depth": cs.last_drawn.iter().map(|p| p.depth).max().unwrap_or(0),
        "pack_depth": crate::engine::map_meshes::pack_detail_now().0,
        "pack_within_m": crate::engine::map_meshes::pack_detail_now().1,
        "eye_above_sea_m": view.anchor.length() - sea_r,
    });
    let mut out = match kind {
        "road" => {
            let r = map.road_near(d).ok_or("no road bed reaches that point")?;
            let (l, rt) = (offset(r.centre, r.across, -r.half_w_m, def.radius), offset(r.centre, r.across, r.half_w_m, def.radius));
            let (rib_l, rib_r) = (road_edge_radius(&mg, r.centre, l, r.class, false) - sea_r, road_edge_radius(&mg, r.centre, rt, r.class, false) - sea_r);
            let span = r.core_m + PAST_BED_M;
            let n = (2.0 * span / STEP_M).round() as i64;
            let mut samples = Vec::with_capacity(n as usize + 1);
            for i in 0..=n {
                let off = -span + i as f64 * STEP_M;
                let p = offset(r.centre, r.across, off, def.radius);
                let (leaf, drawn_m) = drawn(p);
                let ribbon = (off.abs() <= r.half_w_m + 1e-9).then(|| rib_l + (rib_r - rib_l) * (off + r.half_w_m) / (2.0 * r.half_w_m));
                samples.push(serde_json::json!({
                    "off_m": off,
                    "drawn_m": drawn_m,
                    "leaf_depth": leaf,
                    "ground_m": ground.radius(p) - sea_r,
                    "raw_m": raw(p),
                    "ribbon_m": ribbon,
                }));
            }
            let (clat, clon) = dir_to_latlon_f64(r.centre);
            serde_json::json!({
                "kind": "road",
                "osm_id": r.osm_id,
                "class": r.class,
                "half_w_m": r.half_w_m,
                "core_m": r.core_m,
                "centre": [clat, clon],
                "from_asked_m": r.dist_m,
                "bed_m": r.bed_m,
                "samples": samples,
            })
        }
        _ => {
            let p = map.pad_near(d).ok_or("no building pad reaches that point")?;
            let (bottom, floor, _) = building_levels(&mg, &p.ring, 0.0).ok_or("the footprint has no ground under it")?;
            let c = p.ring.iter().fold(DVec3::ZERO, |a, v| a + *v).normalize();
            let mut corners = Vec::with_capacity(p.ring.len());
            for v in &p.ring {
                let (lat, lon) = dir_to_latlon_f64(*v);
                let out = (*v - c * v.dot(c)).normalize_or_zero();
                let a = offset(*v, out, p.apron_m * 0.5, def.radius);
                let ((leaf, drawn_m), (_, apron_m)) = (drawn(*v), drawn(a));
                corners.push(serde_json::json!({
                    "lat": lat,
                    "lon": lon,
                    "drawn_m": drawn_m,
                    "leaf_depth": leaf,
                    "ground_m": ground.radius(*v) - sea_r,
                    "raw_m": raw(*v),
                    "apron_drawn_m": apron_m,
                }));
            }
            serde_json::json!({
                "kind": "building",
                "osm_id": p.osm_id,
                "floor_m": p.floor_m,
                "apron_m": p.apron_m,
                "mesh_bottom_m": bottom - sea_r,
                "mesh_floor_m": floor - sea_r,
                "corners": corners,
            })
        }
    };
    if let (Some(o), Some(h)) = (out.as_object_mut(), head.as_object()) {
        for (k, v) in h {
            o.insert(k.clone(), v.clone());
        }
    }
    Ok(out)
}

/// Write the readout for `spec` to [`REPORT_PATH`] and return the line for the log. A request the
/// game cannot answer still writes a file, with an `error`, so the rig never waits out its timeout.
pub(crate) fn write_report(state: &EngineState, spec: &str) -> String {
    let value = match parse(spec) {
        None => serde_json::json!({ "error": format!("bed_probe wants \"road;lat,lon\" or \"building;lat,lon\", not {spec:?}") }),
        Some((kind, at)) => report(state, &kind, at).unwrap_or_else(|e| serde_json::json!({ "error": e })),
    };
    let summary = match value.get("error") {
        Some(e) => format!("error: {e}"),
        None => format!("{} {} at walk depth {}", value["kind"], value["osm_id"], value["walk_depth"]),
    };
    match serde_json::to_string_pretty(&value) {
        Ok(text) => {
            let _ = std::fs::create_dir_all("debug");
            if let Err(e) = std::fs::write(REPORT_PATH, text) {
                return format!("could not write {REPORT_PATH}: {e}");
            }
        }
        Err(e) => return format!("could not serialise the bed readout: {e}"),
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verb's spec parses a kind and a point, and refuses anything else. Seen red 2026-11-09
    /// with the kind check removed: "a bridge is not a probe kind".
    #[test]
    fn the_bed_probe_spec_parses() {
        assert_eq!(parse("road;47.65,-122.69"), Some(("road".to_string(), (47.65, -122.69))));
        assert_eq!(parse(" building ; 47.6 , -122.7 ").map(|p| p.0), Some("building".to_string()));
        assert!(parse("bridge;47.65,-122.69").is_none(), "a bridge is not a probe kind");
        assert!(parse("road;47.65").is_none() && parse("road;95,0").is_none() && parse("road").is_none());
    }
}
