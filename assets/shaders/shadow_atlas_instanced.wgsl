// THE SHADOW ATLAS'S FAST CASTER PATH (2026-11-11, renderer/local_shadows.rs;
// docs/design/local-light-shadows.md section 10).
//
// Depth only, one draw per run of casters that share a mesh in one view: each
// caster's model matrix comes from a per-INSTANCE vertex attribute (four vec4
// columns, vertex buffer slot 1), with the view's index riding in its first
// column's w (an ordinary TRS matrix has 0 there; the CPU writes the index,
// this shader puts the 0 back), and the view's matrix from a storage array, so
// one bind group serves every view and a caster costs no binding at all. The
// view's viewport and scissor place it in its tile, as for the classic path.
//
// WHY. The first build drew every caster through the megashader's classic
// path, one draw per caster per view: a per-draw object uniform (a dynamic
// offset, so a set_bind_group per caster) and its material's groups, which a
// depth-only draw never reads. Measured in the ground hall at night
// (2026-11-11, RTX 4070, release): 3,540 caster draws a frame took
// cpu.shadow_local to 3.4 ms, about a microsecond a draw, almost all of it
// command recording. Instanced per view by mesh: about 900 draws.
//
// TRIED AND REJECTED (2026-11-11): one draw per mesh across the WHOLE atlas,
// the vertex shader mapping each instance into its tile and the fragment stage
// discarding what fell outside it. It cut the draws to about 330 but took
// gpu.shadow_local from 0.28 ms to 10.9 ms: without a viewport per view the
// hardware clips each face's triangles to the whole atlas instead of the face,
// so everything near a lamp (beside or behind a face) is rasterised across
// thousands of texels and then thrown away. Clip distances would fix that
// (wgpu's CLIP_DISTANCES feature, not core WebGPU); a later rung.
//
// WHAT TAKES THIS PATH. Only casters whose vertices the megashader's vs_main
// would not move: the plain surface class, never a cutout (fs_shadow's alpha
// test needs the material), never the wind classes (material types 19 to 23)
// or the water shell (16). Everything else keeps the classic path, so a
// swaying plant's shadow sways with it. Standard Z, `Less`, no culling and
// no bias, the sun shadow PSOs' state, so the two paths write the same depth.

@group(0) @binding(0) var<storage, read> view_proj: array<mat4x4<f32>>;

struct CasterIn {
    @location(0) position: vec3<f32>,
    @location(1) model0: vec4<f32>,
    @location(2) model1: vec4<f32>,
    @location(3) model2: vec4<f32>,
    @location(4) model3: vec4<f32>,
};

@vertex
fn vs_shadow_instanced(v: CasterIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(vec4<f32>(v.model0.xyz, 0.0), v.model1, v.model2, v.model3);
    let world = model * vec4<f32>(v.position, 1.0);
    // The homogeneous w rebuilt as vs_main does.
    return view_proj[u32(v.model0.w + 0.5)] * vec4<f32>(world.xyz, 1.0);
}
