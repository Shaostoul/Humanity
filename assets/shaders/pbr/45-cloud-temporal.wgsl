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
//
// v0.1286: the octa fragment itself is gone (dormant since the v0.1250
// one-renderer cut); this file now holds the near-field screen march
// (fs_cloud_screen) and the sun-shadow cache bake (fs_cloud_light_bake, at
// the end), which reuses the albedo-slot trick for its slice atlas.

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

// (fs_cloud_octa, the octahedral history march, and its vertex entry
// vs_cloud_octa were deleted in v0.1286: dormant since the v0.1250
// one-renderer cut, no pipeline referenced either. The pipeline slot is now
// the sun-shadow cache bake, fs_cloud_light_bake at the end of this file,
// which uses vs_cloud_screen. The vs_cloud_octa mirror-bug note lives on in
// git history at v0.1237 if the v-flip ever needs re-deriving.)

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
    // The pad is a BIT FIELD (v0.1259): bit 0 = dither off, bit 1 = shape
    // frame off. Test bit 0 only, or turning the shape frame off would
    // silently take the dither with it.
    let dither_on = fract(camera.light7_color.w * 0.5) <= 0.25;
    let jitter = select(0.5,
        pcg2d_hash(vec2<u32>(in.pos.xy) + vec2<u32>(0xA511u, 0x93D1u)),
        dither_on);
    // FROZEN lod dither: same ring-dissolving job (lodb is monotone in
    // screen radius on a down look; a spatial dither breaks the mip
    // circles), zero temporal churn. Salted to stay decorrelated from
    // the depth jitter.
    // ── THE RING CURE IS NOT A LOOK PREFERENCE (v0.1270) ──
    // This used to ride on dither_on, so ONE checkbox drove two unrelated
    // jobs: the depth jitter (a look trade - it buys smoothness and costs
    // agate arcs on overcast) and this, the 2026-08-24 cure for mip circles.
    // The operator turned the dither off to kill what they called TV static,
    // which silently switched the ring cure off too and brought the radial
    // artifact back at full strength. Measured at 9.2 km with the clock
    // pinned: direct-sun radial energy 23.97 with the cure on, 28.11 with it
    // off, against a 0.7 noise floor - the largest single effect found in the
    // whole rosette arc. So the two now have separate switches and this one
    // defaults ON. It is frozen per pixel (no temporal churn), so it costs
    // fixed grain, never the moving static.
    let ring_cure_on = fract(camera.light7_color.w * 0.03125) < 0.5;
    g_lod_jitter = select(0.0,
        pcg2d_hash(
            vec2<u32>(in.pos.xy) + vec2<u32>(0x51EDu, 0xB5C9u)) - 0.5,
        ring_cure_on);
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
    // FOOTPRINT = this march grid's OWN angular pixel size, computed from
    // the rasterizer instead of assumed (v0.1255): ndc_step.y is 2/march_rows
    // by construction, so ndc_step.y * tanf IS the march pixel's angular
    // width. At the historical quarter-res this equals the old
    // cloud_pix_ang_screen() * 4.0 exactly, and it now TRACKS the cloud
    // resolution setting automatically - raising the resolution actually
    // buys finer field detail instead of just a sharper upsample.
    let cur_s = cloud_march_core(
        rd_w, center, shell_r, jitter,
        ndc_step.y * tanf);

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
    } else if (diag < 4.5 && diag >= 3.5) {
        // ── MARCH STEP COUNT (v0.1268) ──
        // Rebuilt instrument. The v0.1242 comment in 40-clouds.wgsl records
        // that an iteration-count diagnostic once existed and that its
        // contours matched the flower rings the operator was seeing - then it was
        // deleted along with the fix it justified, so five later rosette
        // hunts had to re-derive the same question by argument. It is a
        // permanent channel now. White = this ray spent the whole 224-step
        // budget, which means its tail was integrated in ONE giant step.
        let l4 = clamp(g_march_iters / f32(CLOUD_STEP_ITER_CAP), 0.0, 1.0);
        cur_d = vec4<f32>(l4, l4, l4, 1.0);
    } else if (diag >= 9.5 && diag < 10.5) {
        // ── PROFILE SHARE (increment 4, the far rung) ── w_pf accumulated
        // with the colour's own weights: white = the sample was drawn from
        // the planet-fixed profile, black = the marched field. Must read 0
        // at the nadir below each arm's footprint floor (the prove-red).
        let l10 = clamp(g_march_pf_acc, 0.0, 1.0);
        cur_d = vec4<f32>(l10, l10, l10, 1.0);
    } else if (diag >= 10.5 && diag < 11.5) {
        // ── PROFILE LEVEL (increment 4) ── level / 6 (6 = the global): a
        // forced level renders a flat L / 6 frame; auto a monotone staircase
        // of rounded squares from the nadir outward; HARD the same with
        // sharp treads. The window edges are visible here by design.
        let l11 = clamp(g_march_lvl_acc, 0.0, 1.0);
        cur_d = vec4<f32>(l11, l11, l11, 1.0);
    } else if (diag >= 11.5 && diag < 12.5) {
        // ── PROFILE FRACTION (increment 4) ── pf.f. Against the A17 synthetic
        // atlas this is the i / 512 sawtooth, PLANET-FIXED: it must not slide
        // when the camera moves (a sliding ramp is a scroll bug).
        let l12 = clamp(g_march_frac_acc, 0.0, 1.0);
        cur_d = vec4<f32>(l12, l12, l12, 1.0);
    } else if (diag >= 8.5 && diag < 9.5) {
        // ── SUN SOURCE (increment 1, v0.1286) ── which sun-shadow source lit
        // each sample, accumulated with the colour's own weights: fine
        // window white (1.0), coarse window grey (0.5), the ladder /
        // analytic fallback dark (0.15). Shows exactly where the window
        // edges fall so the ring gates know where to look.
        let l9 = clamp(g_march_src_acc, 0.0, 1.0);
        cur_d = vec4<f32>(l9, l9, l9, 1.0);
    } else if (diag >= 7.5) {
        // ── BURIAL PROFILE (v0.1280) ── where the in-cloud light engages.
        let l8 = clamp(g_march_prof_acc, 0.0, 1.0);
        cur_d = vec4<f32>(l8, l8, l8, 1.0);
    } else if (diag >= 5.5) {
        // ── ENTRY DEPTH (v0.1272) ──
        // How far below the last clear sample the first accepted sample
        // landed, 0-600 m -> black-white. The old march: a per-pixel hash
        // (311 +- 280 m at 1.5 km, the twin). The sample-anchored march:
        // flat, <= 30 m. Prove it red on the old march before believing green.
        let l6 = clamp(g_march_first_depth_m / 600.0, 0.0, 1.0);
        cur_d = vec4<f32>(l6, l6, l6, 1.0);
    } else if (diag >= 4.5) {
        // Same count as a sawtooth: every 8 steps prints a contour band, so
        // the step-count STAIRCASE in screen radius is visible directly
        // rather than inferred from a smooth ramp.
        let l5 = fract(g_march_iters * 0.125);
        cur_d = vec4<f32>(l5, l5, l5, 1.0);
    }
    var out: CloudMarchOut;
    out.color = vec4<f32>(cur_d.rgb * cur_d.a, cur_d.a);
    out.dist_km = max(t_rep, 0.0) * 0.001;
    return out;
}

// ── THE SUN-SHADOW CACHE BAKE (increment 1, v0.1286) ─────────────────────
//
// Replaces the dead fs_cloud_octa (the octahedral history march; dormant
// since the v0.1250 one-renderer cut, see lib.rs near_mix). This fragment
// runs fullscreen over the R16F slice ATLAS (15360 x 256 texels, see the
// CLOUD_LC_* block in 40-clouds.wgsl) and writes, per texel, the sun
// ladder's rungs 2-11 optical depth at ONE planet-fixed lattice point. The
// march then reads the atlas with one trilinear tap instead of walking ten
// density evaluations per view sample.
//
// Rust (renderer::cloud_temporal::CloudLightCache) sets a scissor rect over
// one eighth of each window's slices per frame in a fixed order (a full
// refresh every 8 frames) and bakes a window whole on a re-anchor or when
// the sun has moved more than 2 degrees. The vertex entry is the plain
// fullscreen triangle vs_cloud_screen.
//
// Nothing view-dependent enters: the point's regime comes from ITS OWN
// direction (cloud_regime(cloud_type_coord(normalize(point), ...)), the
// rule BUG-074 established), the weather from that direction, the sun from
// camera.sun_direction like the march, and the cone-spiral phase is 0 (no
// per-pixel jitter on a planet-fixed lattice). g_deep_sample stays at its
// 0 default, so the bit-20 experiment's skip never fires here.
//
// Bindings are the standard groups 0-3, bound like the march pass: camera,
// the cloud SHELL's object uniform (obj_normal_matrix gives the planet
// frame the sun direction is rotated into), the cloud material, and the
// texture group (the noise volumes; the albedo slot is unread here).
@fragment
fn fs_cloud_light_bake(in: CloudScreenVsOut) -> @location(0) vec4<f32> {
    cloud_set_slab_bounds();
    // Which texel: @builtin(position) is the pixel CENTRE (x.5, y.5), so the
    // truncation yields the integer texel index.
    let px = vec2<u32>(in.pos.xy);
    let x = f32(px.x);
    let j = f32(px.y);
    // Decode (window, k, i, j) from the packing: fine slices first, k * 256
    // + i across x; the coarse slices start at CLOUD_LC_COARSE_X0 with
    // k * 128 + i. Rows beyond a window's height (the coarse rows 128..255)
    // hold nothing and write 0.
    var anchor = camera.light3_color.xyz;
    var cell_h = camera.light3_color.w;
    var cell_v_m = CLOUD_LC_FINE_CELL_V_M;
    var nx = CLOUD_LC_FINE_NX;
    var nz = CLOUD_LC_FINE_NZ;
    var xs = x;
    if (x >= CLOUD_LC_COARSE_X0) {
        anchor = camera.light4_color.xyz;
        cell_h = camera.light4_color.w;
        cell_v_m = CLOUD_LC_COARSE_CELL_V_M;
        nx = CLOUD_LC_COARSE_NX;
        nz = CLOUD_LC_COARSE_NZ;
        xs = x - CLOUD_LC_COARSE_X0;
    }
    let k = floor(xs / nx);
    let i = xs - k * nx;
    if (j >= nx || k >= nz || cell_h <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let cell_v = cell_v_m * 0.001 * g_cloud_upkm;
    // The lattice point in the march's own p-space (the inverse of the
    // read side's coordinate mapping, one function shared by both).
    let point = light_cache_point(anchor, cell_h, cell_v, nx, i, j, k);

    let t = camera.sun_color.w;
    let seed = material.params.x;
    let coverage = material.base_color.a;
    let inv_model = transpose(obj_normal_matrix());
    let sun = normalize(camera.sun_direction.xyz);
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);

    // Regime and weather at the point's OWN direction (BUG-074 stays dead
    // by construction: no ray midpoint, no camera anywhere in this).
    let dirp = normalize(point);
    let reg = cloud_regime(cloud_type_coord(dirp, t, seed));
    let wind_ang = t * cloud_wind_omega(reg.wind_lo);
    // The weather tap's band limit is the lattice cell (190 m or 760 m),
    // both far below the 27.8 km weather texel, so mip 0, the same value
    // every near view sample uses.
    let wlod = max(log2(max(cell_h / g_cloud_upkm / 27.8, 1.0)), 0.0);
    let weather_a = clamp(
        cloud_alpha_from_field(
            cloud_weather_adv(dirp, t, seed, wind_ang, wlod), coverage)
            + reg.cover_bias, 0.0, 1.0);
    // The component bisect (dev pad bits 13-15) applies to the bake exactly
    // as it does to the march, so an A/B with one term off stays an A/B.
    let bis = cloud_bisect_index();
    let detail_amt = select(1.0, 0.0, bis == 1u);
    let puff_amt = select(1.0, 0.0, bis == 2u);
    let cell_amt = select(1.0, 0.0, bis == 3u);
    // Constructed bodies (Ultra) are welcome, as in the screen march: the
    // profile body the ladder taps IS the built cluster at that tier.
    g_v2_allowed = true;
    // The body's surface-displacement mip is SHAPE and must not depend on a
    // viewer; the lattice has none, so the world-anchored value the
    // cloud_world_shape experiment uses stands in for every lattice point.
    g_v2_disp_lod = CLOUD_V2_SHAPE_LOD_WORLD;
    // Rungs 2-11 from this lattice point, rung 0-1 depth 0: the per-pixel
    // rungs are added by the reader. No view footprint enters (v0.1264:
    // each tap band-limits by its own segment), so the stored quantity is
    // the same whatever the camera is doing.
    let tau = cloud_sun_tau_far(
        point, sun_local, t, seed, weather_a, reg,
        detail_amt, puff_amt, cell_amt, 0.0);
    // Flag hygiene (the ladder sets the profile flag; this is its exit).
    g_sun_profile = 0.0;
    return vec4<f32>(clamp(tau, 0.0, CLOUD_LC_TAU_MAX), 0.0, 0.0, 1.0);
}

// ── THE CLOUD PROFILE BAKE (perf arc increment 4, the far rung) ──────────
//
// Contract: docs/design/cloud-far-rung.md, "The bake fragment". The constants,
// lattice and slice layout live in the CLOUD_FR_* block of 40-clouds.wgsl.
// One RGBA8Unorm target = the profile atlas MIP 0 (6144 x 3584); group 3
// binding 14 = the atlas's MIP 1 view (which holds the calibration table; mip
// 0 is this pass's attachment and may not also be sampled). Bound like the sun
// bake: camera, the cloud SHELL's object slot, the cloud material. Rust
// scissors the rects (scroll columns / rows, fills, rolling refresh rows, the
// global's pass rows) and this fragment decodes, from @builtin(position)
// alone, which planet cell and which slice it writes. No ray, no camera
// anywhere (BUG-074 stays dead by construction).
//
// What one texel holds, and how it is made (contract "The bake fragment"):
//   1. DECODE: from the texel position alone, which planet cell (its centre
//      direction dir_c and its extent in km) and which slice (a pair slice
//      holds two height bins of (fraction f, mean density G); a column
//      slice holds the encoded columns C_k = sum of G above bin k).
//   2. The NOISE part, every tier: cloud_density_hi at the cell's own mip
//      (lodb = log2(cell km), where the compact carve hinge IS the cell
//      mean) at four heights per bin, with the constructed bodies OFF.
//      f_n = the hinge's own areal fraction scaled by the erosion survivor
//      ratio, max over the heights; G_n = the mean density.
//   3. The BUILT part, Ultra only: every cv2 cell whose cloud can touch this
//      profile cell is enumerated (the body only ever searches a 3x3
//      neighbourhood, so the one-cell margin is exact), placed by the SAME
//      cv2_place the march uses, and replaced by an ellipse per height
//      from the calibration table (per archetype and cloud-relative height:
//      equivalent-circle radius ratio and mean in-cloud density, measured
//      once on the shipped SDF). Fine levels (cells <= 1 km) count 16
//      stratified points of the profile cell inside the ellipse; coarse
//      levels and the global attribute the analytic ellipse area to the
//      cell holding the cloud's centre. Clouds union (or Poisson-union past
//      the enumeration cap), and the built and noise parts combine exactly
//      as cloud_carve's sheet union does.
//   4. REFERENCE (knob 9, dev): the same texel point-sampled from the field
//      itself (8 x 8 x 2 per bin, bodies on), the analytic bake's truth.
//   5. WRITE: pooled to four bins for the global; columns sqrt-encoded.
@fragment
fn fs_cloud_profile_bake(in: CloudScreenVsOut) -> @location(0) vec4<f32> {
    cloud_set_slab_bounds();
    // Which texel: @builtin(position) is the pixel CENTRE (x.5, y.5), so the
    // truncation yields the integer texel index.
    let px = vec2<u32>(in.pos.xy);
    let x = i32(px.x);
    let y = i32(px.y);
    let planet_km = material.params2.z;
    if (planet_km < 0.5) {
        return vec4<f32>(0.0);
    }
    // ── 1. DECODE ──
    // The bake's per-texel inputs, whichever region this texel lies in: the
    // cell centre (lon_c, lat_c), the cell's extent in km (north-south, and
    // east-west at the centre latitude), its angular half-extent dl (both
    // axes: the lattice is equal-angle), and the slice's role.
    var lon_c = 0.0;
    var lat_c = 0.0;
    var cell_km_lat = 0.0;
    var cell_km_lon = 0.0;
    var dl = 0.0;
    var is_global = false;
    var is_column = false;
    // The slice index inside its region: window 0..8 (pairs 0..5, columns
    // 6..8), global 0..2 (pairs 0, 1, the column 2).
    var p_slice = 0;
    var L = 0;
    // The per-texel hash key (the cell's planet index): the fine-level test
    // points and the stride phase are jittered by it.
    var key_i = 0;
    var key_j = 0;
    if (y >= CLOUD_FR_GLOBAL_Y0) {
        // ── the global equirect map ──
        is_global = true;
        let slice = x / CLOUD_FR_GLOBAL_W;          // 0 pair0, 1 pair1, 2 column
        let i = x - slice * CLOUD_FR_GLOBAL_W;      // x mod 2048
        let j = y - CLOUD_FR_GLOBAL_Y0;
        is_column = slice >= 2;
        p_slice = slice;
        key_i = i;
        key_j = j;
        // Texel centre; row 0 = north, matching the weather map's w_uv.
        lon_c = (f32(i) + 0.5) / f32(CLOUD_FR_GLOBAL_W) * TAU - PI;
        lat_c = 0.5 * PI - (f32(j) + 0.5) / f32(CLOUD_FR_GLOBAL_H) * PI;
        let global_km = TAU * planet_km / f32(CLOUD_FR_GLOBAL_W);
        cell_km_lat = global_km;
        cell_km_lon = global_km * cos(lat_c);
        dl = PI / f32(CLOUD_FR_GLOBAL_W);           // half of 2 pi / 2048 = half of pi / 1024
    } else {
        // ── a window slice ──
        let s = (y / CLOUD_FR_NX) * CLOUD_FR_SLICE_COLS + x / CLOUD_FR_NX;
        if (s >= CLOUD_FR_LEVELS * CLOUD_FR_SLICES_PER_LEVEL) {
            return vec4<f32>(0.0);      // row 4, columns 6..11: spare at mip 0
        }
        L = s / CLOUD_FR_SLICES_PER_LEVEL;
        let p = s % CLOUD_FR_SLICES_PER_LEVEL;
        let sx = x % CLOUD_FR_NX;
        let sy = y % CLOUD_FR_NX;
        is_column = p >= CLOUD_FR_PAIRS;
        p_slice = p;
        let c_km = CLOUD_FR_CELL0_KM * exp2(f32(L));
        let cell_rad = c_km / planet_km;
        let NI = floor(TAU * planet_km / c_km);
        let NJ = floor(PI * planet_km / c_km);
        // The window origin from the ground cell (pad), per level.
        let half = CLOUD_FR_NX / 2;
        let I0 = i32(floor(camera.light2_color.x / exp2(f32(L)))) - half;
        let J0 = i32(floor(camera.light2_color.y / exp2(f32(L)))) - half;
        // The window-frame cell this storage texel holds (toroidal).
        let I_abs = I0 + pmod(sx - I0, CLOUD_FR_NX);
        let J_abs = J0 + pmod(sy - J0, CLOUD_FR_NX);
        if (J_abs < 0 || f32(J_abs) >= NJ) {
            return vec4<f32>(0.0);      // void rows beyond the poles
        }
        let I = pmod(I_abs, i32(NI));
        key_i = I;
        key_j = J_abs;
        lon_c = (f32(I) + 0.5) * cell_rad - PI;
        lat_c = (f32(J_abs) + 0.5) * cell_rad - 0.5 * PI;
        cell_km_lat = c_km;
        cell_km_lon = c_km * cos(lat_c);
        dl = 0.5 * cell_rad;
    }
    let dir_c = vec3<f32>(cos(lat_c) * cos(lon_c), sin(lat_c), -cos(lat_c) * sin(lon_c));

    // The field's inputs at this cell (no ray, no camera: BUG-074 stays dead
    // by construction). The regime at the cell's OWN direction, its base
    // wind, the weather band limit at the cell's own size, the component
    // bisect exactly as the march and the sun bake apply it.
    let t = camera.sun_color.w;
    let seed = material.params.x;
    let coverage = material.base_color.a;
    let tc_c = cloud_type_coord(dir_c, t, seed);
    let reg = cloud_regime(tc_c);
    let wind_ang = t * cloud_wind_omega(reg.wind_lo);
    let wlod = max(log2(max(cell_km_lat / 27.8, 1.0)), 0.0);
    let bis = cloud_bisect_index();
    let detail_amt = select(1.0, 0.0, bis == 1u);
    let puff_amt = select(1.0, 0.0, bis == 2u);
    let cell_amt = select(1.0, 0.0, bis == 3u);
    let knob = cloud_profile_knob();
    let ref_mode = knob == CLOUD_FR_KNOB_REF;
    // Slab geometry: one bin is 1/12 of the slab; heights in metres above
    // the slab base feed the built part.
    let slab_p = g_cloud_rt - g_cloud_rb;
    let slab_m = slab_p / max(g_cloud_upkm, 1.0e-9) * 1000.0;

    // ── The bin set of this texel ──
    // A pair slice p computes bins 2p, 2p + 1. A column slice q = p - 6
    // holds C_4q .. C_4q+3 and needs bins 4q + 1 .. 11; the last one also
    // carries T, which needs bin 0, so it computes all twelve. The global
    // pair slices compute the six slab bins of their two pooled bins; the
    // global column slice all twelve.
    var k_lo = 0;
    var k_hi = CLOUD_FR_NZ - 1;
    if (is_global) {
        if (!is_column) {
            k_lo = 6 * p_slice;
            k_hi = k_lo + 5;
        }
    } else if (!is_column) {
        k_lo = 2 * p_slice;
        k_hi = k_lo + 1;
    } else {
        let q = p_slice - CLOUD_FR_PAIRS;
        k_lo = select(4 * q + 1, 0, q == CLOUD_FR_CSLICES - 1);
    }
    // Per-bin results (only k_lo..k_hi are written; the rest stay 0 and are
    // never read by this texel's write).
    var f_k: array<f32, CLOUD_FR_NZ>;
    var G_k: array<f32, CLOUD_FR_NZ>;
    for (var k = 0; k < CLOUD_FR_NZ; k = k + 1) {
        f_k[k] = 0.0;
        G_k[k] = 0.0;
    }

    if (ref_mode) {
        // ── 4. THE REFERENCE BAKE (knob 9, dev only, slow) ──
        // The same texel from the field itself: CLOUD_FR_REF_K^2 points
        // stratified across the cell, CLOUD_FR_REF_KZ heights per bin, the
        // FULL cloud_density_hi (constructed bodies on, each point at its own
        // footprint lodb = log2(cell / 8), the weather at each point's own
        // direction). f = the fraction of points with density above the
        // interior gate at either height; G = the mean over all points.
        g_v2_allowed = true;
        g_sun_profile = 0.0;
        g_lod_jitter = 0.0;
        let lodb_pt = log2(max(cell_km_lat / f32(CLOUD_FR_REF_K), 1.0e-4));
        g_v2_disp_lod = lodb_pt;
        // The 64 point directions' weather alphas, once (height-independent).
        var wa_pts: array<f32, 64>;
        for (var n = 0; n < 64; n = n + 1) {
            let a = n % CLOUD_FR_REF_K;
            let b = n / CLOUD_FR_REF_K;
            let lon_p = lon_c + ((f32(a) + 0.5) / f32(CLOUD_FR_REF_K) - 0.5) * 2.0 * dl;
            let lat_p = lat_c + ((f32(b) + 0.5) / f32(CLOUD_FR_REF_K) - 0.5) * 2.0 * dl;
            let dir_p = vec3<f32>(cos(lat_p) * cos(lon_p), sin(lat_p), -cos(lat_p) * sin(lon_p));
            wa_pts[n] = clamp(
                cloud_alpha_from_field(
                    cloud_weather_adv(dir_p, t, seed, wind_ang, wlod), coverage)
                    + reg.cover_bias, 0.0, 1.0);
        }
        let n_pts = CLOUD_FR_REF_K * CLOUD_FR_REF_K;
        for (var k = k_lo; k <= k_hi; k = k + 1) {
            var n_in = 0;
            var dsum = 0.0;
            for (var n = 0; n < n_pts; n = n + 1) {
                let a = n % CLOUD_FR_REF_K;
                let b = n / CLOUD_FR_REF_K;
                let lon_p = lon_c + ((f32(a) + 0.5) / f32(CLOUD_FR_REF_K) - 0.5) * 2.0 * dl;
                let lat_p = lat_c + ((f32(b) + 0.5) / f32(CLOUD_FR_REF_K) - 0.5) * 2.0 * dl;
                let dir_p = vec3<f32>(cos(lat_p) * cos(lon_p), sin(lat_p), -cos(lat_p) * sin(lon_p));
                var any_in = false;
                for (var s = 0; s < CLOUD_FR_REF_KZ; s = s + 1) {
                    let h_s = (f32(k) + (f32(s) + 0.5) / f32(CLOUD_FR_REF_KZ)) / f32(CLOUD_FR_NZ);
                    let r_s = g_cloud_rb + h_s * slab_p;
                    let dens = cloud_density_hi(
                        dir_p * r_s, t, seed, wa_pts[n], reg,
                        detail_amt, puff_amt, cell_amt, lodb_pt).x;
                    any_in = any_in || dens > CLOUD_STEP_INTERIOR_GATE;
                    dsum = dsum + dens;
                }
                n_in = n_in + select(0, 1, any_in);
            }
            f_k[k] = f32(n_in) / f32(n_pts);
            G_k[k] = dsum / f32(n_pts * CLOUD_FR_REF_KZ);
        }
    } else {
        // ── 2. THE NOISE PART (every tier) ──
        // The weather alpha at the cell centre (height-independent, once),
        // then per (bin, height) the noise density at the cell's own mip
        // with the bodies OFF. The fraction is the compact hinge's own areal
        // fraction (g_cloud_frac, published by cloud_carve) scaled by how
        // much of the pre-erosion carve survived erosion at this height,
        // unioned (max) over the heights; the mean density is the plain mean.
        let wa_c = clamp(
            cloud_alpha_from_field(
                cloud_weather_adv(dir_c, t, seed, wind_ang, wlod), coverage)
                + reg.cover_bias, 0.0, 1.0);
        let lodb_cell = log2(max(cell_km_lat, 1.0e-4));
        g_v2_allowed = false;
        g_sun_profile = 0.0;
        g_lod_jitter = 0.0;
        g_v2_disp_lod = CLOUD_V2_SHAPE_LOD_WORLD;
        for (var k = k_lo; k <= k_hi; k = k + 1) {
            var f_n = 0.0;
            var G_n = 0.0;
            for (var s = 0; s < CLOUD_FR_ZSUB; s = s + 1) {
                let h_s = (f32(k) + (f32(s) + 0.5) / f32(CLOUD_FR_ZSUB)) / f32(CLOUD_FR_NZ);
                let r_s = g_cloud_rb + h_s * slab_p;
                let dens_s = cloud_density_hi(
                    dir_c * r_s, t, seed, wa_c, reg,
                    detail_amt, puff_amt, cell_amt, lodb_cell).x;
                let frac_s = g_cloud_frac
                    * clamp(dens_s / max(g_cloud_carve_pt, 1.0e-3), 0.0, 1.0);
                f_n = max(f_n, frac_s);
                G_n = G_n + dens_s;
            }
            f_k[k] = f_n;
            G_k[k] = G_n / f32(CLOUD_FR_ZSUB);
        }

        // ── 3. THE BUILT PART (Ultra only, from the calibration table) ──
        // Gated on the calibration flag (pad bit 8): before the table exists
        // the built part is zero, and Rust orders a FILL of every level plus
        // a fast global pass once the calibration lands.
        let arch_i = cv2_arch_index(tc_c);
        if (material.params.y >= 2.5 && arch_i >= 0 && cloud_profile_flag(8)) {
            // The cv2 grid this family places on (41-cloud-bodies.wgsl: cells
            // of g km, longitude cells scaled by 1 / cos(lat)), and this
            // profile cell's extent in cv2 grid coordinates at the centre
            // latitude (the body's own cx / cy law).
            let g_km = cv2_cell_km(arch_i);
            let cell_rad_cv2 = g_km / planet_km;
            let coslat_c = max(cos(lat_c), 0.05);
            let cx_scale = cell_rad_cv2 / coslat_c;
            let cx_lo = (lon_c - dl) / cx_scale;
            let cx_hi = (lon_c + dl) / cx_scale;
            let cy_lo = (lat_c - dl) / cell_rad_cv2;
            let cy_hi = (lat_c + dl) / cell_rad_cv2;
            let cx_c = lon_c / cx_scale;
            let cy_c = lat_c / cell_rad_cv2;
            // The one-cell margin is EXACT: the body only ever searches the
            // 3x3 neighbourhood of a sample's cell, so no cloud beyond one cv2
            // cell of this profile cell's extent can touch it.
            let ci_lo = floor(cx_lo) - 1.0;
            let ci_hi = floor(cx_hi) + 1.0;
            let cj_lo = floor(cy_lo) - 1.0;
            let cj_hi = floor(cy_hi) + 1.0;
            let n_cols = i32(ci_hi - ci_lo) + 1;
            let n_rows = i32(cj_hi - cj_lo) + 1;
            let n_cand = n_cols * n_rows;
            // Stride subsampling past the cap (A3) with a per-texel hash
            // phase on both axes; the union then takes the Poisson form. On
            // Earth the cap is never reached (worst case 400 < 512 at the
            // global's equatorial humilis rows), so stride is 1 everywhere
            // here and a larger planet is where it engages.
            var stride = 1;
            if (n_cand > CLOUD_FR_MAX_CV2) {
                stride = i32(ceil(sqrt(f32(n_cand) / f32(CLOUD_FR_MAX_CV2))));
            }
            let ph = floor(cv2_hash(vec2<f32>(f32(key_i), f32(key_j)), 41.0) * f32(stride));
            let m_per_cell = g_km * 1000.0;
            // The profile cell in metres: half-extents, area, and (fine
            // levels only) the 16 cell-stratified test points relative to
            // the cell centre, jittered per texel so adjacent cells do not
            // share one comb.
            let half_x = 0.5 * cell_km_lon * 1000.0;
            let half_y = 0.5 * cell_km_lat * 1000.0;
            let cell_area = cell_km_lon * cell_km_lat * 1.0e6;
            let fine = !is_global && L <= 2;
            var ptx: array<f32, CLOUD_FR_PTS>;
            var pty: array<f32, CLOUD_FR_PTS>;
            if (fine) {
                // Only the fine levels test points; the coarse form (levels
                // 3..5 and the global, the bulk of the pass) attributes
                // whole clouds and never reads ptx/pty, so the 32 hashes
                // are skipped there.
                for (var n = 0; n < CLOUD_FR_PTS; n = n + 1) {
                    let jx = cv2_hash(vec2<f32>(f32(key_i), f32(key_j)), 43.0 + f32(n));
                    let jy = cv2_hash(vec2<f32>(f32(key_i), f32(key_j)), 43.0 + f32(CLOUD_FR_PTS) + f32(n));
                    let qx = (f32(n % 4) + jx) / 4.0 - 0.5;
                    let qy = (f32(n / 4) + jy) / 4.0 - 0.5;
                    ptx[n] = qx * cell_km_lon * 1000.0;
                    pty[n] = qy * cell_km_lat * 1000.0;
                }
            }
            // The calibration table row for this texel's family, hoisted
            // once per fragment: arch_i is uniform over the texel and the
            // table is CLOUD_FR_CALIB_ROWS texels wide, so reading it per
            // (candidate, bin, height) slot would be ~10k dependent fetches
            // on an equatorial global texel (the critic's cost finding).
            // The bound view is mip 1: the reduced table (stage 2 of the
            // calibration, fs_cloud_profile_calib_reduce). .r = rho (the
            // equivalent-circle radius as a fraction of the width), .g =
            // Dbar (the mean in-cloud density) per height row.
            var cal_rho: array<f32, CLOUD_FR_CALIB_ROWS>;
            var cal_dbar: array<f32, CLOUD_FR_CALIB_ROWS>;
            for (var rr = 0; rr < CLOUD_FR_CALIB_ROWS; rr = rr + 1) {
                let cal = textureLoad(tree_atlas_tex,
                    vec2<i32>(CLOUD_FR_CALIB_X0 + rr, CLOUD_FR_CALIB_Y0 + arch_i), 0);
                cal_rho[rr] = cal.r;
                cal_dbar[rr] = cal.g;
            }
            // Accumulators per (bin, height) slot: the union product, the
            // area sum (the Poisson form and the density weight) and the
            // area-weighted in-cloud density.
            var acc_u: array<f32, CLOUD_FR_NZ * CLOUD_FR_ZSUB>;
            var acc_a: array<f32, CLOUD_FR_NZ * CLOUD_FR_ZSUB>;
            var acc_ad: array<f32, CLOUD_FR_NZ * CLOUD_FR_ZSUB>;
            for (var n = 0; n < CLOUD_FR_NZ * CLOUD_FR_ZSUB; n = n + 1) {
                acc_u[n] = 1.0;
                acc_a[n] = 0.0;
                acc_ad[n] = 0.0;
            }
            for (var a = 0; a < n_cols; a = a + stride) {
                let ci = ci_lo + ph + f32(a);
                if (ci > ci_hi) {
                    break;
                }
                for (var b = 0; b < n_rows; b = b + stride) {
                    let cj = cj_lo + ph + f32(b);
                    if (cj > cj_hi) {
                        break;
                    }
                    // Presence is a threshold on the weather alpha, so it is
                    // read at the cv2 cell's OWN centre (A2): one value per
                    // profile cell would flip whole clusters at once.
                    let lon_i = (ci + 0.5) * cx_scale;
                    let lat_i = (cj + 0.5) * cell_rad_cv2;
                    let dir_i = vec3<f32>(cos(lat_i) * cos(lon_i), sin(lat_i), -cos(lat_i) * sin(lon_i));
                    let wa_i = clamp(
                        cloud_alpha_from_field(
                            cloud_weather_adv(dir_i, t, seed, wind_ang, wlod), coverage)
                            + reg.cover_bias, 0.0, 1.0);
                    // The body's own clear-sky gate on the weather alpha.
                    if (wa_i <= 0.02) {
                        continue;
                    }
                    let pl = cv2_place(ci, cj, arch_i, wa_i, g_km);
                    if (!pl.present) {
                        continue;
                    }
                    // The profile cell centre relative to the cloud centre,
                    // metres east (x) and north (y) in the tangent plane: the
                    // same convention as the body's (ox, oy) for a sample.
                    let ox = (cx_c - pl.centre_cells.x) * m_per_cell;
                    let oy = (cy_c - pl.centre_cells.y) * m_per_cell;
                    // Coarse form: the whole cloud is attributed to the cell
                    // holding its centre.
                    let in_cell = abs(ox) <= half_x && abs(oy) <= half_y;
                    if (!fine && !in_cell) {
                        continue;
                    }
                    // The cloud's height in the shape frame, and its
                    // wind-frame semi-axes per unit calibration ratio: the
                    // calibration measures the equivalent-circle radius as a
                    // fraction of the WIDTH (rho = r_eq / width), so the
                    // semi-axes are width * rho times the frame's stretch.
                    let h_cloud = max(pl.sy * pl.height_m, 1.0);
                    let a_ax0 = pl.width_m * pl.sx;
                    let b_ax0 = pl.width_m * pl.sz;
                    for (var k = k_lo; k <= k_hi; k = k + 1) {
                        for (var s = 0; s < CLOUD_FR_ZSUB; s = s + 1) {
                            let h_s = (f32(k) + (f32(s) + 0.5) / f32(CLOUD_FR_ZSUB)) / f32(CLOUD_FR_NZ);
                            let up_m_s = h_s * slab_m;
                            let y_rel = up_m_s / h_cloud;
                            if (y_rel >= CLOUD_FR_CALIB_YMAX) {
                                continue;
                            }
                            let row = clamp(i32(floor(y_rel / CLOUD_FR_CALIB_YMAX * f32(CLOUD_FR_CALIB_ROWS))), 0, CLOUD_FR_CALIB_ROWS - 1);
                            // The hoisted table row (fetched once per
                            // fragment above).
                            let rho = cal_rho[row];
                            let dbar = cal_dbar[row];
                            if (rho <= 0.0) {
                                continue;
                            }
                            let a_ax = a_ax0 * rho;
                            let b_ax = b_ax0 * rho;
                            var a_i = 0.0;
                            if (fine) {
                                // 16 points of the profile cell, rotated into
                                // the cloud's wind frame, tested against the
                                // ellipse.
                                var hit = 0;
                                for (var n = 0; n < CLOUD_FR_PTS; n = n + 1) {
                                    let qx = ptx[n] + ox;
                                    let qy = pty[n] + oy;
                                    let wu = qx * pl.cwx + qy * pl.cwy;
                                    let wv = -qx * pl.cwy + qy * pl.cwx;
                                    let eu = wu / a_ax;
                                    let ev = wv / b_ax;
                                    if (eu * eu + ev * ev <= 1.0) {
                                        hit = hit + 1;
                                    }
                                }
                                a_i = f32(hit) / f32(CLOUD_FR_PTS);
                            } else {
                                a_i = min(PI * a_ax * b_ax / cell_area, 1.0);
                            }
                            a_i = clamp(a_i, 0.0, 1.0);
                            let idx = k * CLOUD_FR_ZSUB + s;
                            acc_u[idx] = acc_u[idx] * (1.0 - a_i);
                            acc_a[idx] = acc_a[idx] + a_i;
                            acc_ad[idx] = acc_ad[idx] + a_i * dbar;
                        }
                    }
                }
            }
            // Per (bin, height): the union (exact at stride 1, Poisson past
            // the cap) and the area-weighted density, never above f_b. Per
            // bin: the union over heights (max) and the mean density. Then
            // the combine, exactly as cloud_carve's sheet union: the built
            // weight ramps in over the convective boundary of the type
            // coordinate, and past ~0.6 sky-wide coverage the noise field is
            // unioned back in under the clusters.
            let w_b = smoothstep(0.20, 0.30, tc_c);
            let sheet_w = smoothstep(0.60, 0.90, coverage);
            let stride2 = f32(stride * stride);
            for (var k = k_lo; k <= k_hi; k = k + 1) {
                var f_b = 0.0;
                var G_b = 0.0;
                for (var s = 0; s < CLOUD_FR_ZSUB; s = s + 1) {
                    let idx = k * CLOUD_FR_ZSUB + s;
                    let f_bs = select(1.0 - acc_u[idx], 1.0 - exp(-stride2 * acc_a[idx]), stride > 1);
                    let G_bs = f_bs * acc_ad[idx] / max(acc_a[idx], 1.0e-4);
                    f_b = max(f_b, f_bs);
                    G_b = G_b + G_bs;
                }
                G_b = G_b / f32(CLOUD_FR_ZSUB);
                let f_n = f_k[k];
                let G_n = G_k[k];
                let f_u = 1.0 - (1.0 - f_b) * (1.0 - f_n);
                let G_u = G_b + G_n * (1.0 - f_b);
                f_k[k] = mix(f_n, mix(f_b, f_u, sheet_w), w_b);
                G_k[k] = mix(G_n, mix(G_b, G_u, sheet_w), w_b);
            }
        }
    }

    // ── 5. WRITE ──
    if (is_global) {
        // Pooled bins, three slab bins each: the union over heights is the
        // max (one cloud spanning three bins occupies one area, exactly as
        // the heights inside a bin are unioned), the density the mean.
        var fp: array<f32, 4>;
        var Gp: array<f32, 4>;
        for (var q = 0; q < CLOUD_FR_GLOBAL_NZ; q = q + 1) {
            fp[q] = max(f_k[3 * q], max(f_k[3 * q + 1], f_k[3 * q + 2]));
            Gp[q] = (G_k[3 * q] + G_k[3 * q + 1] + G_k[3 * q + 2]) / 3.0;
        }
        if (!is_column) {
            let q0 = 2 * p_slice;
            return vec4<f32>(fp[q0], Gp[q0], fp[q0 + 1], Gp[q0 + 1]);
        }
        // (Cp_0, Cp_1, Cp_2, Tp) in pooled-bin units, sqrt-encoded.
        let Tp = Gp[0] + Gp[1] + Gp[2] + Gp[3];
        let Cp_0 = Gp[1] + Gp[2] + Gp[3];
        let Cp_1 = Gp[2] + Gp[3];
        let Cp_2 = Gp[3];
        return cloud_profile_col_enc(vec4<f32>(Cp_0, Cp_1, Cp_2, Tp));
    }
    if (!is_column) {
        let k0 = 2 * p_slice;
        return vec4<f32>(f_k[k0], G_k[k0], f_k[k0 + 1], G_k[k0 + 1]);
    }
    // The columns C_k = sum of G over the bins ABOVE k (exclusive; C_11 is
    // identically zero, and its channel carries the whole column T instead),
    // in slab-bin units, sqrt-encoded.
    var C_k: array<f32, CLOUD_FR_NZ>;
    var run = 0.0;
    for (var k = CLOUD_FR_NZ - 1; k >= 0; k = k - 1) {
        C_k[k] = run;
        run = run + G_k[k];
    }
    let T = run;
    let q = p_slice - CLOUD_FR_PAIRS;
    if (q < CLOUD_FR_CSLICES - 1) {
        return cloud_profile_col_enc(vec4<f32>(C_k[4 * q], C_k[4 * q + 1], C_k[4 * q + 2], C_k[4 * q + 3]));
    }
    return cloud_profile_col_enc(vec4<f32>(C_k[8], C_k[9], C_k[10], T));
}

// ── THE GLOBAL MIP PASS (increment 4; real, not a stub) ────────────────────
//
// Attachment = the profile atlas at mip m, binding 14 = the atlas's mip m-1
// view (so textureLoad level 0 IS the previous mip), scissored to the global
// region at mip m: origin (0, 2560 >> m), size (6144 >> m, 1024 >> m). Six
// passes, run once per completed global pass. The mip index is not a builtin:
// the six destination row ranges are disjoint ([1280,1792), [640,896),
// [320,448), [160,224), [80,112), [40,56)), so the row alone names m.
// Channels of the column slice (x >= 4096 >> m) are decoded before the
// average and re-encoded after (averaging square roots would under-read
// coarse columns).
@fragment
fn fs_cloud_profile_mip(in: CloudScreenVsOut) -> @location(0) vec4<f32> {
    let px = vec2<u32>(in.pos.xy);
    let x = i32(px.x);
    let y = i32(px.y);
    var m = 0;
    for (var mm = 1; mm < CLOUD_FR_GLOBAL_MIPS; mm = mm + 1) {
        let y0 = CLOUD_FR_GLOBAL_Y0 >> u32(mm);
        let hm = CLOUD_FR_GLOBAL_H >> u32(mm);
        if (y >= y0 && y < y0 + hm) {
            m = mm;
        }
    }
    if (m == 0) {
        return vec4<f32>(0.0);          // outside every global region (the scissor never lands here)
    }
    // Source rows: the destination's local row doubled, at the SOURCE
    // region's origin (2560 >> (m - 1)); the destination's origin is
    // 2560 >> m. Because 2560 = 5 * 2^9 both origins are exact at every mip,
    // so this equals 2y + dy; written out so the two offsets stay visible.
    let y_dst0 = CLOUD_FR_GLOBAL_Y0 >> u32(m);
    let y_src0 = CLOUD_FR_GLOBAL_Y0 >> u32(m - 1);
    let ys = 2 * (y - y_dst0) + y_src0;
    let xs = 2 * x;
    let t00 = textureLoad(tree_atlas_tex, vec2<i32>(xs, ys), 0);
    let t10 = textureLoad(tree_atlas_tex, vec2<i32>(xs + 1, ys), 0);
    let t01 = textureLoad(tree_atlas_tex, vec2<i32>(xs, ys + 1), 0);
    let t11 = textureLoad(tree_atlas_tex, vec2<i32>(xs + 1, ys + 1), 0);
    let col_x0 = (2 * CLOUD_FR_GLOBAL_W) >> u32(m);   // the column slice at this mip
    if (x >= col_x0) {
        let c = (cloud_profile_col_dec(t00) + cloud_profile_col_dec(t10)
            + cloud_profile_col_dec(t01) + cloud_profile_col_dec(t11)) * 0.25;
        return cloud_profile_col_enc(c);
    }
    return (t00 + t10 + t01 + t11) * 0.25;
}

// ── THE CALIBRATION (A1, contract "Calibration") ───────────────────────────
//
// The built part of the profile bake replaces every placed cloud by an
// ELLIPSE per height instead of point-testing its SDF, which needs, per
// archetype and cloud-relative height, the equivalent-circle radius of the
// cloud's cross-section as a fraction of its width (rho) and the mean
// in-cloud density across that cross-section (Dbar). Both are measured HERE,
// once, on the shipped SDF and the shipped density law (cv2_cloud_sdf with
// every lobe built, cv2_density_tail with every zero-mean field at its mean),
// over eight canonical clouds per archetype whose size draws are stratified
// over the power-law (so the eight ARE the size distribution), and averaged.
// Re-run by Rust whenever the tier, the interior saturation, the wide-edge
// bit or the component bisect changes.
//
// Stage 1, fs_cloud_profile_calib: attachment = the atlas's MIP 2, scissor
// (CLOUD_FR_CALIB_STAGE_X0, _Y0, 32, 32) in mip-2 texels. Texel (x, y): row =
// x - 768 (the height row, 0..31 over 0 to 1.5 cloud heights), arch_i =
// (y - 512) / 8, seed_s = (y - 512) mod 8. 64 x 64 points over the cloud's
// bounding square [-br, br]^2 at that height, in the cloud's UNIT shape frame
// (sx = sy = sz = 1: the frame's stretch is applied by the bake per placed
// cloud). Writes (rho, Dbar, 0, 1). rho = sqrt(A / pi) / width stays at most
// 2 br / (sqrt(pi) width) = 0.90 for the smallest humilis, so it fits the
// RGBA8Unorm channel; the bake's semi-axes are width * stretch * rho.
//
// Stage 2, fs_cloud_profile_calib_reduce: attachment = MIP 1, binding 14 =
// the mip-2 view, scissor (CLOUD_FR_CALIB_X0, _Y0, 32, 4) in mip-1 texels.
// Texel (x, y): row = x - 1536, arch_i = y - 1024; the plain mean of the
// eight staging texels. Rust raises pad flag bit 8 after this frame; the
// bake reads the table through its mip-1 view at (1536 + row, 1024 + arch).
//
// Both areas live in the window band's mip levels, which nothing else
// writes (the global's mip-1 region starts at row 1280 and its mip-2 region
// at row 640, both below these areas). Cost: 1024 fragments x 4096 SDF
// evaluations, once.
@fragment
fn fs_cloud_profile_calib(in: CloudScreenVsOut) -> @location(0) vec4<f32> {
    cloud_set_slab_bounds();
    let px = vec2<u32>(in.pos.xy);
    let x = i32(px.x);
    let y = i32(px.y);
    let row = x - CLOUD_FR_CALIB_STAGE_X0;
    let ys = y - CLOUD_FR_CALIB_STAGE_Y0;
    if (row < 0 || row >= CLOUD_FR_CALIB_ROWS || ys < 0 || ys >= 4 * CLOUD_FR_CALIB_SEEDS) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);   // outside the staging area (the scissor never lands here)
    }
    let arch_i = ys / CLOUD_FR_CALIB_SEEDS;
    let seed_s = ys % CLOUD_FR_CALIB_SEEDS;
    // The canonical cloud: the size draw stratified over the power law, no
    // coverage growth, the seed from the same hash family the field uses.
    let u_s = (f32(seed_s) + 0.5) / f32(CLOUD_FR_CALIB_SEEDS);
    let arch = cv2_arch(arch_i, u_s);
    let seed = cv2_hash(vec2<f32>(f32(seed_s), f32(arch_i)), 7.0) * 4096.0;
    let width = arch.width_m;
    let height_m = width * arch.aspect;
    let br = width * 0.5 + CLOUD_V2_RIND_M;
    let a_box = 4.0 * br * br;
    // This row's height: rows span 0 to CLOUD_FR_CALIB_YMAX cloud heights (the
    // body rejects samples above height * sy + br, and 1.5 covers that for
    // every archetype).
    let y_local = (f32(row) + 0.5) / f32(CLOUD_FR_CALIB_ROWS) * CLOUD_FR_CALIB_YMAX * height_m;
    // The EYE's density law: every lobe built (g_sun_profile = 0), the domain
    // warp fetched at the world shape LOD (shape is viewer-independent), the
    // interior saturation as the march reads it.
    g_sun_profile = 0.0;
    g_v2_disp_lod = CLOUD_V2_SHAPE_LOD_WORLD;
    let sat = clamp(camera.light5_color.w, 0.0, 1.0);
    var n_in = 0;
    var dsum = 0.0;
    let n_grid = CLOUD_FR_CALIB_GRID;
    for (var gy = 0; gy < n_grid; gy = gy + 1) {
        let pz = -br + (f32(gy) + 0.5) / f32(n_grid) * 2.0 * br;
        for (var gx = 0; gx < n_grid; gx = gx + 1) {
            let pxm = -br + (f32(gx) + 0.5) / f32(n_grid) * 2.0 * br;
            let local_m = vec3<f32>(pxm, y_local, pz);
            let d = cv2_cloud_sdf(local_m, seed, arch);
            // Displacement zero-mean (0), Worley mean 0.5, turbulence mean
            // 0.5, the eye's rind, the march's saturation.
            let dens = cv2_density_tail(d, y_local, height_m, 0.0, 0.5, 0.5, CLOUD_V2_RIND_M, sat);
            if (dens > CLOUD_STEP_INTERIOR_GATE) {
                n_in = n_in + 1;
                dsum = dsum + dens;
            }
        }
    }
    let area = f32(n_in) / f32(n_grid * n_grid) * a_box;
    let rho = sqrt(area / PI) / max(width, 1.0);
    let dbar = dsum / max(f32(n_in), 1.0);
    return vec4<f32>(clamp(rho, 0.0, 1.0), clamp(dbar, 0.0, 1.0), 0.0, 1.0);
}

@fragment
fn fs_cloud_profile_calib_reduce(in: CloudScreenVsOut) -> @location(0) vec4<f32> {
    let px = vec2<u32>(in.pos.xy);
    let x = i32(px.x);
    let y = i32(px.y);
    let row = x - CLOUD_FR_CALIB_X0;
    let arch_i = y - CLOUD_FR_CALIB_Y0;
    if (row < 0 || row >= CLOUD_FR_CALIB_ROWS || arch_i < 0 || arch_i >= 4) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);   // outside the table (the scissor never lands here)
    }
    // The eight staging texels of this (row, archetype), through the bound
    // mip-2 view (level 0 of the view IS mip 2): plain means.
    var acc = vec2<f32>(0.0);
    for (var s = 0; s < CLOUD_FR_CALIB_SEEDS; s = s + 1) {
        let tex = textureLoad(tree_atlas_tex,
            vec2<i32>(CLOUD_FR_CALIB_STAGE_X0 + row, CLOUD_FR_CALIB_STAGE_Y0 + arch_i * CLOUD_FR_CALIB_SEEDS + s), 0);
        acc = acc + tex.rg;
    }
    acc = acc / f32(CLOUD_FR_CALIB_SEEDS);
    return vec4<f32>(acc.x, acc.y, 0.0, 1.0);
}
