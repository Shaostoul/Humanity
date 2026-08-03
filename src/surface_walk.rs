//! Planetary surface FPS math (task #76 increment 1) - pure helpers for the
//! surface-oriented camera and the stand/walk ground clamp.
//!
//! WHY: away from the ship the camera's "up" is world-Y, so at a tilted point
//! on the globe the horizon tilts and the player floats. On a planet surface
//! "down" must point at the body's CENTER (the gravity well). These helpers
//! build a TANGENT-frame camera basis whose up is the local radial (the
//! direction from the body centre to the camera): yaw then spins around the
//! radial and pitch tilts toward/away from the ground, so looking down points
//! at the planet centre and the horizon is level regardless of where on the
//! sphere you stand.
//!
//! Everything here is pure glam (no GPU, no winit, no cfg gate), so it
//! compiles in every feature set and is fully unit-tested headless. The main
//! loop (native `lib.rs`) and the camera (`renderer::camera`) call in; they
//! own the state (which body, current spin, ship_world_pos) while this module
//! owns the geometry.

use glam::{DVec3, Vec3};

use crate::terrain::drawn_surface::DrawnPatchSurface;
use crate::terrain::ocean_mask::OceanMask;
use crate::terrain::planet::PlanetDef;
use crate::terrain::planet_chunks::{DetailNoise, ElevationSource};
use crate::terrain::terrain_tiles::TerrainTiles;

/// Standing eye height above the ground (metres): the radial distance the
/// stand/walk clamp rests the camera at above the sampled ground radius.
pub const EYE_HEIGHT_M: f64 = 1.7;

/// Clearance above the ground when the only reference available is the
/// elevation FIELD (v0.889, and since v0.1108 the FALLBACK ONLY).
///
/// The field is not the mesh: the clamp samples the full-detail elevation,
/// but the patch actually DRAWN under the player is a lattice of that field
/// linearly interpolated, and on a ridge the drawn chord can sit above the
/// fine model - the "seeing through the Earth while standing on it" clip.
/// With the drawn depth UNKNOWN (before the first patch selection, on a
/// planet with no chunk state, while the LOD ladder is still climbing from
/// depth 1 during world entry) the mismatch is unbounded: at depth 5 a
/// triangle is 1.7 km across and its chord can miss a peak by hundreds of
/// metres, so this stays generous. It is NOT the standing case any more -
/// see `DRAWN_CLEARANCE_M`.
pub const LOD_CLEARANCE_M: f64 = 2.5;

/// Clearance above the DRAWN patch surface (v0.1108).
///
/// THE 2.5 m STILT. Until now both the rest height and the floor were
/// `ground + eye + LOD_CLEARANCE_M`, so the eye sat 4.2 m above a sward of
/// 0.24-0.52 m blades: a first-floor window, not a pair of eyes. Every
/// ground-cover figure computed against 1.7 m was inflated ~2.5x with it,
/// which is why no amount of grass tuning ever made the near field read.
///
/// Standing on the DRAWN surface dissolves the problem instead of trading
/// against it. `DrawnPatchSurface` reproduces `build_patch_mesh` exactly -
/// same lattice, same depth-gated elevation, same flat triangle - so the
/// mismatch the old slop covered is gone by construction, and this residual
/// only has to absorb the ONE-LEVEL uncertainty in the drawn depth (the
/// depth proxy is last frame's selection, and an LOD crossfade draws the
/// coarser patch for another 0.30 s).
///
/// MEASURED, so the number is not a guess (`drawn_ground_beats_the_field`,
/// five real sites x 1,681 samples each at depths 13/17/20):
///   * drawn vs field, the disagreement the 2.5 m was covering: worst
///     +0.46 m, i.e. the old constant was ~5x oversized;
///   * drawn(depth) vs drawn(depth-1), the residual this constant covers:
///     worst 0.58 m at depth 13, 0.46 m at 17, 0.32 m at 20.
///
/// 5 cm rather than 60 cm because a wrong-by-one-level depth CANNOT put the
/// eye underground: the worst gap measured is 0.58 m against a 1.7 m eye, so
/// even a two-level miss leaves a metre of headroom (the
/// `eye_never_sinks_below_the_drawn_ground` gate proves it by deliberately
/// clamping with the wrong depth). This is numerical slop, not LOD slop.
pub const DRAWN_CLEARANCE_M: f64 = 0.05;

/// Altitude (metres above the ground) past which the clamp stops consulting
/// the drawn surface and goes back to the field sample.
///
/// NOT a fidelity knob - a blast-radius bound on a known cost (see the note
/// in `walk_ground_radius`). Standing on the drawn triangle matters when your
/// feet are on it; 40 m up in the flight band the clamp is only a floor you
/// are nowhere near, and paying 0.6 ms a frame to place it to the centimetre
/// is pure waste.
///
/// It CANNOT pop, and that is a property of the number, not an argument: the
/// two floors differ by `LOD_CLEARANCE_M - DRAWN_CLEARANCE_M` = 2.45 m, and
/// switching between them is only visible to a camera sitting ON the floor.
/// At 40 m the floor is ~38 m below the camera, so neither `clamp_above_ground`
/// (a max), `settle_radius` (armed within 0.5 m of rest) nor the grounded test
/// (within 0.35 m) can be touching it. `switching_ground_reference_cannot_pop`
/// holds the margin.
pub const DRAWN_GROUND_MAX_ALT_M: f64 = 40.0;

/// WHERE THE PLAYER STANDS: the radius of the ground under `unit_dir`, and
/// the clearance that belongs with that reference. Returns
/// `(ground_radius_m, clearance_m)` for `rest_radius` / `clamp_above_ground`.
///
/// Prefers the DRAWN patch surface - the triangle you can actually see -
/// falling back to `field_r` (the caller's elevation-field sample, i.e.
/// `engine::frame_lock::ground_radius_m`) whenever the drawn mesh cannot be
/// reproduced or cannot matter:
///   * `drawn_depth == 0`: no patch selection yet, so which mesh is drawn
///     here is unknown;
///   * no planet def / heightmap / detail noise: not a chunked planet;
///   * a water world whose connected-ocean mask has not loaded: the patch
///     builder is then drawing the SEA-CLAMPED surface while the field
///     reports true bathymetry, and taking the drawn value would stand a
///     diver on the water instead of the seafloor. Above sea level the two
///     displacement formulas agree exactly, so this only ever costs the
///     brief window before the mask arrives;
///   * `alt_m` above `DRAWN_GROUND_MAX_ALT_M`: flying, not standing.
pub fn walk_ground_radius(
    field_r: f64,
    def: Option<&PlanetDef>,
    hm: Option<&crate::terrain::planet_heightmap::PlanetHeightmap>,
    detail: Option<&DetailNoise>,
    tiles: Option<&TerrainTiles>,
    ocean: Option<&OceanMask>,
    drawn_depth: u8,
    alt_m: f64,
    unit_dir: DVec3,
) -> (f64, f64) {
    let fallback = (field_r, LOD_CLEARANCE_M);
    if drawn_depth == 0 || alt_m > DRAWN_GROUND_MAX_ALT_M || unit_dir.length_squared() < 0.5 {
        return fallback;
    }
    let (Some(d), Some(h), Some(dn)) = (def, hm, detail) else {
        return fallback;
    };
    if d.has_water && ocean.is_none() {
        return fallback;
    }
    // Same source the patch builder was handed this frame, so this IS the
    // mesh under the player and not a second opinion about it.
    //
    // KNOWN COST, measured not guessed (`drawn_lookup_cost`, release):
    // 604.6 us per call, of which 597.1 us is `DrawnPatchSurface::new`
    // allocating and zeroing a 65,536-slot (~2.6 MB) vertex memo and 0.25 us
    // is the actual query. That memo exists for the grass/tree HARVESTS, which
    // ask it thousands of questions; the clamp asks one, so 99.96% of the cost
    // is a page-fault storm for nothing. The fix is a single-shot constructor
    // with a small memo in `terrain::drawn_surface` (filed as a wiring request
    // rather than done here - that file is another lane's), which takes this
    // to 0.25 us. Until then `DRAWN_GROUND_MAX_ALT_M` bounds who pays it to a
    // camera actually near the ground. Do NOT "fix" it by caching a stale
    // offset off the field sample: that trades away the exactness this whole
    // change exists for.
    let src = ElevationSource::Heightmap { hm: h, detail: dn, tiles, ocean };
    let r = DrawnPatchSurface::new_single_shot(d, &src, drawn_depth).radius_at(unit_dir);
    if r.is_finite() && r > 0.0 {
        (r, DRAWN_CLEARANCE_M)
    } else {
        fallback
    }
}

/// Build the orthonormal tangent basis `(east, north)` for a given radial
/// `up`. Pole-safe: near the poles `up` is nearly parallel to world-Y, so the
/// world reference axis switches to world-X to keep the cross product well
/// conditioned. Returns unit vectors with `east x north = up` (a right-handed
/// frame in the order east, north, up).
pub fn tangent_basis(up: Vec3) -> (Vec3, Vec3) {
    let up = up.normalize_or_zero();
    if up.length_squared() < 0.5 {
        // Degenerate up: fall back to the world axes so callers never NaN.
        return (Vec3::X, Vec3::Z);
    }
    // Reference axis: world-Y, unless up hugs the pole (then world-X).
    let world_ref = if up.dot(Vec3::Y).abs() > 0.999 { Vec3::X } else { Vec3::Y };
    let east = world_ref.cross(up).normalize_or_zero();
    let north = up.cross(east); // unit already (up perp east, both unit)
    (east, north)
}

/// Forward direction of a surface-oriented camera: `up` is the local radial,
/// `yaw` spins around it (in the tangent plane) and `pitch` tilts toward the
/// zenith (positive) or the ground (negative). At pitch 0 the forward lies in
/// the tangent plane, so the horizon is level; at negative pitch the forward
/// gains a `-up` component, i.e. it points below the horizon toward the body
/// centre.
pub fn surface_forward(up: Vec3, yaw: f32, pitch: f32) -> Vec3 {
    let up = up.normalize_or_zero();
    let (east, north) = tangent_basis(up);
    let horizontal = north * yaw.cos() + east * yaw.sin();
    (horizontal * pitch.cos() + up * pitch.sin()).normalize_or_zero()
}

/// Inverse of `surface_forward`: the (yaw, pitch) that aim a surface-oriented
/// camera with radial `up` along `dir`. Used so a scenic capture or a mode
/// transition can preserve a desired look direction across the world-Y ->
/// radial basis change. Pitch is clamped just inside +-90 degrees (the same
/// limit mouse-look enforces).
pub fn surface_look_angles(up: Vec3, dir: Vec3) -> (f32, f32) {
    let up = up.normalize_or_zero();
    let d = dir.normalize_or_zero();
    if up.length_squared() < 0.5 || d.length_squared() < 0.5 {
        return (0.0, 0.0);
    }
    let (east, north) = tangent_basis(up);
    let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
    let pitch = d.dot(up).clamp(-1.0, 1.0).asin().clamp(-max_pitch, max_pitch);
    // horizontal = cos(yaw)*north + sin(yaw)*east  =>  yaw = atan2(e, n).
    let yaw = d.dot(east).atan2(d.dot(north));
    (yaw, pitch)
}

/// Inverse of the WORLD-Y camera `forward()` (yaw about world-Y, pitch about
/// world-X): `forward = (sin yaw cos pitch, sin pitch, -cos yaw cos pitch)`,
/// so yaw = atan2(x, -z) and pitch = asin(y). Used when LEAVING surface mode
/// to preserve the look direction back into the default basis. Mirrors
/// `dev_travel::look_angles` but for a Vec3.
pub fn world_look_angles(dir: Vec3) -> (f32, f32) {
    let d = dir.normalize_or_zero();
    if d.length_squared() < 0.5 {
        return (0.0, 0.0);
    }
    let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
    let pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-max_pitch, max_pitch);
    let yaw = d.x.atan2(-d.z);
    (yaw, pitch)
}

/// Radial distance the camera rests at when standing: the ground radius plus
/// one eye height plus the clearance that belongs with that ground reference.
/// `clearance` comes from `walk_ground_radius` alongside the radius itself -
/// the two are a pair and passing one without the other is the v0.889 /
/// v0.1108 bug in both directions.
pub fn rest_radius(ground_r: f64, eye_height: f64, clearance: f64) -> f64 {
    ground_r + eye_height + clearance
}

/// Never sink below standing height: clamp a radial distance so the eye stays
/// at least `eye_height` (+ `clearance`) above the ground radius. Shares the
/// floor with `rest_radius` so settle never fights the clamp.
pub fn clamp_above_ground(r: f64, ground_r: f64, eye_height: f64, clearance: f64) -> f64 {
    r.max(ground_r + eye_height + clearance)
}

/// Ease a radial distance toward its rest height (gravity): exponential decay
/// at `rate` per second, clamped so the result is never below `rest` (you
/// settle onto the ground, you do not tunnel through it). If already at or
/// below rest, snap up to rest.
/// NOTE v0.1005: this is now only the GROUNDED fine-tune (breathing terrain
/// under a standing player). Airborne motion is real ballistics via
/// `vertical_step` below - the operator's "falls extremely fast... not
/// remotely like gravity" was this exponential snap being applied to
/// kilometre falls.
pub fn settle_radius(current: f64, rest: f64, dt: f64, rate: f64) -> f64 {
    if current <= rest {
        return rest;
    }
    let eased = rest + (current - rest) * (-rate * dt.max(0.0)).exp();
    eased.max(rest)
}

/// Human terminal velocity in a thick atmosphere (m/s, belly-to-earth).
/// The free-fall clamp `vertical_step` applies; powered descent (thrust
/// down) may exceed it deliberately.
pub const TERMINAL_FALL_MPS: f64 = 55.0;

/// How hard vertical thrust pulls the velocity toward its target, as a
/// multiple of the local gravity (with a floor so tiny bodies still feel
/// responsive). ~3g reaches a 50 m/s climb in under 2 s while still
/// reading as a spool-up rather than a teleport.
pub const THRUST_ACCEL_G_MULT: f64 = 3.0;
pub const THRUST_ACCEL_MIN: f64 = 20.0;
/// The ramp must also SCALE with the commanded rate (v0.1006, operator:
/// "space bar seems fixed speed now... the mouse wheel doesn't seem to
/// work"): a fixed 3g toward a geared 400 m/s target made the gear
/// irrelevant for the first half minute. Whatever the target, the spool
/// reaches it in about this many seconds.
pub const THRUST_RAMP_S: f64 = 1.5;

/// One step of real vertical ballistics (v0.1005, operator: "It's not
/// remotely like gravity. The up speed is super slow while the down speed
/// is always very fast").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalStep {
    pub r: f64,
    pub v_r: f64,
    /// Landed on (or resting at) standing height this step.
    pub grounded: bool,
}

/// Integrate the walk band's radial motion for one frame.
///
/// - `thrust_mps`: target vertical rate from input (+ = climb from Space,
///   - = powered descent from Shift, 0 = no vertical input). The velocity
///   RAMPS toward it at ~3g (a jetpack spool, not a teleport), and the
///   same ramp brakes a fall when you thrust against it.
/// - No input: gravity `g` accelerates the fall, clamped at `terminal`
///   (real free-fall: ~4.5 s and 55 m/s from a 100 m drop, minutes from
///   the band ceiling - not the old fixed-rate snap).
/// - Landing at `rest` zeroes the velocity; standing there stays put.
pub fn vertical_step(
    r: f64,
    v_r: f64,
    rest: f64,
    g: f64,
    dt: f64,
    thrust_mps: f64,
    terminal: f64,
) -> VerticalStep {
    let dt = dt.max(0.0);
    let g = g.max(0.0);
    let mut v = v_r;
    if thrust_mps.abs() > 1e-9 {
        // Powered: ramp toward the commanded rate from EITHER side (also
        // how an upward burn arrests a fall). The rate scales with the
        // command so a geared-up target is reached in ~THRUST_RAMP_S
        // seconds instead of pinning everything to the 3g floor.
        let accel = (g * THRUST_ACCEL_G_MULT)
            .max(THRUST_ACCEL_MIN)
            .max(thrust_mps.abs() / THRUST_RAMP_S.max(1e-6));
        let dv = accel * dt;
        if v < thrust_mps {
            v = (v + dv).min(thrust_mps);
        } else {
            v = (v - dv).max(thrust_mps);
        }
    } else {
        // Free fall, terminal-velocity clamped. (An upward coast also
        // decays through zero into a fall - the ballistic arc.)
        v = (v - g * dt).max(-terminal.abs());
    }
    let mut r_new = r + v * dt;
    let mut grounded = false;
    if r_new <= rest {
        r_new = rest;
        // Ground stops downward motion; an upward thrust may lift off the
        // same frame it starts, so never clamp a positive velocity.
        if v < 0.0 {
            v = 0.0;
        }
        grounded = v <= 0.0;
    }
    VerticalStep { r: r_new, v_r: v, grounded }
}

/// THE GROUND-CONTACT GATE FOR THE PLAYER (v0.1108) - third of its family,
/// after grass (v0.1091) and near-field trees (v0.1097), and it exists for
/// exactly the same reason at the last remaining scale: the thing standing on
/// the ground was placed by a sampler that is not the mesh.
///
/// EVERY measurement here is taken against the REAL BUILT PATCH MESH, by ray
/// casting `build_patch_mesh`'s own triangles. Measuring the clamp against
/// `DrawnPatchSurface` would be measuring the implementation against itself -
/// a check that cannot fail.
///
/// Native-gated because it calls `engine::frame_lock::ground_radius_m`, the
/// REAL field sampler the clamp feeds in, rather than a local copy of the
/// elevation formula (a second copy is the very defect class under test).
#[cfg(all(test, feature = "native"))]
mod drawn_ground_tests {
    use super::*;
    use crate::engine::frame_lock::ground_radius_m;
    use crate::terrain::planet_chunks::{
        build_patch_mesh, patch_corners, patch_edge_arc_m, PatchId, PatchMesh, PATCH_TESS,
    };
    use crate::terrain::planet_heightmap::PlanetHeightmap;

    /// The shipped Earth grids plus the real earth.ron (NOT the test fixture:
    /// `planet_chunks::tests::earth_like` carries surface_relief 0.011, 3.5x
    /// the shipped 0.003123, which would inflate every number below).
    fn real_earth() -> (PlanetHeightmap, OceanMask, PlanetDef) {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
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
    ///     the 0.1 m camera near plane by a wide margin: > 0.3 m.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vec3, b: Vec3, eps: f32) -> bool {
        (a - b).length() < eps
    }

    #[test]
    fn tangent_basis_is_orthonormal_and_right_handed() {
        for up in [
            Vec3::Y,
            Vec3::X,
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-3.0, 2.0, 5.0),
            Vec3::new(0.2, -0.9, 0.1),
        ] {
            let up_n = up.normalize();
            let (east, north) = tangent_basis(up);
            assert!((east.length() - 1.0).abs() < 1e-5, "east not unit for {up}");
            assert!((north.length() - 1.0).abs() < 1e-5, "north not unit for {up}");
            assert!(east.dot(up_n).abs() < 1e-5, "east not perp up for {up}");
            assert!(north.dot(up_n).abs() < 1e-5, "north not perp up for {up}");
            assert!(east.dot(north).abs() < 1e-5, "east not perp north for {up}");
            // Right-handed in the order (east, north, up).
            assert!(approx(east.cross(north), up_n, 1e-4), "handedness wrong for {up}");
        }
    }

    #[test]
    fn tangent_basis_is_pole_safe() {
        // Almost straight up (and its mirror): must not collapse to zero.
        for up in [Vec3::new(0.0, 1.0, 1e-6), Vec3::new(1e-7, -1.0, 0.0)] {
            let (east, north) = tangent_basis(up);
            assert!(east.is_finite() && north.is_finite());
            assert!((east.length() - 1.0).abs() < 1e-4, "east collapsed at the pole");
            assert!((north.length() - 1.0).abs() < 1e-4, "north collapsed at the pole");
        }
    }

    #[test]
    fn horizon_is_level_at_pitch_zero() {
        // At pitch 0 the forward has zero radial component for ANY up, so the
        // view lies in the tangent plane: the horizon reads level.
        for up in [Vec3::Y, Vec3::new(1.0, 0.3, -0.5), Vec3::new(-2.0, 5.0, 1.0)] {
            let up_n = up.normalize();
            for yaw in [-2.0f32, -0.5, 0.0, 1.0, 3.0] {
                let fwd = surface_forward(up, yaw, 0.0);
                assert!(
                    fwd.dot(up_n).abs() < 1e-5,
                    "pitch-0 forward not tangent: up={up} yaw={yaw} dot={}",
                    fwd.dot(up_n)
                );
            }
        }
    }

    #[test]
    fn looking_down_points_below_the_horizon_toward_center() {
        // Negative pitch => forward gains a -up component => points at the
        // ground/body centre. Positive pitch => toward the zenith.
        for up in [Vec3::Y, Vec3::new(3.0, 1.0, -2.0), Vec3::new(0.0, -1.0, 0.5)] {
            let up_n = up.normalize();
            let down = surface_forward(up, 0.7, -0.6);
            assert!(down.dot(up_n) < -0.4, "negative pitch did not look down: {}", down.dot(up_n));
            let upward = surface_forward(up, 0.7, 0.6);
            assert!(upward.dot(up_n) > 0.4, "positive pitch did not look up: {}", upward.dot(up_n));
        }
    }

    #[test]
    fn look_angles_reproduce_the_direction() {
        // surface_look_angles must invert surface_forward for the given up.
        let ups = [Vec3::Y, Vec3::new(1.0, 2.0, -3.0), Vec3::new(-4.0, 1.0, 0.5)];
        for up in ups {
            for (yaw, pitch) in [(0.0f32, 0.0f32), (1.2, -0.3), (-2.5, 0.8), (2.0, -1.4)] {
                let dir = surface_forward(up, yaw, pitch);
                let (ry, rp) = surface_look_angles(up, dir);
                let rebuilt = surface_forward(up, ry, rp);
                assert!(
                    approx(rebuilt, dir, 1e-4),
                    "look_angles missed: up={up} yaw={yaw} pitch={pitch} rebuilt={rebuilt} dir={dir}"
                );
            }
        }
    }

    #[test]
    fn world_look_angles_matches_world_forward() {
        // world_look_angles must invert the default camera forward formula.
        let world_forward = |yaw: f32, pitch: f32| {
            Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()).normalize()
        };
        for (yaw, pitch) in [(0.3f32, 0.2f32), (-1.7, -0.5), (2.9, 0.9)] {
            let dir = world_forward(yaw, pitch);
            let (ry, rp) = world_look_angles(dir);
            assert!(approx(world_forward(ry, rp), dir, 1e-4), "world inverse missed");
        }
    }

    #[test]
    fn ground_clamp_never_sinks_below_standing_height() {
        let ground = 6.371e6;
        let eye = EYE_HEIGHT_M;
        // Both clearances, because both are live: DRAWN_CLEARANCE_M whenever
        // the drawn patch depth is known (the standing case) and
        // LOD_CLEARANCE_M as the unknown-depth fallback.
        for slop in [DRAWN_CLEARANCE_M, LOD_CLEARANCE_M] {
            let floor = ground + eye + slop;
            // Below ground snaps up to standing height.
            assert_eq!(clamp_above_ground(ground - 100.0, ground, eye, slop), floor);
            // Above the floor is left alone.
            assert_eq!(
                clamp_above_ground(ground + 50.0, ground, eye, slop),
                ground + 50.0
            );
            // Rest and clamp share the floor so settle never fights the clamp.
            assert_eq!(rest_radius(ground, eye, slop), floor);
        }
        // The standing floor is a BODY HEIGHT off the ground, not a storey
        // (v0.1108): 1.75 m, not 4.20 m.
        assert!(
            rest_radius(ground, eye, DRAWN_CLEARANCE_M) - ground < 2.0,
            "the standing eye is back on a stilt"
        );
    }

    #[test]
    fn free_fall_accelerates_at_g_and_caps_at_terminal() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let mut r = rest + 10_000.0;
        let mut v = 0.0;
        let dt = 1.0 / 60.0;
        // After 2 s of free fall: v ~ -g*t (well under terminal).
        for _ in 0..120 {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
        }
        assert!((v + 9.81 * 2.0).abs() < 0.5, "2 s fall should be ~ -19.6 m/s, got {v}");
        // Long fall: clamps at terminal, never faster.
        for _ in 0..1200 {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            assert!(v >= -TERMINAL_FALL_MPS - 1e-9, "fell past terminal: {v}");
        }
        assert!((v + TERMINAL_FALL_MPS).abs() < 1e-6, "did not reach terminal: {v}");
    }

    #[test]
    fn hundred_metre_drop_takes_real_seconds_not_a_snap() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let mut r = rest + 100.0;
        let mut v = 0.0;
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while r > rest {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            t += dt;
            assert!(t < 30.0, "fall never landed");
        }
        // sqrt(2h/g) = 4.52 s; discrete integration lands within half a second.
        assert!((4.0..5.0).contains(&t), "100 m drop took {t:.2} s, expected ~4.5 s");
    }

    #[test]
    fn thrust_ramps_to_target_and_release_goes_ballistic() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let mut r = rest;
        let mut v = 0.0;
        let dt = 1.0 / 60.0;
        // Hold Space (target 50 m/s): the ramp reaches the target in ~1.7 s
        // at 3g and NEVER exceeds it.
        let mut t = 0.0;
        while v < 50.0 - 1e-6 {
            let s = vertical_step(r, v, rest, 9.81, dt, 50.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            t += dt;
            assert!(v <= 50.0 + 1e-9, "overshot the commanded climb: {v}");
            assert!(t < 5.0, "ramp never reached the target");
        }
        assert!((1.0..3.0).contains(&t), "3g spool to 50 m/s took {t:.2} s");
        // Release: the coast decays through zero into a fall (ballistic arc),
        // and the peak sits above the release height.
        let release_r = r;
        let mut peak = r;
        for _ in 0..1200 {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            peak = peak.max(r);
            if s.grounded {
                break;
            }
        }
        assert!(peak > release_r + 50.0, "no ballistic coast above release point");
        assert!((r - rest).abs() < 1e-6, "arc never landed");
        assert_eq!(v, 0.0, "landing must zero the velocity");
    }

    #[test]
    fn upward_burn_arrests_a_fall() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        // Falling at terminal from 2 km up: thrust up must brake the fall
        // long before the ground.
        let mut r = rest + 2_000.0;
        let mut v = -TERMINAL_FALL_MPS;
        let dt = 1.0 / 60.0;
        for _ in 0..600 {
            let s = vertical_step(r, v, rest, 9.81, dt, 10.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            if v >= 10.0 - 1e-6 {
                break;
            }
        }
        assert!(v >= 10.0 - 1e-6, "burn never arrested the fall: v={v}");
        assert!(r > rest + 500.0, "braked too late: {r}");
    }

    #[test]
    fn standing_on_ground_stays_grounded_and_still() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let s = vertical_step(rest, 0.0, rest, 9.81, 1.0 / 60.0, 0.0, TERMINAL_FALL_MPS);
        assert_eq!(s.r, rest);
        assert_eq!(s.v_r, 0.0);
        assert!(s.grounded);
        // Lifting off from the ground works the same frame thrust starts.
        let s2 = vertical_step(rest, 0.0, rest, 9.81, 1.0, 5.0, TERMINAL_FALL_MPS);
        assert!(s2.r > rest && s2.v_r > 0.0 && !s2.grounded);
    }

    #[test]
    fn settle_eases_down_and_clamps_to_rest() {
        let rest = 6.371e6 + EYE_HEIGHT_M;
        // From 50 m up, one step eases toward rest but never past it.
        let mut r = rest + 50.0;
        for _ in 0..600 {
            r = settle_radius(r, rest, 1.0 / 60.0, 4.0);
            assert!(r >= rest, "settle tunneled below the ground: {r} < {rest}");
        }
        // After ~10 s of easing at rate 4 it is essentially resting.
        assert!((r - rest) < 1e-3, "did not settle: {r} vs {rest}");
        // A camera already below ground snaps up to rest immediately.
        assert_eq!(settle_radius(rest - 500.0, rest, 0.016, 4.0), rest);
    }
}
