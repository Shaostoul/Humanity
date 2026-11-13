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
//
// SHAPE TARGETS first (2026-11-09, the MakeHuman body's morph targets, which the look's height
// drives): the body's morph block in the palette (`morph_base`, in matrices; pose.rs MORPH_SLOTS)
// says how many targets are on and, two to a row after its first, each target's index and weight;
// each adds its position and normal delta (two vec4 per vertex per target in `morphs`, at
// (target * vertex count + vertex) * 2) to the vertex before it is skinned.
//
// THE OWN-MESH POOL (2026-11-10, src/renderer/mesh_pool.rs): a near person's own mesh is a range of
// the pool's buffers, so a body's vertices start at `vertex_base` in `source` and its skinned copy
// at the same place in `skinned` (0 for a body with buffers of its own). The morph targets are read
// by the body's own vertex number (a pooled body has none on: its shape is in its vertices).

struct SkinParams {
    vertex_count: u32,
    joint_base: u32,
    morph_base: u32,
    vertex_base: u32,
};

@group(0) @binding(0) var<storage, read> palette: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> skinned: array<f32>;
@group(0) @binding(3) var<uniform> params: SkinParams;
@group(0) @binding(4) var<storage, read> morphs: array<vec4<f32>>;
// TEMPORAL ANTI-ALIASING (renderer/taa.rs): the previous palette (each person's rows from the last
// frame, renderer/skinning.rs `skin_upload_prev`) and the vertices skinned by it, in the same layout
// as `skinned`: the region meshes' vertex slot 2, where each vertex WAS for the motion target.
@group(0) @binding(5) var<storage, read> prev_palette: array<vec4<f32>>;
@group(0) @binding(6) var<storage, read_write> prev_skinned: array<f32>;

fn joint_apply_prev(joint: u32, p: vec4<f32>) -> vec3<f32> {
    let b = (params.joint_base + joint) * 3u;
    return vec3<f32>(dot(prev_palette[b], p), dot(prev_palette[b + 1u], p), dot(prev_palette[b + 2u], p));
}

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
    let w = (params.vertex_base + i) * 12u;
    var shaped_p = vec3<f32>(bitcast<f32>(source[w]), bitcast<f32>(source[w + 1u]), bitcast<f32>(source[w + 2u]));
    var shaped_n = vec3<f32>(bitcast<f32>(source[w + 3u]), bitcast<f32>(source[w + 4u]), bitcast<f32>(source[w + 5u]));
    let mb = params.morph_base * 3u;
    let head = palette[mb];
    let on = u32(head.x);
    let vcount = u32(head.y);
    for (var a = 0u; a < on; a = a + 1u) {
        let row = palette[mb + 1u + a / 2u];
        var target_index = row.x;
        var weight = row.y;
        if ((a & 1u) == 1u) {
            target_index = row.z;
            weight = row.w;
        }
        let k = (u32(target_index) * vcount + i) * 2u;
        shaped_p = shaped_p + morphs[k].xyz * weight;
        shaped_n = shaped_n + morphs[k + 1u].xyz * weight;
    }
    let pos = vec4<f32>(shaped_p, 1.0);
    let nrm = vec4<f32>(shaped_n, 0.0);
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
    // Last frame's pose, positions only (the motion target needs no normal).
    var q = joint_apply_prev(j01 & 0xffffu, pos) * w01.x;
    if (w01.y > 0.0) {
        q = q + joint_apply_prev(j01 >> 16u, pos) * w01.y;
    }
    if (w23.x > 0.0) {
        q = q + joint_apply_prev(j23 & 0xffffu, pos) * w23.x;
    }
    if (w23.y > 0.0) {
        q = q + joint_apply_prev(j23 >> 16u, pos) * w23.y;
    }
    let o = (params.vertex_base + i) * 8u;
    prev_skinned[o] = q.x;
    prev_skinned[o + 1u] = q.y;
    prev_skinned[o + 2u] = q.z;
    skinned[o] = p.x;
    skinned[o + 1u] = p.y;
    skinned[o + 2u] = p.z;
    skinned[o + 3u] = n.x;
    skinned[o + 4u] = n.y;
    skinned[o + 5u] = n.z;
    skinned[o + 6u] = bitcast<f32>(source[w + 6u]);
    skinned[o + 7u] = bitcast<f32>(source[w + 7u]);
}
