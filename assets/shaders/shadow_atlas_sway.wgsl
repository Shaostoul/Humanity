// THE SHADOW ATLAS'S GPU-DRIVEN SWAYING CASTERS (2026-11-14, rung 4; renderer/shadow_gpu.rs;
// docs/design/local-light-shadows.md section 10, "Built: the film pyramid as one compute pass, the
// swaying leaves and the middle tier on the GPU").
//
// The plants that sway in the wind (the photoscanned leaves, type 19 with its wind opt-in in
// params.w; the procedural plant meshes, 20; the cluster cards that are a clustered species'
// crown, 21; the baked bark, 22) move their vertices in the megashader's vertex stage, so until
// this file every one of them was a draw of its own through vs_main and fs_shadow, about 0.7
// microseconds of the processor each: 214 of the hall's 230 per-draw casters. Here a sway set (one
// material: its albedo bound at group 1) is drawn by ONE indirect command a shadow face, from the
// instances the cull writes (shadow_cull.wgsl, the same 80-byte instance every set reads): the
// caster's model matrix with the view's index in column 0's w, and its extra (x the material type,
// y the wind class, z the LOD crossfade that rides column 0's w in the megashader's object uniform).
//
// THE WIND IS vs_main's, LOCKSTEP: `wind_displace` below is the vertex stage's own function
// (00-bindings-vertex.wgsl) copied verbatim, and `the_sway_sets_wind_is_vs_mains` compares the two
// texts. It reads `camera.light7_cone_inner` (the live wind) and `vs_model()` / `vs_rot()`, so this
// file defines those three names for it: `camera` holds the wind and the clock the per-draw loop's
// view uniform holds (the same `lit_uniform` values, written each frame), `vs_model()` the
// caster's model matrix, `vs_rot()` its normal matrix (the inverse transpose; exact for the
// translate-rotate-scale matrices every caster is, column i over its squared length).
//
// The fragment test is fs_shadow's (90-fragment-main.wgsl): the LOD crossfade dither and the type
// 19 and type 21 alpha cuts, held equal to it by the same test. Types 20 and 22 cut nothing, and the
// per-draw loop draws them without a fragment stage, so they take neither the dither nor a cut.

@group(0) @binding(0) var<storage, read> view_proj: array<mat4x4<f32>>;
@group(1) @binding(0) var albedo_texture: texture_2d<f32>;
@group(1) @binding(1) var albedo_sampler: sampler;

// The two numbers `wind_displace` reads from the camera uniform, under the camera's own field names.
struct SwayCamera {
    // xyz the world wind direction, w its speed in m/s (zero: the fallback breeze).
    light7_cone_inner: vec4<f32>,
    // w the wind's clock in seconds.
    sun_color: vec4<f32>,
};
@group(2) @binding(0) var<uniform> camera: SwayCamera;

var<private> g_model: mat4x4<f32>;

fn vs_model() -> mat4x4<f32> {
    return g_model;
}

// The normal matrix (inverse transpose of the model's 3 x 3): for M = R * S its columns are M's own
// columns, each over its squared length, so transpose(vs_rot()) * w is M^-1 * w, as vs_main has it.
fn vs_rot() -> mat4x4<f32> {
    let c0 = g_model[0].xyz;
    let c1 = g_model[1].xyz;
    let c2 = g_model[2].xyz;
    return mat4x4<f32>(
        vec4<f32>(c0 / max(dot(c0, c0), 1.0e-12), 0.0),
        vec4<f32>(c1 / max(dot(c1, c1), 1.0e-12), 0.0),
        vec4<f32>(c2 / max(dot(c2, c2), 1.0e-12), 0.0),
        vec4<f32>(0.0, 0.0, 0.0, 1.0));
}

fn wind_displace(vpos: vec3<f32>, vuv_x: f32, grass_phase: f32, is_grass: bool, wind_class: f32, t: f32) -> vec3<f32> {
    // ── PER-MESH HEIGHT NORMALISATION (v0.1089) ──────────────────────
    // Everything below is written in METRES, and the cantilever profile
    // `hn` divides by a 12 m reference. Object-space height is metres for
    // a procedural tree (built at t.height_m, instance scale ~1) but MODEL
    // UNITS for a photoscan: the fir/pine meshes are 0.70-1.27 units tall
    // and are scaled 15.1-31.4x at draw time (TREE_MODEL_H in lib.rs). Raw
    // object height would therefore ask a 22 m fir to bend like a 1.27 m
    // shrub - about 11% of the intended lean, i.e. visually nothing.
    //
    // `length(obj_model()[0].xyz)` is the instance's uniform scale (every
    // tree is pushed with Vec3::splat), so h * s is a metric height valid
    // for BOTH mesh paths, and dividing the object-space displacement back
    // by s makes the WORLD displacement depend only on metres. No new
    // per-material data, no vertex channel, no wiring.
    let iscale = max(length(vs_model()[0].xyz), 1.0e-4);
    let h = max(vpos.y, 0.0) * iscale;
    // Reference height for the cantilever profile. A tree bends against a
    // 12 m stand; a 35 cm grass blade IS the whole cantilever, so its own
    // height is the reference - otherwise `hn` sits on its 0.1 floor and
    // the sward is rigid in a gale.
    let wind_ref_m = select(12.0, iscale, is_grass);
    // ── LIVE WIND INPUT (v0.1079) ────────────────────────────────────
    // light7_cone_inner = (wind_dir_world.xyz, wind_speed_m_s), poked by
    // the renderer from WeatherState. The light0..7 cone_inner fields are
    // LEGACY (see the struct comment above: real scene lights moved to the
    // uncapped storage buffer in v0.782), which is why light4/5/6 already
    // carry the ocean anchor, sea crest and sea sphere. light7 was the last
    // free one.
    //
    // THE POKE MUST LIVE BESIDE THE SEA POKES in the CELESTIAL-pass
    // uniform stamp (renderer/mod.rs, the block that re-writes offsets
    // 528/544/560 after celestial_uniforms() has stamped the whole
    // buffer). Planet-surface trees are pushed as celestial_objects, so a
    // poke made only against the main-pass stamp never reaches them.
    //
    // FALLBACK, deliberately not a zero: an unwritten or zero slot means
    // "no publisher yet / weather off", and a zero wind speed would freeze
    // every plant on the planet - strictly worse than what shipped before.
    // A 4 m/s default breeze reproduces the pre-v0.1079 look.
    var wind_w = camera.light7_cone_inner.xyz;
    var wind_v = camera.light7_cone_inner.w;
    if (dot(wind_w, wind_w) < 0.25 || wind_v <= 0.0) {
        wind_w = vec3<f32>(0.86, 0.0, 0.32);
        wind_v = 4.0;
    }
    wind_w = normalize(wind_w);
    // World wind -> OBJECT space. transpose(normal_matrix) is model^-1 for
    // the rotation + uniform-scale transforms plants use (the same
    // identity the water branch uses below and the leaf-detail branch in
    // 90-fragment-main uses). This is the fix for the fanning stand: the
    // displacement is applied BEFORE obj_model(), so undoing the instance
    // yaw here is exactly what makes every tree in a stand lean the SAME
    // compass direction. Before this the wind vector was a CONSTANT in
    // object space, so each tree's random yaw (planet_chunks.rs, `az`)
    // rotated it and neighbours swayed in opposite directions.
    //
    // Object +Y IS the trunk axis by construction, so zeroing it leaves a
    // purely tangential lean: trees bend across the ground, never into or
    // out of it, at any latitude.
    var wind_dir = (transpose(vs_rot()) * vec4<f32>(wind_w, 0.0)).xyz;
    wind_dir.y = 0.0;
    let wl = length(wind_dir);
    if (wl < 1.0e-4) {
        // Wind blowing straight down the trunk axis (only at a pole or on
        // a tipped-over instance): pick any tangent rather than NaN.
        wind_dir = vec3<f32>(1.0, 0.0, 0.0);
    } else {
        wind_dir = wind_dir / wl;
    }
    // Per-plant phase so a stand does not sway in lockstep.
    //
    // Built to be EXACTLY 64 m-PERIODIC in both horizontal axes. The
    // translation below is render space, which re-snaps as the floating
    // origin rebases; an arbitrary phase would jump at every rebase and
    // pop the whole forest. With integer harmonics of TAU/64 a rebase by a
    // multiple of 64 m shifts the phase by a whole number of periods, so
    // the sway is continuous across it. Same discipline the ocean chop
    // trains use (CLAUDE.md, f32-at-planet-scale).
    let o = vs_model()[3].xyz;
    let k = 6.283185307 / 64.0;
    // Grass adds its OWN phase from the planet-fixed stream. The
    // positional term alone spans 0.29 rad/m, and tillers stand 15 cm
    // apart at peak density, so a whole clump would sway in lockstep -
    // which is what a stiff mat looks like, not a sward. The stream phase
    // is a per-tiller constant, so it survives every floating-origin
    // rebase exactly like the 64 m-periodic term does.
    let phase = o.x * k * 3.0 + o.z * k * 5.0
        + select(0.0, grass_phase, is_grass);
    // TRAVELLING GUST FRONT. A gust is a spatial cell advecting downwind
    // at roughly the mean wind speed (the visible wave that crosses a
    // canopy), not a standing pulse - the old envelope used a
    // position-derived phase with no time-space coupling, so it throbbed
    // in place.
    //
    // The travel direction is SNAPPED to integer harmonics of TAU/64 for
    // the same reason the per-plant phase is: an arbitrary dot(o, wind)
    // is not 64 m-periodic and every floating-origin rebase would pop the
    // whole forest. Rounding the horizontal wind to a x2 integer lattice
    // gives a 32-64 m gust wavelength and an angular resolution far finer
    // than anyone can read off a canopy.
    var nz = round(vec2<f32>(wind_w.x, wind_w.z) * 2.0);
    if (abs(nz.x) + abs(nz.y) < 0.5) {
        nz = vec2<f32>(1.0, 0.0);
    }
    let kv = nz * k;
    // Phase = K.x - |K| v t: the crests move downwind at the wind speed.
    let travel = o.x * kv.x + o.z * kv.y - length(kv) * wind_v * t;
    let gust = 0.65 + 0.35 * sin(travel);
    // STATIC LEAN. Drag on a crown goes as v^2, so a stand holds a mean
    // downwind deflection proportional to the square of the wind speed and
    // oscillates ABOUT that lean; at Beaufort 8 (17-20 m/s) the textbook
    // description is "whole trees in motion". A zero-mean sway, which is
    // all this branch had, renders a hurricane as a calm day.
    //
    // hn is a cantilever profile: zero at the foot, growing to the tip, and
    // normalised against a 12 m reference so a 22 m fir does not fold flat
    // while an 8 m cherry barely moves. lean_frac is capped at 0.30 so even
    // a 25 m/s hurricane roll bends the stand hard without laying it down.
    //
    // GRASS (type 23) uses the same shape with its own two numbers, and
    // both have to move together. A 35 cm blade is a slender cantilever
    // with almost no section modulus: at 10 m/s it lies well over, where
    // a 20 m fir has barely started. Raising only the CAP would change
    // nothing - 6.0e-4 * 10^2 is 0.06, so the tree curve never reaches
    // even the tree cap at grass-relevant speeds. So the coefficient goes
    // up 10x AND the cap goes to 0.85: 4 m/s (the fallback breeze) leans
    // ~10%, 10 m/s leans ~60%, and anything past ~12 m/s pins at 0.85,
    // which is laid over without folding the blade through the ground.
    let v2 = wind_v * wind_v;
    let lean_frac = select(
        min(6.0e-4 * v2, 0.30),
        min(6.0e-3 * v2, 0.85),
        is_grass,
    );
    let hn = clamp(h / wind_ref_m, 0.1, 1.0);
    let lean_m = h * hn * lean_frac;
    // Sway about the lean: a wind-scaled term plus a small calm-air term,
    // so a still day still breathes instead of freezing. Grass ripples at
    // roughly twice a tree's period - the visible wave crossing a meadow
    // is a fast one.
    let sway_amp = h * (0.020 + 0.55 * hn * lean_frac);
    let sway_hz = select(0.9 + 0.02 * wind_v, 1.9 + 0.06 * wind_v, is_grass);
    let sway = sway_amp * sin(t * sway_hz + phase);
    // Metres of displacement -> OBJECT units (see iscale above).
    let obj_per_m = 1.0 / iscale;
    var wind_pos = vpos
        + wind_dir * ((lean_m + sway) * gust * obj_per_m);
    // Leaf flutter: blades only (organ bit 19), faster and smaller, partly
    // across the wind so foliage shimmers rather than shunting sideways.
    // Foliage class only, and structurally impossible on the other two
    // transports anyway: a cluster card's uv.x is 2*ao_code + u01 (max 127)
    // and a bark tube's is a tile coordinate, so bit 19 can never be set on
    // either - which is exactly why the class, not the packed bit, decides
    // whether a mesh sways at all.
    let packed = u32(round(max(vuv_x, 0.0)));
    if (wind_class >= 1.5 && (packed & 524288u) != 0u) {
        // Flutter scales with plant size. A flat amplitude made a 0.3 m
        // strawberry shake as hard as an 18 m oak, and the crop path uses
        // this same material. fv adds the wind-speed response: leaves are
        // the first thing to show a rising wind and the last to stop.
        // Grass: the same amplitudes are metres tuned for a ~10 cm tree
        // leaf, so a 35 cm blade needs its own weight (1.6/m puts a
        // full-height blade tip at ~0.56, i.e. ~3 cm of flutter), and it
        // flutters at roughly TWICE the frequency - a blade is far
        // lighter and stiffer relative to its length than a leaf on a
        // petiole, so it buzzes where foliage wafts.
        let fl = clamp(h * select(0.35, 1.6, is_grass), 0.10, 1.0);
        let fv = clamp(wind_v / 6.0, 0.35, 3.0);
        let fmul = select(1.0, 2.0, is_grass);
        let f = sin(t * (5.1 + 0.15 * wind_v) * fmul + phase * 3.7 + h * 2.3);
        let g = cos(t * (6.7 + 0.20 * wind_v) * fmul + phase * 2.1 + h * 1.7);
        wind_pos = wind_pos
            + wind_dir * (f * 0.055 * gust * fl * fv * obj_per_m)
            + vec3<f32>(0.0, 1.0, 0.0) * (g * 0.030 * gust * fl * fv * obj_per_m)
            + vec3<f32>(-wind_dir.z, 0.0, wind_dir.x)
                * (g * 0.040 * gust * fl * fv * obj_per_m);
    }
    return wind_pos;
}

struct CasterIn {
    @location(0) position: vec3<f32>,
    @location(1) model0: vec4<f32>,
    @location(2) model1: vec4<f32>,
    @location(3) model2: vec4<f32>,
    @location(4) model3: vec4<f32>,
    @location(5) extra: vec4<f32>,
    @location(6) uv: vec2<f32>,
};

struct SwayOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) extra: vec4<f32>,
};

@vertex
fn vs_sway(v: CasterIn) -> SwayOut {
    g_model = mat4x4<f32>(vec4<f32>(v.model0.xyz, 0.0), v.model1, v.model2, v.model3);
    var local = v.position;
    // vs_main's call (no grass here: type 23 is instanced and stays on the per-draw loop).
    let wind_class = v.extra.y;
    if (wind_class >= 0.5) {
        local = wind_displace(v.position, v.uv.x, 0.0, false, wind_class, camera.sun_color.w);
    }
    let world = vs_model() * vec4<f32>(local, 1.0);
    var o: SwayOut;
    // The homogeneous w rebuilt as vs_main does.
    o.clip = view_proj[u32(v.model0.w + 0.5)] * vec4<f32>(world.xyz, 1.0);
    o.uv = v.uv;
    o.extra = v.extra;
    return o;
}

@fragment
fn fs_sway(in: SwayOut) {
    // The uv derivatives in uniform control flow, before any discard (as fs_shadow takes them).
    let uv_dx = dpdx(in.uv);
    let uv_dy = dpdy(in.uv);
    let material_type = in.extra.x;
    let cut = (material_type >= 18.5 && material_type < 19.5) || (material_type >= 20.5 && material_type < 21.5);
    // fs_shadow's LOD crossfade dither (its step 1), in light space.
    let lod_fade = select(0.0, in.extra.z, cut);
    if (lod_fade != 0.0) {
        let px = vec2<u32>(u32(in.clip.x), u32(in.clip.y));
        let bx = px.x % 4u;
        let by = px.y % 4u;
        let bayer_i = (bx % 2u) * 8u + (by % 2u) * 4u + ((bx / 2u) % 2u) * 2u + (by / 2u) % 2u;
        let b = (f32(bayer_i) + 0.5) / 16.0;
        if (lod_fade > 0.0) {
            if (b >= lod_fade) { discard; }
        } else {
            if (b < -lod_fade) { discard; }
        }
    }
    // Type 19, photoscanned foliage.
    if (material_type >= 18.5 && material_type < 19.5) {
        let mesh_tex = textureSampleGrad(
            albedo_texture, albedo_sampler, in.uv, uv_dx, uv_dy);
        if (mesh_tex.a < 0.35) {
            discard;
        }
    }
    // Type 21, a baked cluster card: uv.x = 2 * ao_code + u01 (tree_mesh::encode_card_uv).
    if (material_type >= 20.5 && material_type < 21.5) {
        let cc_code = floor(in.uv.x * 0.5);
        let cc_u = in.uv.x - 2.0 * cc_code;
        let cc_tex = textureSampleGrad(
            albedo_texture, albedo_sampler,
            vec2<f32>(cc_u, in.uv.y), uv_dx, uv_dy);
        if (cc_tex.a < 0.5) {
            discard;
        }
    }
}
