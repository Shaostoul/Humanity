// pluto.wgsl — Procedural Pluto dwarf planet with nitrogen ice plains,
// the heart-shaped Tombaugh Regio, cratered highlands, and thin haze atmosphere.
// Based on New Horizons imagery: reddish-brown tholins, bright nitrogen ice,
// and subtle atmospheric limb glow.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + Pluto parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct PlutoUniforms {
    model: mat4x4<f32>,
    time: f32,
    ice_brightness: f32,     // Nitrogen ice plain brightness (default ~0.8)
    tholin_intensity: f32,   // Reddish-brown organic color strength (default ~0.5)
    haze_density: f32,       // Thin atmospheric haze at limb (default ~0.15)
    sun_direction: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> pluto: PlutoUniforms;

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
    let world_pos = pluto.model * vec4<f32>(in.position, 1.0);
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

fn crater(p: vec3<f32>, center: vec3<f32>, radius: f32, depth: f32) -> f32 {
    return smoothstep(radius, 0.0, distance(p, center)) * depth;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let sun_direction = normalize(pluto.sun_direction);

    // Base surface: mix of icy and rocky terrain
    let base_noise = fbm(local_pos * 8.0);
    let detail_noise = noise3(local_pos * 80.0);
    let micro_noise = noise3(local_pos * 200.0);
    let surface = base_noise * 0.6 + detail_noise * 0.3 + micro_noise * 0.1;

    // Tombaugh Regio (heart-shaped bright nitrogen ice region)
    // Two overlapping circular lobes forming the heart shape
    let heart_center1 = vec3<f32>(0.7, 0.1, 0.5);
    let heart_center2 = vec3<f32>(0.5, 0.1, 0.7);
    let heart_lobe1 = smoothstep(0.45, 0.0, distance(local_pos, heart_center1));
    let heart_lobe2 = smoothstep(0.4, 0.0, distance(local_pos, heart_center2));
    let tombaugh = max(heart_lobe1, heart_lobe2);

    // Nitrogen ice plains within Tombaugh Regio (Sputnik Planitia)
    let ice_cells = fbm(local_pos * 25.0 + 7.3);
    let ice_pattern = smoothstep(0.4, 0.6, ice_cells) * tombaugh;

    // Dark highland regions (tholins - reddish organic compounds)
    let highland_noise = fbm(local_pos * 5.0 + 13.7);
    let highland_mask = smoothstep(0.3, 0.6, highland_noise) * (1.0 - tombaugh * 0.8);

    // Craters in highland regions
    let cr1 = crater(local_pos, vec3<f32>(-0.6, 0.5, -0.4), 0.2, 0.3);
    let cr2 = crater(local_pos, vec3<f32>(0.3, -0.7, -0.5), 0.15, 0.25);
    let cr3 = crater(local_pos, vec3<f32>(-0.4, -0.3, 0.8), 0.18, 0.28);

    var small_craters = 0.0;
    for (var i = 0; i < 15; i++) {
        let angle = f32(i) * 0.418879;
        let radius = 0.6 + 0.4 * noise3(vec3<f32>(f32(i), 0.0, 0.0));
        let x = cos(angle) * radius;
        let z = sin(angle) * radius;
        let y = 0.8 * sin(f32(i) * 1.1);
        let cp = vec3<f32>(x, y, z);
        let cs = 0.04 + 0.03 * noise3(vec3<f32>(f32(i), 1.0, 0.0));
        let cd = 0.1 + 0.08 * noise3(vec3<f32>(f32(i), 2.0, 0.0));
        small_craters += crater(local_pos, cp, cs, cd);
    }

    let all_craters = cr1 + cr2 + cr3 + small_craters;

    // Color palette
    let ice_color = vec3<f32>(0.9, 0.88, 0.85) * pluto.ice_brightness;  // Bright nitrogen ice
    let tholin_color = vec3<f32>(0.55, 0.35, 0.25) * pluto.tholin_intensity; // Reddish-brown organics
    let highland_color = vec3<f32>(0.45, 0.4, 0.35);  // Dark rocky highlands
    let crater_color = vec3<f32>(0.3, 0.28, 0.25);    // Darker crater material

    // Blend terrain types
    var color = highland_color;
    color = mix(color, tholin_color, highland_mask);
    color = mix(color, ice_color, tombaugh * 0.7);
    color = mix(color, ice_color * 1.1, ice_pattern * 0.5);
    color = mix(color, crater_color, all_craters * 0.4);

    // Subtle surface variation
    color *= 0.95 + 0.1 * noise3(local_pos * 60.0);

    // Limb darkening (very minimal atmosphere)
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 1.3) * 0.9 + 0.1;
    color *= limb;

    // Thin blue atmospheric haze at limb (discovered by New Horizons)
    let limb_glow = smoothstep(0.0, 0.4, 1.0 - cos_angle);
    let haze_color = vec3<f32>(0.5, 0.6, 0.8);
    color = mix(color, haze_color, limb_glow * pluto.haze_density);

    // Diffuse lighting (very distant sun, but still lit)
    let diffuse = max(dot(normal, sun_direction), 0.0);
    color *= 0.1 + 0.9 * diffuse; // Keep some ambient since Pluto is very far from the sun

    return vec4<f32>(color, 1.0);
}
