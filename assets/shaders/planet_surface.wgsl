// planet_surface.wgsl — Generic configurable planet surface shader.
// Generates procedural terrain with oceans, biomes, ice caps, mountains, and atmospheric
// effects. All visual properties are controlled through uniforms for maximum reuse.
// Suitable for rocky planets, ocean worlds, desert worlds, and ice worlds.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + planet parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct PlanetUniforms {
    model: mat4x4<f32>,

    // Time and lighting
    time: f32,
    atmosphere_density: f32,
    surface_roughness: f32,
    planet_type: f32,           // 0=rocky, 1=gas_giant, 2=ice_world, 3=desert, 4=ocean_world

    sun_direction: vec3<f32>,
    sea_level: f32,

    // Color palette
    primary_color: vec3<f32>,
    mountain_height: f32,

    secondary_color: vec3<f32>,
    desert_coverage: f32,

    accent_color: vec3<f32>,
    ice_coverage: f32,

    ocean_color: vec3<f32>,
    continent_scale: f32,

    ice_color: vec3<f32>,
    detail_scale: f32,

    atmosphere_color: vec3<f32>,
    noise_amplitude: f32,
};

@group(1) @binding(0)
var<uniform> planet: PlanetUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) local_position: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
    @location(5) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = planet.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.normal = in.normal;
    out.local_position = in.position;
    out.tangent = in.tangent;
    out.bitangent = in.bitangent;
    out.tex_coords = in.tex_coords;
    out.clip_position = camera.view_proj * world_pos;
    return out;
}

// --- Noise utilities ---

fn hash3(p: vec3<f32>) -> f32 {
    let p3 = fract(p * 0.1031);
    let p4 = p3 + dot(p3, p3.yzx + 19.19);
    return fract((p4.x + p4.y) * p4.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(hash3(i + vec3<f32>(0,0,0)), hash3(i + vec3<f32>(1,0,0)), u.x),
            mix(hash3(i + vec3<f32>(0,1,0)), hash3(i + vec3<f32>(1,1,0)), u.x), u.y),
        mix(
            mix(hash3(i + vec3<f32>(0,0,1)), hash3(i + vec3<f32>(1,0,1)), u.x),
            mix(hash3(i + vec3<f32>(0,1,1)), hash3(i + vec3<f32>(1,1,1)), u.x), u.y), u.z);
}

fn fbm(p: vec3<f32>) -> f32 {
    var f = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 5; i++) {
        f += noise3(p * freq) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    return f;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(planet.sun_direction);

    // Generate elevation using configurable noise
    let base_elevation = fbm(local_pos * planet.continent_scale) * planet.noise_amplitude;
    let detail_elevation = fbm(local_pos * (planet.detail_scale * 2.5)) * 0.35;
    let micro_elevation = fbm(local_pos * (planet.detail_scale * 8.0)) * 0.12;
    let elevation = base_elevation + detail_elevation + micro_elevation;

    // Latitude for climate zones
    let latitude = abs(local_pos.y);

    // Land/ocean threshold
    let sea_level = planet.sea_level;
    let is_ocean = step(elevation, sea_level - 0.01);
    let is_land = 1.0 - is_ocean;

    // Latitude-based biome blending
    let pole = smoothstep(0.7, 1.0, latitude);
    let desert_zone = smoothstep(0.2, 0.5, abs(latitude));
    let forest_zone = smoothstep(0.2, 0.5, 1.0 - abs(latitude));

    // Land biome weights
    let mountain = smoothstep(sea_level + planet.mountain_height * 0.7, sea_level + planet.mountain_height, elevation);
    let snow = pole * is_land * smoothstep(0.7, 1.0, elevation);
    let desert = desert_zone * is_land * (1.0 - snow) * planet.desert_coverage;
    let forest = forest_zone * is_land * (1.0 - snow) * (1.0 - desert);
    let grass = is_land * (1.0 - snow) * (1.0 - desert) * (1.0 - forest) * (1.0 - mountain);
    let beach = is_land * smoothstep(sea_level - 0.01, sea_level + 0.01, elevation);

    // Color selection
    var color: vec3<f32>;
    if (is_ocean > 0.5) {
        let t = smoothstep(sea_level - 0.05, sea_level, elevation);
        let deep_ocean = planet.ocean_color * 0.5;
        color = mix(deep_ocean, planet.ocean_color, t);
    } else {
        let mountain_color = planet.accent_color;
        color = planet.primary_color * grass
              + planet.secondary_color * forest
              + planet.accent_color * desert
              + mountain_color * mountain
              + planet.ice_color * snow * planet.ice_coverage
              + planet.primary_color * beach;
        let total = grass + forest + desert + mountain + snow + beach;
        if (total > 0.0) {
            color /= total;
        }
    }

    // Add subtle noise variation
    color = pow(color, vec3<f32>(0.8));
    color *= 0.95 + 0.1 * noise3(local_pos * 80.0);

    // Atmospheric scattering
    let atmospheric_scattering = smoothstep(-1.0, 1.0, dot(local_pos, sun_direction));
    color = mix(color, planet.atmosphere_color, atmospheric_scattering * planet.atmosphere_density * 0.3);

    // Limb darkening
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 2.0) * 0.8 + 0.2;
    color *= limb;

    // Atmospheric glow at limb
    let limb_glow = smoothstep(0.0, 0.4, 1.0 - cos_angle);
    color = mix(color, planet.atmosphere_color * 0.6, limb_glow * planet.atmosphere_density * 0.4);

    // Diffuse lighting
    let diffuse = max(dot(normal, sun_direction), 0.0);
    color *= diffuse;

    return vec4<f32>(color, 1.0);
}
