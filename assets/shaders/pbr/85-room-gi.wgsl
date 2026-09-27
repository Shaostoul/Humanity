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
    // x = atlas width, y = atlas height, z = first row of the depth region.
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
fn gi_irr_origin(probe: u32) -> vec2<u32> {
    let per = room_gi.header.info.z;
    return vec2<u32>(probe % per, probe / per) * GI_IRR_TILE;
}
fn gi_depth_origin(probe: u32) -> vec2<u32> {
    let per = room_gi.header.info.w;
    return vec2<u32>(probe % per, probe / per) * GI_DEPTH_TILE + vec2<u32>(0u, room_gi.header.atlas.z);
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
fn gi_sample_room(ri: u32, p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
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
        // Smooth backface weight: a probe behind the surface counts for little.
        let to_probe = ppos - p;
        let tp_len = length(to_probe);
        var bf = 0.5;
        if (tp_len > 1.0e-6) {
            bf = (dot(to_probe / tp_len, n) + 1.0) * 0.5;
        }
        var w = bf * bf + 0.2;
        // Chebyshev visibility from the probe's depth moments.
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
        w = max(w, 1.0e-6);
        if (w < GI_CRUSH) {
            w = w * w * w / (GI_CRUSH * GI_CRUSH);
        }
        w = w * tri;
        let irr = textureSampleLevel(room_gi_atlas, room_gi_samp, gi_atlas_uv(gi_irr_origin(probe), GI_IRR_N, n), 0.0).rgb;
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
    var best = -1;
    var best_d = GI_PICK_MARGIN + 1.0e-3;
    var best_v = 3.4e38;
    for (var i = 0u; i < h.info.x; i = i + 1u) {
        let bmin = room_gi.rooms[i].bmin.xyz;
        let bmax = room_gi.rooms[i].bmax.xyz;
        let d = length(max(max(bmin - q, q - bmax), vec3<f32>(0.0)));
        let e = bmax - bmin;
        let v = e.x * e.y * e.z;
        if (d < best_d - 1.0e-4 || (d <= best_d + 1.0e-4 && v < best_v)) {
            best = i32(i);
            best_d = d;
            best_v = v;
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
