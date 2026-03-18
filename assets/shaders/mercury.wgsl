// mercury.wgsl — Procedural Mercury surface with craters and temperature variation.
// Dense cratered surface (large, medium, and small craters), no atmosphere,
// extreme day/night temperature color shifts based on sun angle.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Mercury parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct MercuryUniforms {
    model: mat4x4<f32>,
    time: f32,
    crater_depth: f32,       // Multiplier for crater darkness (default ~0.5)
    temperature_blend: f32,  // How strongly temperature affects color (default ~0.3)
    _padding: f32,
    sun_direction: vec3<f32>,
    surface_detail: f32,     // Detail noise frequency multiplier (default ~200.0)
};

@group(1) @binding(0)
var<uniform> mercury: MercuryUniforms;

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
    let world_pos = mercury.model * vec4<f32>(in.position, 1.0);
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

fn crater(p: vec3<f32>, center: vec3<f32>, radius: f32, depth: f32) -> f32 {
    let dist = distance(p, center);
    return smoothstep(radius, 0.0, dist) * depth;
}

fn mercury_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.3, 0.3, 0.3);
    let c2 = vec3<f32>(0.5, 0.5, 0.5);
    let c3 = vec3<f32>(0.7, 0.7, 0.7);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(mercury.sun_direction);

    let base_noise = noise3(local_pos * 50.0);
    let detail_noise = noise3(local_pos * mercury.surface_detail);
    let surface = mix(base_noise, detail_noise, 0.3);

    // Craters at multiple scales
    let crater1 = crater(local_pos, vec3<f32>(0.8, 0.2, 0.3), 0.3, 0.4);
    let crater2 = crater(local_pos, vec3<f32>(-0.6, 0.7, -0.2), 0.25, 0.35);
    let crater3 = crater(local_pos, vec3<f32>(0.3, -0.8, 0.5), 0.2, 0.3);
    let crater4 = crater(local_pos, vec3<f32>(-0.4, -0.3, -0.8), 0.35, 0.45);
    let med_crater1 = crater(local_pos, vec3<f32>(0.5, 0.6, 0.1), 0.15, 0.25);
    let med_crater2 = crater(local_pos, vec3<f32>(-0.2, 0.4, 0.7), 0.12, 0.2);
    let med_crater3 = crater(local_pos, vec3<f32>(0.1, -0.5, -0.6), 0.18, 0.28);

    var small_craters = 0.0;
    for (var i = 0; i < 8; i++) {
        let angle = f32(i) * 0.785398;
        let radius = 0.8 + 0.2 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.5 * sin(f32(i) * 1.5);
        let crater_pos = vec3<f32>(x, y, z);
        let crater_size = 0.05 + 0.03 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let crater_depth_val = 0.15 + 0.1 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_craters += crater(local_pos, crater_pos, crater_size, crater_depth_val);
    }

    let all_craters = (crater1 + crater2 + crater3 + crater4 + med_crater1 + med_crater2 + med_crater3 + small_craters) * mercury.crater_depth;
    let roughness = noise3(local_pos * 100.0) * 0.1;

    // Temperature: day side warm tint, night side cool tint
    let sun_angle = dot(local_pos, sun_direction);
    let temperature = smoothstep(-1.0, 1.0, sun_angle);

    let surface_height = surface + roughness - all_craters;
    let intensity = smoothstep(0.2, 0.8, surface_height);
    let base_color = mercury_color(intensity);
    let hot_color = vec3<f32>(0.6, 0.4, 0.2);
    let cold_color = vec3<f32>(0.2, 0.2, 0.3);

    var color = mix(base_color, hot_color, temperature * mercury.temperature_blend);
    color = mix(color, cold_color, (1.0 - temperature) * mercury.temperature_blend * 0.67);

    let crater_color = vec3<f32>(0.15, 0.15, 0.15);
    color = mix(color, crater_color, all_craters * 0.5);

    // Limb darkening (no atmosphere)
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 1.5) * 0.9 + 0.1;
    color *= limb;

    return vec4<f32>(color, 1.0);
}
