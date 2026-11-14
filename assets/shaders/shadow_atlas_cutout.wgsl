// THE SHADOW ATLAS'S GPU-DRIVEN CUTOUT CASTERS (2026-11-14, renderer/shadow_gpu.rs;
// docs/design/local-light-shadows.md section 10, "Built: the rest of the casters on the GPU").
//
// An alpha-tested caster's shape is its texture, so it cannot take the depth-only fast path: until
// this file every one was a draw of its own through the megashader's fs_shadow, about 0.7
// microseconds of the processor each, a hundred of them in the busiest hall. Here a cutout set (one
// material: its albedo bound at group 1) is drawn by ONE indirect command a shadow face, from the
// instances the cull writes (shadow_cull.wgsl), the same 80-byte instance every set reads: the
// caster's model matrix with the view's index in column 0's w, and its extra (x the material type,
// y the alpha threshold).
//
// ONLY CARD HAIR (type 26) is routed here (shadow_gpu::gpu_cutout_type): the photoscanned leaves
// (19) and cluster cards (21) sway in the megashader's vertex stage, and the sprite tree cards (12)
// are terrain, so those stay on the per-draw loop. The test below is fs_shadow's type 26 branch
// (90-fragment-main.wgsl) written again, LOCKSTEP (local_shadows' tests compare the two).

@group(0) @binding(0) var<storage, read> view_proj: array<mat4x4<f32>>;
@group(1) @binding(0) var albedo_texture: texture_2d<f32>;
@group(1) @binding(1) var albedo_sampler: sampler;

struct CasterIn {
    @location(0) position: vec3<f32>,
    @location(1) model0: vec4<f32>,
    @location(2) model1: vec4<f32>,
    @location(3) model2: vec4<f32>,
    @location(4) model3: vec4<f32>,
    @location(5) extra: vec4<f32>,
    @location(6) uv: vec2<f32>,
};

struct CutOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) extra: vec4<f32>,
};

@vertex
fn vs_cutout(v: CasterIn) -> CutOut {
    let model = mat4x4<f32>(vec4<f32>(v.model0.xyz, 0.0), v.model1, v.model2, v.model3);
    let world = model * vec4<f32>(v.position, 1.0);
    var o: CutOut;
    // The homogeneous w rebuilt as vs_main does.
    o.clip = view_proj[u32(v.model0.w + 0.5)] * vec4<f32>(world.xyz, 1.0);
    o.uv = v.uv;
    o.extra = v.extra;
    return o;
}

@fragment
fn fs_cutout(in: CutOut) {
    // The uv derivatives in uniform control flow, before the discard (as fs_shadow takes them).
    let uv_dx = dpdx(in.uv);
    let uv_dy = dpdy(in.uv);
    // Type 26, card hair: the opaque core's cut, from the character pack's colour half.
    let hair_uv = vec2<f32>(clamp(fract(in.uv.x), 0.002, 0.998) * 0.5, in.uv.y);
    let hair_tex = textureSampleGrad(
        albedo_texture, albedo_sampler, hair_uv,
        vec2<f32>(uv_dx.x * 0.5, uv_dx.y), vec2<f32>(uv_dy.x * 0.5, uv_dy.y));
    if (hair_tex.a < max(in.extra.y, 0.0)) {
        discard;
    }
}
