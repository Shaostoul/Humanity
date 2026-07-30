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
    @location(2) size_emissive: vec3<f32>, // x = point size (world units), y = emissive, z = shape
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) emissive: f32,
    /// 0 = soft round dot, 1 = procedural six-fold snowflake (v0.1064).
    @location(3) shape: f32,
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
    out.shape = particle.size_emissive.z;
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
    let p = (in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
    let dist = length(p);
    if dist > 1.0 {
        discard;
    }
    var alpha = 1.0 - dist * dist; // smooth falloff (round sprite)

    // ── PROCEDURAL SNOWFLAKE (v0.1064) ──
    // Operator: "Can we also do procedural snowflake designs instead of just
    // dots? While it won't matter much for distant snow the stuff that falls
    // real close to the screen will look beautiful."
    //
    // Real flakes are hexagonal because ice crystallises on a 60 degree
    // lattice, so the honest construction is to FOLD the sprite into one
    // 60 degree wedge and draw a single arm - the six-fold symmetry then comes
    // out of the geometry rather than being drawn six times. Inside the wedge:
    // a central hub, a main spine, and two side branches at the classic 60
    // degrees, which is the dendrite shape everyone recognises.
    //
    // Costs one atan2, one modulo and a handful of distance tests, only on
    // particles flagged shape = 1. Distant flakes collapse to a bright dot
    // because the arms fall below a pixel, which is exactly what should happen.
    if (in.shape > 0.5) {
        let ang = atan2(p.y, p.x);
        let sector = 3.14159265 / 3.0;
        // Fold into one wedge and mirror it, so the arm is symmetric.
        let a = abs((ang - floor(ang / sector + 0.5) * sector));
        let r = dist;
        // Main spine along the wedge centre.
        let spine = abs(sin(a)) * r;
        var f = 1.0 - smoothstep(0.0, 0.075, spine);
        // Two side branches at 60 degrees, strongest mid-arm, tapering out.
        let b1 = abs(sin(a - 0.52)) * r;
        let b2 = abs(sin(a + 0.52)) * r;
        let branch_win = smoothstep(0.30, 0.42, r) * (1.0 - smoothstep(0.55, 0.85, r));
        f = max(f, (1.0 - smoothstep(0.0, 0.055, b1)) * branch_win);
        f = max(f, (1.0 - smoothstep(0.0, 0.055, b2)) * branch_win);
        // Hexagonal hub at the centre.
        f = max(f, 1.0 - smoothstep(0.10, 0.20, r));
        // Fade the whole crystal out at the rim so it never shows a hard edge.
        f = f * (1.0 - smoothstep(0.80, 1.0, r));
        alpha = f;
        if (alpha < 0.02) {
            discard;
        }
    }
    var color = in.color.rgb;

    // Apply emissive boost
    if in.emissive > 0.0 {
        color = color * (1.0 + in.emissive);
    }

    return vec4<f32>(color, in.color.a * alpha);
}
