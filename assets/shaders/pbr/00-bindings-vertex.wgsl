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
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
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
const OCEAN_W4_LAMBDA: f32 = 50.0;
const OCEAN_W4_CPS: f32 = 0.18;
const OCEAN_W4_HEIGHT: f32 = 0.45;
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
const OCEAN_W5_LAMBDA: f32 = 18.0;
const OCEAN_W5_CPS: f32 = 0.30;
const OCEAN_W5_HEIGHT: f32 = 0.35;
const OCEAN_W6_LAMBDA: f32 = 6.0;
const OCEAN_W6_CPS: f32 = 0.52;
const OCEAN_W6_HEIGHT: f32 = 0.1;

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
fn ocean_wave_height(p_m: vec3<f32>, t: f32, cam_dist: f32) -> f32 {
    var h = ocean_height_train(p_m, WAVE1_DIR, OCEAN_W1_LAMBDA, OCEAN_W1_CPS, OCEAN_W1_HEIGHT, t);
    h = h + ocean_height_train(p_m, WAVE3_DIR, OCEAN_W2_LAMBDA, OCEAN_W2_CPS, OCEAN_W2_HEIGHT, t);
    h = h + ocean_height_train(p_m, WAVE4_DIR, OCEAN_W3_LAMBDA, OCEAN_W3_CPS, OCEAN_W3_HEIGHT, t);
    h = h + ocean_height_train(p_m, WAVE6_DIR, OCEAN_W4_LAMBDA, OCEAN_W4_CPS, OCEAN_W4_HEIGHT, t);
    // Short chop only near the camera (sub-vertex beyond ~800 m; the CPU
    // float twin runs at the player, where this fade is ~1).
    let near = 1.0 - smoothstep(250.0, 800.0, cam_dist);
    if (near > 0.001) {
        var s = ocean_height_train(p_m, WAVE2_DIR, OCEAN_W5_LAMBDA, OCEAN_W5_CPS, OCEAN_W5_HEIGHT, t);
        s = s + ocean_height_train(p_m, WAVE5_DIR, OCEAN_W6_LAMBDA, OCEAN_W6_CPS, OCEAN_W6_HEIGHT, t);
        h = h + s * near;
    }
    return h;
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var world_pos = object.model * vec4<f32>(vertex.position, 1.0);
    // The model matrix's w ROW carries per-object metadata (model[0].w =
    // LOD crossfade, v0.920), so rebuild the homogeneous w explicitly. For
    // an ordinary TRS matrix this is a no-op; with metadata present it is
    // what keeps clip_position correct. xyz is untouched by the w row.
    world_pos = vec4<f32>(world_pos.xyz, 1.0);
    // Water shell (type 16): displace the vertex radially by the analytic
    // wave height, computed in the planet-local frame via the same
    // center + inverse-rotation trick the planet fragment branch uses
    // (material.base_color.xyz = planet center in render space;
    // transpose(normal_matrix) = model^-1). Skirt vertices displace with
    // their parent edge (same dir), so LOD seams stay sealed.
    if (material.params.z >= 15.5 && material.params.z < 16.5) {
        let inv_model = transpose(object.normal_matrix);
        let dir_world = world_pos.xyz - material.base_color.xyz;
        let r = length(dir_world);
        if (r > 1.0) {
            let radial = dir_world / r;
            let dir = normalize((inv_model * vec4<f32>(dir_world, 0.0)).xyz);
            // Distance fade (v0.878.2): waves are invisible beyond a few km
            // anyway, and fading the displacement to ZERO makes every far
            // patch an EXACT sphere - so patches of different LODs share
            // bit-matching borders with no skirts (see the water builder
            // comment). 2..8 km band; inside 2 km, full height.
            let cam_dist = length(camera.view_pos.xyz - world_pos.xyz);
            let fade = 1.0 - smoothstep(2000.0, 8000.0, cam_dist);
            if (fade > 0.001) {
                // Shoal damping (v0.957): the packed UV carries the baked
                // water depth (see build_water_patch_mesh - low 16 bits =
                // decimetres), so the taller chop dies smoothly toward the
                // waterline instead of stabbing through beach terrain.
                // CPU twin: ocean_waves::shoal_factor (drawn == sampled).
                let depth_m = f32(u32(round(max(vertex.uv.x, 0.0))) & 65535u) / 10.0;
                let shoal = smoothstep(0.4, 7.0, depth_m);
                let h = ocean_wave_height(dir * r, camera.sun_color.w, cam_dist) * fade * shoal;
                world_pos = vec4<f32>(world_pos.xyz + radial * h, 1.0);
            }
        }
    }
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((object.normal_matrix * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.uv = vertex.uv;
    return out;
}

