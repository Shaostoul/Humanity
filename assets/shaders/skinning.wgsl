// COMPUTE SKINNING for near bodies (character animation rung 1, 2026-11-08;
// docs/design/character-animation.md section 2.2; driven by src/renderer/skinning.rs).
//
// Each near body is skinned ONCE a frame here, into a vertex buffer in the ordinary mesh layout
// (position, normal, uv: 8 floats), which every pass then draws as a plain mesh: the depth, the sun
// shadow cascades and the colour pass all read the same skinned vertices, so a body is not skinned
// again for every shadow cascade (plan 2.2). Middle-distance bodies skin in the vertex shader instead
// (skinning_vs.wgsl), which costs no memory.
//
// Linear blend skinning: a vertex is the weighted sum of its four joints' skinning matrices applied
// to it. The pose palette (crates/humanity-animation/src/pose.rs) holds each joint's matrix as three
// vec4 rows (48 bytes); a body's joints start at `joint_base` (in matrices).
//
// The source vertex is humanity_core::vertex::SkinnedVertex read as twelve u32 words: position (3
// floats), normal (3), uv (2), four u16 joint indices in two words, four unorm16 weights in two words
// (little-endian: the first u16 is the low half of its word).

struct SkinParams {
    vertex_count: u32,
    joint_base: u32,
    pad0: u32,
    pad1: u32,
};

@group(0) @binding(0) var<storage, read> palette: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> skinned: array<f32>;
@group(0) @binding(3) var<uniform> params: SkinParams;

// One joint's skinning matrix applied to a point (w = 1) or a direction (w = 0).
fn joint_apply(joint: u32, p: vec4<f32>) -> vec3<f32> {
    let b = (params.joint_base + joint) * 3u;
    return vec3<f32>(dot(palette[b], p), dot(palette[b + 1u], p), dot(palette[b + 2u], p));
}

@compute @workgroup_size(64)
fn cs_skin(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.vertex_count) {
        return;
    }
    let w = i * 12u;
    let pos = vec4<f32>(bitcast<f32>(source[w]), bitcast<f32>(source[w + 1u]), bitcast<f32>(source[w + 2u]), 1.0);
    let nrm = vec4<f32>(bitcast<f32>(source[w + 3u]), bitcast<f32>(source[w + 4u]), bitcast<f32>(source[w + 5u]), 0.0);
    let j01 = source[w + 8u];
    let j23 = source[w + 9u];
    let w01 = unpack2x16unorm(source[w + 10u]);
    let w23 = unpack2x16unorm(source[w + 11u]);
    // Unrolled, so nothing is indexed at run time (no private-memory spill).
    var p = joint_apply(j01 & 0xffffu, pos) * w01.x;
    var n = joint_apply(j01 & 0xffffu, nrm) * w01.x;
    if (w01.y > 0.0) {
        p = p + joint_apply(j01 >> 16u, pos) * w01.y;
        n = n + joint_apply(j01 >> 16u, nrm) * w01.y;
    }
    if (w23.x > 0.0) {
        p = p + joint_apply(j23 & 0xffffu, pos) * w23.x;
        n = n + joint_apply(j23 & 0xffffu, nrm) * w23.x;
    }
    if (w23.y > 0.0) {
        p = p + joint_apply(j23 >> 16u, pos) * w23.y;
        n = n + joint_apply(j23 >> 16u, nrm) * w23.y;
    }
    let nl = length(n);
    if (nl > 1.0e-8) {
        n = n / nl;
    } else {
        n = vec3<f32>(0.0, 1.0, 0.0);
    }
    let o = i * 8u;
    skinned[o] = p.x;
    skinned[o + 1u] = p.y;
    skinned[o + 2u] = p.z;
    skinned[o + 3u] = n.x;
    skinned[o + 4u] = n.y;
    skinned[o + 5u] = n.z;
    skinned[o + 6u] = bitcast<f32>(source[w + 6u]);
    skinned[o + 7u] = bitcast<f32>(source[w + 7u]);
}
