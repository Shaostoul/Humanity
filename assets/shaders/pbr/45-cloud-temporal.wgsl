// ── The temporal cloud octa pass (clouds phase 4) ──────────────────────
//
// The operator's report after the physical-medium rewrite: "they look
// like static instead of clouds." Correct: at physical extinction a
// 16-48 sample march per pixel per frame is heavy spatial noise, and any
// animated jitter makes it BOIL. The production answer (Horizon, Nubis
// Evolved, Decima) is temporal accumulation, and this pass is our
// engine-shaped version of it:
//
// - The history buffer is a 1024^2 RGBA16F OCTAHEDRAL MAP indexed by
//   world DIRECTION, not screen position. Camera rotation therefore
//   needs no reprojection matrix at all (the direction does not change),
//   and camera translation against km-distant clouds moves a direction
//   by well under a texel per frame - the EMA absorbs it. No previous
//   view-proj plumbing, no disocclusion vectors, no screen-space smear.
// - Each frame this fullscreen pass re-marches EVERY map texel (1M rays,
//   ~30% of a full-res frame's sky pixels) with the ANIMATED golden-ratio
//   jitter, and blends the result into the ping-pong partner with an
//   adaptive EMA: alpha 0.10 at rest (10-frame convergence - a
//   supersampled march the single frame could never afford), rising
//   toward 0.6 where the new sample disagrees hard (weather rolling in,
//   sun moving, dev pins flipped) so change never smears.
// - The main pipeline's type-15 fragment then SAMPLES the map by its own
//   ray direction (see cloud_layer_volumetric's temporal branch) instead
//   of marching. The map rides the cloud material's ALBEDO slot, so the
//   composite needs zero bind-group-layout changes - the v0.1029
//   every-create-site hazard never applies. In THIS pass the albedo slot
//   of the bound group-3 carries the ping-pong PARTNER (the history to
//   read), wired by renderer::cloud_temporal.
//
// Bindings are the standard groups 0-3: camera, the cloud SHELL's object
// uniform (obj_model gives the planet frame the march needs), the cloud
// material, and the texture group with history in the albedo slot.

struct CloudOctaVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_cloud_octa(@builtin(vertex_index) vi: u32) -> CloudOctaVsOut {
    // The classic single fullscreen triangle.
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi & 2u) * 2 - 1);
    var out: CloudOctaVsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    // THE MIRROR BUG (found 2026-08-21 from the operator's live report:
    // "up/down flicking... a mirror like effect"). NDC +y is UP but
    // texture row 0 is the TOP: a fragment at NDC y = +1 rasterizes into
    // ROW 0, so its texture-space coordinate is v = 0, not v = 1. The old
    // `uv = ndc * 0.5 + 0.5` handed the fragment a v that addressed the
    // MIRRORED row - so every texel (a) marched the direction belonging
    // to the opposite side of the map, and worse, (b) sampled its EMA
    // HISTORY from the mirrored texel: each frame every sky direction
    // blended 4% of its own mirror image. The converged map was a ghost
    // double exposure of the sky with its reflection, endlessly
    // re-excited into a period-2 mirror oscillation - the flicker, the
    // "state 1 / state 2", and the 10b empty-overhead mystery (the GPU
    // was showing the MIRRORED direction's lane while the CPU reference
    // correctly saw the true direction's deck). Flipping v here makes
    // decode(in.uv), the history sample, and the readers' encode all
    // address the same texel.
    out.uv = vec2<f32>(x, -y) * 0.5 + vec2<f32>(0.5);
    return out;
}

@fragment
fn fs_cloud_octa(in: CloudOctaVsOut) -> @location(0) vec4<f32> {
    // The Lambert projection lives in the inscribed DISC of the square
    // map; the composite's encode always lands inside it, so the 21.5%
    // of texels in the corners are never read - skip their march.
    let pd = in.uv * 2.0 - vec2<f32>(1.0);
    if (dot(pd, pd) > 1.001) {
        return vec4<f32>(0.0);
    }
    // QUARTER-CADENCE MARCH (4096-map brute force): only a quarter of
    // the 32x32-texel BLOCKS march each frame; the rest reproject their
    // history and carry it forward unblended. Per-frame march count
    // therefore equals the old full-rate 2048 map while the stored map
    // holds 4x the texels. BLOCKS, not a per-pixel checkerboard,
    // deliberately: a 2x2 interleave put marching lanes in EVERY
    // hardware wave, so no wave could skip and the whole pass paid
    // full-march occupancy (measured: fps halved). 32x32 blocks let
    // entire waves take the cheap path.
    //
    // TWO LESSONS from the operator's "dome of triangularish clouds"
    // (v0.1189 live report - the cadence lattice itself became visible
    // as a tiled dome at close range):
    // - The block phase is HASHED per block, not a regular 2x2 spatial
    //   pattern: aligned neighbours updating in lockstep read as a
    //   coherent grid sweeping the sky; hashed phases turn the update
    //   order into unstructured noise the EMA hides.
    // - INSIDE/UNDER the deck (camera inside the drawn shell) the map
    //   marches FULL RATE: map texels are enormous on screen there, the
    //   content moves fastest, and a 4-frame-stale block is a visible
    //   tile. The full-rate cost at 4096 is acceptable precisely
    //   because the under-deck slab segment is short and the ground
    //   cull kills most rays.
    var do_march = true;
    {
        let cad_ctr = obj_model()[3].xyz;
        let cad_shr = length(obj_model()[0].xyz);
        let ro_in = (camera.view_pos.xyz - cad_ctr) / cad_shr;
        let cam_inside = dot(ro_in, ro_in) < 1.0;
        if (camera.light4.w > 0.5 && !cam_inside) {
            let px = vec2<u32>(in.pos.xy);
            let bid = (px.x >> 5u) + (px.y >> 5u) * 128u;
            let h = (bid * 747796405u + 2891336453u) >> 9u;
            let cell = (h ^ (h >> 11u)) & 3u;
            let phase = u32(camera.light4.w - 0.5);
            do_march = cell == phase;
        }
    }
    // RESTING FAST PATH: a cadence-skipped texel under a sub-texel camera
    // delta (< 4 m - above the ~2 m f32 quantization noise of the
    // baseline, still < 0.2 texel of parallax at any cloud distance) has
    // nothing to reproject - copy the texel forward and skip the whole
    // decode/reproject/encode chain. Three quarters of the map take this
    // branch whenever the camera is still, which is what pays for the
    // 4096 storage at rest. Resample frames (light3.w) keep the full
    // path - their history lives under the OLD mapping.
    if (!do_march && camera.light3.w < 0.5
        && dot(camera.light4.xyz, camera.light4.xyz) < 16.0) {
        return textureSampleLevel(albedo_texture, albedo_sampler, in.uv, 0.0);
    }
    cloud_set_slab_bounds();
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);
    let rd_w = cloud_map_decode(in.uv, center);
    // History: the ping-pong partner, bound in this pass's albedo slot.
    // 12c RESAMPLE: on the one frame where the CPU controller re-anchored
    // the map (anchor drift, extent drift, or a regime flip), the history
    // texture still holds the OLD mapping - so look this texel's direction
    // up through the OLD params (camera.light3: xy = old anchor octa pair,
    // z = old cos(theta_max), w = flag). The re-anchor is then invisible:
    // no content jump, no ghost EMA-fading out - the operator's "big jump
    // and the old one phases out slowly" dies here. A direction outside
    // the OLD extent has no history; take the fresh march outright.
    // The accumulation sequence: per-texel stratum + the golden-ratio
    // step per cloud-clock tick. Across frames each texel's march visits
    // a low-discrepancy sequence of sample offsets, which is exactly the
    // supersampling the EMA converges. The march runs FIRST (slice B):
    // its first-hit distance (g_march_first_t) is the exact parallax
    // distance for the content this texel shows, which the history
    // reprojection below needs.
    let jitter = fract(
        hash21(in.uv * 8192.0 + vec2<f32>(17.0, 39.0))
            + fract(camera.sun_color.w * 11.0) * 0.618034,
    );
    // Footprint for band-limited volume sampling: this pass's pixel is a
    // Lambert map texel; the footprint deliberately stays at the
    // 2048-map angular size (see cloud_pix_ang_map) so the 4096 texels
    // spatially supersample the same band-limited field instead of
    // paying finer march steps.
    var cur_s = vec4<f32>(0.0);
    if (do_march) {
        cur_s = cloud_march_core(rd_w, center, shell_r, jitter, cloud_pix_ang_map());
    }
    // TRANSLATION REPROJECTION (slice B, from the operator's "solitaire
    // artifact" report - radial streaks converging on the sub-camera
    // point whenever the camera MOVES, clean when still). Rotation is
    // free in a direction-indexed map, but translation was never
    // corrected: the fresh march puts a cloud at its NEW direction while
    // ~25 frames of history still hold it at the OLD one, and the EMA
    // smears the difference along the motion. Fix: ask where this
    // texel's CONTENT was from the PREVIOUS camera - at the distance the
    // march ACTUALLY HIT cloud this frame (exact per-texel parallax; the
    // analytic shell-sphere hit is the fallback for clear rays, and was
    // the whole story before this - right at the shell, wrong inside the
    // slab, which is exactly where the operator still saw ghosting).
    // PRECISION: the pads carry the camera DELTA in the PLANET-LOCAL
    // frame rotated to world axes (camera.light4.xyz, w = flag), never
    // absolute positions - all math stays small (p - prev = rd*t -
    // delta), immune to the f32-at-planet-scale cancellation class.
    var d_hist = rd_w;
    var shift_tx = 0.0;
    var teleported = false;
    if (camera.light4.w > 0.5) {
        var t_rep = g_march_first_t;
        if (t_rep <= 0.0) {
            // Clear ray: no marched surface - the shell-sphere hit stands
            // in so clear sky still counter-scrolls correctly.
            let ro_c = camera.view_pos.xyz - center;
            let b = dot(ro_c, rd_w);
            let cc = dot(ro_c, ro_c) - shell_r * shell_r;
            let disc = b * b - cc;
            if (disc > 0.0) {
                let sq = sqrt(disc);
                t_rep = -b - sq;
                if (t_rep <= 0.0) {
                    t_rep = -b + sq;
                }
            }
        }
        if (t_rep > 0.0) {
            d_hist = normalize(rd_w * t_rep - camera.light4.xyz);
            // TELEPORT GUARD: the small-parallax model only holds for
            // per-frame baselines well under the cloud distance. A
            // teleport-scale delta bends every texel's lookup toward
            // one direction and smears the whole map with one texel's
            // content (seen live on a rig re-park jump). Beyond ~15
            // degrees of reprojection, history is disoccluded - take
            // the fresh march and let the EMA converge clean.
            if (dot(d_hist, rd_w) < 0.966) {
                teleported = true;
            }
        }
    }
    var hist: vec4<f32>;
    var have_hist = true;
    if (camera.light3.w > 0.5) {
        let up_old = cloud_map_axis_world(camera.light3.x, camera.light3.y);
        let k_old = clamp(1.0 - camera.light3.z, 1.0e-3, 2.0);
        let e = cloud_map_encode_at(d_hist, up_old, k_old);
        hist = textureSampleLevel(albedo_texture, albedo_sampler, e.xy, 0.0);
        have_hist = e.z <= 1.0;
        shift_tx = length(e.xy - in.uv) * 4096.0;
    } else if (camera.light4.w > 0.5) {
        let e = cloud_map_encode_at(d_hist, cloud_map_up(center), cloud_map_k());
        hist = textureSampleLevel(albedo_texture, albedo_sampler, e.xy, 0.0);
        have_hist = e.z <= 1.0;
        shift_tx = length(e.xy - in.uv) * 4096.0;
    } else {
        hist = textureSampleLevel(albedo_texture, albedo_sampler, in.uv, 0.0);
    }
    // Cadence-skipped texel: carry the (reprojected) history forward
    // untouched; its own march comes within the next three frames.
    // THE CHECKERBOARD LESSON (operator round 4 - "hall of mirrors or
    // tiles", literal cloud/empty checker across the deck in fast
    // flight): at teleport-scale deltas the guard fires EVERY frame, and
    // this branch used to return transparent black for skipped blocks -
    // zeroing three quarters of the map each frame while the marching
    // quarter refilled, which painted the cadence grid as alternating
    // cloud/empty tiles. A skipped block with an invalid reprojection
    // now keeps its OWN texel: stale for at most three frames, coherent,
    // and invisible next to a fresh march - never a hole.
    if (!do_march) {
        if (teleported || !have_hist) {
            return textureSampleLevel(albedo_texture, albedo_sampler, in.uv, 0.0);
        }
        return hist;
    }
    // PREMULTIPLY before accumulating (the fidelity audit's top finding):
    // the march returns straight alpha, and vec4(0) for clear/early-out
    // rays. EMA-ing straight alpha makes a direction whose jittered
    // marches sometimes miss converge to rgb = f*C with a = f*A (f = hit
    // fraction); the straight-alpha composite then contributes f^2*C*A -
    // every cloud darkened by its own hit fraction TWICE - and bilinear
    // filtering between a cloud texel and a clear texel dragged rgb
    // toward BLACK (the dark fringe/pepper on every edge). Premultiplied
    // storage makes both the EMA and the bilinear filter linear in the
    // right space; the composite divides by alpha to return to straight.
    let cur = vec4<f32>(cur_s.rgb * cur_s.a, cur_s.a);
    // EMA, DEEP and nearly flat (the v0.1159 lesson, from the operator's
    // "tiny dots became big dots"): the first cut raised the blend
    // aggressively wherever the new sample disagreed with history - but
    // in a noisy region the new sample ALWAYS disagrees, that is what
    // noise is, so the map kept chasing individual marches exactly where
    // it most needed to average them, and its unconverged texel churn
    // upscaled into the big dots. Convergence IS the feature: alpha 0.04
    // averages ~25 recent marches per direction (about 1.5 s), which is
    // the supersample that turns grain into cloud. The adaptive term is
    // now a whisper - real changes (weather fronts, the sun, dev pins)
    // evolve over many seconds and a 1.5 s catch-up never reads as
    // ghosting on a cloud.
    let diff = abs(cur.a - hist.a)
        + (abs(cur.r - hist.r) + abs(cur.g - hist.g) + abs(cur.b - hist.b)) * 0.333;
    // DISOCCLUSION CATCH-UP (operator "solitaire" round 3, the ghost
    // copies trailing every mass during descent): reprojection recovers
    // content that MOVED, but sky newly revealed behind a mass edge has
    // no valid history anywhere - those texels were EMA-ing off stale
    // cloud at the resting cap, and the trail was the visible result.
    // During real motion (shift >= ~1 texel) the deep-blend rationale
    // inverts - content change is signal, not noise - so both the diff
    // response and its cap scale with the measured shift. At rest the
    // v0.1159 anti-boil discipline stands untouched.
    let motion = clamp(shift_tx - 0.75, 0.0, 1.0);
    var alpha = clamp(
        0.04 + diff * mix(0.05, 0.6, motion),
        0.04,
        mix(0.12, 0.5, motion),
    );
    // Residual parallax error also scales with shift - a floor keeps the
    // trail from persisting even where diff happens to be small.
    alpha = max(alpha, min(0.35, max(shift_tx - 1.0, 0.0) * 0.02));
    // No history for this direction (outside the old extent on a resample
    // frame, or a teleport-scale reprojection): start from the fresh
    // march instead of EMA-ing toward zero or smearing stale content.
    if (!have_hist || teleported) {
        alpha = 1.0;
    }
    return mix(hist, cur, alpha);
}
