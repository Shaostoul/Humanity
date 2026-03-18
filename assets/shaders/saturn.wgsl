// saturn.wgsl — Procedural Saturn gas giant with subtle pale yellow bands,
// hexagonal north pole storm, white spots, and warm atmospheric effects.
// Note: Ring rendering is handled separately by the engine's ring geometry.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Saturn parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct SaturnUniforms {
    model: mat4x4<f32>,
    time: f32,
    band_contrast: f32,       // Band darkness (default ~0.3)
    hex_storm_intensity: f32, // North pole hexagonal storm strength (default ~0.4)
    atmosphere_density: f32,  // Atmospheric scattering (default ~0.15)
    sun_direction: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> saturn: SaturnUniforms;

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
    let world_pos = saturn.model * vec4<f32>(in.position, 1.0);
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

fn saturn_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.9, 0.8, 0.6);
    let c2 = vec3<f32>(1.0, 0.9, 0.7);
    let c3 = vec3<f32>(1.0, 0.95, 0.8);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(saturn.sun_direction);
    let time = saturn.time;

    let latitude = abs(local_pos.y);
    let band1 = smoothstep(0.0, 0.25, latitude) * smoothstep(0.5, 0.25, latitude);
    let band2 = smoothstep(0.4, 0.65, latitude) * smoothstep(0.9, 0.65, latitude);
    let bands = band1 + band2;

    let base_noise = noise3(local_pos * 18.0 + time * 0.0008);
    let detail_noise = noise3(local_pos * 90.0 + time * 0.002);
    let atmosphere = mix(base_noise, detail_noise, 0.3);

    // Hexagonal north pole storm
    let north_pole_dist = distance(local_pos, vec3<f32>(0.0, 1.0, 0.0));
    let hex_storm = smoothstep(0.3, 0.0, north_pole_dist);
    let hex_noise = noise3(local_pos * 25.0 + time * 0.004);
    let hex_final = hex_storm * (0.6 + 0.4 * hex_noise);

    // White spots
    let ws1 = smoothstep(0.1, 0.0, distance(local_pos, vec3<f32>(0.6, 0.4, 0.3)));
    let ws2 = smoothstep(0.08, 0.0, distance(local_pos, vec3<f32>(-0.4, 0.7, 0.5)));
    let ws3 = smoothstep(0.12, 0.0, distance(local_pos, vec3<f32>(0.2, -0.8, 0.4)));
    let white_spots = ws1 + ws2 + ws3;

    // Small features
    var small_features = 0.0;
    for (var i = 0; i < 20; i++) {
        let angle = f32(i) * 0.314159;
        let radius = 0.5 + 0.5 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.8 * sin(f32(i) * 0.7);
        let fp = vec3<f32>(x, y, z);
        let fs = 0.025 + 0.015 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let fi = 0.2 + 0.3 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_features += smoothstep(fs, 0.0, distance(local_pos, fp)) * fi;
    }

    let turb1 = noise3(local_pos * 35.0 + time * 0.006);
    let turb2 = noise3(local_pos * 70.0 - time * 0.009);
    let turbulence = (turb1 * 0.6 + turb2 * 0.4) * 0.15;

    let intensity = smoothstep(0.2, 0.8, atmosphere + turbulence + bands * 0.2);
    let base_color = saturn_color(intensity);

    let band_color = vec3<f32>(0.8, 0.7, 0.5);
    var color = mix(base_color, band_color, bands * saturn.band_contrast);

    let hex_color = vec3<f32>(0.7, 0.6, 0.4);
    color = mix(color, hex_color, hex_final * saturn.hex_storm_intensity);

    let white_spot_color = vec3<f32>(0.98, 0.95, 0.9);
    color = mix(color, white_spot_color, white_spots * 0.6);

    let feature_color = vec3<f32>(0.85, 0.75, 0.55);
    color = mix(color, feature_color, small_features * 0.2);

    // Strong limb darkening
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 3.0) * 0.6 + 0.4;
    color *= limb;

    let limb_glow = smoothstep(0.0, 0.3, 1.0 - cos_angle);
    let glow_color = vec3<f32>(0.9, 0.8, 0.6);
    color = mix(color, glow_color, limb_glow * 0.3);

    let sun_angle = dot(local_pos, sun_direction);
    let atmospheric_scattering = smoothstep(-1.0, 1.0, sun_angle);
    let atmosphere_color = vec3<f32>(1.0, 0.9, 0.7);
    color = mix(color, atmosphere_color, atmospheric_scattering * saturn.atmosphere_density);

    return vec4<f32>(color, 1.0);
}
