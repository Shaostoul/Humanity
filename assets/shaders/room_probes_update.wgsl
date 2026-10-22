// ── ROOM GI, RUNG 1: THE PROBE UPDATE (docs/design/room-gi.md) ──
//
// Five compute entries in one pass, in this order (renderer::room_probes_gpu,
// `RoomGi::encode`):
//
//   cs_ambient          one workgroup per room: the room's AMBIENT COLOUR
//                       (room_probes::room_ambient) into the scratch's last
//                       row, every frame.
//   cs_ambient_resolve  copies that row into the atlas's last row, where the
//                       fragments read it (room_gi_ambient in 85-room-gi.wgsl).
//   cs_seed             one workgroup per probe of each room that came into
//                       the atlas this frame (probe streaming,
//                       room_probes::Residency): its tiles set to the room's
//                       ambient colour, so it starts exactly where the room
//                       already stood (room_probes::Twin::seed).
//   cs_trace, cs_resolve  one workgroup per probe being updated this frame
//                       (the camera's room every frame, the other resident
//                       rooms round-robin under a budget), below.
//
//   cs_trace    64 threads, one ray each. The ray set is a spherical Fibonacci
//               set turned by this update's random rotation, traced against
//               the ROOM'S OWN BOX analytically (rung 1: the room's contents
//               are not traced yet; rung 2 swaps this intersection for a
//               voxel trace and nothing else here changes). Each hit is shaded
//               as a Lambert surface lit by the room's own lights (the
//               fragment loop's attenuation exactly), the sun if the way to it
//               leaves through a glass lid, and the previous update's probes
//               for every bounce after the first. Then each thread folds the 64
//               rays into its irradiance texel and four depth texels, blends
//               them into the probe's previous values with hysteresis, and
//               writes the tile (with its octahedral border) to the SCRATCH
//               texture.
//   cs_resolve  copies each updated probe's two tiles from the scratch into
//               the atlas. Two passes because a probe's bounce reads its
//               NEIGHBOURS' previous values while they are being rewritten;
//               the atlas is read-only in the trace and write-only here.
//
// This file is compiled with 85-room-gi.wgsl in front of it (the structs, the
// octahedral map and gi_sample_room, shared with the megashader). The CPU twin
// is src/renderer/room_probes.rs (`Twin::update_probe`, `ray_radiance`),
// which is what the tests run. LOCKSTEP.

const GI_PI: f32 = 3.14159265;
const GI_RAYS: u32 = 64u;
const GI_DEPTH_SHARPNESS: f32 = 50.0;
const GI_DEPTH_MIN_COS: f32 = 0.9;
const GI_MAX_COUNT: f32 = 255.0;

// One scene light, the renderer's GpuLight packing (light::gpu_packed):
// pos_intensity = [pos, intensity], color_range = [rgb, range],
// spot = [aim or line end B, cos_outer (-1 point, -2 line)], cone_inner.x.
struct GiLight {
    pos_intensity: vec4<f32>,
    color_range: vec4<f32>,
    spot: vec4<f32>,
    cone_inner: vec4<f32>,
};

// room_probes::GiParamsGpu, 128 bytes.
struct GiParams {
    rot0: vec4<f32>,
    rot1: vec4<f32>,
    rot2: vec4<f32>,
    // xyz = toward the sun, w = sun intensity.
    sun_dir: vec4<f32>,
    // rgb = sun colour, w = 1 when the sky-view table was rendered this frame.
    sun_color: vec4<f32>,
    // xyz = local up for the sky table, w = the sky's lighting exposure.
    sky_up: vec4<f32>,
    // x = settled hysteresis, z = normal bias (unused here; the shared part
    // carries it as a constant), y and w unused.
    misc: vec4<f32>,
    // x = probes traced in this dispatch (gi_updates' first x entries; the
    // probes cs_seed seeds follow them), y = scratch irradiance tiles per row,
    // z = scratch depth tiles per row, w = first row of the scratch depth region.
    counts: vec4<u32>,
};

@group(0) @binding(0) var<uniform> gi_params: GiParams;
@group(0) @binding(1) var<storage, read> room_gi: GiRooms;
@group(0) @binding(2) var<storage, read> gi_lights: array<GiLight>;
// (global probe index, room index) per workgroup.
@group(0) @binding(3) var<storage, read> gi_updates: array<vec2<u32>>;
@group(0) @binding(4) var room_gi_atlas: texture_2d<f32>;
@group(0) @binding(5) var room_gi_samp: sampler;
@group(0) @binding(6) var gi_scratch_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(7) var gi_sky_tex: texture_2d<f32>;
@group(0) @binding(8) var gi_sky_samp: sampler;
// cs_resolve's pair: the scratch read back, the atlas written.
@group(0) @binding(10) var gi_scratch_in: texture_2d<f32>;
@group(0) @binding(11) var gi_atlas_out: texture_storage_2d<rgba16float, write>;

var<workgroup> wg_dir: array<vec3<f32>, 64>;
var<workgroup> wg_rad: array<vec3<f32>, 64>;
var<workgroup> wg_dist: array<f32, 64>;

// Ray i of the 64-ray spherical Fibonacci set (room_probes::fibonacci_ray).
fn gi_fibonacci_ray(i: u32) -> vec3<f32> {
    let phi = 2.0 * GI_PI * fract(f32(i) * 0.618034);
    let cos_t = 1.0 - (2.0 * f32(i) + 1.0) / f32(GI_RAYS);
    let sin_t = sqrt(max(1.0 - cos_t * cos_t, 0.0));
    return vec3<f32>(cos(phi) * sin_t, sin(phi) * sin_t, cos_t);
}

// Inward normal of face f (room_probes::face_normal): -x, +x, floor, lid, -z, +z.
fn gi_face_normal(f: u32) -> vec3<f32> {
    switch (f) {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 1u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 3u: { return vec3<f32>(0.0, -1.0, 0.0); }
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, 0.0, -1.0); }
    }
}

// Distance from p (inside the box) along d to the wall it hits, and which
// face (room_probes::trace_box). x = t, y = face.
fn gi_trace_box(p: vec3<f32>, d: vec3<f32>, bmin: vec3<f32>, bmax: vec3<f32>) -> vec2<f32> {
    var t = 3.4e38;
    var face = 2u;
    if (d.x > 1.0e-8) {
        let tx = (bmax.x - p.x) / d.x;
        if (tx < t) { t = tx; face = 1u; }
    } else if (d.x < -1.0e-8) {
        let tx = (bmin.x - p.x) / d.x;
        if (tx < t) { t = tx; face = 0u; }
    }
    if (d.y > 1.0e-8) {
        let ty = (bmax.y - p.y) / d.y;
        if (ty < t) { t = ty; face = 3u; }
    } else if (d.y < -1.0e-8) {
        let ty = (bmin.y - p.y) / d.y;
        if (ty < t) { t = ty; face = 2u; }
    }
    if (d.z > 1.0e-8) {
        let tz = (bmax.z - p.z) / d.z;
        if (tz < t) { t = tz; face = 5u; }
    } else if (d.z < -1.0e-8) {
        let tz = (bmin.z - p.z) / d.z;
        if (tz < t) { t = tz; face = 4u; }
    }
    return vec2<f32>(max(t, 0.0), f32(face));
}

// Irradiance the room's own lights deliver at h facing n: the fragment loop
// in 80-fragment-shared.wgsl without the BRDF (room_probes::light_irradiance).
fn gi_lights_irradiance(ri: u32, h: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let first = room_gi.rooms[ri].lights.x;
    let count = room_gi.rooms[ri].lights.y;
    var e = vec3<f32>(0.0);
    for (var k = 0u; k < count; k = k + 1u) {
        let l = gi_lights[first + k];
        var pos = l.pos_intensity.xyz;
        let sent = l.spot.w;
        if (sent < -1.5) {
            let ab = l.spot.xyz - pos;
            let t = clamp(dot(h - pos, ab) / max(dot(ab, ab), 1.0e-6), 0.0, 1.0);
            pos = pos + ab * t;
        }
        let to_light = pos - h;
        let dist = length(to_light);
        let radius = l.color_range.w;
        if (dist >= radius) {
            continue;
        }
        let ldir = to_light / max(dist, 0.001);
        var att = l.pos_intensity.w / (1.0 + dist * dist) * max(1.0 - dist / max(radius, 0.001), 0.0);
        if (sent > -1.0) {
            let cos_angle = dot(normalize(l.spot.xyz), -ldir);
            att = att * smoothstep(sent, l.cone_inner.x, cos_angle);
        }
        if (att > 0.001) {
            e = e + l.color_range.xyz * att * max(dot(n, ldir), 0.0);
        }
    }
    return e;
}

// The lid's transmittance if the way from h (facing n) to the sun leaves the
// box through a glass lid, else 0 (room_probes::sun_path).
fn gi_sun_path(ri: u32, h: vec3<f32>, n: vec3<f32>) -> f32 {
    let s = gi_params.sun_dir.xyz;
    let tr = room_gi.rooms[ri].bmax.w;
    if (tr <= 0.0 || s.y <= 0.0 || dot(n, s) <= 0.0) {
        return 0.0;
    }
    let hit = gi_trace_box(h + n * 1.0e-3, s, room_gi.rooms[ri].bmin.xyz, room_gi.rooms[ri].bmax.xyz);
    return select(0.0, tr, u32(hit.y) == 3u);
}

// Sky radiance along d, in lighting units, from this frame's sky-view table,
// or 0 when there is no table this frame (no atmosphere near the camera, e.g.
// a station in orbit, whose lid looks out at space). The mapping is
// water_sky_lut's in 20-surface-detail.wgsl without the sea-specific horizon
// dip. LOCKSTEP with it and with sky_view_lut.wgsl.
fn gi_sky(d: vec3<f32>) -> vec3<f32> {
    if (gi_params.sun_color.w < 0.5) {
        return vec3<f32>(0.0);
    }
    let up = gi_params.sky_up.xyz;
    let l = asin(clamp(dot(d, up), -1.0, 1.0));
    let v = 0.5 + 0.5 * sign(l) * sqrt(abs(l) / (GI_PI * 0.5));
    let s = normalize(gi_params.sun_dir.xyz);
    let sun_h = s - up * dot(s, up);
    let view_h = d - up * dot(d, up);
    var u = 0.25;
    if (length(sun_h) > 1.0e-4 && length(view_h) > 1.0e-4) {
        u = acos(clamp(dot(normalize(sun_h), normalize(view_h)), -1.0, 1.0)) / (2.0 * GI_PI);
    }
    return textureSampleLevel(gi_sky_tex, gi_sky_samp, vec2<f32>(u, v), 0.0).rgb * gi_params.sky_up.w;
}

// Where update slot `slot`'s tiles live in the scratch texture.
fn gi_scratch_irr(slot: u32) -> vec2<u32> {
    let per = gi_params.counts.y;
    return vec2<u32>(slot % per, slot / per) * GI_IRR_TILE;
}
fn gi_scratch_depth(slot: u32) -> vec2<u32> {
    let per = gi_params.counts.z;
    return vec2<u32>(slot % per, slot / per) * GI_DEPTH_TILE + vec2<u32>(0u, gi_params.counts.w);
}

// Write interior texel (x, y) of an n x n tile, and every border texel that
// mirrors it (room_probes::border_source, inverted): a texel on an edge is
// also shown across the octahedral seam, a corner texel at the opposite corner.
fn gi_write_tile_texel(origin: vec2<u32>, n: u32, x: u32, y: u32, v: vec4<f32>) {
    let o = vec2<i32>(origin);
    let xi = i32(x);
    let yi = i32(y);
    let ni = i32(n);
    textureStore(gi_scratch_out, o + vec2<i32>(xi + 1, yi + 1), v);
    if (y == 0u) { textureStore(gi_scratch_out, o + vec2<i32>(ni - xi, 0), v); }
    if (y == n - 1u) { textureStore(gi_scratch_out, o + vec2<i32>(ni - xi, ni + 1), v); }
    if (x == 0u) { textureStore(gi_scratch_out, o + vec2<i32>(0, ni - yi), v); }
    if (x == n - 1u) { textureStore(gi_scratch_out, o + vec2<i32>(ni + 1, ni - yi), v); }
    if (x == 0u && y == 0u) { textureStore(gi_scratch_out, o + vec2<i32>(ni + 1, ni + 1), v); }
    if (x == n - 1u && y == 0u) { textureStore(gi_scratch_out, o + vec2<i32>(0, ni + 1), v); }
    if (x == 0u && y == n - 1u) { textureStore(gi_scratch_out, o + vec2<i32>(ni + 1, 0), v); }
    if (x == n - 1u && y == n - 1u) { textureStore(gi_scratch_out, o + vec2<i32>(0, 0), v); }
}

@compute @workgroup_size(64)
fn cs_trace(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let slot = wg.x;
    // Uniform per workgroup (slot and the count both are), so the barrier
    // below stays in uniform control flow.
    if (slot >= gi_params.counts.x) {
        return;
    }
    let entry = gi_updates[slot];
    let probe = entry.x;
    let ri = entry.y;
    let bmin = room_gi.rooms[ri].bmin.xyz;
    let bmax = room_gi.rooms[ri].bmax.xyz;
    let counts = room_gi.rooms[ri].counts;
    let local = probe - counts.w;
    let grid = vec3<u32>(local % counts.x, (local / counts.x) % counts.y, local / (counts.x * counts.y));
    let cell = (bmax - bmin) / vec3<f32>(counts.xyz);
    let ppos = bmin + (vec3<f32>(grid) + vec3<f32>(0.5)) * cell;

    // ── One ray per thread ──
    let rot = mat3x3<f32>(gi_params.rot0.xyz, gi_params.rot1.xyz, gi_params.rot2.xyz);
    // The rows were uploaded, so multiply as row-vector-times-matrix
    // (transpose of the column-major product) to get rot * v.
    let d = normalize(gi_fibonacci_ray(li) * rot);
    let hit = gi_trace_box(ppos, d, bmin, bmax);
    let t = hit.x;
    let face = u32(hit.y);
    let h = ppos + d * t;
    let nrm = gi_face_normal(face);
    let refl = room_gi.rooms[ri].refl[face].rgb;
    var e = gi_lights_irradiance(ri, h, nrm);
    e = e + gi_params.sun_color.rgb * gi_params.sun_dir.w * max(dot(nrm, gi_params.sun_dir.xyz), 0.0)
        * gi_sun_path(ri, h, nrm);
    var rad = refl / GI_PI * e + refl * gi_sample_room(ri, h, nrm);
    // A ray that meets a glass lid also brings back the sky through it.
    if (face == 3u) {
        rad = rad + room_gi.rooms[ri].bmax.w * gi_sky(d);
    }
    wg_dir[li] = d;
    wg_rad[li] = rad;
    wg_dist[li] = t;
    workgroupBarrier();

    // The probe's update count rides in the alpha of its irradiance texels.
    let io = gi_irr_origin(probe);
    let count = textureLoad(room_gi_atlas, vec2<i32>(io) + vec2<i32>(1, 1), 0).a;
    // Running mean for a fresh probe, then the settled hysteresis
    // (room_probes::blend_weight_h).
    let a = 1.0 - min(count / (count + 1.0), gi_params.misc.x);
    let next_count = min(count + 1.0, GI_MAX_COUNT);

    // ── Irradiance: this thread's texel of the 8 x 8 interior ──
    {
        let tx = li % 8u;
        let ty = li / 8u;
        let td = gi_oct_decode((vec2<f32>(f32(tx), f32(ty)) + vec2<f32>(0.5)) / GI_IRR_N * 2.0 - vec2<f32>(1.0));
        var acc = vec3<f32>(0.0);
        var ws = 0.0;
        for (var r = 0u; r < GI_RAYS; r = r + 1u) {
            let w = max(dot(td, wg_dir[r]), 0.0);
            acc = acc + wg_rad[r] * w;
            ws = ws + w;
        }
        let prev = textureLoad(room_gi_atlas, vec2<i32>(io) + vec2<i32>(i32(tx) + 1, i32(ty) + 1), 0).rgb;
        var est = prev;
        if (ws > 1.0e-6) {
            est = acc / ws;
        }
        let v = prev + (est - prev) * a;
        gi_write_tile_texel(gi_scratch_irr(slot), 8u, tx, ty, vec4<f32>(v, next_count));
    }

    // ── Depth moments: four texels of the 16 x 16 interior per thread ──
    let dg = gi_depth_origin(probe);
    for (var j = 0u; j < 4u; j = j + 1u) {
        let k = li + j * 64u;
        let tx = k % 16u;
        let ty = k / 16u;
        let td = gi_oct_decode((vec2<f32>(f32(tx), f32(ty)) + vec2<f32>(0.5)) / GI_DEPTH_N * 2.0 - vec2<f32>(1.0));
        var acc = vec2<f32>(0.0);
        var ws = 0.0;
        for (var r = 0u; r < GI_RAYS; r = r + 1u) {
            let c = dot(td, wg_dir[r]);
            if (c < GI_DEPTH_MIN_COS) {
                continue;
            }
            let w = pow(c, GI_DEPTH_SHARPNESS);
            let dr = wg_dist[r];
            acc = acc + vec2<f32>(dr, dr * dr) * w;
            ws = ws + w;
        }
        let prev = textureLoad(room_gi_atlas, vec2<i32>(dg) + vec2<i32>(i32(tx) + 1, i32(ty) + 1), 0).xy;
        var est = prev;
        if (ws > 1.0e-6) {
            est = acc / ws;
        }
        let v = prev + (est - prev) * a;
        gi_write_tile_texel(gi_scratch_depth(slot), 16u, tx, ty, vec4<f32>(v, 0.0, 0.0));
    }
}

// ── THE ROOM'S AMBIENT COLOUR (room_probes::room_ambient, its section
//    comment says why and how) ──
//
// LOCKSTEP with room_probes.rs: AMBIENT_GRID, AMBIENT_SKY_DIRS, SEED_COUNT,
// face_area, face_cell_point, sky_dir, ambient_closed_form
// (room_probes_gpu::tests checks the constants).
const GI_AMB_GRID: u32 = 8u;
const GI_AMB_CELLS: u32 = 64u;
const GI_AMB_SKY_DIRS: u32 = 64u;
const GI_SEED_COUNT: f32 = 8.0;

// Area of face f of the box (room_probes::face_area).
fn gi_face_area(bmin: vec3<f32>, bmax: vec3<f32>, f: u32) -> f32 {
    let s = bmax - bmin;
    if (f < 2u) {
        return s.y * s.z;
    }
    if (f < 4u) {
        return s.x * s.z;
    }
    return s.x * s.y;
}

// Centre of cell `cell` (x fastest) of face f's GI_AMB_GRID x GI_AMB_GRID grid
// (room_probes::face_cell_point).
fn gi_amb_cell_point(bmin: vec3<f32>, bmax: vec3<f32>, f: u32, cell: u32) -> vec3<f32> {
    let u = (f32(cell % GI_AMB_GRID) + 0.5) / f32(GI_AMB_GRID);
    let v = (f32(cell / GI_AMB_GRID) + 0.5) / f32(GI_AMB_GRID);
    let d = bmax - bmin;
    switch (f) {
        case 0u: { return vec3<f32>(bmin.x, bmin.y + d.y * u, bmin.z + d.z * v); }
        case 1u: { return vec3<f32>(bmax.x, bmin.y + d.y * u, bmin.z + d.z * v); }
        case 2u: { return vec3<f32>(bmin.x + d.x * u, bmin.y, bmin.z + d.z * v); }
        case 3u: { return vec3<f32>(bmin.x + d.x * u, bmax.y, bmin.z + d.z * v); }
        case 4u: { return vec3<f32>(bmin.x + d.x * u, bmin.y + d.y * v, bmin.z); }
        default: { return vec3<f32>(bmin.x + d.x * u, bmin.y + d.y * v, bmax.z); }
    }
}

// Sky direction i: the cosine-weighted Fibonacci spiral over the upper
// hemisphere, +y up (room_probes::sky_dir).
fn gi_amb_sky_dir(i: u32) -> vec3<f32> {
    let s2 = (f32(i) + 0.5) / f32(GI_AMB_SKY_DIRS);
    let s = sqrt(s2);
    let phi = 2.0 * GI_PI * fract(f32(i) * 0.618034);
    return vec3<f32>(cos(phi) * s, sqrt(max(1.0 - s2, 0.0)), sin(phi) * s);
}

// Howell's catalogue C-11: equal, directly opposed parallel rectangles a x b,
// c apart (room_probes::ff_parallel).
fn gi_ff_parallel(a: f32, b: f32, c: f32) -> f32 {
    let x = a / max(c, 1.0e-6);
    let y = b / max(c, 1.0e-6);
    let x2 = x * x;
    let y2 = y * y;
    let sx = sqrt(1.0 + x2);
    let sy = sqrt(1.0 + y2);
    let t = 0.5 * log((1.0 + x2) * (1.0 + y2) / (1.0 + x2 + y2)) + x * sy * atan(x / sy) + y * sx * atan(y / sx)
        - x * atan(x) - y * atan(y);
    return max(2.0 / (GI_PI * x * y) * t, 0.0);
}

// Howell's catalogue C-14: from a rectangle l x w to a perpendicular one l x h
// sharing its edge l (room_probes::ff_perpendicular).
fn gi_ff_perpendicular(l_in: f32, w: f32, h: f32) -> f32 {
    let l = max(l_in, 1.0e-6);
    let ww = w / l;
    let hh = h / l;
    let w2 = ww * ww;
    let h2 = hh * hh;
    let r = sqrt(w2 + h2);
    let atans = ww * atan(1.0 / ww) + hh * atan(1.0 / hh) - r * atan(1.0 / r);
    let logs = log((1.0 + w2) * (1.0 + h2) / (1.0 + w2 + h2))
        + w2 * log(w2 * (1.0 + w2 + h2) / ((1.0 + w2) * (w2 + h2)))
        + h2 * log(h2 * (1.0 + w2 + h2) / ((1.0 + h2) * (w2 + h2)));
    return max((atans + 0.25 * logs) / (GI_PI * ww), 0.0);
}

// The form factor from face i to face j of a box of `size`, before the row is
// held to sum to 1 (room_probes::box_form_factor).
fn gi_box_form_factor(size: vec3<f32>, i: u32, j: u32) -> f32 {
    let ki = i / 2u;
    let kj = j / 2u;
    if (i == j) {
        return 0.0;
    }
    if (ki == kj) {
        return gi_ff_parallel(size[(ki + 1u) % 3u], size[(ki + 2u) % 3u], size[ki]);
    }
    return gi_ff_perpendicular(size[3u - ki - kj], size[kj], size[ki]);
}

// The six faces' direct flux and the sky's mean radiance, summed by the
// workgroup: wg_amb[k * 64] for face k, wg_amb[6 * 64] for the sky.
var<workgroup> wg_amb: array<vec3<f32>, 448>;

// THE ROOM'S AMBIENT COLOUR from the summed flux (room_probes::ambient_from_faces):
// the six-face radiosity solve, then the area-weighted mean over the opaque
// faces of the light arriving indirectly, as E / pi. Thread 0 only.
fn gi_ambient_from_faces(ri: u32, sky_mean: vec3<f32>) -> vec3<f32> {
    let bmin = room_gi.rooms[ri].bmin.xyz;
    let bmax = room_gi.rooms[ri].bmax.xyz;
    let size = bmax - bmin;
    let lid_t = room_gi.rooms[ri].bmax.w;
    var ff: array<array<f32, 6>, 6>;
    for (var i = 0u; i < 6u; i = i + 1u) {
        var sum = 0.0;
        for (var j = 0u; j < 6u; j = j + 1u) {
            ff[i][j] = gi_box_form_factor(size, i, j);
            sum = sum + ff[i][j];
        }
        if (sum > 0.0) {
            for (var j = 0u; j < 6u; j = j + 1u) {
                ff[i][j] = ff[i][j] / sum;
            }
        }
    }
    var m: array<array<vec3<f32>, 6>, 6>;
    var rhs: array<vec3<f32>, 6>;
    for (var i = 0u; i < 6u; i = i + 1u) {
        let rho = room_gi.rooms[ri].refl[i].rgb;
        let area = max(gi_face_area(bmin, bmax, i), 1.0e-6);
        for (var j = 0u; j < 6u; j = j + 1u) {
            m[i][j] = select(vec3<f32>(0.0), vec3<f32>(1.0), i == j) - rho * ff[i][j];
        }
        var sky = vec3<f32>(0.0);
        if (i == 3u) {
            sky = sky_mean * GI_PI * lid_t;
        }
        rhs[i] = sky + rho * wg_amb[i * 64u] / area;
    }
    for (var k = 0u; k < 6u; k = k + 1u) {
        for (var i = k + 1u; i < 6u; i = i + 1u) {
            let q = m[i][k] / m[k][k];
            for (var j = k; j < 6u; j = j + 1u) {
                m[i][j] = m[i][j] - q * m[k][j];
            }
            rhs[i] = rhs[i] - q * rhs[k];
        }
    }
    var b: array<vec3<f32>, 6>;
    for (var ii = 0u; ii < 6u; ii = ii + 1u) {
        let i = 5u - ii;
        var s = rhs[i];
        for (var j = i + 1u; j < 6u; j = j + 1u) {
            s = s - m[i][j] * b[j];
        }
        b[i] = s / m[i][i];
    }
    var sum = vec3<f32>(0.0);
    var area = 0.0;
    for (var i = 0u; i < 6u; i = i + 1u) {
        if (i == 3u && lid_t > 0.0) {
            continue;
        }
        var incident = vec3<f32>(0.0);
        for (var j = 0u; j < 6u; j = j + 1u) {
            incident = incident + b[j] * ff[i][j];
        }
        let a = gi_face_area(bmin, bmax, i);
        sum = sum + incident * a;
        area = area + a;
    }
    if (area <= 1.0e-6) {
        return vec3<f32>(0.0);
    }
    return sum / (area * GI_PI);
}

// One workgroup per room (the dispatch is exactly the room count, so no bound
// test, and the barriers stay in uniform control flow): thread li takes cell
// li of each of the six faces and sky direction li, the workgroup sums them
// face by face, and thread 0 solves the six faces' radiosity and writes the
// room's colour into the scratch's last row at the room's own index. The
// direct light at a cell is exactly cs_trace's at a hit (the room's own
// lights, the sun through a glass lid).
@compute @workgroup_size(64)
fn cs_ambient(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let ri = wg.x;
    let bmin = room_gi.rooms[ri].bmin.xyz;
    let bmax = room_gi.rooms[ri].bmax.xyz;
    let sun = gi_params.sun_dir.xyz;
    let sun_e = gi_params.sun_color.rgb * gi_params.sun_dir.w;
    for (var f = 0u; f < 6u; f = f + 1u) {
        let h = gi_amb_cell_point(bmin, bmax, f, li);
        let n = gi_face_normal(f);
        var e = gi_lights_irradiance(ri, h, n);
        e = e + sun_e * max(dot(n, sun), 0.0) * gi_sun_path(ri, h, n);
        wg_amb[f * 64u + li] = e * (gi_face_area(bmin, bmax, f) / f32(GI_AMB_CELLS));
    }
    var sky = vec3<f32>(0.0);
    if (room_gi.rooms[ri].bmax.w > 0.0) {
        sky = gi_sky(gi_amb_sky_dir(li));
    }
    wg_amb[384u + li] = sky;
    workgroupBarrier();
    for (var s = 32u; s > 0u; s = s >> 1u) {
        if (li < s) {
            for (var k = 0u; k < 7u; k = k + 1u) {
                wg_amb[k * 64u + li] = wg_amb[k * 64u + li] + wg_amb[k * 64u + li + s];
            }
        }
        workgroupBarrier();
    }
    if (li == 0u) {
        let amb = gi_ambient_from_faces(ri, wg_amb[384] / f32(GI_AMB_SKY_DIRS));
        let row = i32(textureDimensions(gi_scratch_out).y) - 1;
        textureStore(gi_scratch_out, vec2<i32>(i32(room_gi.rooms[ri].lights.w), row), vec4<f32>(amb, 1.0));
    }
}

// The scratch's ambient row into the atlas's (texel x = the room's index, for
// every room). Its own pass because the atlas is read in cs_ambient's bind
// group and can only be written in this one's.
@compute @workgroup_size(64)
fn cs_ambient_resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= room_gi.header.info.x) {
        return;
    }
    let src = vec2<i32>(i32(id.x), i32(textureDimensions(gi_scratch_in).y) - 1);
    let dst = vec2<i32>(i32(id.x), i32(textureDimensions(gi_atlas_out).y) - 1);
    textureStore(gi_atlas_out, dst, textureLoad(gi_scratch_in, src, 0));
}

// Which interior texel tile texel (bx, by), border included, shows: itself
// inside, across the octahedral seam on the border (room_probes::border_source).
fn gi_border_source(n: u32, bx: u32, by: u32) -> vec2<u32> {
    let last = n + 1u;
    if (bx == 0u && by == 0u) { return vec2<u32>(n - 1u, n - 1u); }
    if (bx == last && by == 0u) { return vec2<u32>(0u, n - 1u); }
    if (bx == 0u && by == last) { return vec2<u32>(n - 1u, 0u); }
    if (bx == last && by == last) { return vec2<u32>(0u, 0u); }
    if (by == 0u) { return vec2<u32>(n - bx, 0u); }
    if (by == last) { return vec2<u32>(n - bx, n - 1u); }
    if (bx == 0u) { return vec2<u32>(0u, n - by); }
    if (bx == last) { return vec2<u32>(n - 1u, n - by); }
    return vec2<u32>(bx - 1u, by - 1u);
}

// SEED one probe of a room that came into the atlas this frame
// (room_probes::Twin::seed): every irradiance texel (border included) the
// room's ambient colour, with GI_SEED_COUNT as its update count, and every
// depth texel the exact distance to the room's box along its direction and
// its square (a box-only room's depth moments). The entry is gi_updates'
// (counts.x + workgroup)th: the traced probes come first in the list. The
// room's colour is read from the scratch's ambient row, written by cs_ambient
// earlier in this same pass, so the seed is this frame's colour exactly.
@compute @workgroup_size(64)
fn cs_seed(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let entry = gi_updates[gi_params.counts.x + wg.x];
    let probe = entry.x;
    let ri = entry.y;
    let bmin = room_gi.rooms[ri].bmin.xyz;
    let bmax = room_gi.rooms[ri].bmax.xyz;
    let counts = room_gi.rooms[ri].counts;
    let local = probe - counts.w;
    let grid = vec3<u32>(local % counts.x, (local / counts.x) % counts.y, local / (counts.x * counts.y));
    let cell = (bmax - bmin) / vec3<f32>(counts.xyz);
    let ppos = bmin + (vec3<f32>(grid) + vec3<f32>(0.5)) * cell;
    let amb_at = vec2<i32>(i32(room_gi.rooms[ri].lights.w), i32(textureDimensions(gi_scratch_in).y) - 1);
    let amb = textureLoad(gi_scratch_in, amb_at, 0).rgb;
    let ai = vec2<i32>(gi_irr_origin(probe));
    let ad = vec2<i32>(gi_depth_origin(probe));
    let irr_texels = GI_IRR_TILE * GI_IRR_TILE;
    let total = irr_texels + GI_DEPTH_TILE * GI_DEPTH_TILE;
    for (var k = li; k < total; k = k + 64u) {
        if (k < irr_texels) {
            let c = vec2<i32>(i32(k % GI_IRR_TILE), i32(k / GI_IRR_TILE));
            textureStore(gi_atlas_out, ai + c, vec4<f32>(amb, GI_SEED_COUNT));
        } else {
            let kd = k - irr_texels;
            let b = vec2<u32>(kd % GI_DEPTH_TILE, kd / GI_DEPTH_TILE);
            let s = gi_border_source(16u, b.x, b.y);
            let td = gi_oct_decode((vec2<f32>(s) + vec2<f32>(0.5)) / GI_DEPTH_N * 2.0 - vec2<f32>(1.0));
            let t = gi_trace_box(ppos, td, bmin, bmax).x;
            textureStore(gi_atlas_out, ad + vec2<i32>(b), vec4<f32>(t, t * t, 0.0, 0.0));
        }
    }
}

@compute @workgroup_size(64)
fn cs_resolve(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let slot = wg.x;
    if (slot >= gi_params.counts.x) {
        return;
    }
    let probe = gi_updates[slot].x;
    let si = vec2<i32>(gi_scratch_irr(slot));
    let sd = vec2<i32>(gi_scratch_depth(slot));
    let ai = vec2<i32>(gi_irr_origin(probe));
    let ad = vec2<i32>(gi_depth_origin(probe));
    let irr_texels = GI_IRR_TILE * GI_IRR_TILE;
    let total = irr_texels + GI_DEPTH_TILE * GI_DEPTH_TILE;
    for (var k = li; k < total; k = k + 64u) {
        if (k < irr_texels) {
            let c = vec2<i32>(i32(k % GI_IRR_TILE), i32(k / GI_IRR_TILE));
            textureStore(gi_atlas_out, ai + c, textureLoad(gi_scratch_in, si + c, 0));
        } else {
            let kd = k - irr_texels;
            let c = vec2<i32>(i32(kd % GI_DEPTH_TILE), i32(kd / GI_DEPTH_TILE));
            textureStore(gi_atlas_out, ad + c, textureLoad(gi_scratch_in, sd + c, 0));
        }
    }
}
