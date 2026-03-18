// neptune.wgsl — Procedural Neptune ice giant with deep blue atmosphere,
// Great Dark Spot, methane ice clouds, and the solar system's strongest winds.
// More active and stormy than Uranus despite similar composition.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Neptune parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct NeptuneUniforms {
    model: mat4x4<f32>,
    time: f32,
    dark_spot_intensity: f32, // Great Dark Spot darkness (default ~0.7)
    cloud_brightness: f32,    // Methane ice cloud brightness (default ~0.7)
    atmosphere_density: f32,  // Atmospheric scattering (default ~0.2)
    sun_direction: vec3<f32>,
    wind_speed: f32,          // Turbulence animation speed multiplier (default ~1.0)
};

@group(1) @binding(0)
var<uniform> neptune: NeptuneUniforms;

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
    let world_pos = neptune.model * vec4<f32>(in.position, 1.0);
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

fn neptune_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.2, 0.4, 0.8);
    let c2 = vec3<f32>(0.3, 0.5, 0.9);
    let c3 = vec3<f32>(0.4, 0.6, 1.0);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(neptune.sun_direction);
    let time = neptune.time;

    let latitude = abs(local_pos.y);
    let band1 = smoothstep(0.0, 0.25, latitude) * smoothstep(0.5, 0.25, latitude);
    let band2 = smoothstep(0.4, 0.65, latitude) * smoothstep(0.9, 0.65, latitude);
    let bands = band1 + band2;

    let base_noise = noise3(local_pos * 16.0 + time * 0.001);
    let detail_noise = noise3(local_pos * 80.0 + time * 0.002);
    let atmosphere = mix(base_noise, detail_noise, 0.3);

    // Great Dark Spot
    let dark_spot_center = vec3<f32>(-0.6, 0.3, 0.4);
    let dark_spot = smoothstep(0.35, 0.0, distance(local_pos, dark_spot_center));
    let dark_spot_noise = noise3(local_pos * 28.0 + time * 0.006);
    let dark_spot_final = dark_spot * (0.7 + 0.3 * dark_spot_noise);

    // Small Dark Spot
    let small_dark = smoothstep(0.2, 0.0, distance(local_pos, vec3<f32>(0.5, -0.6, 0.3)));
    let small_dark_noise = noise3(local_pos * 32.0 + time * 0.008);
    let small_dark_final = small_dark * (0.6 + 0.4 * small_dark_noise);

    // White methane ice clouds
    let wc1 = smoothstep(0.08, 0.0, distance(local_pos, vec3<f32>(0.7, 0.5, 0.2)));
    let wc2 = smoothstep(0.06, 0.0, distance(local_pos, vec3<f32>(-0.3, 0.8, 0.4)));
    let wc3 = smoothstep(0.1, 0.0, distance(local_pos, vec3<f32>(0.2, -0.7, 0.6)));
    let white_clouds = wc1 + wc2 + wc3;

    // Small features
    var small_features = 0.0;
    for (var i = 0; i < 18; i++) {
        let angle = f32(i) * 0.349066;
        let radius = 0.5 + 0.5 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.8 * sin(f32(i) * 0.8);
        let fp = vec3<f32>(x, y, z);
        let fs = 0.025 + 0.02 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let fi = 0.2 + 0.3 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_features += smoothstep(fs, 0.0, distance(local_pos, fp)) * fi;
    }

    // Strong winds = more turbulence
    let turb1 = noise3(local_pos * 45.0 + time * 0.01 * neptune.wind_speed);
    let turb2 = noise3(local_pos * 90.0 - time * 0.015 * neptune.wind_speed);
    let turbulence = (turb1 * 0.6 + turb2 * 0.4) * 0.25;

    let intensity = smoothstep(0.2, 0.8, atmosphere + turbulence + bands * 0.2);
    let base_color = neptune_color(intensity);

    let band_color = vec3<f32>(0.1, 0.3, 0.7);
    var color = mix(base_color, band_color, bands * 0.4);

    let dark_spot_color = vec3<f32>(0.05, 0.15, 0.4);
    color = mix(color, dark_spot_color, dark_spot_final * neptune.dark_spot_intensity);
    color = mix(color, dark_spot_color, small_dark_final * neptune.dark_spot_intensity * 0.86);

    let white_cloud_color = vec3<f32>(0.95, 0.95, 1.0);
    color = mix(color, white_cloud_color, white_clouds * neptune.cloud_brightness);

    let feature_color = vec3<f32>(0.2, 0.4, 0.8);
    color = mix(color, feature_color, small_features * 0.2);

    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 3.0) * 0.6 + 0.4;
    color *= limb;

    let limb_glow = smoothstep(0.0, 0.3, 1.0 - cos_angle);
    let glow_color = vec3<f32>(0.2, 0.4, 0.8);
    color = mix(color, glow_color, limb_glow * 0.3);

    let sun_angle = dot(local_pos, sun_direction);
    let atmospheric_scattering = smoothstep(-1.0, 1.0, sun_angle);
    let atmosphere_color = vec3<f32>(0.3, 0.5, 0.9);
    color = mix(color, atmosphere_color, atmospheric_scattering * neptune.atmosphere_density);

    return vec4<f32>(color, 1.0);
}
