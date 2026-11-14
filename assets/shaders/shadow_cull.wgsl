// THE SHADOW ATLAS'S GPU-DRIVEN CASTERS (2026-11-13, renderer/shadow_gpu.rs;
// docs/design/local-light-shadows.md section 10, "Built: GPU-driven casters").
//
// The graphics card builds its own draw list for every shadow face, so the
// processor sends ONE command per face (a multi_draw_indexed_indirect_count)
// instead of one draw per run of casters. Three dispatches, one thread per
// (caster, view) pair or per (batch, view) pair:
//
//   1. cull_count:      each caster tested against each drawn view by the rung
//                       1 / 1b rule (below); a kept pair adds one to its
//                       (view, batch) count. A batch is one mesh in the arena:
//                       every caster of one mesh, as the fast path's runs were.
//   2. emit_draws:      each (view, batch) with a count takes a contiguous run
//                       of the instance buffer and writes its DrawIndexedIndirect
//                       arguments into its DRAW SET's run of the view's section
//                       of `draws`, that (view, set)'s draw count going up by one.
//                       A set is one pipeline over one vertex source (plain or
//                       card-hair cutout or see-through sheet; the arena or a
//                       skinned body's own buffers, 2026-11-14), so the
//                       processor draws each set of a view with ONE command.
//   3. write_instances: the same test again; a kept pair writes its caster's
//                       model matrix (the view's atlas index in column 0's w, as
//                       the fast path's shader reads it) into its draw's run.
//
// The draw order inside a view follows the atomics, so it changes from frame to
// frame; the depth does not (depth only, `Less`: the nearest surface wins
// whatever the order).
//
// THE RULE IS LOCKSTEP with `local_shadows::face_keeps` and `sun_keeps` (the
// processor's own rule, still used for the casters this path does not take and
// for deciding which faces get a tile) and `shadow_gpu::gpu_keeps` (this file's
// test written again in Rust, line for line; tests/the unit tests compare all
// three on one scene). A radius below zero means unbounded: always kept, as
// f32::INFINITY is on the processor.
//
// Published method: GPU-driven rendering as in Haar and Aaltonen, "GPU-Driven
// Rendering Pipelines" (SIGGRAPH 2015, Advances in Real-Time Rendering) and
// Wihlidal, "Optimizing the Graphics Pipeline with Compute" (GDC 2016): cull on
// the GPU, compact into indirect arguments, draw with one multi-draw.

struct Params {
    cam: array<vec4<f32>, 6>,
    n_casters: u32,
    n_views: u32,
    n_batches: u32,
    cull_off: u32,
    margin: f32,
    instance_cap: u32,
    // how many draw sets (a set: one pipeline and one vertex source, drawn by one command a view)
    n_sets: u32,
    pad1: u32,
};

struct View {
    planes: array<vec4<f32>, 6>,
    // xyz the light (or the eye for the near sun view), w the texel size per metre (flat for the sun)
    origin: vec4<f32>,
    // x the light's shadow reach, y 1 for the orthographic near sun view
    info: vec4<f32>,
    // x the view's index in the atlas's view array (the fast path's `view_proj`)
    index: vec4<u32>,
};

struct Caster {
    model: mat4x4<f32>,
    // xyz the bounding sphere's centre in render space, w its radius (below 0 = unbounded)
    sphere: vec4<f32>,
    // x the batch (arena mesh), y 1 for a celestial piece that casts into the near sun view only
    tag: vec4<u32>,
    // what the set's pipeline reads beside the matrix: a sheet's transmittance (shadow_transmit),
    // a cutout's material type and alpha threshold; zeros for a plain caster
    extra: vec4<f32>,
};

struct Batch {
    index_count: u32,
    first_index: u32,
    base_vertex: i32,
    // the draw set this batch is drawn in (`set` is a WGSL reserved word), and where that set's run starts in a view's section
    draw_set: u32,
    set_first: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

// One instance as the sets' pipelines read it: the caster's matrix (the view's index in column 0's
// w) and its extra (80 bytes; shadow_transmit's instance layout).
struct Instance {
    model: mat4x4<f32>,
    extra: vec4<f32>,
};

struct DrawArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> views: array<View>;
@group(0) @binding(2) var<storage, read> casters: array<Caster>;
@group(0) @binding(3) var<storage, read> batches: array<Batch>;
// [count: views x batches][fill: views x batches][start + 1: views x batches][total]; draw_counts
// one per (view, set)
@group(0) @binding(4) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> draws: array<DrawArgs>;
@group(0) @binding(6) var<storage, read_write> draw_counts: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> instances: array<Instance>;

// local_shadows::sphere_in over a view's planes.
fn view_sphere_in(v: u32, c: vec3<f32>, r: f32) -> bool {
    if (r < 0.0) {
        return true;
    }
    for (var i = 0u; i < 6u; i = i + 1u) {
        let p = views[v].planes[i];
        if (dot(p.xyz, c) + p.w < -r) {
            return false;
        }
    }
    return true;
}

// local_shadows::big_enough.
fn big_enough(c: vec3<f32>, r: f32, origin: vec3<f32>, texel_m: f32, ortho: bool) -> bool {
    if (r < 0.0) {
        return true;
    }
    var texel = texel_m;
    if (!ortho) {
        texel = texel_m * max(distance(c, origin) - r, 0.05);
    }
    return 2.0 * r >= texel;
}

// local_shadows::shadow_reaches_view against the camera's planes.
fn shadow_reaches_view(light: vec3<f32>, c: vec3<f32>, r: f32, reach: f32, pad: f32) -> bool {
    if (r < 0.0) {
        return true;
    }
    let to = c - light;
    let d = length(to);
    if (d <= r + 1e-4) {
        return true;
    }
    let far_d = max(reach, d);
    let far = light + to * (far_d / d);
    let near_r = r + pad;
    let far_r = r * far_d / d + pad;
    for (var i = 0u; i < 6u; i = i + 1u) {
        let p = params.cam[i];
        if (dot(p.xyz, c) + p.w < -near_r && dot(p.xyz, far) + p.w < -far_r) {
            return false;
        }
    }
    return true;
}

// Whether caster ci is drawn into view vi: local_shadows::sun_keeps for the near
// sun view, face_keeps (with the light's reach first) for a light's face.
fn keeps(ci: u32, vi: u32) -> bool {
    let s = casters[ci].sphere;
    let c = s.xyz;
    let r = s.w;
    let o = views[vi].origin;
    let info = views[vi].info;
    if (info.y > 0.5) {
        return view_sphere_in(vi, c, r) && big_enough(c, r, o.xyz, o.w, true);
    }
    if (casters[ci].tag.y != 0u) {
        return false;
    }
    let reach = info.x;
    if (r >= 0.0 && !(distance(c, o.xyz) < reach + r)) {
        return false;
    }
    if (!(view_sphere_in(vi, c, r) && big_enough(c, r, o.xyz, o.w, false))) {
        return false;
    }
    return params.cull_off != 0u || shadow_reaches_view(o.xyz, c, r, reach, o.w * params.margin);
}

@compute @workgroup_size(64)
fn cull_count(@builtin(global_invocation_id) id: vec3<u32>) {
    let ci = id.x;
    let vi = id.y;
    if (ci >= params.n_casters || vi >= params.n_views) {
        return;
    }
    if (keeps(ci, vi)) {
        atomicAdd(&counters[vi * params.n_batches + casters[ci].tag.x], 1u);
    }
}

@compute @workgroup_size(64)
fn emit_draws(@builtin(global_invocation_id) id: vec3<u32>) {
    let b = id.x;
    let vi = id.y;
    let nb = params.n_batches;
    if (b >= nb || vi >= params.n_views) {
        return;
    }
    let vb = vi * nb + b;
    let n = atomicLoad(&counters[vb]);
    if (n == 0u) {
        return;
    }
    let nvb = params.n_views * nb;
    let at = atomicAdd(&counters[3u * nvb], n);
    // The processor sizes the instance buffer for every candidate pair, so this never trips; if it
    // did, the draw is left out rather than written past the buffer.
    if (at + n > params.instance_cap) {
        return;
    }
    atomicStore(&counters[2u * nvb + vb], at + 1u);
    let bt = batches[b];
    // Each set's draws are a run of its own in the view's section (its batches' count long), so one
    // command draws the set.
    let d = atomicAdd(&draw_counts[vi * params.n_sets + bt.draw_set], 1u);
    draws[vi * nb + bt.set_first + d] = DrawArgs(bt.index_count, n, bt.first_index, bt.base_vertex, at);
}

@compute @workgroup_size(64)
fn write_instances(@builtin(global_invocation_id) id: vec3<u32>) {
    let ci = id.x;
    let vi = id.y;
    let nb = params.n_batches;
    if (ci >= params.n_casters || vi >= params.n_views) {
        return;
    }
    if (!keeps(ci, vi)) {
        return;
    }
    let vb = vi * nb + casters[ci].tag.x;
    let nvb = params.n_views * nb;
    let start = atomicLoad(&counters[2u * nvb + vb]);
    if (start == 0u) {
        return;
    }
    let slot = atomicAdd(&counters[nvb + vb], 1u);
    var m = casters[ci].model;
    m[0].w = f32(views[vi].index.x);
    instances[start - 1u + slot] = Instance(m, casters[ci].extra);
}
