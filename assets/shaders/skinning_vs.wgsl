// VERTEX-SHADER SKINNING for middle-distance bodies (character animation rung 1, 2026-11-08;
// docs/design/character-animation.md section 2.2; built by src/renderer/skinning.rs).
//
// Appended to the PBR megashader's source for the skinned surface PSO ONLY (the class PSOs never
// compile this entry, so nothing they bind changes). `vs_skinned` bends a SkinnedVertex by the
// body's joints and places it in the world, and hands the surface class's fragment entry exactly
// what `vs_main` hands it for a surface material: world position and normal, the uv, and zeros for
// the per-instance and wind channels (no wind, no water, no grass on a person).
//
// The pose palette is three vec4 rows per matrix (crates/humanity-animation/src/pose.rs). For a
// body drawn here, `instance_index` (the draw's first instance, core WebGPU for direct draws: the
// terrain batch relies on the same) is the index of its MODEL matrix in the palette; its joints'
// skinning matrices follow it. Group 1 is the skinned PSO's own layout: binding 0 the object uniform
// (an identity, because the palette carries the placement), binding 1 the palette.

@group(1) @binding(1) var<storage, read> skin_palette: array<vec4<f32>>;

struct SkinnedVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) joints: vec4<u32>,
    @location(4) weights: vec4<f32>,
};

// One palette matrix applied to a point (w = 1) or a direction (w = 0).
fn skin_apply(m: u32, p: vec4<f32>) -> vec3<f32> {
    let b = m * 3u;
    return vec3<f32>(dot(skin_palette[b], p), dot(skin_palette[b + 1u], p), dot(skin_palette[b + 2u], p));
}

@vertex
fn vs_skinned(v: SkinnedVertexInput, @builtin(instance_index) model_index: u32) -> VertexOutput {
    let jb = model_index + 1u;
    let pos = vec4<f32>(v.position, 1.0);
    let nrm = vec4<f32>(v.normal, 0.0);
    var p = skin_apply(jb + v.joints.x, pos) * v.weights.x;
    var n = skin_apply(jb + v.joints.x, nrm) * v.weights.x;
    if (v.weights.y > 0.0) {
        p = p + skin_apply(jb + v.joints.y, pos) * v.weights.y;
        n = n + skin_apply(jb + v.joints.y, nrm) * v.weights.y;
    }
    if (v.weights.z > 0.0) {
        p = p + skin_apply(jb + v.joints.z, pos) * v.weights.z;
        n = n + skin_apply(jb + v.joints.z, nrm) * v.weights.z;
    }
    if (v.weights.w > 0.0) {
        p = p + skin_apply(jb + v.joints.w, pos) * v.weights.w;
        n = n + skin_apply(jb + v.joints.w, nrm) * v.weights.w;
    }
    let world = skin_apply(model_index, vec4<f32>(p, 1.0));
    var wn = skin_apply(model_index, vec4<f32>(n, 0.0));
    let wl = length(wn);
    if (wl > 1.0e-8) {
        wn = wn / wl;
    } else {
        wn = vec3<f32>(0.0, 1.0, 0.0);
    }
    var out: VertexOutput;
    out.world_position = world;
    out.wind_offset = vec3<f32>(0.0, 0.0, 0.0);
    out.clip_position = camera.view_proj * vec4<f32>(world, 1.0);
    out.world_normal = wn;
    out.uv = v.uv;
    out.pack = v.uv;
    out.inst_data = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    return out;
}
