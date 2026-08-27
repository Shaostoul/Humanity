// ── CLOUDS V2: bodies are BUILT, not carved ───────────────────────────
//
// The finding that reframed the cloud arc (2026-08-18): no shipped AAA
// cloud system builds cumulus bodies out of noise. Every one uses noise
// only to ERODE a shape that came from somewhere else - Nubis3 applies
// its 128^3 noise as ValueErosion(dimensional_profile, noise) on top of
// a stored voxel body. We had that inverted: inverted-Worley noise WAS
// the body, and inverted Worley is by definition a field of round balls
// whose cells are all near the same size. That is structurally
// incapable of the power-law size spectrum a real cumulus field has,
// which is exactly the operator's "uniform spheres".
//
// So the body is CONSTRUCTED here, matching src/renderer/cloud_primitives.rs
// (whose unit tests lock the shape statistics - flat base, power-law
// lobes, per-genus proportions - on the CPU where they are testable):
//
//   * clouds live on a PLANET-FIXED cell grid, exactly like the
//     vegetation scatter, so a cloud stays put as the camera moves and
//     never reshuffles with LOD (the v0.897 lesson that killed the
//     forest flicker),
//   * each cell either holds one cloud or is clear sky, decided by the
//     weather field - so live MODIS placement still drives everything,
//   * a cloud is a hard FLAT BASE plus smoothly-unioned lobes whose
//     radii come from a Pareto draw: a few big masses and many small
//     ones, laid as a spread body with a leaning cauliflower crown.
//
// The existing erosion bands then bite into this body's rind, which is
// the job noise is actually good at, and every lighting term downstream
// is untouched.

// One cloud per cell; cell size sets the spacing of the field.
// PER-GENUS since increment 6: with one 3.2 km grid, a 700 m humilis
// could never exceed ~4% areal coverage however cloudy the weather -
// the occupancy law's clamp binds at foot/cell^2. Small genera get
// small cells so their achievable coverage matches their real skies
// (humilis fields reach ~30-40% broken cover); the wide genera keep
// the coarse grid their footprints need.
const CLOUD_V2_CELL_KM: f32 = 3.2;

fn cv2_cell_km(idx: i32) -> f32 {
    var t = array<f32, 4>(1.1, 2.2, 3.2, 3.2);
    return t[clamp(idx, 0, 3)];
}
// Lobes evaluated per cloud. The CPU model draws 6-48; the shader caps the
// loop for cost. This used to be a flat 14 for every genus, which is why a
// flat stratocumulus sheet and a towering cumulonimbus were built from the
// same number of parts: the sheet was over-described and the tower was
// under-described, and both read as the same handful of balls. Now the count
// comes from the archetype (see cv2_arch), and this is only the ceiling that
// bounds the array and the loop.
const CLOUD_V2_LOBES: i32 = 20;

// ── DOMAIN WARP (v0.1230) ──
//
// The single change that stops a union of spheres reading as spheres.
//
// Surface displacement, added in v0.1221, pushes the surface in or out along
// its own normal. It can roughen a sphere but it cannot make one FOLD, so a
// displaced sphere is a bumpy sphere - which is exactly what the operator kept
// reporting after each amplitude increase. Warping the DOMAIN instead bends
// the space the distance field is measured in, so the resulting surface can
// overhang, pinch and fold: the cauliflower geometry a real cumulus has and a
// blended sphere union never can.
//
// Applied BEFORE the lobe reduction, which is the whole point - warping after
// it would just be displacement by another name.
//
// Amplitude is a fraction of the LOBE radius, not a fixed metre count. That is
// the lesson from the displacement work: a 111 m detail on a 1238 m storm lobe
// is 9% and invisible, while the same number on a 169 m fair-weather lobe is
// 66% and destroys it. Scale-relative detail is the only thing that reads
// correctly across a genus range spanning 300 m to 8 km.
const CLOUD_V2_WARP_FRAC: f32 = 0.42;
// Warp tile as a multiple of lobe radius. Near 1 means the warp turns lobes
// inside out; well above 1 means it merely translates them. This is the band
// that folds them.
const CLOUD_V2_WARP_TILE_R: f32 = 1.7;
const CLOUD_V2_WARP_LODC: f32 = -5.2;
// Rind over which density falls to zero at the surface, in metres. The
// erosion bands chew into this, so it must be wide enough to have room.
const CLOUD_V2_RIND_M: f32 = 90.0;
// Interior structure (v0.1231). Density at the condensation base as a
// fraction of the adiabatic peak: a real cumulus base is thin and ragged,
// which is why you can often see through the bottom edge of one.
const CLOUD_V2_BASE_FRAC: f32 = 0.30;
// Turbulent interior: tile size in km and how far it swings the density.
const CLOUD_V2_INT_TILE_KM: f32 = 0.34;
const CLOUD_V2_INT_LODC: f32 = -7.9;
const CLOUD_V2_TURB_AMP: f32 = 0.42;
// How far a cloud may sit from its cell centre, as a fraction of the
// cell, so the field is not a visible lattice.
const CLOUD_V2_JITTER: f32 = 0.38;
// ── SURFACE DISPLACEMENT (2026-08-25, the operator: "up close they are
// still an obvious ball pit. How do we get rid of the ball shapes?") ──
//
// No amount of smooth-union blending can stop a union of spheres reading
// as spheres, because BETWEEN the blend seams the surface is still exactly
// spherical. Every shipped volumetric cloud system answers this the same
// way: noise does not build the body, it DISPLACES the body surface. A
// real cumulus boundary is fractal at every scale - that is what makes
// cauliflower - so the distance field gets pushed in and out by an FBM
// before it becomes density.
//
// Two octaves, in METRES, applied to the reduced distance: coarse for the
// billow silhouette, fine for the cauliflower rind. Amplitudes are
// physical, NOT footprint-scaled - a term whose scale rides the rind is
// exactly the class that produced the eyeball rings.
const CLOUD_V2_DISP_TILE_KM: f32 = 2.0;
const CLOUD_V2_DISP_M: f32 = 85.0;
const CLOUD_V2_DISP2_TILE_KM: f32 = 0.55;
const CLOUD_V2_DISP2_M: f32 = 26.0;
// log2 of each octave voxel size in km (256^3 volume), for band-limiting.
const CLOUD_V2_DISP_LODC: f32 = -6.90;
const CLOUD_V2_DISP2_LODC: f32 = -8.76;

// Deterministic hash on a 2D integer cell -> [0,1). Matches the CPU
// model's avalanche family closely enough for placement; the shapes
// themselves are locked by the CPU tests.
fn cv2_hash(cell: vec2<f32>, salt: f32) -> f32 {
    var p3 = fract(vec3<f32>(cell.x, cell.y, cell.x + salt) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + vec3<f32>(dot(p3, vec3<f32>(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
    return fract((p3.x + p3.y) * p3.z);
}

// Bounded-Pareto draw: the power-law that gives a few large lobes and
// many small ones. Mirrors `pareto` in cloud_primitives.rs.
fn cv2_pareto(u: f32, lo: f32, hi: f32, expo: f32) -> f32 {
    let a = max(expo, 1.05);
    let uu = clamp(u, 0.0, 0.9999);
    let lo_a = pow(lo, a - 1.0);
    let hi_a = pow(hi, a - 1.0);
    let denom = 1.0 - uu * (1.0 - lo_a / hi_a);
    return clamp(pow(lo_a / denom, 1.0 / (a - 1.0)), lo, hi);
}

// Polynomial smooth minimum - melts lobes together instead of creasing.
fn cv2_smin(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 1.0e-4) {
        return min(a, b);
    }
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// The per-genus construction rules, mirroring data/clouds/archetypes.ron.
// Index: 0 humilis, 1 congestus, 2 stratocumulus, 3 cumulonimbus.
// (Kept as scalar arrays because naga's HLSL backend cannot pass
// array<f32,N> across function boundaries - the v0.893 lesson.)
struct Cv2Arch {
    width_m: f32,
    aspect: f32,
    expo: f32,
    base_flat: f32,
    crown_bias: f32,
    blend: f32,
    lobes: i32,
};

fn cv2_arch(idx: i32, u: f32) -> Cv2Arch {
    var w_lo = array<f32, 4>(300.0, 800.0, 1500.0, 3000.0);
    // Cb capped at 8 km (increment 6): the 3.2 km cell grid's 3x3
    // neighbourhood with jitter only guarantees an envelope radius of
    // ~4.19 km. A coarse cloud-grid tier for storm-scale systems is the
    // permanent fix (logged in PRIORITIES) - this cap must not silently
    // become the design ceiling. Mirrors data/clouds/archetypes.ron.
    var w_hi = array<f32, 4>(1200.0, 3000.0, 6000.0, 8000.0);
    var a_lo = array<f32, 4>(0.45, 1.20, 0.12, 1.60);
    var a_hi = array<f32, 4>(0.75, 2.60, 0.28, 3.20);
    var t_expo = array<f32, 4>(2.0, 1.8, 2.3, 1.7);
    var t_flat = array<f32, 4>(0.92, 0.95, 0.80, 0.97);
    var t_crown = array<f32, 4>(0.35, 0.70, 0.10, 0.80);
    var t_blend = array<f32, 4>(0.45, 0.38, 0.70, 0.32);
    // Lobes per genus: a flat sheet needs few, a turret needs many.
    var t_lobes = array<i32, 4>(10, 20, 12, 18);
    let i = clamp(idx, 0, 3);
    var o: Cv2Arch;
    o.lobes = t_lobes[i];
    // ── POWER-LAW SIZE (v0.1230) ──
    // This was `mix(w_lo, w_hi, u)`: a UNIFORM draw, so a cloud field held as
    // many 8 km giants as 3 km ones. Real cumulus fields are strongly
    // power-law - observed cloud-area distributions run about n(A) ~ A^-1.7 -
    // which is why a real sky is mostly small clouds with a few big ones, and
    // why a uniform draw reads as a field of same-sized blobs no matter how
    // good each individual blob is. Half the "ball pit" is this line.
    //
    // Area exponent 1.7 with A ~ w^2 gives a width exponent of 2*1.7-1 = 2.4,
    // sampled by the exact inverse CDF of a bounded power law rather than by a
    // pow() curve chosen to look about right.
    let alpha = 2.4;
    let e = 1.0 - alpha;
    let lo_e = pow(w_lo[i], e);
    let hi_e = pow(w_hi[i], e);
    o.width_m = pow(mix(lo_e, hi_e, clamp(u, 0.0, 1.0)), 1.0 / e);
    o.aspect = mix(a_lo[i], a_hi[i], fract(u * 7.31));
    o.expo = t_expo[i];
    o.base_flat = t_flat[i];
    o.crown_bias = t_crown[i];
    o.blend = t_blend[i];
    return o;
}

// Map the 7-family regime type coordinate onto the 4 built archetypes.
// tc 0 = cirrus .. 1 = stratocumulus; the constructed bodies only model
// the CONVECTIVE families, so thin high cloud keeps the old noise body
// (see the caller's blend).
// Rebalanced (increment 6): thin high genera (cirrus/altocumulus, low
// tc) return -1 = KEEP THE NOISE BODY (the caller blends - grape
// clusters cannot be wisps); cumulonimbus holds only its own narrow
// band instead of the whole middle (the old mapping made ~11x too much
// Cb, which read as a wall of giants); the stratus side builds flat
// stratocumulus sheets.
fn cv2_arch_index(tc: f32) -> i32 {
    if (tc < 0.25) {
        return -1; // cirrus/altocu: thin - noise body, not built
    }
    if (tc < 0.40) {
        return 0; // humilis (fair-weather cumulus)
    }
    // CONGESTUS (v0.1230). Index 1 was UNREACHABLE: the old ladder returned
    // -1, 0, 3 and 2 and never 1, so the towering-cumulus genus - the tall
    // cauliflower with aspect 1.2 to 2.6, the most recognisable cloud shape
    // there is - had never been rendered once since the archetypes were
    // written. Every "cloud" in the convective band was either a squat humilis
    // or a full cumulonimbus, which is a large part of why the sky read as
    // uniform blobs with no vertical development.
    if (tc < 0.50) {
        return 1; // congestus: the tall towers
    }
    if (tc < 0.58) {
        return 3; // cumulonimbus - its own band only
    }
    return 2; // stratus..stratocumulus: flat broken sheets
}

// Signed distance (METRES) to one cloud, in that cloud's local frame:
// base plane at y = 0, +y up. Negative inside.
fn cv2_cloud_sdf(local_m: vec3<f32>, seed: f32, arch: Cv2Arch) -> f32 {
    let width = arch.width_m;
    let height = width * arch.aspect;
    // A lobe can never be taller than the cloud it builds - without this
    // a flat genus draws lobes bigger than its own deck depth.
    let r_hi = max(min(width * 0.34, height * 0.9), width * 0.07);
    let r_lo = width * 0.06;

    // ── GRAPE-CLUSTER CONSTRUCTION ──
    // The first cut spread the body lobes flat across the base plane and
    // it built DISCS - the operator would have read them as flying
    // saucers. Real cumulus grow by BUDDING: each puff swells off the
    // shoulder of an existing one, which is what makes cauliflower.
    // So every lobe after the core attaches to a previously placed lobe,
    // offset along a direction whose upward bias is the archetype's
    // crown_bias (flat stratocumulus buds sideways, congestus buds
    // straight up), at a separation that keeps the pair merged.
    let n_lobes = clamp(arch.lobes, 1, CLOUD_V2_LOBES);
    var lc: array<vec4<f32>, 20>; // xyz = centre (m), w = radius (m)
    let r0 = r_hi * mix(0.7, 1.0, cv2_hash(vec2<f32>(seed, 0.0), 31.0));
    // Core lobe: its surface respects the height cap too - flat genera
    // draw r0 comparable to their whole deck depth.
    lc[0] = vec4<f32>(0.0, min(r0, max(height - r0, r0 * arch.base_flat)), 0.0, r0);
    var mean_r = r0;
    for (var i = 1; i < n_lobes; i = i + 1) {
        let fi = f32(i);
        // Pick a parent among the lobes placed so far. Later lobes
        // prefer recent (higher, smaller) parents, which grows a turret
        // rather than a ring.
        let pu = cv2_hash(vec2<f32>(seed, fi), 41.0);
        let pj = i32(floor(pu * pu * f32(i)));
        let parent = lc[clamp(pj, 0, i - 1)];
        // Child radius: power-law, and never larger than its parent, so
        // the cluster tapers the way a real turret does.
        let r = min(
            cv2_pareto(cv2_hash(vec2<f32>(seed, fi), 43.0), r_lo, r_hi, arch.expo),
            parent.w * 0.92,
        );
        // Bud direction: azimuth free, elevation biased by crown_bias.
        let ang = cv2_hash(vec2<f32>(seed, fi), 45.0) * 6.2831853;
        let up_lo = mix(-0.35, 0.15, arch.crown_bias);
        let up = mix(up_lo, 1.0, cv2_hash(vec2<f32>(seed, fi), 47.0));
        let horiz = sqrt(max(1.0 - up * up, 0.0));
        let dir = vec3<f32>(cos(ang) * horiz, up, sin(ang) * horiz);
        // Separation: close enough that the smooth union merges them
        // into one body rather than leaving a string of beads.
        let sep = (parent.w + r) * mix(0.55, 0.78, cv2_hash(vec2<f32>(seed, fi), 49.0));
        var c = parent.xyz + dir * sep;
        // ENVELOPE CLAMP on centre PLUS radius (increment 6): the whole
        // lobe SURFACE stays inside the width/2 cylinder and under the
        // height cap. Clamping only the centre let lobes reach 0.84 x
        // width past the bounding reject - overhanging their cells on
        // one side and truncating at cell seams on the other.
        let y_lo = min(r * arch.base_flat, height - r);
        c.y = clamp(c.y, y_lo, max(height - r, y_lo));
        let horiz_len = length(c.xz);
        let horiz_max = max(width * 0.5 - r, 0.0);
        if (horiz_len > horiz_max) {
            let k = horiz_max / max(horiz_len, 1.0e-4);
            c = vec3<f32>(c.x * k, c.y, c.z * k);
        }
        lc[i] = vec4<f32>(c, r);
        mean_r = mean_r + r;
    }
    mean_r = mean_r / f32(n_lobes);

    // Evaluate the smooth union of the cluster. The blend radius never
    // drops below the rind (increment 6): a 90 m rind thresholded by a
    // crease-sharp union produced as little as 7 m of density
    // transition - a guaranteed salt-and-pepper generator under any
    // sampling.
    // TAPER MERGE (2026-08-25, operator: "can we taper merge the sections
    // together as to remove some of the ball pit look... the skirt of one
    // model merged to another via modifiers instead of by hand"). The blend
    // radius IS that modifier: it is the width over which two lobe surfaces
    // melt into one. Floored at 1.6x the rind rather than 1.0x so even the
    // smallest cluster fuses rather than reading as touching spheres.
    let k = max(mean_r * arch.blend, CLOUD_V2_RIND_M * 1.6);

    // ── DOMAIN WARP (see CLOUD_V2_WARP_FRAC) ──
    // Bend the space the cluster is measured in, so the union can fold and
    // overhang instead of only bulging. One texture fetch, and it is the
    // difference between cauliflower and marbles.
    //
    // The vertical component is applied to the lobe field ONLY. The flat base
    // below still intersects against the UNWARPED local_m.y, because that
    // level base marks the lifting condensation level - a real thermodynamic
    // surface, and the most recognisable cue the whole constructed path has.
    // Warping it would buy nothing and would bend the one thing that is right.
    let warp_tile_m = max(mean_r * CLOUD_V2_WARP_TILE_R, 1.0);
    let wn = textureSampleLevel(
        cloud_detail_tex, cloud_tile_sampler,
        local_m / warp_tile_m,
        clamp(g_v2_disp_lod - CLOUD_V2_WARP_LODC, 0.0, 8.0),
    ).rgb;
    let warp_amp_m = mean_r * CLOUD_V2_WARP_FRAC;
    g_v2_warp_m = warp_amp_m;
    let warped = local_m + (wn - vec3<f32>(0.5)) * 2.0 * warp_amp_m;

    var d = length(warped - lc[0].xyz) - lc[0].w;
    for (var i = 1; i < n_lobes; i = i + 1) {
        let ds = length(warped - lc[i].xyz) - lc[i].w;
        d = cv2_smin(d, ds, k);
    }

    // Hard flat base: intersect with the half-space y >= 0. A real
    // cumulus base is LEVEL because it marks the lifting condensation
    // level, a thermodynamic surface - the single most recognisable cue
    // the old noise body could not produce.
    return max(d, -local_m.y);
}

// The constructed cloud BODY at a planet-local sample point, in [0,1].
// `p` is in drawn-shell units (the march's own space); `wa` is the
// weather/coverage alpha at this point, which decides whether a cell
// holds a cloud at all, so live MODIS placement still rules.
fn cloud_v2_body(p: vec3<f32>, wa: f32, tc: f32, lodb: f32) -> f32 {
    if (wa <= 0.02) {
        return 0.0;
    }
    let arch_i = cv2_arch_index(tc);
    if (arch_i < 0) {
        // Thin high genera keep the noise body - the caller blends.
        return 0.0;
    }
    let r = length(p);
    let dir = p / max(r, 1.0e-6);
    // Cell grid in RADIANS of arc, planet-fixed (the vegetation-scatter
    // pattern): a cloud stays where it is as the camera moves.
    let planet_km = max(material.params2.z, 1.0);
    let cell_km = cv2_cell_km(arch_i);
    let cell_rad = cell_km / planet_km;
    let lat = asin(clamp(dir.y, -1.0, 1.0));
    let lon = atan2(-dir.z, dir.x);
    // Longitude cells shrink with latitude so cloud spacing stays
    // constant in km rather than bunching at the poles.
    let coslat = max(cos(lat), 0.05);
    let cx = lon / (cell_rad / coslat);
    let cy = lat / cell_rad;
    let base_i = floor(cx);
    let base_j = floor(cy);

    // Height of this sample above the slab base, in metres.
    let up_m = (r - g_cloud_rb) / g_cloud_upkm * 1000.0;
    if (up_m < -50.0) {
        return 0.0;
    }

    var best = 1.0e9;
    var best_height_m = 1.0;
    // Check the 3x3 neighbourhood so a wide cloud reaches across cells.
    for (var dj = -1; dj <= 1; dj = dj + 1) {
        for (var di = -1; di <= 1; di = di + 1) {
            let ci = base_i + f32(di);
            let cj = base_j + f32(dj);
            let cell = vec2<f32>(ci, cj);
            let seed = cv2_hash(cell, 7.0) * 4096.0;
            let arch = cv2_arch(arch_i, cv2_hash(cell, 9.0));
            // OCCUPANCY LAW (increment 6): the probability a cell holds
            // a cloud makes the MODIS fraction equal the actual AREAL
            // coverage - p = wa * cell_area / cloud_footprint_area. The
            // old bare hash>wa filled cells regardless of cloud size, so
            // an 8 km cumulonimbus in every second 3.2 km cell tiled the
            // sky ~11x denser than the weather said - the wall of
            // giants.
            let m_per_cell_o = cell_km * 1000.0;
            let foot = 3.14159265 * arch.width_m * arch.width_m * 0.25;
            let p_cell = clamp(wa * (m_per_cell_o * m_per_cell_o) / max(foot, 1.0), 0.0, 1.0);
            if (cv2_hash(cell, 3.0) > p_cell) {
                continue;
            }
            // Cell centre + jitter, as an offset in metres from the
            // sample, measured in the local tangent plane.
            let jx = (cv2_hash(cell, 17.0) - 0.5) * CLOUD_V2_JITTER;
            let jy = (cv2_hash(cell, 19.0) - 0.5) * CLOUD_V2_JITTER;
            let dx_cells = (ci + 0.5 + jx) - cx;
            let dy_cells = (cj + 0.5 + jy) - cy;
            // Cell-space offset -> metres on the ground.
            let m_per_cell = cell_km * 1000.0;
            let ox = -dx_cells * m_per_cell;
            let oy = -dy_cells * m_per_cell;
            // Cheap bounding reject before the lobe loop: the envelope
            // is now a true bound (centre+radius clamped), so width/2
            // plus the rind suffices - the old 0.62 factor let clamped
            // lobes truncate at the reject edge.
            let height_m = arch.width_m * arch.aspect;
            let br = arch.width_m * 0.5 + CLOUD_V2_RIND_M;
            if (ox * ox + oy * oy > br * br) {
                continue;
            }
            if (up_m > height_m + br) {
                continue;
            }
            let local_m = vec3<f32>(ox, up_m, oy);
            let d_cell = cv2_cloud_sdf(local_m, seed, arch);
            if (d_cell < best) {
                best = d_cell;
                // Remember the WINNING cloud's own height, so the interior
                // profile below is expressed as a fraction of the cloud the
                // sample is actually inside rather than of the whole slab.
                best_height_m = height_m;
            }
        }
    }
    // Publish the distance for the march to steer by (v0.1230). `best` is a
    // real signed distance in METRES to the lobe cluster, and until now it was
    // computed, thresholded into a density, and thrown away - while the march
    // outside stepped by a fixed hop that knew nothing about where the surface
    // was. See the SDF-guided step in 40-clouds.wgsl.
    g_v2_sdf_m = best;
    if (best >= 0.0) {
        return 0.0;
    }
    // Soft rind so silhouettes are not stencils; the erosion bands then
    // bite into exactly this falloff. Widened to the sampling FOOTPRINT
    // (increment 6): a rind narrower than the march's own step aliases
    // into salt-and-pepper no matter how the field is shaped - the same
    // prefilter law the noise mips obey.
    // PER-RAY footprint, never the caller's per-tap lodb (see
    // g_v2_foot_m in 40-clouds.wgsl): the sun-shadow ladder passes eight
    // different lodb values per shading evaluation, and since this body
    // is a DISTANCE FIELD the rind is a metric radius - so a per-tap rind
    // shaded eight concentrically shrunken copies of the same lobe and
    // printed them as nested rings (the "eyeball" artifact). Falls back
    // to the constant floor when unset, which is the pre-increment-6
    // behaviour.
    // FIXED PHYSICAL WIDTH (2026-08-25, the operator: "the clouds do not
    // really seem to maintain their shape while moving around them... the
    // fine cloud detail is weirdly non-permanent, always morphing" and "I
    // still get the feel that the clouds are oriented to me").
    //
    // The rind used to be max(90 m, the ray footprint). The footprint is a
    // function of CAMERA DISTANCE, and the rind is the width of the density
    // ramp - i.e. the cloud SHAPE. So the shape was a function of where the
    // viewer stood: at the far edge of the v2 range (lodb 2 = a 4 km
    // footprint) the rind reached 4000 m and the entire cloud dissolved into
    // one soft gradient, then firmed up into lobes as you approached. That is
    // the morphing, and it is why the clouds read as oriented to the viewer
    // rather than to the planet: their placement is planet-fixed, but their
    // detail was camera-fixed.
    //
    // A real cloud edge has a physical transition width that does not care
    // where it is seen from. Band-limiting is the job of the noise mip chain
    // and the temporal accumulation, NOT of reshaping the body.
    // FRACTAL SURFACE DISPLACEMENT (see the constants above). Applied HERE,
    // after the min-reduction over cells and lobes, for four reasons that
    // all matter:
    //  - this is the only point where the value is still a metric distance
    //    in METRES, so the amplitude is footprint-independent,
    //  - it is a pure function of world position, so the view sample and
    //    all eight sun-shadow taps see the SAME displaced surface (a
    //    per-tap quantity here is what drew the eyeball rings),
    //  - being post-reduction it also warps the seams BETWEEN neighbouring
    //    clouds, which is the other half of the ball-pit look, for free,
    //  - one texture fetch per sample instead of one per lobe: 42 calls per
    //    ray rather than 882.
    // The texture is used rather than procedural hash noise specifically so
    // the mip chain band-limits it; unfiltered value noise here would alias
    // straight back into the salt-and-pepper this is meant to remove. The
    // mip is taken WITHOUT the per-pixel lod dither: this displacement is
    // SHAPE, and shape must be identical for every ray that reaches this
    // point in the world, or it stipples.
    // PER-RAY mip, never the caller's per-tap `lodb` (v0.1230).
    //
    // The comment above states the requirement - shape must be identical for
    // every ray reaching this point - and the code used to violate it. The
    // sun-shadow ladder calls this eight times per view sample with a lod
    // band-limited by each tap's OWN stride (40-clouds.wgsl, `lod_t`), and
    // those strides grow geometrically, so the far taps landed at a mip where
    // the displacement has been filtered away entirely.
    //
    // The consequence was quietly ruinous: the EYE saw a displaced, bumpy
    // surface while the SUN saw a smooth sphere, so the silhouette had relief
    // and the shading across it was a smooth radial gradient. Adding more
    // displacement could never fix it, because the light was not looking at
    // the displacement - which is why the v0.1221 amplitude work read as
    // having done nothing at all.
    //
    // g_v2_disp_lod is frozen once per ray next to g_v2_foot_m, so the view
    // sample and all eight sun taps now shade the SAME surface.
    let d1 = textureSampleLevel(cloud_detail_tex, cloud_tile_sampler,
        p * (1.0 / (CLOUD_V2_DISP_TILE_KM * g_cloud_upkm)),
        clamp(g_v2_disp_lod - CLOUD_V2_DISP_LODC, 0.0, 8.0));
    let d2 = textureSampleLevel(cloud_detail_tex, cloud_tile_sampler,
        p * (1.0 / (CLOUD_V2_DISP2_TILE_KM * g_cloud_upkm)),
        clamp(g_v2_disp_lod - CLOUD_V2_DISP2_LODC, 0.0, 8.0));
    let n1 = d1.r * 0.625 + d1.g * 0.25 + d1.b * 0.125;
    let n2 = d2.r * 0.625 + d2.g * 0.25 + d2.b * 0.125;
    let disp_m = (n1 - 0.5) * 2.0 * CLOUD_V2_DISP_M
               + (n2 - 0.5) * 2.0 * CLOUD_V2_DISP2_M;
    let rind = CLOUD_V2_RIND_M;
    let core = clamp(-(best - disp_m) / rind, 0.0, 1.0);

    // ── THE INTERIOR (v0.1231) ──
    //
    // `core` alone is a linear ramp off a distance field, so it saturates one
    // rind (90 m) inside the surface and is CONSTANT everywhere beyond that.
    // Measured against the archetypes, that left 79% of a cumulus lobe and 93%
    // of a storm lobe at exactly full density with no internal structure at
    // all. That is the operator's "spheres with ZERO TRANSPARENCY", and it was
    // arithmetic rather than taste: light cannot get into a medium that is
    // uniformly opaque, so no lighting work could ever have made it translucent.
    //
    // Two physical terms replace the constant.
    //
    // 1. THE ADIABATIC PROFILE. In a rising parcel, liquid water content grows
    //    roughly linearly with height above the condensation base as vapour
    //    condenses out, then falls off near the top where the parcel detrains
    //    and mixes with dry air. So a real cumulus is thin and ragged at its
    //    base, densest in its upper-middle, and soft at the crown - it is NOT a
    //    uniform lump. This single gradient is most of what makes a cloud read
    //    as a body of water rather than a solid.
    //
    // 2. THE TURBULENT FIELD. Cloud interiors are stirred at every scale, so
    //    the density has structure at 50-500 m throughout, not just on the
    //    skin. This is what lets light find paths through and gives the mass
    //    depth instead of a painted-on surface.
    let hf = clamp(up_m / max(best_height_m, 1.0), 0.0, 1.0);
    let adiabatic = smoothstep(0.0, 0.30, hf) * (1.0 - smoothstep(0.68, 1.0, hf));
    let t1 = textureSampleLevel(cloud_detail_tex, cloud_tile_sampler,
        p * (1.0 / (CLOUD_V2_INT_TILE_KM * g_cloud_upkm)),
        clamp(g_v2_disp_lod - CLOUD_V2_INT_LODC, 0.0, 8.0));
    let turb = t1.r * 0.6 + t1.g * 0.25 + t1.b * 0.15;
    let interior = mix(CLOUD_V2_BASE_FRAC, 1.0, adiabatic)
        * mix(1.0 - CLOUD_V2_TURB_AMP, 1.0 + CLOUD_V2_TURB_AMP, turb);
    return clamp(core * interior, 0.0, 1.0);
}
