// ── ROOM GI, RUNG 1: SAMPLING THE ROOM PROBES (docs/design/room-gi.md) ──
//
// Ship interiors had no indirect light: the interior passes zero the pad that
// carries the local up, so frag_tail decides the fragment is not under a sky
// and the indirect term was the 0.005 silhouette floor. A wall no lamp faced
// rendered black beside a floor the sun lit. This part gives each ROOM its own
// grid of DDGI irradiance probes (Majercik et al., JCGT 8(2) 2019) and lets a
// fragment read its room's grid. It is sampled in frag_tail's indirect term
// behind the HAS_ROOM_GI switch, which is on only in the surface and
// vegetation PSOs.
//
// SHARED TEXT. This file is part 85 of the megashader AND the head of the probe
// update shader (renderer::room_probes_gpu prefixes it onto
// assets/shaders/room_probes_update.wgsl), so the fragment and the update read
// the probes through ONE function. It names three resources it does not
// declare: `room_gi` (the room table), `room_gi_atlas` (the probe atlas) and
// `room_gi_samp` (bilinear, clamp). The megashader declares them at group 0,
// bindings 5 to 7 (00-bindings-vertex.wgsl); the update shader at its own.
// Everything here uses textureSampleLevel, never textureSample, because the
// compute stage has no derivatives.
//
// CPU TWIN: src/renderer/room_probes.rs (`oct_encode`, `oct_decode`,
// `sample_grid`, `pick_room`), which is what the tests run. LOCKSTEP: change a
// constant or a weight here and there in the same edit.

// Interior texels per tile side (irradiance, depth) and the tile sizes with
// their 1-texel octahedral border (room_probes::IRR_N, DEPTH_N, *_TILE).
const GI_IRR_N: f32 = 8.0;
const GI_DEPTH_N: f32 = 16.0;
const GI_IRR_TILE: u32 = 10u;
const GI_DEPTH_TILE: u32 = 18u;
// DDGI normal bias: sample the grid 10 cm off the surface (NORMAL_BIAS_M).
const GI_NORMAL_BIAS: f32 = 0.1;
// Room pick: decide at p + n * 0.5 m, accept within 0.25 m of a room box
// (ROOM_PICK_OFFSET_M / ROOM_PICK_MARGIN_M). An inside wall face picks the
// room it faces; the outside of the hull picks nothing.
const GI_PICK_OFFSET: f32 = 0.5;
const GI_PICK_MARGIN: f32 = 0.25;
// DDGI's weight crush threshold (CRUSH_THRESHOLD).
const GI_CRUSH: f32 = 0.2;

// Room table header, 64 bytes (room_probes::GiHeaderGpu).
struct GiHeader {
    // xyz = min corner of every room (render space), w = 1 while room GI is on.
    gmin: vec4<f32>,
    // xyz = max corner of every room.
    gmax: vec4<f32>,
    // x = room count, y = probe count, z = irradiance tiles per atlas row,
    // w = depth tiles per atlas row.
    info: vec4<u32>,
    // x = atlas width, y = atlas height, z = first row of the depth region,
    // w = how many solid FLOOR SLABS follow the rooms (ship homes increment 7,
    // the slab rows below; 0 for a one-storey home and with the dev switch
    // showcase {"slab_occlusion":"0"}).
    atlas: vec4<u32>,
};

// One room, 160 bytes (room_probes::GiRoomGpu).
struct GiRoom {
    // xyz = box min (render space).
    bmin: vec4<f32>,
    // xyz = box max, w = lid transmittance (0 = opaque roof).
    bmax: vec4<f32>,
    // xyz = probes per axis, w = the room's first probe in the atlas.
    counts: vec4<u32>,
    // x = first light in the update's light list, y = how many.
    lights: vec4<u32>,
    // Diffuse reflectance per face, in face order -x, +x, floor, lid, -z, +z.
    refl: array<vec4<f32>, 6>,
};

struct GiRooms {
    header: GiHeader,
    rooms: array<GiRoom>,
};

// sign() that never returns 0, so the octahedral fold is defined on the axes.
fn gi_sign_not_zero(v: vec2<f32>) -> vec2<f32> {
    return select(vec2<f32>(-1.0), vec2<f32>(1.0), v >= vec2<f32>(0.0));
}

// Unit direction -> octahedral point in [-1, 1]^2 (+z pole in the diamond).
fn gi_oct_encode(d: vec3<f32>) -> vec2<f32> {
    let l1 = abs(d.x) + abs(d.y) + abs(d.z);
    var p = d.xy / max(l1, 1.0e-20);
    if (d.z < 0.0) {
        p = (vec2<f32>(1.0) - abs(p.yx)) * gi_sign_not_zero(p);
    }
    return p;
}

// Octahedral point -> unit direction; exact inverse of gi_oct_encode.
fn gi_oct_decode(p: vec2<f32>) -> vec3<f32> {
    var v = vec3<f32>(p.x, p.y, 1.0 - abs(p.x) - abs(p.y));
    if (v.z < 0.0) {
        let xy = (vec2<f32>(1.0) - abs(v.yx)) * gi_sign_not_zero(v.xy);
        v = vec3<f32>(xy.x, xy.y, v.z);
    }
    return normalize(v);
}

// Top-left border texel of probe `probe`'s irradiance and depth tiles.
// Tiles per atlas row are CONSTANTS (the atlas is always ATLAS_WIDTH = 8100
// texels wide, room_probes.rs), so the divide and modulo below compile to a
// multiply and a shift instead of a slow integer division per probe. The
// header carries the same numbers (info.z, info.w) for the CPU side to check.
const GI_IRR_PER_ROW: u32 = 810u;
const GI_DEPTH_PER_ROW: u32 = 450u;
fn gi_irr_origin(probe: u32) -> vec2<u32> {
    return vec2<u32>(probe % GI_IRR_PER_ROW, probe / GI_IRR_PER_ROW) * GI_IRR_TILE;
}
fn gi_depth_origin(probe: u32) -> vec2<u32> {
    return vec2<u32>(probe % GI_DEPTH_PER_ROW, probe / GI_DEPTH_PER_ROW) * GI_DEPTH_TILE
        + vec2<u32>(0u, room_gi.header.atlas.z);
}

// Atlas UV of direction d inside an n x n tile whose border starts at `origin`
// (room_probes::tile_coord, divided by the atlas size).
fn gi_atlas_uv(origin: vec2<u32>, n: f32, d: vec3<f32>) -> vec2<f32> {
    let texel = vec2<f32>(origin) + vec2<f32>(1.0) + (gi_oct_encode(d) * 0.5 + vec2<f32>(0.5)) * n;
    return texel / vec2<f32>(f32(room_gi.header.atlas.x), f32(room_gi.header.atlas.y));
}

// The cosine-weighted mean radiance (E / pi, the unit sky_ambient returns)
// that room `ri`'s probes give a surface at p with normal n. DDGI's sample:
// the 8 probes of the cell around p + n * bias, each weighted by a smooth
// backface term, a Chebyshev visibility test on its depth moments, the weight
// crush and its trilinear weight, blended in square-root space.
//
// WHEN THE VISIBILITY TEST RUNS. The Chebyshev test and its depth fetch are
// half of this function's cost (measured 2026-09-27, docs/design/room-gi.md),
// so they run only in a room that asks for them (bit 0 of `lights.z`,
// GI_ROOM_VISIBILITY). A room whose probes trace only its own box (rung 1)
// never does, because there the test is an identity: the box is convex, so a
// probe's ray toward any point inside it leaves the box no nearer than that
// point, the stored mean depth is never short of the point's distance, and the
// weight is 1. The twin proves it
// (`room_probes::tests::visibility_is_an_identity_inside_a_box`). Rung 2, which
// traces a room's contents, sets the bit for its rooms; the dev switch
// showcase {"room_gi_vis":"1"} sets it everywhere for A/B.
const GI_ROOM_VISIBILITY: u32 = 1u;
fn gi_sample_room(ri: u32, p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let vis = (room_gi.rooms[ri].lights.z & GI_ROOM_VISIBILITY) != 0u;
    let bmin = room_gi.rooms[ri].bmin.xyz;
    let bmax = room_gi.rooms[ri].bmax.xyz;
    let counts = room_gi.rooms[ri].counts;
    let cf = vec3<f32>(counts.xyz);
    let cell = (bmax - bmin) / cf;
    let ps = p + n * GI_NORMAL_BIAS;
    let g = (ps - bmin) / cell - vec3<f32>(0.5);
    let base = clamp(floor(g), vec3<f32>(0.0), cf - vec3<f32>(2.0));
    let alpha = clamp(g - base, vec3<f32>(0.0), vec3<f32>(1.0));
    let base_u = vec3<u32>(base);
    // Loop invariants: the atlas's texel-to-uv scale, and where the normal
    // lands inside an irradiance tile (the same for all eight probes).
    let inv_size = vec2<f32>(1.0) / vec2<f32>(f32(room_gi.header.atlas.x), f32(room_gi.header.atlas.y));
    let n_tile = (gi_oct_encode(n) * 0.5 + vec2<f32>(0.5)) * GI_IRR_N + vec2<f32>(1.0);
    var sum = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let off = vec3<u32>(i & 1u, (i >> 1u) & 1u, (i >> 2u) & 1u);
        let pc = base_u + off;
        let probe = counts.w + pc.x + counts.x * (pc.y + counts.y * pc.z);
        let ppos = bmin + (vec3<f32>(pc) + vec3<f32>(0.5)) * cell;
        let offf = vec3<f32>(off);
        let tri3 = (vec3<f32>(1.0) - alpha) * (vec3<f32>(1.0) - offf) + alpha * offf;
        let tri = tri3.x * tri3.y * tri3.z;
        // A probe with no trilinear weight contributes nothing whatever its
        // other weights are, so skip its two fetches. This is most of them
        // on a room's own surfaces: probes sit at cell centres, so a floor,
        // wall or ceiling point is clamped to the outermost layer on that
        // axis and four of the eight probes weigh exactly zero.
        if (tri <= 0.0) {
            continue;
        }
        // Smooth backface weight: a probe behind the surface counts for little.
        let to_probe = ppos - p;
        let tp_len = length(to_probe);
        var bf = 0.5;
        if (tp_len > 1.0e-6) {
            bf = (dot(to_probe / tp_len, n) + 1.0) * 0.5;
        }
        var w = bf * bf + 0.2;
        // Chebyshev visibility from the probe's depth moments.
        if (vis) {
            let p2pt = ps - ppos;
            let dist = length(p2pt);
            var dir = n;
            if (dist > 1.0e-4) {
                dir = p2pt / dist;
            }
            let m = textureSampleLevel(room_gi_atlas, room_gi_samp, gi_atlas_uv(gi_depth_origin(probe), GI_DEPTH_N, dir), 0.0).xy;
            if (dist > m.x) {
                let variance = abs(m.y - m.x * m.x);
                let excess = dist - m.x;
                let cheb = variance / max(variance + excess * excess, 1.0e-9);
                w = w * max(cheb * cheb * cheb, 0.0);
            }
        }
        w = max(w, 1.0e-6);
        if (w < GI_CRUSH) {
            w = w * w * w / (GI_CRUSH * GI_CRUSH);
        }
        w = w * tri;
        let irr = textureSampleLevel(room_gi_atlas, room_gi_samp, (vec2<f32>(gi_irr_origin(probe)) + n_tile) * inv_size, 0.0).rgb;
        sum = sum + sqrt(max(irr, vec3<f32>(0.0))) * w;
        wsum = wsum + w;
    }
    let r = sum / max(wsum, 1.0e-9);
    return r * r;
}

// Which room a surface at p facing n belongs to, or -1 (room GI off, outside
// every room, or the outside of the hull). room_probes::pick_room.
fn gi_pick_room(p: vec3<f32>, n: vec3<f32>) -> i32 {
    let h = room_gi.header;
    if (h.gmin.w < 0.5) {
        return -1;
    }
    let q = p + n * GI_PICK_OFFSET;
    let slack = vec3<f32>(GI_PICK_MARGIN);
    if (any(q < h.gmin.xyz - slack) || any(q > h.gmax.xyz + slack)) {
        return -1;
    }
    // The table is ordered for this walk every frame (room_probes::pick_order):
    // nearest the eye first, so what is on screen usually stops within a step
    // or two, and every room ahead of any room whose box contains it. So the
    // FIRST box that contains the point is the answer; only a point outside
    // every box (a doorway, a wall's thickness) walks them all for the nearest
    // one within the margin.
    var best = -1;
    var best_d = GI_PICK_MARGIN + 1.0e-3;
    for (var i = 0u; i < h.info.x; i = i + 1u) {
        let bmin = room_gi.rooms[i].bmin.xyz;
        let bmax = room_gi.rooms[i].bmax.xyz;
        let d = length(max(max(bmin - q, q - bmax), vec3<f32>(0.0)));
        if (d <= 0.0) {
            return i32(i);
        }
        if (d < best_d) {
            best = i32(i);
            best_d = d;
        }
    }
    return best;
}

// frag_tail's entry point into room GI: rgb = the room's indirect light for
// this fragment, a = 1 if the fragment is in a room (0 = keep the old floor).
fn room_gi_irradiance(p: vec3<f32>, n: vec3<f32>) -> vec4<f32> {
    let ri = gi_pick_room(p, n);
    if (ri < 0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(gi_sample_room(u32(ri), p, n), 1.0);
}

// ── FLOORS BETWEEN STOREYS (ship homes increment 7, lane L) ──
//
// A solid floor slab keeps lamp light and sunlight on its own side. The slabs
// ride in the room table AFTER the rooms, as GiRoom-sized rows of which only
// bmin.xyz and bmax.xyz (the slab's box, render space) are read:
//   rows [info.x, info.x + atlas.w)   every solid slab once (the sun's list);
//   then, per scene light, the RUN of slabs its range reaches, copied, which
//   the light names as (first, count) in cone_inner.yz, first counted from
//   info.x + atlas.w (light::gpu_packed_with_slabs, room_probes_gpu).
// Glass slabs pass light and have no row. No layout change: the table is the
// one every pipeline already binds at group 0 binding 7.
//
// The test, for a point q (a lamp) or the sun along d: the fragment and the
// light on opposite sides of the slab's mid-plane, and the crossing inside the
// slab's rectangle shrunk by SLAB_EDGE_TOL (so a fragment on the slab's own
// edge face is not shadowed by its own slab).
//
// CPU TWIN: room_probes::OccluderSlab::blocks_segment / blocks_ray and
// room_probes::sun_blocked_by_slabs. LOCKSTEP with SLAB_EDGE_TOL_M and
// SLAB_SUN_REACH_M there.
//
// Read only through the two guarded call sites (frag_tail's light loop and
// sun_shadow_offset), both behind HAS_SLAB_OCCLUSION. The probe update shader
// compiles these functions as part of this shared text and calls none of them:
// a room box never holds a solid slab (room_probes::light_irradiance says why).
const SLAB_EDGE_TOL: f32 = 0.01;
const SLAB_SUN_REACH: f32 = 1.0;

// The crossing point c lies inside row r's rectangle, shrunk by the tolerance.
fn slab_rect_holds(r: u32, c: vec3<f32>) -> bool {
    let bmin = room_gi.rooms[r].bmin.xyz;
    let bmax = room_gi.rooms[r].bmax.xyz;
    return c.x > bmin.x + SLAB_EDGE_TOL && c.x < bmax.x - SLAB_EDGE_TOL
        && c.z > bmin.z + SLAB_EDGE_TOL && c.z < bmax.z - SLAB_EDGE_TOL;
}

// Row r's slab stands between p and the point q.
fn slab_segment_blocks(r: u32, p: vec3<f32>, q: vec3<f32>) -> bool {
    let m = (room_gi.rooms[r].bmin.y + room_gi.rooms[r].bmax.y) * 0.5;
    let dp = p.y - m;
    let dq = q.y - m;
    if (dp * dq >= 0.0) {
        return false;
    }
    // Opposite signs, so dp - dq is never 0.
    let t = dp / (dp - dq);
    return slab_rect_holds(r, p + (q - p) * t);
}

// Row r's slab stands between p and the sun along d (any length).
fn slab_ray_blocks(r: u32, p: vec3<f32>, d: vec3<f32>) -> bool {
    let m = (room_gi.rooms[r].bmin.y + room_gi.rooms[r].bmax.y) * 0.5;
    let dp = p.y - m;
    if (dp * d.y >= 0.0) {
        return false;
    }
    let t = -dp / d.y;
    return slab_rect_holds(r, p + d * t);
}

// The light loop's test: does any slab of the light's run (count rows from
// `first`) stand between p and the light at q?
fn slab_blocks_light(p: vec3<f32>, q: vec3<f32>, first: u32, count: u32) -> bool {
    let base = room_gi.header.info.x + room_gi.header.atlas.w + first;
    for (var k = 0u; k < count; k = k + 1u) {
        if (slab_segment_blocks(base + k, p, q)) {
            return true;
        }
    }
    return false;
}

// The sun's test: does any solid slab stand between p and the sun along d?
// Only within SLAB_SUN_REACH of the box around every room (the header's gmin
// and gmax, written whether or not room GI is on), so a planet's trees, seen
// from orbit, never walk the list.
fn slab_sun_blocked(p: vec3<f32>, d: vec3<f32>) -> bool {
    let h = room_gi.header;
    let reach = vec3<f32>(SLAB_SUN_REACH);
    if (any(p < h.gmin.xyz - reach) || any(p > h.gmax.xyz + reach)) {
        return false;
    }
    for (var k = 0u; k < h.atlas.w; k = k + 1u) {
        if (slab_ray_blocks(h.info.x + k, p, d)) {
            return true;
        }
    }
    return false;
}
