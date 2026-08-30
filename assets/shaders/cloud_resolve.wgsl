// ── Cloud temporal RESOLVE (12e, the march/resolve split) ──────────────
//
// Accumulates the quarter-res per-frame cloud march into the half-res
// history pair with DEEP accumulation + VARIANCE-CLIPPED history - the
// production-standard combination (Decima/Frostbite-class) that ends the
// 12d trade-off between static (shallow blend never converges the
// jittered march) and ghosting (deep blend fades stale content for half
// a second):
//
// - History is reprojected through the actual camera motion using the
//   march's own per-pixel first-hit distance (translation-exact).
// - The reprojected history is then CLIPPED to mean +- gamma*sigma of
//   the current march's 3x3 neighbourhood. Content the current frame
//   cannot corroborate is snapped to the plausible range in ONE frame -
//   ghosts die instantly - while corroborated content accumulates
//   ~1/alpha frames deep, converging the subpixel-jittered march into a
//   smooth, detailed image (the static is the unconverged estimator, so
//   depth IS the cure).
// - Sky pixels carry the analytic shell-top distance so clear regions
//   track parallax too instead of smearing when clouds arrive.
//
// Runs at the accumulation pair's half resolution; the fullscreen cloud
// composite then samples the freshly written buffer at each fragment's
// own screen uv (unchanged).

struct CloudResolveUniforms {
    // Current camera: eye + tan(fov_y/2), forward + aspect, right, up.
    cam_pos: vec4<f32>,   // xyz = eye (render frame), w = tan(fov_y/2)
    cam_fwd: vec4<f32>,   // xyz = forward, w = aspect
    cam_right: vec4<f32>, // xyz; w = base accumulation alpha
    cam_up: vec4<f32>,    // xyz; w = snap flag (1 = drop history entirely)
    // Previous camera: eye offset (prev - cur, render frame) + basis.
    prev_dpos: vec4<f32>, // xyz = prev_pos - cam_pos (RAW camera delta when
                          // the spin split is live), w = clip gamma
    prev_fwd: vec4<f32>,  // xyz
    prev_right: vec4<f32>,// xyz
    prev_up: vec4<f32>,   // xyz
    // ── Spin-aware content reprojection (v0.1251) ──
    // Planet-fixed content rotates: p_prev = C + M*(p - C), M = the
    // frame-to-frame spin rotation. xyz = column i of M; w = component i
    // of spin_off = M*e - e (e = cam - C, computed f64 on the CPU -
    // planet-magnitude cancellation must not touch f32). Identity + zero
    // when the split is unavailable, which makes the math below collapse
    // to the old translation-only form exactly.
    spin_c0: vec4<f32>,
    spin_c1: vec4<f32>,
    spin_c2: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: CloudResolveUniforms;
@group(0) @binding(1) var march_color: texture_2d<f32>;
@group(0) @binding(2) var march_dist: texture_2d<f32>;
@group(0) @binding(3) var history: texture_2d<f32>;
@group(0) @binding(4) var lin_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi & 2u) * 2 - 1);
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    // Texture-space uv (v = 0 at the top row = NDC +y), matching how the
    // composite samples the accumulation buffer at 1 - screen_v.
    out.uv = vec2<f32>(x, -y) * 0.5 + vec2<f32>(0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Current march, upsampled bilinearly (the subpixel jitter makes
    // this a rotating reconstruction kernel over frames).
    let cur = textureSampleLevel(march_color, lin_sampler, in.uv, 0.0);

    // 3x3 neighbourhood moments of the march at quarter res: the
    // variance-clip bounding box (Karis-style). Premultiplied rgba
    // moments clip all four channels consistently.
    let qdim = vec2<f32>(textureDimensions(march_color));
    let qpx = vec2<i32>(clamp(in.uv * qdim, vec2<f32>(0.0), qdim - vec2<f32>(1.0)));
    var m1 = vec4<f32>(0.0);
    var m2 = vec4<f32>(0.0);
    // BILINEAR moments (v0.1246, the operator's blocky clouds): the old
    // textureLoad taps made box_lo/box_hi piecewise-constant per quarter-res
    // texel - ~12-screen-px plateaus - and wherever the clip or the motion
    // floor is active (chronically, inside the layer at low FPS) those
    // plateaus stamped straight into the output as the blocks. Sampling the
    // same 3x3 window bilinearly at this pixel's own uv makes the box vary
    // continuously per output pixel; 9 bilinear taps at quarter res is
    // trivially cheap.
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let s = textureSampleLevel(
                march_color, lin_sampler,
                in.uv + vec2<f32>(f32(dx), f32(dy)) / qdim, 0.0);
            m1 = m1 + s;
            m2 = m2 + s * s;
        }
    }
    let mu = m1 / 9.0;
    let sigma = sqrt(max(m2 / 9.0 - mu * mu, vec4<f32>(0.0)));
    let gamma = max(u.prev_dpos.w, 0.25);
    let box_lo = mu - sigma * gamma;
    let box_hi = mu + sigma * gamma;

    // Reproject this pixel's content point into the previous frame via
    // the march's own first-hit distance (km in R16F).
    let t_w = textureLoad(march_dist, qpx, 0).r * 1000.0;
    let ndc = vec2<f32>(in.uv.x, 1.0 - in.uv.y) * 2.0 - vec2<f32>(1.0);
    let tanf = max(u.cam_pos.w, 1.0e-4);
    let aspect = max(u.cam_fwd.w, 1.0e-4);
    let rd = normalize(
        u.cam_fwd.xyz
            + u.cam_right.xyz * (ndc.x * tanf * aspect)
            + u.cam_up.xyz * (ndc.y * tanf),
    );

    var hist = cur;
    var have_hist = false;
    // How far this texel's history moved on screen, in half-res texels. The
    // motion gate below needs it, and the reprojection is already computing
    // the two UVs it is the distance between.
    var shift_tx = 0.0;
    if (u.cam_up.w < 0.5 && t_w > 0.0) {
        // SMALL FORM (v0.1238): algebraically identical to
        // (cam + rd*t_w) - (cam + prev_dpos), but the big form routed two
        // small quantities through the camera position - which after a real
        // flight from the homestead sits at ~3.6e7 m in f32 (no
        // floating-origin rebase), where one ulp is 4 m. Each add rounded to
        // that axis-aligned lattice, so the reprojected history direction
        // carried up to ~8 m of cardinal-locked error - worst exactly where
        // t_w is smallest, directly below the feet: the operator's starburst.
        // The rig never saw it because a teleport keeps camera.position ~30 m
        // (exact); the starburst-far vantage (far_frame_km) reproduces the
        // flown state and proved this red before the fix.
        // Rotate the hit point by the content's own frame-to-frame spin
        // (v0.1251), then subtract the raw camera delta. The old form
        // folded the spin into prev_dpos as a translation - first-order
        // correct at the view centre, wrong toward the limb, and it made
        // every non-co-rotating camera read as "moving" (see the floor
        // note below).
        let q = rd * t_w;
        let q_prev = u.spin_c0.xyz * q.x + u.spin_c1.xyz * q.y
            + u.spin_c2.xyz * q.z
            + vec3<f32>(u.spin_c0.w, u.spin_c1.w, u.spin_c2.w);
        let d_prev = normalize(q_prev - u.prev_dpos.xyz);
        let z = dot(d_prev, u.prev_fwd.xyz);
        if (z > 1.0e-4) {
            let nx = dot(d_prev, u.prev_right.xyz) / (z * tanf * aspect);
            let ny = dot(d_prev, u.prev_up.xyz) / (z * tanf);
            let uv_p = vec2<f32>(nx, -ny) * 0.5 + vec2<f32>(0.5);
            if (uv_p.x >= 0.0 && uv_p.x <= 1.0
                && uv_p.y >= 0.0 && uv_p.y <= 1.0)
            {
                hist = textureSampleLevel(history, lin_sampler, uv_p, 0.0);
                have_hist = true;
                let dims = vec2<f32>(textureDimensions(history));
                shift_tx = length((uv_p - in.uv) * dims);
            }
        }
    }
    if (!have_hist) {
        return cur;
    }

    // THE CLIP: history the current neighbourhood cannot corroborate is
    // snapped into the plausible box - a one-frame ghost death that a
    // blend constant could never provide.
    let hist_c = clamp(hist, box_lo, box_hi);

    // Deep accumulation, accelerated by residual disagreement (the clip
    // already absorbed the gross part) but ONLY in proportion to real motion.
    //
    // ── THE STATIC (v0.1228) ──
    // The acceleration used to be ungated: `base + smoothstep(diff) * 0.5`,
    // capped at 1.0. That is a positive feedback loop against noise. A noisy
    // pixel disagrees with its own history BECAUSE it is noisy, which raised
    // its blend rate from 0.07 toward 0.57, which stopped it averaging, which
    // kept it noisy. The filter switched itself off at exactly the pixels it
    // existed to fix, and stayed off with the camera completely still - so the
    // operator saw permanent fizz on every cloud silhouette while the flat
    // interiors, which had nothing to fix, converged beautifully.
    //
    // The FAR path never had this bug (45-cloud-temporal.wgsl): it scales both
    // the diff response and its cap by measured motion, capping at 0.12 at
    // rest. This is that same shape. At rest, disagreement can lift alpha only
    // to 0.12, so noise still averages away over ~8 frames; under motion the
    // cap opens to 1.0, because then a changed pixel is signal rather than
    // noise and holding stale history would be a ghost.
    let diff = abs(cur.a - hist_c.a)
        + (abs(cur.r - hist_c.r) + abs(cur.g - hist_c.g)
            + abs(cur.b - hist_c.b)) * 0.333;
    let base = clamp(u.cam_right.w, 0.02, 1.0);
    // ── THE EPIPOLE BLIND SPOT (v0.1235) ──
    //
    // Operator, inside the clouds: "this very obvious weird effect that comes
    // to a point at the bottom of my feet. Kind of like the bottom of my feet
    // are a balloon tied off... most noticeable in the clouds."
    //
    // shift_tx measures how far a texel SLID on screen. Flying TOWARD content,
    // texels at the centre of motion do not slide - they EXPAND radially
    // around a fixed point (the epipole: straight ahead in forward flight,
    // straight down in a descent). There shift_tx reads ~0, the gate calls the
    // camera at rest, caps blending at the anti-boil 0.12, and stale history
    // smears radially around the very point being flown toward. A starburst
    // knotted at the feet is this gate lying.
    //
    // The cure is the second motion channel the texel already carries: how
    // much CLOSER its content got this frame. prev_dpos is the camera
    // translation, t_w this texel's content distance - their ratio is the
    // zoom rate, and 3 percent closer per frame is unambiguous flight however
    // little the texel slid.
    let zoom_rel = length(u.prev_dpos.xyz) / max(t_w, 1.0);
    let motion = max(
        clamp(shift_tx - 0.75, 0.0, 1.0),
        smoothstep(0.005, 0.03, zoom_rel),
    );
    var alpha = clamp(
        base + smoothstep(0.08, 0.45, diff) * mix(0.05, 0.5, motion),
        base,
        max(base, mix(0.12, 1.0, motion)),
    );
    // ── THE FLOOR MUST RISE WITH MOTION, NOT JUST THE CAP (v0.1236) ──
    //
    // The v0.1235 zoom gate opened the CAP under motion, but alpha itself
    // only rises with DISAGREEMENT - and a smooth cloud interior barely
    // disagrees with its own smear. Inside a cloud the content is metres
    // away, so walking even 1 m per frame is a 25 percent parallax error:
    // the history lookup lands far from where it should, and at alpha 0.07
    // that wrongly-fetched history still won. The operator's starburst
    // survived the gate because the gate opened a door nothing walked
    // through. Under real motion the fresh march must simply WIN.
    // ── THE FLOOR KEYS ON REPROJECTION RELIABILITY, NOT RAW SLIDE (v0.1251) ──
    //
    // With the spin folded into prev_dpos as translation, any camera not
    // co-rotating with the planet read 1-5 texels of perpetual slide, the
    // floor pinned alpha at 0.6+, and the resolve was effectively OFF -
    // the operator's "TV static" over the whole disc from space, and the
    // "uncanny low detail" against the full-res terrain (an unconverged
    // estimator IS lower resolution). A coherent slide is exactly what
    // the reprojection handles - now spin-exact across the whole view -
    // so modest slides keep deep accumulation and only two things force
    // fresh: real zoom (parallax error the translation-form cannot fix)
    // and extreme slides where iterated bilinear rewarping degrades.
    let slide_hard = clamp(shift_tx / 8.0 - 0.25, 0.0, 1.0);
    let zoom_gate = smoothstep(0.005, 0.03, zoom_rel);
    alpha = max(alpha, max(zoom_gate, slide_hard) * 0.6);

    // ── VARIANCE-ADAPTIVE SPATIAL FILTER (v0.1252, the operator's
    // "jitter / tv static ... worse as I get closer") ──
    // Near clouds the temporal filter is CORRECTLY shallow: interior
    // parallax under flight is not reprojectable by a single first-hit
    // distance, so the floor above opens and each frame shows mostly the
    // raw march - whose per-pixel jitter over a near-binary density field
    // is coin-flip static no history exists to average. But the SPATIAL
    // neighbourhood already holds the answer: the 3x3 mean mu is a smooth
    // partial-coverage estimate of the same content. Where the
    // neighbourhood is statistically NOISE (high sigma relative to its
    // own level) and the blend is shallow (temporal depth unavailable),
    // the current frame contributes mu instead of its own coin flip.
    // Structured edges keep detail two ways: their sigma is contrast, not
    // noise, only partially engaging the filter, and what mu costs there
    // is one quarter-res texel of softness - against animated static,
    // the trade the operator is explicitly asking for.
    let lvl = mu.a + dot(mu.rgb, vec3<f32>(0.333));
    let sig = sigma.a + dot(sigma.rgb, vec3<f32>(0.333));
    let rel_sig = sig / max(lvl, 0.02);
    let noise_w = smoothstep(0.15, 0.60, rel_sig);
    let shallow = clamp((alpha - 0.12) / 0.5, 0.0, 1.0);
    let cur_s = mix(cur, mu, noise_w * mix(0.35, 0.75, shallow));
    return mix(hist_c, cur_s, alpha);
}
