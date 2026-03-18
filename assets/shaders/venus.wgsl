// venus.wgsl — Procedural Venus surface with thick sulfuric acid cloud layer.
// Features volcanic plains, animated cloud bands, strong limb darkening from
// the dense CO2 atmosphere, and yellowish atmospheric scattering.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Venus parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VenusUniforms {
    model: mat4x4<f32>,
    time: f32,
    cloud_opacity: f32,       // How opaque the cloud layer is (default ~0.6)
    atmosphere_density: f32,  // Atmospheric scattering strength (default ~0.3)
    volcanic_intensity: f32,  // How pronounced volcanic features are (default ~0.4)
    sun_direction: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> venus: VenusUniforms;

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
    let world_pos = venus.model * vec4<f32>(in.position, 1.0);
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

fn venus_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.8, 0.6, 0.4);
    let c2 = vec3<f32>(0.9, 0.7, 0.5);
    let c3 = vec3<f32>(1.0, 0.8, 0.6);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(venus.sun_direction);
    let time = venus.time;

    let base_noise = noise3(local_pos * 30.0);
    let detail_noise = noise3(local_pos * 150.0);
    let surface = mix(base_noise, detail_noise, 0.4);

    // Volcanic features
    let v1 = volcano(local_pos, vec3<f32>(0.7, 0.3, 0.4), 0.4, 0.5);
    let v2 = volcano(local_pos, vec3<f32>(-0.5, 0.8, -0.3), 0.35, 0.45);
    let v3 = volcano(local_pos, vec3<f32>(0.2, -0.7, 0.6), 0.3, 0.4);
    let mv1 = volcano(local_pos, vec3<f32>(0.4, 0.5, 0.2), 0.2, 0.3);
    let mv2 = volcano(local_pos, vec3<f32>(-0.3, 0.3, 0.8), 0.18, 0.25);
    let mv3 = volcano(local_pos, vec3<f32>(0.1, -0.4, -0.7), 0.22, 0.28);

    var small_volcanoes = 0.0;
    for (var i = 0; i < 12; i++) {
        let angle = f32(i) * 0.523599;
        let radius = 0.7 + 0.3 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.6 * sin(f32(i) * 1.2);
        let vp = vec3<f32>(x, y, z);
        let vs = 0.08 + 0.05 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let vh = 0.2 + 0.15 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_volcanoes += volcano(local_pos, vp, vs, vh);
    }

    let all_volcanoes = v1 + v2 + v3 + mv1 + mv2 + mv3 + small_volcanoes;
    let roughness = noise3(local_pos * 80.0) * 0.15;
    let sun_angle = dot(local_pos, sun_direction);
    let atmospheric_scattering = smoothstep(-1.0, 1.0, sun_angle);

    // Cloud layer
    let cn1 = noise3(local_pos * 20.0 + time * 0.01);
    let cn2 = noise3(local_pos * 40.0 - time * 0.015);
    let cn3 = noise3(local_pos * 80.0 + time * 0.02);
    let cloud_mask = smoothstep(0.4, 0.7, cn1 * 0.5 + cn2 * 0.3 + cn3 * 0.2);

    let surface_height = surface + roughness + all_volcanoes;
    let intensity = smoothstep(0.1, 0.9, surface_height);
    let base_color = venus_color(intensity);

    let volcanic_color = vec3<f32>(0.7, 0.4, 0.2);
    var color = mix(base_color, volcanic_color, all_volcanoes * venus.volcanic_intensity);

    let atmosphere_color = vec3<f32>(1.0, 0.9, 0.7);
    color = mix(color, atmosphere_color, atmospheric_scattering * venus.atmosphere_density);

    let cloud_color = vec3<f32>(1.0, 0.95, 0.8);
    color = mix(color, cloud_color, cloud_mask * venus.cloud_opacity);

    // Strong limb darkening from thick atmosphere
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 2.5) * 0.7 + 0.3;
    color *= limb;

    let limb_glow = smoothstep(0.0, 0.3, 1.0 - cos_angle);
    let glow_color = vec3<f32>(1.0, 0.8, 0.6);
    color = mix(color, glow_color, limb_glow * 0.2);

    return vec4<f32>(color, 1.0);
}
