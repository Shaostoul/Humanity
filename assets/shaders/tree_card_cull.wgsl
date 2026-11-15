// THE TREE CARD CULL (2026-11-14, renderer/tree_card_cull.rs has the whole story).
//
// One workgroup per terrain patch that carries tree cards. Each thread takes quads of that patch
// and tests the quad's bounding sphere (its cull record: centre in the patch frame, radius):
// - the MAIN view keeps it when the sphere meets the window the fragment stage keeps (the hide
//   radius to the far cutoff, read from the SAME shadow uniform fs_terrain reads) and the camera's
//   frustum (the SAME camera uniform the draw reads, its four side planes, widened for the
//   temporal jitter);
// - the SUN view keeps it when the sphere meets the shadow map's box (the light's own matrix, all
//   six planes: what lies outside them is clipped away anyway).
// A kept quad's indices are appended to its patch's region of the compact index buffer and its
// count added to the patch's indirect draw. The main view's compact indices are a window of the
// patch arena's own index buffer, and its card draw sits right after the patch's ground draw in one
// argument list, so the cards keep the draw order the whole-patch draw had (renderer/tree_card_cull.rs). The main view takes both windings (the pass culls back
// faces, as it did); the sun view takes ONE, since it draws without face culling and the second
// winding covered the same texels again.
//
// Every test is a bounding sphere against a plane or a distance, so it only ever keeps MORE than
// the per-pixel tests let through: those still decide every edge, and the picture does not change.
// The Rust twin of `keeps_main` and `keeps_sun` is `tree_card_cull::cpu_keeps`; change one, change
// both (`the_gpu_cull_keeps_exactly_the_quads_the_rust_twin_keeps`).

struct Cam {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
};

// The first three fields of the celestial pass's shadow uniform (celestial.rs `su`): params.x =
// shadows on, params.w = the card hide radius, params2.x = the card far cutoff.
struct ShadowU {
    vp: mat4x4<f32>,
    params: vec4<f32>,
    params2: vec4<f32>,
};

// One card-bearing drawn patch. `pos` = its translation (render space); `rec` its first cull
// record; `quads` its card count; `v0` its first card vertex in the arena (absolute); `dst` and
// `sdst` its regions in the main window and the sun buffer (12 and 6 indices a quad); `arg` its
// card draw's place in the main argument list.
struct Job {
    pos: vec4<f32>,
    rec: u32,
    quads: u32,
    v0: u32,
    dst: u32,
    sdst: u32,
    arg: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<uniform> cam: Cam;
@group(0) @binding(1) var<uniform> light: Cam;
@group(0) @binding(2) var<uniform> su: ShadowU;
@group(0) @binding(3) var<uniform> rot: mat4x4<f32>;
@group(0) @binding(4) var<storage, read> recs: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read> jobs: array<Job>;
@group(0) @binding(6) var<storage, read_write> main_args: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> sun_args: array<atomic<u32>>;
@group(0) @binding(8) var<storage, read_write> main_idx: array<u32>;
@group(0) @binding(9) var<storage, read_write> sun_idx: array<u32>;

const WORKGROUP: u32 = 64u;
// The temporal jitter moves the picture by under a pixel; widen the side planes by this many
// radians of view (about five pixels at 2560 across a 60 degree view), times the distance.
const JITTER_MARGIN: f32 = 0.004;

fn row(m: mat4x4<f32>, i: u32) -> vec4<f32> {
    return vec4<f32>(m[0][i], m[1][i], m[2][i], m[3][i]);
}

// Signed distance from a clip-space plane (a row combination of a view-projection matrix) to `c`.
fn plane_dist(p: vec4<f32>, c: vec3<f32>) -> f32 {
    return (dot(p.xyz, c) + p.w) / max(length(p.xyz), 1.0e-20);
}

fn keeps_main(c: vec3<f32>, r: f32) -> bool {
    let d = distance(c, cam.view_pos.xyz);
    if (d + r < su.params.w || d - r > su.params2.x) {
        return false;
    }
    let m = cam.view_proj;
    let r0 = row(m, 0u);
    let r1 = row(m, 1u);
    let r3 = row(m, 3u);
    let rm = -(r + d * JITTER_MARGIN);
    return plane_dist(r3 + r0, c) >= rm && plane_dist(r3 - r0, c) >= rm
        && plane_dist(r3 + r1, c) >= rm && plane_dist(r3 - r1, c) >= rm;
}

fn keeps_sun(c: vec3<f32>, r: f32) -> bool {
    let m = light.view_proj;
    let r0 = row(m, 0u);
    let r1 = row(m, 1u);
    let r2 = row(m, 2u);
    let r3 = row(m, 3u);
    return plane_dist(r3 + r0, c) >= -r && plane_dist(r3 - r0, c) >= -r
        && plane_dist(r3 + r1, c) >= -r && plane_dist(r3 - r1, c) >= -r
        && plane_dist(r2, c) >= -r && plane_dist(r3 - r2, c) >= -r;
}

@compute @workgroup_size(64)
fn cull_cards(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let j = jobs[wg.x];
    let sun_on = su.params.x > 0.5;
    for (var q = lid.x; q < j.quads; q = q + WORKGROUP) {
        let rec = recs[j.rec + q];
        let c = (rot * vec4<f32>(rec.xyz, 0.0)).xyz + j.pos.xyz;
        let v = j.v0 + 4u * q;
        if (keeps_main(c, rec.w)) {
            let o = j.dst + atomicAdd(&main_args[j.arg * 5u], 12u);
            // The patch builder's twelve (planet_chunks emit_card / emit_sprite_card): the quad,
            // then the quad wound backwards.
            main_idx[o] = v;
            main_idx[o + 1u] = v + 1u;
            main_idx[o + 2u] = v + 2u;
            main_idx[o + 3u] = v;
            main_idx[o + 4u] = v + 2u;
            main_idx[o + 5u] = v + 3u;
            main_idx[o + 6u] = v;
            main_idx[o + 7u] = v + 2u;
            main_idx[o + 8u] = v + 1u;
            main_idx[o + 9u] = v;
            main_idx[o + 10u] = v + 3u;
            main_idx[o + 11u] = v + 2u;
        }
        if (sun_on && keeps_sun(c, rec.w)) {
            let o = j.sdst + atomicAdd(&sun_args[wg.x * 5u], 6u);
            sun_idx[o] = v;
            sun_idx[o + 1u] = v + 1u;
            sun_idx[o + 2u] = v + 2u;
            sun_idx[o + 3u] = v;
            sun_idx[o + 4u] = v + 2u;
            sun_idx[o + 5u] = v + 3u;
        }
    }
}
