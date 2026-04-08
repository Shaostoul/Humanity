// particle.wgsl — Billboard point-sprite particle shader.
//
// Each particle is a single vertex expanded into a screen-facing quad.
// Supports: color interpolation, size over lifetime, emissive glow,
//           alpha/additive blending (controlled by pipeline state).
//
// Bind groups:
//   Group 0: Camera (shared with pbr_simple, same view_proj + view_pos)

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    // (light data follows but we don't use it for particles)
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct ParticleVertex {
    @location(0) position: vec3<f32>,   // world position
    @location(1) color: vec4<f32>,      // RGBA (pre-interpolated on CPU)
    @location(2) size_emissive: vec2<f32>, // x = point size (world units), y = emissive strength
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) emissive: f32,
};

// Expand a single vertex into 4 corners of a billboard quad.
// We use @builtin(vertex_index) % 4 to select the corner.
@vertex
fn vs_main(
    particle: ParticleVertex,
    @builtin(vertex_index) vertex_id: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = particle.position;
    let size = particle.size_emissive.x;
    out.emissive = particle.size_emissive.y;
    out.color = particle.color;

    // Billboard: extract camera right and up from view_proj
    // For simplicity, use a fixed screen-facing quad
    let clip = camera.view_proj * vec4<f32>(world_pos, 1.0);

    // Quad corner offsets in clip space (NDC-proportional)
    let corner_id = vertex_id % 4u;
    var offset = vec2<f32>(0.0, 0.0);
    var uv = vec2<f32>(0.0, 0.0);
    switch corner_id {
        case 0u: { offset = vec2<f32>(-1.0, -1.0); uv = vec2<f32>(0.0, 1.0); }
        case 1u: { offset = vec2<f32>( 1.0, -1.0); uv = vec2<f32>(1.0, 1.0); }
        case 2u: { offset = vec2<f32>(-1.0,  1.0); uv = vec2<f32>(0.0, 0.0); }
        case 3u: { offset = vec2<f32>( 1.0,  1.0); uv = vec2<f32>(1.0, 0.0); }
        default: {}
    }
    out.uv = uv;

    // Scale offset by particle size projected to screen
    let dist = length(world_pos - camera.view_pos.xyz);
    let screen_size = size / max(dist, 0.01) * 200.0; // approximate projection
    out.clip_position = clip + vec4<f32>(offset * screen_size / vec2<f32>(1.0, 1.0), 0.0, 0.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Soft circular particle (radial falloff)
    let dist = length(in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
    if dist > 1.0 {
        discard;
    }
    let alpha = 1.0 - dist * dist; // smooth falloff
    var color = in.color.rgb;

    // Apply emissive boost
    if in.emissive > 0.0 {
        color = color * (1.0 + in.emissive);
    }

    return vec4<f32>(color, in.color.a * alpha);
}
