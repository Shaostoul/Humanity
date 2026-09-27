// ── ROOM GI, RUNG 1: THE PROBE UPDATE (docs/design/room-gi.md) ──
//
// Three compute entries in one pass: `cs_clear` once after the rooms change,
// then `cs_trace` and `cs_resolve` back to back with one workgroup per probe
// being updated this frame (renderer::room_probes_gpu builds the list: the
// camera's room every frame, the rest of the ship round-robin under a budget).
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
    // x = probes in this dispatch, y = scratch irradiance tiles per row,
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
    var rad = refl / GI_PI * e + refl * gi_sample_room(ri, h, nrm, true);
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

// Zero the whole atlas: run once after the rooms change, so every probe starts
// "never updated" (count 0, which the blend reads as "take the new estimate
// whole") instead of inheriting whatever the old layout kept in its tile.
@compute @workgroup_size(8, 8)
fn cs_clear(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(gi_atlas_out);
    if (id.x < size.x && id.y < size.y) {
        textureStore(gi_atlas_out, vec2<i32>(id.xy), vec4<f32>(0.0));
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
