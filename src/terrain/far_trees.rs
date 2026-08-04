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
//! All three bands use flat-coloured cards: the atlas texture is sub-pixel
//! out here and plain quads mip better. (The module header claimed band 1
//! used sprite-atlas cards through v0.1109; `emit_card` has only ever been
//! called with `sprite_tile: None`, so that was doc drift. The `Some(tile)`
//! arm is kept because it is the encoding a future instanced sheet wants.)
//!
//! WHAT "FLAT-COLOURED" MEANT, AND WHY IT WAS NOT (v0.1110). The colour this
//! module computes below - the local imagery darkened and greened toward a
//! closed canopy, jittered per cell - never reached the screen. The sheet
//! draws with the planet's TEXTURED material (lib.rs picks `textured_mat`),
//! so the type-12 fragment branch took its `has_tex` path and overwrote the
//! packed colour with the raw equirect imagery at the card's own lat/lon.
//! Every far card therefore rendered in the exact albedo of the ground under
//! it, lit as flat ground (a card's shading normal is the radial up), and -
//! since the analytic pixel footprint stays under a metre out to ~10 km -
//! even took the ground PBR detail pass, so distant forest was textured with
//! grass and dirt. Operator, v0.1109.3: "The LOD billboards seem like they're
//! getting full light making them much brighter than the high poly trees."
//!
//! The cards now carry a MARKER the shader can see (`encode_canopy_card_uv`),
//! which routes them to the canopy tint + the canopy lighting kernel and off
//! the ground-detail path. Flat colour is still the plan for the detail; only
//! the tone and the lighting changed.

use glam::DVec3;

use super::planet::PlanetDef;
use super::planet_albedo::PlanetAlbedo;
use super::planet_heightmap::PlanetHeightmap;
use super::planet_chunks::{veg_biome_weight, TREELINE_M, TREE_CELL_RAD, VEG_WEIGHT_MIN};
use super::planet_surface::{surface_color, SurfaceMeshData, SurfaceVertexData};

/// Inner edge of the sheet: the patch-baked cards + models own the space
/// inside this (slight overlap with the card slider is deliberate - a
/// density blip beats a bare ring).
pub const FAR_TREE_NEAR_M: f64 = 1200.0;
/// Outer edge: beyond this even a clump card is sub-pixel from any
/// altitude where the planet still fills the screen.
pub const FAR_TREE_FAR_M: f64 = 150_000.0;
/// Rebuild when the camera ground point moved this far from the anchor.
pub const FAR_TREE_REBUILD_M: f64 = 1_000.0;

/// (outer radius m, per-axis cell stride, clump height m, clump width m,
/// horizontal canopy quad?) per band.
///
/// v3 scale correction (operator field report, 2026-07-28: "a bunch of
/// squares that in some cases block out the entire sky" + camo-confetti
/// terrain from mid altitudes): v2 sized clump cards to the COLLAPSED
/// cell span (200/840/3200 m), which reads as texture from 12 km but is
/// architectural-slab nonsense from anywhere lower - and a stale sheet
/// during fast flight parked those slabs directly overhead. Cards are
/// now tree-cluster scale (a sparse impression of the forest, not a
/// 1:1 canopy replacement), horizontal crown quads exist only in the
/// far bands (their from-above job), and lib.rs hides a sheet whose
/// build anchor the camera has outrun (better briefly treeless than
/// wrong). True GPU instancing remains the arc's real endgame.
const BANDS: [(f64, u32, f32, f32, bool); 3] = [
    (10_000.0, 1, 22.0, 30.0, false),
    (40_000.0, 4, 26.0, 120.0, true),
    (FAR_TREE_FAR_M, 16, 30.0, 400.0, true),
];

/// How much darker and greener a closed canopy is than the open ground it
/// grows out of. Applied to the planet imagery, so the forest tracks the
/// biome under it instead of being one painted green.
///
/// LOCKSTEP with `CANOPY_GROUND_TINT` in
/// `assets/shaders/pbr/10-lighting-patterns.wgsl`; the shader applies it to
/// the imagery fetch, this module applies it to the packed fallback colour a
/// planet with no imagery uses, and
/// `canopy_constants_match_the_shader` fails if the two drift.
pub const FAR_CARD_CANOPY_TINT: [f32; 3] = [0.68, 0.76, 0.62];

/// Per-cell tone jitter range. Quantised to `FAR_CARD_SHADE_LEVELS + 1` steps
/// so the CPU colour and the GPU's decoded multiplier are the SAME number
/// rather than two roundings of one intent.
pub const FAR_CARD_SHADE_LO: f32 = 0.85;
pub const FAR_CARD_SHADE_HI: f32 = 1.15;
/// Top code of the 4-bit tone field (bits 18..21 of the packed channel).
pub const FAR_CARD_SHADE_LEVELS: u32 = 15;
/// Weight of bit 18: where the tone code sits above type 12's own bits
/// (colour in 0..15, water at 16, tree card at 17).
const FAR_CARD_SHADE_BIT: f32 = 262_144.0;

/// The tone multiplier a code decodes to. Mirror of `canopy_card_shade` in
/// the shader.
pub fn far_card_shade(code: u32) -> f32 {
    let t = code.min(FAR_CARD_SHADE_LEVELS) as f32 / FAR_CARD_SHADE_LEVELS as f32;
    FAR_CARD_SHADE_LO + (FAR_CARD_SHADE_HI - FAR_CARD_SHADE_LO) * t
}

/// Pack one far-sheet card's colour and tone code into the two floats
/// `mesh::planet_surface_vertices` hands through UNTOUCHED when
/// `color[0] < 0` (that sentinel means "the vertex already carries its own
/// uv"; the sprite-tile arm below uses the same door).
///
/// `uv.x` is the ordinary type-12 packed colour - `r*256 + g` - with the tone
/// code added at bit 18. Deliberately NO bit 17: that is the patch cards'
/// LOD-swap flag and would make the whole sheet distance-discard.
///
/// `uv.y` is `-(b + 1)`, which is the MARKER. A ground face's blue is clamped
/// to 0..1 by `pack_color_to_uv`, so any negative value is unambiguously a
/// card, and the +1 keeps a card whose blue is exactly zero from encoding as
/// -0.0 (which is not less than zero in IEEE754).
///
/// Max magnitude: 15*262144 + 255*256 + 255 = 3_997_695, well inside f32's
/// 2^24 exact-integer range, and the channel is `@interpolate(flat)` so no
/// interpolation can perturb it.
pub fn encode_canopy_card_uv(color: [f32; 3], shade_code: u32) -> [f32; 2] {
    let r = (color[0].clamp(0.0, 1.0) * 255.0).round();
    let g = (color[1].clamp(0.0, 1.0) * 255.0).round();
    let s = shade_code.min(FAR_CARD_SHADE_LEVELS) as f32 * FAR_CARD_SHADE_BIT;
    [s + r * 256.0 + g, -(color[2].clamp(0.0, 1.0) + 1.0)]
}

/// Inverse of `encode_canopy_card_uv`, and the Rust twin of the shader's
/// decode. Returns None when the uv is not a canopy card.
pub fn decode_canopy_card_uv(uv: [f32; 2]) -> Option<([f32; 3], u32)> {
    if uv[1] >= -0.5 {
        return None;
    }
    let packed = uv[0].round().max(0.0) as u32;
    Some((
        [
            ((packed >> 8) & 255) as f32 / 255.0,
            (packed & 255) as f32 / 255.0,
            -uv[1] - 1.0,
        ],
        (packed >> 18) & 15,
    ))
}

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
                         shade_code: u32,
                         sprite_tile: Option<u8>| {
        // Two-sided quad, exactly the baked-card construction (4 tris).
        let corner = |u01: f32, v01: f32| -> (glam::Vec3, [f32; 3]) {
            let p = base + up * (h * v01) + side * (w * (u01 - 0.5));
            match sprite_tile {
                Some(tile) => {
                    let enc_x = -((1 + tile) as f32 + u01 * 0.5);
                    (p, [-1.0, enc_x, v01])
                }
                // v0.1110: through the SAME `color[0] < 0` door, because the
                // marker this needs (a negative blue) cannot survive
                // pack_color_to_uv's 0..1 clamp.
                None => {
                    let uv = encode_canopy_card_uv(color, shade_code);
                    (p, [-1.0, uv[0], uv[1]])
                }
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
                });
            }
        }
    };

    let cell = TREE_CELL_RAD;
    let mut inner = FAR_TREE_NEAR_M;
    for (outer, stride, card_h, card_w, canopy_quad) in BANDS {
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
                let r2 = next();
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
                // Biome edges are GRADIENTS, not cliffs (v0.1108), and the far
                // sheet has to agree with the near harvest or the ~1200 m
                // handoff shows a density step: distant trees at full density
                // right up to a line, near trees fading across it.
                //
                // This stream draws ONE representative per cell rather than a
                // count loop, so the weight thins REPRESENTATIVES instead of
                // items - and it uses `r2`, which the fixed-six draw already
                // spends and nothing else reads, so the stream discipline that
                // keeps this sheet aligned with the bake is untouched.
                let vw = veg_biome_weight(sc);
                if vw < VEG_WEIGHT_MIN || ((r2 % 4096) as f32 / 4096.0) >= vw {
                    continue;
                }
                let r_m = radius * super::planet_surface::displaced_radius_f64(def, e as f64);
                let base = ((dir * r_m) - anchor).as_vec3();
                let up = dir.as_vec3();
                let east = glam::Vec3::Y.cross(up).normalize_or_zero();
                let north = up.cross(east).normalize_or_zero();
                let az = (r3 % 6283) as f32 / 1000.0;
                let side = east * az.cos() + north * az.sin();
                // Canopy green from the local imagery, gently darkened +
                // varied per cell (v3: 0.5x read as black slabs against
                // bright terrain - forests are darker than grass, not
                // silhouettes).
                //
                // v0.1110: the tone jitter is QUANTISED to the 4-bit code the
                // card carries, so the colour packed here and the multiplier
                // the shader applies to the imagery are the same number. This
                // colour is the no-imagery fallback; on a planet with baked
                // imagery the shader reproduces it from the fetch.
                let shade_code =
                    (r5 % 256) as u32 * (FAR_CARD_SHADE_LEVELS + 1) / 256;
                let shade = far_card_shade(shade_code);
                let color = [
                    sc[0] * FAR_CARD_CANOPY_TINT[0] * shade,
                    sc[1] * FAR_CARD_CANOPY_TINT[1] * shade,
                    sc[2] * FAR_CARD_CANOPY_TINT[2] * shade,
                ];
                // Crossed vertical cards: the grazing-view silhouette.
                emit_card(base, up, side, card_w, card_h, color, shade_code, None);
                let side2 = up.cross(side).normalize_or_zero();
                emit_card(base, up, side2, card_w, card_h, color, shade_code, None);
                if canopy_quad {
                    // Horizontal crown quad, FAR bands only (v3): its job
                    // is the from-above read at 10+ km, where vertical
                    // cards are edge-on-invisible; near the player it was
                    // a sky-blocking ceiling.
                    emit_card(
                        // Centered: the closure treats its "up" axis as
                        // 0..1, so back up half a span along side2.
                        base + up * (card_h * 0.8) - side2 * (card_w * 0.5),
                        side2,
                        side,
                        card_w,
                        card_w,
                        color,
                        shade_code,
                        None,
                    );
                }
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

    /// The marker the whole v0.1110 canopy fix hangs off. If a card's uv can
    /// be mistaken for a ground face's, the shader paints a tree with ground
    /// albedo (which is exactly what it did through v0.1109); if a ground
    /// face's uv can be mistaken for a card's, the shader paints the ground as
    /// forest. Both directions are checked.
    #[test]
    fn canopy_card_uv_round_trips_and_cannot_collide_with_a_ground_face() {
        for (c, code) in [
            ([0.0_f32, 0.0, 0.0], 0u32),
            ([1.0, 1.0, 1.0], FAR_CARD_SHADE_LEVELS),
            ([0.13, 0.29, 0.07], 7),
            ([0.62, 0.51, 0.44], 15),
        ] {
            let uv = encode_canopy_card_uv(c, code);
            let (got, got_code) = decode_canopy_card_uv(uv).expect("encodes as a card");
            assert_eq!(got_code, code.min(FAR_CARD_SHADE_LEVELS));
            for k in 0..3 {
                assert!(
                    (got[k] - c[k]).abs() <= 1.0 / 255.0 + 1e-6,
                    "channel {k}: {} -> {}",
                    c[k],
                    got[k]
                );
            }
            // Exact-integer safety: the shader rounds this back to u32.
            assert!(uv[0] <= 4_000_000.0 && uv[0].fract() == 0.0, "uv.x = {}", uv[0]);
            // Bit 17 must stay clear or the sheet distance-discards itself.
            assert_eq!(
                (uv[0] as u32) & 0x2_0000,
                0,
                "the canopy encoding set the tree-card LOD bit"
            );
        }
        // Every ordinary ground face decodes as NOT a card, including the
        // pathological blue == 0 one that a naive -blue marker would lose.
        for b in [0.0_f32, 0.5, 1.0] {
            let uv = super::super::planet_surface::pack_color_to_uv([0.3, 0.4, b], false);
            assert!(
                decode_canopy_card_uv(uv).is_none(),
                "a ground face with blue = {b} decoded as a canopy card"
            );
        }
    }

    /// A built sheet must actually carry the marker - the encoder can be
    /// perfect and still never be called (the v0.1107 inert-discard shape).
    #[test]
    fn every_sheet_card_is_marked_as_canopy() {
        let def = forest_def();
        let hm = land_heightmap();
        let cam = DVec3::new(0.6, 0.5, 0.4).normalize() * (def.radius + 500.0);
        let (mesh, _) = build_far_tree_sheet(&def, &hm, None, cam).expect("grows");
        let mut codes = std::collections::BTreeSet::new();
        for v in &mesh.vertices {
            assert!(
                v.color[0] < 0.0,
                "a sheet vertex did not take the pre-encoded uv door"
            );
            let (_, code) = decode_canopy_card_uv([v.color[1], v.color[2]])
                .expect("every sheet card carries the canopy marker");
            codes.insert(code);
        }
        assert!(
            codes.iter().all(|c| *c <= FAR_CARD_SHADE_LEVELS),
            "tone code out of range: {codes:?}"
        );
    }

    /// The tint and the tone range live in TWO languages. The shader applies
    /// them to the imagery fetch, this module bakes them into the fallback
    /// colour, and a drift makes a planet with imagery and a planet without
    /// grow visibly different forests.
    #[test]
    fn canopy_constants_match_the_shader() {
        let src = crate::renderer::shader_loader::assembled_pbr_source();
        let want_tint = format!(
            "const CANOPY_GROUND_TINT: vec3<f32> = vec3<f32>({:.2}, {:.2}, {:.2});",
            FAR_CARD_CANOPY_TINT[0], FAR_CARD_CANOPY_TINT[1], FAR_CARD_CANOPY_TINT[2]
        );
        assert!(
            src.contains(&want_tint),
            "the shader's CANOPY_GROUND_TINT does not match far_trees::FAR_CARD_CANOPY_TINT \
             (expected `{want_tint}`)"
        );
        for (name, v) in [
            ("CANOPY_CARD_SHADE_LO", FAR_CARD_SHADE_LO),
            ("CANOPY_CARD_SHADE_HI", FAR_CARD_SHADE_HI),
        ] {
            let want = format!("const {name}: f32 = {v:.2};");
            assert!(src.contains(&want), "the shader lost `{want}`");
        }
        // The shader decodes the tone from bits 18..21; this is the weight.
        assert_eq!(FAR_CARD_SHADE_BIT, 262_144.0);
        assert!(src.contains("(packed >> 18u) & 15u"));
        // And the two shade decoders agree at every code.
        for code in 0..=FAR_CARD_SHADE_LEVELS {
            let t = code as f32 / FAR_CARD_SHADE_LEVELS as f32;
            let want = FAR_CARD_SHADE_LO + (FAR_CARD_SHADE_HI - FAR_CARD_SHADE_LO) * t;
            assert!((far_card_shade(code) - want).abs() < 1e-6);
        }
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
