// sun_glow.wgsl — Screen-space solar corona/glow billboard rendered as a textured quad.
// Creates animated filamentary corona rays around the sun using layered pseudo-noise.
// Rendered additively on top of the scene as a post-effect overlay.
//
// Binding conventions:
//   Group 0: Camera/view uniforms (sun glow parameters)
//   Group 1: Material/object uniforms (not used)
//   Group 2: Textures/samplers (glow texture + sampler)
//   Vertex inputs: position (vec2), tex_coords (vec2)

struct SunGlowUniform {
    sun_ndc_pos: vec2<f32>,
    sun_scale: f32,
    aspect_ratio: f32,
    time: f32,
    flicker_speed: f32,    // Controls flicker animation speed (default ~0.5)
    filament_intensity: f32, // Filament ray brightness (default ~1.0)
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> sun_glow: SunGlowUniform;

@group(2) @binding(0)
var sun_glow_texture: texture_2d<f32>;
@group(2) @binding(1)
var sun_glow_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let pos = vec2<f32>(in.position.x / sun_glow.aspect_ratio, in.position.y) * sun_glow.sun_scale + sun_glow.sun_ndc_pos;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    return out;
}

fn pseudo_noise(a: f32, d: f32, t: f32) -> f32 {
    let n = sin(a * 7.3 + d * 5.1 + t * 0.21)
          + cos(a * 13.7 - d * 3.8 + t * 0.13)
          + sin(a * 2.9 + d * 11.1 - t * 0.17);
    return n / 3.0;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let uv = in.tex_coords;
    let dir = uv - center;
    let dist = length(dir) * 2.0;
    let angle = atan2(dir.y, dir.x);
    let t = sun_glow.time;

    // Broad soft background glow
    let base_glow = pow(1.0 - dist, 2.2) * 0.5;

    // Layered smooth noise for filament rays
    let noise1 = pseudo_noise(angle, dist, t);
    let noise2 = pseudo_noise(angle * 2.3 + t * 0.1, dist * 1.7 - t * 0.2, t * 0.7);
    let filaments = pow((0.6 + 0.4 * noise1) * (1.0 - dist), 4.0) * (0.7 + 0.3 * noise2) * sun_glow.filament_intensity;

    let corona = base_glow + filaments;

    // Subtle brightness flicker
    let flicker = 0.92 + 0.08 * sin(t * sun_glow.flicker_speed + angle * 1.2);

    // Color ramp: yellow-white core to orange-red outer
    let color_inner = vec3<f32>(1.0, 0.95, 0.7);
    let color_outer = vec3<f32>(1.2, 0.5, 0.1);
    let color = mix(color_outer, color_inner, pow(1.0 - dist, 1.5));
    let alpha = corona * flicker * 0.66;

    return vec4<f32>(color * alpha, alpha);
}
