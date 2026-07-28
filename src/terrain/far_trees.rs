//! Far-tree card sheet (vegetation instancing arc increment 1, v0.1022).
//!
//! Trees used to exist in exactly two rings: real 3D models (~70 m) and
//! patch-baked sprite cards (out to the tree-card slider, ~1.5-3 km, and
//! only where depth >= TREE_MIN_DEPTH patches are resident). Beyond that:
//! nothing - the operator's "as I fly up I can easily see a lot of trees
//! aren't loaded", plus the square/circle boundary artifact (the square =
//! card-bearing PATCH edges; the circle = the radial card-fade slider).
//!
//! This module builds the THIRD ring: one streamed mesh of hash-decimated
//! representative cards covering ~1.2-150 km, decoupled from patch LOD
//! entirely. Density bands thin the planet-fixed tree-cell grid by a
//! per-axis stride (1x within 10 km, 4x to 40 km, 16x to 150 km), each
//! surviving cell contributing one clump card whose size grows with the
//! band so visual mass is roughly preserved. The SAME cell grid + xorshift
//! stream as the patch bake (planet_chunks vegetation pass) drives
//! position and species, so a sheet card stands exactly where the nearest
//! baked tree of its cell stands - the handoff never teleports a tree.
//!
//! The sheet rebuilds on a worker thread when the camera has moved far
//! enough (lib.rs owns the schedule); positions are emitted relative to a
//! camera-ground ANCHOR so the f32 mesh stays precise (<= 150 km offsets).
//! Bands 2-3 use flat-colored cards (the atlas texture is sub-pixel out
//! there and plain quads mip better); band 1 uses sprite-atlas cards that
//! match the baked ones.

use glam::DVec3;

use super::planet::PlanetDef;
use super::planet_albedo::PlanetAlbedo;
use super::planet_heightmap::PlanetHeightmap;
use super::planet_chunks::{veg_biome_ok, TREELINE_M, TREE_CELL_RAD};
use super::planet_surface::{surface_color, SurfaceMeshData, SurfaceVertexData};

/// Inner edge of the sheet: the patch-baked cards + models own the space
/// inside this (slight overlap with the card slider is deliberate - a
/// density blip beats a bare ring).
pub const FAR_TREE_NEAR_M: f64 = 1200.0;
/// Outer edge: beyond this even a clump card is sub-pixel from any
/// altitude where the planet still fills the screen.
pub const FAR_TREE_FAR_M: f64 = 150_000.0;
/// Rebuild when the camera ground point moved this far from the anchor.
pub const FAR_TREE_REBUILD_M: f64 = 2_000.0;

/// (outer radius m, per-axis cell stride, clump height m, clump width m)
/// per band. Each surviving cell renders a CANOPY CLUMP: two crossed
/// vertical cards spanning most of the collapsed cell area (grazing
/// views) plus one horizontal canopy quad at crown height (views from
/// above - a vertical billboard is edge-on-invisible from altitude,
/// which is exactly how v1 of this sheet vanished from 12 km). One
/// tree-sized card per cell was also 1/800th of forest density; clumps
/// approximate the CANOPY, not individual trees.
const BANDS: [(f64, u32, f32, f32); 3] = [
    (10_000.0, 1, 24.0, 200.0),
    (40_000.0, 4, 32.0, 840.0),
    (FAR_TREE_FAR_M, 16, 44.0, 3200.0),
];

/// Deterministic per-cell stream, IDENTICAL to the patch bake's (salt +
/// hash + xorshift draw order), so the sheet agrees with baked trees.
#[inline]
fn cell_stream(ix: i64, iy: i64) -> impl FnMut() -> u64 {
    let mut s = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ 0x51F0_A11C;
    if s == 0 {
        s = 0x94D0_49BB_1331_11EB;
    }
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }
}

/// Build the sheet around `cam_local` (planet-local metres). Returns the
/// mesh (positions relative to the returned anchor) or None when nothing
/// grew (open ocean, poles).
pub fn build_far_tree_sheet(
    def: &PlanetDef,
    hm: &PlanetHeightmap,
    albedo: Option<&PlanetAlbedo>,
    cam_local: DVec3,
) -> Option<(SurfaceMeshData, DVec3)> {
    let radius = def.radius;
    let sea = def.sea_level.clamp(0.0, 1.0) as f32;
    let range_m = hm.max_meters() - hm.min_meters();
    let cam_dir = cam_local.normalize_or_zero();
    if cam_dir.length_squared() < 0.5 {
        return None;
    }
    let anchor = cam_dir * radius;
    let cam_lat = cam_dir.y.clamp(-1.0, 1.0).asin();
    let cam_lon = (-cam_dir.z).atan2(cam_dir.x);

    let mut vertices: Vec<SurfaceVertexData> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut emit_card = |base: glam::Vec3,
                         up: glam::Vec3,
                         side: glam::Vec3,
                         w: f32,
                         h: f32,
                         color: [f32; 3],
                         sprite_tile: Option<u8>| {
        // Two-sided quad, exactly the baked-card construction (4 tris).
        let corner = |u01: f32, v01: f32| -> (glam::Vec3, [f32; 3]) {
            let p = base + up * (h * v01) + side * (w * (u01 - 0.5));
            match sprite_tile {
                Some(tile) => {
                    let enc_x = -((1 + tile) as f32 + u01 * 0.5);
                    (p, [-1.0, enc_x, v01])
                }
                None => (p, color),
            }
        };
        let c00 = corner(0.0, 0.0);
        let c10 = corner(1.0, 0.0);
        let c01 = corner(0.0, 1.0);
        let c11 = corner(1.0, 1.0);
        let nrm = up.to_array();
        for tri in [
            [c00, c10, c11],
            [c00, c11, c01],
            [c00, c11, c10],
            [c00, c01, c11],
        ] {
            for (p, col) in tri {
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: p.to_array(),
                    normal: nrm,
                    color: col,
                    water: false,
                    tree_card: sprite_tile.is_some(),
                    grass_card: false,
                });
            }
        }
    };

    let cell = TREE_CELL_RAD;
    let mut inner = FAR_TREE_NEAR_M;
    for (outer, stride, card_h, card_w) in BANDS {
        let stride_i = stride as i64;
        let ang_outer = outer / radius;
        let ylo = ((cam_lat - ang_outer) / cell).floor() as i64;
        let yhi = ((cam_lat + ang_outer) / cell).floor() as i64;
        for iy in (ylo..=yhi).filter(|iy| iy.rem_euclid(stride_i) == 0) {
            let cell_lat = (iy as f64 + 0.5) * cell;
            let cl = cell_lat.cos();
            if cl < 0.05 {
                continue; // polar caps: lon cells degenerate; no trees anyway
            }
            // Longitude span of the band at this latitude row.
            let lon_half = ang_outer / cl;
            let xlo = ((cam_lon - lon_half) / cell).floor() as i64;
            let xhi = ((cam_lon + lon_half) / cell).floor() as i64;
            for ix in (xlo..=xhi).filter(|ix| ix.rem_euclid(stride_i) == 0) {
                let mut next = cell_stream(ix, iy);
                let r0 = next();
                let r1 = next();
                let _r2 = next();
                let r3 = next();
                let _r4 = next();
                let r5 = next();
                // First tree of the cell = the representative (identical
                // draws to the bake, so it stands on a real baked tree).
                let lat = (iy as f64 + (r0 % 4096) as f64 / 4096.0) * cell;
                let lon = (ix as f64 + (r1 % 4096) as f64 / 4096.0) * cell;
                let cl2 = lat.cos();
                let dir = DVec3::new(cl2 * lon.cos(), lat.sin(), -cl2 * lon.sin());
                let dist = (dir * radius - cam_local).length();
                if dist < inner || dist > outer {
                    continue;
                }
                let e = hm.normalized_at(dir.as_vec3());
                let elev_m = (e - sea) * range_m;
                if elev_m < 6.0 || elev_m > TREELINE_M {
                    continue;
                }
                let sc = surface_color(def, albedo, dir.as_vec3(), e);
                if !veg_biome_ok(sc) {
                    continue;
                }
                let r_m = radius * super::planet_surface::displaced_radius_f64(def, e as f64);
                let base = ((dir * r_m) - anchor).as_vec3();
                let up = dir.as_vec3();
                let east = glam::Vec3::Y.cross(up).normalize_or_zero();
                let north = up.cross(east).normalize_or_zero();
                let az = (r3 % 6283) as f32 / 1000.0;
                let side = east * az.cos() + north * az.sin();
                // Canopy green from the local imagery, darkened + slightly
                // varied per cell so the far field reads as forest mass
                // with texture, not a flat lawn.
                let shade = 0.85 + (r5 % 256) as f32 / 256.0 * 0.3;
                let color = [
                    sc[0] * 0.50 * shade,
                    sc[1] * 0.58 * shade,
                    sc[2] * 0.46 * shade,
                ];
                // Crossed vertical cards: the grazing-view silhouette.
                emit_card(base, up, side, card_w, card_h, color, None);
                let side2 = up.cross(side).normalize_or_zero();
                emit_card(base, up, side2, card_w, card_h, color, None);
                // Horizontal canopy quad at crown height: the from-above
                // view (vertical cards are edge-on-invisible from
                // altitude). Axes swapped so the quad lies flat.
                emit_card(
                    // Centered: the closure treats its "up" axis as 0..1,
                    // so back up half a span along side2.
                    base + up * (card_h * 0.8) - side2 * (card_w * 0.5),
                    side2,
                    side,
                    card_w,
                    card_w,
                    color,
                    None,
                );
            }
        }
        inner = outer;
    }
    if vertices.is_empty() {
        return None;
    }
    Some((SurfaceMeshData { vertices, indices }, anchor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forest_def() -> PlanetDef {
        ron::from_str(
            r#"(
                name: "FarTreeTest",
                radius: 6371000.0,
                gravity: 9.81,
                terrain_seed: 7,
                ore_seed: 1,
                has_water: true,
                sea_level: 0.5,
                surface_relief: 0.011,
            )"#,
        )
        .expect("def parses")
    }

    fn land_heightmap() -> PlanetHeightmap {
        use super::super::planet_heightmap::{quantize_meters, HEIGHTMAP_MAGIC};
        let (w, h, min_m, max_m) = (16u32, 8u32, -1000.0f32, 1000.0f32);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HEIGHTMAP_MAGIC);
        bytes.extend_from_slice(&w.to_le_bytes());
        bytes.extend_from_slice(&h.to_le_bytes());
        bytes.extend_from_slice(&min_m.to_le_bytes());
        bytes.extend_from_slice(&max_m.to_le_bytes());
        for _ in 0..(w * h) {
            bytes.extend_from_slice(&quantize_meters(150.0, min_m, max_m).to_le_bytes());
        }
        PlanetHeightmap::from_bytes(&bytes).expect("heightmap parses")
    }

    #[test]
    fn sheet_builds_and_is_anchored_near_the_camera() {
        let def = forest_def();
        let hm = land_heightmap();
        let cam = DVec3::new(0.6, 0.5, 0.4).normalize() * (def.radius + 500.0);
        // No albedo: classify_color drives the biome gate; a uniform 150 m
        // land world classifies green enough for vegetation.
        let out = build_far_tree_sheet(&def, &hm, None, cam);
        let (mesh, anchor) = out.expect("uniform land world grows a sheet");
        assert!(mesh.vertices.len() >= 12, "at least one card");
        assert_eq!(mesh.indices.len(), mesh.vertices.len(), "flat card layout");
        // Every vertex within the far radius of the anchor (f32 safety).
        for v in &mesh.vertices {
            let p = glam::Vec3::from_array(v.position);
            assert!(
                (p.length() as f64) < FAR_TREE_FAR_M + 60_000.0,
                "vertex {} m from anchor",
                p.length()
            );
        }
        // Anchor is the camera ground point.
        assert!((anchor.length() - def.radius).abs() < 1.0);
    }

    #[test]
    fn sheet_is_deterministic_for_a_fixed_camera() {
        let def = forest_def();
        let hm = land_heightmap();
        let cam = DVec3::new(-0.2, 0.7, 0.7).normalize() * (def.radius + 1000.0);
        let a = build_far_tree_sheet(&def, &hm, None, cam).expect("grows");
        let b = build_far_tree_sheet(&def, &hm, None, cam).expect("grows");
        assert_eq!(a.0.vertices.len(), b.0.vertices.len());
        assert_eq!(a.1, b.1);
    }

    #[test]
    fn ocean_world_grows_nothing() {
        let def = forest_def();
        use super::super::planet_heightmap::{quantize_meters, HEIGHTMAP_MAGIC};
        let (w, h, min_m, max_m) = (16u32, 8u32, -1000.0f32, 1000.0f32);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HEIGHTMAP_MAGIC);
        bytes.extend_from_slice(&w.to_le_bytes());
        bytes.extend_from_slice(&h.to_le_bytes());
        bytes.extend_from_slice(&min_m.to_le_bytes());
        bytes.extend_from_slice(&max_m.to_le_bytes());
        for _ in 0..(w * h) {
            bytes.extend_from_slice(&quantize_meters(-500.0, min_m, max_m).to_le_bytes());
        }
        let hm = PlanetHeightmap::from_bytes(&bytes).expect("parses");
        let cam = DVec3::new(0.6, 0.5, 0.4).normalize() * (def.radius + 500.0);
        assert!(build_far_tree_sheet(&def, &hm, None, cam).is_none());
    }
}
