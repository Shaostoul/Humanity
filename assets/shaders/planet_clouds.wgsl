// planet_clouds.wgsl — Animated cloud layer rendered as a semi-transparent sphere overlay.
// Uses fractal Brownian motion noise to generate swirling cloud patterns with
// configurable coverage, density, speed, scale, and color.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + cloud parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct CloudUniforms {
    model: mat4x4<f32>,

    time: f32,
    cloud_coverage: f32,     // 0.0-1.0, percentage of sky covered
    cloud_density: f32,      // 0.0-1.0, cloud thickness / alpha
    cloud_speed: f32,        // Animation speed multiplier

    sun_direction: vec3<f32>,
    cloud_scale: f32,        // Cloud pattern scale

    cloud_color: vec3<f32>,  // Main cloud color (e.g., white for Earth)
    _padding1: f32,

    cloud_shadow: vec3<f32>, // Shadow/depth color for clouds
    _padding2: f32,

    atmosphere_tint: vec3<f32>, // Atmospheric scattering tint color
    _padding3: f32,
};

@group(1) @binding(0)
var<uniform> clouds: CloudUniforms;

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
    let world_pos = clouds.model * vec4<f32>(in.position, 1.0);
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
    for (var i = 0; i < 4; i++) {
        f += noise3(p * freq) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    return f;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(clouds.sun_direction);

    // Generate animated cloud patterns
    let cloud_base = fbm(local_pos * clouds.cloud_scale + clouds.time * clouds.cloud_speed * 0.01);
    let cloud_detail = fbm(local_pos * clouds.cloud_scale * 2.0 - clouds.time * clouds.cloud_speed * 0.015);
    let cloud_mask = smoothstep(0.5, 0.7, cloud_base + 0.3 * cloud_detail);

    // Apply cloud coverage
    let coverage_mask = smoothstep(0.0, clouds.cloud_coverage, cloud_mask);
    let cloud_alpha = coverage_mask * clouds.cloud_density;

    // Cloud color with depth variation
    let depth_noise = fbm(local_pos * clouds.cloud_scale * 3.0);
    let cloud_color = mix(clouds.cloud_color, clouds.cloud_shadow, depth_noise * 0.5);

    // Lighting
    let normal = normalize(in.normal);
    let diffuse = max(dot(normal, sun_direction), 0.0);
    let final_color = cloud_color * (0.3 + 0.7 * diffuse);

    // Atmospheric scattering tint
    let atmospheric_scattering = smoothstep(-1.0, 1.0, dot(local_pos, sun_direction));
    let scattered_color = mix(final_color, clouds.atmosphere_tint, atmospheric_scattering * 0.2);

    return vec4<f32>(scattered_color, cloud_alpha);
}
