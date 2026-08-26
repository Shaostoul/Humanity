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
    prev_dpos: vec4<f32>, // xyz = prev_pos - cam_pos, w = clip gamma
    prev_fwd: vec4<f32>,  // xyz
    prev_right: vec4<f32>,// xyz
    prev_up: vec4<f32>,   // xyz
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
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let p = clamp(
                qpx + vec2<i32>(dx, dy),
                vec2<i32>(0),
                vec2<i32>(qdim) - vec2<i32>(1),
            );
            let s = textureLoad(march_color, p, 0);
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
        let p_w = u.cam_pos.xyz + rd * t_w;
        let prev_pos = u.cam_pos.xyz + u.prev_dpos.xyz;
        let d_prev = normalize(p_w - prev_pos);
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
    let motion = clamp(shift_tx - 0.75, 0.0, 1.0);
    let alpha = clamp(
        base + smoothstep(0.08, 0.45, diff) * mix(0.05, 0.5, motion),
        base,
        max(base, mix(0.12, 1.0, motion)),
    );
    return mix(hist_c, cur, alpha);
}
