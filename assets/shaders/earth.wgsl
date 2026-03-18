// earth.wgsl — Procedural Earth surface with continents, oceans, biomes, and atmosphere.
// Generates fractal continents with latitude-based biomes (forest, desert, ice caps),
// ocean depth variation, atmospheric scattering, and limb glow.
// All parameters are exposed as uniforms for runtime tuning.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Earth parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct EarthUniforms {
    model: mat4x4<f32>,

    time: f32,
    sea_level: f32,
    mountain_height: f32,
    desert_coverage: f32,

    sun_direction: vec3<f32>,
    surface_roughness: f32,

    ocean_color: vec3<f32>,
    atmosphere_density: f32,

    ice_color: vec3<f32>,
    ice_coverage: f32,

    land_color: vec3<f32>,
    continent_scale: f32,

    atmosphere_color: vec3<f32>,
    detail_scale: f32,

    noise_amplitude: f32,
    debug_mode: f32,     // 0=normal, 1=red debug, 2=elevation, 3=sun direction
    _padding1: f32,
    _padding2: f32,
};

@group(1) @binding(0)
var<uniform> earth: EarthUniforms;

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
    let world_pos = earth.model * vec4<f32>(in.position, 1.0);
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
    let sun_direction = normalize(earth.sun_direction);

    // Fractal continents
    let base_fbm = fbm(local_pos * earth.continent_scale) * earth.noise_amplitude;
    let continent_fbm = fbm(local_pos * earth.detail_scale) * (earth.noise_amplitude * 0.8);
    let elevation = base_fbm * 0.7 + continent_fbm * 0.3;

    // Debug modes
    if (earth.debug_mode == 1.0) {
        return vec4<f32>(vec3<f32>(1.0, 0.0, 0.0), 1.0);
    } else if (earth.debug_mode == 2.0) {
        return vec4<f32>(vec3<f32>(elevation), 1.0);
    } else if (earth.debug_mode == 3.0) {
        return vec4<f32>(abs(sun_direction), 1.0);
    }

    // Biome masks
    let ice_lat = abs(local_pos.y);
    let ice = smoothstep(0.7, 0.9, ice_lat) * smoothstep(0.3, 0.7, elevation) * earth.ice_coverage;
    let desert = smoothstep(0.15, 0.35, elevation) * smoothstep(0.1, 0.5, abs(local_pos.y)) * earth.desert_coverage;
    let forest = smoothstep(0.3, 0.7, elevation) * (1.0 - ice) * (1.0 - desert);
    let mountain = pow(max(elevation - 0.7, 0.0), 2.0) * 2.5 * earth.mountain_height;
    let ocean = 1.0 - smoothstep(earth.sea_level, earth.sea_level + 0.04, elevation);

    // Biome colors
    let desert_color = vec3<f32>(1.0, 0.93, 0.45);
    let forest_color = earth.land_color;
    let grass_color = mix(earth.land_color, vec3<f32>(0.2, 0.8, 0.2), 0.5);
    let mountain_color = vec3<f32>(0.7, 0.7, 0.7);
    let ocean_shallow = mix(earth.ocean_color, vec3<f32>(0.15, 0.55, 1.0), 0.2);
    let ocean_deep = mix(earth.ocean_color, vec3<f32>(0.01, 0.08, 0.3), 0.7);

    // Biome blending
    var biome_color: vec3<f32>;
    if (elevation < earth.sea_level) {
        let t = clamp(elevation / earth.sea_level, 0.0, 1.0);
        biome_color = mix(ocean_deep, ocean_shallow, t);
        let wave_noise = noise3(local_pos * 80.0);
        biome_color += vec3<f32>(0.1, 0.1, 0.2) * wave_noise * 0.1;
    } else {
        let t = (elevation - earth.sea_level) / (1.0 - earth.sea_level);
        let grass_amt = smoothstep(0.0, 0.4, t) * (1.0 - ice) * (1.0 - desert);
        let forest_amt = smoothstep(0.3, 0.7, t) * (1.0 - ice) * (1.0 - desert);
        let desert_amt = desert;
        let ice_amt = ice;
        let mountain_amt = mountain;
        biome_color = grass_color * grass_amt + forest_color * forest_amt + desert_color * desert_amt + earth.ice_color * ice_amt + mountain_color * mountain_amt;
        let total = grass_amt + forest_amt + desert_amt + ice_amt + mountain_amt;
        if (total > 0.0) {
            biome_color /= total;
        }
        biome_color *= 0.95 + 0.1 * noise3(local_pos * 50.0);
    }

    var color = biome_color;

    // Surface roughness
    let roughness = noise3(local_pos * 60.0) * 0.12 * earth.surface_roughness;
    let surface_height = elevation + roughness;
    let is_land = smoothstep(0.1, 0.3, surface_height);

    // Atmospheric scattering
    let atmospheric_scattering = smoothstep(-1.0, 1.0, dot(local_pos, sun_direction)) * earth.atmosphere_density;
    color = mix(color, earth.atmosphere_color, atmospheric_scattering * 0.08);

    // Limb darkening
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 2.0) * 0.8 + 0.2;
    color *= limb;

    // Atmospheric glow at limb
    let limb_glow = smoothstep(0.0, 0.4, 1.0 - cos_angle);
    let glow_color = vec3<f32>(0.3, 0.5, 0.8);
    color = mix(color, glow_color, limb_glow * 0.08);

    // Diffuse lighting
    let diffuse = clamp(max(dot(normal, sun_direction), 0.0), 0.0, 0.8);
    color *= diffuse;

    return vec4<f32>(color, 1.0);
}
