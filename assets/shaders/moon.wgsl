// moon.wgsl — Procedural lunar surface with craters at multiple scales and dark maria.
// Features large named craters, medium impact sites, small crater fields,
// and dark basaltic maria (ancient lava flow regions). No atmosphere.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Moon parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct MoonUniforms {
    model: mat4x4<f32>,
    time: f32,
    crater_depth: f32,     // Crater darkness multiplier (default ~0.5)
    maria_darkness: f32,   // How dark the maria regions are (default ~0.7)
    surface_detail: f32,   // Fine surface noise frequency (default ~300.0)
    sun_direction: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> moon: MoonUniforms;

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
    let world_pos = moon.model * vec4<f32>(in.position, 1.0);
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
    return smoothstep(radius, 0.0, distance(p, center)) * depth;
}

fn mare(p: vec3<f32>, center: vec3<f32>, radius: f32, intensity: f32) -> f32 {
    return smoothstep(radius, 0.0, distance(p, center)) * intensity;
}

fn moon_color(t: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.4, 0.4, 0.4);
    let c2 = vec3<f32>(0.6, 0.6, 0.6);
    let c3 = vec3<f32>(0.8, 0.8, 0.8);
    if (t < 0.5) { return mix(c1, c2, t * 2.0); }
    return mix(c2, c3, (t - 0.5) * 2.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);

    let base_noise = noise3(local_pos * 60.0);
    let detail_noise = noise3(local_pos * moon.surface_detail);
    let surface = mix(base_noise, detail_noise, 0.3);

    // Large craters
    let cr1 = crater(local_pos, vec3<f32>(0.8, 0.1, 0.3), 0.25, 0.4);
    let cr2 = crater(local_pos, vec3<f32>(-0.7, 0.6, -0.2), 0.2, 0.35);
    let cr3 = crater(local_pos, vec3<f32>(0.4, -0.8, 0.4), 0.18, 0.3);
    let cr4 = crater(local_pos, vec3<f32>(-0.5, -0.4, -0.7), 0.22, 0.32);
    let mc1 = crater(local_pos, vec3<f32>(0.6, 0.5, 0.1), 0.12, 0.25);
    let mc2 = crater(local_pos, vec3<f32>(-0.3, 0.3, 0.8), 0.1, 0.2);
    let mc3 = crater(local_pos, vec3<f32>(0.2, -0.6, -0.7), 0.14, 0.22);

    var small_craters = 0.0;
    for (var i = 0; i < 30; i++) {
        let angle = f32(i) * 0.209440;
        let radius = 0.7 + 0.3 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.9 * sin(f32(i) * 1.3);
        let cp = vec3<f32>(x, y, z);
        let cs = 0.03 + 0.02 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let cd = 0.1 + 0.08 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_craters += crater(local_pos, cp, cs, cd);
    }

    // Maria
    let m1 = mare(local_pos, vec3<f32>(0.9, 0.2, 0.4), 0.4, 0.6);
    let m2 = mare(local_pos, vec3<f32>(-0.8, 0.7, -0.3), 0.35, 0.55);
    let m3 = mare(local_pos, vec3<f32>(0.3, -0.9, 0.5), 0.3, 0.5);
    let m4 = mare(local_pos, vec3<f32>(-0.6, -0.5, -0.8), 0.25, 0.45);

    let all_craters = (cr1 + cr2 + cr3 + cr4 + mc1 + mc2 + mc3 + small_craters) * moon.crater_depth;
    let all_maria = m1 + m2 + m3 + m4;
    let roughness = noise3(local_pos * 120.0) * 0.08;

    let surface_height = surface + roughness - all_craters;
    let intensity = smoothstep(0.2, 0.8, surface_height);
    let base_color = moon_color(intensity);

    let mare_color = vec3<f32>(0.2, 0.2, 0.2);
    var color = mix(base_color, mare_color, all_maria * moon.maria_darkness);

    let crater_color = vec3<f32>(0.3, 0.3, 0.3);
    color = mix(color, crater_color, all_craters * 0.5);

    // Subtle limb darkening (no atmosphere)
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 1.2) * 0.95 + 0.05;
    color *= limb;

    return vec4<f32>(color, 1.0);
}
