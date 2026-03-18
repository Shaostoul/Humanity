// jupiter.wgsl — Procedural Jupiter gas giant with atmospheric bands, Great Red Spot,
// white ovals, small storms, and turbulent atmosphere. No solid surface.
// Strong limb darkening from the thick hydrogen/helium atmosphere.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Jupiter parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct JupiterUniforms {
    model: mat4x4<f32>,
    time: f32,
    band_contrast: f32,       // How dark the band stripes are (default ~0.4)
    red_spot_intensity: f32,  // Great Red Spot color strength (default ~0.6)
    atmosphere_density: f32,  // Atmospheric scattering (default ~0.2)
    sun_direction: vec3<f32>,
    turbulence_scale: f32,    // Turbulence noise frequency (default ~40.0)
};

@group(1) @binding(0)
var<uniform> jupiter: JupiterUniforms;

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
    let world_pos = jupiter.model * vec4<f32>(in.position, 1.0);
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

fn jupiter_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.8, 0.6, 0.4);
    let c2 = vec3<f32>(0.9, 0.7, 0.5);
    let c3 = vec3<f32>(1.0, 0.8, 0.6);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(jupiter.sun_direction);
    let time = jupiter.time;

    let latitude = abs(local_pos.y);

    // Characteristic banded structure
    let band1 = smoothstep(0.0, 0.2, latitude) * smoothstep(0.4, 0.2, latitude);
    let band2 = smoothstep(0.3, 0.5, latitude) * smoothstep(0.7, 0.5, latitude);
    let band3 = smoothstep(0.6, 0.8, latitude) * smoothstep(1.0, 0.8, latitude);
    let bands = band1 + band2 + band3;

    let base_noise = noise3(local_pos * 20.0 + time * 0.001);
    let detail_noise = noise3(local_pos * 100.0 + time * 0.003);
    let atmosphere = mix(base_noise, detail_noise, 0.3);

    // Great Red Spot
    let red_spot_center = vec3<f32>(0.7, 0.2, 0.3);
    let red_spot_dist = distance(local_pos, red_spot_center);
    let red_spot = smoothstep(0.4, 0.0, red_spot_dist);
    let red_spot_noise = noise3(local_pos * 30.0 + time * 0.005);
    let red_spot_final = red_spot * (0.7 + 0.3 * red_spot_noise);

    // White ovals
    let wo1 = smoothstep(0.15, 0.0, distance(local_pos, vec3<f32>(-0.5, 0.6, 0.4)));
    let wo2 = smoothstep(0.12, 0.0, distance(local_pos, vec3<f32>(0.3, -0.7, 0.5)));
    let wo3 = smoothstep(0.18, 0.0, distance(local_pos, vec3<f32>(-0.2, -0.4, -0.8)));
    let white_ovals = wo1 + wo2 + wo3;

    // Small storms
    var small_storms = 0.0;
    for (var i = 0; i < 25; i++) {
        let angle = f32(i) * 0.251327;
        let radius = 0.4 + 0.6 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.9 * sin(f32(i) * 0.8);
        let sp = vec3<f32>(x, y, z);
        let ss = 0.03 + 0.02 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let si = 0.3 + 0.4 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_storms += smoothstep(ss, 0.0, distance(local_pos, sp)) * si;
    }

    // Turbulence
    let turb1 = noise3(local_pos * jupiter.turbulence_scale + time * 0.008);
    let turb2 = noise3(local_pos * jupiter.turbulence_scale * 2.0 - time * 0.012);
    let turbulence = (turb1 * 0.6 + turb2 * 0.4) * 0.2;

    let intensity = smoothstep(0.2, 0.8, atmosphere + turbulence + bands * 0.3);
    let base_color = jupiter_color(intensity);

    let band_color = vec3<f32>(0.6, 0.4, 0.2);
    var color = mix(base_color, band_color, bands * jupiter.band_contrast);

    let red_spot_color = vec3<f32>(0.8, 0.3, 0.1);
    color = mix(color, red_spot_color, red_spot_final * jupiter.red_spot_intensity);

    let white_oval_color = vec3<f32>(0.95, 0.9, 0.8);
    color = mix(color, white_oval_color, white_ovals * 0.5);

    let storm_color = vec3<f32>(0.7, 0.5, 0.3);
    color = mix(color, storm_color, small_storms * 0.3);

    // Strong limb darkening
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 3.0) * 0.6 + 0.4;
    color *= limb;

    let limb_glow = smoothstep(0.0, 0.3, 1.0 - cos_angle);
    let glow_color = vec3<f32>(0.8, 0.6, 0.4);
    color = mix(color, glow_color, limb_glow * 0.3);

    let sun_angle = dot(local_pos, sun_direction);
    let atmospheric_scattering = smoothstep(-1.0, 1.0, sun_angle);
    let atmosphere_color = vec3<f32>(0.9, 0.7, 0.5);
    color = mix(color, atmosphere_color, atmospheric_scattering * jupiter.atmosphere_density);

    return vec4<f32>(color, 1.0);
}
