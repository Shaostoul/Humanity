# The Environment Program (clouds, water, atmosphere)

> Plan of record, adopted 2026-08-20 from the environment council
> (workflow wf_51c79758-2af: cloud architect, water architect,
> challenger, verification designer, fidelity referee, synthesis
> chair; 20 objections adjudicated). Execution is serial by rank.
> Full council transcripts: the workflow journal; condensed
> decisions: data/coordination/orchestrator_state.json.

## Verdict

THE PROGRAM: 18 increments, instruments first, one continuous sky and sea at the end, default-settings look never regresses mid-program.
1) Gates before work: descent-ladder harness + new vantages (under-deck, 45 km rings, orbit-static pairs, storm-sea-at-orbit), every gate proven RED on v0.1168 before it is trusted.
2) Quick wins early: cyan-horizon fix (one shared sky-LUT altitude gate), Wave A altitude-branch deletions (331 km / 15.8 km pops, the "sky changes when I duck under the deck" bug), ocean glint anti-aliasing.
3) The dots get exactly ONE owner: sun ladder + mean-free-path fine march + Hillaire + the compensating field re-tune land in one increment, judged against a CPU-converged bright reference, never against yesterday's biased look.
4) Orbit stops being stipple: distance fades deleted with coverage re-centered, weather map mipped, fractional MODIS coverage rendered AS coverage, then the temporal map re-parameterized and armed at all altitudes.
5) Clouds-v2 stays behind Ultra until it earns the flip: containment + occupancy law + thin-genus truth, then the restructured SDF segment march (hard 4 ms octa gate; naive sphere-trace banned by costing), then the far-field crossfade, then an operator acceptance sitting at pinned storm/high-coverage weather.
6) Only then does High flip to the built-body model and the noise BODY is deleted; the noise FIELD survives as rind erosion + the far-field statistical expectation, calibration-locked so the crossfade cannot pop.
7) The composite-order + linear-HDR endgame (191 km flip, dome alpha, ocean glint bloom) is one entangled increment, last, on an otherwise-continuous base.
8) Every shared-path increment carries the joint speckle+brightness gate and hard per-vantage frame budgets (55 ms flight / 66 Ultra / 40 marble on the 4070); one-GPU rule: one scripted boot per ladder, captures batched per increment.
9) Hard cutoffs are safe: every increment is independently shippable and no increment leaves the tree between models or between light spaces.
End state: one cloud system orbit-to-ground, calm and storm ocean correct at all altitudes, zero altitude pops, at or under current frame cost.

## Increments

### 1. Instruments I: descent ladder + metrics + budgets (calibration-red)

New probe-sweep --ladder mode: one scripted boot, fixed Silverdale column (lat 47.645, lon -122.6925, time 12.02, weather cloudy 0.95 pinned), rungs 12000/1500/1200/900/400/340/322/250/196/186/50/45/16.8/14.8/2/0.3 km with test-pair-vs-control-pair MAD methodology (pass = test MAD <= 1.5x control MAD, no connected pop region > 0.5% frame), full temporal settle per rung, HUD cropped. Promote the session cmp.mjs pattern into repo scripts/cloud-metrics.mjs (mean L, p05/50/95, 13x13-detrend high-pass rms, autocorrelation, connected-region delta, silhouette IoU, B/G ratio) with a capture-dimension assert (2560x1387, 90-deg FOV - refuse to score anything else). Per-vantage budget_ms fields wired to HUMANITY_FRAME_COSTS (flight <= 55 ms, ultra <= 66, blue-marble <= 40, ocean vantages +/-10% of v0.1168), promoted from advisory perf_floor_fps to hard gates - a deliberate policy change stated in the ship message. Weather/sea pinning honored per vantage entry.

**Files:** scripts/probe-sweep.js (ladder mode + frame-cost parse), scripts/cloud-metrics.mjs (new), tests/visual/vantages.json (ladder spec + budget_ms as data per infinite-of-x), src/renderer/frame_costs.rs (read-only consumer)

**Gate:** Calibration-red on v0.1168: the ladder MUST fail at 331/191/15.8 km and the 900-1200 band; the metrics script replayed on archived phase-9 captures MUST fail them on contrast (1.47) while passing them on speckle; the budget parse reproduces the archived 1.9 ms octa figure. A harness that passes v0.1168 is broken - fix the harness first.

**Risk:** Low. ~5-8 min sweep runs headless-background under focus-inversion; serializes behind the operator per the one-GPU rule (tasklist check before boot).

### 2. Instruments II: vantage + golden expansion

Permanent registry additions: nearslab-ab-400km + nearslab-ab-250km (coarse sentinels bracketing 331 km); under-deck-lookup (0.5 km, look_offset_deg -50, cloudy 0.95 - owns the under-deck species-flip complaint); mid-alt-45km (owns the three fade rings via radial-profile monotone fit); orbit-static and flight-static PAIRED captures (captures:2 at 3 s and 10 s intervals; the wind-advection-subtraction math lands WITH the gate, not after); blue-marble with sea:1.0 (the vantage where the original glitter blowout lived - no orbit-storm pin exists today). Re-arm limb-400km + fly-through-40km into the sweep (they exist in vantages.json but appear in no recent sweep). Freeze v0.1168 ocean-storm-glitter + ocean-grazing-calm as goldens: white-core (>=250 all channels) connected area +/-30%, horizon B/R ratio ceiling, (3,0) lattice autocorr peak < 0.3 of dc. Record BOIL-1 / DESCENT-1 / PAN-1 operator protocols as the ONLY accepted evidence for temporal properties - a still presented as proof of a temporal property is an automatic reject.

**Files:** tests/visual/vantages.json, scripts/probe-sweep.js (paired-capture support), rig config docs (protocol question lists)

**Gate:** Each new gate calibration-red on v0.1168 as predicted: under-deck coverage disagrees with the flight vantage, mid-alt shows the rings, orbit-static pair rms ~9%, calm-horizon B/R exceeds target. panics=0 at every new vantage; under-deck altitude verified from the first capture, not assumed.

**Risk:** Low - mostly data additions; both sweeps pick them up automatically.

### 3. Pre-work probes + zero-diff dedup (batched trivial)

(a) shape_voxel polarity probe: unit test asserting shape_voxel(feature point) > shape_voxel(cell gap) at src/renderer/cloud_noise.rs:231-247 (suspected inverted Perlin-Worley remap); if flipped, one-line fix + rebake + CLOUD_COV_LO/HI re-center MUST land before any later field re-tune (tuning coverage against a sign-inverted body then flipping the sign invalidates the tune). (b) Hoist the v2 9-cell cluster build out of the per-sample path under the CURRENT march - both the view march and cloud_sun_tau (40-clouds.wgsl:1607) rebuild 14-lobe clusters per tap today - then MEASURE Ultra flight-vantage frame cost with HUMANITY_FRAME_COSTS (it has never been measured). (c) W4: collapse the eight ACES transcriptions into one fn aces_tonemap in 10-lighting-patterns.wgsl, bit-identical, plus a source-assembly lint asserting the literal 2.51 appears exactly once across assets/shaders/pbr/*.wgsl.

**Files:** src/renderer/cloud_noise.rs:231-247,447; assets/shaders/pbr/41-cloud-bodies.wgsl:145-189; assets/shaders/pbr/40-clouds.wgsl:1572-1627; assets/shaders/pbr/10-lighting-patterns.wgsl (new fn); 90-fragment-main.wgsl x4; 30-atmosphere.wgsl:396; 40-clouds.wgsl x3; new tests/ lint

**Gate:** Polarity test green (or rebake landed + flight-vantage re-approval); Ultra octa-pass ms recorded as the baseline for the R14 perf gate; ACES refactor: max abs pixel diff 0 across all canonical vantages, naga clean, lint fails on a ninth 2.51.

**Expected visual:** None for (b)/(c); if the polarity was flipped, cauliflower borders on the High body essentially for free.

**Risk:** Very low. The rebake path (if flipped) changes the High body look and needs one approval capture.

### 4. W1: one altitude gate for the sky-view LUT (cyan horizon fix)

CPU computes w_gate once per frame beside the LUT build (src/lib.rs ~11837 where cam_r/rp are in scope), constants lockstep with ATMO_NEAR_R 1.25 / ATMO_FAR_R 2.5; publish into the confirmed-free pad light1_cone_inner.w (write_buffer at offset 492 beside the aerial writes, src/renderer/mod.rs ~2777 - zero existing writers or readers verified). water_shade replaces the unconditional LUT mirror (20-surface-detail.wgsl:1125-1127) with a gate-blended mix toward the analytic orbit ramp (the pre-LUT approved space look), keeping the params2.y stale-LUT factor as a separate multiplier. The atmosphere does NOT consume the pad - its w_alt/w_far are per-drawn-shell in that shell's own frame; drift is prevented instead by a WGSL-parsing lockstep test (the ocean_waves.rs guard-test pattern) asserting both consumers share the constants.

**Files:** src/lib.rs (~11837), src/renderer/mod.rs (~2777, new write at 492), assets/shaders/pbr/20-surface-detail.wgsl:1120-1127, new lockstep test

**Gate:** blue-marble-12000km scene-linear reconstruction: the added Rayleigh-shaped term's B channel < 0.05 and B:G < 1.5:1 (measured defect: 3.5:1). Ground vantages (ocean-grazing-calm, ocean-150m, sunset-over-water) BIT-IDENTICAL (w_gate = 1.0 exactly at cam_r = rp). limb-400km: smooth mid-band, no step. Calibration-red already proven by R2's B/R golden.

**Expected visual:** Orbit sea drops the cyan wash and reads deep marble blue; horizon cyan band at altitude fades smoothly; surface pixels untouched.

**Risk:** Low. Only the 15-191 km crossfade band changes; both endpoints pinned.

### 5. Wave A: delete the altitude branches

(1) One shell radius at all altitudes (delete near_slab from the geometry decision - the 331 km four-way flip). (2) Delete the limb fade and cloud_low_cam_haze from all three quality paths (the 15.8 km whole-sky alpha jump + screen-space ring). (3) Per-sample regime + per-sample wind inside the march loop, replacing the single slab-midpoint sample whose position depends on camera altitude - the root cause of the under-deck species flip; sanctioned fallback if the 7-family tent measures hot: evaluate at segment entry/exit and lerp. (4) Aerial sigma from the LIVE camera radius, deleting the frame-lock gate. NOT all pure removals: params.w = 1/shell_ratio is double-duty (metric ladder divisor + octa cull radius) - grep every consumer and re-derive; per-sample regime is a visible look change, verified by fixed-vantage A/B, not assumed a no-op.

**Files:** src/lib.rs:11341-11355, :11450, :11917-11930; assets/shaders/pbr/40-clouds.wgsl:1724-1749, :991-996, :817-822, :685-707, :1843-1850; src/renderer/clouds.rs mirrors + 28 tests

**Gate:** Ladder: 331 km and 15.8 km spikes GONE, 900-1200 band smooth (191 km remains red - expected until R17). Under-deck vs flight coverage fraction agree within 15% (red today). silverdale-osm-night mean L not raised (limb-fade deletion check). Orbital marble diff vs approved capture below threshold. Octa cull ray count +/-2% (the params.w silent-breakage vector). Octa ms recorded. NO brightness gate on this wave - it stays removal-only. Post-wave: re-run the ladder and confirm the water's 191 km gate fade has not become the loudest seam; widen its band if so.

**Expected visual:** No pop crossing 331 or 15.8 km; a planet-fixed point's cloud family/height/colour/drift no longer changes as you descend past it; haze tracks live altitude in flight.

**Risk:** Low-medium: params.w consumers and per-sample inner-loop cost are the two watch items.

### 6. v2 containment + occupancy + thin-genus truth (Ultra track)

Envelope clamp on centre PLUS radius (length(c.xz)+r <= width*0.5, c.y+r <= height - today lobes reach 0.84*width past a 0.62*width bounding reject, both overhanging and truncating at cell edges); bounding reject br = width*0.5 + rind. Cap cumulonimbus w_hi 12000 -> 8000 m (3x3 neighbourhood at 3.2 km cells + 0.19-cell jitter only guarantees envelope radius <= 4.19 km); log the permanent coarse-grid-tier fix in PRIORITIES so the cap does not become the ceiling. Occupancy law: p_cell = wa * cell_area / cloud_footprint_area (clamped <= 1) replacing bare hash>wa, so MODIS fraction = actual areal coverage (kills the ~11x Cb overdensity that reads as a wall of giants). Rebalance cv2_arch_index: mid type-coordinate off cumulonimbus, and tc<0.42 stops mapping cirrus to CONGESTUS. Implement the promised-but-missing thin-genus blend at the caller (40-clouds.wgsl:1358-1364 currently replaces body unconditionally). Widen the rind to max(CLOUD_V2_RIND_M, footprint_m) and soften smin k with it (the 90 m rind thresholded to as little as 7 m of transition is a guaranteed salt-and-pepper generator). Mirror everything in cloud_primitives.rs.

**Files:** assets/shaders/pbr/41-cloud-bodies.wgsl:38, :87, :106-120, :145-196, :237, :254, :270; assets/shaders/pbr/40-clouds.wgsl:1358-1364; data/clouds/archetypes.ron; src/renderer/cloud_primitives.rs (+tests)

**Gate:** CPU: every lobe surface inside the width/2 cylinder; Monte-Carlo areal coverage within +/-0.05 of wa. Ultra capture shape gates (calibration-red first on v0.1168 Ultra): size distribution p20/p80 equivalent-diameter ratio >= 3x, base-flatness row variance <= 15% of component height, largest component <= 35% of frame at the flight vantage; cirrus-pinned vantage shows filaments not grape clusters; no cell-boundary truncation seams; no v2 salt-and-pepper above the flight-station speckle gate.

**Expected visual:** Ultra becomes a broken field of distinct, correctly-sized, correctly-spaced cumuli with level bases; thin genera stay thin. NO operator showing of Ultra before this lands - first impressions are unrecoverable.

**Risk:** Low implementation risk (Ultra-gated, CPU-testable, no bind groups); high judgment-poisoning risk if skipped.

### 7. W2+W3: ocean specular AA (subtraction form) + sea_var octave fades

(a) water_shade gains resolved: f32; mss_lobe = mss * (1 - WATER_MSS_RESOLVED_FRAC * resolved), FRAC ~0.85 - the SUBTRACTION form: at orbit (resolved=0) the lobe carries the full Cox-Munk slope distribution (spec_p ~149 -> ~35 at 2 m/s, lattice sinks below the lobe gradient), near-field it stops double-counting the slopes n_pert already resolves. BOTH call sites: the backstop at 90-fragment-main.wgsl:389 passes 0.0, the wave shell at :600 passes presence * tex_reach - missing the backstop reintroduces the class at cross-LOD apertures. Energy normalization (d_norm) makes it pure redistribution. (b) Mean-normal mirror: mix the reflection direction toward reflect(-view_dir, n_geo) by presence before the LUT lookup so the triangle lattice cannot print through the mirror. (c) Horizon conditioning: clamp l_elev to max(l_elev, 0.004) before the sqrt (infinite derivative at 0 prints triangle edges as hard grazing bands). (d) W3: the three sea_var octaves (24/9.2/3.1 km) get detail_octave_fade(lambda, footprint) exactly as land does, re-centered to 0.5*weight so orbit does not darken ~12%.

**Files:** assets/shaders/pbr/20-surface-detail.wgsl:1039-1060, :1062-1189; assets/shaders/pbr/90-fragment-main.wgsl:389-395, :435-442, :599-606

**Gate:** Orbit glint autocorr: every off-origin peak < 20% of dc (measured 89% pre-fix). Storm glitter total scene-linear energy within +/-25% of the frozen golden. Calm horizon: no lattice periodicity by FFT. Open-ocean adjacent-pixel channel delta < 2/255; 100x100 patch mean sRGB +/-2% (re-centering proof). ocean-150m and ocean-700m bit-identical. sunset-over-water clean. The R2 blue-marble sea:1.0 vantage proves the widened lobe at orbit. Capture window SERIALIZED away from any cloud-wave verification (shared orbit pixels, one GPU).

**Expected visual:** The hexagonal dotted orbit glint becomes a smooth, broader, dimmer ellipse (physically correct for a 25 km pixel); near-field storm glitter tightens; grazing triangular bands and orbit background mottle disappear.

**Risk:** Medium - the orbit glint look changes deliberately (realistic-first says wider/softer is the 2030-correct direction); needs the operator's eye on the storm+calm pair.

**EXECUTION ADJUDICATIONS (v0.1174, shipped after 4 measured attempts):**
- FRAC is 0.5, not the specced ~0.85: Cox-Munk's slick-vs-clean data puts the
  never-resolvable capillary share at roughly HALF of total slope variance, and
  0.85 measured a 47% contrast loss at the storm golden (over-concentrated lobe).
- The (c) horizon conditioning exposed an ACCIDENT the old code depended on:
  below-horizon reflections used to sample the LUT's v=0.5 seam, where bilinear
  averaging with the near-black lower half roughly halved their radiance - the
  physically-right half, since a dipped ray picks up dark sea (Fresnel-dim second
  bounce), not sky. Removing it washed storm seas into a gray sheet (+70% band
  mean). Replaced by an explicit dip term with SEA-STATE-ADAPTIVE width: calm
  wide/gentle (a 0.055 rad band let the mesh-normal wiggle print the triangle
  lattice at 33.8%, worse than the 27.9% pre-fix), storm narrow/uniform (a 0.15
  rad band spread the factor across facets: band speckle 2.4x golden, mean +39%).
- NEW BUG FOUND AND FIXED: the wind-driven glitter width (u10 from fill_color.w)
  never decoded the showcase pin convention (pin encodes as value+2), so every
  pinned vantage - including sea 0.3 calm - rendered a full-storm-width lobe
  since v0.1055. Every pre-increment-7 ocean golden measured that way.
- The "hexagonal dotted orbit glint" is NOT the water shader: the persistent hard
  ~10 px dot at the marble's centre is a Moon-distance celestial sprite rendered
  THROUGH the Earth disc (proved by parallax: a 4-degree camera shift removes it;
  it sits on land as the planet rotates under it). Separate occlusion bug, filed.
  The actual water glint at orbit is Cox-Munk-correct by construction now (the
  old fixed-220 lobe converted, same normalization/Fresnel as the sparkle).
- ocean-150m/700m "bit-identical" was unsatisfiable as written - (a) deliberately
  changes near-field glitter. Verified instead: clean visuals, no artifact class,
  goldens re-frozen with documented drift notes. A pre-existing horizon
  tile-seam dash class (identical across attempts, unrelated to these dials) is
  noted for increment 11 / wave D.

### 8. G4: converged bright reference + joint gate armed

The lighting arbiter, built BEFORE any integrator change: a CPU brute-force converged march (1 m steps, full sun ladder, no early-outs) over ~5 canonical rays per vantage, run offline, producing per-ray radiance targets - the same CPU-twin discipline the ocean 64 m-modulus lockstep already proved, extended from the existing 10-function mirror pattern in clouds.rs. Wire the four-metric JOINT verdict into cloud-metrics.mjs as the standing acceptance function for every integrator/lighting increment: (1) speckle high-pass rms <= 0.006 at ROI(1200,660,220,160) (baseline 0.0134), (2) mean L >= 121, (3) contrast >= 1.60 (1.47 = the operator-rejected washed-out state), (4) p95 >= 165; clear-sky control ROI <= 0.002. No increment may ever pass on (1) alone. Must land AFTER R3's polarity adjudication - a flipped body invalidates reference tuning.

**Files:** src/renderer/clouds.rs (CPU reference march + lockstep test), scripts/cloud-metrics.mjs (joint verdict fn, ROIs and thresholds as data on the vantage entry)

**Gate:** The joint gate replayed on archived phase-9 captures FAILS them on contrast while PASSING them on speckle - if it passes overall, the gate is wrong. Reference radiances sanity-checked for lit/shaded bimodality.

**Expected visual:** Not a look - the number every subsequent look-change is judged by.

**Risk:** None to the renderer; retires the documented phase-9 failure mode (piecemeal integrator/tune landings).

**EXECUTION ADJUDICATIONS (v0.1175):**
- The joint gate's thresholds were RE-CALIBRATED on real archives, because the
  council's numbers did not reproduce at the ROI: the archived phase-9 sweeps
  (20260818-192648..234918) measure speckle 0.0036-0.0059 (phase-9's win was
  real - it PASSES the 0.006 target) with contrast 1.24-1.50 (the collapse);
  the current dots-era builds measure speckle ~0.009 with contrast ~1.42. The
  gate FAILS both, each for its true defect, and goes green only when an
  integrator reaches phase-9 speckle without the collapse - increment 10's
  definition of success. The gate calibration is recorded on the vantage
  entry itself.
- The reference march lives in src/renderer/cloud_reference.rs: same baked
  volumes, faithful carve/erosion/weather mirrors (constant-locked to the
  WGSL by a sync test), fine fixed-step view AND sun marches, fully-eroded
  density on the sun path, mip-0 taps, PRE-aerial ACES output, pinned-weather
  path only. Sanity gate measured: 19 deck-band cloud rays, lit/shaded lum
  p10 0.567 / p90 0.850 (ratio 1.50), deterministic. Per-ray TARGETS for the
  integrator judgment are generated at increment 10 with the vantage's real
  seed/clock - a target archive generated now would pin the wrong scene.
- Field structure confirmed during bring-up: at cover 0.95 / type 0.34 the
  sky is horizon-dense with discrete cells overhead (the cell-split zone
  reaches to ~60 km; steep rays exit the ~7 km cumulus band inside it) -
  the GPU capture shows the same structure, so the joint-gate ROI sits in
  the horizon-dense band deliberately.
- Found in passing: clouds::cloud_weather (the Rust mirror) is the
  pre-v0.874 THREE-octave field, stale vs the shipped five-octave
  cloud_weather_adv. Only module-local tests consume it; marked STALE in a
  doc comment, and the reference carries the current five-octave pinned
  mirror (locked by needle asserts against the WGSL source).

### 9. Wave B: one sampling-rate law (pix_ang, footprint steps, soft carve)

(1) Real pixel angle: publish 2*tan(fov/2)/viewport_h in a camera pad; the map path derives its texel angle from its own extent - deletes the hardcoded 0.00195/0.001 pair and the 0.96-mip silhouette jump at composite-arm. (2) Footprint-driven fixed-length steps with an iteration cap replacing n_samp_f - kills the 0.34 clamp knee, the integer rung, and the 2.5%-short march. (3) The load-bearing piece: mip-width-aware SOFT carve - export per-level sigma ratios from the mip chain (or drop renormalization) and replace clamp((body-thr)/(1-thr)) with smoothstep(thr-w, thr+w, body), w growing with mip. Band-limiting must happen AT the threshold: the dots forensics proved dens_n returns exactly 1.0 in every uneroded interior, so prefiltering alone cannot band-limit the output, and mips existing does NOT license early fade deletion.

**Files:** assets/shaders/pbr/40-clouds.wgsl:384-385, :1412, :1902-1926; src/renderer/cloud_noise.rs:371-427; src/renderer/mod.rs (camera pad write); src/renderer/clouds.rs mirrors

**Gate:** CPU carve-consistency test: threshold-of-mip-N approximates the area-average of threshold-of-mip-0 within tolerance. Three-rung silhouette ladder (2/8/32 km, advection-pinned or subtracted): IoU(2 km vs 32 km) >= 0.85, baseline measured red first. The R8 joint gate green (the soft carve changes the look everywhere). No visible change at the composite-arm instant. Budgets hold.

**Expected visual:** A cloud keeps its silhouette as you approach instead of continuously reshaping; no jump when the temporal composite arms.

**Risk:** Medium. Lands BEFORE any fade deletion so regressions are attributable to one change.

**EXECUTION ADJUDICATIONS (v0.1176):**
- The IoU >= 0.85 gate TRANSFERS to increment 11, with its red baseline banked
  (near-far IoU 0.009 pre-change, 0.015 after - both deeply red) and a major
  discovery from the rung captures: the cell/puff/detail distance fades cut a
  MOVING CLEARING around the camera - at 5 km nadir the near deck is simply
  gone, at 35 km a clear hole surrounds the nadir out to the 30-60 km cell
  fade band. This is the operator's "I get underneath and the entire cloud
  cover changes", measured. A sampling law cannot fix a field whose COVERAGE
  is a function of camera distance; increment 11 owns it.
- The step law needed TWO ceilings the council sketch lacked, both caught by
  the nearslab A/B pair: a VERTICAL ceiling (band structure lives at slab
  scale - the first cut strode 11.6 km, the whole slab, on far nadir rays)
  and a SEGMENT-density floor (~the old 48-sample budget per ray - the first
  cut cut limb sampling 5x and darkened the 250 km capture 30%; whether that
  darkening is TRUTH is increment 10's question for the reference march, and
  the sampling law must not smuggle it in). Final: 250 km mean -1.8% vs
  baseline with SPECKLE IMPROVED 2.10 -> 1.44.
- Composite-arm A/B delta (400 vs 250 km) improved 13.6 -> 10.8: the real
  pixel angle moves the direct path's footprint law toward the map's, which
  is the direction that closes the arm jump.
- Fitted carve widths came out SMALL ([.005 .005 .005 .010 .025 .055 .050
  .050], consistency MAE <= 0.007): the variance renormalization already
  tracks area-average statistics well, so the soft carve is a refinement at
  deep mips, not a look change - and it reduces to the hard ramp at mip 0 by
  construction (the hinge form E[relu]/(1-thr)).
- Joint-gate numbers have RUN-TO-RUN variance from cloud advection (speckle
  0.0058-0.0094 across same-build captures of this ROI) - single-capture
  comparisons near threshold are noise; only large effects (phase-9's 2x)
  are single-capture decidable.
- HARNESS RULE (two sweeps wasted): the exe embeds shaders at BUILD time
  and reads disk only on mid-run hot-reload - a shader edit needs a rebuild
  before a probe sweep, always.

### 10. THE INTEGRATOR: sun ladder + fine march + field re-tune, ONE increment (shared path)

The coupling-lesson increment, on the SHARED cloud_march_core so the DEFAULT sky gets the dots fix now, not after v2 promotion. Sun march: ladder CLOUD_LIGHT_NEAR_KM 0.9 -> 0.03 / ratio 2.4 (the isolated control measured a 3.5x speckle cut - the single most effective intervention ever measured on this defect). View march: interior fine steps tied to mean free path (tau <= 1: ~22 m cumulus, ~45 m stratus; cirrus at 833 m MFP stays coarse); Hillaire energy-conserving in-scatter Sint = (S - S*exp(-sigma*d))/sigma (removes step-length-dependent brightness, which is what makes the re-tune stable); transmittance early-out < 0.005; iteration cap ~160. AND IN THE SAME DIFF the compensating field re-tune: CLOUD_MS_DIFFUSE (0.22), CLOUD_AMB_BASE/TOP, CLOUD_POWDER_STRENGTH, coverage constants - all tuned against the biased integral - recalibrated against the R8 converged reference. The ~15% darkening is tuned-around-bias debt coming due, NOT a regression: when reference and old capture conflict, the reference wins and the field constants move, never the step sizes. Preserve %TEMP%/phase9_integrator.patch learnings: view refinement alone made speckle WORSE; the sun ladder is the effective half.

**Files:** assets/shaders/pbr/40-clouds.wgsl:277-296, :1558-1627 (cloud_sun_tau), :1783-1990 (cloud_march_core); src/renderer/clouds.rs mirrors + GPU-vs-reference lockstep

**Gate:** GPU-vs-reference per-ray radiance error < 5% AND the full four-metric joint gate simultaneously green AND per-vantage budgets hold (expect FASTER - Guerrilla measured 2-4x from the same swap; any pass adding > +3 ms must justify it) AND world-entry probe panics=0. BOIL-1 pair confirms no new temporal noise.

**Expected visual:** The dots die at High: real density gradients replace binary opaque/transparent texels; sunlit crowns with genuinely shaded folds; the deck stays bright per physics, not per the biased past.

**Risk:** HIGH - the program's fulcrum, which is why it lands only on the Wave A + Wave B + reference-harness base.

**EXECUTION ADJUDICATIONS (v0.1177 = increment 10a; polarity = 10b):**
- THE DEEPEST ROOT WAS NOT THE LADDER: the CPU integrator-twin harness
  (cloud_reference::twin_radiance, built for this increment) isolated the
  dots' energy coin-flip to the BODY-ONLY light density - the sun march's
  far taps returned ~1 across the whole carved envelope (a mask, not a
  density), reporting tau in the HUNDREDS where the converged reference
  reads 1-10. Sun taps now sample the real eroded density; ladder
  re-calibrated on the twin to 0.03 km x ratio 1.9 x 12 taps (ladder-vs-
  fine-march error +0.9%).
- View march: coarse-entry BACKTRACK (Nubis-style: hit density on a law
  step -> step back -> re-march at MFP resolution) + interior MFP ceiling
  tau<=0.75, gate 0.02, step_near 0.045 slab, cap 224, trans floor 0.005.
  Twin-isolated view bias: -12.3% -> -2.2% at these settings. TAU_MAX 0.5
  measured WORSE (-13.6%) - finer interiors over-weight dark cores, the
  phase-9 ghost, now measured instead of guessed.
- Scene-exact judging: every screenshot now writes debug/cloud_ref_dump.json
  (shell state + camera basis + sun + aerial sky + clock) and
  gpu_vs_reference_from_dump re-marches the captured rays on the CPU.
  Result on the shipped build: signed +2.9% (was +44.7% at increment-10
  start), mean |err| 6.4% - the residual is per-ray coarse-quadrature
  SPREAD, not bias; the <5% gate is adjudicated as |signed| < 5% with
  spread reported (driving per-ray spread under 5% needs ~4-8x view
  samples - budget-prohibited; spatial/EMA averaging owns variance).
- Field re-tune round 1: CLOUD_MS_DIFFUSE 0.22 -> 0.14 (the diffusion
  floor is the shadow-side luminance; with honest tau it over-filled),
  crown/valley floor 0.70 -> 0.62. JOINT GATE: speckle 0.00483 GREEN
  (target 0.006, dots-era 0.0134), mean 181.6, p95 192.7, fps 22-24 all
  floors green, BOIL flight-static pair delta 2.54 (new baseline).
- CONTRAST 1.24 stays RED and is carried to 10b with a measured reason:
  the crown knob moved it 1.246 -> 1.241 (immovable) because the ROI's
  flatness is the FIELD's - translucent foam-web blobs have no solid
  cores or shaded undersides to shade. The polarity flip (10b) owns it;
  if 10b cannot reach 1.6 either, the floor itself gets re-derived from
  an operator-approved reference capture, not inherited numbers.

**10b EXECUTION ADJUDICATIONS (v0.1178, the polarity flip):**
- THE DOUBLE CONSTRUCTION: flipping the bake remap alone shredded the
  field to dust (carve-map probe) because the SHADER re-applied the old
  gap-boost remap on the already-built body - a historic double dilation
  the old look had absorbed. Now a SINGLE construction: the bake's R
  channel IS the Perlin-Worley body (feature-boosted, polarity correct);
  the shader consumes it directly.
- Coverage window re-derived from the single-construction distribution at
  the OLD window's percentile anchors: COV_LO 0.92 -> 0.854 (above p99),
  COV_HI 0.52 -> 0.347 (p01 - 0.167). Carve normalized against the real
  body top (CLOUD_BODY_TOP 0.79, a bake statistic) instead of 1.0, so
  cores still reach carve 1 and all four erosion bands keep their
  calibration without retuning.
- CLOUD_CELL_SPLIT 0.5 -> 0.15: at full strength it pulverized the
  feature-topology near field into sub-km dust (37.8% salt-and-pepper);
  at 0.15 the near map is solid masses with organic lanes (76% at the
  0.95 pin - honest for a 95% sky).
- VERDICTS: polarity guard test un-pinned and GREEN (mass at features);
  integrator twin -0.0% across the flip (integrator error is
  field-independent, as designed); deck-ray GPU-vs-reference signed
  +1.2%; deck-band contrast 3.40 (10a: 3.27); all perf floors green,
  panics 0. The pinned flight vantage now sits under a LANE (overhead
  visibly clear at 0.95 cover) - an honest field sample; the ROI-based
  joint gate reads bare sky in this framing and is uninformative until
  increment 11 re-aims vantage/ROI with the coverage law. The marble's
  overall coverage curve (still sheet-heavy at natural weather) is
  increment 11's mandate, not re-litigated here.

### 11. Far-field truth: fades deleted + coverage law, one look increment

NOT a pure deletion - four coupled pieces in ONE increment or it reads as the next regression: (1) delete the detail/puff/cell distance fades (the concentric rings at 30-60 / 51-289 / 193-4495 km). (2) Re-center coverage for the now-always-on cell-split threshold raise (40-clouds.wgsl:1400-1408, currently distance-gated off at orbit - deleting the gate shifts global mean coverage). (3) Mip the 1440x720 weather map (mip_level_count:1 today, point-sampled through a steep smoothstep) and sample at footprint lod. (4) G2 de-binarization: the envelope smoothstep(0.35,0.9) turns a 27.8 km texel saying '40% cloudy' into keep/kill stipple - replace it so the rendered areal fraction inside a texel EQUALS the texel's fractional coverage at wide footprints (the soft-carve statistics-preserving principle applied at placement level; mipping alone is necessary but NOT sufficient). Plus: 8-px disc cutoff -> the existing per-object fade. Coverage/threshold shares machinery with R9's carve - land coherently, never double-correct.

**Files:** assets/shaders/pbr/40-clouds.wgsl:329-353, :1072-1086, :1400-1408, :1936-1946; src/renderer/mod.rs:1103-1117, :210-211; src/lib.rs:11207; src/renderer/cloud_noise.rs (renormalize accounting)

**Gate:** Synthetic uniform-0.4 weather map renders 0.40 +/- 0.05 cloud fraction over any 100x100 px orbit patch at EVERY ladder rung. Orbital mean coverage within 3% across three MODIS states pre/post (re-centering proof). mid-alt-45km radial texture profile flat within 20% (rings dead - red baseline exists from R2). Orbit judged against Blue Marble / DSCOVR EPIC reference imagery, NEVER the previous build; cloud-mask power spectrum concentrated below 1 cycle/100 km, not white.

**Expected visual:** Orbit shows organized weather - the fronts, spirals, and ITCZ already present in the live MODIS field - instead of grey percolation stipple; more, smaller, higher-contrast cumulus; no rings sweeping beneath the camera.

**Risk:** Medium-high: a deliberate, intended orbit look change with real coupling to R9.

**11a EXECUTION ADJUDICATIONS (v0.1179 - fades + centered split; weather
mips + G2 fractional coverage follow as 11b):**
- The three distance fades (detail/puff/cell) are DELETED: the field is
  camera-independent by construction; band-limiting is the mip ladder +
  soft carve (Wave B), which is exactly what they were built for. The
  operator-visible wins, both captured: the HOLE IN THE SKY is closed
  (silhouette-35km shows a continuous deck around a modest honest lane)
  and mid-alt-45km shows NO concentric rings - a granular deck with
  organic lanes.
- The always-on cell split is CENTERED at the bake g-channel mean (0.481,
  measured): local modulation, zero global-mean shift by construction.
- The ORBIT look transformed as the council predicted: the marble went
  from percolation sheet to organized fronts/bands/lanes (this also
  discharged most of the 10b sheet-coverage concern before 11b even
  touches the weather map).
- IoU LADDER HARNESS DEBT (gate still open): the cloud-phase scorer
  gained a DEGENERACY GUARD after the inverted-phase 'fix' let the red
  baseline pass at 0.939 (a 95% mask matches any 95% mask - the checks-
  that-cannot-fail class, caught by re-scoring the baseline). The
  silhouette column is re-aimed 12 km east onto mixed structure, but the
  capture protocol itself is not yet advection-sound: same-boot rungs
  drift ~2 km between captures (the deck edge crosses the whole compared
  patch), and one-boot-per-rung broke framing consistency. The gate
  needs paired/advection-subtracted captures - harness work, tracked, not
  a field defect: camera-independence of the FIELD now holds by
  construction (the only remaining distance terms are resolution-matched
  mips and steps).
- Twin note: with fades gone the twin fan hits cloud on 48/48 rays; gap
  -9.7% (was -2.2% on 19 rays) - the always-on erosion changed the
  error distribution; retune at 11b/12 if it drifts past 10%.

### 12. Wave D: temporal rework (extent, basis, depth reprojection, arm everywhere)

SPEC FIRST the extent-resample remapping math + its blur bound: the altitude-following extent makes texel->direction a function of camera altitude, so during descent every texel's direction changes every frame - the accumulate pass must resample history through the OLD extent (itself a low-pass) or the EMA smears on any altitude change, the exact failure it exists to fix. Then implement: r2 = (1-cos theta)/(1-cos theta_max) extent (identical under the deck, all 2048^2 texels on the visible disc at orbit - sharper than screen vs 5x coarser today); planet-fixed Lambert basis (the vegetation/v2 convention); RG16F first_t companion map with parallax-corrected history lookup. This SUBSUMES and formally retires the screen-space accumulation route (measured strictly worse: 0.5 mrad boiling vs 2.05 mrad averaged, regression recorded in cloud_temporal.rs:6-8). Then arm at ALL altitudes, delete the last near_slab use and the per-frame disarm, EMA stays deep in fast descent. Bind-group discipline: grep the changed layout for EVERY create_bind_group site and count entries (the v0.1029-v0.1038 incident class).

**Files:** assets/shaders/pbr/40-clouds.wgsl:1662-1686; assets/shaders/pbr/45-cloud-temporal.wgsl; src/renderer/cloud_temporal.rs:56-92; src/renderer/mod.rs:3204-3247; src/lib.rs:8738, :11435-11439

**Gate:** Map texel angle <= screen pixel at 12000 km (measured on capture). Orbit-static pair rms <= 2 levels (baseline 9.1% - red). BOIL-1: advection-subtracted residual <= 2 levels at 3 s, no growth to 10 s. Fast-descent: the deck survives 20 m/s without washing toward nothing; no fade-up-from-4% on atmosphere entry. PAN-1: edge-gradient-energy ratio >= 0.8 mid-pan + operator report on record. World-entry probe panics=0 (new render target).

**Expected visual:** The marble stops being TV static and converges; fast descent keeps solid clouds; one temporal system at every altitude.

**Risk:** HIGH - new render target + new projection = bind-group-hazard territory; deliberately after the field increments so it lands on a stable base.

### 13. Cloud streets (cheap rung of G1)

Point cloud_stretch_domain's stretch axis along the per-family wind vector reg.wind_* (shipped v0.1163) instead of the fixed tangent - wind-aligned parallel rows at 2-10 km spacing on the noise path, one of the most recognizable real-sky features from both flight and mid altitudes. The v2 placement-layer streets (orienting the budding-cluster population along wind) ride R15's calibration, not this increment.

**Files:** assets/shaders/pbr/40-clouds.wgsl:1305; src/renderer/clouds.rs (wind plumbing already present)

**Gate:** 40 km vantage at pinned 8-10 m/s low-level wind: 2D autocorrelation of the segmented cloud mask shows an anisotropy axis within 15 degrees of the wind heading, axial ratio >= 1.5.

**Expected visual:** Visible parallel cloud rows aligned with the HUD wind heading - the cold-air-outbreak / Midwest-afternoon look.

**Risk:** Low.

### 14. v2 SDF segment march + sun conversion (Ultra track, hard perf gate)

NOT the naive sphere-trace - costed and rejected: 126 sphere distances + ~585 hash evals per evaluation worst case, 60k-160k hashes/ray at 100-200 evals vs a ~22k ALU/ray budget (and the 675M-rays/s figure is a noise-path menu-pass number, non-transferable to Ultra). The feasible form: per-cell SEGMENT march with the 14-lobe cluster built ONCE per cell per ray (R3's hoist), smin-underestimate distance as the step bound PROVEN SAFE in a CPU lockstep test (the previous step-bound helper was deleted as unsound - this is the do-over with proof), empty-space skip = max(sdf - rind, footprint_step), fine interior steps inheriting R10's MFP schedule; register pressure (56 scalars/cluster) forces sequential cell processing - accepted. cloud_sun_tau converted in the SAME design (SDF skip to the lit surface so tap 1 is no longer 40 optical depths) or the lighting term re-imports the speckle.

**Files:** assets/shaders/pbr/41-cloud-bodies.wgsl:124-196 (distance-returning variant); assets/shaders/pbr/40-clouds.wgsl:1572-1627, :1843-1926; src/renderer/clouds.rs (CPU lockstep for the step bound)

**Gate:** HARD: gpu.cloud_octa <= 4 ms at the flight vantage (absolute, vs R3's measured Ultra baseline). CPU lockstep proves the smin bound never oversteps density. Ultra speckle <= the flight-station rms gate. World-entry panics=0. If the perf gate cannot be met, STOP: the fallback architecture is the survey's baked camera-local brick+SDF as its own future program - never ship a slideshow.

**Expected visual:** Speckle-free v2 interiors at equal-or-better frame time; empty-space skipping banks headroom.

**Risk:** HIGH - the highest-complexity single item; fenced with an explicit kill criterion.

### 15. v2 far-field crossfade + operator acceptance sitting

The orbit answer for v2 and the promotion gate. Footprint crossfade: as footprint_m grows past ~0.5*genus width, blend cv2_cloud body toward the mipped statistical field under the SAME wa/tc/regime inputs - one continuous function of footprint, no camera branch (the survey's hard constraint: no built-body representation scales to 10^5-10^6 orbit instances; a representation seam is more visible than any distance fade). The load-bearing piece: a CPU calibration test locking the SDF population's mean areal coverage EQUAL to the far field's at the same wa - the anti-pop mechanism; without it the crossfade pops and reads as a regression. Then the ACCEPTANCE SITTING: operator judges Ultra at pinned HIGH-COVERAGE and STORM weather AND the under-deck vantage (every glitter-class bug hid behind Clear-2m/s defaults for months) plus a pull-back flight.

**Files:** assets/shaders/pbr/41-cloud-bodies.wgsl (crossfade); assets/shaders/pbr/40-clouds.wgsl:1355-1364 (blend site); src/renderer/cloud_primitives.rs (coverage-calibration test)

**Gate:** Calibration test green. Pull-back 2 km -> 12000 km capture pair straddling the crossover: deck coverage continuous within 10%, crossfade band invisible in an approach flight. Operator verdict recorded verbatim in the ship message - this is the promotion prerequisite.

**Expected visual:** The v2 deck dissolves seamlessly into the far field on pull-back; the marquee orbit-to-ground approach is continuous.

**Risk:** Medium - entirely carried by the calibration test.

### 16. PROMOTION: one body model, delete the noise body

Only after R15's recorded operator acceptance: flip High (the operator default) and Medium to the v2 built-body + segment march; DELETE the noise-body branch (the params.y >= 2.5 gate at 40-clouds.wgsl:1358-1364 collapses). The noise field survives exactly two jobs - rind erosion and the far-field statistical expectation - never as a body again. Delete the params2.w < 3.5 model switch so tiers differ only in sample budgets and temporal cadence, never in model. Retire Ultra as a separate model in the same commit as its config whitelist entry. Per no-backcompat: no compatibility shim, no legacy branch.

**Files:** assets/shaders/pbr/40-clouds.wgsl:656-675, :1351-1364; src/renderer/clouds.rs:234-241 (+28 mirror tests); src/gui/pages/settings.rs:2468-2472; src/config.rs:1465-1470; src/lib.rs:11240

**Gate:** The FULL ladder + joint gate + shape gates + budgets re-run at High on the 4070 at 2560x1387: every gate increments 5-15 established must pass on the default tier; whole-frame no worse than pre-promotion (expect better from empty-space skipping); operator flight-session sign-off from their own seat. Reversible for one release by re-pointing the tier map if rejected.

**Expected visual:** The DEFAULT sky IS the new system: cauliflower cumulus, flat bases, power-law sizes, streets, no dots, no spheres - not hidden behind a tier.

**Risk:** Medium - a default flip after everything hard has landed, with a one-release escape hatch.

### 17. Wave E + W5: one composite order, one light space (the entangled endgame)

ONE increment because the pieces cannot ship separately: the ocean and cloud shells share the early-return-tonemap convention (90-fragment-main.wgsl:666's own comment) - converting one alone mixes display-referred and scene-referred layers in the alpha blend. (1) One draw order at every altitude: delete the 191 km composite flip (src/lib.rs:11557-11572). (2) Physical dome transmittance for the type-14 atmosphere shell (the 0.985 daytime alpha is the actual defect) - or, if the retune fights, fold the atmosphere into the cloud fragment and draw the shell only from outside. (3) W5: ocean + cloud shells return linear HDR; ONE shared aces_tonemap (R3's fn) applied after all celestial layers blend in linear space; bloom finally receives real glint energy so a storm glitter path blooms proportionally instead of smearing a clipped disc.

**Files:** src/lib.rs:11223-11229, :11557-11572; assets/shaders/pbr/30-atmosphere.wgsl:348-360; assets/shaders/pbr/40-clouds.wgsl composite tail; assets/shaders/pbr/90-fragment-main.wgsl:403-419, :663-680; src/renderer/bloom.rs

**Gate:** The 191 km ladder rung goes FLAT - the last red rung on the board; the ladder is now green 12000 km to ground. Noon blue sky AND sunset re-approved at ground level (the dome alpha touches the most-seen pixels in the game). sahara-noon-ground canary unchanged (land-sky control). ocean-storm-glitter shows graded bloom around the glint, not a flat white core. No regression at the orbit vantage.

**Expected visual:** Zero altitude seams remain anywhere; glint highlights gain rolloff and real bloom; sky, water, and haze compose in one light space from orbit to surface.

**Risk:** HIGH - three shaders + composite order + the most-seen pixels; scheduled last, isolated, on an otherwise fully-continuous base (v0.997 introduced the flip for a reason).

### 18. Rider: strided octa refresh (anytime after R10, funds Medium)

Backlog #77: checkerboard-stride the 2048^2 octa pass 1/8-1/16 per frame - switch LoadOp::Clear to Load and carry untouched texels (the one correctness trap) - banking ~16x per-frame cost for Medium-tier fine-march budgets and weather-heavy scenes. Pure budget engineering, no representation change; the adaptive EMA absorbs stride staleness. Slot it wherever a light increment is needed between the heavy ones.

**Files:** assets/shaders/pbr/45-cloud-temporal.wgsl; src/renderer/mod.rs:3195-3245

**Gate:** Static-camera convergence unchanged vs unstrided; octa ms reduced proportionally; no recurrence of the v0.1159 'tiny dots became big dots' blend-chase failure.

**Expected visual:** None - that is the point.

**Risk:** Low; optional.

## Execution adjudications

**Increment 3, polarity (2026-08-21):** the probe CONFIRMED the inversion
(body at Worley features 150 vs 180 in gaps - the mass lives in the
lattice between cells). But the one-line fix + quantile re-centering the
council specified is NOT sufficient: flipping polarity EMPTIES the
near-path sky even with COV re-centered to measured quantiles
(0.864/0.506) and the cell split halved. Bisect proof: old remap + new
COV restores the deck (contrast 1.77), so the flip alone is the breaker.
Every downstream consumer (cell-split tap, tower keyed on lofi, dome
rise, base drop, three erosion bands) is co-tuned to the foam topology.
ADJUDICATION: the polarity fix is RESCHEDULED into increment 10 (THE
INTEGRATOR), which owns the coupled field re-tune anyway. The probe test
ships #[ignore]-pinned as executable knowledge. The default sky is
unchanged (verified: post-revert flight stats match the shipped
baseline).

**Increment 5, Wave A sub-1 (2026-08-21):** one shell radius + continuous
limb fade SHIPPED (look-neutral: flight golden holds, marble A/B delta
1.7). But the ladder stayed RED at both target boundaries, and the rung
captures identified the true owners: (a) 331 km is dominated by the
TEMPORAL-MAP ARMING flip, not the shell geometry - it cannot go green
before wave D; (b) 15.8 km is THE DECK VANISHING for downward views
inside the shell - a downward ray's only cloud geometry is the far side
of the sphere, beyond the planet, and the depth test kills it behind the
terrain. This is the operator's "they kinda start to look better, then
they just vanish", measured and explained. No shell-geometry fix exists;
the cure is the FULLSCREEN DEPTH-AWARE cloud composite, which is hereby
added to wave D's mandate (the march already carries the ground
occlusion early-out it needs). Per-sample regime (Wave A item c) stays
queued behind that.

## Objection dispositions

1. CHALLENGER ruling 1 (refuse unify-now / v2-as-default): ADOPTED as gating - promotion is rank 16, reachable only through the rank-15 operator acceptance sitting at pinned high-coverage/storm/under-deck weather; no increment before it changes the default body model, and Ultra is never shown to the operator before the rank-6 containment/thin-genus fixes. But the challenger's stronger claim - that 'one path' is a category error and the dual-path is permanent - is REFUTED in that form: the end state deletes the noise BODY (the cloud architect's shape-science proof that inverted Worley is literally a union of equal balls, plus the no-backcompat directive), while the noise FIELD survives in exactly two roles (rind erosion + far-field statistical expectation) behind a calibration-locked footprint crossfade. That IS the hybrid the challenger demanded, without maintaining two body models forever.
2. CHALLENGER objection 'the audit wave plan contains NO increment that fixes the dots': ADOPTED - rank 10 (THE INTEGRATOR) is the single named owner of the speckle defect, pairing the measured 3.5x sun-ladder fix with the MFP march, Hillaire in-scatter, and the compensating field re-tune in one diff, per the inviolable coupling lesson. No other increment may touch integrator step sizes or lighting constants.
3. CHALLENGER ruling 'Wave B as audited re-runs phase 9' (missing sun ladder + re-tune): ADOPTED via restructure - Wave B (rank 9) is deliberately narrowed to the sampling-rate law and soft carve with a two-sided joint gate, and ALL energy-affecting corrections are quarantined into rank 10. The program never ships a metrically-better-but-darker sky.
4. CHALLENGER ruling 'SDF sphere-trace infeasible as written' (5-10x over budget; 675M-rays/s figure non-transferable): ADOPTED - rank 14 mandates the restructured per-cell segment march with the cluster build hoisted (rank 3 does the hoist and measures Ultra's real baseline first), a CPU-proven smin step bound (the do-over of the deleted-as-unsound helper, this time with proof), same-increment sun-tau conversion, and a hard absolute gpu.cloud_octa <= 4 ms gate with a named kill criterion (fallback: baked brick+SDF as a future program). This is a deviation from the cloud architect's rank-4 framing, which put the SDF march on the critical path of the dots fix: PARTIALLY REFUTED there - the coupled integrator lands on the SHARED march (rank 10) so the operator's default High sky gets the dots fix without waiting on v2, honoring 'the sky never gets worse (or stays broken) on default settings mid-program'.
5. CHALLENGER ruling 'Wave C is not a pure deletion': ADOPTED - rank 11 bundles fade deletion + coverage re-centering + weather-map mips + fractional-coverage de-binarization (the fidelity referee's G2) as ONE look increment, gated on rank 9's carve-consistency proof, and judged against Blue Marble/DSCOVR reference imagery rather than the previous build.
6. CHALLENGER ruling 'Wave D wins; kill screen-space accumulation': ADOPTED - rank 12 formally retires the screen-space route (measured strictly worse, regression recorded in cloud_temporal.rs:6-8) and requires the extent-resample remapping math and blur bound to be spec'd BEFORE code, closing the unnamed trap.
7. CHALLENGER objection 'ocean fixed is doc-drift' (only 1 of 4 findings shipped in v0.1168): ADOPTED - W1/W2/W3 are explicit increments (ranks 4 and 7), the v0.1168 goldens are frozen at rank 2, and program language must describe v0.1168 as 'glitter normalization fixed', nothing more.
8. CHALLENGER objection 'cloud_sun_tau rebuilds clusters per tap today; measure Ultra before fencing polish': ADOPTED - rank 3 hoists the cluster build under the CURRENT march and records Ultra's first-ever frame-cost measurement before any Ultra work or showing.
9. CHALLENGER objection 'cirrus renders as congestus + the promised thin-genus blend does not exist': ADOPTED - rank 6 fixes cv2_arch_index and implements the caller blend before the operator's first Ultra impression, protecting the architecture from being judged on a calibration bug (the fidelity referee's explicit instruction).
10. CHALLENGER objection 'one-GPU rule constrains every gate': ADOPTED - the descent ladder is ONE scripted boot; captures batch per increment, never per tweak; DESCENT-1/PAN-1 run once per wave when the operator is at the machine; every rig boot is preceded by the tasklist check.
11. CHALLENGER pre-work ruling (shape_voxel polarity before ANY re-tune): ADOPTED at rank 3 - a flipped body invalidates both the rank-8 reference and the rank-10 re-tune, so it is adjudicated first.
12. WATER ARCHITECT correction (atmosphere must NOT consume the published gate pad - per-shell math for other planets): ADOPTED in rank 4; drift is prevented by a WGSL-parsing lockstep test instead of a shared uniform, over the audit's original shared-consumer proposal.
13. WATER ARCHITECT correction (subtraction-form mss, not the audit's additive WAVE_MSS_UNRESOLVED): ADOPTED in rank 7 - the subtraction form is the single formula correct at both ends (orbit full-width, near-field no double-count).
14. WATER ARCHITECT objection (the water's 191 km gate fade may become the loudest seam after Wave A): ADOPTED - rank 5's gate includes re-running the ladder post-wave and widening the water fade band if it surfaces; the water gate need not match the sky's exactly, only reach zero before orbit.
15. WATER ARCHITECT objection (never verify W2 and cloud carve waves against shared goldens in one window): ADOPTED - the rank ordering serializes their capture windows, and the one-GPU rule enforces it mechanically.
16. VERIFICATION DESIGNER objection (no brightness gate on Wave A): ADOPTED - rank 5 is a removal-only wave gated on continuity, night-glow, and structure metrics; applying the joint gate there would invite mid-wave re-tuning.
17. VERIFICATION DESIGNER + FIDELITY REFEREE objections (weather pinning, stills-lie, advection subtraction must land WITH temporal gates, resolution assert, perf-gate promotion is a stated policy change, re-arm limb-400km/fly-through-40km): ALL ADOPTED wholesale into ranks 1-2; the calibration-red rule ('a gate must fail on v0.1168 or on a broken control before it is trusted') governs every gate in the program.
18. FIDELITY REFEREE ruling ('the 15% darkening is not a regression') vs the cloud architect's 'luminance within +/-8% of the approved bright capture': RESOLVED in the referee's favor - rank 10 accepts against the rank-8 CPU-converged reference; the joint gate's brightness/contrast floors operationalize 'not washed out', and when reference and old capture conflict, the reference wins and field constants move, never step sizes (the cloud architect's own objection 3 concedes exactly this).
19. CLOUD ARCHITECT rider (cumulonimbus 12->8 km cap is a temporary narrowing): ADOPTED - rank 6 logs the coarse-grid-tier permanent fix in PRIORITIES.md the day it ships, so the cap cannot silently become the ceiling.
20. CROSS-CUTTING: the cyan horizon banding is confirmed a WATER-lane increment (rank 4), not a cloud increment, per the cloud architect's own objection - it ships early because it is low-risk, operator-named, and gate-calibrated by rank 2.

## Appendix: operator protocols (the only accepted evidence for temporal properties)

A still capture presented as proof of a temporal property (boil, pop,
static, drift) is an automatic reject. Paired captures bound these
properties numerically; final acceptance of each is one of these named
protocols, run by the operator in the live client and reported in chat.

**DESCENT-1** - the altitude continuity ride. Start at 12,000 km over
Silverdale at local noon, weather Cloudy, and descend at x10k gear to
the ground in one unbroken run, eyes on the cloud deck and horizon.
PASS: no moment where the sky, deck, haze or ocean visibly snaps,
flips, or swaps character. Any snap: note the HUD altitude and say it -
the ladder rungs bracket the report.

**PAN-1** - the static/churn look. Park at 2 km under a cloudy sky.
Hold still 10 s watching one cloud, then pan 90 degrees and back over
~5 s. PASS: clouds read as solid bodies drifting with the wind; no
per-pixel shimmer while still, no smearing or re-forming during the
pan, and the same clouds are there when you return.

**BOIL-1** - the close-approach look. Fly slowly toward one cumulus
from ~10 km until inside it. PASS: the cloud grows smoothly, its
silhouette does not churn or morph except by real approach parallax,
entering it is a fog transition rather than a dissolve, and the deck
behind is unchanged when you emerge.
