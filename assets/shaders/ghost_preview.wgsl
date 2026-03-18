// ghost_preview.wgsl — Semi-transparent preview overlay for placement/building previews.
// Renders objects with a tinted, translucent appearance and basic directional lighting
// so the user can see where they are placing something before confirming.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (ghost_color, ghost_alpha via uniform)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct GhostMaterial {
    color: vec3<f32>,
    alpha: f32,
    light_direction: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> ghost: GhostMaterial;

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
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.world_position = input.position;
    output.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(ghost.light_direction);
    let diffuse = max(dot(input.normal, light_dir), 0.0);
    let lit_color = ghost.color * (0.5 + 0.5 * diffuse);
    return vec4<f32>(lit_color, ghost.alpha);
}
