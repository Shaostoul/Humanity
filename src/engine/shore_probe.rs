//! THE SHORE PROBE: the ground along a transect across a coast, read out for the rig (whole-Earth
//! maps, rung 1, 2026-11-09; docs/design/whole-earth-maps.md section 5.5).
//!
//! The operator, 2026-11-08: "We did have a problem where the heightmap of the world and the
//! Silverdale waterfront area wasn't even. Like in the water front initially didn't have any water
//! and then we had weird sheer cliffs along water area instead of nice slopes like in real life."
//! A picture shows a wall or a dry harbour; it cannot say how high or where. This readout samples,
//! every few metres along a great-circle transect between two points:
//!
//! * `ground_m`: the ground the player stands on there ([`Ground::radius`], the walk clamp's own
//!   oracle: the drawn patch surface at the deepest drawn depth, the map packs included), minus
//!   the sea radius. Metres above sea level, as drawn.
//! * `leaf_depth` / `drawn_m`: the patch actually drawn over that point this frame and its surface
//!   there (what the eye sees, which can be coarser than the walk oracle far from the camera).
//! * `raw_m` / `raw_source`: the unmodified land layer, the source the ground is supposed to
//!   follow: a map pack's GEDTM30 lattice where a pack covers (raw: no shore band), else the same
//!   drawn surface with no pack. The judge compares rises against it (no false wall).
//! * `class` / `class_source`: what the game's own coastline data says is under the point: a map
//!   pack's OSM water polygons where a pack covers (exact, "pack:<file>"), else the connected-ocean
//!   mask (0.05 degree cells, about 5.5 km). The judge needs to know which, because the coarse one
//!   cannot place a coast to the metre (that gap IS what rung 4's downloader closes).
//!
//! Requested with the showcase verb `{"shore_probe":"lat,lon;lat,lon","shore_step_m":"2.5"}`
//! (engine/ipc.rs); written to `debug/shore_probe.json`. Judged by
//! scripts/lib/shore-probe-check.js for the vantages' `shore_probe` (scripts/probe-sweep.js). Dev
//! tooling, permanent. Cost: one request, a few thousand ground samples on the main thread (a
//! few milliseconds), never per frame.

use crate::engine::planet_build::{Ground, PlayerView};
use crate::engine::state::EngineState;
use glam::DVec3;
use humanity_terrain::drawn_surface::{drawn_leaf_containing, DrawnPatchSurface};
use humanity_terrain::planet_chunks::ElevationSource;
use humanity_terrain::map_ground;
use humanity_terrain::map_pack::WaterRingKind;
use std::collections::HashMap;

/// Where the readout lands, relative to the working directory like every other dev IPC file.
pub(crate) const REPORT_PATH: &str = "debug/shore_probe.json";

/// The plan's sample spacing (whole-earth-maps.md 5.5).
pub(crate) const STEP_M_DEFAULT: f64 = 2.5;

/// A transect longer than this many samples is refused (a mistyped coordinate, not a coast).
const MAX_SAMPLES: usize = 20_000;

/// Parse `"lat,lon;lat,lon"` (degrees) into the transect's two ends.
pub(crate) fn parse_ends(spec: &str) -> Option<[(f64, f64); 2]> {
    let mut ends = spec.split(';').map(|p| {
        let mut it = p.split(',').map(|v| v.trim().parse::<f64>());
        match (it.next(), it.next(), it.next()) {
            (Some(Ok(lat)), Some(Ok(lon)), None) if lat.abs() <= 90.0 && lon.abs() <= 360.0 => Some((lat, lon)),
            _ => None,
        }
    });
    let a = ends.next()??;
    let b = ends.next()??;
    if ends.next().is_some() {
        return None;
    }
    Some([a, b])
}

/// The unit directions `step_m` apart along the great circle from `a` to `b` on a sphere of
/// `radius_m` (both ends included), f64 throughout.
pub(crate) fn transect(a: DVec3, b: DVec3, radius_m: f64, step_m: f64) -> Vec<DVec3> {
    let ang = a.angle_between(b);
    let n = ((ang * radius_m / step_m.max(0.01)).ceil() as usize).max(1);
    let sin = ang.sin();
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            if sin.abs() < 1e-12 {
                a
            } else {
                (a * ((1.0 - t) * ang).sin() / sin + b * (t * ang).sin() / sin).normalize()
            }
        })
        .collect()
}

/// Build the readout along the transect `ends` (land end first for a shore, by the vantage's
/// convention), `step_m` apart, on the body the player stands on.
pub(crate) fn report(state: &EngineState, ends: [(f64, f64); 2], step_m: f64) -> Result<serde_json::Value, String> {
    let view = PlayerView::of(state);
    let body = view.body.clone().ok_or("not locked to a body")?;
    let def = state.planet_defs.get(&body).ok_or("no planet definition for the body")?;
    let hm: &humanity_terrain::planet_heightmap::PlanetHeightmap = state.planet_heightmaps.get(&body).ok_or("the body has no heightmap")?;
    let cs = state.planet_chunk_states.get(&body).ok_or("the body has no chunked terrain")?;
    let ground = Ground::for_body(state, &body).ok_or("no ground for the body")?;
    let earth = body == "earth";
    let tiles = (earth && state.terrain_tiles.tier_installed()).then_some(&state.terrain_tiles);
    let ocean = if earth { state.ocean_mask.as_ref() } else { None };
    let src = ElevationSource::Heightmap { hm, detail: &cs.detail, tiles, ocean };
    let map = map_ground::snapshot();
    let max_drawn_depth = cs.last_drawn.iter().map(|p| p.depth).max().unwrap_or(0);
    let sea_r = def.radius * humanity_terrain::planet_surface::displaced_radius_f64_true(def, def.sea_level.clamp(0.0, 1.0) as f64);

    let a = humanity_terrain::osm_region::latlon_to_dir_f64(ends[0].0, ends[0].1);
    let b = humanity_terrain::osm_region::latlon_to_dir_f64(ends[1].0, ends[1].1);
    let dirs = transect(a, b, def.radius, step_m);
    if dirs.len() > MAX_SAMPLES {
        return Err(format!("the transect is {} samples long (over {MAX_SAMPLES}): check the coordinates", dirs.len()));
    }
    // The same surface with no map pack: the land layer outside every pack.
    let mut base = DrawnPatchSurface::new(def, &src, max_drawn_depth.max(1)).with_map(None);
    let mut drawn_at: HashMap<u8, DrawnPatchSurface> = HashMap::new();
    let mut samples = Vec::with_capacity(dirs.len());
    let mut sources: HashMap<String, usize> = HashMap::new();
    let length_m = a.angle_between(b) * def.radius;
    let spacing_m = length_m / (dirs.len() - 1).max(1) as f64;
    for (i, &d) in dirs.iter().enumerate() {
        let (lat, lon) = humanity_terrain::osm_region::dir_to_latlon_f64(d);
        let ground_m = ground.radius(d) - sea_r;
        let leaf = drawn_leaf_containing(&cs.last_drawn, d);
        let drawn_m = leaf.map(|l| {
            drawn_at.entry(l.depth).or_insert_with(|| DrawnPatchSurface::new(def, &src, l.depth).with_map(map.clone())).radius_at(d) - sea_r
        });
        let (raw_m, raw_source) = match map.as_ref().and_then(|m| m.raw_land_m(d)) {
            Some((h, pack)) => (h, format!("land_h:{pack}")),
            None => (base.radius_at(d) - sea_r, "base_ground".to_string()),
        };
        let (class, class_source) = match map.as_ref().and_then(|m| m.class_at(d)) {
            Some((c, pack)) => (
                match c {
                    None | Some(WaterRingKind::Island) => "land",
                    Some(WaterRingKind::Sea) => "sea",
                    Some(_) => "lake",
                },
                format!("pack:{pack}"),
            ),
            None => match ocean {
                Some(m) => (if m.is_ocean(d.as_vec3()) { "sea" } else { "land" }, "ocean_mask:0.05deg".to_string()),
                None => ("unknown", "none".to_string()),
            },
        };
        *sources.entry(class_source.clone()).or_default() += 1;
        samples.push(serde_json::json!({
            "s_m": i as f64 * spacing_m,
            "lat": lat,
            "lon": lon,
            "ground_m": ground_m,
            "leaf_depth": leaf.map(|l| l.depth),
            "drawn_m": drawn_m,
            "raw_m": raw_m,
            "raw_source": raw_source,
            "class": class,
            "class_source": class_source,
        }));
    }
    let eye = view.anchor;
    Ok(serde_json::json!({
        "body": body,
        "from": [ends[0].0, ends[0].1],
        "to": [ends[1].0, ends[1].1],
        "step_m": spacing_m,
        "length_m": length_m,
        "sea_radius_m": sea_r,
        "walk_depth": max_drawn_depth,
        "drawn_patches": cs.last_drawn.len(),
        "tile_tier": tiles.is_some(),
        "map_packs": map.as_ref().map(|m| m.packs.len()).unwrap_or(0),
        "dev_switches": {
            "shore_band": std::env::var(map_ground::SHORE_BAND_ENV).map(|v| v.trim() != "0").unwrap_or(true),
        },
        "eye_above_sea_m": eye.length() - sea_r,
        "class_sources": sources,
        "samples": samples,
    }))
}

/// Write the readout for `spec` (`"lat,lon;lat,lon"`) to [`REPORT_PATH`] and return the line for
/// the log. A request the game cannot answer still writes a file, with an `error`, so the rig
/// never waits out its timeout for nothing.
pub(crate) fn write_report(state: &EngineState, spec: &str, step: Option<&str>) -> String {
    let step_m = step.and_then(|s| s.trim().parse::<f64>().ok()).filter(|s| *s > 0.0).unwrap_or(STEP_M_DEFAULT);
    let value = match parse_ends(spec) {
        None => serde_json::json!({ "error": format!("shore_probe wants \"lat,lon;lat,lon\", not {spec:?}") }),
        Some(ends) => report(state, ends, step_m).unwrap_or_else(|e| serde_json::json!({ "error": e })),
    };
    let summary = match value.get("error") {
        Some(e) => format!("error: {e}"),
        None => format!(
            "{} samples over {:.0} m at walk depth {}",
            value["samples"].as_array().map(Vec::len).unwrap_or(0),
            value["length_m"].as_f64().unwrap_or(0.0),
            value["walk_depth"]
        ),
    };
    match serde_json::to_string_pretty(&value) {
        Ok(text) => {
            let _ = std::fs::create_dir_all("debug");
            if let Err(e) = std::fs::write(REPORT_PATH, text) {
                return format!("could not write {REPORT_PATH}: {e}");
            }
        }
        Err(e) => return format!("could not serialise the shore readout: {e}"),
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verb's two ends, and the refusals. Seen red 2026-11-09 with the third-field check
    /// dropped (`(Some(Ok(lat)), Some(Ok(lon)), _)`): "lat,lon,alt is not a transect end".
    #[test]
    fn parse_ends_reads_two_points() {
        assert_eq!(parse_ends("47.645,-122.695;47.640,-122.695"), Some([(47.645, -122.695), (47.64, -122.695)]));
        assert_eq!(parse_ends(" 51.138, 1.36 ; 51.13 ,1.36"), Some([(51.138, 1.36), (51.13, 1.36)]));
        assert_eq!(parse_ends("47.645,-122.695"), None, "one end is not a transect");
        assert_eq!(parse_ends("47.645,-122.695;47.64,-122.695;47.6,-122.6"), None, "three ends");
        assert_eq!(parse_ends("47.645,-122.695,10;47.64,-122.695"), None, "lat,lon,alt is not a transect end");
        assert_eq!(parse_ends("95,0;0,0"), None, "latitude past the pole");
        assert_eq!(parse_ends("north;south"), None);
    }

    /// The transect: both ends exactly, samples evenly spaced on the great circle, f64. Seen red
    /// 2026-11-09 with a straight `a.lerp(b, t)` for the slerp: "sample 1 is
    /// 0.000000000016998402685430847 off the sphere" (a chord: 0.1 mm low at the first sample, about
    /// 6 mm at the middle of 556 m, and every sample's direction off its true place).
    #[test]
    fn transect_samples_the_great_circle_evenly() {
        let r = 6_371_000.0;
        let a = humanity_terrain::osm_region::latlon_to_dir_f64(47.6450, -122.6950);
        let b = humanity_terrain::osm_region::latlon_to_dir_f64(47.6400, -122.6950);
        let pts = transect(a, b, r, 2.5);
        let len = a.angle_between(b) * r;
        assert_eq!(pts.len(), (len / 2.5).ceil() as usize + 1);
        assert!((pts[0] - a).length() < 1e-15 && (pts[pts.len() - 1] - b).length() < 1e-12);
        let step = len / (pts.len() - 1) as f64;
        for (i, w) in pts.windows(2).enumerate() {
            assert!((w[0].length() - 1.0).abs() < 1e-12, "sample {i} is {} off the sphere", (w[0].length() - 1.0).abs());
            let d = (w[1] - w[0]).length() * r;
            assert!((d - step).abs() < 1e-6, "step {i} is {d} m, want {step} m");
        }
        assert_eq!(transect(a, a, r, 2.5).len(), 2, "a zero-length transect is its one point twice");
    }
}
