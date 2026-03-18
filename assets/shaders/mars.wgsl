// mars.wgsl — Procedural Mars surface with volcanoes, craters, dust storms, and ice caps.
// Features Olympus Mons-scale volcanic features, animated dust storms, polar CO2 ice,
// and thin reddish atmospheric scattering from iron oxide dust.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Mars parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct MarsUniforms {
    model: mat4x4<f32>,
    time: f32,
    dust_storm_intensity: f32, // 0.0-1.0, how active dust storms are (default ~0.4)
    polar_ice_extent: f32,     // Latitude threshold for ice caps (default ~0.8)
    atmosphere_density: f32,   // Thin atmosphere scattering (default ~0.15)
    sun_direction: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> mars: MarsUniforms;

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
    let world_pos = mars.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.normal = in.normal;
    out.local_position = in.position;
    out.tangent = in.tangent;
    out.bitangent = in.bitangent;
    out.tex_coords = in.tex_coords;
    out.clip_position = camera.view_proj * world_pos;
    return out;
}

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

fn volcano(p: vec3<f32>, center: vec3<f32>, radius: f32, height: f32) -> f32 {
    return smoothstep(radius, 0.0, distance(p, center)) * height;
}

fn crater(p: vec3<f32>, center: vec3<f32>, radius: f32, depth: f32) -> f32 {
    return smoothstep(radius, 0.0, distance(p, center)) * depth;
}

fn mars_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.6, 0.2, 0.1);
    let c2 = vec3<f32>(0.8, 0.4, 0.2);
    let c3 = vec3<f32>(1.0, 0.6, 0.3);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(mars.sun_direction);
    let time = mars.time;

    let base_noise = noise3(local_pos * 35.0);
    let detail_noise = noise3(local_pos * 180.0);
    let surface = mix(base_noise, detail_noise, 0.4);

    // Volcanoes and craters
    let v1 = volcano(local_pos, vec3<f32>(0.8, 0.1, 0.4), 0.6, 0.8);
    let v2 = volcano(local_pos, vec3<f32>(-0.7, 0.6, -0.3), 0.5, 0.7);
    let v3 = volcano(local_pos, vec3<f32>(0.4, -0.8, 0.5), 0.45, 0.65);
    let mv1 = volcano(local_pos, vec3<f32>(0.3, 0.7, 0.2), 0.25, 0.4);
    let mv2 = volcano(local_pos, vec3<f32>(-0.4, 0.2, 0.8), 0.22, 0.35);
    let mv3 = volcano(local_pos, vec3<f32>(0.2, -0.3, -0.9), 0.28, 0.42);

    let c1 = crater(local_pos, vec3<f32>(0.6, 0.4, 0.1), 0.2, 0.3);
    let c2 = crater(local_pos, vec3<f32>(-0.3, 0.8, 0.4), 0.18, 0.25);
    let c3 = crater(local_pos, vec3<f32>(0.1, -0.6, -0.7), 0.22, 0.28);

    var small_craters = 0.0;
    for (var i = 0; i < 20; i++) {
        let angle = f32(i) * 0.314159;
        let radius = 0.5 + 0.5 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.8 * sin(f32(i) * 0.9);
        let cp = vec3<f32>(x, y, z);
        let cs = 0.04 + 0.03 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let cd = 0.12 + 0.08 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_craters += crater(local_pos, cp, cs, cd);
    }

    let all_volcanoes = v1 + v2 + v3 + mv1 + mv2 + mv3;
    let all_craters = c1 + c2 + c3 + small_craters;
    let roughness = noise3(local_pos * 90.0) * 0.15;

    // Polar ice caps
    let north_pole = smoothstep(mars.polar_ice_extent, 1.0, local_pos.y);
    let south_pole = smoothstep(mars.polar_ice_extent, 1.0, -local_pos.y);
    let polar_ice = max(north_pole, south_pole);

    // Animated dust storms
    let dn1 = noise3(local_pos * 10.0 + time * 0.01);
    let dn2 = noise3(local_pos * 20.0 - time * 0.015);
    let dn3 = noise3(local_pos * 40.0 + time * 0.02);
    let dust_mask = smoothstep(0.6, 0.9, dn1 * 0.5 + dn2 * 0.3 + dn3 * 0.2);

    let surface_height = surface + roughness + all_volcanoes - all_craters;
    let intensity = smoothstep(0.1, 0.9, surface_height);
    let base_color = mars_color(intensity);

    let volcanic_color = vec3<f32>(0.5, 0.2, 0.1);
    var color = mix(base_color, volcanic_color, all_volcanoes * 0.3);

    let crater_color = vec3<f32>(0.4, 0.15, 0.05);
    color = mix(color, crater_color, all_craters * 0.4);

    let ice_color = vec3<f32>(0.9, 0.95, 1.0);
    color = mix(color, ice_color, polar_ice * 0.8);

    let dust_color = vec3<f32>(0.7, 0.3, 0.1);
    color = mix(color, dust_color, dust_mask * mars.dust_storm_intensity);

    let sun_angle = dot(local_pos, sun_direction);
    let atmospheric_scattering = smoothstep(-1.0, 1.0, sun_angle);
    let atmosphere_color = vec3<f32>(0.8, 0.4, 0.2);
    color = mix(color, atmosphere_color, atmospheric_scattering * mars.atmosphere_density);

    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 1.8) * 0.85 + 0.15;
    color *= limb;

    let limb_glow = smoothstep(0.0, 0.5, 1.0 - cos_angle);
    let glow_color = vec3<f32>(0.6, 0.3, 0.1);
    color = mix(color, glow_color, limb_glow * 0.25);

    return vec4<f32>(color, 1.0);
}
