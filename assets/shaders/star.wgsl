// star.wgsl — Point-based star rendering with magnitude-driven brightness.
// Each star is a point with color (RGB + magnitude in alpha) and a point_size hint.
// Uses exponential brightness mapping based on apparent magnitude for realistic star fields.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Vertex inputs: position (vec3), color (vec4 — alpha encodes magnitude), point_size (f32)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) point_size: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) magnitude: f32,
    @location(2) point_coord: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.view_proj * vec4<f32>(input.pos, 1.0);
    out.color = input.color.rgb;
    out.magnitude = input.color.a;
    out.point_coord = vec2<f32>(0.5, 0.5);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Exponential brightness from astronomical magnitude
    // Brighter stars have lower (even negative) magnitude values
    let min_mag = -1.5;
    let brightness = exp2(-0.4 * (input.magnitude - min_mag));
    let color = input.color * brightness;
    let alpha = clamp(brightness, 0.05, 1.0);
    return vec4<f32>(color, alpha);
}
