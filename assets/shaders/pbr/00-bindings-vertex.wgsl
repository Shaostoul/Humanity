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
// Light-tile lists (clustering L1b): per-screen-tile counts + indices into
// scene_lights, from renderer/light_tiles.rs. Active when shadow_u.params2.z
// (the tile pixel width) is non-zero; zero = the classic full loop.
@group(0) @binding(2) var<storage, read> tile_counts: array<u32>;
@group(0) @binding(3) var<storage, read> tile_indices: array<u32>;
const TILE_COLS: u32 = 16u;
const TILE_ROWS: u32 = 9u;
const TILE_CAP: u32 = 64u;
// BEGIN OBJECT-SOURCE. Everything between these markers is the CLASSIC
// per-draw object plumbing (one uniform slot per draw, dynamic offset).
// The terrain-batch pipeline compiles a SECOND module where
// shader_loader::batched_variant_of() replaces this whole block with a
// version that rebuilds the model matrix from the per-INSTANCE vertex
// attribute (inst_pos_fade, vertex buffer slot 1) plus one shared batch
// uniform -- that is how 12k patch draws share one bind group with zero
// per-draw rebinds, and it works for BOTH direct draws (i..i+1 ranges)
// and multi_draw_indexed_indirect, because instance-rate ATTRIBUTE fetch
// respects first_instance everywhere (unlike the instance_index builtin,
// which this DX12 adapter reports broken for indirect draws). Shared code
// must therefore NEVER touch `object.` directly: go through obj_model() /
// obj_normal_matrix() / obj_lod_fade(), which both variants define.
// g_inst_data is set at the top of vs_main (the attribute) and fs_main
// (flat varying) so the accessors work identically in both stages; the
// classic variant ignores it (classic draws bind a 16-byte zero dummy at
// slot 1).
@group(1) @binding(0) var<uniform> object: ObjectUniforms;
var<private> g_inst_data: vec4<f32> = vec4<f32>(0.0);
fn obj_model() -> mat4x4<f32> { return object.model; }
fn obj_normal_matrix() -> mat4x4<f32> { return object.normal_matrix; }
fn obj_lod_fade() -> f32 { return object.model[0].w; }
// END OBJECT-SOURCE
@group(2) @binding(0) var<uniform> material: MaterialUniforms;
// Per-pixel planet albedo imagery (v0.811): an equirectangular sRGB texture
// (sampling returns LINEAR automatically) with the orbital-look grading
// already baked in at upload time (terrain::planet_surface::
// bake_albedo_rgba). Non-planet draws bind a 1x1 white fallback and never
// sample it (the type-12 params.w flag gates the lookup), so this group is
// harmless to every other material type.
@group(3) @binding(0) var albedo_texture: texture_2d<f32>;
@group(3) @binding(1) var albedo_sampler: sampler;
// Tiling 3D cloud-noise volumes (clouds increment 3, material type 15 High
// quality). Generated procedurally at startup (renderer::cloud_noise) and
// shared engine-wide: SHAPE = 128^3 Perlin-Worley (R) + inverted-Worley
// octaves (GBA); DETAIL = 64^3 inverted-Worley octaves (RGB). The sampler
// repeats on all axes -- the volumes tile seamlessly by construction. Only
// the type-15 High path ever samples these; every other draw binds them
// inertly (same pattern as the albedo texture above).
@group(3) @binding(2) var cloud_shape_tex: texture_3d<f32>;
@group(3) @binding(3) var cloud_detail_tex: texture_3d<f32>;
@group(3) @binding(4) var cloud_tile_sampler: sampler;
// Live weather map (v0.874): equirect RG8 from NASA GIBS MODIS cloud
// fraction. R = real cloud fraction 0..1, G = validity (0 = no data ->
// pure procedural coverage). Zero-filled until the fetcher delivers, so
// the procedural sky is always the fallback with no mode flag needed.
@group(3) @binding(5) var weather_map: texture_2d<f32>;
// Sun shadow map (v0.899): a 4096^2 near-field ortho depth map rendered
// from the sun each frame (renderer::render_celestial_onto), covering
// ~1.5 km around the camera. Terrain, vegetation cards, and celestial
// geometry all cast; every lit fragment (terrain AND interior) receives.
@group(3) @binding(6) var shadow_map: texture_depth_2d;
@group(3) @binding(7) var shadow_samp: sampler_comparison;
struct ShadowUniforms {
    light_vp: mat4x4<f32>,
    // x = enable, y = shadow strength (0..1), z = 1/map size,
    // w = tree-card HIDE radius (cards yield to 3D models inside it).
    params: vec4<f32>,
    // v0.924 vegetation LOD: x = tree-card FAR cutoff (m) - the silhouette
    // stage's outer distance, the "Tree silhouette distance" Settings
    // slider. yzw reserved for the grass/shrub ladder stages.
    params2: vec4<f32>,
};
@group(3) @binding(8) var<uniform> shadow_u: ShadowUniforms;
// Ground PBR texture array (v0.907): the ambientCG CC0 material sets that
// give terrain REAL close-range surface texture. Layers 0..3 = color
// (grass, dirt, rock, sand) already converted to LINEAR bytes on load;
// layers 4..7 = the matching OpenGL tangent-space normal maps. ground_samp
// wraps (tiling) with 4x anisotropy; the albedo sampler clamps, hence the
// separate binding. A build without the asset pack gets neutral 1x1 layers
// that render identically to the pre-texture look.
@group(3) @binding(9) var ground_tex: texture_2d_array<f32>;
@group(3) @binding(10) var ground_samp: sampler;
// Sky-view LUT (stage 3c): per-frame distant-sky radiance, sampled by the
// near-surface sky hybrid in atmosphere_scattering.
@group(3) @binding(13) var sky_view_tex: texture_2d<f32>;
// Tree-card sprite atlas (v0.961): 3x2 grid of baked conifer sprites the
// type-12 sprite-card branch samples (gated by material.params.w bit 2).
@group(3) @binding(14) var tree_atlas_tex: texture_2d<f32>;
// FFT ocean displacement tile (v0.1029, water-fft.md increment 1): a
// 128x128 R32Float height field over a 64 m periodic tile, computed on
// the CPU (terrain/ocean_fft.rs) and re-uploaded each frame. VERTEX
// stage: the type-16 water branch samples it via manual-bilinear
// textureLoad so the CPU buoyancy twin reads bit-matching heights from
// the SAME array. Gated by camera.light0_cone_inner.x (the FFT-mode
// flag beside the ground anchor in .yzw).
@group(3) @binding(15) var water_fft_tex: texture_2d<f32>;

// 3x3 PCF visibility of the sun from a world-space point. 1.0 = fully lit.
// Fragments outside the ortho box (or with shadows off) return fully lit,
// so the effect fades to the status quo beyond the near field.
fn sun_shadow(world_pos: vec3<f32>, n_dot_l: f32) -> f32 {
    if (shadow_u.params.x < 0.5) {
        return 1.0;
    }
    let lc = shadow_u.light_vp * vec4<f32>(world_pos, 1.0);
    let ndc = lc.xyz / lc.w;
    if (abs(ndc.x) > 0.99 || abs(ndc.y) > 0.99 || ndc.z <= 0.001 || ndc.z >= 0.999) {
        return 1.0;
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    // Slope-scaled bias: grazing sun needs more to dodge acne on the
    // 1056-vert terrain triangles; vegetation cards (normal = radial up)
    // land near the flat-bias end.
    let bias = max(0.0025 * (1.0 - n_dot_l), 0.0006);
    let texel = shadow_u.params.z;
    var lit = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let o = vec2<f32>(f32(dx), f32(dy)) * texel;
            lit = lit + textureSampleCompareLevel(shadow_map, shadow_samp, uv + o, ndc.z - bias);
        }
    }
    lit = lit / 9.0;
    return mix(1.0 - shadow_u.params.y, 1.0, lit);
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // Per-INSTANCE data from vertex buffer slot 1 (step_mode Instance):
    // xyz = batched-patch translation, w = LOD fade. Classic draws bind a
    // 16-byte zero dummy here and never read it; the terrain-batch
    // variant's accessors are built from it.
    @location(4) inst_pos_fade: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // Per-instance data carried to the fragment stage (flat = no
    // interpolation): the terrain-batch variant's fragment accessors
    // rebuild the model matrix from it. Zero for classic draws.
    @location(3) @interpolate(flat) inst_data: vec4<f32>,
    // The packed per-face color/flags channel, FLAT (v0.1013, draw-batching
    // increment 3): flat interpolation takes the triangle's FIRST vertex,
    // which lets terrain grid vertices be SHARED between faces while each
    // face still reads its own pack from its provoking vertex. Cards and
    // the water shell keep reading the smooth `uv` (sprite texcoords and
    // the baked shoreline depth genuinely interpolate); only the type-12
    // per-face decode reads this. With unshared meshes pack == uv exactly.
    @location(4) @interpolate(flat) pack: vec2<f32>,
};

// ── Ocean surface waves (material type 16, v0.876 real-water Stage 1) ──
//
// GEOMETRIC wave height for the water-shell mesh: four directional trains
// (a subset of the shading octaves' wavelengths) summed as plain vertical
// sinusoids. Heights are REAL-WORLD swell amplitudes, deliberately separate
// from the WAVE*_SLOPE shading table above (those are slope-only tunings for
// normal perturbation; converting them to heights via A = slope*lambda/tau
// would give 16 m swells). No domain warp here: the CPU physics twin
// (terrain::ocean_waves) must reproduce this height EXACTLY (the drawn ==
// sampled golden rule from docs/design/ocean.md), and keeping the sum to
// pure cosines makes the twin trivial + testable. Crest snaking still comes
// from the fragment normal warp, which is shading-only.
// KEEP IN LOCKSTEP with terrain/ocean_waves.rs (guard test parses this
// file for the OCEAN_W* constants).
const OCEAN_W1_LAMBDA: f32 = 2000.0;
const OCEAN_W1_CPS: f32 = 0.028;
const OCEAN_W1_HEIGHT: f32 = 1.1;
const OCEAN_W2_LAMBDA: f32 = 360.0;
const OCEAN_W2_CPS: f32 = 0.07;
const OCEAN_W2_HEIGHT: f32 = 0.7;
const OCEAN_W3_LAMBDA: f32 = 150.0;
const OCEAN_W3_CPS: f32 = 0.105;
const OCEAN_W3_HEIGHT: f32 = 0.45;
// W4 joined the anchored-domain family (v0.1018, residual jitter: at
// lambda 50 the planet-coordinate f32 noise was still ~1% of a wavelength
// of camera-coupled wobble). 32 divides the 64 m anchor modulus;
// dispersion retuned (c ~ 1.25 sqrt(lambda)).
const OCEAN_W4_LAMBDA: f32 = 32.0;
const OCEAN_W4_CPS: f32 = 0.221;
const OCEAN_W4_HEIGHT: f32 = 0.4;
// v0.912 (operator: "add fake geometry to the ocean to simulate the
// smaller waves... right now the waves seem exclusively texture
// related"): two SHORT geometric trains give near water real moving
// chop. Speeds follow deep-water dispersion (c ~ 1.25 sqrt(lambda)).
// Faded out by ~800 m in the vertex shader - beyond that they are
// sub-vertex and shading owns the detail.
// v0.957 (operator: "still just look like a flat 2D shape. No actual
// wave height"): trains 4-6 lifted to Beaufort-4-ish amplitudes now that
// WATER_MAX_PATCH_DEPTH 17 gives ~4.8 m vertices near the camera - the
// chop is real displaced geometry with silhouettes, not shading fiction.
// CPU twin: terrain::ocean_waves::TRAINS (lockstep-tested).
// v0.1017 (operator: "the water is very jittery... all the verts are
// kinda bouncing up and down rapidly"): the chop trains used to phase off
// planet-radius f32 coordinates, whose ~0.5 m quantization SHIFTS as the
// camera moves - most of a 6 m wavelength of camera-coupled noise. They
// now phase in the camera-ANCHORED small domain (the wave texture's 64 m
// modulus anchor) with AXIS-ALIGNED directions and wavelengths that
// DIVIDE 64, so anchor re-snaps shift phase by exact integers (no pops)
// and the CPU twin's f64 planet-coordinate phase matches mod 1 exactly.
// Dispersion retuned to the new lengths (c ~ 1.25 sqrt(lambda)).
const OCEAN_W5_LAMBDA: f32 = 16.0;
const OCEAN_W5_CPS: f32 = 0.31;
const OCEAN_W5_HEIGHT: f32 = 0.35;
const OCEAN_W6_LAMBDA: f32 = 6.4;
const OCEAN_W6_CPS: f32 = 0.49;
const OCEAN_W6_HEIGHT: f32 = 0.1;
const OCEAN_W4_DIR: vec3<f32> = vec3<f32>(0.0, 1.0, 0.0);
const OCEAN_W5_DIR: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);
const OCEAN_W6_DIR: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);

// One train's vertical height contribution at planet-local point p_m.
// Phase = distance along the fixed 3D direction in wavelengths, wrapped
// through fract() BEFORE the cos exactly like wave_octave above (at
// planet-radius coordinates a raw phase argument kills GPU sin precision).
fn ocean_height_train(p_m: vec3<f32>, d: vec3<f32>, lambda_m: f32, cps: f32, h: f32, t: f32) -> f32 {
    let phase = fract(dot(p_m, d) / lambda_m - t * cps);
    return h * cos(phase * TAU);
}

// Total wave height (metres, signed) at planet-local position p_m. Wave
// directions reuse the shading octaves' fixed unit vectors so crests align
// with what the fragment normals show.
//
// PER-TRAIN RESOLUTION FADES (v0.1017, water arc increment 1 - operator:
// "holes through the water along the seams" + the hard checkerboard tile
// lines at altitude). The water mesh's vertex spacing grows with camera
// distance (screen-size-driven LOD, ~dist/300 at default split), so each
// train stops being resolvable past dist ~ 60 * lambda: beyond that the
// COARSE side of a cross-depth patch border draws the wave as a straight
// under-sampled edge while the fine side follows the curve - T-junction
// GAPS up to the train's amplitude (the reported holes), and each LOD
// ring polygonalizes the train differently (the reported tile lines).
// Fading every train out BEFORE its under-resolution distance makes any
// border's displacement negligible exactly where neighbors stop agreeing,
// which is the promise the old single global fade only kept for the
// longest swell. The CPU physics twin evaluates at the player (dist ~ 0,
// every fade = 1), so drawn == sampled still holds where it matters.
fn ocean_train_fade(cam_dist: f32, lambda_m: f32) -> f32 {
    let reach = 60.0 * lambda_m;
    return 1.0 - smoothstep(reach * 0.6, reach, cam_dist);
}

// `p_m` is the planet-radius position (long swells tolerate its f32
// noise: <= 1% of wavelength at lambda >= 50); `p_anch` is the SAME point
// in the camera-anchored small domain (see the chop constants above).
fn ocean_wave_height(p_m: vec3<f32>, p_anch: vec3<f32>, t: f32, cam_dist: f32) -> f32 {
    // Long swells: the global 2-8 km fade (applied by the caller) is well
    // inside their resolution reach, so they need no per-train fade.
    var h = ocean_height_train(p_m, WAVE1_DIR, OCEAN_W1_LAMBDA, OCEAN_W1_CPS, OCEAN_W1_HEIGHT, t);
    h = h + ocean_height_train(p_m, WAVE3_DIR, OCEAN_W2_LAMBDA, OCEAN_W2_CPS, OCEAN_W2_HEIGHT, t);
    h = h + ocean_height_train(p_m, WAVE4_DIR, OCEAN_W3_LAMBDA, OCEAN_W3_CPS, OCEAN_W3_HEIGHT, t)
        * ocean_train_fade(cam_dist, OCEAN_W3_LAMBDA);
    h = h + ocean_height_train(p_anch, OCEAN_W4_DIR, OCEAN_W4_LAMBDA, OCEAN_W4_CPS, OCEAN_W4_HEIGHT, t)
        * ocean_train_fade(cam_dist, OCEAN_W4_LAMBDA);
    // Short chop: resolution-faded like the rest (the old fixed 250-800 m
    // band under-resolved the 6 m train past ~360 m), phased in the
    // anchored domain (the jitter fix).
    let near5 = ocean_train_fade(cam_dist, OCEAN_W5_LAMBDA);
    let near6 = ocean_train_fade(cam_dist, OCEAN_W6_LAMBDA);
    if (near5 > 0.001 || near6 > 0.001) {
        h = h + ocean_height_train(p_anch, OCEAN_W5_DIR, OCEAN_W5_LAMBDA, OCEAN_W5_CPS, OCEAN_W5_HEIGHT, t) * near5;
        h = h + ocean_height_train(p_anch, OCEAN_W6_DIR, OCEAN_W6_LAMBDA, OCEAN_W6_CPS, OCEAN_W6_HEIGHT, t) * near6;
    }
    return h;
}

// ---- FFT ocean (increments 1-3) ---------------------------------------
// TWO cascades since v0.1040 (operator: "the texture repeats A LOT...
// looks like a grid"): cascade A (64 m tile, short chop) rides the 64 m
// ground anchor; cascade B (256 m tile, mid/long waves) rides its OWN
// mod-256 anchor in light4_cone_inner.xyz. Each tile divides its own
// anchor modulus, so every snap shifts each tile by exactly one period.
// ONE 128x256 texture: A in rows [0,128), B in rows [128,256). LOCKSTEP
// with ocean_fft.rs (FFT_TILE_M, FFT_B_TILE_M, FFT_N, FFT_TEX_H,
// PLANE_OFF, fade lambdas) - a mismatch breaks drawn == sampled.
const OCEAN_FFT_TILE_M: f32 = 64.0;
const OCEAN_FFT_B_TILE_M: f32 = 256.0;
const OCEAN_FFT_N: i32 = 128;
const OCEAN_FFT_ROW_B: i32 = 128;
// Per-cascade resolution fades (via ocean_train_fade): A's short chop
// dies by ~1 km (the seam fix - under-resolved waves vanish before
// cross-LOD borders can disagree about them); B's mid waves carry the
// far field the operator called flat.
const OCEAN_FFT_A_FADE_LAMBDA: f32 = 16.0;
const OCEAN_FFT_B_FADE_LAMBDA: f32 = 96.0;
const OCEAN_FFT_OFF_X: vec2<f32> = vec2<f32>(0.0, 0.0);
const OCEAN_FFT_OFF_Y: vec2<f32> = vec2<f32>(0.271, 0.417);
const OCEAN_FFT_OFF_Z: vec2<f32> = vec2<f32>(0.613, 0.129);

// Manual-bilinear tile sample (wraps within one cascade's 128 rows;
// row_base selects the cascade). textureLoad instead of a sampler:
// exact texel math the CPU twin reproduces line for line. Returns the
// packed texel: r = height, g = slope_u, b = slope_v, a = foam.
fn fft_tile_sample(uv_in: vec2<f32>, row_base: i32) -> vec4<f32> {
    let nf = f32(OCEAN_FFT_N);
    let u = fract(uv_in.x) * nf;
    let v = fract(uv_in.y) * nf;
    let x0 = i32(floor(u)) % OCEAN_FFT_N;
    let y0 = i32(floor(v)) % OCEAN_FFT_N;
    let x1 = (x0 + 1) % OCEAN_FFT_N;
    let y1 = (y0 + 1) % OCEAN_FFT_N;
    let fx = fract(u);
    let fy = fract(v);
    let a = mix(textureLoad(water_fft_tex, vec2<i32>(x0, row_base + y0), 0),
                textureLoad(water_fft_tex, vec2<i32>(x1, row_base + y0), 0), fx);
    let b = mix(textureLoad(water_fft_tex, vec2<i32>(x0, row_base + y1), 0),
                textureLoad(water_fft_tex, vec2<i32>(x1, row_base + y1), 0), fx);
    return mix(a, b, fy);
}

// Triplanar projection of ONE cascade's tile, blended by the squared
// radial normal: a single 2D parameterization of a sphere always
// degenerates somewhere (xz collapses at the equator), so - like the
// trains' three axis-aligned directions - the tile is projected on all
// three planes. Fixed per-plane UV offsets decorrelate the projections.
fn fft_cascade_height(p_anch: vec3<f32>, radial: vec3<f32>, tile_m: f32, row_base: i32) -> f32 {
    let w2 = radial * radial;
    let q = p_anch / tile_m;
    var h = w2.x * fft_tile_sample(vec2<f32>(q.y, q.z) + OCEAN_FFT_OFF_X, row_base).x;
    h = h + w2.y * fft_tile_sample(vec2<f32>(q.x, q.z) + OCEAN_FFT_OFF_Y, row_base).x;
    h = h + w2.z * fft_tile_sample(vec2<f32>(q.x, q.y) + OCEAN_FFT_OFF_Z, row_base).x;
    return h;
}

// Both cascades with their own resolution fades: p64 is the 64 m-anchored
// position, p256 the 256 m-anchored one.
fn fft_ocean_height(p64: vec3<f32>, p256: vec3<f32>, radial: vec3<f32>, cam_dist: f32) -> f32 {
    return fft_cascade_height(p64, radial, OCEAN_FFT_TILE_M, 0)
        * ocean_train_fade(cam_dist, OCEAN_FFT_A_FADE_LAMBDA)
        + fft_cascade_height(p256, radial, OCEAN_FFT_B_TILE_M, OCEAN_FFT_ROW_B)
            * ocean_train_fade(cam_dist, OCEAN_FFT_B_FADE_LAMBDA);
}

// Increment 2 (v0.1031): triplanar-blended shading fields - xyz = the 3D
// height gradient in the planet-local frame, w = the Jacobian whitecap
// factor. Slopes are stored per tile axis (g = d h/d u, b = d h/d v);
// each projection plane maps (u, v) onto its own two world axes before
// the radial^2 blend, so the summed gradient lives in the same frame as
// `dir` and can perturb the water normal directly.
fn fft_cascade_shading(p_anch: vec3<f32>, radial: vec3<f32>, tile_m: f32, row_base: i32) -> vec4<f32> {
    let w2 = radial * radial;
    let q = p_anch / tile_m;
    let sx = fft_tile_sample(vec2<f32>(q.y, q.z) + OCEAN_FFT_OFF_X, row_base);
    let sy = fft_tile_sample(vec2<f32>(q.x, q.z) + OCEAN_FFT_OFF_Y, row_base);
    let sz = fft_tile_sample(vec2<f32>(q.x, q.y) + OCEAN_FFT_OFF_Z, row_base);
    var g = w2.x * vec3<f32>(0.0, sx.y, sx.z);
    g = g + w2.y * vec3<f32>(sy.y, 0.0, sy.z);
    g = g + w2.z * vec3<f32>(sz.y, sz.z, 0.0);
    let foam = w2.x * sx.w + w2.y * sy.w + w2.z * sz.w;
    return vec4<f32>(g, foam);
}

// Both cascades' shading, distance-faded like the geometry so normals
// never carry detail the verts no longer draw.
fn fft_ocean_shading(p64: vec3<f32>, p256: vec3<f32>, radial: vec3<f32>, cam_dist: f32) -> vec4<f32> {
    return fft_cascade_shading(p64, radial, OCEAN_FFT_TILE_M, 0)
        * ocean_train_fade(cam_dist, OCEAN_FFT_A_FADE_LAMBDA)
        + fft_cascade_shading(p256, radial, OCEAN_FFT_B_TILE_M, OCEAN_FFT_ROW_B)
            * ocean_train_fade(cam_dist, OCEAN_FFT_B_FADE_LAMBDA);
}

// FFT-mode total height: the analytic long swells (2000/360/150 m) stay,
// the two cascades carry everything from 256 m down. The fields carry
// their own time evolution (the CPU re-uploads each frame), so no t for
// them. CPU twin: ocean_fft::wave_height_shoaled_fft_m.
fn ocean_wave_height_fft(p_m: vec3<f32>, p64: vec3<f32>, p256: vec3<f32>, t: f32, cam_dist: f32, radial: vec3<f32>) -> f32 {
    var h = ocean_height_train(p_m, WAVE1_DIR, OCEAN_W1_LAMBDA, OCEAN_W1_CPS, OCEAN_W1_HEIGHT, t);
    h = h + ocean_height_train(p_m, WAVE3_DIR, OCEAN_W2_LAMBDA, OCEAN_W2_CPS, OCEAN_W2_HEIGHT, t);
    h = h + ocean_height_train(p_m, WAVE4_DIR, OCEAN_W3_LAMBDA, OCEAN_W3_CPS, OCEAN_W3_HEIGHT, t)
        * ocean_train_fade(cam_dist, OCEAN_W3_LAMBDA);
    h = h + fft_ocean_height(p64, p256, radial, cam_dist);
    return h;
}

// One call for the active water model (trains or FFT cascades).
fn water_disp_height(p_m: vec3<f32>, p64: vec3<f32>, p256: vec3<f32>, t: f32, cam_dist: f32, radial: vec3<f32>) -> f32 {
    if (camera.light0_cone_inner.x > 0.5) {
        return ocean_wave_height_fft(p_m, p64, p256, t, cam_dist, radial);
    }
    return ocean_wave_height(p_m, p64, t, cam_dist);
}

// Geomorph weld window (v0.1041), in units of cell * K where K =
// px_per_rad / split_px arrives per frame in light4_cone_inner.w - the
// SELECTION's own pixel-error scale, so the morph tracks the real LOD
// handoff at any resolution/FOV/split setting. From screen_error_px +
// the 1.15/0.7 hysteresis: a cell-c leaf is born below ~1.74cK and
// dies above ~2.86cK; full morph by 1.74cK, and the next level starts
// morphing at 1.43*(2c)K = 2.86cK - the windows dovetail exactly.
// (The first attempt hardcoded a guessed scale; every patch collapsed
// a level early and the sea became giant plates.)
const WATER_WELD_MORPH_START: f32 = 1.43;
const WATER_WELD_MORPH_END: f32 = 1.74;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    g_inst_data = vertex.inst_pos_fade;
    var out: VertexOutput;
    var world_pos = obj_model() * vec4<f32>(vertex.position, 1.0);
    // The model matrix's w ROW carries per-object metadata (model[0].w =
    // LOD crossfade, v0.920), so rebuild the homogeneous w explicitly. For
    // an ordinary TRS matrix this is a no-op; with metadata present it is
    // what keeps clip_position correct. xyz is untouched by the w row.
    world_pos = vec4<f32>(world_pos.xyz, 1.0);
    // Water shell (type 16): displace the vertex radially by the analytic
    // wave height, computed in the planet-local frame via the same
    // center + inverse-rotation trick the planet fragment branch uses
    // (material.base_color.xyz = planet center in render space;
    // transpose(normal_matrix) = model^-1).
    //
    // params.x (the metallic slot, unused by water) > 0.5 marks the FLAT
    // BACKSTOP shell (v0.1019): the coarse deep-water layer under the
    // wave shell never takes WAVES - but since v0.1043 it DOES take the
    // geomorph weld below, so its own cross-LOD cracks close too.
    if (material.params.z >= 15.5 && material.params.z < 16.5) {
        let inv_model = transpose(obj_normal_matrix());
        let dir_world = world_pos.xyz - material.base_color.xyz;
        let r = length(dir_world);
        if (r > 1.0) {
            let radial = dir_world / r;
            let dir = normalize((inv_model * vec4<f32>(dir_world, 0.0)).xyz);
            let cam_dist = length(camera.view_pos.xyz - world_pos.xyz);
            // ── GEOMORPH WELD (v0.1042 window, v0.1043 base-position fix) ──
            // Odd lattice verts carry the half-offset to their two
            // coarser-level parents in the NORMAL slot (zero on verts that
            // survive coarsening). weld_w ramps to 1 before the LOD switch
            // (window = [1.43, 1.74] * |delta| * K, K = px_per_rad/split_px
            // from the selection itself, in light4_cone_inner.w).
            let delta = vertex.normal;
            let dl = length(delta);
            let weld_k = camera.light4_cone_inner.w;
            var weld_w = 0.0;
            if (dl > 0.01 && weld_k > 1.0) {
                weld_w = smoothstep(
                    WATER_WELD_MORPH_START * dl * weld_k,
                    WATER_WELD_MORPH_END * dl * weld_k,
                    cam_dist,
                );
            }
            // CHORD SAG (v0.1043, the seam the v0.1042 weld still left -
            // operator: "the vertices seem welded but I keep seeing gaps
            // along the edges... easiest to notice at dusk/dawn"): this
            // vert sits ON the sea sphere, but the coarser neighbor draws
            // that span as a straight triangle EDGE whose midpoint lies
            // |delta|^2 / 2r INSIDE the sphere (sagitta of a 2|delta|
            // chord). The v0.1042 weld morphed only the WAVE height, so
            // the BASE surface still bulged above the coarse chord by
            // exactly that much at every LOD border - a real crack,
            // widening with the square of tile size (hence "the slightly
            // larger tiles"), and at a grazing dusk view a centimetre of
            // vertical gap smears across many pixels. Drop the vert onto
            // the chord as it morphs. Applies with NO wave fade gate: far
            // patches are exact spheres, and exact spheres still crack.
            var disp = -weld_w * (dl * dl) / (2.0 * r);
            if (material.params.x < 0.5) {
                // Distance fade (v0.878.2): waves are invisible beyond a
                // few km, so fading displacement to zero keeps the far
                // field a clean sphere. 2..8 km band; inside 2 km, full.
                let fade = 1.0 - smoothstep(2000.0, 8000.0, cam_dist);
                if (fade > 0.001) {
                    // Shoal damping (v0.957): the packed UV carries the
                    // baked water depth (low 16 bits = decimetres), so the
                    // taller chop dies toward the waterline instead of
                    // stabbing through beach terrain. CPU twin:
                    // ocean_waves::shoal_factor (drawn == sampled).
                    let depth_m = f32(u32(round(max(vertex.uv.x, 0.0))) & 65535u) / 10.0;
                    let shoal = smoothstep(0.4, 7.0, depth_m);
                    // Camera-anchored small-domain positions (v0.1017
                    // jitter fix + v0.1040 cascade B): camera-to-vertex
                    // delta in the planet-local frame plus each cascade's
                    // own modulus anchor - small magnitudes only.
                    let anchw = vec3<f32>(
                        camera.light0_cone_inner.y,
                        camera.light0_cone_inner.z,
                        camera.light0_cone_inner.w,
                    );
                    let dvw = (inv_model
                        * vec4<f32>(world_pos.xyz - camera.view_pos.xyz, 0.0)).xyz;
                    let anch256 = vec3<f32>(
                        camera.light4_cone_inner.x,
                        camera.light4_cone_inner.y,
                        camera.light4_cone_inner.z,
                    );
                    let p_m = dir * r;
                    let p64 = anchw + dvw;
                    let p256 = anch256 + dvw;
                    let t = camera.sun_color.w;
                    let h = water_disp_height(p_m, p64, p256, t, cam_dist, dir);
                    // NO wave-height morph here (v0.1044). Collapsing odd
                    // verts onto their parents' mean is textbook CDLOD,
                    // but this lattice is normalize(barycentric) per patch,
                    // so a child's edge verts are NOT a subset of the
                    // parent's - the morph target is wrong, and visually it
                    // halves the wave resolution over whole patches: the
                    // operator's "weird basic simple blue tiles... most
                    // prominent resting at water level" (v0.1041 + v0.1042,
                    // reproduced in the rig, proven by toggling the morph
                    // off live). The waves are a continuous field sampled
                    // at different rates, so their cross-LOD mismatch is
                    // small; the CHORD SAG above is the real crack.
                    disp = disp + h * fade * shoal;
                }
            }
            world_pos = vec4<f32>(world_pos.xyz + radial * disp, 1.0);
        }
    }
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    // Water meshes carry the geomorph delta in the normal slot (v0.1041);
    // their geometric normal is the radial, which the water FS re-derives
    // from position anyway - this keeps the interpolated value sane.
    if (material.params.z >= 15.5 && material.params.z < 16.5) {
        out.world_normal = normalize(world_pos.xyz - material.base_color.xyz);
    } else {
        out.world_normal = normalize((obj_normal_matrix() * vec4<f32>(vertex.normal, 0.0)).xyz);
    }
    out.uv = vertex.uv;
    out.pack = vertex.uv;
    out.inst_data = vertex.inst_pos_fade;
    return out;
}

