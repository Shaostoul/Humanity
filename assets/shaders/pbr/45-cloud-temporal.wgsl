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

// Strong per-texel hash (v0.1237). The old jitter fed hash21 with uv * 8192,
// which steps by exactly 2.0 per texel through a fract(x * 0.1031) - a
// quasi-periodic sequence with a ~5-texel cycle, not noise. The stratified
// march average of a BIASED sequence keeps the bias, and the bias is what the
// operator saw: a deterministic moire rosette (radial petals + cardinal cross)
// converging at the map anchor - at the view centre from space, at the feet
// inside the layer. Proven by bisect: history OFF and jitter CONSTANT both
// left the rosette standing, so it was never temporal smear - it is aliasing
// the supersampler was supposed to remove and could not, because its sample
// positions were not actually well distributed.
fn pcg2d_hash(v_in: vec2<u32>) -> f32 {
    var v = v_in * vec2<u32>(1664525u, 1013904223u);
    v.x = v.x + v.y * 1664525u;
    v.y = v.y + v.x * 1013904223u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x = v.x + v.y * 1664525u;
    v.y = v.y + v.x * 1013904223u;
    v = v ^ (v >> vec2<u32>(16u));
    return f32(v.x ^ v.y) * (1.0 / 4294967296.0);
}

// Interleaved gradient noise (Jimenez 2014) - the production dither for
// exactly this pipeline (v0.1252, the operator's "jitter / tv static").
// A PCG hash is WHITE noise: its error clumps at every frequency, so an
// unconverged march (flight keeps the temporal filter shallow near
// clouds - interior parallax is genuinely unreprojectable) reads as
// salt-and-pepper static. IGN concentrates the error in the highest
// spatial frequencies, which bilinear upsampling, the resolve's spatial
// pass, and the eye all suppress - the same jitter budget becomes far
// less visible while converging to the same average. The per-frame
// advance (5.588238 * frame) walks the pattern so the stratified
// average stays flat; distinct pixel-space salts decorrelate consumers.
fn ign(px: vec2<f32>, f: f32) -> f32 {
    let p = px + vec2<f32>(5.588238 * f);
    return fract(52.9829189 * fract(0.06711056 * p.x + 0.00583715 * p.y));
}

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
        // Full rate INSIDE the shell AND in the band just above it
        // (< ~1.0075 planet radii = ~64 km on Earth, shell units 1.015):
        // the operator's "worst layer is at cloud layer to just above" -
        // map texels are screen-huge there, so any cadence staleness is
        // a visible tile. w > 8.5 is the CPU teleport sentinel: the
        // frame delta is so large that no history is valid, so cadence
        // is suspended and everything marches fresh.
        // v0.1245: full-rate only while the MAP is what the player sees.
        // Inside/under the deck the near screen arm owns the sky (per-pixel
        // regime split) and the map is occluded - full-rate marching 16.7M
        // hidden texels was most of the operator's 3 FPS in the layer, and
        // the low FPS is itself what kept the accumulator unconverged
        // ("the static of being inside the clouds is insanely bad"). The
        // CPU pokes near_mix into light7_color.x; above 0.5 the near arm
        // dominates and the map drops back to quarter cadence.
        // v0.1246 REGRESSION FIX: the v0.1245 near-owns gate starved the
        // UNDER-DECK horizon band. Below the deck the per-pixel split hands
        // near rays only ~32 km of mostly-horizontal slab - the horizon band
        // is still the MAP's - yet near_mix reads 1.0 down there, so the
        // gate dropped the one visible map region to quarter cadence: the
        // operator's wall of static at the horizon and the stale-bright band
        // at night. The occlusion argument is only valid INSIDE the slab
        // (everything within near ownership); under the deck the map stays
        // full rate as it always was.
        let r2_in = dot(ro_in, ro_in);
        // v0.1246 units fix: ro_in is in DRAWN-SHELL radii (divided by
        // shell_r above); the slab base in that unit is slab_rb (planet
        // radii, material.params2.x) times 1/shell_ratio (material.params.w).
        // The previous literal 1.000314 was slab-base-squared in PLANET
        // radii - ~16.8 km altitude in shell units, which made cam_near
        // full-rate for the whole slab interior (the v0.1245 in-layer cost,
        // masked at the time by the dispatch skip).
        let rb_shell = material.params2.x * material.params.w;
        let under_deck = r2_in < rb_shell * rb_shell;
        let cam_near = r2_in < 1.015
            && (under_deck || camera.light7_color.x < 0.5);
        let w = camera.light4.w;
        if (w > 0.5 && w < 8.5 && !cam_near) {
            let px = vec2<u32>(in.pos.xy);
            let bid = (px.x >> 5u) + (px.y >> 5u) * 128u;
            let h = (bid * 747796405u + 2891336453u) >> 9u;
            let cell = (h ^ (h >> 11u)) & 3u;
            let phase = u32(w - 0.5);
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
    let texel = vec2<u32>(clamp(in.uv, vec2<f32>(0.0), vec2<f32>(0.99999)) * 4096.0);
    let jitter = fract(
        pcg2d_hash(texel) + fract(camera.sun_color.w * 11.0) * 0.618034,
    );
    // Sub-texel DIRECTION jitter (v0.1237): the anti-moire supersample. Each
    // frame this texel marches a slightly different direction inside its own
    // footprint, so over the EMA window the texel integrates its solid angle
    // instead of point-sampling one fixed direction of a field with structure
    // finer than the texel. Point-sampling is where the rosette was born; no
    // jitter QUALITY could fix what was structurally a point sample.
    let dj = vec2<f32>(
        pcg2d_hash(texel + vec2<u32>(7919u, 104729u)),
        pcg2d_hash(texel + vec2<u32>(15485863u, 32452843u)),
    );
    // R2 pair (v0.1246): the old single golden scalar splatted on both uv
    // axes made the temporal kernel RANK-1 - each texel time-averaged a
    // LINE segment, not its 2-D solid angle: permanent per-texel bias
    // noise + uniform diagonal micro-streaking. Decorrelated axes, same R2
    // constants the screen path uses.
    // AMPLITUDE 0.4 (v0.1249, the rosette's true author found by the
    // channel bisect): direction jitter is a DIRECTION-SPACE FILTER, and
    // filtering directions over a THICK slab shears depth structure
    // radially about the anchor - a +-0.8-texel angular wobble swings the
    // far end of a 200 km grazing chord by kilometres while the near end
    // barely moves, and the EMA averages that into radial streaks. Terms
    // that vary along the ray smear hardest: the AMBIENT channel (height +
    // cavity dependent) carried the operator's petal rosette loud and
    // clear in the bisect dump while the column-uniform DIRECT channel was
    // clean. 0.4 keeps enough sub-texel coverage to hold the v0.1237
    // moire down (the PCG hash carries most of that fix) while cutting
    // the depth shear by more than half.
    let uv_j = clamp(
        in.uv + (dj + fract(camera.sun_color.w * 7.0 * vec2<f32>(0.7548777, 0.5698403))
            - vec2<f32>(1.0)) * (0.4 / 4096.0),
        vec2<f32>(0.0), vec2<f32>(1.0));
    let rd_wj = cloud_map_decode(uv_j, center);
    // Footprint for band-limited volume sampling: this pass's pixel is a
    // Lambert map texel; the footprint deliberately stays at the
    // 2048-map angular size (see cloud_pix_ang_map) so the 4096 texels
    // spatially supersample the same band-limited field instead of
    // paying finer march steps.
    var cur_s = vec4<f32>(0.0);
    if (do_march) {
        cur_s = cloud_march_core(rd_wj, center, shell_r, jitter, cloud_pix_ang_map());
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
        // v0.1246: same coherence bound as the marching branch - past ~48
        // texels of shift the reprojected fetch is advection, and iterating
        // it every skipped frame was a fibre painter with NO cut at all.
        // Keep the texel's own value instead (stale <= 3 frames, coherent).
        if (teleported || !have_hist || shift_tx > 48.0) {
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
    // Rosette-bisect diagnostic (v0.1249, camera.light7_color.z via the
    // showcase map_diag pin): render ONE channel of the march into the map
    // with the EMA bypassed, so a dump shows the raw per-texel quantity.
    // 1 = first-hit t (geometry), 2 = direct-sun luminance, 3 = ambient
    // luminance. Whichever carries the anchor-centred petals is the biased
    // term. Zero-cost when the pin is 0 (uniform branch).
    let map_diag = camera.light7_color.z;
    if (map_diag > 0.5) {
        var dv = 0.0;
        if (map_diag < 1.5) {
            dv = fract(g_march_first_t * 20.0);
        } else if (map_diag < 2.5) {
            dv = g_march_sun_acc * 2.0;
        } else {
            dv = g_march_amb_acc * 4.0;
        }
        cur_s = vec4<f32>(vec3<f32>(dv), 1.0);
    }
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
    // Same epipole blind spot as the near resolve (see cloud_resolve.wgsl
    // v0.1235): shift-only motion reads ~0 at the point being flown toward.
    // light4.xyz is this frame's translation baseline in world axes;
    // g_march_first_t is this texel's own content distance when it hit cloud.
    var zoom_rel = 0.0;
    if (g_march_first_t > 0.0) {
        zoom_rel = length(camera.light4.xyz) / max(g_march_first_t, 1.0);
    }
    let motion = max(
        clamp(shift_tx - 0.75, 0.0, 1.0),
        smoothstep(0.005, 0.03, zoom_rel),
    );
    var alpha = clamp(
        0.04 + diff * mix(0.05, 0.6, motion),
        0.04,
        mix(0.12, 0.5, motion),
    );
    // Residual parallax error also scales with shift - a floor keeps the
    // trail from persisting even where diff happens to be small.
    alpha = max(alpha, min(0.35, max(shift_tx - 1.0, 0.0) * 0.02));
    // HARD VALIDITY CUT (v0.1245, the operator's radial starburst). Under
    // SUSTAINED flight (FTL descent: km per frame, every frame) the history
    // fetch is displaced many texels along the epipolar direction - radially
    // about the motion epipole, which on a descent IS the nadir under the
    // crosshair. An EMA of radially-displaced fetches paints radial fibres
    // converging exactly where the operator is headed, at every altitude the
    // map serves, and the floor above still kept 65 percent of that smear
    // per step. Beyond ~6 texels of per-frame shift the fetch is advection,
    // not reprojection: take the fresh march outright. Teleports were
    // already guarded (single-frame spike, 15-degree cut); flight was not -
    // which is why parked rig captures converged clean while every
    // operator ARRIVAL repainted the smear.
    // Threshold raised 6 -> 48 (v0.1246, the critic's sweep analysis): the
    // planet-spin content sweep under an inertial camera shifts ~24 texels
    // per frame at low FPS, and reprojection there is geometrically VALID
    // (a coherent fetch N texels away). Cutting at 6 amputated accumulation
    // in exactly that state - every frame became a raw jittered point
    // sample, structurally re-enabling the anchor-centred rosette the
    // supersampler exists to integrate away (the operator's persistent
    // parked starburst). 48 still catches genuinely incoherent jumps well
    // under the 15-degree guard (~430 texels).
    if (shift_tx > 48.0) {
        alpha = 1.0;
    }
    // Resume/sun-delta EMA floor boost (v0.1246): CPU-driven via
    // light7_color.y - 1.0 on resume after a dispatch freeze (the map is
    // stale; fading would replay it), scaled by sun movement otherwise.
    alpha = max(alpha, camera.light7_color.y);
    // Zoom-driven floor, same reasoning as the near resolve (v0.1236): close
    // content under translation mis-reprojects too badly for history to be
    // worth keeping, however small the screen-space shift reads.
    alpha = max(alpha, smoothstep(0.005, 0.03, zoom_rel) * 0.6);
    // No history for this direction (outside the old extent on a resample
    // frame, or a teleport-scale reprojection): start from the fresh
    // march instead of EMA-ing toward zero or smearing stale content.
    if (!have_hist || teleported) {
        alpha = 1.0;
    }
    // Diagnostic mode bypasses the EMA entirely (see map_diag above).
    if (map_diag > 0.5) {
        alpha = 1.0;
    }
    return mix(hist, cur, alpha);
}

// ── The NEAR-FIELD MARCH pass (12e, the march/resolve split) ──────────
//
// 12d's single-pass cadence+history hybrid had a structural ceiling the
// operator hit on the first flight: its 0.25-0.6 blend was too shallow
// to converge the jittered march (clouds "look a lot like static",
// worst on cliff-edge silhouettes) yet still deep enough that stale
// history FADED out over ~half a second instead of dying (the residual
// ghosting). The two complaints pull the blend constant in opposite
// directions - no single-pass constant satisfies both.
//
// 12e is the standard production answer (Decima/Frostbite-class):
// - THIS entry marches EVERY pixel of a QUARTER-res target EVERY frame
//   (no cadence, no history here - march only), with a per-frame
//   SUBPIXEL ray jitter so successive frames sample different subpixel
//   positions (temporal supersampling).
// - A separate RESOLVE pass (assets/shaders/cloud_resolve.wgsl) blends
//   this into the half-res accumulation pair with DEEP accumulation
//   plus VARIANCE-CLIPPED history: the reprojected history is clamped
//   to the mean +- gamma*sigma of the current march's 3x3
//   neighbourhood, so stale content (ghosts) is snapped to plausible
//   values in ONE frame while converged content accumulates 8+ frames
//   deep (static gone). Deep accumulation and instant ghost death stop
//   being a trade-off - that is the whole point of the clip.
//
// Quarter res at full rate costs the same ~220k marches/frame the old
// half-res quarter-cadence did, with none of the cadence artifacts.
//
// MRT: location 0 = the premultiplied march result; location 1 = the
// first-hit distance in KM (R16F: f16 holds 0..65k km with ~0.1%
// relative precision - meters would overflow) for the resolve's
// translation-exact reprojection.
//
// Pads (legacy point-light slots, unread since the storage-buffer light
// list): light0/1/2.xyz = CURRENT camera fwd/right/up; light5.w =
// tan(fov/2), light6.w = aspect; light7.w = frame counter (subpixel
// jitter sequence). light4.xyz + light5/6/7.xyz stay owned by the octa
// pass's reprojection; the resolve pass gets its camera state through
// its own uniform buffer instead.

struct CloudScreenVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_cloud_screen(@builtin(vertex_index) vi: u32) -> CloudScreenVsOut {
    // The classic single fullscreen triangle. The fragment rebuilds its
    // ray from NDC + the camera basis pads, so no shell geometry is
    // involved anywhere in this pass.
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi & 2u) * 2 - 1);
    var out: CloudScreenVsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.ndc = vec2<f32>(x, y);
    return out;
}

struct CloudMarchOut {
    @location(0) color: vec4<f32>,
    @location(1) dist_km: f32,
};

@fragment
fn fs_cloud_screen(in: CloudScreenVsOut) -> CloudMarchOut {
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);
    cloud_set_slab_bounds();
    // ANALYTIC pixel ray from the CURRENT camera basis pads (light0/1/2)
    // - NEVER from shell mesh fragments. The shell icosphere is coarse:
    // near the zenith a triangle's chord sags far below the true sphere,
    // so a ground-level camera sits ABOVE the local mesh surface and the
    // interpolated fragment position yields a DOWNWARD ray for an upward
    // pixel - proven 2026-08-23 by the magenta ground-occlusion sentinel
    // filling the under-deck sky (the operator's vanish-on-approach).
    // A fullscreen triangle with analytic rays has no geometry to sag,
    // and the facing/one-layer discard problem disappears with the mesh.
    //
    // SUBPIXEL jitter (12e): the ray is offset inside its own quarter-res
    // pixel by an R2 low-discrepancy sequence keyed on the frame counter
    // (light7.w) + a per-pixel hash phase, so the resolve's accumulation
    // reconstructs finer-than-quarter detail over ~8 frames. dpdx/dpdy of
    // the interpolated NDC IS this target's per-pixel NDC step - no need
    // to know the resolution here.
    let fidx = camera.light7.w;
    // PCG hash (v0.1242): the old hash21(pos * 0.7071) pair was the same
    // structured-hash disease v0.1237 cured for the DEPTH jitter two lines
    // down - a ~13 px quasi-period whose stratified average keeps the
    // pattern, biasing the subpixel reconstruction the resolve integrates.
    // Salted integer coords decorrelate the two axes properly.
    // ── FROZEN JITTERS (v0.1253.2, the operator's own experiment) ──
    // The cloud_temporal live bisect settled the architecture question:
    // temporal ON vs OFF changed only "soft static vs sharp static",
    // and LOW quality (the direct shell path - one smooth unjittered
    // sample per screen pixel) is the only clean tier. The noise was
    // never something the accumulator could remove, because the noise
    // is IN THE INPUT: three frame-advancing white-noise jitters
    // re-rolled every frame. Production cloud renderers the operator
    // compared against (No Man's Sky, Elite, Helldivers) sample
    // smoothly and let AA do mild cleanup - they do not scatter fresh
    // noise per frame and hope. All three jitters are now FROZEN to
    // static per-pixel patterns: spatial dither survives (it still
    // breaks banding/rings), but a parked frame is pixel-identical to
    // the last - no fizz, no film-grain crawl, no blinking clouds. The
    // fidx advance terms are deliberately gone; do not reintroduce a
    // frame term without re-running the operator's on/off experiment.
    let j2 = vec2<f32>(
        pcg2d_hash(vec2<u32>(in.pos.xy)),
        pcg2d_hash(vec2<u32>(in.pos.xy) + vec2<u32>(0x9E37u, 0x79B9u)),
    );
    let ndc_step = vec2<f32>(abs(dpdx(in.ndc.x)), abs(dpdy(in.ndc.y)));
    let ndc_j = in.ndc + (j2 - vec2<f32>(0.5)) * ndc_step;

    let tanf = max(camera.light5.w, 1.0e-4);
    let aspect = max(camera.light6.w, 1.0e-4);
    let rd_w = normalize(
        camera.light0.xyz
            + camera.light1.xyz * (ndc_j.x * tanf * aspect)
            + camera.light2.xyz * (ndc_j.y * tanf),
    );

    // Depth jitter: decorrelated per pixel, advanced per frame - the
    // resolve's deep accumulation is what integrates it now.
    // Same structured-hash disease as the octa pass had (v0.1237): pixel
    // coords times 0.7182 through fract(x * 0.1031) cycles every ~13 px - a
    // patterned jitter whose stratified average keeps the pattern. The inside-
    // cloud starburst rode on it. Proper integer hash, same as the map pass.
    // WHITE noise (PCG), deliberately - the v0.1252 IGN experiment is
    // REVERTED. IGN's error is spatially STRUCTURED (diagonal gradient
    // ridges), and the resolve's variance-adaptive spatial filter cannot
    // remove structure: the 3x3 mean of a weave is still a weave, and
    // the shaded cloud faces came out printed with a halftone lattice
    // (rig capture 20260830-204146). White noise is exactly what a
    // local mean annihilates - the filter and the jitter must be
    // spectrally matched. Keep any future dither experiment paired with
    // that filter's kernel.
    // FROZEN depth jitter (see the note above): static per-pixel, still
    // decorrelates the step comb spatially so the ladder rings stay
    // dissolved, but never re-rolls.
    // OPERATOR TOGGLE (v0.1254.3; showcase {"cloud_dither":"0"} via pad
    // light7_color.w): the naked-comb forensics photographed the true
    // trade - frozen dither = stable fine grain on overcast sheets;
    // dither OFF = smooth cotton interiors but agate mip-ring arcs on
    // uniform sheets (the melted-flower's actual body, mip-chain
    // response drift of the tiled fields - the design target of the
    // field-calibration increment). Until that lands, the choice is
    // taste, so it is the operator's, live.
    let dither_on = camera.light7_color.w < 0.5;
    let jitter = select(0.5,
        pcg2d_hash(vec2<u32>(in.pos.xy) + vec2<u32>(0xA511u, 0x93D1u)),
        dither_on);
    // FROZEN lod dither: same ring-dissolving job (lodb is monotone in
    // screen radius on a down look; a spatial dither breaks the mip
    // circles), zero temporal churn. Salted to stay decorrelated from
    // the depth jitter.
    g_lod_jitter = select(0.0,
        pcg2d_hash(
            vec2<u32>(in.pos.xy) + vec2<u32>(0x51EDu, 0xB5C9u)) - 0.5,
        dither_on);
    // Footprint = one quarter-res pixel = 4x the screen pixel angle,
    // CAPPED at the octa map's texel angle (regime parity). At planetary
    // range the screen-driven footprint reaches mips 5-6, where the
    // erosion bands flatten into a uniform dim that carves NO areal gaps
    // - the deck renders as a featureless ~white VEIL (the "white
    // continent": a uniform veil whose contrast against land vs ocean
    // faked geography, v0.1204-05 forensics). The cap keeps the erosion
    // mips as fine as the proven octa path so real gaps survive at any
    // distance; near the ground the screen term is finer and wins. (This
    // cap was briefly reverted when a live-MODIS-saturated map made its
    // A/B look like an over-render - the placement, not the footprint,
    // was the white there.)
    // Screen march (the near regime): constructed bodies allowed.
    g_v2_allowed = true;
    // ONE RENDERER (v0.1250): the ownership leash is GONE. This march is
    // the only cloud renderer at every altitude (the octa map is dormant -
    // see lib.rs near_mix), so it renders ALL content its ray crosses; the
    // footprint-proportional stride keeps far content cheap (a few coarse
    // steps at range).
    // FOOTPRINT = the near grid's OWN Nyquist (v0.1244). The old cap
    // min(screen*4, map_texel) forced this quarter-res march to resolve the
    // field ~5x finer than its sampling grid at long slant - undersampling
    // moire over the cellular deck, printing as thin radial combing on a
    // down look (the operator's persistent starburst; named in the 2026-08-29
    // blend forensics and untouched until now). This path anti-aliases to
    // the rate it actually samples at, at every range - the compact-support
    // carve hinge keeps clear-sky footprints clear at coarse mips (the old
    // white-veil class), and the disc-range look is judged on the probe
    // ladder.
    let cur_s = cloud_march_core(
        rd_w, center, shell_r, jitter,
        cloud_pix_ang_screen() * 4.0);

    // First-hit distance for the resolve's reprojection; analytic
    // shell-top hit when the march saw no cloud (clear-sky pixels still
    // need SOME parallax distance so their history tracks).
    var t_rep = g_march_first_t;
    if (t_rep <= 0.0) {
        let ro_c = camera.view_pos.xyz - center;
        let b = dot(ro_c, rd_w);
        let disc = b * b - (dot(ro_c, ro_c) - shell_r * shell_r);
        if (disc > 0.0) {
            let sq = sqrt(disc);
            t_rep = select(-b + sq, -b - sq, -b - sq > 0.0);
        }
    }
    // ── SCREEN-PATH CHANNEL BISECT (v0.1252; showcase {"map_diag":N}) ──
    // The stipple forensics instrument, extended from the octa pass's
    // v0.1249 edition to the one renderer that remains. Renders ONE raw
    // ingredient of this pixel's march as grayscale so a capture can
    // convict the carrier of pixel-scale grain:
    //   1 = coverage alpha, UNLIT (grain here = the density field/its
    //       thresholding); 2 = accumulated direct-sun luminance (grain =
    //       sun taps / powder / cavity-on-direct); 3 = accumulated
    //       ambient luminance (grain = ambient shaping). The resolve and
    //       composite run unchanged - the diag converges like content.
    let diag = camera.light7_color.z;
    var cur_d = cur_s;
    if (diag > 0.5 && diag < 1.5) {
        cur_d = vec4<f32>(cur_s.a, cur_s.a, cur_s.a, 1.0);
    } else if (diag < 2.5 && diag >= 1.5) {
        let l2 = clamp(g_march_sun_acc, 0.0, 1.0);
        cur_d = vec4<f32>(l2, l2, l2, 1.0);
    } else if (diag < 3.5 && diag >= 2.5) {
        let l3 = clamp(g_march_amb_acc * 4.0, 0.0, 1.0);
        cur_d = vec4<f32>(l3, l3, l3, 1.0);
    }
    var out: CloudMarchOut;
    out.color = vec4<f32>(cur_d.rgb * cur_d.a, cur_d.a);
    out.dist_km = max(t_rep, 0.0) * 0.001;
    return out;
}
