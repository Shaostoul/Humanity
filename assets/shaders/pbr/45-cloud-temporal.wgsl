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
    } else if (diag >= 8.5) {
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
