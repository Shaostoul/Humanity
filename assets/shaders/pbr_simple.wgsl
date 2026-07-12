// pbr_simple.wgsl — Cook-Torrance GGX PBR shader with procedural materials.
//
// Bind groups:
//   Group 0: Camera (view_proj, view_pos)
//   Group 1: Object (model, normal_matrix) — dynamic offset
//   Group 2: Material (base_color, params: metallic/roughness/material_type)
//   Group 3: Albedo texture + sampler (v0.811, per-pixel planet imagery).
//            Every pipeline sharing this shader binds SOMETHING here: draws
//            without real imagery get a 1x1 white fallback, so only material
//            type 12 with params.w > 0.5 ever actually samples it.

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    // Point lights: xyz = position, w = intensity. Up to 8 lights.
    light0: vec4<f32>,
    light1: vec4<f32>,
    light2: vec4<f32>,
    light3: vec4<f32>,
    light4: vec4<f32>,
    light5: vec4<f32>,
    light6: vec4<f32>,
    light7: vec4<f32>,
    // xyz = color for each light, w = radius
    light0_color: vec4<f32>,
    light1_color: vec4<f32>,
    light2_color: vec4<f32>,
    light3_color: vec4<f32>,
    light4_color: vec4<f32>,
    light5_color: vec4<f32>,
    light6_color: vec4<f32>,
    light7_color: vec4<f32>,
    // Spot cone aim (v0.639): xyz = aim direction (light-to-fragment sense), w = cos(outer
    // cone half-angle). w == -1.0 is the Point/Bar sentinel -- no cone, skipped entirely.
    light0_spot: vec4<f32>,
    light1_spot: vec4<f32>,
    light2_spot: vec4<f32>,
    light3_spot: vec4<f32>,
    light4_spot: vec4<f32>,
    light5_spot: vec4<f32>,
    light6_spot: vec4<f32>,
    light7_spot: vec4<f32>,
    // Spot cone inner angle: x = cos(inner cone half-angle), yzw unused.
    light0_cone_inner: vec4<f32>,
    light1_cone_inner: vec4<f32>,
    light2_cone_inner: vec4<f32>,
    light3_cone_inner: vec4<f32>,
    light4_cone_inner: vec4<f32>,
    light5_cone_inner: vec4<f32>,
    light6_cone_inner: vec4<f32>,
    light7_cone_inner: vec4<f32>,
    // x = number of active point lights
    light_count: vec4<f32>,
    // Directional sun light: xyz = direction (toward light), w = intensity
    sun_direction: vec4<f32>,
    // Sun color: rgb, w = unused
    sun_color: vec4<f32>,
    // Fill light: xyz = direction, w = intensity
    fill_direction: vec4<f32>,
    // Fill color: rgb, w = unused
    fill_color: vec4<f32>,
};

struct ObjectUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};

struct MaterialUniforms {
    base_color: vec4<f32>,
    // x = metallic, y = roughness, z = material_type, w = emissive_strength
    params: vec4<f32>,
};

// One scene light in the UNCAPPED storage-buffer list (v0.782). Packing
// matches Renderer::set_point_lights: pos_intensity = [pos.xyz, intensity],
// color_range = [rgb, range], spot = [aim.xyz, cos_outer (-1 = no cone)],
// cone_inner = [cos_inner, 0, 0, 0]. The light0..7 fields above are legacy
// (unused, kept so no uniform byte offset shifts); camera.light_count.x
// bounds the loop over this buffer.
struct GpuLight {
    pos_intensity: vec4<f32>,
    color_range: vec4<f32>,
    spot: vec4<f32>,
    cone_inner: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<storage, read> scene_lights: array<GpuLight>;
@group(1) @binding(0) var<uniform> object: ObjectUniforms;
@group(2) @binding(0) var<uniform> material: MaterialUniforms;
// Per-pixel planet albedo imagery (v0.811): an equirectangular sRGB texture
// (sampling returns LINEAR automatically) with the orbital-look grading
// already baked in at upload time (terrain::planet_surface::
// bake_albedo_rgba). Non-planet draws bind a 1x1 white fallback and never
// sample it (the type-12 params.w flag gates the lookup), so this group is
// harmless to every other material type.
@group(3) @binding(0) var albedo_texture: texture_2d<f32>;
@group(3) @binding(1) var albedo_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = object.model * vec4<f32>(vertex.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((object.normal_matrix * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.uv = vertex.uv;
    return out;
}

// ── Lighting ──

const PI: f32 = 3.14159265359;

// Directional lights are now driven from Rust via CameraUniforms.
// camera.sun_direction.xyz = direction, .w = intensity
// camera.sun_color.rgb = color
// camera.fill_direction.xyz = direction, .w = intensity
// camera.fill_color.rgb = color

// Ambient
const AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.03, 0.03, 0.05);

// ── GGX PBR Functions ──

// Normal Distribution Function (Trowbridge-Reitz GGX)
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// Geometry function (Schlick-GGX)
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// Smith's method: combined geometry for both view and light directions
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let t = clamp(1.0 - cos_theta, 0.0, 1.0);
    let t2 = t * t;
    return f0 + (vec3<f32>(1.0) - f0) * (t2 * t2 * t);
}

// ── Procedural Patterns ──

// Hash function for procedural noise
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + vec3<f32>(dot(p3, vec3<f32>(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep

    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// FBM (fractal Brownian motion) — 4 octaves
fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pp = p;
    for (var i = 0; i < 4; i = i + 1) {
        value = value + amplitude * value_noise(pp * frequency);
        frequency = frequency * 2.0;
        amplitude = amplitude * 0.5;
    }
    return value;
}

// Panel seam grid (1m panels with 3cm seam lines)
fn grid_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let seam_width = 0.03;
    let fu = fract(u);
    let fv = fract(v);
    let su = smoothstep(0.0, seam_width, fu) * smoothstep(0.0, seam_width, 1.0 - fu);
    let sv = smoothstep(0.0, seam_width, fv) * smoothstep(0.0, seam_width, 1.0 - fv);
    return su * sv;
}

// Brushed metal pattern (directional micro-scratches).
// v0.696 fix: the old vec2(u * 200.0, 0.0) sampled noise along ONE axis with
// the other pinned to zero -- mathematically that is unbroken full-length
// stripes, which is exactly the "horizontal or vertical lines of varied
// colors" the operator screenshotted. A cross-axis coordinate keeps the
// brushed DIRECTION while ending each scratch, and a low-frequency 2D breakup
// varies the field so it reads as metal, not wallpaper.
fn brushed_metal(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let scratch = value_noise(vec2<f32>(u * 200.0, v * 7.0));
    let breakup = value_noise(vec2<f32>(u * 3.0, v * 3.0));
    return mix(0.85, 1.0, scratch * 0.7 + breakup * 0.3);
}

// Concrete texture (rough, speckled surface)
fn concrete_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    let noise = fbm(uv * 3.0);
    let speckle = value_noise(uv * 40.0) * 0.08;
    // Slightly varied grey
    let base = 0.55 + noise * 0.15 + speckle;
    return vec3<f32>(base, base * 0.98, base * 0.96);
}

// Tri-planar UV projection (reusable helper)
fn triplanar_uv(world_pos: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        return world_pos.xz;
    } else if an.x > an.z {
        return world_pos.yz;
    }
    return world_pos.xy;
}

// Voronoi cell noise (returns distance to nearest cell center)
fn voronoi(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var min_dist = 1.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell_center = vec2<f32>(hash21(i + neighbor), hash21(i + neighbor + vec2<f32>(57.0, 113.0)));
            let diff = neighbor + cell_center - f;
            min_dist = min(min_dist, dot(diff, diff));
        }
    }
    return sqrt(min_dist);
}

// Wood grain pattern
fn wood_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    // Ring pattern along one axis
    let ring = sin(uv.x * 25.0 + fbm(uv * 2.0) * 6.0) * 0.5 + 0.5;
    let grain = value_noise(vec2<f32>(uv.x * 2.0, uv.y * 80.0)) * 0.1;
    // Warm wood tones
    let dark = vec3<f32>(0.35, 0.2, 0.1);
    let light = vec3<f32>(0.6, 0.4, 0.2);
    return mix(dark, light, ring) + vec3<f32>(grain);
}

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
// Analytic reflected-sky brightness (fraction of sun intensity).
const WATER_SKY_GAIN: f32 = 0.5;
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
const WAVE1_LAMBDA: f32 = 2000.0;
const WAVE1_CPS: f32 = 0.028;
const WAVE1_SLOPE: f32 = 0.06;
const WAVE1_DIR: vec3<f32> = vec3<f32>(0.7071068, 0.0, 0.7071068);
const WAVE2_LAMBDA: f32 = 850.0;
const WAVE2_CPS: f32 = 0.045;
const WAVE2_SLOPE: f32 = 0.07;
const WAVE2_DIR: vec3<f32> = vec3<f32>(0.9622504, 0.1924501, 0.1924501);
const WAVE3_LAMBDA: f32 = 360.0;
const WAVE3_CPS: f32 = 0.07;
const WAVE3_SLOPE: f32 = 0.09;
const WAVE3_DIR: vec3<f32> = vec3<f32>(0.2672612, 0.5345225, 0.8017837);
const WAVE4_LAMBDA: f32 = 150.0;
const WAVE4_CPS: f32 = 0.105;
const WAVE4_SLOPE: f32 = 0.1;
const WAVE4_DIR: vec3<f32> = vec3<f32>(-0.5773503, 0.5773503, 0.5773503);
const WAVE5_LAMBDA: f32 = 80.0;
const WAVE5_CPS: f32 = 0.145;
const WAVE5_SLOPE: f32 = 0.11;
const WAVE5_DIR: vec3<f32> = vec3<f32>(0.4082483, -0.8164966, 0.4082483);
const WAVE6_LAMBDA: f32 = 50.0;
const WAVE6_CPS: f32 = 0.18;
const WAVE6_SLOPE: f32 = 0.12;
const WAVE6_DIR: vec3<f32> = vec3<f32>(-0.6666667, 0.3333333, -0.6666667);

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

// Per-octave anti-alias fade: how many projected pixels one wavelength
// spans, smoothstepped through the visibility band. Exactly 0 when the
// octave would alias, exactly 1 when it is comfortably resolved.
fn detail_octave_fade(lambda_m: f32, footprint_m: f32) -> f32 {
    return smoothstep(DETAIL_FADE_LO, DETAIL_FADE_HI, lambda_m / footprint_m);
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
    let fade = detail_octave_fade(lambda_m, footprint_m);
    if (fade <= 0.001) {
        return vec3<f32>(0.0);
    }
    var tp = d - n * dot(d, n);
    let l = length(tp);
    if (l < 1e-4) {
        return vec3<f32>(0.0);
    }
    tp = tp / l;
    let cycles = dot(p_m, tp) / lambda_m + t * cps;
    let ph = fract(cycles) * TAU;
    return tp * (slope * fade * cos(ph));
}

// Sum of all six wave octaves: the height-field slope gradient in the
// tangent plane. The perturbed water normal is normalize(n - this).
fn water_wave_gradient(p_m: vec3<f32>, n: vec3<f32>, t: f32, footprint_m: f32) -> vec3<f32> {
    var g = wave_octave(p_m, n, WAVE1_DIR, WAVE1_LAMBDA, WAVE1_CPS, WAVE1_SLOPE, t, footprint_m);
    g = g + wave_octave(p_m, n, WAVE2_DIR, WAVE2_LAMBDA, WAVE2_CPS, WAVE2_SLOPE, t, footprint_m);
    g = g + wave_octave(p_m, n, WAVE3_DIR, WAVE3_LAMBDA, WAVE3_CPS, WAVE3_SLOPE, t, footprint_m);
    g = g + wave_octave(p_m, n, WAVE4_DIR, WAVE4_LAMBDA, WAVE4_CPS, WAVE4_SLOPE, t, footprint_m);
    g = g + wave_octave(p_m, n, WAVE5_DIR, WAVE5_LAMBDA, WAVE5_CPS, WAVE5_SLOPE, t, footprint_m);
    g = g + wave_octave(p_m, n, WAVE6_DIR, WAVE6_LAMBDA, WAVE6_CPS, WAVE6_SLOPE, t, footprint_m);
    return g;
}

// Master water-shading blend: the fade of the LONGEST wave octave. 0 from
// orbit (old path bit-identical), 1 once 2 km swells span DETAIL_FADE_HI
// pixels (~200 km altitude at 1440p), smooth in between.
fn wave_presence(footprint_m: f32) -> f32 {
    return detail_octave_fade(WAVE1_LAMBDA, footprint_m);
}

// Triplanar value noise on the sphere for the LAND detail octaves -- same
// pow-4-weight construction as the cloud field's sphere noise but its own
// seed offsets, so this stays independent of the cloud functions (which
// have their own rework cadence). freq = planet radius / wavelength.
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
    return clamp(1.0 + f, 0.7, 1.3);
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
fn water_shade(
    albedo: vec3<f32>,
    n_geo: vec3<f32>,
    n_pert: vec3<f32>,
    view_dir: vec3<f32>,
) -> vec3<f32> {
    let sun_l = normalize(camera.sun_direction.xyz);
    let sun_i = camera.sun_direction.w;
    let day = clamp(dot(n_geo, sun_l), 0.0, 1.0);
    let cos_v = clamp(dot(n_pert, view_dir), 0.0, 1.0);
    let t1 = 1.0 - cos_v;
    let t2 = t1 * t1;
    let f = WATER_F0 + (1.0 - WATER_F0) * t2 * t2 * t1;
    let refl = reflect(-view_dir, n_pert);
    let elev = clamp(dot(refl, n_geo), 0.0, 1.0);
    let horizon = vec3<f32>(0.62, 0.7, 0.8);
    let zenith = vec3<f32>(0.1, 0.26, 0.55);
    var sky = mix(horizon, zenith, pow(elev, 0.6));
    sky = sky + camera.sun_color.rgb * pow(max(dot(refl, sun_l), 0.0), 8.0) * 0.35;
    let sky_term = sky * (day * sun_i * WATER_SKY_GAIN);
    let body = albedo * camera.sun_color.rgb * (sun_i * day / PI);
    let h = normalize(view_dir + sun_l);
    let sparkle = pow(max(dot(n_pert, h), 0.0), WATER_SPEC_POWER) * WATER_SPEC_GAIN;
    let anchor = pow(max(dot(n_geo, h), 0.0), 220.0) * 0.15;
    let spec = camera.sun_color.rgb * sun_i * (sparkle + anchor) * day;
    return body * (1.0 - f) + sky_term * f + spec + albedo * 0.005;
}

// ── Analytic atmosphere scattering (material type 14, v0.807) ──
//
// Single-scattering approximation evaluated per fragment on the oversized
// atmosphere shell sphere (O'Neil-class: a short numeric march along the
// view ray with an ANALYTIC Chapman-function optical depth toward the sun,
// so there is no nested sampling loop and no precomputed LUT). All positions
// are normalized to SHELL RADII (shell boundary = 1.0) before any math: at
// planetary magnitudes (1e7..1e11 m) the raw world-space ray-sphere terms
// would shred f32 precision, while in shell units everything stays O(1e3).
//
// Look targets (verify by flying at Earth):
//  (a) from space: a thin bright blue limb hugging the horizon;
//  (b) the day side brightens toward the sun and the terminator fades warm
//      (Mie forward lobe + Rayleigh-reddened sun transmittance);
//  (c) the night-side atmosphere is nearly invisible (sun transmittance
//      kills in-scatter; the remaining alpha only darkens, never glows);
//  (d) from INSIDE the atmosphere: deep blue zenith, pale bright horizon.
//      The same math handles it -- the ray segment start is clamped to the
//      camera position whenever the camera is within the shell.
//
// Material packing (producer: lib.rs planet_atmo_materials; Rust mirror +
// unit tests: src/renderer/atmosphere.rs -- keep the constants in sync):
//   base_color.rgb  relative per-channel scattering strengths (LINEAR, the
//                   planet RON's atmosphere_color.rgb verbatim). The mapping
//                   is: per-channel vertical optical depth = rgb * alpha *
//                   ATMO_TAU_RAYLEIGH, and beta = depth / scale height. So a
//                   blue-dominant color scatters blue hardest = blue sky +
//                   warm sunsets (Earth), while a red-dominant color gives a
//                   butterscotch sky (Mars). Any modded planet just works.
//   base_color.a    overall density multiplier (atmosphere_color alpha)
//   params.x        planet radius / shell radius
//   params.y        density scale height / shell radius
//   params.z        14.0 (this shader type)

const ATMO_SAMPLES: i32 = 12;
// Vertical optical depth contributed by a 1.0-strength color channel at
// density (alpha) 1.0. Earth's real blue-channel Rayleigh depth is ~0.28;
// earth.ron ships color.b = 1.0, alpha = 0.5, so 1.0 * 0.5 * 0.6 = 0.30.
const ATMO_TAU_RAYLEIGH: f32 = 0.6;
// Mie (aerosol haze) vertical depth at density 1.0: small, gray, strongly
// forward-scattering; supplies the warm glow around the sun near the limb.
const ATMO_TAU_MIE: f32 = 0.02;
const ATMO_MIE_G: f32 = 0.76;
// Radiance-to-display multiplier: THE artistic brightness knob. Raising it
// brightens limb + sky; the surface stays readable regardless because this
// path only ever alpha-blends (never additive white-out).
const ATMO_EXPOSURE: f32 = 4.0;
// Close-range tune (v0.815): ATMO_EXPOSURE was calibrated against BLACK SPACE
// -- the from-orbit limb and far-disc tint, which the operator approved. But
// the same 4x in-scatter boost applied to rays that TERMINATE ON THE LIT
// SURFACE floods the view once the planet fills the screen (verified capture
// at 400 km: the whole disc washed pale). The in-scatter is boosted 4x while
// the surface behind it is not, so haze contrast is exaggerated 4x exactly
// where the eye wants ground detail. Fix: per fragment, blend the exposure
// between a calm surface value and the full limb value using two weights,
// taking the MAX of:
//  (a) limb weight -- rays that miss the planet (or graze within half a
//      shell thickness of the limb) keep the FULL exposure, so the blue limb
//      glow and the ground-level sky/horizon gradient never change;
//  (b) distance weight -- cameras beyond ATMO_FAR_R shell radii keep the
//      full exposure on the WHOLE disc, so the approved 12,000 km blue-marble
//      look is bit-identical; the disc clears smoothly on approach between
//      FAR_R and NEAR_R (reads as detail resolving, no popping).
// Mirror + unit tests: renderer::atmosphere::atmo_exposure.
const ATMO_EXPOSURE_NEAR: f32 = 1.4;
const ATMO_NEAR_R: f32 = 1.25;
const ATMO_FAR_R: f32 = 2.5;

// Scaled complementary error function erfcx(z) = exp(z^2) * erfc(z) for
// z >= 0, the kernel of the Chapman function below. Two branches, both
// sub-percent (verified in renderer::atmosphere against brute force):
//  - z <= 2.5: Abramowitz-Stegun 7.1.26. Its erfc polynomial carries an
//    exp(-z^2) factor that cancels our exp(z^2) EXACTLY, leaving a pure
//    polynomial. (Its ABSOLUTE erfc error of 1.5e-7 becomes a huge RELATIVE
//    error once multiplied by exp(z^2), which is why large z must switch.)
//  - z > 2.5: the 3-term asymptotic series 1/(sqrt(pi) z) (1 - 1/(2z^2)
//    + 3/(4z^4)), which is where erfc's absolute smallness lives.
fn atmo_erfcx(z: f32) -> f32 {
    if (z <= 2.5) {
        let t = 1.0 / (1.0 + 0.3275911 * z);
        return t
            * (0.254829592
                + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    }
    let inv_z2 = 1.0 / (z * z);
    return 0.5641896 / z * (1.0 + inv_z2 * (-0.5 + 0.75 * inv_z2));
}

// Closed-form Chapman function: relative slant-path air mass at radius x
// (in SCALE HEIGHTS) for zenith cosine mu >= 0, via the large-x asymptotic
// Ch(x, mu) = sqrt(pi*x/2) * erfcx(mu * sqrt(x/2)). ~1 at the zenith,
// sqrt(pi*x/2) at the horizon; ~0.1% error for planetary x (hundreds+),
// tested in Rust against brute-force integration (renderer::atmosphere).
// A simpler rational interpolation was tried first and missed by ~10% at
// mid angles -- a visibly wrong mid-sky -- hence the erfcx machinery.
fn atmo_chapman(x: f32, mu: f32) -> f32 {
    return sqrt(1.5707964 * x) * atmo_erfcx(mu * sqrt(0.5 * x));
}

// Density-integrated path length (units: shell radii at surface density)
// from radius r along zenith cosine mu out to space, for an exponential
// atmosphere over planet radius rp with scale height h. Rays dipping below
// the planet surface return a huge depth (sun geometrically occluded); the
// terminator still fades smoothly because the near-grazing depths are
// already enormous before the hard cutoff engages. Accuracy vs brute-force
// numeric integration: a few percent (unit-tested in renderer::atmosphere).
fn atmo_od_to_space(r: f32, mu: f32, rp: f32, h: f32) -> f32 {
    let x = r / h;
    let alt = max(r - rp, 0.0) / h;
    if (mu >= 0.0) {
        return h * exp(-alt) * atmo_chapman(x, mu);
    }
    // Downward ray: mirror the path at the tangent point (lowest radius on
    // the ray) -- down-leg = 2x the horizontal integral there minus the
    // up-leg we did not traverse.
    let sin_chi = sqrt(max(1.0 - mu * mu, 0.0));
    let rt = r * sin_chi;
    if (rt < rp) {
        return 1.0e9;
    }
    let alt_t = (rt - rp) / h;
    let horiz_t = h * exp(-alt_t) * atmo_chapman(rt / h, 0.0);
    return max(2.0 * horiz_t - h * exp(-alt) * atmo_chapman(x, -mu), 0.0);
}

// Rayleigh phase 3/(16pi)(1 + cos^2 theta); integrates to 1 over the sphere.
fn atmo_rayleigh_phase(c: f32) -> f32 {
    return 0.0596831 * (1.0 + c * c);
}

// Henyey-Greenstein phase for the Mie forward lobe; integrates to 1.
fn atmo_mie_phase(c: f32) -> f32 {
    let g = ATMO_MIE_G;
    let denom = 1.0 + g * g - 2.0 * g * c;
    return (1.0 - g * g) / (12.566371 * denom * sqrt(denom));
}

fn atmosphere_scattering(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    // Shell center + radius recovered from the object transform: the shell
    // mesh is a UNIT icosphere placed via Vec3::splat(scale), so column 0's
    // length IS the shell radius and column 3 is the planet center. Nothing
    // extra to plumb through the material uniforms.
    let center = object.model[3].xyz;
    let shell_r = length(object.model[0].xyz);
    let rp = clamp(material.params.x, 0.01, 0.9999); // planet radius (shell units)
    let h = max(material.params.y, 1.0e-6);          // scale height (shell units)

    // Camera + ray in shell units, planet center at the origin.
    let ro = (camera.view_pos.xyz - center) / shell_r;
    let rd = normalize(world_position - camera.view_pos.xyz);
    let cam_inside = dot(ro, ro) < 1.0;

    // The transparent pipeline draws BOTH faces of the shell (cull_mode:
    // None, shared with glass). A camera outside the shell would therefore
    // blend the same ray twice (front face + back face). Keep exactly one
    // layer: front faces when outside, back faces when inside (from inside a
    // sphere only back faces are visible, so this is also what makes the
    // sky appear at low altitude instead of vanishing on shell entry).
    if (front_facing == cam_inside) {
        discard;
    }

    // Ray vs shell sphere (radius 1) via the geometric formulation: the
    // naive b^2 - c quadratic catastrophically cancels when the camera is
    // thousands of radii out; the explicit perpendicular foot does not.
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= 1.0) {
        return vec4<f32>(0.0); // grazing numeric miss: fully transparent
    }
    let thc = sqrt(1.0 - d2);
    var t0 = tca - thc;
    var t1 = tca + thc;
    if (t1 <= 0.0) {
        return vec4<f32>(0.0); // shell entirely behind the camera
    }
    t0 = max(t0, 0.0); // camera inside the shell: integrate from the eye

    // Clip the segment at the planet surface: air BEHIND the planet
    // contributes nothing to this pixel (the opaque surface occludes it).
    if (d2 < rp * rp && tca > 0.0) {
        let t_planet = tca - sqrt(rp * rp - d2);
        if (t_planet > t0) {
            t1 = min(t1, t_planet);
        }
    }
    if (t1 <= t0) {
        return vec4<f32>(0.0);
    }

    // Scattering coefficients per shell radius. The vertical optical depth
    // of an exponential profile is beta * H, so beta = target_depth / H --
    // this keeps the LOOK invariant across planet sizes AND across the
    // far-body disc-size floor (which inflates the drawn radius).
    let density_mul = material.base_color.a;
    let beta_ray = material.base_color.rgb * (density_mul * ATMO_TAU_RAYLEIGH / h);
    let beta_mie = density_mul * ATMO_TAU_MIE / h;
    // Extinction carries a touch of Mie absorption (the classic /0.9).
    let beta_ext = beta_ray + vec3<f32>(beta_mie * 1.11);

    let sun = normalize(camera.sun_direction.xyz);

    // Midpoint march along the view segment. od_view accumulates the density
    // integral camera->sample numerically (needed for in-scatter anyway);
    // the per-sample sun leg is ANALYTIC -- that is the O'Neil-class trick
    // that removes the nested loop.
    let dt = (t1 - t0) / f32(ATMO_SAMPLES);
    var od_view = 0.0;
    var inscatter = vec3<f32>(0.0);
    for (var i = 0; i < ATMO_SAMPLES; i = i + 1) {
        let t = t0 + (f32(i) + 0.5) * dt;
        let p = ro + rd * t;
        let r = length(p);
        let dens = exp(-max(r - rp, 0.0) / h);
        // Half-sample lag: transmittance to the CENTER of this slice.
        let od_here = od_view + dens * dt * 0.5;
        od_view = od_view + dens * dt;
        let mu_s = dot(p, sun) / max(r, 1.0e-6);
        let od_sun = atmo_od_to_space(r, mu_s, rp, h);
        let tau = beta_ext * (od_here + od_sun);
        inscatter = inscatter + dens * exp(-tau) * dt;
    }

    // Phase evaluation: cos of the angle between view ray and sun direction;
    // +1 = looking straight at the sun (forward scattering).
    let cos_theta = dot(rd, sun);
    // Exposure blend (see the ATMO_EXPOSURE_NEAR comment): surface-terminated
    // rays from a close camera get the calm exposure; limb rays and far
    // cameras keep the full one. A ray hits the planet iff it runs forward
    // (tca > 0) with impact parameter below rp -- for a camera above the
    // surface, b rises through rp BEFORE tca changes sign as the ray tilts
    // from down to up, so the tca gate never introduces a visible seam and
    // the ground-level SKY (upward rays: tca <= 0 or b >= rp) keeps today's
    // approved full-exposure look untouched.
    let b_impact = sqrt(d2);
    var w_limb = 1.0;
    if (tca > 0.0 && b_impact < rp) {
        w_limb = smoothstep(rp - (1.0 - rp) * 0.5, rp, b_impact);
    }
    let w_far = smoothstep(ATMO_NEAR_R, ATMO_FAR_R, length(ro));
    let exposure = mix(ATMO_EXPOSURE_NEAR, ATMO_EXPOSURE, max(w_limb, w_far));
    let sun_radiance = camera.sun_color.rgb * camera.sun_direction.w * exposure;
    let radiance = sun_radiance
        * (beta_ray * atmo_rayleigh_phase(cos_theta)
            + vec3<f32>(beta_mie) * atmo_mie_phase(cos_theta))
        * inscatter;

    // Per-channel transmittance of whatever sits behind this pixel,
    // collapsed to the single gray alpha fixed-function blending can
    // express. The surface stays readable at every angle because this path
    // only ever alpha-blends over it.
    let trans = exp(-beta_ext * od_view);
    let alpha = clamp(1.0 - (trans.r + trans.g + trans.b) / 3.0, 0.0, 1.0);

    // Tone-map the in-scattered light with the SAME ACES curve as the rest
    // of the pipeline; all math above is linear. The render target is an
    // sRGB view, so writing linear values is the honest handoff -- the
    // hardware applies the sRGB transfer on store, and blending against an
    // sRGB target happens in LINEAR space per the WebGPU spec (the
    // v0.802/v0.803 glow-layer lesson: know the target's gamma, encode once,
    // never twice).
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance * (aces_a * radiance + vec3<f32>(aces_b)))
            / (radiance * (aces_c * radiance + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // ALPHA_BLENDING computes src.rgb * src.a + dst * (1 - src.a); divide
    // the radiance back out of the alpha so exactly `mapped` lands on
    // screen. Both terms go to zero together for thin air, so the ratio
    // stays finite; the clamp guards the pathological alpha -> 0 corner.
    let rgb = clamp(mapped / max(alpha, 1.0e-3), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, alpha);
}

// ── Procedural cloud layer (material type 15, clouds increment 2) ──
//
// An animated cloud DECK on a SECOND translucent shell just above the planet
// surface and BELOW the scattering atmosphere shell. lib.rs pushes the cloud
// shell into the transparent celestial list BEFORE the atmosphere shell, and
// that list draws in order with no depth writes, so the air blends OVER the
// clouds -- physically correct: the atmosphere scatters in front of the deck.
//
// Increment 2 (v0.815): the deck is now RAYMARCHED through a thin spherical
// slab (CLOUD_BASE_SCALE..CLOUD_TOP_SCALE planet radii; the drawn shell at
// CLOUD_SHELL_SCALE sits mid-slab and only supplies the fragments/rays).
// Exactly the reuse contract designed into increment 1:
//   density(p_local) = cloud_alpha_from_field(
//       cloud_field(normalize(p_local), t, seed), coverage)
//       * cloud_altitude_envelope(length(p_local))
// cloud_field/cloud_alpha_from_field are UNCHANGED; the altitude envelope and
// the march loop are the only new math. Front-to-back alpha accumulation with
// an early-out at opacity saturation, per-sample macro N-dot-L lighting, a
// one-tap sun-direction density gradient for volumetric self-shadow, and a
// base-to-top height gradient (bases darker, tops brighter). The silver
// lining and the ACES tail keep increment 1's posture. The increment-1
// single-sample path is kept verbatim as `cloud_layer_flat`, selected by
// setting CLOUD_MARCH_SAMPLES to 0 (the quality switch; this file is
// hot-reloaded from disk, so the fallback is one edit away on weak GPUs).
//
// Material packing (producer: lib.rs planet_cloud_materials; Rust mirror +
// unit tests: src/renderer/clouds.rs -- keep every CLOUD_* constant in sync,
// the wgsl_cloud_constants_stay_in_sync test enforces it by parsing this
// file):
//   base_color.rgb  cloud tint (white today; a future per-planet cloud_color
//                   field can ride here with zero shader changes)
//   base_color.a    coverage 0..1 (planet RON cloud_coverage)
//   params.x        per-planet noise seed (terrain_seed % 1024) so two
//                   cloudy worlds never show the same pattern
//   params.z        15.0 (this shader type)
//
// TIME rides in camera.sun_color.w -- that slot was a documented-unused pad,
// so animating the clouds needed no uniform layout change (the same
// no-layout-churn rule as the type-14 material packing; the v0.782
// device-limit incident is why layout churn is avoided). Written by
// render_celestial_onto each frame; app-start-relative seconds, so f32
// precision stays comfortable for days of uptime at these drift rates.

// Peak opacity of the thickest cloud core. Deliberately below 1.0 so the
// planet surface stays readable through even the densest deck. Lowered
// 0.85 -> 0.72 after the first orbital field test (2026-07-11): at 0.85 the
// decks fused into a featureless white cue ball.
const CLOUD_MAX_ALPHA: f32 = 0.72;
// Empirical contrast window of the raw octave sum over the sphere (p03/p96
// of 20,000 spiral samples, measured in renderer::clouds's tuning probe):
// the triplanar blend + octave averaging concentrate the sum tightly around
// ~0.49, so WITHOUT this stretch a mid coverage value catches only the
// distribution's thin upper tail and Earth reads nearly cloudless (caught
// by the coverage_maps_monotonically test on first tuning). smoothstep
// through this window spreads the sum to a roughly UNIFORM 0..1
// "cloudiness", which is what lets the coverage knob track actual sky
// fraction via the simple threshold below.
const CLOUD_FIELD_LO: f32 = 0.32;
const CLOUD_FIELD_HI: f32 = 0.65;
// Softness of the cloud edge: alpha ramps over this field range above the
// threshold, giving wispy borders instead of cookie-cutter blobs. Widened
// 0.18 -> 0.30 with the detail octaves (2026-07-11): the wider ramp lets
// the high-frequency octaves erode the borders into filigree instead of
// stamping hard blob outlines.
const CLOUD_EDGE: f32 = 0.30;
// Zonal anisotropy of the cloud field: the sampling direction's y (the spin
// axis) is scaled UP by this factor before the noise lookup, so the noise
// varies faster with latitude than longitude and features stretch east-west
// like real storm bands and jet-stream streaks. 1.0 = isotropic blobs.
const CLOUD_BAND_STRETCH: f32 = 1.75;
// The "weather" of increment 1: two octave SETS drift as rigid rotations at
// different speeds around different axes, so their SUM genuinely evolves
// (morphs) rather than sliding as one frozen texture. Radians per second of
// cloud-clock time; zonal ~0.0015 rad/s is a visible-but-calm crawl (a
// pattern crosses a planet disc in tens of minutes). Increment 2 can promote
// these to per-planet data.
const CLOUD_DRIFT_ZONAL: f32 = 0.0015;
const CLOUD_DRIFT_CROSS: f32 = -0.0009;
// Self-shadow lookup: great-circle step (radians) toward the sun, and how
// hard a density rise over that step darkens this fragment. SHARP amplifies
// the (already contrast-stretched) field differences into a usable shading
// range without saturating everywhere.
const CLOUD_SHADOW_STEP: f32 = 0.05;
const CLOUD_SHADOW_STRENGTH: f32 = 0.65;
const CLOUD_SHADOW_SHARP: f32 = 2.5;
// Silver lining: forward-scatter glow at THIN cloud edges when looking
// toward the sun (Henyey-Greenstein lobe, reusing the atmosphere's phase
// function -- no third scattering model).
const CLOUD_SILVER_GAIN: f32 = 0.3;
// Ambient skylight floor on the day side (shadowed cloud flanks stay
// visibly white, not gray mush) and the night-side floor (near-black but
// not absolute zero, matching the surface shader's ambient posture).
const CLOUD_AMBIENT: f32 = 0.08;
const CLOUD_NIGHT_FLOOR: f32 = 0.006;
// ── Increment-2 raymarch constants (Rust mirror: renderer::clouds) ──
// The slab, in PLANET-RADIUS multiples: the drawn shell (fragments/rays only)
// sits mid-slab at CLOUD_SHELL_SCALE; density lives between BASE and TOP.
// For Earth: base ~25.5 km, drawn shell ~51 km, top ~76.5 km. Terrain peaks
// (up to ~1.0041 R) may poke ~100 m into the base -- mountains in cloud,
// physically charming and harmless (the envelope is ~0 there).
const CLOUD_SHELL_SCALE: f32 = 1.008;
const CLOUD_BASE_SCALE: f32 = 1.004;
const CLOUD_TOP_SCALE: f32 = 1.012;
// March quality switch: samples along the view segment through the slab.
// 8-12 is the designed band; 0 selects the increment-1 flat deck
// (cloud_layer_flat) for weak GPUs. Measured on the RTX 4070 at 2560x1373
// (2026-07-11, same-regime march-vs-flat pairs): +1.5 ms at the 12,000 km
// orbit view, +0.2 ms at 400 km with the deck filling the frame -- the
// clear-sky probe gate and the saturation early-out keep the worst case
// cheap, and the ~90 FPS orbit baseline holds with the march on.
const CLOUD_MARCH_SAMPLES: i32 = 10;
// Extinction per drawn-shell unit at density 1. Calibrated so a full-density
// radial pass through the slab (envelope integral ~0.6 * thickness) reaches
// ~93% opacity -- matching increment 1's thick-core look after the
// CLOUD_MAX_ALPHA cap: 1 - exp(-560 * 0.6 * 0.00794) ~ 0.93.
const CLOUD_SIGMA_T: f32 = 560.0;
// Self-shadow tap for the march: a 3D offset TOWARD the sun (drawn-shell
// units, ~half the slab thickness) replaces increment 1's great-circle step;
// density rising toward the sun = this sample sits in a cloud mass's shadow.
// SHARP converts the (envelope-scaled, so smaller) density difference into a
// usable shading range.
const CLOUD_MARCH_SHADOW_STEP: f32 = 0.004;
const CLOUD_MARCH_SHADOW_SHARP: f32 = 4.0;
// Height gradient: cloud BASES receive less sky/sun light than tops. The
// classic volumetric cue -- flat bottoms read dark, sunlit tops read bright.
const CLOUD_BASE_DARKEN: f32 = 0.75;

// Rigid rotation around the local Y axis (the planet's spin axis in the
// icosphere's local frame): zonal drift, like real weather bands.
fn cloud_rot_y(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * v.x + s * v.z, v.y, c * v.z - s * v.x);
}

// Rigid rotation around the local X axis: the cross-drift for octave set B,
// deliberately a DIFFERENT axis so the two sets shear against each other.
fn cloud_rot_x(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(v.x, c * v.y - s * v.z, c * v.z + s * v.y);
}

// Seamless noise on the sphere: TRIPLANAR blend of the existing 2D value
// noise (reusing hash21/value_noise above -- not a third noise
// implementation). For a UNIT direction the squared components already sum
// to 1, so dir*dir are the blend weights for free. Each plane gets a
// different seed offset so the three projections never mirror each other at
// the +/- axis crossings.
fn cloud_noise(dir: vec3<f32>, freq: f32, seed: f32) -> f32 {
    // Blend weights sharpened to the 4th power (2026-07-11 field report):
    // with plain dir*dir weights, the wide 3-way blend zones near the
    // diagonal great circles average two disagreeing projections into
    // visible straight creases once the contrast stretch amplifies them
    // ("hard lines" through the deck). Pow-4 narrows the blend band so one
    // projection dominates almost everywhere.
    var w = dir * dir;
    w = w * w;
    let wn = w / (w.x + w.y + w.z);
    let p = dir * freq;
    let o = vec2<f32>(seed, seed * 0.617);
    let nx = value_noise(p.yz + o);
    let ny = value_noise(p.zx + o * 1.3);
    let nz = value_noise(p.xy + o * 1.7);
    return nx * wn.x + ny * wn.y + nz * wn.z;
}

// The cloud density field: 4 octaves in two independently drifting sets.
// Set A (3 octaves, zonal drift) carries the main cloud masses; set B (one
// mid-frequency octave on a different axis and speed) makes the sum evolve
// over time instead of rotating rigidly. Pure function of (planet-fixed
// direction, time, seed) -- exactly the sampling contract the increment-2
// raymarcher needs. The raw amplitude-normalized sum is contrast-stretched
// through its empirical window (see CLOUD_FIELD_LO/HI) so the output is a
// roughly uniform 0..1 "cloudiness".
fn cloud_field(dir: vec3<f32>, t: f32, seed: f32) -> f32 {
    let da0 = cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL);
    // Band stretch (see CLOUD_BAND_STRETCH): re-normalized so the triplanar
    // weights in cloud_noise still see a unit direction.
    let da = normalize(vec3<f32>(da0.x, da0.y * CLOUD_BAND_STRETCH, da0.z));
    let db = cloud_rot_x(dir, t * CLOUD_DRIFT_CROSS);
    var f = 0.5 * cloud_noise(da, 5.0, seed);
    f = f + 0.25 * cloud_noise(da, 11.0, seed + 19.0);
    f = f + 0.125 * cloud_noise(da, 23.0, seed + 47.0);
    f = f + 0.0625 * cloud_noise(da, 47.0, seed + 83.0);
    f = f + 0.35 * cloud_noise(db, 7.0, seed + 101.0);
    return smoothstep(CLOUD_FIELD_LO, CLOUD_FIELD_HI, f / 1.2875);
}

// Coverage (0..1, from the planet RON) -> cloud body opacity. Because
// cloud_field is ~uniform after its contrast stretch, the fraction of sky
// above a threshold thr is ~(1 - thr), so thr = 1 - coverage makes the knob
// track real sky fraction; the -CLOUD_EDGE endpoint lets coverage 1.0 reach
// FULL opacity everywhere (thr + edge <= 0) instead of leaving thin holes.
// smoothstep softens the edge. Monotonic in both arguments (unit-tested in
// renderer::clouds).
fn cloud_alpha_from_field(field: f32, coverage: f32) -> f32 {
    let thr = mix(1.0, -CLOUD_EDGE, clamp(coverage, 0.0, 1.0));
    return smoothstep(thr, thr + CLOUD_EDGE, field);
}

// Altitude envelope (increment 2): shapes density across the slab. r is in
// DRAWN-SHELL units (drawn shell = 1.0, so the slab spans BASE/SHELL ..
// TOP/SHELL). Smooth rise from the base, a full-density plateau through the
// middle (the drawn-shell radius u = 0.5 sits inside it, so the increment-1
// fragment altitude evaluates at envelope 1), fade to zero at the top.
// Pure in r; mirrored + unit-tested in renderer::clouds.
fn cloud_altitude_envelope(r: f32) -> f32 {
    let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
    let top = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
    let u = clamp((r - base) / (top - base), 0.0, 1.0);
    return smoothstep(0.0, 0.4, u) * (1.0 - smoothstep(0.6, 1.0, u));
}

// The increment-2 sampling contract from the increment-1 design note --
// horizontal coverage field times the altitude envelope -- with one response
// shaping: the horizontal alpha is SQUARED. Beer-Lambert accumulation is
// concave (1 - exp(-tau) inflates mid densities toward opaque), so feeding
// it the raw ~uniform alpha fused the whole deck into a pale shroud on the
// first orbital capture (2026-07-11) -- the same cue-ball failure increment
// 1 hit and solved with its core-vs-skirt density ramp. Squaring restores
// that response through the march: 1 - exp(-2.67 a^2) tracks increment 1's
// a * (0.4 + 0.6 a) skirt curve within a few percent across the range,
// keeping skirts translucent while cores still saturate. p is a point in
// the mesh's LOCAL frame (planet-fixed, drawn shell = radius 1).
fn cloud_density(p: vec3<f32>, t: f32, seed: f32, coverage: f32) -> f32 {
    let r = length(p);
    let env = cloud_altitude_envelope(r);
    if (env <= 0.0) {
        return 0.0;
    }
    let a_h = cloud_alpha_from_field(cloud_field(normalize(p), t, seed), coverage);
    return a_h * a_h * env;
}

// Quality dispatcher: the march is the default; CLOUD_MARCH_SAMPLES = 0
// falls back to the increment-1 painted deck (both paths stay compiled and
// naga-validated, so the switch can never rot).
fn cloud_layer(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    if (CLOUD_MARCH_SAMPLES <= 0) {
        return cloud_layer_flat(world_position, front_facing);
    }
    return cloud_layer_march(world_position, front_facing);
}

// Increment-1 fallback: one field sample at the fragment, painted-on deck.
fn cloud_layer_flat(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    // Shell center + radius recovered from the object transform, same trick
    // as the atmosphere shell: unit icosphere placed via Vec3::splat(scale),
    // so column 0's length IS the shell radius and column 3 the center.
    let center = object.model[3].xyz;
    let shell_r = length(object.model[0].xyz);

    // Exactly ONE shell layer (same rule as the atmosphere): the transparent
    // pipeline draws both faces (cull off, shared with glass). Keep front
    // faces when the camera is outside the shell, back faces when inside --
    // the inside view is the increment-2 under-the-deck flight case, which
    // this rule already handles correctly.
    let ro = (camera.view_pos.xyz - center) / shell_r;
    let cam_inside = dot(ro, ro) < 1.0;
    if (front_facing == cam_inside) {
        discard;
    }

    // PLANET-FIXED sample direction: rotate the world direction back into
    // the mesh's local frame so the pattern rides the planet's spin and the
    // drift constants are true weather motion relative to the ground.
    // transpose(normal_matrix) IS model.inverse() exactly (normal_matrix is
    // inverse-transpose), so no matrix inversion is needed in the shader.
    let inv_model = transpose(object.normal_matrix);
    let dir = normalize((inv_model * vec4<f32>(world_position, 1.0)).xyz);

    let t = camera.sun_color.w; // the cloud clock (see header comment)
    let seed = material.params.x;
    let coverage = material.base_color.a;

    let field = cloud_field(dir, t, seed);
    let body = cloud_alpha_from_field(field, coverage);
    if (body <= 0.002) {
        // Clear sky at this fragment: fully transparent, skip the lighting.
        return vec4<f32>(0.0);
    }

    // Macro lighting from the SPHERE normal: the deck is a thin wrap, so the
    // planet's own day/night curvature dominates. Computed from geometry
    // (position - center), not the interpolated mesh normal, so the level-3
    // icosphere facets never show in the shading.
    let n = normalize(world_position - center);
    let sun = normalize(camera.sun_direction.xyz);
    let ndl = dot(n, sun);
    // Soft terminator; the night side fades to near-black (clouds are lit by
    // the sun alone -- moonlight/city glow are future increments).
    let day = smoothstep(-0.05, 0.3, ndl);

    // Cheap self-shadow: re-sample the field a short great-circle step
    // TOWARD the sun (sun projected into the tangent plane at dir; the
    // projection goes to zero when the sun is overhead, so the step -- and
    // the shadow -- smoothly vanish there, with no normalize-of-zero NaN).
    // If density RISES toward the sun, this fragment sits on the shaded
    // flank of a cloud mass -> darken. Fake but effective from orbit: flat
    // coverage blobs pick up an internal sun-facing gradient and read puffy.
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);
    let tang = sun_local - dir * dot(sun_local, dir);
    let sdir = normalize(dir + tang * CLOUD_SHADOW_STEP);
    let field_sun = cloud_field(sdir, t, seed);
    let shade = 1.0
        - CLOUD_SHADOW_STRENGTH
            * clamp((field_sun - field) * CLOUD_SHADOW_SHARP, 0.0, 1.0);

    // Silver lining: HG forward lobe (the atmosphere's phase function,
    // reused) at THIN edges -- thick cores block the forward-scattered sun,
    // so weight by (1 - body). Gated by a twilight-wide day window so the
    // deep night limb never glows.
    let rd = normalize(world_position - camera.view_pos.xyz);
    let cos_vs = dot(rd, sun);
    let silver = CLOUD_SILVER_GAIN * atmo_mie_phase(cos_vs) * (1.0 - body)
        * smoothstep(-0.15, 0.1, ndl);

    // Sun energy matches the celestial pass's directional light so the deck
    // sits in the same exposure regime as the surface below it.
    let sun_energy = camera.sun_color.rgb * camera.sun_direction.w;
    let lit = clamp(ndl, 0.0, 1.0);
    var radiance = material.base_color.rgb
        * (sun_energy * (CLOUD_AMBIENT + lit * shade) * day
            + vec3<f32>(CLOUD_NIGHT_FLOOR));
    radiance = radiance + sun_energy * silver;

    // Same ACES curve as the rest of the pipeline: all math above is linear,
    // the render target view is sRGB, blending happens in linear space per
    // the WebGPU spec (the v0.802/v0.803 lesson: encode once, never twice).
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance * (aces_a * radiance + vec3<f32>(aces_b)))
            / (radiance * (aces_c * radiance + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Density ramp (2026-07-11 orbital field test): `body` saturates within
    // CLOUD_EDGE of the threshold, which painted every deck the same solid
    // white ("cue ball"). Re-shape by the field's headroom above the
    // threshold so cloud SKIRTS stay translucent and only the dense cores
    // approach max alpha -- the surface reads through most of the deck.
    let thr = mix(1.0, -CLOUD_EDGE, clamp(coverage, 0.0, 1.0));
    let t_core = clamp((field - thr) / max(1.0 - thr, CLOUD_EDGE), 0.0, 1.0);
    let density = 0.40 + 0.60 * t_core * t_core * (3.0 - 2.0 * t_core);
    // Limb fade: near the disc edge the shell is seen almost edge-on and
    // stacks over the atmosphere's own limb brightening into a hard white
    // ring; ease the deck off as the view grazes the sphere.
    let mu = clamp(abs(dot(rd, n)), 0.0, 1.0);
    let limb = mix(0.55, 1.0, smoothstep(0.0, 0.35, mu));
    return vec4<f32>(mapped, body * density * limb * CLOUD_MAX_ALPHA);
}

// Increment-2 raymarch: real thickness, parallax, and volumetric
// self-shadow. Everything happens in the mesh's LOCAL frame (planet-fixed,
// drawn shell = radius 1): the model transform is rotation + uniform scale,
// so directions transfer with one normalize and dot products are preserved.
fn cloud_layer_march(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    let center = object.model[3].xyz;
    let shell_r = length(object.model[0].xyz);

    // Exactly ONE shell layer, same rule as the flat path and the
    // atmosphere: front faces when the camera is outside the drawn shell,
    // back faces when inside (the under-the-deck flight case).
    let ro_w = (camera.view_pos.xyz - center) / shell_r;
    let cam_inside = dot(ro_w, ro_w) < 1.0;
    if (front_facing == cam_inside) {
        discard;
    }

    // transpose(normal_matrix) IS model.inverse() exactly (see the flat
    // path); it maps world points into the unit-icosphere local frame.
    let inv_model = transpose(object.normal_matrix);
    let ro = (inv_model * vec4<f32>(camera.view_pos.xyz, 1.0)).xyz;
    let rd_w = normalize(world_position - camera.view_pos.xyz);
    let rd = normalize((inv_model * vec4<f32>(rd_w, 0.0)).xyz);
    let dirf = normalize((inv_model * vec4<f32>(world_position, 1.0)).xyz);

    let t = camera.sun_color.w; // the cloud clock (see header comment)
    let seed = material.params.x;
    let coverage = material.base_color.a;

    // Slab interval along the ray: inside the TOP sphere, outside the BASE
    // sphere, in front of the camera. Only the FIRST such interval is
    // marched: a ray that dives below the base either hits the planet (the
    // far-side re-entry is depth-occluded) or grazes the limb where the near
    // interval alone already saturates opacity.
    let rb = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
    let rt = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= rt * rt) {
        return vec4<f32>(0.0); // grazing numeric miss of the whole slab
    }
    let thc_t = sqrt(rt * rt - d2);
    var m0 = max(tca - thc_t, 0.0);
    var m1 = tca + thc_t;
    if (m1 <= 0.0) {
        return vec4<f32>(0.0); // slab entirely behind the camera
    }
    if (d2 < rb * rb) {
        let thc_b = sqrt(rb * rb - d2);
        let b0 = tca - thc_b;
        let b1 = tca + thc_b;
        if (b0 > m0) {
            m1 = min(m1, b0); // clipped where the ray dives below the base
        } else if (b1 > m0) {
            m0 = b1; // started under the deck: begin at the base exit above
        }
    }
    if (m1 <= m0) {
        return vec4<f32>(0.0);
    }

    // Clear-sky gate: probe the horizontal field at the segment's start,
    // middle, and end before paying for the full march. Most pixels over a
    // partly-cloudy planet are clear; 3 field evaluations instead of ~20
    // keeps them cheap. (A cloud strictly between probes on a long grazing
    // segment can slip through -- only skirt-thin alpha is at stake.)
    let seg = m1 - m0;
    let probe = max(
        max(
            cloud_alpha_from_field(
                cloud_field(normalize(ro + rd * m0), t, seed), coverage),
            cloud_alpha_from_field(
                cloud_field(normalize(ro + rd * (m0 + seg * 0.5)), t, seed), coverage),
        ),
        cloud_alpha_from_field(
            cloud_field(normalize(ro + rd * m1), t, seed), coverage),
    );
    if (probe <= 0.002) {
        return vec4<f32>(0.0);
    }

    let sun = normalize(camera.sun_direction.xyz);
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);
    let sun_energy = camera.sun_color.rgb * camera.sun_direction.w;

    // Stratified jitter from the planet-fixed fragment direction: one sample
    // offset shared by the whole ray de-bands the thin slab at grazing
    // angles without screen-space shimmer (the pattern rides the planet).
    let jitter = hash21(dirf.xy * 4096.0 + vec2<f32>(dirf.z * 1024.0, 17.0));

    // Front-to-back accumulation with early-out at opacity saturation.
    let dtm = seg / f32(CLOUD_MARCH_SAMPLES);
    var trans = 1.0;
    var acc = vec3<f32>(0.0);
    var acc_w = 0.0;
    for (var i = 0; i < CLOUD_MARCH_SAMPLES; i = i + 1) {
        let tm = m0 + (f32(i) + jitter) * dtm;
        let p = ro + rd * tm;
        let dens = cloud_density(p, t, seed, coverage);
        if (dens <= 0.0005) {
            continue; // empty sample: skip the lighting taps
        }
        let a_i = 1.0 - exp(-CLOUD_SIGMA_T * dens * dtm);
        // Macro lighting from the sample's own sphere normal (local frame
        // preserves dots), soft terminator as in increment 1.
        let n_i = normalize(p);
        let ndl = dot(n_i, sun_local);
        let day = smoothstep(-0.05, 0.3, ndl);
        let lit = clamp(ndl, 0.0, 1.0);
        // One-tap self-shadow: density gradient toward the sun in 3D.
        let d_sun = cloud_density(
            p + sun_local * CLOUD_MARCH_SHADOW_STEP, t, seed, coverage);
        let shade = 1.0
            - CLOUD_SHADOW_STRENGTH
                * clamp((d_sun - dens) * CLOUD_MARCH_SHADOW_SHARP, 0.0, 1.0);
        // Height gradient: bases darker, tops brighter.
        let u_h = clamp((length(p) - rb) / (rt - rb), 0.0, 1.0);
        let grad = mix(CLOUD_BASE_DARKEN, 1.0, u_h);
        let c_i = material.base_color.rgb
            * (sun_energy * (CLOUD_AMBIENT + lit * shade * grad) * day
                + vec3<f32>(CLOUD_NIGHT_FLOOR));
        acc = acc + c_i * (trans * a_i);
        acc_w = acc_w + trans * a_i;
        trans = trans * (1.0 - a_i);
        if (trans <= 0.02) {
            break; // opacity saturated: the rest of the slab is invisible
        }
    }
    let body_total = 1.0 - trans;
    if (body_total <= 0.003) {
        return vec4<f32>(0.0);
    }
    // Transmittance-weighted mean color of the marched samples.
    var radiance = acc / max(acc_w, 1.0e-4);

    // Silver lining: same HG forward lobe + thin-edge weighting + twilight
    // gate as increment 1, driven by the marched total instead of the single
    // sample.
    let n_frag = normalize(world_position - center);
    let cos_vs = dot(rd_w, sun);
    let silver = CLOUD_SILVER_GAIN * atmo_mie_phase(cos_vs) * (1.0 - body_total)
        * smoothstep(-0.15, 0.1, dot(n_frag, sun));
    radiance = radiance + sun_energy * silver;

    // Same ACES curve as the rest of the pipeline (linear in, sRGB target).
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance * (aces_a * radiance + vec3<f32>(aces_b)))
            / (radiance * (aces_c * radiance + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Limb fade, as in increment 1: the deck stacks over the atmosphere's
    // own limb brightening at grazing view angles; ease it off there.
    let mu = clamp(abs(dot(rd_w, n_frag)), 0.0, 1.0);
    let limb = mix(0.55, 1.0, smoothstep(0.0, 0.35, mu));
    return vec4<f32>(mapped, body_total * limb * CLOUD_MAX_ALPHA);
}

// ── Cook-Torrance BRDF Evaluation ──

fn evaluate_light(
    light_dir: vec3<f32>,
    light_color: vec3<f32>,
    light_intensity: f32,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    f0: vec3<f32>,
) -> vec3<f32> {
    let l = normalize(light_dir);
    let h = normalize(view_dir + l);

    let n_dot_l = max(dot(normal, l), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.001);
    let n_dot_h = max(dot(normal, h), 0.0);
    let h_dot_v = max(dot(h, view_dir), 0.0);

    // Cook-Torrance specular BRDF
    let ndf = distribution_ggx(n_dot_h, roughness);
    let geo = geometry_smith(n_dot_v, n_dot_l, roughness);
    let fresnel = fresnel_schlick(h_dot_v, f0);

    let numerator = ndf * geo * fresnel;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;

    // Energy conservation: diffuse is reduced by specular reflection
    let ks = fresnel;
    var kd = vec3<f32>(1.0) - ks;
    kd = kd * (1.0 - metallic); // Metals have no diffuse

    let diffuse = kd * albedo / PI;

    return (diffuse + specular) * light_color * light_intensity * n_dot_l;
}

// ── Fragment Shader ──

@fragment
fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);

    var albedo = material.base_color.rgb;
    var metallic = material.params.x;
    var roughness = material.params.y;
    let material_type = material.params.z;
    var proc_emissive = vec3<f32>(0.0); // extra emissive from procedural materials (e.g. lava cracks)
    var out_alpha = material.base_color.a; // types below may modulate (atmosphere fresnel)
    // Emissive strength normally rides in params.w -- but material type 12
    // REPURPOSES params.w as the "albedo texture present" flag (v0.811), so
    // the type-12 branch zeroes this to keep planets from self-glowing.
    var emissive_strength = material.params.w;

    // Types 14 + 15 short-circuit the whole PBR surface path: an atmosphere
    // is a participating MEDIUM and a cloud deck is a self-lit coverage
    // field -- neither takes its color from a BRDF. Types >= 15.5 would fall
    // through to the default panel-grid look (none exist yet).
    if (material_type >= 13.5 && material_type < 14.5) {
        return atmosphere_scattering(in.world_position, front_facing);
    }
    if (material_type >= 14.5 && material_type < 15.5) {
        return cloud_layer(in.world_position, front_facing);
    }

    // Apply procedural material based on type:
    //   0 = Panel grid (walls, floors)    4 = Glass            8 = Crystal
    //   1 = Brushed metal                 5 = Ice              9 = Rust/Corroded
    //   2 = Concrete                      6 = Water surface   10 = Moss/Growth
    //   3 = Wood                          7 = Leather         11 = Lava
    //  12 = Planet surface (per-pixel imagery when params.w > 0.5, else per-face
    //       color + water flag packed in UV; ocean sun glint either way)
    //  13 = Atmosphere shell (fresnel limb tint -- the pre-v0.807 fallback)
    //  14 = Atmosphere shell (analytic single scattering -- handled above)
    //  15 = Cloud layer (animated procedural deck -- handled above)
    if material_type < 0.5 {
        // Type 0: Default panel grid (walls, floors)
        if metallic < 0.1 && roughness > 0.3 {
            let panel = grid_pattern(in.world_position, normal);
            albedo = albedo * mix(0.65, 1.0, panel);
            roughness = mix(roughness + 0.1, roughness, panel);
        }
    } else if material_type < 1.5 {
        // Type 1: Brushed metal (metallic surfaces)
        let scratch = brushed_metal(in.world_position, normal);
        albedo = albedo * scratch;
        roughness = roughness + (1.0 - scratch) * 0.15;
    } else if material_type < 2.5 {
        // Type 2: Concrete
        albedo = concrete_pattern(in.world_position, normal) * albedo * 2.0;
        roughness = roughness + fbm(in.world_position.xz * 5.0) * 0.1;
    } else if material_type < 3.5 {
        // Type 3: Wood
        albedo = wood_pattern(in.world_position, normal);
        roughness = 0.5 + value_noise(in.world_position.xz * 10.0) * 0.2;
        metallic = 0.0;
    } else if material_type < 4.5 {
        // Type 4: Glass -- high reflectivity via Fresnel boost, subtle color shift
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 3.0);
        albedo = mix(albedo * 0.15, vec3<f32>(0.8, 0.9, 1.0), fresnel * 0.6);
        metallic = 0.1;
        roughness = 0.05 + value_noise(triplanar_uv(in.world_position, normal) * 20.0) * 0.03;
    } else if material_type < 5.5 {
        // Type 5: Ice -- blue-white tint, wrap lighting approx, crystalline noise
        let uv = triplanar_uv(in.world_position, normal);
        let crystal = voronoi(uv * 8.0);
        let wrap = dot(normal, normalize(camera.sun_direction.xyz)) * 0.5 + 0.5; // wrap lighting for SSS
        albedo = mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.95, 0.98, 1.0), crystal) * (0.7 + wrap * 0.3);
        roughness = 0.1 + crystal * 0.2;
        metallic = 0.05;
    } else if material_type < 6.5 {
        // Type 6: Water surface -- animated wave normals, blue-green, foam at shallow angles
        let uv = in.world_position.xz;
        let t = in.world_position.x * 0.01; // pseudo-time from position for static shader
        let wave = fbm(uv * 2.0 + vec2<f32>(t * 3.0, t * 1.7)) * 0.5;
        let foam = smoothstep(0.35, 0.5, wave);
        albedo = mix(vec3<f32>(0.02, 0.15, 0.2), vec3<f32>(0.05, 0.3, 0.35), wave);
        albedo = mix(albedo, vec3<f32>(0.8, 0.9, 0.95), foam * 0.6);
        roughness = mix(0.05, 0.6, foam);
        metallic = 0.02;
    } else if material_type < 7.5 {
        // Type 7: Leather -- Voronoi pore pattern, warm brown tones
        let uv = triplanar_uv(in.world_position, normal);
        let pores = voronoi(uv * 15.0);
        let coarse = fbm(uv * 4.0) * 0.15;
        albedo = mix(vec3<f32>(0.25, 0.13, 0.06), vec3<f32>(0.45, 0.28, 0.14), pores + coarse);
        roughness = 0.5 + (1.0 - pores) * 0.25;
        metallic = 0.0;
    } else if material_type < 8.5 {
        // Type 8: Crystal -- faceted sharp noise, prismatic color from view angle, high metallic
        let uv = triplanar_uv(in.world_position, normal);
        let facets = voronoi(uv * 12.0);
        let angle = dot(normal, view_dir);
        let prism = vec3<f32>(
            smoothstep(0.3, 0.7, sin(angle * 12.0) * 0.5 + 0.5),
            smoothstep(0.3, 0.7, sin(angle * 12.0 + 2.094) * 0.5 + 0.5),
            smoothstep(0.3, 0.7, sin(angle * 12.0 + 4.189) * 0.5 + 0.5)
        );
        albedo = mix(albedo * 0.3, prism, 0.7) * (0.6 + facets * 0.4);
        roughness = 0.02 + (1.0 - facets) * 0.08;
        metallic = 0.9;
    } else if material_type < 9.5 {
        // Type 9: Rust/Corroded -- FBM-driven orange-brown patches over base metal
        let uv = triplanar_uv(in.world_position, normal);
        let rust_mask = smoothstep(0.35, 0.65, fbm(uv * 3.0));
        let rust_color = vec3<f32>(0.5, 0.2, 0.05) + value_noise(uv * 20.0) * 0.1;
        albedo = mix(albedo, rust_color, rust_mask);
        roughness = mix(roughness, 0.85, rust_mask);
        metallic = mix(metallic, 0.1, rust_mask);
    } else if material_type < 10.5 {
        // Type 10: Moss/Growth -- green patches on upward/north-facing surfaces (world-space)
        let uv = in.world_position.xz;
        let up_factor = smoothstep(0.3, 0.8, normal.y); // grows on tops
        let coverage = smoothstep(0.3, 0.6, fbm(uv * 2.5)) * up_factor;
        let moss_color = vec3<f32>(0.15, 0.35, 0.08) + value_noise(uv * 12.0) * 0.08;
        albedo = mix(albedo, moss_color, coverage);
        roughness = mix(roughness, 0.9, coverage);
        metallic = mix(metallic, 0.0, coverage);
    } else if material_type < 11.5 {
        // Type 11: Lava -- black rock with glowing orange cracks, emissive in veins
        let uv = triplanar_uv(in.world_position, normal);
        let cracks = 1.0 - smoothstep(0.0, 0.12, voronoi(uv * 5.0));
        let heat = cracks * (0.7 + value_noise(uv * 8.0) * 0.3);
        albedo = mix(vec3<f32>(0.05, 0.04, 0.03), vec3<f32>(1.0, 0.35, 0.0), heat);
        proc_emissive = vec3<f32>(1.0, 0.3, 0.0) * heat * 3.0; // glowing cracks
        roughness = mix(0.9, 0.3, heat);
        metallic = 0.0;
    } else if material_type < 12.5 {
        // Type 12: Planet surface (v0.763) -- per-face color packed into the UV
        // channel by Mesh::from_planet_surface / terrain::planet_surface::
        // pack_color_to_uv. uv.x holds two 8-bit channels plus a water flag as
        // one exact integer (water*65536 + round(r*255)*256 + round(g*255),
        // max 131071 -- well inside f32's 2^24 exact-integer range); uv.y
        // holds blue as a plain float. All three corners of a flat-shaded face
        // carry the SAME uv, so linear interpolation leaves the packed integer
        // intact. Keep the decode in sync with terrain::planet_surface::
        // unpack_uv_to_color (unit-tested).
        //
        // params.w REPURPOSED for this type as a BIT FIELD (v0.816; a
        // single texture flag since v0.811): bit 0 = a baked per-pixel
        // albedo texture is bound at group 3 (replacing the per-face color
        // mosaic with smooth imagery), bit 1 = Settings > Graphics >
        // Planets "Surface detail" (the ocean waves + land micro-texture
        // below). lib.rs rewrites the value every frame, so the toggle
        // applies live. It never doubles as emissive here:
        emissive_strength = 0.0;
        let packed = u32(round(max(in.uv.x, 0.0)));
        let pr = f32((packed >> 8u) & 255u) / 255.0;
        let pg = f32(packed & 255u) / 255.0;
        let pw_bits = u32(round(max(material.params.w, 0.0)));
        let has_tex = (pw_bits & 1u) != 0u;
        let detail_on = (pw_bits & 2u) != 0u;
        // Planet-local frame pieces, filled on the textured path and reused
        // by the detail effects: the unit direction (equirect UV + land
        // noise domain), the local position in METRES (wave phases -- the
        // render-space radius converts the unit direction back to metric),
        // and that radius itself (converts wavelengths in metres to angular
        // noise frequencies).
        var dir = vec3<f32>(0.0, 0.0, 1.0);
        var p_local = vec3<f32>(0.0);
        var r_render = 1.0;
        if (has_tex) {
            // Per-pixel imagery path (v0.811). base_color.xyz is REPURPOSED
            // as the planet CENTER in render space (lib.rs updates it every
            // frame -- the floating origin moves it), because the chunked
            // patch meshes are anchored at their own patch centers, so
            // object.model[3] is NOT the planet center for them the way it
            // is for the uniform sphere. From the center, the planet-local
            // unit direction is exact for BOTH mesh paths:
            //   dir_world = fragment - center        (world space)
            //   dir_local = model^-1 * dir_world     (w=0: rotation only)
            // transpose(object.normal_matrix) IS model.inverse() exactly
            // (normal_matrix is inverse-transpose -- same trick as the
            // type-15 cloud shell), and any uniform scale in it washes out
            // in the normalize. This rides the planet's spin by
            // construction: the imagery is pinned to the rotating body.
            let inv_model = transpose(object.normal_matrix);
            let dir_world = in.world_position - material.base_color.xyz;
            dir = normalize((inv_model * vec4<f32>(dir_world, 0.0)).xyz);
            // Planet-local metric frame for the wave math: |dir_world| is
            // the fragment's render-space (= metre) distance from the
            // center, so dir * that IS the local position in metres --
            // inv_model's inverse scale never enters (it would land the
            // point in unit-sphere units).
            r_render = max(length(dir_world), 1.0);
            p_local = dir * r_render;
            // Equirectangular UV with the SAME handedness as terrain::
            // planet_heightmap::dir_to_latlon_deg (east = -z; +Y = north),
            // and the same registration: u = (lon+180)/360 puts texel
            // centers where the CPU sampler's cell centers are. The sampler
            // wraps u (antimeridian) and clamps v (poles), mirroring the
            // CPU grid's edge policy. textureSampleLevel (level 0) because
            // implicit-derivative sampling would smear a full-width texture
            // fetch across the u = 1 -> 0 seam.
            let lon = atan2(-dir.z, dir.x);
            let lat = asin(clamp(dir.y, -1.0, 1.0));
            let eq_uv = vec2<f32>(lon * 0.15915494 + 0.5, 0.5 - lat * 0.31830987);
            // Grading (ocean floor / land gain / sea ice) is baked into the
            // texture; the sRGB view decodes to linear on sample. No
            // base_color tint here -- that slot carries the center.
            albedo = textureSampleLevel(albedo_texture, albedo_sampler, eq_uv, 0.0).rgb;
        } else {
            // Fallback: the per-face packed color (classifier planets, or a
            // planet whose imagery failed to bake).
            albedo = vec3<f32>(pr, pg, in.uv.y) * material.base_color.rgb;
        }
        // Pixel footprint on the surface (metres per pixel), the analytic
        // anti-alias estimate every detail octave fades against (see the
        // PLANET_PIXEL_ANGLE block above -- no derivatives needed).
        let dist_frag = length(camera.view_pos.xyz - in.world_position);
        let footprint = max(dist_frag * PLANET_PIXEL_ANGLE, 0.001);
        let is_water = (packed & 65536u) != 0u;
        // Land close-range detail (v0.816): multiplicative luminance
        // variation under the photo -- orbit view identical (every octave
        // fades to zero there), descent keeps revealing structure instead
        // of bilinear blur. Textured path only: the per-face fallback has
        // no planet-local frame to sample in.
        if (has_tex && detail_on && !is_water) {
            albedo = albedo * land_detail_factor(dir, r_render, footprint);
        }
        // Ocean sun glint (v0.810): every orbital photo has a bright specular
        // spot where the sun mirrors off the sea; without it the ocean reads
        // as painted plastic. Water faces are flagged in bit 16 by the mesh
        // builder (below-sea-level faces of has_water planets -- their
        // normals are the smooth sphere normals, so the lobe is round).
        // Implemented as an explicit Blinn-Phong lobe toward the SUN only,
        // added via proc_emissive AFTER the diffuse path: reusing the
        // material roughness would also glint the fixed cool fill light,
        // painting a second physically bogus hotspot. Land gets nothing.
        //
        // v0.816: up close this single smooth lobe becomes REAL water. Wave
        // presence (the anti-alias fade of the longest wave octave) blends
        // the whole water response from the v0.810 far-field look (presence
        // 0: bit-identical diffuse + glint) to the full wave-perturbed
        // shading in water_shade (presence 1: Fresnel sky mirror, bathymetry
        // body, moving sun sparkle). The diffuse albedo hands its energy to
        // the water term as presence rises so nothing double-counts.
        if (is_water) {
            let sun_l = normalize(camera.sun_direction.xyz);
            let half_v = normalize(view_dir + sun_l);
            // Day gate: the glint fades smoothly at the terminator and never
            // appears on the night side (emissive would otherwise ignore
            // the sun's geometry entirely).
            let day = clamp(dot(normal, sun_l), 0.0, 1.0);
            // Exponent 220 = a ~5 degree half-vector lobe: a glint spot
            // roughly a tenth of the disc across, matching the soft bright
            // patch (sun + surrounding wave glitter) in orbital photos.
            let spec = pow(max(dot(normal, half_v), 0.0), 220.0);
            // 0.7 * sun intensity 2.5 peaks ~1.75 pre-tonemap: bright, not
            // a blown white hole.
            let old_glint =
                camera.sun_color.rgb * camera.sun_direction.w * spec * day * 0.7;
            var presence = 0.0;
            if (has_tex && detail_on) {
                presence = wave_presence(footprint);
                // Sea ice carries the water flag too (below-sea polar faces
                // graded toward cap white) -- fade the waves out as the
                // albedo brightens so pack ice never shades like open sea.
                let lum = max(albedo.r, max(albedo.g, albedo.b));
                presence = presence
                    * (1.0 - smoothstep(WATER_ICE_LUM_LO, WATER_ICE_LUM_HI, lum));
            }
            if (presence > 0.001) {
                // The cloud clock doubles as the wave clock (same
                // documented-pad time slot, app-start-relative seconds).
                let t_wave = camera.sun_color.w;
                let grad = water_wave_gradient(p_local, dir, t_wave, footprint);
                let n_pert_local = normalize(dir - grad);
                let n_pert = normalize(
                    (object.model * vec4<f32>(n_pert_local, 0.0)).xyz,
                );
                let water_rgb = water_shade(albedo, normal, n_pert, view_dir);
                proc_emissive = mix(old_glint, water_rgb, presence);
                // Hand the diffuse + ambient energy over to the water term
                // and flatten the residual GGX response as presence rises.
                albedo = albedo * (1.0 - presence);
                roughness = mix(roughness, 1.0, presence);
            } else {
                proc_emissive = old_glint;
            }
        }
    } else if material_type < 13.5 {
        // Type 13: Atmosphere shell (v0.763) -- fresnel limb tint on a slightly
        // oversized transparent sphere. Nearly invisible looking straight
        // through the center, densest at the grazing-angle limb, so it reads as
        // a thin halo of air hugging the planet. Airless bodies simply never
        // spawn the shell. KEPT as the fallback behind Settings > Graphics >
        // Planets > "Scattering atmosphere" (off = this path): forever-dev
        // A/B reference + a safety hatch if a GPU dislikes the type-14 math.
        let limb = pow(1.0 - abs(dot(normal, view_dir)), 2.0);
        out_alpha = material.base_color.a * limb;
        proc_emissive = albedo * limb * 0.6; // limb stays visible on the night side edge
        roughness = 1.0;
        metallic = 0.0;
    }

    // Fresnel reflectance at normal incidence
    // Dielectrics: 0.04, metals: tinted by albedo
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    // Evaluate main directional light (from camera uniforms)
    var lo = evaluate_light(
        camera.sun_direction.xyz, camera.sun_color.rgb, camera.sun_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0);

    // Evaluate fill light (from camera uniforms)
    lo = lo + evaluate_light(
        camera.fill_direction.xyz, camera.fill_color.rgb, camera.fill_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0);

    // Point + spot lights — UNCAPPED (v0.782): the storage buffer holds every
    // scene light; light_count bounds the loop. The early range/attenuation
    // rejection keeps far lights nearly free, so the practical ceiling is GPU
    // fill cost, not a software cap.
    let num_lights = i32(camera.light_count.x);
    for (var i = 0; i < num_lights; i = i + 1) {
        var light_pos = scene_lights[i].pos_intensity.xyz;
        let intensity = scene_lights[i].pos_intensity.w;
        let light_color = scene_lights[i].color_range.xyz;
        let radius = scene_lights[i].color_range.w;
        let sent = scene_lights[i].spot.w;

        // LINE light (v0.786, sentinel cos_outer == -2.0): the whole segment
        // [pos, spot.xyz] emits -- light each fragment from the CLOSEST point
        // on the segment (capsule-light representative point), so a strip
        // washes the full wall instead of pooling at one point. Rust mirror +
        // tests: light::line_light_closest_point.
        if (sent < -1.5) {
            let a = light_pos;
            let b = scene_lights[i].spot.xyz;
            let ab = b - a;
            let t = clamp(dot(in.world_position - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
            light_pos = a + ab * t;
        }

        let to_light = light_pos - in.world_position;
        let dist = length(to_light);

        // Cheap reject: outside the light's range, contribution is exactly 0
        // (the linear range window below hits zero at dist == radius).
        if (dist >= radius) { continue; }

        let light_dir = to_light / max(dist, 0.001);

        // Attenuation: inverse square with radius falloff
        var attenuation = intensity / (1.0 + dist * dist) * max(1.0 - dist / max(radius, 0.001), 0.0);

        // Spot cone (v0.639): cos_outer == -1.0 is the Point/Bar sentinel, so this only narrows
        // an actual spot light -- zero extra cost/behavior change for every other light.
        let spot = scene_lights[i].spot;
        let cos_outer = spot.w;
        if (cos_outer > -1.0) {
            let cos_inner = scene_lights[i].cone_inner.x;
            // spot.xyz is the aim direction in the light-to-fragment sense; -light_dir (which
            // points fragment-to-light) flips to the same sense for the dot product.
            let cos_angle = dot(normalize(spot.xyz), -light_dir);
            attenuation = attenuation * smoothstep(cos_outer, cos_inner, cos_angle);
        }

        if (attenuation > 0.001) {
            lo = lo + evaluate_light(light_dir, light_color, attenuation, normal, view_dir, albedo, metallic, roughness, f0);
        }
    }

    // Ambient (near-zero so space is truly black and the sun is the only
    // light source). A thin floor prevents absolute black so unlit faces
    // still have a subtle silhouette against the starfield instead of
    // vanishing into artefacts from tone mapping.
    let ambient = albedo * vec3<f32>(0.005, 0.005, 0.006);

    var color = ambient + lo;

    // Emissive: params.w controls emissive strength (0 = none, 1+ = glow)
    // Emissive objects use base_color as their glow color, bypassing lighting.
    // (Declared as a var at the top; type 12 zeroes it -- see there.)
    if (emissive_strength > 0.0) {
        color = color + albedo * emissive_strength;
    }

    // Procedural emissive (e.g. lava cracks) -- additive, independent of params.w
    color = color + proc_emissive;

    // ACES-like tone mapping (more filmic than Reinhard)
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, out_alpha);
}
