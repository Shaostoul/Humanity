// ── Planet surface close-range detail (v0.816): ocean waves + land texture ──
//
// Both effects live in the material type-12 branch and engage ONLY on the
// textured per-pixel path (params.w bit 0) with the Surface-detail toggle on
// (params.w bit 1, Settings > Graphics > Planets). Rust mirrors + unit
// tests: src/renderer/water.rs -- its wgsl_water_constants_stay_in_sync test
// parses this file, so keep every WATER_* / DETAIL_* / WAVE* / LAND* constant
// byte-identical with the Rust module.
//
// ANTI-ALIASING RULE (the load-bearing design decision): every octave, wave
// or land noise, fades out as its wavelength approaches the pixel footprint,
// estimated ANALYTICALLY as fragment distance * PLANET_PIXEL_ANGLE (no
// screen-space derivatives: cheap, and valid in any control flow). An octave
// is fully on once it spans >= DETAIL_FADE_HI pixels and exactly zero below
// DETAIL_FADE_LO pixels, so the ocean converges to the smooth v0.810 look
// from orbit (wave presence hits a literal 0.0 -- bit-identical far field)
// and never shimmers at any altitude in between.

// Estimated view angle of one pixel (radians): ~90 deg vertical FOV over a
// ~1400 px viewport, rounded down slightly so octaves fade EARLIER (safer
// against shimmer) on small windows. footprint_m = distance_m * this.
const PLANET_PIXEL_ANGLE: f32 = 0.0008;
// Octave visibility band, in projected pixels per wavelength: zero at or
// below LO, fully on at or above HI (both comfortably above Nyquist).
const DETAIL_FADE_LO: f32 = 4.0;
const DETAIL_FADE_HI: f32 = 12.0;

// ── THE GRAZING FOOTPRINT (the ocean zebra-stripe root cause) ─────────────
//
// `dist * PLANET_PIXEL_ANGLE` is the pixel footprint ACROSS the sightline -
// the angular one. The footprint ALONG the sightline is that divided by
// cos(incidence) = dot(surface normal, view direction), and THAT is the
// sample spacing the anti-alias gates actually have to respect: a band limit
// must be set by the LOWEST sampling rate in any direction, or the content
// aliases along that direction.
//
// Every other surface in the engine is mostly seen face-on, so the isotropic
// estimate is close enough. The SEA is the one surface that is essentially
// always viewed at extreme incidence, and the stretch is unbounded at the
// horizon. Worked numbers for the two rig captures that show the defect:
//
//   ocean-storm-horizon, 20 m eye, 40 px under the horizon: distance 308 m,
//   ndv 0.065. Across = 0.25 m, ALONG = 3.8 m. The wave texture was sampled
//   at mip log2(0.25 * 128) = 5.0 when the along-axis needs 8.9 - sixteen
//   times under-filtered. Measured: per-pixel horizontal RMS luminance
//   gradient peaks at 33 in that band and DECAYS with distance from the
//   horizon, the exact inverse of what a correctly filtered surface does.
//
//   ocean-150m, 150 m eye, mid-field row 850: distance 477 m, ndv 0.30,
//   along 0.86 m vs the 0.31 m the gates assumed. RMS peaks at 9.2 there.
//
// The two visible faces of the same defect: at MODERATE stretch (3-6x) the
// folded frequencies land mid-band and read as shimmer/speckle; at EXTREME
// stretch (>10x, everything approaching the horizon) they fold to near-DC
// and read as LARGE REGULAR PARALLEL BANDS riding the glitter - the
// operator's zebra stripes. They run perpendicular to the view because the
// sampling compression is along the sightline, so the alias fringes follow
// iso-distance lines.
//
// The gates were never wrong about their thresholds; they were fed the wrong
// footprint. `water_footprint` returns the along-sightline one and stashes
// the anisotropy so `ocean_tex_gradient` can hand the hardware the TRUE
// gradient pair instead of one isotropic mip (the same fix the ground path
// took in v0.977; water was left behind on the old explicit-LOD form).
//
// Numeric guard only - at the true horizon the footprint really is infinite
// (one pixel spans an unbounded strip of sea), and every gate retiring there
// is the correct answer: `resolved` goes to 0, the mirror retires to the
// geometric normal, and the Cox-Munk lobe widens to carry the whole slope
// distribution. That machinery already exists in water_shade; it was simply
// never reached, because the footprint never grew.
const WATER_GRAZE_MIN_NDV: f32 = 0.01;
// The sampler's anisotropy_clamp (renderer::ground_textures). Beyond this
// ratio the hardware falls back to an isotropic long-axis mip anyway.
const WATER_ANISO_MAX: f32 = 16.0;

// Fragment-scope water footprint state, written by `water_footprint` and
// read by `ocean_tex_gradient`. Same var<private> transport pattern as
// g_inst_data: it keeps the anisotropy out of every intermediate signature.
// Zero `g_water_fp_across` means "never set" - the texture path then falls
// back to the isotropic footprint it is passed, so this file is inert until
// the caller opts in.
var<private> g_water_fp_across: f32 = 0.0;
var<private> g_water_ndv: f32 = 1.0;
// Unit view direction (fragment -> eye) projected into the local tangent
// plane, in the PLANET-LOCAL frame - the long axis of the footprint ellipse.
// Zero when the view is exactly along the normal (no anisotropy to orient).
var<private> g_water_vt: vec3<f32> = vec3<f32>(0.0);

// Pixel footprint on the WATER surface, in metres, along its longest axis.
// `n_local` and `view_local` are the planet-local sphere normal and the unit
// fragment-to-eye direction (water_shade's own frame).
fn water_footprint(dist_m: f32, n_local: vec3<f32>, view_local: vec3<f32>) -> f32 {
    let across = max(dist_m * PLANET_PIXEL_ANGLE, 0.001);
    let ndv = max(abs(dot(n_local, view_local)), WATER_GRAZE_MIN_NDV);
    g_water_fp_across = across;
    g_water_ndv = ndv;
    let vt = view_local - n_local * dot(view_local, n_local);
    let l = length(vt);
    if (l > 1.0e-5) {
        g_water_vt = vt / l;
    } else {
        g_water_vt = vec3<f32>(0.0);
    }
    return across / ndv;
}
// Water Fresnel reflectance at normal incidence (n = 1.33 -> ~0.02).
const WATER_F0: f32 = 0.02;
// W2 (environment program increment 7): fraction of the Cox-Munk slope
// variance the RESOLVED wave normal already carries when the wave texture
// is fully readable. Cox-Munk measured the TOTAL sea-surface slope
// distribution - every scale from swell to capillary - so a lobe that uses
// the full mss on top of n_pert (which already tilts by the resolved
// swell + texture chop) counts the large scales twice, washing the
// near-field glitter wide. Subtracting what the normal resolves leaves the
// lobe carrying only the sub-texel capillary tail near the camera, and the
// FULL distribution at orbit where nothing is resolved (resolved = 0), so
// the glint is a wide smooth ellipse there instead of a lattice of dots.
// 0.5, not the council's ~0.85 first guess: Cox-Munk's own slick-vs-clean
// data (oil slicks damp the capillaries) attributes roughly HALF the total
// slope variance to capillary scales no wave texture can ever resolve, and
// 0.85 measured a 47% contrast loss at the storm-glitter golden (the lobe
// concentrated into too few pixels). At 0.5 the resolved-out share matches
// the physically unresolvable share.
const WATER_MSS_RESOLVED_FRAC: f32 = 0.5;

// Sea state 0..1 from the fill_color.w pad, DECODING the pin convention:
// values >= 1.5 mean "pinned at (value - 2)" (the showcase {"sea":x} dev
// override, renderer/mod.rs writes pin + 2.0). The sea_state block in
// 90-fragment-main.wgsl always decoded this; the wind-driven glitter width
// here never did, so EVERY pinned vantage - including sea 0.3 calm pins,
// which encode as 2.3 - clamped to full storm wind (u10 = 15) and rendered
// a storm-wide specular lobe on a calm sea. Every rig golden before
// increment 7 was measured that way.
fn water_sea_state01() -> f32 {
    let w = camera.fill_color.w;
    if (w >= 1.5) {
        return clamp(w - 2.0, 0.0, 1.0);
    }
    return clamp(w, 0.0, 1.0);
}
// Sun sparkle: Blinn-Phong exponent on the WAVE-PERTURBED normal (tight --
// the moving glitter field) and its gain. Sun-only, same reasoning as the
// v0.810 glint: the fixed fill light would paint a bogus second hotspot.
const WATER_SPEC_POWER: f32 = 900.0;
const WATER_SPEC_GAIN: f32 = 1.1;
// Analytic reflected-sky brightness (fraction of sun intensity). Trimmed
// again v0.826: 0.4 still lit the whole grazing mid-field into a white
// cross-hatch corduroy at 1.5 km. 0.20 (with the deeper reflected-sky colour
// in water_shade) keeps a subtle blue sky mirror while letting the localized
// sun glitter -- not a uniform grazing sheen -- carry the bright highlights.
const WATER_SKY_GAIN: f32 = 0.20;
// Sea ice rides the water flag (below-sea faces of has_water planets) but
// must not shade like open ocean: wave presence fades out as the graded
// albedo brightens from ocean blue toward cap white across this band
// (max-channel luminance).
const WATER_ICE_LUM_LO: f32 = 0.35;
const WATER_ICE_LUM_HI: f32 = 0.6;
const TAU: f32 = 6.28318530718;

// Wave octave table: 6 directional gravity-wave trains, wavelengths 2 km
// down to 50 m, each with its own fixed planet-local direction, temporal
// frequency (cycles/sec of cloud-clock time, near the deep-water dispersion
// rate sqrt(g/(2 pi lambda))), and SLOPE amplitude (dimensionless steepness
// A*k -- what normal perturbation actually consumes, scale-free).
// Slopes halved v0.819: the v0.818 steepness (sum 0.55) tilted so many wave
// faces to grazing that the whole sea streaked bright-white and aliased. The
// big swells keep the most slope (rolling structure); the short chop is
// gentled hardest (it drove the shimmer). Sum ~0.27.
const WAVE1_LAMBDA: f32 = 2000.0;
const WAVE1_CPS: f32 = 0.028;
const WAVE1_SLOPE: f32 = 0.035;
const WAVE1_DIR: vec3<f32> = vec3<f32>(0.7071068, 0.0, 0.7071068);
const WAVE2_LAMBDA: f32 = 850.0;
const WAVE2_CPS: f32 = 0.045;
const WAVE2_SLOPE: f32 = 0.05;
const WAVE2_DIR: vec3<f32> = vec3<f32>(0.9622504, 0.1924501, 0.1924501);
const WAVE3_LAMBDA: f32 = 360.0;
const WAVE3_CPS: f32 = 0.07;
const WAVE3_SLOPE: f32 = 0.05;
const WAVE3_DIR: vec3<f32> = vec3<f32>(0.2672612, 0.5345225, 0.8017837);
const WAVE4_LAMBDA: f32 = 150.0;
const WAVE4_CPS: f32 = 0.105;
const WAVE4_SLOPE: f32 = 0.045;
const WAVE4_DIR: vec3<f32> = vec3<f32>(-0.5773503, 0.5773503, 0.5773503);
const WAVE5_LAMBDA: f32 = 80.0;
const WAVE5_CPS: f32 = 0.145;
const WAVE5_SLOPE: f32 = 0.04;
const WAVE5_DIR: vec3<f32> = vec3<f32>(0.4082483, -0.8164966, 0.4082483);
const WAVE6_LAMBDA: f32 = 50.0;
const WAVE6_CPS: f32 = 0.18;
const WAVE6_SLOPE: f32 = 0.035;
const WAVE6_DIR: vec3<f32> = vec3<f32>(-0.6666667, 0.3333333, -0.6666667);

// Crest domain-warp (v0.826): the six trains above are pure directional plane
// waves, so every crest is a dead-straight parallel line -- the "very straight
// water" the operator flagged at 1.5 km over Oahu. Real open water has crests
// that SNAKE and interfere, each stretch different from the next. Fix: before
// the cos, offset each octave's phase by a TWO-OCTAVE (fractal) value-noise
// domain warp sampled on the sphere. A single warp frequency just makes every
// crest undulate identically (still reads as parallel bands); summing a COARSE
// warp (shifts whole crests by different amounts) with a FINER one (local
// wiggle) makes crests wander irregularly so no two stretches look the same.
// The warp only shifts phase (never amplitude), so the per-octave anti-alias
// fade still kills every wave from orbit -- the far field stays bit-identical,
// and it is fully decoupled from wave HEIGHT (slope), which stays gentle.
//   WAVE_WARP_AMP / _MULT   coarse warp: amplitude (in wavelengths) and spatial
//                           wavelength as a multiple of the wave wavelength.
//   WAVE_WARP_AMP2 / _MULT2  fine warp: the local snaking detail.
//   WAVE_WARP_SEED  base noise seed; per-octave seed = this + lambda * 0.01 so
//                   the six trains snake on decorrelated noise fields.
const WAVE_WARP_AMP: f32 = 1.35;
const WAVE_WARP_MULT: f32 = 3.5;
const WAVE_WARP_AMP2: f32 = 0.32;
const WAVE_WARP_MULT2: f32 = 1.4;
const WAVE_WARP_SEED: f32 = 4.7;

// Land detail octaves: multiplicative luminance variation synthesized UNDER
// the photo albedo (no biome recoloring), +-amp per octave.
const LAND1_LAMBDA: f32 = 10000.0;
const LAND1_AMP: f32 = 0.1;
const LAND1_SEED: f32 = 3.7;
const LAND2_LAMBDA: f32 = 1000.0;
const LAND2_AMP: f32 = 0.08;
const LAND2_SEED: f32 = 17.3;
const LAND3_LAMBDA: f32 = 150.0;
const LAND3_AMP: f32 = 0.06;
const LAND3_SEED: f32 = 31.9;
// v0.898 (operator: "hard to make anything out... everything uniformly
// lit" at walking height): the ladder used to STOP at 150 m, so the ground
// underfoot was one flat bilinear color. Two finer octaves carry visible
// structure down to ~8 m. Sub-8 m octaves are NOT safe here: the noise
// domain is the f32 unit direction, whose per-fragment quantization step
// at ground level is ~6e-8 rad (~0.4 m of arc) - finer wavelengths would
// band. True sub-meter ground texture needs a tangent-space detail map
// (journaled follow-up).
const LAND4_LAMBDA: f32 = 25.0;
const LAND4_AMP: f32 = 0.07;
const LAND4_SEED: f32 = 47.1;
const LAND5_LAMBDA: f32 = 8.0;
const LAND5_AMP: f32 = 0.07;
const LAND5_SEED: f32 = 63.7;

// Per-octave anti-alias fade: how many projected pixels one wavelength
// spans, smoothstepped through the visibility band. Exactly 0 when the
// octave would alias, exactly 1 when it is comfortably resolved.
fn detail_octave_fade(lambda_m: f32, footprint_m: f32) -> f32 {
    // v0.905 (operator: "everything is too smooth too soon" at 120 FPS
    // vsync with GPU headroom): the detail-distance factor from Settings >
    // Planets scales how far EVERY detail octave survives - land noise,
    // micro texture, waves. Rides the view_pos.w pad; 0 means an older
    // writer, treated as 1x.
    let ddk = select(1.0, max(camera.view_pos.w, 0.05), camera.view_pos.w > 0.01);
    return smoothstep(DETAIL_FADE_LO / ddk, DETAIL_FADE_HI / ddk, lambda_m / footprint_m);
}

// Triplanar value noise on the sphere -- same pow-4-weight construction as the
// cloud field's sphere noise but its own seed offsets, so this stays
// independent of the cloud functions (which have their own rework cadence).
// freq = planet radius / wavelength. Used by BOTH the wave crest domain-warp
// (wave_octave, below) and the land detail octaves (land_detail_factor), so it
// is declared here ahead of the first caller.
// ── Camera-relative micro detail (v0.902) ──
// Sub-8 m ground/water texture was impossible in the planet-frame f32
// domain (unit-dir quantization ~0.4 m at ground level banded anything
// finer). The fix: per-fragment offsets are taken CAMERA-RELATIVE (small
// => full f32 precision) and anchored to the planet by the camera's
// planet-frame position mod 64 m, poked from Rust into the
// light0_cone_inner.yzw pads. Anchor jumps are exact 64 m steps, so any
// pattern with a period dividing 64 m is seamless across them.
fn micro_hash(c: vec3<f32>) -> f32 {
    return fract(sin(dot(c, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

// Trilinear value noise on a PERIODIC integer lattice (period = 64 m /
// wavelength, so the anchor's 64 m jumps land on exact periods).
fn micro_noise(p: vec3<f32>, period: f32) -> f32 {
    let i0 = floor(p);
    let f = p - i0;
    let u = f * f * (3.0 - 2.0 * f);
    var s = 0.0;
    for (var dz = 0; dz < 2; dz = dz + 1) {
        for (var dy = 0; dy < 2; dy = dy + 1) {
            for (var dx = 0; dx < 2; dx = dx + 1) {
                let c = i0 + vec3<f32>(f32(dx), f32(dy), f32(dz));
                let cc = c - floor(c / period) * period;
                let w = mix(1.0 - u.x, u.x, f32(dx))
                    * mix(1.0 - u.y, u.y, f32(dy))
                    * mix(1.0 - u.z, u.z, f32(dz));
                s = s + w * micro_hash(cc);
            }
        }
    }
    return s;
}

// ── Ground PBR texturing (v0.907) ──
// Triplanar sample of one ground_tex layer in the camera-relative
// planet-pinned metre domain (micro_noise's anchor domain, so tiles stay
// seamless across the 64 m anchor jumps -- the tile size divides 64).
//
// LEGACY (v0.907) entry point. `ground_detail` below supersedes it and every
// new caller should use that; this stays only until the type-12 branch in
// 90-fragment-main.wgsl is moved over, so the two can land in separate
// commits without a broken frame in between. Delete both this constant and
// `ground_triplanar_grad` in the same change that rewires the call site.
const GROUND_LEGACY_TILE_M: f32 = 2.0;

// Gradient sampling (v0.977, the grazing-angle smear fix): the old
// explicit-LOD form picked ONE isotropic mip from the analytic footprint,
// which bypasses the sampler's anisotropy entirely - a flat sightline
// footprint is metres long along the view but centimetres across it, so
// the isotropic mip smeared the across-view detail into mush. Passing the
// true per-plane UV gradients via textureSampleGrad lets the hardware
// anisotropic filter (x8, ground_textures.rs) take multiple taps along
// the long axis instead. Gradients come from dpdx/dpdy of world_position
// taken at the TOP of fs_main (uniform control flow, always valid) and
// rotated into the pinned domain by the caller - pt is anchor + inv_m *
// (wp - eye), both constant per draw, so d(pt) = inv_m * d(wp) exactly.
fn ground_triplanar_grad(
    layer: i32,
    p: vec3<f32>,
    w: vec3<f32>,
    gx: vec3<f32>,
    gy: vec3<f32>,
) -> vec3<f32> {
    let uv_x = p.yz / GROUND_LEGACY_TILE_M;
    let uv_y = p.xz / GROUND_LEGACY_TILE_M;
    let uv_z = p.xy / GROUND_LEGACY_TILE_M;
    return textureSampleGrad(ground_tex, ground_samp, uv_x, layer, gx.yz / GROUND_LEGACY_TILE_M, gy.yz / GROUND_LEGACY_TILE_M).rgb * w.x
        + textureSampleGrad(ground_tex, ground_samp, uv_y, layer, gx.xz / GROUND_LEGACY_TILE_M, gy.xz / GROUND_LEGACY_TILE_M).rgb * w.y
        + textureSampleGrad(ground_tex, ground_samp, uv_z, layer, gx.xy / GROUND_LEGACY_TILE_M, gy.xy / GROUND_LEGACY_TILE_M).rgb * w.z;
}

// ═══════════════════════════════════════════════════════════════════════════
//  GROUND SURFACE DETAIL (v0.1101) -- full PBR under the planet imagery
// ═══════════════════════════════════════════════════════════════════════════
//
// What this replaces: a linear 4-way average of four colour scans, one
// dominant-material bump map, and no roughness at all. Underfoot that renders
// as a flat tinted carpet, which is exactly what the operator's fuji-forest
// capture shows -- the single largest non-vegetation gap between our frames
// and a photograph.
//
// The technique, and why it is the one a 2030 release uses:
//
//  1. HEIGHT-AWARE SPLATTING (Mishkinis, "Advanced Terrain Texture Splatting",
//     the standard successor to linear alpha blending). Each material carries
//     a real height field in its colour layer's alpha, and the blend keeps
//     whichever material is physically ON TOP at that texel instead of
//     cross-fading them. Gravel pokes through sand, leaves lie over soil, and
//     material boundaries stop looking like airbrush. Linear blending of
//     photoscans is the number one reason splatted terrain reads as mud.
//
//  2. FULL PBR, NOT A DIFFUSE TINT. Albedo, tangent-space normal, per-texel
//     ROUGHNESS and a cavity AO term all come out of the same blend. The
//     roughness is baked with the Toksvig/LEAN variance rule at every mip
//     (renderer::ground_textures::toksvig_roughness), so as the normal detail
//     averages away with distance the specular lobe widens to match. That is
//     what makes strong ground normals safe: without it, a bump map this
//     strong shimmers viciously at exactly the grazing sun angles that make
//     ground interesting.
//
//  3. TWO-SCALE TILING. A second sample of the dominant material at 8x the
//     tile modulates the fine one, so the 2 m repeat stops reading as a grid.
//     Contrast is preserved because the macro octave modulates rather than
//     averages (averaging two octaves is what flattens the usual "detail
//     texture" approach).
//
//  4. THE PROJECTION IS SHARPENED, NOT BLURRED. The old triplanar spread its
//     weight across all three planes with a pow-4 falloff; at a typical ground
//     direction that is a 3-way overlay of the same texture at different UVs,
//     which destroys about 40% of the scan's contrast before it is ever lit.
//     GROUND_TRIPLANAR_POW makes one plane dominate on near-flat ground (it
//     recovers the contrast AND costs fewer samples, because the sub-threshold
//     planes are skipped), while cliffs still get a real 2-3 plane blend.
//
// f32-AT-PLANET-SCALE (CLAUDE.md defect class): every UV here is computed from
// `pt`, the CAMERA-ANCHORED planet-pinned metre domain built by the caller as
// anchor + inv_model * (world_position - eye). Both terms are small, so there
// is no planet-radius magnitude anywhere in this file's UV math, and the
// anchor's jumps are exact 64 m steps -- which is why every GROUND_TILE_M
// entry must DIVIDE 64 (enforced by ground_textures::shipped_ground_materials
// _parse). Nothing here derives a UV or a phase from a unit direction.
//
// ── MIRRORS data/ground/materials.ron ────────────────────────────────────
// That file is the source of truth; these tables are its GPU-side copy, and
// `ground_textures::shipped_ground_materials_match_the_shader` fails the
// build the moment they drift. Order is the RON's order:
//   0 grass   1 dirt   2 rock   3 sand   4 forest_litter
const GROUND_MAT_COUNT: i32 = 5;
// Layer 8 is the procedural ocean wave tile, not a material; materials append
// around it (colour 0..3 / normal 4..7 are the original quartet, extras from
// 9) so an added material can never shift an existing layer.
const GROUND_LAYER_OCEAN: i32 = 8;
var<private> GROUND_COLOR_LAYER: array<i32, 5> = array<i32, 5>(0, 1, 2, 3, 9);
var<private> GROUND_NORMAL_LAYER: array<i32, 5> = array<i32, 5>(4, 5, 6, 7, 10);
var<private> GROUND_TILE_M: array<f32, 5> = array<f32, 5>(2.0, 2.0, 4.0, 2.0, 2.0);
var<private> GROUND_HEIGHT_CONTRAST: array<f32, 5> =
    array<f32, 5>(0.35, 0.30, 0.55, 0.25, 0.50);
var<private> GROUND_NORMAL_STRENGTH: array<f32, 5> =
    array<f32, 5>(0.85, 0.90, 1.00, 0.70, 1.00);
var<private> GROUND_TINT_STRENGTH: array<f32, 5> =
    array<f32, 5>(0.12, 0.18, 0.28, 0.15, 0.55);
// Unit-LUMINANCE chromaticities (tint_linear / its luminance, computed by
// GroundMaterialDef::tint_chromaticity). Mixing toward one of these rotates
// hue without touching brightness, so a tint can never darken or blow out a
// biome -- the failure that made v0.907 strip hue from the materials outright.
var<private> GROUND_TINT_RGB: array<vec3<f32>, 5> = array<vec3<f32>, 5>(
    vec3<f32>(0.73105, 1.28233, 0.25167),
    vec3<f32>(1.50600, 0.82598, 0.56890),
    vec3<f32>(0.47663, 1.17569, 1.46803),
    vec3<f32>(1.29233, 0.93661, 0.55966),
    vec3<f32>(1.36037, 0.92119, 0.46060),
);
// Presence octave (metres): the whole layer fades against this like every
// other detail octave, so distant terrain is bit-identical to the no-detail
// path and nothing can pop at an LOD split.
const GROUND_PRESENCE_M: f32 = 4.0;
// Macro (repeat-breaking) octave as a multiple of each material's tile.
const GROUND_MACRO_MULT: f32 = 8.0;

// Triplanar sharpness. 4.0 (v0.907) spreads weight over all three planes even
// on flat ground; 12.0 gives the best-aligned plane ~85% and lets the other
// two be skipped entirely.
const GROUND_TRIPLANAR_POW: f32 = 12.0;
// Planes and materials below these weights contribute less than the sampling
// costs; skipping them is most of what pays for the extra maps.
const GROUND_PLANE_CUT: f32 = 0.02;
const GROUND_MAT_CUT: f32 = 0.012;
// Height-blend transition width, in the same units as the weights. Small =
// crisp interlocking edges, large = soft. 0.22 keeps leaf and stone edges
// readable without the single-texel popping a hard threshold gives.
const GROUND_BLEND_DEPTH: f32 = 0.22;
// How far the macro octave is allowed to swing local BRIGHTNESS.
//
// 0.30, not 0.55 (v0.1108.2). Measured on a real capture, the >= 8 m band was
// running at 13.5% of the strip's mean luma while the 2 m photoscan tile it is
// meant to be modulating contributed 1.2% - so the variation read as a separate
// overlay rather than as the ground varying. Halving it, together with the
// anisotropic fade at the use site, is what brings the two back into the same
// order of magnitude.
//
// The deeper point, worth keeping when someone tunes this again: large-scale
// ground variation in the real world is MOISTURE AND LITTER COVER, which change
// hue and gloss far more than they change brightness. A luminance-only swing is
// the wrong channel for the phenomenon, which is why it has to be kept small to
// avoid looking painted on. Routing most of this amplitude into the tint mix and
// into roughness instead - and, further out, replacing the whole octave with
// histogram-preserving stochastic tiling (Heitz & Neyret 2018, HPG) so the
// repeat never needs hiding - is the direction this should go.
const GROUND_MACRO_AMP: f32 = 0.30;
// Cavity AO from the blended height. 0.5 at the mean height leaves the ground
// energy-neutral; crevices go down, ridges up.
const GROUND_AO_AMOUNT: f32 = 0.75;

// ── DEFERRED: make the grass TEXTURE agree with the grass STRANDS ─────────
// The 3D sward is placed by `terrain::grass::grass_clump_gain`, so the ground
// texture underneath it currently clumps somewhere else entirely: tufts stand
// where the scatter says, and the painted grass modulates on its own macro
// octave. Driving GROUND_TILE_M[0]'s macro term from the SAME field would
// make the two agree and is the real next increment for forest/meadow floors.
//
// It does NOT fall out of this change, and mirroring the field naively here
// would be the f32-at-planet-scale defect (CLAUDE.md) in its purest form.
// `grass_clump_gain` is a value noise on a lat/lon lattice in RADIANS:
// GRASS_FIELD_RAD = 1.2557e-6, coarse cell 0.22x = 2.76e-7 rad, fine cell
// 0.09x = 1.13e-7 rad. One f32 ULP of longitude near the antimeridian is
// 2.38e-7 rad -- 2.1x the ENTIRE fine cell and 1.2 ULP for the coarse one.
// A WGSL mirror could not sample that lattice at all; it would alias.
//
// The real fix is to move the clump field itself off lat/lon radians and onto
// the camera-anchored 64 m-modulus metre domain this file already uses (any
// period dividing 64 m stays seamless across an anchor re-snap), so the CPU
// scatter and the shader evaluate one identical field. That is an edit to
// src/terrain/grass.rs plus a CPU/GPU lockstep test, and it MOVES EVERY CLUMP
// ON THE PLANET, so it needs its own increment rather than riding this one.

// Classifier thresholds. There is no per-fragment biome ID on this path, so
// the NASA imagery IS the biome map -- the same principle the vegetation
// scatter already uses ("Real Earth imagery is the planet-wide biome map for
// free", planet_chunks.rs).
//
// ── THESE LIVE IN *RAW* IMAGERY SPACE. READ ground_ungrade BELOW. ─────────
// Measured directly out of the shipped data/planets/earth_albedo.bin (linear,
// before any grading): closed canopy sits at luminance 0.009-0.028 and is
// green-dominant (Fuji 0.0090, Pacific NW 0.0107, Congo 0.0157, taiga 0.0161,
// Iowa 0.0284); grassland and savanna sit at 0.055-0.115 and are RED-dominant
// (Great Plains 0.0553, Serengeti 0.0913, France 0.1146); desert runs to 0.42
// (Sahara 0.4249). So luminance separates forest floor from meadow, and the
// red/green ratio separates vegetated from arid.
//
// What the GPU actually samples is NOT that. `albedo_texture` holds the
// GRADED bake, and the classifier undoes the grading before comparing. Do not
// "recalibrate" these numbers against a graded measurement -- that is the
// v0.1101 defect this block now documents (see ground_ungrade).
const GROUND_GREEN_LO: f32 = 1.02;
const GROUND_GREEN_HI: f32 = 1.14;
const GROUND_CANOPY_LUM_LO: f32 = 0.030;
const GROUND_CANOPY_LUM_HI: f32 = 0.055;
const GROUND_DRY_LUM_LO: f32 = 0.04;
const GROUND_DRY_LUM_HI: f32 = 0.09;
const GROUND_DRY_FADE_LO: f32 = 0.13;
const GROUND_DRY_FADE_HI: f32 = 0.22;
const GROUND_DRY_GRASS_MIX: f32 = 0.45;
const GROUND_SAND_WARM_LO: f32 = 0.02;
const GROUND_SAND_WARM_HI: f32 = 0.08;
const GROUND_SAND_LUM_LO: f32 = 0.18;
const GROUND_SAND_LUM_HI: f32 = 0.32;
const GROUND_ROCK_STEEP_LO: f32 = 0.20;
const GROUND_ROCK_STEEP_HI: f32 = 0.50;
// Snow and ice keep the pure photo: there is no snow scan in the pack, and a
// dirt tile over bright ice reads as mud.
const GROUND_SNOW_LUM_LO: f32 = 0.50;
const GROUND_SNOW_LUM_HI: f32 = 0.68;

// ── Undoing the orbital grading before classifying (v0.1103) ─────────────
//
// THE BUG THIS FIXES. `albedo_texture` does not hold raw Blue Marble. The
// bake (`terrain::planet_surface::grade_albedo`) multiplies every above-sea
// land texel by `land_gain(raw)` -- a calibrated 1.6x, times a shadow lift
// that pulls dark land up a square-root curve, times up to +50% more for
// green-dominant cover. Measured on the shipped grid that total ranges from
// 1.60 (Sahara) to 9.00 (Fuji): a 5.6x spread, and it is a FUNCTION of the
// very luminance being classified. The v0.1101 classifier compared that
// graded value against thresholds measured in RAW space, so:
//
//   * `canopy` was identically ZERO for any raw luminance above ~0.004, and
//     every vegetated texel on Earth measures 0.009-0.13. Across the whole
//     globe, canopy survived on 93.60% of green texels in raw space and on
//     0.19% in graded space -- so forest_litter could never be selected and
//     Fuji, the Congo, the taiga and the Pacific NW all rendered at grass
//     weight 1.000. That is the "mown park" the operator reported, and it is
//     exactly what the classifier comment claimed to prevent.
//   * the SNOW gate (0.50-0.68) caught the Sahara: raw 0.4249 -> graded
//     0.6798, so snowy = 1.00 and `keep` went to zero, switching the entire
//     ground PBR layer OFF across the desert.
//   * the SAND band (0.18-0.32) reached down into temperate farmland: France
//     47N raw 0.1146 -> graded 0.2097, i.e. 12% desert sand under Europe.
//
// WHY INVERT RATHER THAN RETUNE. Re-deriving the thresholds in graded space
// cannot work with one threshold set: the gain is not a constant, it depends
// on the sample's own luminance AND its greenness, so a raw band maps to a
// different graded interval at every chromaticity. Worse, GROUND_SAND_WARM is
// a DIFFERENCE (img.r - img.b) which scales by that same varying gain, so no
// constant rebase exists for it at all. And every retuned number would have
// to be re-derived by hand whenever anyone touched one of four constants in
// a file this shader does not own. The inverse, by contrast, is exact
// (measured 0.00% round-trip error at all ten sample sites) and self-
// maintaining: `ground_textures::wgsl_land_grading_constants_match_the_bake`
// fails the build if the Rust constants move.
//
// THE INVERSE. grade_albedo scales all three channels by ONE scalar, so
// chromaticity survives it untouched -- which is why `gr` (a ratio) was
// always fine and only the luminance tests broke. Greenness is a ratio too,
// so the veg lift is recoverable exactly from the graded colour, and the
// remaining map on luminance is piecewise and continuous at the knee:
//     l >= knee : graded = l * a           -> l = graded / a
//     l <  knee : graded = a * sqrt(knee*l) -> l = graded^2 / (a^2 * knee)
// with a = LAND_GAIN * veg_lift.
//
// PRECONDITION, and the gate that guards it. This is applied unconditionally
// because every planet that can reach `ground_detail` is a graded one: the
// path needs `has_tex`, which needs a baked albedo texture, which the loader
// only builds when the planet ships BOTH a heightmap and an albedo grid, and
// Earth is the only def that ships a heightmap. Mars, the Moon and Pluto have
// `heightmap: None` and `has_water: false`, so their imagery is passed
// through UNGRADED and never reaches this code -- which matters, because
// un-grading them unconditionally would cost Mars 91% of its sand. That
// precondition is not left as a comment: `ground_textures::
// only_graded_planets_can_reach_the_ground_classifier` fails the moment any
// `has_water: false` def gains a heightmap.
//
// Mirrors terrain::planet_surface::{LAND_ALBEDO_GAIN, LAND_SHADOW_KNEE,
// LAND_SHADOW_EXP} and the veg-lift block inside `land_gain`.
const GROUND_LAND_GAIN: f32 = 1.6;
const GROUND_LAND_KNEE: f32 = 0.15;
const GROUND_LAND_VEG_LO: f32 = 0.1;
const GROUND_LAND_VEG_BAND: f32 = 0.5;
const GROUND_LAND_VEG_AMP: f32 = 0.5;

fn ground_ungrade(img: vec3<f32>) -> vec3<f32> {
    let lg = dot(img, vec3<f32>(0.299, 0.587, 0.114));
    if (lg <= 1.0e-6) {
        return img;
    }
    // Greenness is a RATIO, so the bake's scalar gain divides straight out of
    // it: computing it here from the GRADED colour returns the same number
    // land_gain computed from the raw one. (The 0.001 floor in the divisor is
    // the one term that is not scale-free, and it only binds on a texel whose
    // green channel is already blacker than a millionth of daylight.)
    let greenness = clamp((img.g - max(img.r, img.b)) / max(img.g, 0.001), 0.0, 1.0);
    let vt = clamp((greenness - GROUND_LAND_VEG_LO) / GROUND_LAND_VEG_BAND, 0.0, 1.0);
    let a = GROUND_LAND_GAIN * (1.0 + GROUND_LAND_VEG_AMP * (vt * vt * (3.0 - 2.0 * vt)));
    // Above the knee the bake was a plain scale; below it a square root. The
    // two agree exactly at lg = knee * a, so the branch is seamless.
    var l_raw = lg / a;
    if (lg < GROUND_LAND_KNEE * a) {
        l_raw = (lg * lg) / (a * a * GROUND_LAND_KNEE);
    }
    // Rescale at constant chromaticity. Texels whose graded value CLIPPED at
    // 1.0 (only fresh snow and ice do) come back short -- they are gated out
    // by GROUND_SNOW_LUM anyway, which now reads the raw value it was
    // measured against.
    return img * (l_raw / lg);
}

/// Everything the ground detail contributes at one fragment.
struct GroundDetail {
    /// Multiply straight into albedo. Exactly vec3(1) when faded out.
    albedo_mul: vec3<f32>,
    /// World-space normal with the tangent-space bump folded in.
    normal: vec3<f32>,
    /// Absolute perceptual roughness of the ground material here. Blend the
    /// caller's own roughness toward it by `presence`.
    roughness: f32,
    /// Cavity occlusion, 1 = open. Already faded.
    ao: f32,
    /// 0..1 detail presence. 0 means nothing changed and every other field
    /// is its identity, so a caller can skip the whole block on it.
    presence: f32,
};

/// Triplanar sample of one layer, RGBA, skipping planes below the cut and
/// renormalising by what was actually taken.
///
/// `p` and the gradients are in the pinned metre domain; the caller has
/// already rotated the fs_main-top dpdx/dpdy of world_position into it, which
/// is exact because pt = anchor + inv_m * (wp - eye) with anchor/eye constant
/// per draw. Passing true gradients (rather than one analytic LOD) is what
/// engages the x8 anisotropic filter, without which a flat sightline smears.
fn ground_tri4(
    layer: i32,
    p: vec3<f32>,
    w: vec3<f32>,
    gx: vec3<f32>,
    gy: vec3<f32>,
    tile: f32,
) -> vec4<f32> {
    let it = 1.0 / tile;
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    if (w.x > GROUND_PLANE_CUT) {
        acc = acc + w.x * textureSampleGrad(
            ground_tex, ground_samp, p.yz * it, layer, gx.yz * it, gy.yz * it);
        wsum = wsum + w.x;
    }
    if (w.y > GROUND_PLANE_CUT) {
        acc = acc + w.y * textureSampleGrad(
            ground_tex, ground_samp, p.xz * it, layer, gx.xz * it, gy.xz * it);
        wsum = wsum + w.y;
    }
    if (w.z > GROUND_PLANE_CUT) {
        acc = acc + w.z * textureSampleGrad(
            ground_tex, ground_samp, p.xy * it, layer, gx.xy * it, gy.xy * it);
        wsum = wsum + w.z;
    }
    return acc / max(wsum, 0.0001);
}

/// One-plane sample (the dominant plane only) -- used for the macro octave,
/// where a full triplanar would triple the cost for a low-frequency term
/// nobody can localise anyway.
fn ground_plane4(
    layer: i32,
    p: vec3<f32>,
    axis: i32,
    gx: vec3<f32>,
    gy: vec3<f32>,
    tile: f32,
) -> vec4<f32> {
    let it = 1.0 / tile;
    if (axis == 0) {
        return textureSampleGrad(
            ground_tex, ground_samp, p.yz * it, layer, gx.yz * it, gy.yz * it);
    }
    if (axis == 1) {
        return textureSampleGrad(
            ground_tex, ground_samp, p.xz * it, layer, gx.xz * it, gy.xz * it);
    }
    return textureSampleGrad(
        ground_tex, ground_samp, p.xy * it, layer, gx.xy * it, gy.xy * it);
}

/// Material weights from the imagery colour and the local slope. See the
/// GROUND_* classifier constants above for the measurements behind each
/// threshold. Returns weights summing to 1.
///
/// `img` and `lum` MUST be in RAW imagery space -- the caller runs the sample
/// through `ground_ungrade` first. Two of the four tests here (the luminance
/// bands, and `img.r - img.b`) are absolute, so feeding them the graded bake
/// silently disables the material this whole classifier exists to select.
/// Material weights, as a STRUCT rather than `array<f32, 5>`.
///
/// WGSL permits returning an array and naga validates it happily, but the HLSL
/// backend cannot express it: DXC rejected the generated code with "cannot
/// initialize return object of type 'float' with an lvalue of type 'float[5]'"
/// and the app died at device init on the operator's DX12 adapter, having
/// passed every static check including the naga megashader gate. Keep function
/// RETURNS to scalars, vectors and structs; arrays are fine as locals and as
/// module-scope constants.
struct GroundWeights {
    w: vec4<f32>,
    e: f32,
}

fn ground_material_weights(img: vec3<f32>, lum: f32, steep: f32) -> GroundWeights {
    let w_rock = smoothstep(GROUND_ROCK_STEEP_LO, GROUND_ROCK_STEEP_HI, steep);
    let flat = 1.0 - w_rock;
    // Green dominance: the imagery's own green channel against its strongest
    // other channel, which is stable under the exposure differences between
    // imagery tiles in a way an absolute green threshold is not.
    let gr = img.g / max(max(img.r, img.b), 0.003);
    let green = smoothstep(GROUND_GREEN_LO, GROUND_GREEN_HI, gr);
    // Dark AND green = closed canopy. The ground beneath is leaf and needle
    // mat, not lawn -- this is the correction that stops a Japanese conifer
    // forest floor rendering as a golf course.
    let canopy = 1.0 - smoothstep(GROUND_CANOPY_LUM_LO, GROUND_CANOPY_LUM_HI, lum);
    let sand = smoothstep(GROUND_SAND_WARM_LO, GROUND_SAND_WARM_HI, img.r - img.b)
        * smoothstep(GROUND_SAND_LUM_LO, GROUND_SAND_LUM_HI, lum);
    // Warm mid-luminance land is prairie/steppe/stubble: dry grass standing
    // in soil, so grass structure height-blends up through the dirt rather
    // than the whole biome being bare earth.
    let dry = smoothstep(GROUND_DRY_LUM_LO, GROUND_DRY_LUM_HI, lum)
        * (1.0 - smoothstep(GROUND_DRY_FADE_LO, GROUND_DRY_FADE_HI, lum))
        * (1.0 - green)
        * (1.0 - sand);
    let w0 = flat * (green * (1.0 - canopy) + GROUND_DRY_GRASS_MIX * dry);
    let w2 = w_rock;
    let w3 = flat * sand * (1.0 - green);
    let w4 = flat * green * canopy;
    let w1 = max(1.0 - w0 - w2 - w3 - w4, 0.0);
    var out: GroundWeights;
    out.w = vec4<f32>(w0, w1, w2, w3);
    out.e = w4;
    return out;
}

/// The ground detail layer. `pt` is the pinned-domain position, `dir` the
/// planet-local radial unit direction, `up_w` the world-space radial up.
///
/// `img` is the imagery colour at this fragment BEFORE any detail modulation
/// -- which is the GRADED bake, not raw Blue Marble (the raw grid does not
/// exist on the GPU). The two roles it plays are therefore in two different
/// spaces and must not be confused, which is precisely the v0.1101 defect:
///   * CLASSIFICATION uses `ground_ungrade(img)`, because the thresholds were
///     measured on the raw grid;
///   * the COLOUR math keeps `img` itself, because the multiplier handed back
///     is applied by the caller to the graded value it already has.
fn ground_detail(
    img: vec3<f32>,
    pt: vec3<f32>,
    dir: vec3<f32>,
    normal_w: vec3<f32>,
    up_w: vec3<f32>,
    g_x: vec3<f32>,
    g_y: vec3<f32>,
    footprint_m: f32,
) -> GroundDetail {
    var out: GroundDetail;
    out.albedo_mul = vec3<f32>(1.0);
    out.normal = normal_w;
    out.roughness = 1.0;
    out.ao = 1.0;
    out.presence = 0.0;

    // Everything from here to the end of the classifier runs on the RAW
    // imagery estimate, the space GROUND_* was measured in. `img` (graded)
    // resumes at the colour blend below.
    let img_c = ground_ungrade(img);
    let lum = dot(img_c, vec3<f32>(0.299, 0.587, 0.114));
    let snowy = smoothstep(GROUND_SNOW_LUM_LO, GROUND_SNOW_LUM_HI, lum);
    let keep = detail_octave_fade(GROUND_PRESENCE_M, footprint_m) * (1.0 - snowy);
    if (keep <= 0.003) {
        return out;
    }

    let steep = 1.0 - clamp(dot(normal_w, up_w), 0.0, 1.0);
    // Unpacked from the struct into a function-scope `var` array: the loops
    // below index it with a runtime counter, and a local var array is the form
    // every backend lowers cleanly. The struct exists only to cross the
    // function RETURN, which HLSL cannot do with an array.
    let gw = ground_material_weights(img_c, lum, steep);
    var w: array<f32, 5>;
    w[0] = gw.w.x;
    w[1] = gw.w.y;
    w[2] = gw.w.z;
    w[3] = gw.w.w;
    w[4] = gw.e;

    // Triplanar plane weights from the RADIAL direction, never the surface
    // normal: `dir` is smooth over the whole globe, so the projection cannot
    // swim when a slope changes or an LOD split moves a vertex.
    let aw = pow(abs(dir), vec3<f32>(GROUND_TRIPLANAR_POW));
    let tw = aw / max(aw.x + aw.y + aw.z, 0.0001);
    var axis = 0;
    var apk = tw.x;
    if (tw.y > apk) { axis = 1; apk = tw.y; }
    if (tw.z > apk) { axis = 2; apk = tw.z; }

    // Pass 1: colour + height for the materials that are actually present,
    // and pick the dominant one for the macro octave.
    var cs: array<vec4<f32>, 5>;
    var dom = 0;
    var dom_w = -1.0;
    for (var i = 0; i < GROUND_MAT_COUNT; i = i + 1) {
        if (w[i] <= GROUND_MAT_CUT) {
            cs[i] = vec4<f32>(0.5, 0.5, 0.5, 0.5);
            continue;
        }
        cs[i] = ground_tri4(GROUND_COLOR_LAYER[i], pt, tw, g_x, g_y, GROUND_TILE_M[i]);
        if (w[i] > dom_w) {
            dom = i;
            dom_w = w[i];
        }
    }

    // Macro octave: the same material 8x larger, one plane, faded on its own
    // pixel coverage. It shifts the height field (so the blend itself varies
    // over metres, not just the colour) and swings local brightness, which is
    // what stops the fine tile from reading as a grid.
    // FADED ON THE ANISOTROPIC FOOTPRINT, not the isotropic estimate
    // (v0.1108.2). This is the whole bug the operator photographed as "a large
    // low detail voronoi cell texture laying over a smaller higher detail
    // texture", and it is a broken invariant rather than a taste problem: A
    // REPEAT-BREAKING OCTAVE MUST NEVER OUTLIVE THE REPEAT IT HIDES.
    //
    // At a grazing ground pixel 35 m out, one pixel covers 0.05 m across the
    // sightline and 1.04 m ALONG it - 21:1, past the ground sampler's
    // anisotropy clamp - so the hardware drops to a coarse isotropic mip and
    // the 2 m photoscan tile averages to near-flat. The 16 m macro octave is
    // longer than that footprint and survives untouched. Meanwhile
    // `footprint_m` is the ISOTROPIC analytic estimate (dist * 0.0008 = 0.028 m
    // there), 36x smaller than the truth, so the fade believed everything was
    // fully resolved and held the macro at full amplitude over a tile that had
    // already vanished. Measured on a real capture: the >= 8 m band ran 11.6
    // luma against the fine tile's 0.99 - the anti-tiling was 11.7x LOUDER than
    // the tiling it exists to hide.
    //
    // The gradients are right here and already used for the texture fetches, so
    // the honest footprint costs two `length()` calls.
    let macro_tile = GROUND_TILE_M[dom] * GROUND_MACRO_MULT;
    let aniso_fp_m = max(max(length(g_x), length(g_y)), footprint_m);
    let macro_fade = detail_octave_fade(macro_tile, aniso_fp_m);
    var macro_h = 0.0;
    var macro_l = 1.0;
    if (macro_fade > 0.01) {
        let ms = ground_plane4(GROUND_COLOR_LAYER[dom], pt, axis, g_x, g_y, macro_tile);
        macro_h = (ms.a - 0.5) * macro_fade;
        let ml = dot(ms.rgb, vec3<f32>(0.299, 0.587, 0.114)) * 2.0;
        macro_l = mix(1.0, clamp(ml, 0.45, 1.9), GROUND_MACRO_AMP * macro_fade);
    }

    // Height-aware weights (Mishkinis). Each material's presence is offset by
    // its own relief, then only those within GROUND_BLEND_DEPTH of the tallest
    // survive -- so the winner is whatever is physically on top, and the
    // boundary follows the material's real edges instead of a soft ramp.
    var hb: array<f32, 5>;
    var hmax = -1000.0;
    for (var i = 0; i < GROUND_MAT_COUNT; i = i + 1) {
        if (w[i] <= GROUND_MAT_CUT) {
            hb[i] = -1000.0;
            continue;
        }
        hb[i] = w[i] + (cs[i].a + macro_h) * GROUND_HEIGHT_CONTRAST[i];
        hmax = max(hmax, hb[i]);
    }
    // The weights are a partition of unity, so one is always >= 1/5 and this
    // cannot trigger today -- but if a future classifier ever broke that, the
    // subtraction below would hand every material an equal share of garbage.
    if (hmax < -900.0) {
        return out;
    }
    var tot = 0.0;
    for (var i = 0; i < GROUND_MAT_COUNT; i = i + 1) {
        hb[i] = max(hb[i] - hmax + GROUND_BLEND_DEPTH, 0.0);
        tot = tot + hb[i];
    }
    let inv_tot = 1.0 / max(tot, 0.0001);

    // Pass 2: combine. Normals and roughness are sampled only for materials
    // that survived the height blend, which is usually one or two.
    var col = vec3<f32>(0.0);
    var bump = vec2<f32>(0.0);
    var rough = 0.0;
    var hgt = 0.0;
    var tint = vec3<f32>(0.0);
    var tstr = 0.0;
    for (var i = 0; i < GROUND_MAT_COUNT; i = i + 1) {
        let f = hb[i] * inv_tot;
        if (f <= 0.001) {
            continue;
        }
        col = col + f * cs[i].rgb;
        hgt = hgt + f * cs[i].a;
        tint = tint + f * GROUND_TINT_STRENGTH[i] * GROUND_TINT_RGB[i];
        tstr = tstr + f * GROUND_TINT_STRENGTH[i];
        let ns = ground_tri4(GROUND_NORMAL_LAYER[i], pt, tw, g_x, g_y, GROUND_TILE_M[i]);
        // xy is the tangent bump slope; z is implied by the frame below. The
        // ALPHA is the Toksvig-corrected roughness for this mip -- the reason
        // the strong bump above does not shimmer at distance.
        bump = bump + f * (ns.xy * 2.0 - 1.0) * GROUND_NORMAL_STRENGTH[i];
        rough = rough + f * ns.a;
    }

    // The colour layers are energy-neutral multipliers around 0.5 (geometric
    // mean, baked that way in ground_textures::pack_color_layer), so tex * 2
    // is a multiplier whose average is exactly 1: the imagery keeps owning the
    // large-scale colour and the scan contributes pure structure.
    let mul = clamp(col * 2.0 * macro_l, vec3<f32>(0.28), vec3<f32>(2.2));
    // Hue: rotate toward the blended material chromaticity at CONSTANT
    // luminance. This is what puts brown leaf mat under dark green canopy
    // imagery, and it is incapable of darkening or blowing out a biome.
    let base = img * mul;
    let bl = max(dot(base, vec3<f32>(0.299, 0.587, 0.114)), 0.00001);
    let tint_n = tint / max(tstr, 0.0001);
    let shifted = mix(base, tint_n * bl, clamp(tstr, 0.0, 1.0));
    // Handed back as a MULTIPLIER (not a colour) so the caller's own detail
    // layers -- the land noise octaves and the micro noise -- stay independent
    // multiplicative terms instead of being overwritten. The guard on `img`
    // matters: a near-black imagery channel would otherwise divide out to a
    // huge ratio.
    let mul_out = clamp(
        shifted / max(img, vec3<f32>(0.0001)),
        vec3<f32>(0.15),
        vec3<f32>(3.0),
    );
    // Exactly vec3(1) at keep = 0, so the far field is bit-identical to the
    // no-detail path and nothing can pop at an LOD split.
    out.albedo_mul = mix(vec3<f32>(1.0), mul_out, keep);

    // Tangent frame around the radial up. For rough ground the bump
    // ORIENTATION is arbitrary, only its consistency across neighbouring
    // fragments matters, and a frame built from `up_w` is smooth everywhere
    // the pole guard is not straddled.
    let ref_a = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(up_w.y) > 0.9);
    let t1 = normalize(cross(up_w, ref_a));
    let t2 = cross(up_w, t1);
    out.normal = normalize(normal_w + (bump.x * t1 + bump.y * t2) * keep);
    out.roughness = clamp(rough, 0.04, 1.0);
    out.ao = mix(1.0, clamp(0.55 + 0.9 * hgt, 0.35, 1.15), keep * GROUND_AO_AMOUNT);
    out.presence = keep;
    return out;
}

fn surface_detail_noise(dir: vec3<f32>, freq: f32, seed: f32) -> f32 {
    var w = dir * dir;
    w = w * w;
    let wn = w / (w.x + w.y + w.z);
    let p = dir * freq;
    let o = vec2<f32>(seed, seed * 0.713);
    let nx = value_noise(p.yz + o);
    let ny = value_noise(p.zx + o * 1.31);
    let nz = value_noise(p.xy + o * 1.73);
    return nx * wn.x + ny * wn.y + nz * wn.z;
}

// One directional wave train's contribution to the tangent-plane slope
// gradient at planet-local point p_m (metres), sphere normal n. The fixed
// 3D direction d projects onto the local tangent plane, so one constant
// serves the whole globe (the projection degenerates only where d is
// radial -- that octave simply vanishes there, the other five cover it).
// The phase wraps through fract() BEFORE the sin so the argument stays in
// one period -- at planet-radius coordinates (6.4e6 m over a 50 m wave)
// a raw phase would hit ~8e5 rad, where GPU sin precision dies.
fn wave_octave(
    p_m: vec3<f32>,
    n: vec3<f32>,
    d: vec3<f32>,
    lambda_m: f32,
    cps: f32,
    slope: f32,
    t: f32,
    footprint_m: f32,
) -> vec3<f32> {
    let fade = detail_octave_fade_aa(lambda_m, footprint_m);
    if (fade <= 0.001) {
        return vec3<f32>(0.0);
    }
    var tp = d - n * dot(d, n);
    let l = length(tp);
    if (l < 1e-4) {
        return vec3<f32>(0.0);
    }
    tp = tp / l;
    // Phase = distance along the 3D propagation direction d, in wavelengths.
    // MUST dot with d (the raw wave direction), NOT tp: the caller's p_m is
    // the RADIAL planet-local position (p_m = dir * r, parallel to n), and tp
    // is tangent (perpendicular to n), so dot(p_m, tp) is identically ZERO --
    // that collapses the whole ocean to one globally-uniform, time-only phase
    // (no crests, no glitter). dot(p_m, d) = r * dot(dir, d) varies across the
    // surface, giving real travelling wave trains. tp remains the SLOPE
    // (gradient) direction; only the phase argument changes.
    // Fractal domain warp: snake the crests by nudging the phase with TWO
    // octaves of value-noise sampled on the sphere normal n (same planet-local
    // frame as the wave). The coarse octave (WAVE_WARP_MULT * lambda) shifts
    // whole crests by differing amounts; the fine one (WAVE_WARP_MULT2 * lambda)
    // adds local wiggle. Each noise is centred to +-0.5, then scaled to its
    // amplitude in wavelengths and summed before the cos.
    let r_m = length(p_m);
    // Warp gate (v0.1020 perf): the crest-snaking domain warp costs two
    // value-noise evaluations PER OCTAVE per pixel, but the wiggle it adds
    // is invisible once a wavelength spans under ~24 px on screen. Skip
    // both noises there - the far field keeps its straight-crest look
    // (which the AA fade is already blurring out anyway).
    var warp = 0.0;
    let warp_gate = 1.0 - smoothstep(lambda_m * 0.028, lambda_m * 0.042, footprint_m);
    if (warp_gate > 0.001) {
        let warp_seed = WAVE_WARP_SEED + lambda_m * 0.01;
        let warp_c = (surface_detail_noise(n, r_m / (lambda_m * WAVE_WARP_MULT), warp_seed) - 0.5)
            * WAVE_WARP_AMP;
        let warp_f = (surface_detail_noise(n, r_m / (lambda_m * WAVE_WARP_MULT2), warp_seed + 19.7) - 0.5)
            * WAVE_WARP_AMP2;
        // Faded, not cut: a hard boundary would draw a ring where crests
        // suddenly straighten; the phase eases to unwarped instead.
        warp = (warp_c + warp_f) * warp_gate;
    }
    let cycles = dot(p_m, d) / lambda_m + warp + t * cps;
    let ph = fract(cycles) * TAU;
    return tp * (slope * fade * cos(ph));
}

// Swell-only slope gradient (v0.922 near-field rework): the three LONG wave
// trains (2000/360/150 m) keep their analytic shading - at those wavelengths
// the AA fade genuinely protects them. The three FINE trains (50/18/6 m) and
// the micro ripples are shading-retired: analytic trig has no mip chain, so
// up close they aliased into zebra stripes and coherent moire rings
// (operator screenshots, 2026-07-21). Their role now belongs to the mipped
// ocean texture (ocean_tex_gradient below). The fine trains still DISPLACE
// geometry in the vertex shader - silhouette unchanged, CPU swim-height twin
// untouched.
fn water_wave_gradient(p_m: vec3<f32>, n: vec3<f32>, t: f32, footprint_m: f32) -> vec3<f32> {
    var g = wave_octave(p_m, n, WAVE1_DIR, WAVE1_LAMBDA, WAVE1_CPS, WAVE1_SLOPE, t, footprint_m);
    g = g + wave_octave(p_m, n, WAVE2_DIR, WAVE2_LAMBDA, WAVE2_CPS, WAVE2_SLOPE, t, footprint_m);
    g = g + wave_octave(p_m, n, WAVE3_DIR, WAVE3_LAMBDA, WAVE3_CPS, WAVE3_SLOPE, t, footprint_m);
    return g;
}

// ── Ocean detail from the tiling wave texture (v0.922, ground_tex layer 8) ──
// Two scrolled octaves of the procedurally generated random-phase wave tile,
// sampled with the TRUE gradient pair (textureSampleGrad) so the GPU clamps
// screen-space frequency automatically on BOTH footprint axes - the property
// the analytic octaves could never have, and which the pre-fix explicit-LOD
// form only half-had (one isotropic mip for an ellipse up to 16:1). RG =
// tangent slope, B = crest height (foam mask). `p_anch` is the camera-
// anchored planet-local metre domain (the micro-ripple anchor), so UV math
// stays in small floats. Octave tiles 16 m and 64 m both divide the 64 m
// anchor modulus. Returns xyz = tangent-plane gradient, w = crest 0..1.
fn ocean_tex_gradient(p_anch: vec3<f32>, n: vec3<f32>, t: f32, footprint_m: f32) -> vec4<f32> {
    // Stable tangent basis from the sphere normal (poles guarded).
    var up_ref = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(n.y) > 0.94) {
        up_ref = vec3<f32>(1.0, 0.0, 0.0);
    }
    let t1 = normalize(cross(n, up_ref));
    let t2 = cross(n, t1);
    let uv_m = vec2<f32>(dot(p_anch, t1), dot(p_anch, t2));
    // ── ANISOTROPIC FOOTPRINT (the zebra fix, second half) ────────────────
    // The old form picked ONE isotropic mip from `footprint_m`. Fed the
    // across-sightline footprint that under-filtered the long axis by up to
    // 16x at grazing (the alias that reads as zebra bands); fed the
    // along-sightline one it would over-blur the short axis by the same
    // factor and smear the chop laterally. Neither is a filter for an
    // ellipse. Hand the hardware the real gradient PAIR instead and let the
    // x16 anisotropic sampler take taps down the long axis - byte-for-byte
    // the reasoning that fixed the ground in v0.977.
    //
    // Metres per screen step along each footprint axis, in the tangent
    // plane, then mapped through the same (t1, t2) basis as uv_m. The axes
    // come from `water_footprint`; when the caller has not called it (the
    // pre-wiring state, g_water_fp_across == 0) this degrades to an
    // isotropic pair equal to `footprint_m`, which picks exactly the mip the
    // explicit-LOD form used to - so that path is unchanged.
    var f_across = g_water_fp_across;
    var f_along = g_water_fp_across / g_water_ndv;
    var e_along = t1;
    var e_across = t2;
    if (f_across <= 0.0) {
        f_across = footprint_m;
        f_along = footprint_m;
    } else if (dot(g_water_vt, g_water_vt) > 0.25) {
        // Re-project defensively: `n` here is the same sphere normal
        // water_footprint was given, so this is a no-op in practice.
        let a = g_water_vt - n * dot(g_water_vt, n);
        let al = length(a);
        if (al > 1.0e-4) {
            e_along = a / al;
            e_across = cross(n, e_along);
        }
    }
    let d_long = vec2<f32>(dot(e_along, t1), dot(e_along, t2)) * f_along;
    let d_short = vec2<f32>(dot(e_across, t1), dot(e_across, t2)) * f_across;
    // Octave A: 16 m tile, the main chop. Octave B: 64 m tile, slow rollers.
    // Different scroll directions decorrelate the shared content.
    let s_a = textureSampleGrad(
        ground_tex, ground_samp,
        uv_m / 16.0 + vec2<f32>(t * 0.021, t * 0.009), GROUND_LAYER_OCEAN,
        d_long / 16.0, d_short / 16.0);
    let s_b = textureSampleGrad(
        ground_tex, ground_samp,
        uv_m / 64.0 + vec2<f32>(-t * 0.0035, t * 0.0055), GROUND_LAYER_OCEAN,
        d_long / 64.0, d_short / 64.0);
    let g_a = s_a.rg * 2.0 - 1.0;
    let g_b = s_b.rg * 2.0 - 1.0;
    let g2 = g_a * 0.80 + g_b * 0.55;
    // Crest mask: only the TOP of octave A's height range counts as a
    // breaking crest, weighted up where the roller octave also peaks
    // (compound crests foam first). Random-phase heights are near-Gaussian,
    // so the 0.72 threshold keeps coverage to the top few percent.
    let crest = smoothstep(0.72, 0.95, s_a.b) * (0.5 + 0.9 * clamp((s_b.b - 0.5) * 2.0, 0.0, 1.0));
    return vec4<f32>(t1 * g2.x + t2 * g2.y, crest);
}

// Fixed screen-space anti-alias fade (v0.909): like detail_octave_fade but
// WITHOUT the detail-distance multiplier. Waves and foam ANIMATE - pushing
// them past their pixel-coverage bound with the Settings detail slider
// turns them into shimmering speckle (the v0.906 "ocean is mostly white"
// regression at altitude), so animated water content fades on raw pixel
// coverage only. Static land octaves keep the scaled fade.
fn detail_octave_fade_aa(lambda_m: f32, footprint_m: f32) -> f32 {
    // v0.912 (operator: dotted moire patterns on the mid-distance sea):
    // waves at 4-12 px per wavelength sit right at the sampling limit and
    // their few FIXED directions beat against the pixel grid as dot
    // gratings. Water octaves now need 9 px to start fading in and 24 px
    // to reach full strength - the shimmer band simply never renders.
    return smoothstep(9.0, 24.0, lambda_m / footprint_m);
}

// Master water-shading blend: the fade of the LONGEST wave octave. 0 from
// orbit (old path bit-identical), 1 once 2 km swells span DETAIL_FADE_HI
// pixels (~200 km altitude at 1440p), smooth in between.
fn wave_presence(footprint_m: f32) -> f32 {
    return detail_octave_fade_aa(WAVE1_LAMBDA, footprint_m);
}

// Multiplicative land albedo factor: 2-3 octaves of luminance variation
// (each anti-alias faded), clamped so the imagery's own contrast always
// dominates. Returns exactly 1.0 when every octave is faded out (orbit).
fn land_detail_factor(dir: vec3<f32>, r_m: f32, footprint_m: f32) -> f32 {
    var f = 0.0;
    f = f + LAND1_AMP * detail_octave_fade(LAND1_LAMBDA, footprint_m)
        * (2.0 * surface_detail_noise(dir, r_m / LAND1_LAMBDA, LAND1_SEED) - 1.0);
    f = f + LAND2_AMP * detail_octave_fade(LAND2_LAMBDA, footprint_m)
        * (2.0 * surface_detail_noise(dir, r_m / LAND2_LAMBDA, LAND2_SEED) - 1.0);
    f = f + LAND3_AMP * detail_octave_fade(LAND3_LAMBDA, footprint_m)
        * (2.0 * surface_detail_noise(dir, r_m / LAND3_LAMBDA, LAND3_SEED) - 1.0);
    f = f + LAND4_AMP * detail_octave_fade(LAND4_LAMBDA, footprint_m)
        * (2.0 * surface_detail_noise(dir, r_m / LAND4_LAMBDA, LAND4_SEED) - 1.0);
    f = f + LAND5_AMP * detail_octave_fade(LAND5_LAMBDA, footprint_m)
        * (2.0 * surface_detail_noise(dir, r_m / LAND5_LAMBDA, LAND5_SEED) - 1.0);
    return clamp(1.0 + f, 0.62, 1.38);
}

// Full close-range water shading with the wave-perturbed normal:
//   - Schlick Fresnel (F0 = WATER_F0) on the view angle against N';
//   - reflected term: a cheap analytic sky (horizon haze -> zenith blue by
//     the reflected ray's elevation against the LOCAL up = sphere normal,
//     plus a wide sun-tinted glow) -- grazing water mirrors bright sky,
//     straight-down water shows the body color, no reflection probes;
//   - refracted/body term: the graded bathymetry albedo, Lambert-lit by the
//     sun only, darkened at grazing by energy conservation (1 - F);
//   - sun sparkle: tight Blinn lobe on N' (the moving glitter field) plus
//     the v0.810 220-exponent lobe on the smooth normal as the macro
//     anchor so the overall glint region never vanishes.
// Everything is day-gated and SUN-ONLY; a small albedo floor mirrors the
// pipeline's ambient so the night ocean is near-black, not absolute black.
// == Real sky reflection for water (v0.1055) ==
// Operator: "the glassy doesn't really look glass. How do we go about
// reflecting the clouds, land, and plants in the water?"
//
// The sea reflected NOTHING real. water_shade built its mirror from two
// hardcoded literals - a 0.20/0.36/0.55 horizon and a 0.04/0.14/0.38 zenith -
// mixed by elevation and then multiplied by WATER_SKY_GAIN = 0.20. Meanwhile
// the actual Hillaire sky-view LUT for this frame, at this camera altitude and
// this sun elevation, is bound in the SAME bind group at @group(3) @binding(13)
// and was never read. At grazing incidence, where Fresnel goes to 1 and physics
// says the sea must be as bright as the sky it mirrors, ours was 3-5x DARKER,
// the wrong hue at sunset, and collapsed to zero at night. That one multiply is
// why calm water read as flat blue paint rather than glass, and why no cloud
// colour, sunset or haze ever appeared in it.
//
// Same parameterization as 30-atmosphere.wgsl (Hillaire non-linear latitude,
// azimuth measured from the sun on a symmetric half-circle) and the same
// exposure, duplicated here only because the shader parts concatenate in
// filename order and part 20 comes before part 30. LOCKSTEP: if the
// atmosphere mapping or exposure changes, change both.
const WATER_SKY_LUT_EXPOSURE: f32 = 15.0;

fn water_sky_lut(dir: vec3<f32>, up_c: vec3<f32>) -> vec3<f32> {
    let sun_lut = normalize(camera.sun_direction.xyz);
    let l_elev = asin(clamp(dot(dir, up_c), -1.0, 1.0));
    // CLAMP TO THE UPPER HEMISPHERE. A wave tilted at a grazing view reflects
    // slightly BELOW the local horizon, and the lower half of the sky LUT is
    // near-black - which drew a band of hard black dashes along the horizon the
    // moment the real mirror went in (measured in the rig). A ray that would
    // reflect into the sea instead takes the horizon radiance, which is what it
    // would actually pick up after one more bounce off the water in front of it.
    // The 0.004 floor conditions the sqrt (increment 7c): d(v)/d(elev) is
    // INFINITE at zero elevation, so a milliradian of normal wiggle right at
    // the horizon jumped whole texels of the LUT - which printed the wave
    // mesh's triangle edges as hard grazing bands. At 0.004 rad the
    // derivative is finite (~3.2) and the band collapses below one texel.
    let v_lut = clamp(0.5 + 0.5 * sqrt(max(l_elev, 0.004) / (PI * 0.5)), 0.5, 1.0);
    let sun_h = sun_lut - up_c * dot(sun_lut, up_c);
    let view_h = dir - up_c * dot(dir, up_c);
    let sh_len = length(sun_h);
    let vh_len = length(view_h);
    var u_lut = 0.25;
    if (sh_len > 1e-4 && vh_len > 1e-4) {
        let cphi = clamp(dot(sun_h / sh_len, view_h / vh_len), -1.0, 1.0);
        u_lut = acos(cphi) / (2.0 * PI);
    }
    // BELOW-HORIZON reflections pick up the SEA in front of the wave (one
    // more bounce, Fresnel-dim), not bright horizon sky. The pre-increment-7
    // code got a crude version of this by accident: dipped rays sampled the
    // LUT's v = 0.5 seam, where bilinear filtering averages the horizon row
    // with the near-black below-horizon half - roughly halving their
    // radiance. The conditioning floor above removed that accident, and at
    // storm sea (where MOST mid-field grazing reflections dip) the whole sea
    // washed out into a bright gray sheet (measured +70% band mean). This
    // term restores the physics deliberately: dipped rays fade to ~52% of
    // horizon radiance, smoothly, with no derivative cliff at zero.
    //
    // The transition width ADAPTS to the sea state, because the two failure
    // modes it must dodge live at opposite ends (all three measured):
    // - CALM, narrow band (0.055 rad): the mesh-interpolated normal wiggle
    //   (~0.01-0.02 rad) sweeps the band pixel-to-pixel and the dip factor
    //   itself prints the triangle lattice - 33.8% autocorr, WORSE than the
    //   27.9% pre-fix banding. Calm needs a WIDE, gentle curve (and calm
    //   has few deeply-dipped rays, so the width costs nothing).
    // - STORM, wide band (0.15 rad): facet slopes (~0.28 rad rms) spread
    //   across the whole curve, so the factor VARIES facet-to-facet where
    //   the old seam-average was one constant - band speckle measured 2.4x
    //   its golden and the mean ran +39%. Storm needs a NARROW step to a
    //   uniform plateau, and any lattice hides under real facet chaos.
    let dip_w = mix(0.16, 0.04, water_sea_state01());
    let dip = 1.0 - smoothstep(-dip_w, 0.0, l_elev);
    return textureSampleLevel(sky_view_tex, albedo_sampler, vec2<f32>(u_lut, v_lut), 0.0).rgb
        * WATER_SKY_LUT_EXPOSURE * mix(1.0, 0.52, dip);
}

fn water_shade(
    albedo: vec3<f32>,
    n_geo: vec3<f32>,
    n_pert: vec3<f32>,
    view_dir: vec3<f32>,
    // How much of the sea-slope spectrum n_pert actually RESOLVES, 0..1
    // (increment 7): the wave shell passes presence * tex_reach, the calm
    // backstop and anything at orbit pass 0. Drives the specular-lobe
    // width (subtraction form, see WATER_MSS_RESOLVED_FRAC), the mirror
    // direction (unresolved normals must not print their triangle lattice
    // into the sky LUT) and the tight anchor glint (near-field only - at a
    // 25 km orbit footprint a 220-power lobe is pure alias energy).
    resolved: f32,
    // Sun shadow attenuation, 1 = unshadowed (v0.1057). Water was the only lit
    // surface in the engine that never sampled the shadow map at all: the
    // megashader has exactly ONE sun_shadow call site, in the shared PBR tail
    // that the type-16 branch early-returns before ever reaching. So an island,
    // a cliff or a wave in front cast nothing onto the sea. Only the SUN terms
    // take it - the reflected sky arrives from the whole hemisphere and is not
    // blocked by a single occluder.
    sun_shadow_f: f32,
) -> vec3<f32> {
    let sun_l = normalize(camera.sun_direction.xyz);
    let sun_i = camera.sun_direction.w;
    // Sun elevation from the GEOMETRIC normal - this is the day/night and
    // terminator factor, and it must NOT follow the waves or a wave tilted
    // toward a set sun would light up at night.
    let day = clamp(dot(n_geo, sun_l), 0.0, 1.0);
    // ── PER-FACET DIFFUSE (v0.1054) ──
    // Operator: "I can see waves behind waves at the most extreme setting. Like
    // there's no extra shading." Water is the one lit surface in the engine that
    // never reaches the shared PBR tail (the type-16 branch early-returns), so
    // ALL of its lighting is this function - and every diffuse term here used
    // the geometric sphere normal. The wave-perturbed normal reached only the
    // Fresnel, the sky ramp and the specular lobe, so wave FACES had no
    // light-and-shade whatsoever: a sunlit slope and a shaded slope of the same
    // wave returned identical body colour. That is what makes a big sea read as
    // a flat patterned plane rather than as relief.
    //
    // Real water is a poor diffuse reflector, so this is deliberately gentle -
    // a Lambert term on the perturbed normal, blended with the geometric one so
    // the near-field gains facet shading while the far field (where n_pert
    // relaxes to n_geo anyway) is unchanged. Multiplied by `day` so it cannot
    // manufacture light at night.
    let facet = clamp(dot(n_pert, sun_l), 0.0, 1.0);
    let day_facet = day * mix(1.0, clamp(facet / max(day, 0.05), 0.0, 1.6), 0.65);
    let cos_v = clamp(dot(n_pert, view_dir), 0.0, 1.0);
    let t1 = 1.0 - cos_v;
    let t2 = t1 * t1;
    let f = WATER_F0 + (1.0 - WATER_F0) * t2 * t2 * t1;
    // MEAN-NORMAL MIRROR (increment 7b): the sky reflection follows the
    // wave normal only to the degree the wave field is actually resolved.
    // Where it is not (mid-field grazing, cross-LOD seams), n_pert's
    // piecewise-linear triangle interpolation printed the mesh lattice
    // straight into the LUT mirror; retiring the mirror to the geometric
    // normal is exactly what the eye does with sub-resolution slopes - they
    // belong in the specular LOBE (widened below), not the mean direction.
    let refl = normalize(mix(
        reflect(-view_dir, n_geo),
        reflect(-view_dir, n_pert),
        resolved,
    ));
    let elev = clamp(dot(refl, n_geo), 0.0, 1.0);
    // Reflected-sky ramp, SATURATED toward ocean blue (v0.819): the old
    // near-white horizon (0.62,0.7,0.8) made every grazing wave crest flash
    // stark white -- reading as foam we do not simulate. A deeper, bluer ramp
    // makes crests reflect as blue sky (foam-free open ocean), the biggest
    // single realism lever after the phase fix.
    // v0.826: deepened further. At 1.5 km the grazing sky mirror painted every
    // mid-field crest a bright cross-hatch (the "corduroy" band + the operator's
    // "uniform lines"). A deeper, more saturated blue makes grazing crests read
    // as blue swell, not white lines, so the sun glitter carries the highlights.
    let horizon = vec3<f32>(0.20, 0.36, 0.55);
    let zenith = vec3<f32>(0.04, 0.14, 0.38);
    var sky = mix(horizon, zenith, pow(elev, 0.6));
    sky = sky + camera.sun_color.rgb * pow(max(dot(refl, sun_l), 0.0), 8.0) * 0.18;
    var sky_term = sky * (day * sun_i * WATER_SKY_GAIN);
    // The REAL sky, when this frame LUT is valid (shadow_u.params2.y is the
    // rendered-this-frame flag, the same gate the atmosphere uses). No daylight
    // factor and no 0.20 gain: the LUT already carries true radiance, so the
    // mirror is as bright as the sky it reflects, which is the whole point -
    // and it darkens at night on its own because the sky does.
    //
    // W1 ALTITUDE GATE (environment program): the mirror follows the SAME
    // retirement the atmosphere applies to this LUT toward orbit
    // (camera.light1_cone_inner.w = (1 - max(w_alt, w_far)) computed by
    // lib.rs per frame with the shared NEAR_R/FAR_R constants). The water
    // used to mirror the table unconditionally at exposure 15 while the
    // drawn sky gated it to ZERO from orbit - which painted the operator's
    // cyan banding on the horizon and around the orbital glint. Fading to
    // the procedural ramp keeps a plausible mirror at every altitude.
    let w_lut_alt = clamp(camera.light1_cone_inner.w, 0.0, 1.0);
    if (shadow_u.params2.y > 0.5 && w_lut_alt > 0.001) {
        sky_term = mix(sky_term, water_sky_lut(refl, n_geo), w_lut_alt);
    }
    // ── THE MIRROR MUST SEE THE WEATHER TOO ──
    // Everything above answers "what is the sky radiance in the reflected
    // direction" from the Hillaire sky-view LUT, which is a CLEAR-AIR table: it
    // knows the sun, the altitude and the air, and nothing at all about fog,
    // dust, rain or snow. Those live in the separate weather-haze pair the CPU
    // publishes each frame (sigma in light1_cone_inner.y, the tinted airlight in
    // light2_cone_inner.yzw). So in a whiteout the sea kept mirroring a bright
    // blue sky that no longer existed - and at grazing angles, where Fresnel
    // goes to 1, that mirror IS the sea's colour. The eye-path haze then washed
    // the far sea toward the fog while the near sea kept the unfogged mirror,
    // so the two disagreed across the frame and the disagreement SWEPT with the
    // view: the operator's "washes out to near-white and pulsates between dark
    // blue and white".
    //
    // The reflected ray travels through the same weather the eye does, so it
    // gets the same integral: extinction over one slant path of the haze layer,
    // taken along the REFLECTED direction (elev = its elevation cosine against
    // the local up). This is deliberately the SAME law, the same uniforms and
    // the same 1e-4 weather gate that 30-atmosphere.wgsl applies to the drawn
    // sky (v0.1108, "one fog, one integral, or they cannot agree") - now
    // extended to the third leg, the sky the sea reflects. LOCKSTEP: if the
    // atmosphere's fog mix changes, change this with it.
    //
    // Clear air is BIT-IDENTICAL: clear-air sigma is 2.2e-5, an order of
    // magnitude under the gate, so every clear-weather capture and every frozen
    // ocean golden goes through this untouched.
    let fog_sigma_w = camera.light1_cone_inner.y;
    if (fog_sigma_w > 1.0e-4) {
        let fog_rgb_w = vec3<f32>(
            camera.light2_cone_inner.y,
            camera.light2_cone_inner.z,
            camera.light2_cone_inner.w,
        );
        let layer_w = max(camera.light1_cone_inner.z, 1.0);
        let w_fog_w = clamp(
            1.0 - exp(-fog_sigma_w * (layer_w / max(elev, 0.035))),
            0.0,
            1.0,
        );
        sky_term = mix(sky_term, fog_rgb_w, w_fog_w);
    }
    let body = albedo * camera.sun_color.rgb * (sun_i * day_facet / PI) * sun_shadow_f;
    let h = normalize(view_dir + sun_l);
    // GLITTER WIDTH FROM THE WIND (v0.1055, operator: "the sun reflects but
    // there is some weird hard lines"). WATER_SPEC_POWER is a fixed 900-power
    // Blinn lobe - a 2.7 degree half-angle - evaluated on a normal field that is
    // only piecewise-smooth, so the lobe is far narrower than the surface it
    // samples and the highlight breaks into hard slivers along the interpolation
    // seams. Cox and Munk measured the real thing: mean-square slope
    // 0.003 + 0.00512 * U10, an effective roughness 3-5x wider than this lobe at
    // any real wind. Widening it to the physical value removes the slivers AND
    // makes the glitter path spread with the wind the way it does on a real sea.
    // Wind comes from the sea state already in the uniform - no new plumbing.
    let u10 = 2.0 + 13.0 * water_sea_state01();
    let mss = 0.003 + 0.00512 * u10;
    // SUBTRACTION FORM (increment 7a): Cox-Munk is the TOTAL slope
    // variance, and n_pert already resolves the swell + texture part of it
    // near the camera - so the lobe carries only what the normal does NOT
    // resolve. resolved = 0 (orbit, backstop) keeps the full distribution;
    // resolved = 1 leaves the ~15% capillary tail, tightening near-field
    // glitter that the double-count used to wash wide. d_norm below makes
    // the width change pure redistribution - no energy is added or lost.
    let mss_lobe = mss * (1.0 - WATER_MSS_RESOLVED_FRAC * resolved);
    // Blinn power equivalent to GGX alpha = sqrt(mss): p = 2/alpha^2 - 2.
    let spec_p = max(2.0 / max(mss_lobe, 1.0e-4) - 2.0, 8.0);
    // ── THE GLITTER BLOWOUT FIX (operator: "the ocean is getting blown
    // out white and behaving very weird visually") ──
    //
    // The old term was `pow(dot,p) * GAIN` added RAW, and it had two
    // independent errors that stacked:
    //
    // 1. NO FRESNEL. `f` weights the body and the sky mirror, but the
    //    specular was added unweighted. Physically the glitter carries
    //    the same Fresnel reflectance, which near the glint point is
    //    only ~0.02 - so the term ran roughly FORTY TIMES too bright
    //    exactly where the blowout appears.
    // 2. NO ENERGY NORMALIZATION. A Blinn lobe must scale its peak by
    //    (p + 2) / 2pi so that WIDENING the lobe redistributes energy
    //    rather than adding it. With a fixed peak, total glitter energy
    //    scaled as 1/mss - i.e. with the WIND. That is why this never
    //    appeared on the probe rig and always appears in play: every rig
    //    capture runs "Clear 20C 2 m/s" (p = 149, a 1.6 degree cap, the
    //    8 px white dot visible in old orbital captures), while live
    //    weather at 10-15 m/s drops p to 35-23, growing the saturated
    //    cap 2-2.4x in radius and 4-6x in energy until a quarter of the
    //    visible disc is clipped pure white.
    //
    // Now a proper normalized microfacet lobe: widening with the wind
    // spreads the glitter path without brightening it, which is what a
    // real sea does. WATER_SPEC_GAIN stays the single artistic scalar.
    let d_norm = (spec_p + 2.0) / (2.0 * PI);
    let f_h = WATER_F0 + (1.0 - WATER_F0)
        * pow(1.0 - max(dot(view_dir, h), 0.0), 5.0);
    // The 1/(4 n_l n_v) microfacet denominator must be BOUNDED. A real
    // Cook-Torrance carries a geometry/shadowing term that suppresses
    // grazing angles; without one, n_v -> 0 at the horizon amplifies the
    // lobe without limit - which is where the operator saw banding
    // across the water near the horizon. Clamping the product caps the
    // amplification at 5x and stands in for the missing G term.
    let n_l = max(dot(n_geo, sun_l), 0.0);
    let n_v = max(dot(n_geo, view_dir), 0.0);
    let denom = 4.0 * max(n_l * n_v, 0.05);
    let sparkle = d_norm * pow(max(dot(n_pert, h), 0.0), spec_p) / denom;
    // The anchor keeps a tight glint on the geometric normal; it rides
    // the same normalization so it cannot clip on its own either. Gated by
    // `resolved` (increment 7a): it exists to give the NEAR field a hot
    // core inside the wide glitter path - at orbit a 220-power lobe is far
    // narrower than the pixel footprint warrants, and evaluated on
    // interpolated mesh normals it can only print the vertex lattice.
    let anchor_p = 220.0;
    let anchor = ((anchor_p + 2.0) / (2.0 * PI))
        * pow(max(dot(n_geo, h), 0.0), anchor_p) / denom * (0.06 * resolved);
    let spec = camera.sun_color.rgb * sun_i * WATER_SPEC_GAIN * f_h
        * (sparkle + anchor) * day * sun_shadow_f;
    return body * (1.0 - f) + sky_term * f + spec + albedo * 0.005;
}

