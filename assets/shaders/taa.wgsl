// TEMPORAL ANTI-ALIASING: the resolve and the sharpen (renderer roadmap rung 1a, 2026-11-13;
// docs/design/renderer-roadmap.md sections 2 and 3; driven by src/renderer/taa.rs).
//
// Every frame the scene is drawn a fraction of a pixel to one side (the jitter, a Halton 2,3
// sequence of 8, added in vs_main), so over eight frames each pixel sees eight sample positions.
// The RESOLVE blends this frame's pixel into a history carried from the frame before, fetched
// where that surface was on the last frame (the motion target every opaque pass writes), and
// rejects history that no longer matches what the pixel's neighbourhood holds now. The published
// methods: Karis 2014 ("High Quality Temporal Supersampling", SIGGRAPH Advances: the closest-depth
// motion, the YCoCg neighbourhood, the reversible tonemap weighting), Salvi 2016 ("An Excursion
// in Temporal Supersampling", GDC: variance clipping), Pedersen 2016 ("Temporal Reprojection
// Anti-Aliasing in INSIDE", GDC), Jimenez 2016 (the five-tap Catmull-Rom history fetch, from
// "Filmic SMAA", SIGGRAPH Advances). Written from the papers; Bevy's TAA and Playdead's were read
// as references, not copied.
//
// THE MOTION TARGET (Rgba16Float, 8 bytes a pixel), written by every opaque pass:
//   xy = where this surface was on screen last frame, as an offset in UV: (now - then), so the
//        history is fetched at uv - xy. Both positions UNJITTERED.
//   z  = the responsive mask: how much a blended surface (glass, the water's surface, the cloud
//        shell, an atmosphere) covers this pixel; it lowers the history's weight there, since a
//        blended surface writes no motion of its own (FSR 2's reactive mask idea).
//   w  = 1 where a surface was drawn, 0 where it was cleared: the SKY SENTINEL. A sky pixel is
//        reprojected by the camera's turn alone (`sky_reproj`), which is exact for the stars, the
//        sky and the clouds, whose own history is already by direction.
//
// THE SHARPEN restores what the blend softens: a contrast-adaptive sharpen written after AMD's
// published CAS idea (FidelityFX CAS, MIT): it sharpens less where a pixel's neighbourhood is
// already contrasty, so edges do not ring. It writes the scene target; the history stays unsharpened.

struct TaaParams {
    // xy = the target's size in pixels, zw = one over it.
    size: vec4<f32>,
    // x = 1 discards the history this frame (a camera cut, a resize, the first frame, the reuse
    //     switch off), y = sharpening 0..1, z = the Dev page view (0 the picture, 1 motion,
    //     2 the history's weight, 3 the responsive mask), w = the smallest share of this frame.
    ctl: vec4<f32>,
    // x = 1 weights the blend by 1 / (1 + luma) (Karis), for when the scene target turns linear
    //     (hdr-scene-target.md increment 5); 0 while it holds display values. yzw spare.
    extra: vec4<f32>,
    // Spare, keeps the matrix 16-byte aligned for every backend.
    pad: vec4<f32>,
    // A sky pixel's place last frame: the previous celestial view-projection (in this frame's
    // render space) times the inverse of this frame's, both unjittered, applied to the pixel at
    // the far plane.
    sky_reproj: mat4x4<f32>,
};

// ── The resolve (compute) ──
@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var motion_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var history_prev: texture_2d<f32>;
@group(0) @binding(4) var history_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var lin_samp: sampler;
@group(0) @binding(6) var<uniform> params: TaaParams;

fn rgb_to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(c, vec3<f32>(0.25, 0.5, 0.25)),
        dot(c, vec3<f32>(0.5, 0.0, -0.5)),
        dot(c, vec3<f32>(-0.25, 0.5, -0.25)),
    );
}

fn ycocg_to_rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

fn clamp_px(p: vec2<i32>) -> vec2<i32> {
    return clamp(p, vec2<i32>(0, 0), vec2<i32>(params.size.xy) - vec2<i32>(1, 1));
}

// The Karis weight: 1 / (1 + luma) when switched on, so a single bright pixel cannot dominate
// the blend and ghost (only meaningful once the target holds linear light).
fn karis_w(c: vec3<f32>) -> f32 {
    return select(1.0, 1.0 / (1.0 + c.x), params.extra.x > 0.5);
}

// The history at `uv` through a Catmull-Rom filter in five bilinear taps (the four corner taps of
// the full nine have small weights and are dropped), which keeps the history sharp while it is
// reprojected by fractions of a pixel every frame; a bilinear fetch would blur it a little more
// each frame.
fn history_catmull_rom(uv: vec2<f32>) -> vec3<f32> {
    let pos = uv * params.size.xy;
    let c1 = floor(pos - 0.5) + 0.5;
    let f = pos - c1;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let tc0 = (c1 - 1.0) * params.size.zw;
    let tc3 = (c1 + 2.0) * params.size.zw;
    let tc12 = (c1 + w2 / w12) * params.size.zw;
    var acc = vec3<f32>(0.0);
    var wsum = 0.0;
    let ws = array<f32, 5>(w12.x * w0.y, w0.x * w12.y, w12.x * w12.y, w3.x * w12.y, w12.x * w3.y);
    let ts = array<vec2<f32>, 5>(
        vec2<f32>(tc12.x, tc0.y),
        vec2<f32>(tc0.x, tc12.y),
        vec2<f32>(tc12.x, tc12.y),
        vec2<f32>(tc3.x, tc12.y),
        vec2<f32>(tc12.x, tc3.y),
    );
    for (var i = 0; i < 5; i = i + 1) {
        acc = acc + textureSampleLevel(history_prev, lin_samp, ts[i], 0.0).rgb * ws[i];
        wsum = wsum + ws[i];
    }
    return max(acc / max(wsum, 1.0e-4), vec3<f32>(0.0));
}

// Clip `h` toward the box's centre until it lies inside the box (Salvi's variance clip; clipping
// toward the centre rather than clamping per channel keeps the colour's hue).
fn clip_to_box(lo: vec3<f32>, hi: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    let centre = 0.5 * (hi + lo);
    let half_ext = 0.5 * (hi - lo) + vec3<f32>(1.0e-5);
    let d = h - centre;
    let units = abs(d / half_ext);
    let m = max(units.x, max(units.y, units.z));
    if (m > 1.0) {
        return centre + d / m;
    }
    return h;
}

@compute @workgroup_size(8, 8)
fn cs_resolve(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = vec2<i32>(params.size.xy);
    if (i32(gid.x) >= size.x || i32(gid.y) >= size.y) {
        return;
    }
    let p = vec2<i32>(gid.xy);
    let uv = (vec2<f32>(p) + 0.5) * params.size.zw;

    // The 3x3 neighbourhood: its mean and spread in YCoCg (for the clip), this pixel's colour,
    // and the CLOSEST surface's pixel (reverse Z: the largest depth), whose motion is the one
    // used, so a thin near edge drags its own history along instead of the background's.
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);
    var cur = vec3<f32>(0.0);
    var best_d = 0.0;
    var best_p = p;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let q = clamp_px(p + vec2<i32>(dx, dy));
            let c = rgb_to_ycocg(max(textureLoad(scene_tex, q, 0).rgb, vec3<f32>(0.0)));
            m1 = m1 + c;
            m2 = m2 + c * c;
            if (dx == 0 && dy == 0) {
                cur = c;
            }
            let d = textureLoad(depth_tex, q, 0);
            if (d > best_d) {
                best_d = d;
                best_p = q;
            }
        }
    }
    let mean = m1 / 9.0;
    let sigma = sqrt(max(m2 / 9.0 - mean * mean, vec3<f32>(0.0)));

    // The scene pass's depth covers the home, the people and what the scene pass draws; where it
    // drew nothing (cleared to 0) the celestial pass's surfaces and the sky remain, and this
    // pixel's own motion stands.
    let here = textureLoad(motion_tex, p, 0);
    var mv = here;
    if (best_d > 0.0) {
        mv = textureLoad(motion_tex, best_p, 0);
    }
    var prev_uv = uv - mv.xy;
    var valid = true;
    if (mv.w < 0.5) {
        // The sky sentinel: reproject the direction through the camera's turn.
        let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
        let pc = params.sky_reproj * vec4<f32>(ndc, 0.0, 1.0);
        if (pc.w <= 1.0e-6) {
            valid = false;
        } else {
            let pn = pc.xy / pc.w;
            prev_uv = vec2<f32>(pn.x * 0.5 + 0.5, 0.5 - pn.y * 0.5);
        }
    }
    if (any(prev_uv < vec2<f32>(0.0)) || any(prev_uv > vec2<f32>(1.0))) {
        valid = false;
    }

    var out = cur;
    var history_weight = 0.0;
    if (params.ctl.x < 0.5 && valid) {
        var hist = rgb_to_ycocg(history_catmull_rom(prev_uv));
        // Gamma 1.25: the box is the mean plus and minus 1.25 standard deviations, tight enough
        // to drop history that a disocclusion or a lighting change made wrong, loose enough to
        // keep the sub-pixel detail the jitter accumulates.
        let lo = mean - 1.25 * sigma;
        let hi = mean + 1.25 * sigma;
        hist = clip_to_box(lo, hi, hist);
        // How much of THIS frame goes in: the floor (about 1/16, so eight jitter phases and the
        // decay average about sixteen frames), more where the surface moves fast (a moving edge
        // keeps less smeared history), and most where a blended surface covers the pixel.
        let speed_px = length(mv.xy * params.size.xy);
        var alpha = params.ctl.w;
        alpha = mix(alpha, 0.25, clamp(speed_px / 24.0, 0.0, 1.0));
        alpha = max(alpha, clamp(here.z, 0.0, 1.0) * 0.85);
        let wc = karis_w(cur) * alpha;
        let wh = karis_w(hist) * (1.0 - alpha);
        out = (cur * wc + hist * wh) / max(wc + wh, 1.0e-6);
        history_weight = 1.0 - alpha;
    }
    textureStore(history_out, p, vec4<f32>(max(ycocg_to_rgb(out), vec3<f32>(0.0)), history_weight));
}

// ── The sharpen (a fragment pass into the scene target, after the resolve) ──
@group(0) @binding(7) var hist_cur: texture_2d<f32>;

struct FullOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> FullOut {
    // One triangle that covers the screen.
    let xy = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: FullOut;
    o.pos = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

fn hist_at(p: vec2<i32>) -> vec3<f32> {
    return textureLoad(hist_cur, clamp_px(p), 0).rgb;
}

@fragment
fn fs_sharpen(in: FullOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.pos.xy);
    let view = i32(params.ctl.z + 0.5);
    if (view == 1) {
        // Motion: red right, green down, a pixel of motion per tenth of the brightness; the sky
        // sentinel shows dark blue. Blue half-lit: some of the motion is left or up (more than a twentieth
        // of a pixel, so rounding noise on a still surface stays black).
        let mv = textureLoad(motion_tex, clamp_px(p), 0);
        if (mv.w < 0.5) {
            return vec4<f32>(0.0, 0.0, 0.25, 1.0);
        }
        let px = mv.xy * params.size.xy;
        return vec4<f32>(clamp(abs(px) * 0.1, vec2<f32>(0.0), vec2<f32>(1.0)), select(0.0, 0.5, px.x < -0.05 || px.y < -0.05), 1.0);
    }
    if (view == 2) {
        // How much of the picture is history (white) and how much this frame (black).
        let w = textureLoad(hist_cur, clamp_px(p), 0).a;
        return vec4<f32>(w, w, w, 1.0);
    }
    if (view == 3) {
        let r = textureLoad(motion_tex, clamp_px(p), 0).z;
        return vec4<f32>(r, r * 0.5, 0.0, 1.0);
    }
    let c = hist_at(p);
    let s = clamp(params.ctl.y, 0.0, 1.0);
    if (s <= 0.0) {
        return vec4<f32>(c, 1.0);
    }
    let n = hist_at(p + vec2<i32>(0, -1));
    let so = hist_at(p + vec2<i32>(0, 1));
    let e = hist_at(p + vec2<i32>(1, 0));
    let w = hist_at(p + vec2<i32>(-1, 0));
    let mn = min(c, min(min(n, so), min(e, w)));
    let mx = max(c, max(max(n, so), max(e, w)));
    // The amount: large where the neighbourhood is flat, small where it already has contrast
    // (the CAS shape), on display values (kept below 1 for the headroom term).
    let mxc = max(mx, vec3<f32>(1.0e-4));
    let amp = sqrt(clamp(min(mn, max(vec3<f32>(1.0) - mx, vec3<f32>(0.0))) / mxc, vec3<f32>(0.0), vec3<f32>(1.0)));
    let peak = -1.0 / mix(8.0, 5.0, s);
    let wgt = amp * peak;
    let res = (c + (n + so + e + w) * wgt) / (vec3<f32>(1.0) + 4.0 * wgt);
    return vec4<f32>(max(res, vec3<f32>(0.0)), 1.0);
}
