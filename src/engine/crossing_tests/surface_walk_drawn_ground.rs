//! From surface_walk.rs (`drawn_ground_tests`): the drawn ground against the field, through the
//! engine's frame lock (`frame_lock::ground_radius_m`).

//! THE GROUND-CONTACT GATE FOR THE PLAYER (v0.1108) - third of its family,
//! after grass (v0.1091) and near-field trees (v0.1097), and it exists for
//! exactly the same reason at the last remaining scale: the thing standing on
//! the ground was placed by a sampler that is not the mesh.
//!
//! EVERY measurement here is taken against the REAL BUILT PATCH MESH, by ray
//! casting `build_patch_mesh`'s own triangles. Measuring the clamp against
//! `DrawnPatchSurface` would be measuring the implementation against itself -
//! a check that cannot fail.
//!
//! Native-gated because it calls `engine::frame_lock::ground_radius_m`, the
//! REAL field sampler the clamp feeds in, rather than a local copy of the
//! elevation formula (a second copy is the very defect class under test).

use humanity_systems::surface_walk::*;
use crate::engine::frame_lock::ground_radius_m;
use humanity_terrain::planet_chunks::{
    build_patch_mesh, patch_corners, patch_edge_arc_m, PatchId, PatchMesh, PATCH_TESS,
};
use humanity_core::ecs::components::EYE_HEIGHT_M;
use humanity_terrain::drawn_surface::DrawnPatchSurface;
use humanity_terrain::ocean_mask::OceanMask;
use humanity_terrain::planet::PlanetDef;
use humanity_terrain::planet_chunks::DetailNoise;
use humanity_terrain::planet_chunks::ElevationSource;
use glam::DVec3;
use humanity_terrain::planet_heightmap::PlanetHeightmap;

/// The shipped Earth grids plus the real earth.ron (NOT the test fixture:
/// `planet_chunks::tests::earth_like` carries surface_relief 0.011, 3.5x
/// the shipped 0.003123, which would inflate every number below).
fn real_earth() -> (PlanetHeightmap, OceanMask, PlanetDef) {
    let base = std::path::Path::new(env!("HUMANITY_REPO_ROOT"))
        .join("data")
        .join("planets");
    let hm = PlanetHeightmap::load(&base.join("earth_heightmap.bin"))
        .expect("earth heightmap loads");
    let om = OceanMask::load(&base.join("earth_ocean_mask.bin")).expect("ocean mask loads");
    let text = std::fs::read_to_string(base.join("earth.ron")).expect("earth.ron reads");
    let mut def: PlanetDef = ron::from_str(&text).expect("earth.ron parses");
    // The loader overrides sea level with the grid's true 0 m position.
    def.sea_level = hm.sea_level_normalized();
    (hm, om, def)
}

fn dir_of(lat_deg: f64, lon_deg: f64) -> DVec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    let cl = lat.cos();
    DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin())
}

/// Real walking ground, at the depths a walker really sees: 13 is the
/// base-only cap (`MAX_PATCH_DEPTH`, 53.8 m triangles), 17 and 20 are what
/// the streamed tile tier reaches (3.36 m and 0.42 m).
const SITES: [(&str, f64, f64); 5] = [
    ("fuji-flank", 35.29, 138.79),
    ("alps-montblanc", 45.83, 6.865),
    ("himalaya-everest", 27.99, 86.93),
    ("amazon-floodplain", -3.0, -60.0),
    ("kansas-plain", 38.5, -98.0),
];

/// The patch of `depth` whose spherical triangle contains `dir`.
fn patch_containing(dir: DVec3, depth: u8) -> PatchId {
    let inside = |id: &PatchId, d: DVec3| -> bool {
        let c = patch_corners(id);
        let cn = [c[0].normalize(), c[1].normalize(), c[2].normalize()];
        let en = [cn[0].cross(cn[1]), cn[1].cross(cn[2]), cn[2].cross(cn[0])];
        let es = [en[0].dot(cn[2]), en[1].dot(cn[0]), en[2].dot(cn[1])];
        (0..3).all(|i| en[i].dot(d) * es[i] >= 0.0)
    };
    let d = dir.normalize();
    let mut id = (0..20u8)
        .map(PatchId::root)
        .find(|r| inside(r, d))
        .expect("some root face contains the direction");
    while id.depth < depth {
        let mut next = None;
        for c in 0..4u32 {
            let ch = id.child(c);
            if inside(&ch, d) {
                next = Some(ch);
                break;
            }
        }
        id = next.expect("some child contains the direction");
    }
    id
}

/// Where a ray from the planet centre along `dir` crosses the built
/// patch's GROUND, in metres of radius. Moller-Trumbore over the first
/// `PATCH_TESS^2` triangles only: the vegetation cards and the skirt come
/// after them in the index buffer, and the skirt is a near-RADIAL apron a
/// centre-outward ray grazes at an arbitrary distance.
fn drawn_radius_along(pm: &PatchMesh, dir: DVec3) -> Option<f64> {
    let d = dir.normalize();
    let v = |i: u32| -> DVec3 {
        let p = pm.mesh.vertices[i as usize].position;
        pm.anchor + DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
    };
    let grid_idx = (PATCH_TESS * PATCH_TESS * 3) as usize;
    let mut best: Option<f64> = None;
    for tri in pm.mesh.indices[..grid_idx].chunks_exact(3) {
        let (a, b, c) = (v(tri[0]), v(tri[1]), v(tri[2]));
        let (e1, e2) = (b - a, c - a);
        let h = d.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-12 {
            continue;
        }
        let inv = 1.0 / det;
        let s = -a;
        let u = inv * s.dot(h);
        if !(-1e-9..=1.0 + 1e-9).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let w = inv * d.dot(q);
        if w < -1e-9 || u + w > 1.0 + 1e-9 {
            continue;
        }
        let t = inv * e2.dot(q);
        if t > 0.0 && best.map_or(true, |bt| t > bt) {
            best = Some(t);
        }
    }
    best
}

/// Probe directions strictly INSIDE a patch: barycentric points with every
/// weight >= 0.15, so the ray cast always lands on a ground triangle
/// regardless of patch size (6.7 m at depth 20, 860 m at depth 13).
fn probes_in(id: &PatchId) -> Vec<DVec3> {
    let c = patch_corners(id);
    let mut out = Vec::new();
    for w in [
        [0.34, 0.33, 0.33],
        [0.60, 0.22, 0.18],
        [0.20, 0.62, 0.18],
        [0.18, 0.20, 0.62],
        [0.45, 0.40, 0.15],
        [0.15, 0.45, 0.40],
        [0.40, 0.15, 0.45],
    ] {
        out.push((c[0] * w[0] + c[1] * w[1] + c[2] * w[2]).normalize());
    }
    out
}

/// The eye, in metres above the ground the patch mesh actually draws, for
/// one probe direction. This is the whole clamp chain: the field sample
/// the engine feeds in, `walk_ground_radius`'s choice of reference, and
/// `rest_radius`.
#[allow(clippy::too_many_arguments)]
fn eye_above_drawn(
    def: &PlanetDef,
    hm: &PlanetHeightmap,
    dn: &DetailNoise,
    om: &OceanMask,
    pm: &PatchMesh,
    clamp_depth: u8,
    dir: DVec3,
) -> Option<f64> {
    let mesh_r = drawn_radius_along(pm, dir)?;
    let field_r = ground_radius_m(Some(def), Some(hm), Some(dn), None, dir);
    let (g, clearance) = walk_ground_radius(
        field_r,
        Some(def),
        Some(hm),
        Some(dn),
        None,
        Some(om),
        clamp_depth,
        0.0,
        dir,
    );
    Some(rest_radius(g, EYE_HEIGHT_M, clearance) - mesh_r)
}

/// The altitude gate picks the reference it says it picks, and the switch
/// between the two floors CANNOT pop: at `DRAWN_GROUND_MAX_ALT_M` the
/// camera is far above BOTH floors, so the 2.45 m difference between them
/// is invisible to the max in `clamp_above_ground`, to `settle_radius`
/// (armed within 0.5 m of rest) and to the grounded test (0.35 m).
#[test]
fn switching_ground_reference_cannot_pop() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let dir = dir_of(35.29, 138.79);
    let field_r = ground_radius_m(Some(&def), Some(&hm), Some(&dn), None, dir);
    let at = |alt: f64| {
        walk_ground_radius(
            field_r,
            Some(&def),
            Some(&hm),
            Some(&dn),
            None,
            Some(&om),
            17,
            alt,
            dir,
        )
    };
    // THE SWITCH IS A STEP, AND THAT IS THE POINT OF THE NAME.
    //
    // This block used to assert the two clearances at either side of the
    // gate and stop there, which is a test asserting that the pop EXISTS
    // under a name promising it cannot. The operator found what that
    // missed within a day: "I kind of teleport up then fall", reproducible
    // standing perfectly still, because the drawn depth flickers to 0 as
    // patches stream and the rest height jumped by the 2.45 m difference.
    //
    // The step is unavoidable here - this function is stateless and the two
    // references genuinely differ - so what is asserted now is the SIZE of
    // the discontinuity the caller must ease, and the fact that it is a
    // rise (falling back is always MORE conservative, never less). The
    // easing itself lives at the call site in lib.rs, which rate-limits
    // clearance whether the player is moving or not.
    let step = LOD_CLEARANCE_M - DRAWN_CLEARANCE_M;
    assert!(
        step > 0.0,
        "the fallback must be the MORE conservative reference, else a \
         switch can drop the eye into the drawn ground"
    );
    assert!(
        step < 3.0,
        "the reference switch is a {step} m step in stand height. The call \
         site eases it at 2 m/s, so anything much larger is a visible ride \
         upward - re-derive both constants rather than raising this bound."
    );
    assert_eq!(at(DRAWN_GROUND_MAX_ALT_M + 0.001).1, LOD_CLEARANCE_M, "above the gate");
    assert_eq!(at(DRAWN_GROUND_MAX_ALT_M).1, DRAWN_CLEARANCE_M, "at the gate");
    assert_eq!(at(0.0).1, DRAWN_CLEARANCE_M, "standing");
    // Depth 0 (no selection yet) always falls back, at any altitude.
    assert_eq!(
        walk_ground_radius(
            field_r,
            Some(&def),
            Some(&hm),
            Some(&dn),
            None,
            Some(&om),
            0,
            0.0,
            dir
        ),
        (field_r, LOD_CLEARANCE_M),
        "unknown drawn depth must keep the v0.889 fallback"
    );
    // The no-pop margin: the switch happens where the camera clears the
    // HIGHER of the two floors by a wide margin. 3x is arbitrary only in
    // being generous; 1x would already be pop-free.
    let (g_lo, _) = at(0.0);
    let (g_hi, _) = at(DRAWN_GROUND_MAX_ALT_M + 1.0);
    let highest_floor = (g_lo + EYE_HEIGHT_M + DRAWN_CLEARANCE_M)
        .max(g_hi + EYE_HEIGHT_M + LOD_CLEARANCE_M);
    let camera_r = field_r + DRAWN_GROUND_MAX_ALT_M;
    assert!(
        camera_r - highest_floor > 3.0 * (LOD_CLEARANCE_M - DRAWN_CLEARANCE_M),
        "the gate altitude ({DRAWN_GROUND_MAX_ALT_M} m) is close enough to the floor that \
         switching reference can yank the camera: only {:.2} m of margin",
        camera_r - highest_floor
    );
}

/// THE GATE. Standing rests the eye ONE BODY HEIGHT above the ground the
/// player can see - not 4.2 m above it, which is what
/// `EYE_HEIGHT_M + LOD_CLEARANCE_M` was doing while the sward under it is
/// 0.24-0.52 m tall. Fails by ~2.5 m on the pre-v0.1108 clamp.
#[test]
fn eye_stands_one_body_height_above_the_drawn_ground() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap {
        hm: &hm,
        detail: &dn,
        tiles: None,
        ocean: Some(&om),
    };
    let mut worst = 0.0_f64;
    let mut worst_where = String::new();
    for depth in [13u8, 17, 20] {
        for (name, lat, lon) in SITES {
            let id = patch_containing(dir_of(lat, lon), depth);
            let pm = build_patch_mesh(&def, &src, None, &id);
            let mut heights = Vec::new();
            for dir in probes_in(&id) {
                if let Some(h) = eye_above_drawn(&def, &hm, &dn, &om, &pm, depth, dir) {
                    heights.push(h);
                }
            }
            assert!(
                heights.len() >= 5,
                "{name} depth {depth}: only {} probes hit the built mesh",
                heights.len()
            );
            let mean = heights.iter().sum::<f64>() / heights.len() as f64;
            let err = heights
                .iter()
                .fold(0.0_f64, |m, h| m.max((h - EYE_HEIGHT_M).abs()));
            println!(
                "[eye height] depth {depth} ({:5.2} m tri) {name:>18}: eye {mean:.3} m above \
                 the drawn ground (worst probe off by {err:.3} m)",
                patch_edge_arc_m(depth, def.radius) / PATCH_TESS as f64
            );
            if err > worst {
                worst = err;
                worst_where = format!("{name} depth {depth}");
            }
        }
    }
    assert!(
        worst < 0.25,
        "the eye sits {worst:.3} m off a 1.7 m stand height at {worst_where} - the clamp is \
         not resting on the DRAWN patch surface (a 2.5 m error means it is back on the \
         elevation field plus LOD_CLEARANCE_M, i.e. the 2.5 m stilt)"
    );
}

/// THE v0.889 REGRESSION GUARD: the eye must NEVER end up below the drawn
/// ground ("seeing through the Earth while standing on it"), and that has
/// to be impossible rather than merely unlikely.
///
/// So it clamps with a DELIBERATELY WRONG drawn depth and still demands
/// real headroom. Two failure modes, two budgets:
///
///   * ONE level stale - the realistic case. The depth proxy is last
///     frame's selection, and an LOD crossfade keeps drawing the coarser
///     patch for `FADE_SECONDS` (0.30 s) after the swap. Must keep most of
///     a stand height: > 1.0 m.
///   * TWO levels stale - the paranoid case. The world-entry ladder climbs
///     several depths in a handful of frames, so a two-level jump between
///     one frame's selection and the next is reachable. Must still clear
///     the near plane the ground is drawn with (the celestial pass's, 5 cm,
///     terrain/celestial_projection.rs CELESTIAL_NEAR_M) by a wide margin: > 0.3 m.
///
/// Both passing with a 5 cm clearance is the evidence that
/// `DRAWN_CLEARANCE_M` does not need to be metres: the 1.7 m eye height is
/// itself the safety margin, because the worst measured one-level surface
/// gap (0.58 m) is a third of it.
#[test]
fn eye_never_sinks_below_the_drawn_ground() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap {
        hm: &hm,
        detail: &dn,
        tiles: None,
        ocean: Some(&om),
    };
    for (wrong_by, floor_m) in [(1u8, 1.0_f64), (2, 0.3)] {
        let mut lowest = f64::MAX;
        let mut lowest_where = String::new();
        for depth in [13u8, 17, 20] {
            for (name, lat, lon) in SITES {
                let id = patch_containing(dir_of(lat, lon), depth);
                let pm = build_patch_mesh(&def, &src, None, &id);
                for dir in probes_in(&id) {
                    let Some(h) =
                        eye_above_drawn(&def, &hm, &dn, &om, &pm, depth - wrong_by, dir)
                    else {
                        continue;
                    };
                    if h < lowest {
                        lowest = h;
                        lowest_where = format!("{name} depth {depth}");
                    }
                }
            }
        }
        println!(
            "[eye floor] drawn depth {wrong_by} level(s) stale: eye still {lowest:.3} m above \
             the drawn ground at worst ({lowest_where}), floor {floor_m:.1} m"
        );
        assert!(
            lowest > floor_m,
            "clamping {wrong_by} LOD level(s) stale left the eye only {lowest:.3} m above the \
             drawn ground at {lowest_where} (floor {floor_m:.1} m) - this is the road back to \
             the v0.889 clip. Raise DRAWN_CLEARANCE_M or tighten the depth proxy."
        );
    }
}

/// The fix is the REFERENCE SURFACE, not a tuned offset, so prove the two
/// references really do disagree on the ground the player stands on - and
/// by how much, because those numbers are what sized both constants.
/// Without this a later "simplification" back to the field sample would
/// only turn the gate above red with no explanation.
#[test]
fn drawn_ground_beats_the_field_and_bounds_both_clearances() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap {
        hm: &hm,
        detail: &dn,
        tiles: None,
        ocean: Some(&om),
    };
    let (mut worst_gap, mut worst_level) = (0.0_f64, 0.0_f64);
    for depth in [13u8, 17, 20] {
        for (name, lat, lon) in SITES {
            let c = dir_of(lat, lon);
            let east = DVec3::new(-c.z, 0.0, c.x).normalize();
            let north = c.cross(east).normalize();
            let mut fine = DrawnPatchSurface::new_single_shot(&def, &src, depth);
            let mut coarse = DrawnPatchSurface::new_single_shot(&def, &src, depth - 1);
            let (mut lo, mut hi, mut lvl) = (0.0_f64, 0.0_f64, 0.0_f64);
            // A 60 m x 60 m walking neighbourhood, sampled every 3 m.
            for i in -10..=10 {
                for j in -10..=10 {
                    let d = (c
                        + east * (i as f64 * 3.0 / def.radius)
                        + north * (j as f64 * 3.0 / def.radius))
                        .normalize();
                    let g = fine.radius_at(d);
                    let delta = g - ground_radius_m(Some(&def), Some(&hm), Some(&dn), None, d);
                    lo = lo.min(delta);
                    hi = hi.max(delta);
                    lvl = lvl.max((g - coarse.radius_at(d)).abs());
                }
            }
            println!(
                "[drawn vs field] depth {depth} {name:>18}: drawn-field {lo:+.3} .. {hi:+.3} m \
                 | worst one-LOD-level gap {lvl:.3} m"
            );
            worst_gap = worst_gap.max(lo.abs()).max(hi);
            worst_level = worst_level.max(lvl);
        }
    }
    assert!(
        worst_gap > 0.2,
        "the elevation field is only {worst_gap:.3} m from the drawn mesh anywhere tested, so \
         the defect this whole machinery exists for is gone - re-derive the note on \
         DRAWN_CLEARANCE_M before simplifying anything back to a field sample"
    );
    assert!(
        worst_gap < 1.0,
        "the drawn mesh now misses the field by {worst_gap:.3} m; LOD_CLEARANCE_M (2.5 m) was \
         sized for this and the fallback path may need re-deriving"
    );
    assert!(
        worst_level < EYE_HEIGHT_M * 0.5,
        "a one-level LOD miss now costs {worst_level:.3} m, more than half the {EYE_HEIGHT_M} \
         m eye height - DRAWN_CLEARANCE_M's 5 cm is only defensible while this stays small"
    );
}

/// Per-frame cost of the clamp's ground lookup, and where it goes
/// (benchmark, not a gate - `cargo test --release ... -- --ignored
/// --nocapture`; a debug number is meaningless for a memset-bound call).
///
/// The three timings exist to size ONE decision: `DrawnPatchSurface::new`
/// allocates a 65,536-slot vertex memo (~2.6 MB) built for a harvest of
/// thousands of queries, and the clamp asks exactly one question per
/// frame. Splitting construct from query says how much a single-shot
/// constructor would be worth.
#[test]
#[ignore = "benchmark"]
fn drawn_lookup_cost() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap {
        hm: &hm,
        detail: &dn,
        tiles: None,
        ocean: Some(&om),
    };
    let c = dir_of(35.29, 138.79);
    let east = DVec3::new(-c.z, 0.0, c.x).normalize();
    // A walking transect: 2 cm per step is one frame at 1.2 m/s.
    let path: Vec<DVec3> = (0..2000)
        .map(|k| (c + east * (k as f64 * 0.02 / def.radius)).normalize())
        .collect();
    for d in path.iter().take(20) {
        std::hint::black_box(DrawnPatchSurface::new_single_shot(&def, &src, 17).radius_at(*d));
    }
    let t0 = std::time::Instant::now();
    for d in &path {
        std::hint::black_box(walk_ground_radius(
            0.0,
            Some(&def),
            Some(&hm),
            Some(&dn),
            None,
            Some(&om),
            17,
            0.0,
            *d,
        ));
    }
    let whole = t0.elapsed().as_secs_f64() / path.len() as f64 * 1e6;
    let t1 = std::time::Instant::now();
    for _ in &path {
        std::hint::black_box(DrawnPatchSurface::new_single_shot(&def, &src, 17).samples);
    }
    let ctor = t1.elapsed().as_secs_f64() / path.len() as f64 * 1e6;
    let mut reused = DrawnPatchSurface::new_single_shot(&def, &src, 17);
    let t2 = std::time::Instant::now();
    for d in &path {
        std::hint::black_box(reused.radius_at(*d));
    }
    let query = t2.elapsed().as_secs_f64() / path.len() as f64 * 1e6;
    println!(
        "[clamp cost] walk_ground_radius {whole:.1} us/frame = construct {ctor:.1} us + \
         query {query:.2} us. A single-shot constructor would leave {query:.2} us."
    );
}
