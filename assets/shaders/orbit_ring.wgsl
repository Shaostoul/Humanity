// orbit_ring.wgsl — Glowing orbit ring with gradient fade.
//
// Same bind groups as pbr_simple.wgsl:
//   Group 0: Camera (view_proj, view_pos)
//   Group 1: Object (model, normal_matrix) — dynamic offset
//   Group 2: Material (base_color, params)
//
// Uses params.z as the ring's angular position reference (planet_angle in radians).
// The ring glows bright near the planet and fades to dark on the opposite side.

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
};

struct ObjectUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};

struct MaterialUniforms {
    base_color: vec4<f32>,
    // x = metallic, y = roughness, z = planet_angle (radians), w = unused
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(1) @binding(0) var<uniform> object: ObjectUniforms;
@group(2) @binding(0) var<uniform> material: MaterialUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = object.model * vec4<f32>(vertex.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((object.normal_matrix * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.uv = vertex.uv;
    return out;
}

// ── Constants ──

const PI: f32 = 3.14159265359;

// ── Fragment Shader ──

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Object center is at the ring's center (extracted from model matrix column 3)
    let ring_center = vec3<f32>(
        object.model[3][0],
        object.model[3][1],
        object.model[3][2],
    );

    // Angular position of this fragment relative to ring center (XZ plane)
    let dx = in.world_position.x - ring_center.x;
    let dz = in.world_position.z - ring_center.z;
    let frag_angle = atan2(dz, dx);

    // Planet angle from material params
    let planet_angle = material.params.z;

    // Angular gradient: 1.0 at planet, 0.1 at opposite side
    let angle_diff = frag_angle - planet_angle;
    let angular_gradient = cos(angle_diff) * 0.5 + 0.5;
    // Remap from [0,1] to [0.1, 1.0] so the dark side isn't fully invisible
    let gradient = mix(0.1, 1.0, angular_gradient);

    // Core + glow effect from tube cross-section UV
    // UV.y maps across the tube: 0 at one edge, 1 at the other, 0.5 at center
    let dist_from_center = abs(in.uv.y - 0.5) * 2.0;

    // Sharp bright core (narrow gaussian)
    let core = exp(-dist_from_center * dist_from_center * 20.0);
    // Soft outer glow (wider gaussian)
    let glow = exp(-dist_from_center * dist_from_center * 4.0);
    // Combined brightness: core is extra bright, glow is softer
    let brightness = core * 2.0 + glow * 0.5;

    // Final color: base_color tinted by core glow and angular gradient
    let color = material.base_color.rgb * brightness * gradient;

    // Alpha: glow intensity scaled by angular gradient for transparency
    let alpha = clamp(brightness * gradient, 0.0, 1.0) * material.base_color.a;

    return vec4<f32>(color, alpha);
}
