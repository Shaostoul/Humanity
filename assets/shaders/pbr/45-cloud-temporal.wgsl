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
    out.uv = vec2<f32>(x, y) * 0.5 + vec2<f32>(0.5);
    return out;
}

@fragment
fn fs_cloud_octa(in: CloudOctaVsOut) -> @location(0) vec4<f32> {
    cloud_set_slab_bounds();
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);
    let rd_w = cloud_map_decode(in.uv, center);
    // History: the ping-pong partner, bound in this pass's albedo slot.
    let hist = textureSampleLevel(albedo_texture, albedo_sampler, in.uv, 0.0);
    // The accumulation sequence: per-texel stratum + the golden-ratio
    // step per cloud-clock tick. Across frames each texel's march visits
    // a low-discrepancy sequence of sample offsets, which is exactly the
    // supersampling the EMA converges.
    let jitter = fract(
        hash21(in.uv * 4096.0 + vec2<f32>(17.0, 39.0))
            + fract(camera.sun_color.w * 11.0) * 0.618034,
    );
    let cur = cloud_march_core(rd_w, center, shell_r, jitter);
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
    let alpha = clamp(0.04 + diff * 0.05, 0.04, 0.12);
    return mix(hist, cur, alpha);
}
