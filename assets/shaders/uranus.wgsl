// uranus.wgsl — Procedural Uranus ice giant with blue-green atmosphere,
// very subtle bands, and distinct polar regions from its extreme axial tilt (98 degrees).
// Featureless compared to other gas giants, with gentle atmospheric turbulence.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Uranus parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct UranusUniforms {
    model: mat4x4<f32>,
    time: f32,
    band_contrast: f32,       // Very subtle band contrast (default ~0.2)
    polar_brightness: f32,    // Polar region color shift (default ~0.3)
    atmosphere_density: f32,  // Atmospheric scattering (default ~0.15)
    sun_direction: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> uranus: UranusUniforms;

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
    let world_pos = uranus.model * vec4<f32>(in.position, 1.0);
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

fn uranus_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.4, 0.6, 0.7);
    let c2 = vec3<f32>(0.5, 0.7, 0.8);
    let c3 = vec3<f32>(0.6, 0.8, 0.9);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(uranus.sun_direction);
    let time = uranus.time;

    let latitude = abs(local_pos.y);
    let band1 = smoothstep(0.0, 0.3, latitude) * smoothstep(0.6, 0.3, latitude);
    let band2 = smoothstep(0.5, 0.8, latitude) * smoothstep(1.0, 0.8, latitude);
    let bands = band1 + band2;

    let base_noise = noise3(local_pos * 15.0 + time * 0.0006);
    let detail_noise = noise3(local_pos * 75.0 + time * 0.0015);
    let atmosphere = mix(base_noise, detail_noise, 0.3);

    // Polar regions (tilted rotation makes these prominent)
    let north_polar = smoothstep(0.4, 0.0, distance(local_pos, vec3<f32>(0.0, 1.0, 0.0)));
    let south_polar = smoothstep(0.4, 0.0, distance(local_pos, vec3<f32>(0.0, -1.0, 0.0)));
    let polar_regions = max(north_polar, south_polar);

    // Subtle features
    var atm_features = 0.0;
    for (var i = 0; i < 15; i++) {
        let angle = f32(i) * 0.418879;
        let radius = 0.6 + 0.4 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.7 * sin(f32(i) * 0.6);
        let fp = vec3<f32>(x, y, z);
        let fs = 0.02 + 0.015 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let fi = 0.15 + 0.2 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        atm_features += smoothstep(fs, 0.0, distance(local_pos, fp)) * fi;
    }

    let turb1 = noise3(local_pos * 30.0 + time * 0.004);
    let turb2 = noise3(local_pos * 60.0 - time * 0.006);
    let turbulence = (turb1 * 0.6 + turb2 * 0.4) * 0.1;

    let intensity = smoothstep(0.2, 0.8, atmosphere + turbulence + bands * 0.15);
    let base_color = uranus_color(intensity);

    let band_color = vec3<f32>(0.3, 0.5, 0.6);
    var color = mix(base_color, band_color, bands * uranus.band_contrast);

    let polar_color = vec3<f32>(0.5, 0.7, 0.8);
    color = mix(color, polar_color, polar_regions * uranus.polar_brightness);

    let feature_color = vec3<f32>(0.4, 0.6, 0.7);
    color = mix(color, feature_color, atm_features * 0.15);

    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 3.0) * 0.6 + 0.4;
    color *= limb;

    let limb_glow = smoothstep(0.0, 0.3, 1.0 - cos_angle);
    let glow_color = vec3<f32>(0.4, 0.6, 0.7);
    color = mix(color, glow_color, limb_glow * 0.3);

    let sun_angle = dot(local_pos, sun_direction);
    let atmospheric_scattering = smoothstep(-1.0, 1.0, sun_angle);
    let atmosphere_color = vec3<f32>(0.5, 0.7, 0.8);
    color = mix(color, atmosphere_color, atmospheric_scattering * uranus.atmosphere_density);

    return vec4<f32>(color, 1.0);
}
