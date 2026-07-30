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
// Water Fresnel reflectance at normal incidence (n = 1.33 -> ~0.02).
const WATER_F0: f32 = 0.02;
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
// seamless across the 64 m anchor jumps -- GROUND_TILE_M divides 64).
const GROUND_TILE_M: f32 = 2.0;

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
    let uv_x = p.yz / GROUND_TILE_M;
    let uv_y = p.xz / GROUND_TILE_M;
    let uv_z = p.xy / GROUND_TILE_M;
    return textureSampleGrad(ground_tex, ground_samp, uv_x, layer, gx.yz / GROUND_TILE_M, gy.yz / GROUND_TILE_M).rgb * w.x
        + textureSampleGrad(ground_tex, ground_samp, uv_y, layer, gx.xz / GROUND_TILE_M, gy.xz / GROUND_TILE_M).rgb * w.y
        + textureSampleGrad(ground_tex, ground_samp, uv_z, layer, gx.xy / GROUND_TILE_M, gy.xy / GROUND_TILE_M).rgb * w.z;
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
// sampled with explicit mip LOD so the GPU clamps screen-space frequency
// automatically - the property the analytic octaves could never have. RG =
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
    // Octave A: 16 m tile, the main chop. Octave B: 64 m tile, slow rollers.
    // Different scroll directions decorrelate the shared content.
    let lod_a = clamp(log2(max(footprint_m * 2048.0 / 16.0, 1.0)), 0.0, 11.0);
    let s_a = textureSampleLevel(
        ground_tex, ground_samp,
        uv_m / 16.0 + vec2<f32>(t * 0.021, t * 0.009), 8, lod_a);
    let lod_b = clamp(log2(max(footprint_m * 2048.0 / 64.0, 1.0)), 0.0, 11.0);
    let s_b = textureSampleLevel(
        ground_tex, ground_samp,
        uv_m / 64.0 + vec2<f32>(-t * 0.0035, t * 0.0055), 8, lod_b);
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
    let v_lut = clamp(0.5 + 0.5 * sqrt(max(l_elev, 0.0) / (PI * 0.5)), 0.5, 1.0);
    let sun_h = sun_lut - up_c * dot(sun_lut, up_c);
    let view_h = dir - up_c * dot(dir, up_c);
    let sh_len = length(sun_h);
    let vh_len = length(view_h);
    var u_lut = 0.25;
    if (sh_len > 1e-4 && vh_len > 1e-4) {
        let cphi = clamp(dot(sun_h / sh_len, view_h / vh_len), -1.0, 1.0);
        u_lut = acos(cphi) / (2.0 * PI);
    }
    return textureSampleLevel(sky_view_tex, albedo_sampler, vec2<f32>(u_lut, v_lut), 0.0).rgb
        * WATER_SKY_LUT_EXPOSURE;
}

fn water_shade(
    albedo: vec3<f32>,
    n_geo: vec3<f32>,
    n_pert: vec3<f32>,
    view_dir: vec3<f32>,
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
    let refl = reflect(-view_dir, n_pert);
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
    if (shadow_u.params2.y > 0.5) {
        sky_term = water_sky_lut(refl, n_geo);
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
    let u10 = 2.0 + 13.0 * clamp(camera.fill_color.w, 0.0, 1.0);
    let mss = 0.003 + 0.00512 * u10;
    // Blinn power equivalent to GGX alpha = sqrt(mss): p = 2/alpha^2 - 2.
    let spec_p = max(2.0 / max(mss, 1.0e-4) - 2.0, 8.0);
    let sparkle = pow(max(dot(n_pert, h), 0.0), spec_p) * WATER_SPEC_GAIN;
    let anchor = pow(max(dot(n_geo, h), 0.0), 220.0) * 0.15;
    let spec = camera.sun_color.rgb * sun_i * (sparkle + anchor) * day * sun_shadow_f;
    return body * (1.0 - f) + sky_term * f + spec + albedo * 0.005;
}

