// Particle billboards (v0.966): camera-facing quads expanded in the vertex
// stage from per-instance particle data. CPU-simulated (renderer/particles.rs
// + data/particles.ron defs); this shader only billboards and tints.
//
// group(0) binding(0): the same CameraUniforms buffer the line pipeline
// binds (only view_proj + view_pos are read here).
// group(1) binding(0): the particle frame uniform - camera right/up axes
// for billboard expansion (computed on the CPU from yaw/pitch).

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    // The remaining CameraUniforms fields (lights, sun, fill) exist in the
    // buffer but are not declared here: a uniform BINDING may be larger
    // than the struct a shader reads from it.
};
@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct ParticleFrame {
    right: vec4<f32>,
    up: vec4<f32>,
};
@group(1) @binding(0) var<uniform> frame: ParticleFrame;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) corner: vec2<f32>,
    @location(2) emissive: f32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) size_emissive: vec2<f32>,
    @location(3) stretch: vec3<f32>,
) -> VsOut {
    // Triangle-strip quad corners from the vertex index: (0,0)(1,0)(0,1)(1,1)
    // mapped to [-1, 1].
    let cx = f32(vi & 1u) * 2.0 - 1.0;
    let cy = f32((vi >> 1u) & 1u) * 2.0 - 1.0;
    let half = size_emissive.x * 0.5;
    // Motion-blur streaks (v0.1036, rain): reorient the billboard's 2D
    // basis so its long axis follows the particle's velocity projected
    // onto the billboard plane, and lengthen it by half the motion
    // vector. stretch = 0 reduces exactly to the classic round sprite
    // (axis_u = up, axis_r = right, half_l = half).
    let sp = vec2<f32>(dot(stretch, frame.right.xyz), dot(stretch, frame.up.xyz));
    let slen = length(sp);
    var axis_u = vec2<f32>(0.0, 1.0);
    if (slen > 1.0e-4) {
        axis_u = sp / slen;
    }
    let axis_r = vec2<f32>(axis_u.y, -axis_u.x);
    let half_l = half + slen * 0.5;
    let world = center
        + (frame.right.xyz * axis_r.x + frame.up.xyz * axis_r.y) * (cx * half)
        + (frame.right.xyz * axis_u.x + frame.up.xyz * axis_u.y) * (cy * half_l);
    var out: VsOut;
    out.pos = camera.view_proj * vec4<f32>(world, 1.0);
    out.color = color;
    out.corner = vec2<f32>(cx, cy);
    out.emissive = size_emissive.y;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Soft round sprite: alpha falls off toward the quad edge.
    let r2 = dot(in.corner, in.corner);
    if (r2 > 1.0) {
        discard;
    }
    let soft = 1.0 - r2;
    let a = in.color.a * soft * soft;
    if (a < 0.004) {
        discard;
    }
    // Emissive lifts the color toward self-lit (sparks, embers); ambience
    // like leaves and dust stays at its authored tint.
    let rgb = in.color.rgb * (1.0 + in.emissive);
    return vec4<f32>(rgb, a);
}
