# HumanityOS: Priorities

> **v0.1260.0 (2026-09-01): the retired map was still being composited
> - the operator diagnosed it blind.** They asked "is there another
> shader or texture affecting cloud shaders that is not supposed to be?"
> There was exactly one: the octa direction map stopped DISPATCHING in
> v0.1250 but cloud_composite.wgsl kept BINDING its texture and
> blending it under every cloud pixel as the near-over-map backdrop. A
> render target that is never written is not a guaranteed-zero source
> across backends and driver paths, and whatever it held went into the
> final cloud colour - a plausible source of the "clouds disappear in
> weird splotches" report. The backdrop term is now literally zero.
> ALSO: the v0.1258 trapezoid was a NO-OP - dens_prev is advanced one
> line above where it was read, so it averaged a value with itself.
> Fixed (capture before advance); with it actually working the
> coverage-alpha grain went 2.04 -> 1.86, and the 1.91 previously
> credited to it was noise. LESSON for the next march edit: dens_prev /
> sdf_prev are advanced BEFORE the shading block, so anything wanting
> the previous step must capture it first.
> FOLLOW-UP available if splotches persist: the composite still binds
> cloud_map and keeps map_catmull_rom; removing the binding and the
> whole map arm (plus the octa textures and their pass) is a clean
> deletion now that ONE RENDERER is settled, and would also free the
> 4096^2 RGBA16F pair.


> **v0.1259.0 (2026-09-01): BLUEPRINT 1 TESTED AND REFUTED - a real
> negative result, plus the F10 shape A/B.** Implemented the designed
> cure for the rosette (per-mip CDF histogram matching in the noise
> bake, replacing the linear mean/sigma renormalization) and ran BOTH
> harnesses. It measured WORSE: mips 1-6 drifted +27/37/37/56/47/74%
> against the linear form 18/28/27/35/34/53%. The width harness then
> refit the table DOWN and passed its own gate, but coverage did not
> recover. REASON (now written into the width-table comment so nobody
> retries it): matching the GLOBAL distribution does not make a mip
> agree LOCALLY with the area-averaged truth - thresholding a smoothed
> field is not the same as smoothing a thresholded field. Bake and
> table reverted; gate green. THE LEVER IS THE CONSUMPTION SIDE: refit
> the soft-hinge width against COVERAGE rather than the mean - which
> is exactly what the standing note in clouds.rs coverage_vs_mip has
> said since it was written. That is the next attempt at the rosette,
> and it needs a coverage-objective fitter (the existing harness fits
> mean-error), which is a contained, testable piece of work.
> SHIPPED FOR TESTING: F10 "Cloud shape frame" checkbox (and showcase
> {"cloud_shape":"0"}) rendering the pre-v0.1256 isotropic ball
> cluster, so the squash/wind-stretch work can be A/B compared live.
> The pad light7_color.w is now a bit field (bit 0 dither, bit 1
> shape); the dither test was corrected to read bit 0 only.
> QUEUE: coverage-fitted widths (rosette); multi-layer atmosphere (top
> fidelity item); perf part 2 (Nubis3 light grid + adaptive res).


> **v0.1258.0 (2026-09-01): THE ROSETTE UNIFIED WITH THE MEASURED MIP
> DRIFT - Blueprint 1 is the cure, not a side quest.** The operator
> described the artifact geometrically for the first time: "the bottom
> 45 degree cone beneath my feet is scrunching the clouds together...
> at a distance the textures look proper fluffy, closer to my feet they
> warp inwards", plus "the static is concentrated at the CENTER of the
> clouds". THE UNIFICATION: both quantities that set the march sampling
> scale are monotone in the angle from the NADIR - r_rate (radial dot
> ray) drives dt_vert, and foot = tm * pix_ang drives lodb/mip. Step
> size AND detail mip therefore vary radially about the feet BY
> CONSTRUCTION. The v0.1252.4 workflow already MEASURED that the
> rendered result is not invariant to that scale (carve response
> residual doubling per rung from mip 3, ~15% relative near threshold,
> eight uncompensated taps). Appearance varying with a quantity that
> varies radially about the crosshair IS the rosette. So BLUEPRINT 1
> (per-mip histogram matching in cloud_noise.rs renormalize_level, then
> refit the carve width table from the harness) is the direct cure for
> the oldest complaint in the arc and is now THE next increment.
> SHIPPED this round: trapezoid step integration (the exact integral of
> a linear ramp over the step, using the endpoint density already held;
> halves estimator variance, free; speck-alpha 2.04 -> 1.91).
> ELIMINATED WITH NUMBERS: fine near-surface refine (-25% grain but
> +64% cost, rejected as a trade); the fine erosion band (2.13 vs 2.04,
> not the carrier); surface-centred sampling (2.39, worse - the
> deterministic comb returns); interior turbulence 0.42->0.15 (1.99,
> noise). Queue after Blueprint 1: multi-layer atmosphere (the top
> FIDELITY item), perf part 2 (Nubis3 light grid + adaptive march res).


> **v0.1257.0 (2026-09-01): cloud cost pass part 1, and the ATMOSPHERIC
> LAYERS answer.** Operator at sub-1 FPS on max settings. The sun
> ladder outnumbers view samples 12:1 and was doing two kinds of pure
> waste, both now removed: (1) SLAB SKIP - the geometric ladder reaches
> ~125 km while the cloud band is ~12 km, so far taps paid a FULL
> cluster evaluation (3x3 search + 20-lobe build) to learn there is no
> cloud in empty stratosphere (the view march clips to the slab; the
> sun ladder never did). Skip, not break - a low sun re-enters along a
> shallow chord. Physically exact. (2) COARSE SUN CLUSTER - sun taps
> built all 20 lobes and then had the result smoothed by the 260 m sun
> rind, which discards that detail BY DESIGN; lobes are placed
> largest-first so the sun now unions the first 6 only. The eye keeps
> all 20 - silhouettes are never coarsened. MEASURED: closeup 74.5 ->
> 59.7 ms (-20%), full res 256 -> 184.5 ms (-28%), look unchanged.
> NEXT ON PERF (part 2, still needed - this is not enough): the Nubis3
> amortized summed-density light grid, already scoped in the v0.1252.2
> workflow output at ~40% march savings plus long-range inter-cloud
> shadows; then adaptive march resolution keyed on shell screen
> coverage (full res is cheap from space, brutal inside the deck).
> THE OPERATOR ASKED whether real ATMOSPHERIC LAYERS would help the
> clouds. YES, and it is now the TOP FIDELITY ITEM above further
> single-deck polish: a sky of isolated puffs at ONE altitude cannot
> read as a real sky however good each puff is. Real skies are almost
> always multi-layer - low cumulus/stratocumulus, mid altocumulus and
> altostratus, high cirrus - and the layering (different heights,
> different characters, different lighting, one seen THROUGH another)
> is most of what the eye uses to judge a sky. The genus archetypes
> already exist; what is missing is independent DECKS at their own
> altitudes rather than one band that picks a genus.


> **v0.1256.0 (2026-09-01): THE PRIMITIVE WAS A BALL - per-cloud shape
> frame.** Operator: "How can we make these clouds incredibly less
> spherical... still just giant cotton balls of slightly varying
> shape." Root cause, finally named: every lobe is length(p-c)-r, a
> literal SPHERE. Domain warp, displacement, erosion and the smooth
> union all DECORATE a round object - which is why twenty fidelity
> increments never cured the cotton-ball reading. cloud_v2_body now
> transforms the QUERY POINT into a per-cloud shape frame before the
> cluster SDF: rotate into a wind direction shared across 8-cell
> patches (free cloud STREETS), divide by shape axes (vertical squash
> 0.42-0.88 from genus aspect, along-wind stretch 1.0-1.5, cross-wind
> the reciprocal so the three multiply to 1 - volume and areal
> coverage untouched), then scale the distance back by the smallest
> axis so it stays a conservative bound for the march leap. Placement
> AND lobes live in the frame, so a cloud stretches as one object, not
> a string of stretched beads. Cost measured near-zero (12 vs 13.3
> FPS). LOCKSTEP by design: applied in the FIELD, never inside
> cv2_cloud_sdf, so the CPU twin and its 8 tests stay exactly valid.
> NEXT, in order: (1) PERFORMANCE - at full res the march IS the frame
> budget (4-5 FPS); a cost pass (step-count ceiling, empty-space
> skipping, adaptive res) is what makes Half/Full usable, and it is
> now the top item. (2) Coverage continuity across the pre-volumetric
> to volumetric handoff. (3) Blueprint 1 (bake histogram matching) -
> still the one MEASURED suspect left for the rosette. (4) Blueprint 2
> remainder: size-span widening (V2) and the giants tier.


> **v0.1255.0 (2026-09-01): CLOUD RESOLUTION IS A SETTING - the operator
> question answered, and it was the biggest untried lever.** Yes: the
> cloud layer really is lower resolution than the surface. The march has
> always run at a QUARTER of screen resolution (one cloud sample per 4x4
> screen pixels) while terrain renders full-res beside it - that IS the
> "solid pixels" look and the "100% transparent or opaque" edges. New
> cloud_res_div (4 quarter / 2 half / 1 full) in the F10 panel and via
> showcase {"cloud_res"}. The march footprint now derives from
> ndc_step.y * tanf (its own rasterized Nyquist rate) instead of a
> hardcoded *4.0: bit-identical at quarter res, and it makes a
> resolution raise buy real FIELD DETAIL rather than a sharper upsample.
> MEASURED at the closeup vantage: 13.3 / 6.3 / 2.0 FPS for quarter /
> half / full; half renders visible cauliflower turret structure where
> quarter renders blobs. HALF IS THE SWEET SPOT pending a perf pass.
> ALSO SETTLED: the sun-cone spiral is INNOCENT of the rosette - polar
> decomposition of the direct-sun bisect, cone ON vs OFF, is identical
> within content noise (ring 10.77 vs 10.01, spoke 13.73 vs 13.50). The
> rosette is STILL UNCAUGHT after clearing every renderer mechanism;
> the measured mip-response drift (Blueprint 1, histogram matching) is
> the remaining named suspect and the next increment. THEN Blueprint 2
> (field variety) for the cotton-ball shapes. PERF NOTE for the
> resolution work: at 13 FPS quarter-res the march is already the frame
> budget - a cost pass (step-count cap, empty-space skipping) would let
> half res be the default.


> **v0.1254.4 (2026-08-31): the F10 cloud dev panel + THE MEASURED
> CALIBRATION BLUEPRINT.** F10 now opens a Cloud Dev panel (GUI-first:
> dither toggle, temporal toggle, bisect-channel selector) driving the
> SAME gui_state fields the showcase pins write - one source of truth.
> THE WORKFLOW MEASURED THE AGATE (it ran coverage_vs_mip +
> carve_consistency live): areal coverage is mip-INVARIANT at overcast
> thresholds - the arcs are carve RESPONSE drift (residual DOUBLING
> per rung from mip 3; up to ~15% relative near threshold) + the
> trilinear inter-mip sigma dip (5-13%, what the lod dither grinds
> into static). EIGHT uncompensated taps enumerated - the CELL tap
> (5 mips deep, feeds LWP luminance) is the prime agate candidate; the
> weather-map mips have NO renorm at all; the sun-march taps ride the
> whole uncompensated chain deepest. NEXT INCREMENT (Blueprint 1, the
> agate cure): replace the linear per-mip renorm in cloud_noise.rs
> renormalize_level with per-mip HISTOGRAM MATCHING (256-bin CDF LUT
> onto level 0) - drift-free at EVERY threshold by construction and
> cheaper than the current renorm; then re-fit the carve width table
> with the harness; acceptance = flower-nadir with dither OFF shows NO
> arcs, and both dithers retire. THEN Blueprint 2 (the cotton-ball
> cure): field variety - size spread, base-altitude scatter, wind
> stretch/streets (kills the perspective rush + the operator's
> "spherical cotton balls"). Full blueprints:
> %TEMP% claude tasks/w8911qxqs.output. Edge translucency ("100%
> transparent or opaque at edges") rides the same calibration.

> **v0.1254.2 (2026-08-31): THE FADE BAND WAS THE WARP - crescents +
> sheet-to-snowflakes, one root.** Operator on v0.1254.1 ("huge
> improvement!"): two remaining asks - the white sheet devolving into
> snowflake specks on approach, and the residual "rosette" warping a
> FIXED-distance cloud (eaten centers, C-shapes). Rig bisect chain:
> crescents reproduced over the Sahara at 2 km; SDF-leap-off left them
> STANDING (leap innocent, restored); fade-off dissolved them into
> solid masses at identical FPS (CONVICTED). The v2-to-noise
> representation handoff is distance-keyed and its band interferes:
> mid-fade = crescents, far end = the noise sheet collapsing into
> discrete v2 clouds. SHIPPED: CLOUD_V2_FADE_LO/HI 1.9/2.0 -> 3.9/4.0
> - the whole flying band is pure constructed bodies and the morph
> happens where a cloud subtends ~a pixel (invisible by scale
> separation). NOT infinity: at orbital footprints sub-footprint lobes
> would point-sample as speckle; the carve-hinge noise body stays the
> correct coarse representation. Verified: crescent vantage solid at
> 15 FPS; space disc granular banks at 17 FPS. WATCH: far-range
> coverage calibration across the now-distant handoff (occupancy
> growth cap) if an approach still reads a density pop; the
> low-alt/orbit handoff observation from the operator ("voxel clouds
> swap to shader cover") should now also be re-judged - it was this
> same band.

> **v0.1254.0 (2026-08-31): THE OPERATOR'S EXPERIMENT SETTLES IT -
> frozen jitters + the gray-sun gate.** The operator ran the
> cloud_temporal live toggle and delivered the decisive result:
> temporal on/off changes only SOFT static vs SHARP static, and LOW
> quality (the direct shell path - one smooth unjittered sample per
> screen pixel) is the ONLY clean tier. Verdict: the TV static was IN
> THE INPUT all along - three frame-advancing white-noise jitters
> (subpixel ray, depth, lod dither) re-rolled every frame on a
> quarter-res grid, which no accumulator can average under motion and
> which reference titles (NMS, Elite, Helldivers) simply do not do.
> SHIPPED: all three jitters FROZEN to static per-pixel hashes -
> spatial dither survives (rings stay dissolved; flower-nadir guards),
> but a parked frame is now pixel-identical to the last: no fizz, no
> film-grain crawl, no blinking clouds. DO NOT reintroduce a frame
> term without re-running the on/off experiment. ALSO: the permanent
> gray sun after surfacing = sun_cloud_alpha crossing the DRAWN shell
> (~51 km) instead of the density band top (~12 km) - a cloud 10 km
> below the sightline dimmed the disc anywhere in the 6-51 km window;
> now gated on the composite frame's own band top. Shipped without a
> rig sweep (operator game running, one-GPU rule; operator verifying
> live). WATCH: any banding/ring return at nadir (the frozen dither
> keeps spatial decorrelation, but the temporal averaging of residual
> pattern is gone); the low-orbit swap ("voxel clouds disappear and
> swap to the shader cover on approach" - the operator's handoff
> observation, needs its own look).

> **v0.1252.8 (2026-08-31): MERGED-CAP CONSTRUCTION - change 4 landed,
> the arc's numbers.** The structural half of the field-coherence
> rebuild: r_hi 0.44*width, r_lo 0.11 (bounded by r_hi - flat genera
> inverted the pareto clamp, the CPU twin caught it as a panic where
> WGSL would have silently saturated), budding 0.45-0.62, relative
> smin floor 0.5*mean_r (cap 340 m). CPU twin mirrored SAME COMMIT,
> unifying two pre-existing drifts (twin r_lo 0.05 vs shader 0.06;
> twin blend_m unclamped). NEW PERMANENT HARNESS:
> projected_fill_fraction_report in cloud_primitives.rs (24 clouds x 4
> genera) - cv2_fill_frac now carries MEASURED fills 0.807/0.810/
> 0.381/0.849; re-run + re-paste after ANY lobe-construction change.
> MEASURED: speck-sun 1.12 -> 0.36 (below the pre-arc alpha floor);
> closeup 1.01 -> 0.69. ARC TOTALS (one night): closeup grain 2.71 ->
> 0.69 (-75%), sun channel 2.57 -> 0.36 (-86%), plus the orchard-rows
> root cause and the dot-lattice diagnosis. RESIDUAL, the last named
> carrier: dark speckle on cloud faces = view-path fine texture +
> remaining shading terms; next instrument round = a VIEW-PATH TEXTURE
> BISECT (showcase pins disabling erosion bands one at a time, same
> map_diag pattern). Then: small-disc dynamic march resolution;
> terminator vantage; sunset grazing twinkle re-check.

> **v0.1252.7 (2026-08-31): THE ORCHARD ROWS - the starburst-at-the-
> feet ROOT CAUSE, found and fixed.** The operator's oldest complaint
> ("the starburst at my feet", persisting through the map retirement,
> the reprojection rewrite, and the whole lighting overhaul) is the
> cloud PLACEMENT LATTICE seen from above: straight rows of clouds on
> the 1.1 km brick grid project as SPOKES THROUGH THE NADIR (the
> orchard-from-a-drone effect) - view-locked at any altitude, stronger
> with height, content-side so no renderer change could touch it.
> Photographed on the rig at 41 km midday (rows converging at the
> crosshair), killed with CLOUD_V2_ROW_WANDER 0.28 (per-row smooth
> sine, wavelength ~9 cells, random phase per row - no straight line
> of centres in any direction; adjacent clouds shift together so the
> v0.1232 clumping-cost lesson holds; y-only, inside the 3x3 search
> budget). Checked-innocent on the way: godray behind-camera guard,
> the three ray builders' tanf/aspect/ndc consistency, the water
> sky-mirror. OPERATOR-VERIFY next flight: the down-look at altitude
> that always showed the starburst. Remaining queue unchanged:
> merged-cap construction (change 4, fenced, spec in the v0.1252.6
> block), small-disc dynamic march resolution, terminator vantage for
> the low-sun check, the sunset grazing twinkle.

> **v0.1252.6 (2026-08-31): THE DOT LATTICE named and half-killed - the
> field-coherence shading wave.** The bisect instrument photographed
> the truth: the direct-sun channel over cumulus is a LATTICE OF
> DISCRETE DOTS, one per constructed-body lobe - per-lobe cap-vs-
> crevice contrast 4.7-11x where physics allows 1.1-1.35x below the
> ~300 m radiative-smoothing scale (Marshak 1995). THAT is the
> operator's "TV static"/"sandblast"/"atoms of spheres" across 15
> releases, and near the nadir it composes into the melted flower. A
> 3-agent workflow audited the chain (sun-tau carries 85-95%; the
> v0.1252.4 profile-all change had the sun marching the BARE lobe
> cluster) and designed 5 ordered changes. SHIPPED (1,2,3,5): sun
> marches the envelope with a 260 m ramp (CLOUD_V2_SUN_SMOOTH_M) + no
> warp in profile mode; cavity-on-direct compressed to 1.15x on the
> built path (cav_dir_w); hybrid-band ring neutralizer (ring_off);
> stale jitter comment fixed. MEASURED: full closeup grain 2.59 ->
> 1.01 (61%, the largest single improvement of the arc; everything
> before combined was ~5%); the central cumulus renders as a solid
> luminous mass for the first time. FENCED NEXT (change 4, atomic):
> MERGED-CAP CONSTRUCTION - r_hi 0.34->0.44*width, sep 0.45-0.62,
> RELATIVE smin floor 0.5*mean_r (cap 340 m), fill table RE-MEASURED
> via the projection harness (never estimated), CPU-twin mirror in
> src/renderer/cloud_primitives.rs + lib tests IN THE SAME COMMIT,
> CLOUD_BODY_TOP p99 re-checked. Full acceptance protocol G1-G6 (incl.
> the connected-bright-components contiguity metric and the
> anti-cardboard joint gate) in the workflow output:
> %TEMP%/claude tasks/w84w6pbni.output. TUNING KNOBS if the operator
> reads peak whites as washed: SUN_SMOOTH_M toward 200, cav_dir_w 0.12
> toward 0.2. WATCH: low-sun whole-cloud shadowing (add a terminator
> vantage); shaded-side speckle persists until change 4.

> **v0.1252.5 (2026-08-31): the budget stride reverted - it re-created
> the melted flower at night.** Operator night captures on v0.1252.4:
> giant dark melted-agate flower inside the deck at 2-7 FPS. The
> v0.1252.4 every-step budget stride was the creator: with the slab
> exit hundreds of km away (always, inside the deck),
> max(dt, remaining/left) forced KM strides from the FIRST sample,
> overriding the MFP/SDF refinement - and coarse near-steps are the
> v0.1241 melted-flower mechanism verbatim. REPLACED with final-step
> tail integration: only iteration 224 stretches to cover the
> remaining segment as ONE coarse sample (footprint self-selects a
> deep mip); near sampling untouched; truncation bias still bounded.
> NEW permanent vantage night-flower (5.7 km, rain, post-sunset)
> guards the night/inside-deck state no daytime vantage covered - the
> fix capture there: black, soft, structureless, 21 FPS. LESSON (for
> the incident book): a budget clamp must NEVER touch near samples;
> and every march change needs a NIGHT + inside-deck verification, not
> just the daytime ladder. OPEN from the operator's report: the
> grazing-sun TWINKLE (bright spots return when raising height or
> nearing the sun - direct-channel, sun-angle-dependent, watch after
> this fix); the faint 198 km night tail rosette.

> **v0.1252.4 (2026-08-31): the rest-state fixes - four symptoms, four
> mechanisms, all closed.** Operator on v0.1252.3, the most diagnostic
> report of the arc: (a) faint rosette LOCKED TO THE CURSOR while the
> planet turns; (b) "1900s film dust" crawl + clouds BLINKING in/out
> ONLY when parked; (c) moving = clean (the inversion!); (d) white
> sparkle on shadowed faces. Mechanisms: (a) = TWO stacked causes, both
> closed - the radial history-stretch limit cycle about the approach
> epipole (fix: sustained-zoom alpha escalation to 0.95) AND the
> iteration-cap truncation boundary, a function of slant = screen
> radius from the aim point (fix: BUDGET-AWARE STRIDE - when
> iterations run low the stride grows to cover the segment instead of
> truncating; the grazing iteration-cap tail is CLOSED). (b)+(c) = the
> variance clip's fingerprint: parked, deep blend converges but
> clamping history into each frame's noisy box re-injects noise as a
> slow random walk; ghosts require motion, so gamma now widens to 3
> sigma at rest, tightening to 1 under motion (fix: MOTION-ADAPTIVE
> CLIP GAMMA). (d) = the detailed first-two sun taps letting
> single-pixel full sun through dark faces (fix: sun-profile for ALL
> taps; speck-sun grain 2.57 -> 1.58, near the alpha floor).
> flower-nadir's down-look now renders with NO rings/petals/fibers -
> first time in the arc. Remaining fenced bosses unchanged: small-disc
> dynamic march resolution; erosion coherence + Nubis3 light grid.

> **v0.1252.3 (2026-08-30): the dark-cloud clue - the alpha-edge alias
> found by derivation audit.** Operator on v0.1252.2: "static always
> present, even when the clouds are dark" (dusk silhouettes, orange
> sparkle rims). Lighting-independent grain = the carrier is ALPHA at
> edges. Three fixes: (1) CR NEIGHBOURHOOD CLAMP in the composite - the
> v0.1251 Catmull-Rom's negative lobes were SHARPENING unconverged
> march noise into full-screen dots during fast flight (the operator's
> low-orbit pepper); clamped to the 2x2 texel min/max, standard
> practice. (2) ABSOLUTE-sigma gate on the resolve spatial filter - the
> relative-only test under-engaged on bright decks. (3) THE BIG ONE:
> CLOUD_V2_INT_LODC -7.9 -> -9.56. Every v2 tap's lod constant follows
> log2(tile/256) within 0.1 mip EXCEPT the interior turbulence, which
> matched no derivation and sampled 3.2x finer than the footprint -
> view-path aliasing wherever alpha does not saturate: silhouette
> sparkle, thin skirts, down-look pinholes. TWO FENCED BOSSES REMAIN
> (the honest residuals): (a) SMALL-DISC RESOLUTION - the quarter-res
> march gives a 150 px planet disc ~37 cloud samples across (the one
> thing the retired octa map did better); needs dynamic march
> resolution keyed on shell screen coverage. (b) EROSION COHERENCE -
> gaps/gap-edges in the field are near-binary at meter scale where real
> cloud edges are translucent over tens of meters; the field-character
> rebuild (with the Nubis3 summed-density light grid as its companion).

> **v0.1252.2 (2026-08-30): the sun-profile cutover - reference-grade
> light-march smoothing, workflow-designed.** A 3-agent workflow
> (mechanism audit + reference survey + fix design) turned the bisect
> verdict into four landed changes, all shader-only: (1) SUN-PROFILE
> MODE - far sun taps (i>=2) read the constructed body with its sub-MFP
> fields at their means (interior turbulence, fine displacement, Worley
> erosion skipped; g_sun_profile flag) - the audit's #1 carrier was the
> interior turbulence field (~4 m content via the frozen g_v2_disp_lod)
> point-sampled by 200-400 m segments, delta_tau 1.5-4 rms = the direct
> coin flip; Nubis3's first-two-taps-per-pixel rule verbatim, and a
> perf win (4 fetches saved per far tap). (2) CELL-FREE tau_vert
> envelope (second ALU-only hinge; the 20.8 m cell voxels were the
> ambient residual). (3) HZD LIGHT CONE - far taps spiral laterally
> (K=0.12 of distance, golden angle, frame-advanced phase): the line
> integral becomes the area integral lateral scattering physically
> performs. (4) NUBIS-2017 RELAXED-BEER floor (0.7*ph_wide*exp(-0.25
> tau)) - deep-shadow contrast capped at 0.25x plain Beer; the one
> LOOK-affecting change (shadow faces lift 2-4x at tau 6-16; tune
> CLOUD_SUN_RELAX down if washed). MEASUREMENT CAVEAT discovered: the
> weather field phase is BOOT-DEPENDENT, so cross-sweep crop metrics
> are content-confounded; only within-boot channel ratios are valid
> (sun/alpha ratio 3.31 -> ~1.6-2.4 across states). RIG WANT (logged):
> a weather-phase pin for deterministic content across boots.
> RESIDUAL + ENDGAME: the irreducible ladder floor scales with ext_km *
> pixel_pitch; if the operator's eyes still read static, the fenced
> architectural answer is the Nubis3 amortized summed-density light
> grid (256x256x32, 8-frame amortization, first-two-taps-per-pixel;
> ~40% march cost SAVINGS + long-range inter-cloud shadows).

> **v0.1252 (2026-08-30): the stipple forensics - alpha is innocent, the
> LIGHTING carries the grain.** Operator (on v0.1251.1, confirming "way
> better" overall): the close-cloud TV-static/sandblast remains the
> target. NEW INSTRUMENT: the screen-path channel bisect (showcase
> map_diag 1/2/3 renders coverage-alpha / direct-sun / ambient as
> grayscale; vantages speck-alpha/sun/amb + motion-closeup). VERDICT:
> alpha grain 0.78 (SMOOTH - the density field is not the carrier),
> direct sun 2.57 (THE carrier), ambient 1.39. Mechanism: alpha
> SATURATES away fine density structure, lighting is LINEAR in it.
> Shipped: variance-adaptive spatial filter in the resolve; ambient
> cavity AO damped (multiple scattering fills crevices); tau_vert from
> the pre-erosion carve envelope (ambient channel 1.39 -> 0.89). IGN
> dither REVERTED - structured error survives a mean filter as a
> halftone weave; jitter spectrum must match the filter kernel.
> REMAINING (the next increment, workflow-assisted): direct-sun
> self-shadow structure at the 22 m light-mfp scale (sigma 45/km, the
> per-pixel tau of the 2-tap sun ladder + the steep octave response) -
> real contrast real clouds smooth via LATERAL multiple scattering.
> Candidates: multi-tap sun cone at coarser lod, lateral diffusion term,
> response softening, sigma-vs-octave rebalance. Full-image same-pose
> grain 2.71 -> 2.57 so far; the operator wants a step change, not 5%.

> **v0.1251 (2026-08-30): spin-aware reprojection + the static's true
> mechanism.** Operator on the v0.1249 exe: clouds "uncanny valley low
> detail... like TV static", atmosphere lower-detail than the surface.
> Both reads were RIGHT. (1) The resolve's motion floor read the planet's
> spin sweep as camera motion (the planet-local delta folds spin into
> translation) and pinned alpha at 0.6+ for every non-co-rotating camera,
> effectively switching the temporal filter OFF - the whole-disc static
> IS the raw jittered march. Now the resolve gets the motion SPLIT
> exactly (f64 CPU chain): content rotation as a rigid spin rotation
> applied per pixel to the hit point + the RAW camera translation; the
> alpha floor keys on reprojection RELIABILITY (real zoom, or slides
> past ~8 texels/frame) instead of raw slide. (2) The cloud layer
> literally renders at fraction-res vs full-res terrain - the composite
> now reconstructs the half-res buffer with Catmull-Rom (same 9-tap the
> map arm used). Identity-fallback preserves old math when the split is
> unavailable. Rig: no regressions across 5 vantages, panics=0, FPS
> flat-to-up (space 19.9). NEXT FIDELITY ITEM (operator's "uncanny"
> verbalized): cumulus interiors read as granular salt-and-pepper
> stipple, not coherent cauliflower lobes - the fine erosion bands carve
> micro-cavities at their noise floor; needs a fidelity-expert pass on
> erosion coherence (NOT a sampling bug; unchanged by this increment).

> **v0.1250 (2026-08-30): ONE RENDERER - the octa map retired.** The
> operator refuted the v0.1249 rosette kill on their machine (third failed
> kill claim on this artifact: v0.1237, v0.1245+, v0.1249 - every rig
> verification passed under rig conditions and failed under theirs), plus
> new damage: DARK gray-blue daytime sky under the deck (the v0.1248
> near-over-map change backdropped stale/aerial map content over the WHOLE
> sky), the hurricane-eye ownership circle at 2.6 km, and a flight-wobble
> report (no flight code changed in 20 releases - the deck swims at 4-5
> FPS and since v0.1243 correctly rolls with attitude; verify, don't
> dismiss). ARCHITECTURE VERDICT: every view the operator has praised is
> the per-pixel near march; every artifact class they hate is the map or
> one of its seams; at disc views the map re-marched ~1M texels/frame -
> the same budget as a half-res screen march spent mostly off-screen. SO:
> near_mix pinned 1.0 (lib.rs), octa_runs pinned false (mod.rs, texture
> stays zeroed), the 32 km screen-march ownership leash removed
> (45-cloud-temporal), the composite's distance-ramp key + near_has gate
> removed (near owns every pixel it touched; empty map backdrop = no-op
> OVER). Map machinery kept dormant in-tree. RISK held to the ladder: the
> round-3 white-veil note said the screen march integrates sub-grid
> structure to featureless white at disc ranges - but the compact-support
> carve hinge has since made clear footprints exactly clear at coarse
> mips; judged on captures, not assumed. Next if veil confirmed:
> fractional-coverage extinction at coarse footprints (the flight-sim
> technique), NOT a map revival.

> **v0.1248 (2026-08-30): NEAR-OVER-MAP + the two-field diagnosis.** The
> operator's 8-shot ladder pinned the disease: the near arm and the map
> render DIFFERENT skies, and the composite's mix() REPLACED map content
> wherever near claimed - every stitch line was an artifact (blue halos
> punching to raw sky at thin near edges, a clear hole under the camera
> ringed by map deck, inverted blobs on the ceiling). Composite is now
> premultiplied NEAR-OVER-MAP: the map is the backdrop everywhere, near
> refines on top, thin near reveals map never sky; cost = bounded double
> density where both drew the same cloud. The v0.1247 sun-drift floor is
> REMOVED (it perpetually re-noised the map = the surviving checkerboard;
> the diff-driven alpha already handles lighting change; resume-drop
> kept). NEW OPERATOR-CONFIRMED FACTS: the ROSETTE is visible from DEEP
> SPACE instantly (map CONTENT bias, not temporal - reproduce: from-space
> park + cloudmap dump, no flight needed); the gravity-well handoff SNAPS
> the view basis (ship well -> Earth well, camera transition bug - OWN
> ITEM); surfacing from underwater visibly changes the sky (the
> resume-drop after underwater octa idle - correct but visible).
> ENDGAME (dedicated increment, discuss with operator): ONE cloud
> representation with continuous LOD - the near arm as REFINEMENT of the
> map's field rather than a second field; every seam class dies at once.

> **v0.1247 hotfix (2026-08-30): dump crash + checker quilt; the rosette
> hunt's state.** The v0.1246 cloudmap dump panicked the operator's live
> session (octa textures lacked COPY_SRC; the first fix hit the wrong
> create site - mod.rs/lib.rs pristine regions are CRLF, edited regions
> LF: use the Edit tool there and grep-verify every instrument after
> writing). The v0.1246 sun-delta invalidation pulsed alpha every ~7 s on
> the 20-minute day = the diagonal checker quilt; now a continuous floor
> capped 0.25. TWO DIAGNOSTIC CYCLES WERE PHANTOMS: "the compositor never
> runs" (the probe had never survived to disk) and "the map is empty"
> (the rig park had silently collapsed from 112 km to 0.3 km before the
> dump - an OLD latent probe-hold drift over 40-60 s, exposed by settle
> 75; suspect the sweep autopilot holding a stale travel target - OWN RIG
> BUG, diagnose before any long-settle forensics). With a held park the
> map dump shows healthy full-disc content; [CloudArm]/[CloudGate]/
> [CloudPasses] 1 Hz instruments are permanent. THE ROSETTE (operator's
> persistent nadir starburst, present at [CloudReproj] 0.000): map
> machinery proven healthy end to end; next discriminator is OPERATOR-
> SIDE - drop debug/cloudmap_request.json while the rosette is on screen
> (safe as of v0.1247); fibres in the dump = content bias (march-side
> hunt: the octa jitter kernel / footprint at the anchor), clean dump =
> sampling side (composite Catmull-Rom / decode).

> **v0.1246 (2026-08-29): the three-part conviction - frozen map, sentinel
> death spiral, over-tight cut.** (1) The octa pass was CPU-skipped at
> near_mix==1.0 (below ~30 km) while the per-pixel composite still showed
> the map in the whole horizon band: the operator's night-bright band WAS
> frozen daylight (proven: every lit march term is ~0 at night). Dispatch
> now ORs regime 3; resume-after-freeze floors EMA to 1; sun-delta
> invalidation exists at all now. (2) PARKED DOES NOT EXIST here: the
> 20-minute day sweeps content 37 km/s past a world-frame hover (5.3
> km/frame at 7 FPS); the LEVEL-triggered sentinel fired every frame
> (cadence suspended -> 16.7M marches -> the 7 FPS itself, self-locking)
> and the v0.1245 6-texel cut amputated accumulation at the sweep's 24
> texels -> per-frame point-sample = the rosette repainted forever.
> Sentinel now EDGE-triggered (spike vs level); cut raised to 48; the
> cadence-skip branch (which advected with NO bound) gets the same cut.
> Frame-locked parks measure [CloudReproj] 0.000 - why the rig was always
> clean. (3) Resolve clip moments bilinear (12-px block plateaus);
> composite key 2x2 weight-blended; under_deck units fixed (was 16.8 km,
> not the slab base - masked by the dispatch skip); rank-1 octa temporal
> jitter -> R2 pair. NEW INSTRUMENTS: debug/cloudmap_request.json dumps
> the octa map itself (rgb|alpha double-wide PNG - the content-vs-
> sampling discriminator), night-horizon standing vantage. DEFERRED:
> grazing iteration-cap tail (obliquity-scaled refine floor at
> 40-clouds.wgsl:2750), extent-rim fade (composite e.z hard cut -> smooth
> + guard-ring march), composite Catmull-Rom has no mip at minification
> (sparkle at orbit).

> **v0.1245 (2026-08-29): flight-smear hard cut + occluded-map cadence gate
> + rig descent knob. READ THE HONESTY NOTE (superseded by v0.1246 above -
> the cut was over-tight and the sentinel analysis incomplete).** The operator's radial
> starburst converges at the MOTION EPIPOLE (they arrive by FTL flight; the
> map history reprojects along the per-frame delta; sustained descent
> displaces every fetch radially about the nadir, and the old floors kept
> 65 percent of it per step). Fixed: hard per-texel history cut past 6
> texels of shift (45-cloud-temporal octa path), map full-rate cadence only
> while the map is the VISIBLE renderer (near_mix rides light7_color.x,
> offset 320 - the in-layer 3 FPS was 16.7M occluded texels marching every
> frame, and low FPS is itself why the accumulator stayed static), and
> camera_request {"descend_mps":N} + the standing descent-live vantage
> (sustained flight on the rig at last; note the frame-lock compensation:
> the ANCHOR must sink with the pinned camera below the co-rotate ceiling).
> HONESTY: the rig's achievable descent (~10 km/s effective) stays under
> the cut's 6-texel threshold, so the A/B could not photograph the
> operator's 30-100-texel regime - the cut shipped on mechanism + safety
> (it only fires where reprojection is geometrically meaningless; parked
> states verified untouched). IF THE OPERATOR STILL SEES FIBRES after
> v0.1245: next suspects, in order - (a) the REGIME-2 full-sphere map
> (inside the slab, extent = pi, anchor zenith: the antipode at the FEET
> has the projection's true stretch singularity - the operator's "dome"
> instinct; consider splitting regime 2 into two hemispheric windows or
> anchoring at the horizon), (b) the near arm's resolve under 3 FPS
> motion (same epipolar logic, cloud_resolve.wgsl has the shift_tx cut
> already - verify its threshold), (c) capture their live run.log DURING
> a sighting ([CloudRegime] + a new shift_tx histogram instrument).

> **v0.1244 (2026-08-29, in verification): the per-pixel regime split - the
> "missing tech" for the sheet-to-ballpit transition.** The operator's
> persistent down-look starburst was the near march's footprint cap
> (min(screen*4, map_texel)) forcing ~5x-above-Nyquist sampling at long
> slant - undersampling moire, radial on a down look; every altitude-band
> move just relocated it (74 km -> 36 km). Both fixed at the root:
> footprint = the near grid's own Nyquist (screen*4 alone), and the global
> altitude crossfade is REPLACED by a per-pixel key: cloud_march_core gains
> g_march_max_km (screen path 34 km - rays abstain pre-step beyond it, far
> end clamped; kills the both-whales 3 FPS), cloud_composite gains binding
> 5 = march_dist and keys each pixel (near owns content < 20 km, map > 32
> km, claim requires drawn alpha so clear foreground cannot blank distant
> banks), near_mix reduced to an arming gate (px ramp x 45..60 km ceiling).
> Handoff is now per CONTENT at matched apparent scale (the MSFS-style
> continuous LOD). Verify on the blend ladder + flower + starburst +
> marble vantages before ship; watch for (a) near/map representation
> disagreement at the 20-32 km seam, (b) thin-near-over-far-sheet pixels
> (accepted edge case, documented in cloud_composite.wgsl).

> **CLOUDS, state as of v0.1239 (2026-08-29).**
>
> **The operator's starburst-at-the-feet: REPRODUCED AND ROOT-CAUSED (the
> flown camera).** Why every fix "did nothing": the rig TELEPORTS (camera ~30 m
> from the ship-frame origin), the operator FLIES, and with no floating-origin
> rebase the whole journey accumulates in the f32 camera.position (~3.6e7 m,
> ulp 4 m). The new ipc knob `far_frame_km` re-splits the same absolute pose
> onto the rig (vantage `starburst-far` = starburst-repro + far_frame_km
> 36000), and it reproduced the operator's artifact on the first try: murky
> whole-frame veil + cardinal speckle cross at the nadir, while the teleported
> twin rendered normal clouds. THE DOMINANT CAUSE was not even in the shaders:
> `dist = render_off.length()` in the celestial draw loop measured the planet
> from the SHIP-FRAME ORIGIN, not the camera - [CloudRegime] read px=280
> mix=0.00 at 4.3 km altitude, i.e. the ORBITAL far-map cloud regime and a
> starved planet LOD rendered from inside the cloud layer. Fixed: dist =
> (render_off - camera.position).length() in f64; heals px, LOD level,
> visual_scale floor, chunk activation, near_mix, and the atmosphere gate at
> once. Two real f32-lattice sites fixed in the same pass: the cloud motion
> delta (was f32 subtraction at 3.6e7 - fed light4, resolve prev_dpos, motion
> gates; now f64 end to end via DVec3 cloud_prev_cam_local) and the resolve's
> big-form reprojection (now small-form normalize(rd*t_w - prev_dpos)).
> `starburst-far` is a STANDING vantage now - any flown-state regression shows
> up on a teleporting rig. The architectural cure for the whole defect class
> is a FLOATING-ORIGIN REBASE (periodically fold camera.position back into
> ship_world_pos); that is the logged follow-up, not this increment.
>
> **The earlier map-path rosette (v0.1237) was real but separate:**
> quasi-periodic jitter hashes + never-jittered map directions; fixed with a
> PCG hash + sub-texel direction jitter. Faint residue is partly the field's
> REAL east-west stagger anisotropy - re-run the parked bisect before chasing
> it as aliasing. The motion-gate fixes (v0.1235 epipole term, v0.1236 motion
> floor) stay, but they were never this artifact.
>
> **v0.1242 (2026-08-29): the melted flower + sphere-atoms, both cured.**
> Operator on v0.1241.1 still saw a crosshair-centred marble/flower (1.9-134
> km, persisting at hover) and "atoms made of spheres". Critic-led workflow
> refuted temporal feedback (hover co-rotates at every reported altitude;
> the clip box is built from a history-free march buffer); the iteration-
> count diagnostic proved the rings are STEP-COUNT ISOLINES - the march
> jittered the sample inside each step but the step LADDER was one
> deterministic comb anchored at m0, so the integer count staircases in
> screen radius and each tread prints a ring on a flat deck. Fixed with
> LADDER-PHASE jitter (first step advances by the jittered fraction) -
> rings gone same-vantage (flower-nadir, now standing). Sphere-atoms:
> the 2026-08-25 eyeball fix had stripped ALL surface erosion from built
> clouds; restored as Nubis-class ONE-SIDED WORLEY EROSION in the DISTANCE
> domain (moves the surface, cannot ring it; 20-160 m octaves from
> cloud_detail_tex, height-phased, edge-proximity strength; stride margin
> grown by the carve). Closeup verdict: carved fractal silhouettes, cost
> neutral. Orbit FPS (operator 5-8): the honest px now runs the near march
> at planetary slant ranges (~200M samples/frame); near_mix gains an
> ALTITUDE FADE (full below 40 km, octa map owns above 80 km; derived from
> cam_r_ratio so the px=280 origin-distance bug cannot return). Also:
> px_hash + g_lod_jitter moved to PCG (last two hash21 users on this path)
> and the CLOUD_V2_FADE_HI handoff dithered.
>
> **Cloud perf follow-up (small):** on fully-built samples (cs.v2 ~ 1) the
> four density-space erosion band taps in 40-clouds.wgsl:1977-2052 are
> computed and discarded - gate them on cs.v2 < 0.999 to reclaim 4 texture
> taps per sample. Deferred from v0.1242 to keep that increment visual-only.
>
> **v0.1243 (2026-08-29): blend-band streaks, roll misregistration, sun
> bleed - fixed; two handoffs logged.** The 74.4 km radial streaks were the
> crossfade band (mix=0.14): the near arm is ~5.5x above Nyquist for the
> field its footprint cap targets at planetary slant (moire = radial
> combing), AND the cloud ray basis ignored camera ROLL and mode
> transitions (origin audit #19: forward()/right() vs the rendered
> rolled_up view matrix - the whole cloud layer twisted about the
> crosshair whenever the camera rolled; all three consumers now extract
> the view-matrix rows). Crossfade lowered to 22..42 km. LONG-TERM
> (better design, critic-endorsed): key the blend PER-PIXEL on the near
> arm's own first-hit dist_km (MRT loc 1) instead of a global altitude
> proxy - fixes down-look AND horizon cases and lets near rays abstain
> (perf) in one move; needs a composite binding for march_dist (bind-
> group discipline: count entries at EVERY create site). Sun-through-
> clouds: god rays sample the SUNWARD deck crossing now (frame_lock::
> sun_cloud_alpha - max of the pinned procedural field via cloud_
> reference::weather_pinned_field and the live grid) and the type-17
> disc/halo intensities scale by exp(-4a) per frame. STILL OPEN, handed
> to the ABYSSAL rung-2b owner (their lane files): the ocean sun-
> specular ignores clouds - multiply (1 - 0.9*ca) into sun_shadow_f at
> 90-fragment-main.wgsl:1697 (type-12 glint, hardcoded 1.0), :1670
> (old_glint), :478 and :746 (type-16), using the v0.898 ground-shadow
> pattern at :1608-1617 but at the fragment-to-sun deck crossing
> (r = sea radius * CLOUD_SHELL_SCALE). ALSO LOGGED (own increment):
> the aboard-station frame family from the origin audit - celestial/
> godray sun direction not hull-rotated (#17, lib.rs:17762 area), body/
> cloud/atmo rotation spin-only (#18, lib.rs:8948), orbit rings world-
> pinned (#27) - lighting rotates off the visible sun as the hull turns.
>
> **RE-SCOPED after v0.1243: map clipping mostly healed; one residual.**
> The razor-wall clipping was the twisted ray basis (audit #19, fixed) -
> the mismatched rays fell outside the map extent. AFTER the fix,
> marble-inertial (sweep 20260829-071407) shows natural weather-field
> edges everywhere except ONE small triangular cloud fragment with two
> straight edges mid-frame - the signature of an octa-FOLD seam leak
> (reflected/clipped content where map taps cross the octahedral fold
> without proper wrap: Catmull-Rom taps or the sub-texel direction
> jitter). Diagnose with a map-UV/seam visualization at that vantage
> before tuning.
>
> **Old text for context (superseded):** Evidence: marble-inertial capture, sweep
> 20260829-060201 - at 112 km with mix=0.00 (the new altitude fade), cloud
> patches render coherent but end in hard straight edges, all facing the
> same direction. The 12c extent controller's regime-1 window (asin(rt/c)
> + 4 deg + drift, ~92 deg at 112 km) SHOULD cover the disc, so the cut is
> not obviously the cone rim - diagnose with a map-texel visualization
> before tuning anything. This corner (map as SOLE renderer at 40-200 km)
> was never exercised before: pre-v0.1239 the px bug gave it wrong extents,
> post-v0.1239 the near march always covered it. The operator hovers in
> exactly this band.
>
> **Swiss-cheese sheets: fixed.** The v0.1234 union scaled the field density by
> sheet_w, pushing it under the visibility threshold at partial coverage - holes.
> Now mix(built, max(built, body), sheet_w): the sheet keeps full density, the
> UNION is what fades.
>
> **STILL OPEN, the last big look item: clouds read as sphere clusters up
> close.** Placement, coverage, detail mips, temporal artifacts are all fixed or
> gated - the remaining problem is the LOBE SHAPE itself. Next levers, in order:
> (1) stronger domain warp relative to lobe radius (CLOUD_V2_WARP_FRAC 0.42,
> tile 1.7r - try 0.6/1.3 with an A/B on cumulus-closeup-ultra); (2) flatten the
> lobe primitive into a base-weighted ellipsoid so buds read as risen dough
> rather than marbles; (3) only then relight (the smin normal groundwork from
> v0.1232.2 is computed and unused).

> **ACTIVE (parallel lane): THE ABYSSAL ADOPTION ARC (operator, 2026-08-28:
> "Let's do all of it").** Full technical menu, constants and porting cautions:
> `docs/reference/abyssal-ocean-weather.md` (commit 942f1921; source repo
> github.com/Token-Gremlin/natural-disasters, MIT). This arc runs in the
> `ocean-weather` lane (`data/coordination/lanes.json`, carved out of engine)
> so it never collides with the cloud session's files (40/41/45-clouds.wgsl,
> `src/renderer/cloud_*.rs`). Rungs, STRICT order, each verified before the
> next; per-event tuning numbers go in `data/weather/events.ron`
> (infinite-of-x), never hardcoded:
>
> 1. **DONE v0.1238.0 - Ocean event field core (CPU, f64)**:
>    `src/terrain/ocean_events.rs` - the four analytic disaster fields
>    (tsunami soliton with asymmetric shoaling + drawdown, rogue Gerstner
>    group, Rankine vortex + swirl-coord rotation, hurricane eyewall ring +
>    glassy eye) in event-local tangent frames, vec4 uniform packing, 18
>    tests pinning every adopted constant.
> 2. **GEOMETRY HALF DONE v0.1240.0 - WGSL twin + geometry**: event height
>    displaces the drawn sea (CameraUniforms 14-row tail block at offset
>    672, layout-test-pinned); buoyancy rides it (rogue = envelope only);
>    WGSL constant-scanner lockstep test; dev pin showcase
>    {"ocean_event":kind, ocean_event_bearing, ocean_event_distance};
>    probe-proven with close-range captures (tsunami ridge + drawdown,
>    maelstrom bowl; hurricane confirms the pow-negative-base WGSL fix).
>    SHADING DONE v0.1241.0 (rung 2b): tsunami breaking-lip foam
>    (lacework-carved, face-biased), saturating vortex shear foam, rogue
>    crest band, hurricane glassy-eye chop+foam suppression - all analytic
>    at the fragment's planet-model position, no new varyings, buoyancy
>    untouched. REMAINING (rung 2c): swirl-coord advection of the wave
>    lookup - touches the HEIGHT path, so it needs the CPU twin + lockstep
>    extension and the swirl clock (ocean_event row 12.w, reserved). Note
>    for 2c+3: dev-pin amplitudes are clamped to the +-12 m patch band
>    (MAX_SEA_HEIGHT_M); full 34 m walls + full-strength maelstrom foam
>    (the 0.62 shear cap engages near strength 34) need the lifecycle to
>    publish dynamic patch bounds.
> 3. **Lifecycle + gameplay**: event params data-driven in
>    `data/weather/events.ron`; spawn/ramp/decay through `weather_events.rs`;
>    REGISTER `DisasterSystem` (written, never registered); damage + HUD; the
>    float clamp rides the wall via the rung-2 twin.
> 4. **Lightning**: CPU midpoint-displacement bolts (forks p=0.42, return
>    strokes amp 0.62^i with flicker) + instanced ribbon draw; two strongest
>    bolts flash sea/cloud/sky through shared light slots; thunderstorm event
>    emits. (No weather audio exists engine-wide; thunder logged, not built.)
> 5. **Waterspout / tornado funnel**: raymarched analytic funnel on the Vortex
>    event core (rotating-frame detail noise, dual-HG forward phase).
> 6. **Foam v2** in `ocean_fft.rs`: add the steepness criterion (Stokes H/L
>    1/7 + leeward bias + crest gate - catches the spilling breakers the
>    Jacobian misses), bubbles channel; re-verify the Monahan histogram.
> 7. **Crest spray**: `particles_gpu` spawn-from-breaking (candidates roll
>    against the FFT foam/crest field), forward-scatter puff shading.
> 8. **Rain overhaul**: closed-form vertex-shader rain, sub-pixel streak
>    energy conservation (vThin), fbm squall curtains, rain rings on water.
> 9. **Water shading**: backlit crest SSS with the event-thinness gate,
>    mss-to-roughness LOD, sun-disc-widened glint, wind-frame Langmuir foam.
> 10. **Cloud env probe in water reflections** - CROSSOVER rung, touches cloud
>     files; schedule WITH the cloud session when its current arc lands.
> 11. **GPU compute FFT + horizontal chop** (the long-planned water-fft
>     increment 4, using the 8-fields-in-4-complex-IFFTs layout as reference).
>     Riskiest renderer change, deliberately last.
>
> Cloud cherry-picks (erosion bite curves, local-density powder alternative,
> per-pixel-depth reprojection) belong to the CLOUD session's own plan, not
> this arc - they are listed in the reference doc Tier 3 for it to read.

> **CLOUD COVERAGE: RESOLVED (v0.1234).** Asked 0.95, delivered 1.00 from nadir;
> `node scripts/cloud-coverage-metrics.mjs <sweep>` over `overcast-nadir-ultra`
> PASSES. The winning mechanism was the SHEET UNION in cloud_carve: overcast is
> a continuous stratiform layer, so past sky-wide coverage 0.6 the noise field
> is unioned back under the per-cell constructed clusters until the sky closes
> near 0.9. Gate it on the GLOBAL coverage (material.base_color.a), never the
> local weather alpha - the local value is 1.0 inside any cloud at any coverage
> and closes the whole sky. Growth is capped at 1.35x and the smooth-min blend
> radius at 300 m (uncapped, coverage-grown clouds were giant melted-wax blobs).
>
> **THE DETAIL MIP IS PER SAMPLE (v0.1234), keep it that way.** The per-ray
> freeze at the segment midpoint surfaced a cloud 500 m away at the mip of a
> point 300 km downrange - the NEAREST clouds were the smoothest. g_v2_disp_lod
> is now set from each view sample's own footprint just before the density
> call; the eight sun taps that follow reuse it, which is the eye/sun surface
> consistency the freeze existed for.
>
> **THE SHELF (base-tangent seam): visually gone, numerically open.** Repro
> found: inside the layer ~3 km, cover 0.55, level view - the re-authored
> `base-horizon-seam-{3p0,3p7}km` vantages. The gate still measures a 1.93x
> detail step (threshold 1.25) and stays RED rather than being tuned to pass;
> part of the step is genuine depth (crowded translucent cloud above the line
> reads softer), part may remain artifact. Judge against the capture, not the
> ratio alone, before spending on it again.

> **MEASUREMENT LESSONS from this arc. Four wrong answers were produced
> confidently before being caught; each is now enforced in a script.**
>
> 1. **Areal coverage must be measured from NADIR.** From a grazing camera the
>    same A/B read 46% with and without a change, because a sparse field fills
>    the frame near the horizon. `cloud-coverage-metrics.mjs` refuses to score a
>    vantage whose `look_offset_deg` is not 0.
> 2. **Classify cloud on SATURATION, not brightness.** Calibrated on real
>    pixels: cloud 0.05-0.07, sand 0.36, sky 0.39 - while luminance overlaps all
>    three (cloud 163-224, sand 191, sky 170). A brightness threshold reported
>    86% cloud in a frame that was mostly sand.
> 3. **A gate whose subject is not in frame is not a gate.** The first seam
>    vantages had no cloud at the seam row and reported before and after
>    identical. The guard added to catch that was then itself fooled by pale
>    horizon haze passing the cloud-fraction test, producing a confident 6.6x
>    "detail step" that was haze against cloud. Looking at the image caught it;
>    the number never would have. Both guards now sit in
>    `cloud-seam-metrics.mjs`, which currently REFUSES to score and exits 2.
> 4. **Do not judge across non-adjacent captures by eye.** A change that
>    "clearly" enlarged the clouds measured at 6.6% of pixels differing - the
>    comparison being made was against a remembered older frame, not the actual
>    previous one.

> **WORKFLOW: WGSL edits need NO cargo build.** The megashader assembles from
> on-disk parts when they exist (`shader_loader::assembled_pbr_source_from_dir`;
> the embedded copy is the stripped-install fallback) and probe-sweep junctions
> the repo `assets/` into the rig. Edit the shader, run the sweep. A whole
> session was spent paying four-minute rebuilds per shader iteration.

> **SHELL: use a QUOTED heredoc for commit messages.** `<<MSGEOF` interpolates,
> so backticks in prose get command-substituted and words vanish from the
> message (v0.1232.6 lost the word it was defining). Write the message with
> `<<'MSGEOF'` and put the version in with sed afterwards.


> **CLOUDS: THE THREE OPEN ITEMS (v0.1232.3).** Everything below was measured,
> not guessed. Read the findings before reattempting either fix - both are
> written and deliberately switched off, so the work is not the code.
>
> **1. Snowflakes from orbit.** Operator: "a ton of white dots appear
> everywhere... like snow flakes." Cause is understood: the v0.1230 power-law
> made most clouds a few hundred metres, so from orbit each lands on about one
> pixel, and a sub-pixel bright object cannot be filtered - only twinkled. The
> fix shape is right (fade the built body back to the noise body across a 250 m
> to 1 km footprint window, a mip fade) and IS WRITTEN in
> `CLOUD_V2_FADE_LO/HI`, currently neutralised at 1.9/2.0.
>
> **BLOCKER, and this is the real task: the two body models are not
> brightness-matched.** The noise body renders darker, so fading toward it with
> distance darkened the far half of every frame. Measured at
> `cumulus-closeup-ultra`, mean grey over a fixed crop: **191.1 fade off, 157.4
> fade on**, and still 157 with the shading term that was first blamed removed
> entirely. Match the two bodies at the handover, THEN reopen the window. Do
> not tune the window; it is not the window.
>
> **2. Clouds read as opaque fluff, not cloud.** The smooth-min normal is now
> COMPUTED and nearly free (the smin already derives the blend factor h that
> combines the distances; the same h combines the normals - one normalize per
> lobe, no extra field evaluations, against three to six full re-evaluations
> for finite differences). It yields the sky-facing cosine and the seam
> strength 4h(1-h) that peaks in the crevices between buds.
>
> **BLOCKER: wiring it into `ao` turned every cloud into a dark silhouette**,
> and three retunes failed to recover. The diagnosis: the normal is only
> meaningful within a rind of the surface - deep inside a body the gradient
> direction is arbitrary, and interior samples carry most of the accumulated
> weight, so an occlusion built from it is applied hardest exactly where it
> means least. Next attempt: weight by surface proximity (`g_v2_sdf_m` is
> already published) and apply to the AMBIENT only, never to direct - a
> sky-view term is by definition about the sky.
>
> **3. The horizon seam.** Operator screenshots at 5.7 and 6.2 km show a hard
> horizontal line across the frame with visibly different cloud rendering above
> and below it, plus one cloud "indented on the right side". NOT yet
> investigated. The standing theory from the earlier arc is a uniformly-capped
> slab top; the new evidence suggests looking instead at the cloud BASE shell
> horizon, which from 6 km sits about 280 km away and projects as a near-
> straight line at exactly that screen height, and at what changes
> discontinuously in the marched segment as a ray stops intersecting the lower
> slab boundary.
>
> **Method note for this arc.** Perf on this rig has ~1.6x run-to-run variance
> (the same unchanged frame measured 37 ms once and ~60 ms three times), so a
> single sample is not evidence. Take repeated or back-to-back measurements
> before quoting a number; the 137.7-vs-59.8 jitter comparison is trustworthy
> because it was back-to-back.


> **ACTIVE: THE CLOUD PLAN (from the v0.1228 decision).** The operator, out of
> patience: "I am really tired of seeing these spheres with zero transparency
> and TV static effect... I don't get why we can't get rid of this."
>
> **SHIPPED v0.1228.0 (increments 0 + 1).** Ultra never survived a restart (the
> loader whitelist omitted it, and the next save overwrote the choice), so every
> recent cloud change was reviewed on a renderer the operator's game reverted,
> and what they were describing was the older noise path. And the near-field
> temporal denoiser accelerated its own blend rate in proportion to
> disagreement with no motion gate - positive feedback against noise, so it
> switched itself off at exactly the pixels it existed to fix, even at rest.
> Both fixed, both zero frame cost.
>
> **NEXT, in order. Do not reorder without reading why.**
>
> **Inc 2. Make the sun see the surface the eye sees.** The comment at
> `41-cloud-bodies.wgsl:351-360` claims all eight sun-shadow taps sample the
> displaced surface. They do not: the displacement mip comes from the caller's
> `lodb`, and `cloud_sun_tau` passes a different `lod_t` per tap
> (`40-clouds.wgsl:2029`), so near taps land where the displacement is gone.
> The silhouette is bumpy while the LIGHTING still shades a smooth sphere.
> Very likely why the v0.1221 displacement work "did nothing". ~0 ms.
>
> **Inc 3. Kill the coin flip: step by distance, not a fixed hop.** Clear air is
> marched in 495 m hops (`slab_h * CLOUD_STEP_BAND_FRAC`, 11 km slab x 0.045)
> while the cloud edge it is hunting is 90 m thick, so every silhouette pixel is
> a per-frame coin flip. Note the v0.1218 refinement `max(seg/16, 30 m)` does
> NOT help from the ground: `seg` is the whole slab crossing, so seg/16 = 690 m
> and the `min` always picks 495 m. It was verified from inside the deck, where
> it does work. `cv2_cloud_sdf` already returns a real distance in metres and we
> throw it away - use it, in the hoisted per-cell form
> (`environment-program.md:769-779`), NOT the naive per-sample form that gave
> 4 fps at v0.1210. Gate at `gpu.cloud_screen <= 4 ms`, abandon if it misses.
>
> **Inc 4. Demote the sphere from surface to envelope.** This is the "spheres
> with zero transparency" complaint. Density is `clamp(-best / 90 m, 0, 1)`, a
> linear ramp off a distance field, which leaves 79-93% of every lobe at
> constant full opacity with only a 90 m soft shell - measured per archetype.
> Displacement is 9-26% of lobe radius, far too small to disguise a sphere. The
> 14-lobe cluster should decide WHERE a cloud is and its proportions (its flat
> condensation base is genuinely good, keep it) while multi-octave noise carves
> the surface, with a vector domain warp applied BEFORE the lobe reduction so
> shapes can fold and overhang instead of merely getting bumpy. Same increment
> fixes plain defects found in the audit: cloud width is drawn UNIFORMLY
> (`:134`) and should follow a power law; lobe count is hardcoded to 14 (`:46`)
> against 6-48 in `data/clouds/archetypes.ron`; `cv2_arch_index` (`:152-162`)
> can never return 1, so **cumulus congestus has never once been rendered**;
> and placement is not wind-advected while the coverage gating it is, so clouds
> pop in fully formed instead of fading.
>
> **Inc 5. Fix the interior and the light.** Shade on the analytic smooth-min
> normal (the lobe loop can accumulate it free; named "the designed cure" in
> four journal entries and never built). Restore crown as sky-view and pouch as
> a crevice mask. Delete Beer-powder (`CLOUD_POWDER_STRENGTH = 0.92`): droplets
> scatter essentially all the light they receive, so a cloud edge physically
> cannot be darker than the sky behind it, and ours measured 0.71x. Add the
> adiabatic vertical water gradient and a turbulent interior field at 50-500 m.
>
> **Explicitly NOT the plan: a voxel-atlas / full Nubis-3 rebuild.** More famous,
> but this project has no artists and no offline fluid pipeline, and
> `environment-program.md:775` already names it the fallback rather than the plan.
>
> **Clouds should NOT become particles.** Camera-facing cards are right for a
> bounded, short-lived puff (a smoke grenade) and wrong for a deck to the
> horizon: ~10,000 km2 of cloud is a quarter-million sorted blended quads, they
> lose parallax the moment you fly into them (the "oriented to me" complaint,
> already fixed once), and they cannot report absorption along a ray, which is
> what dims the sun disc and casts cloud shadow on the ground. The 2030 version
> of a smoke grenade is a small dense voxel grid marched against the SAME
> scattering model - so if we ever want one, feed a local volume into the cloud
> march rather than building a second billboard system beside it.


> **LESSON (v0.1227): counting frame-conversion sites is not the same as
> finding them.** v0.1225 converted the world into the station hull frame and
> the comments proudly labelled the consumers "site 1 of 3" through "3 of 3".
> There were four. `src/renderer/stars.rs` builds its sky rotation from the
> camera forward/up, which while riding are hull-frame vectors, so the sun and
> Earth swept past correctly and the stars stayed nailed to the deck. The
> release note even asserted the opposite.
>
> What would have caught it: the missed site had no textual link to the
> others - it does not mention the station, the hull, or the frame, so no grep
> for those words finds it. The reliable sweep is to enumerate every consumer
> of a CAMERA-derived direction or its own camera uniform, not every mention of
> the frame. When converting a frame, list the renderer subsystems that hold
> their own view matrix (stars, godrays, particles, any post pass) and rule
> each in or out explicitly.
>
> Still unconverted, and known: moon fill and godrays (deferred, see the
> v0.1225 list below).

> **ATTRIBUTIONS ARE NOW A SURFACE (v0.1227).** `data/credits.ron` +
> Settings > Credits + `LICENSES.md`. **Adding any real-world data source
> means adding a row in the same commit** - `src/credits.rs` has a test that
> fails if a source marked `attribution_required` names no surface showing its
> notice. OpenStreetMap is the one with teeth: ODbL treats the RENDERED view
> as a Produced Work needing a visible notice wherever it is drawn (Maps
> footer + in-world HUD line, both from `credits::OSM_NOTICE`), AND the region
> files as a Derivative Database that must itself be offered under ODbL. A
> credit in the repo alone does not discharge the first.


> **SHIPPED v0.1225.0: the homestead gets a day.** The station now propagates
> real Keplerian elements from `data/stations/home.ron` on the GAME clock, with
> a nadir-pointing (LVLH) attitude. Full reasoning in the release message and
> `src/station/orbit.rs`. The one number worth carrying forward: **orbital
> position cannot light anything** - the sun is 1 AU away, so a whole
> synchronous orbit moves the sun direction by 0.03 degrees. Only attitude can.
> Gate: `scripts/home-clock-metrics.mjs` over the `home-clock-*` vantage trio.
>
> **NOTICED WHILE VERIFYING, not yet chased: solar generation aboard looks
> inverted.** In the A/B captures the HUD read `gen 1948W` at local midnight
> aboard and `gen 150W` at local noon aboard. That is the wrong way round, and
> the likely cause is that `SolarSystem` scales panel output by the GLOBAL
> game hour (a lon-0 ground site) while the station now has its own sun angle
> from its attitude. The two clocks disagreed before this change too; the
> change just made it visible. The fix is presumably to drive aboard-station
> panels from the same hull sun vector the renderer uses
> (`station_world_rot.inverse() * sun_dir`) rather than from the wall-clock
> hour. Verify the inversion first - it was read off a HUD in two captures
> taken at different day counts, which is suggestive, not proof.
>
> **Deferred from this increment, in order:**
> 1. **Eclipse / umbra for hull geometry.** The homestead never enters Earth's
>    shadow. `sun_gate` is hardcoded 1.0 for everything but the planet-surface
>    branch (`90-fragment-main.wgsl`), and `lit_uniform`
>    (`renderer/mod.rs:2235-2237`) stamps a flat 2.5 intensity over the
>    celestial pass's day-gated value. Night aboard is currently "the sun is
>    behind the hull", not "the planet is between us and the sun". Both are
>    needed for a real orbital night.
> 2. **Moon fill and godrays through the hull frame.** Three sites were
>    converted (celestial `render_off`, `sun_dir`, local up); these two were
>    not, so they still reason in world axes aboard.
> 3. **Rotation-aware particle rebase** (`lib.rs` floating-origin rebase
>    handles translation only).
> 4. **The cosmos ephemeris is still on `SystemTime::now()`.** Same class of
>    bug as the one just fixed, one level up: the planets' own positions do
>    not follow the game clock either. Nobody has reported it because the
>    drift is slow, but the hour slider does not move them.
> 5. **Rate-limited attitude slew.** Changing attitude mode re-points the
>    station instantly; a real one would slew on RCS over minutes.
> 6. **Web mirror of the station card** - orbit, attitude and next sunrise read
>    from the same `data/stations/home.ron`.


> **OPEN (2026-08-26, from live play on v0.1223.1) - two operator reports,
> neither reproduced yet. Read this before re-deriving either.**
>
> **1. "Glowing ocean" at night.** Screenshot at 01:05 local: a large soft
> pale-cyan mass over an otherwise correctly dark planet, with dark holes in
> it and two small isolated cells nearby. Their run.log for that session puts
> the camera at **alt=399.8 km with `[CloudRegime] mix=1.00`**, i.e. the NEAR
> screen-march cloud regime at full strength. So despite the name, the prime
> suspect is CLOUD, not water: `CLOUD_NIGHT_FLOOR = 0.006` is added UNGATED at
> three sites in 40-clouds.wgsl (the `day` factor multiplies the sun and
> ambient terms but not the floor), which on the night side leaves every cloud
> sample at a flat `base_color * 0.006` - a shadeless pale mass following the
> coverage field, holes and all. NEXT STEP: an orbital night vantage over a
> cloudy region at ~400 km, then A/B the floor at 0.
>
> **RULED OUT: the water path.** Chased first and disproven, do not redo it.
> The real defect found there is genuine but is NOT this: the sky-view LUT
> cannot represent a deep-night sun at all - its sun-elevation axis spans
> `mu_s in [-0.15, 1.0]` and both samplers clamp below that (atmo_luts
> `u_to_mu`, and the `(mu_s + 0.15) / 1.15` clamps in sky_view_lut.wgsl), so
> past about 8.6 degrees under the horizon it keeps returning civil-twilight
> radiance. The drawn sky and the celestial pass both multiply that away with
> `celestial_sun_day`; the two consumers of `water_sky_lut` (the water mirror
> and `sky_ambient`) never did. A day-gated version was built and A/B captured
> at ocean-night-glow: **both the gated and ungated builds render the night sea
> black**, so at 150 m this changes nothing visible and cannot be the report.
> It was REVERTED rather than shipped, because gating `sky_ambient` removes the
> night ambient fill from all terrain and props and would darken night scenes
> while a black-screen report (below) is open. Worth doing later on its own
> merits, scoped to the water mirror only, with its own evidence.
>
> The A/B pair is kept as `ocean-night-glow` + `ocean-noon-control` in
> vantages.json. They are a matched pair on purpose: night alone cannot fail
> honestly, since a gate that zeroed the mirror at every hour would also pass
> it. The noon twin is what proves a gate is time-dependent rather than off.
>
> **2. "Solid black floor, no homestead, no Earth/planets."** Reported
> immediately after the ocean report, same v0.1223.1 session. NOT reproduced
> and not diagnosable from disk: their run.log ends in a clean shutdown at
> 07:12:59 with no later boot, so there is no failing process or crash trace to
> read. The session it belongs to was at 400 km orbit with terrain and water
> both drawing (`[WaterDiag] draws=1024 covered=true`). Needs a repro from the
> operator: which build, and what they did just before it went black.


> **ACTIVE (2026-08-21): the ENVIRONMENT PROGRAM** - the council plan of
> record at docs/design/environment-program.md, executed serially by rank.
> Done through v0.1184.0: increments 7 (ocean specular AA), 8 (reference
> arbiter + joint gate), 9 (sampling law), 10a/10b (integrator + field
> polarity), 11a/11b (fades deleted + weather fractions), THE MIRROR BUG
> (v0.1183), and 12c SLICE A (extent-parametrized temporal map, resample
> re-anchors, arm everywhere, atmo-order fix - A/B-proven vs a v0.1183.1
> control; adversarial review's 5 findings fixed pre-ship).
> **SHIPPED v0.1186.0 (slice B): translation reprojection** - the
> operator's motion smear ("solitaire artifact") killed via per-texel
> history reprojection with an analytic shell-sphere parallax distance,
> a PLANET-LOCAL motion baseline (the world-frame one slides 1.3-2.1 km
> per frame at a PARKED camera - measured), a >15 deg teleport guard,
> and motion-adaptive blend. Parked captures crisp, delta ~0 at rest;
> the MOTION verdict is the operator's. Rig unblocked: unconditional
> re-park + 6 s settle in probe-sweep.js (first with-time request of a
> boot lands ~8 h early; engine ordering fix owed, chip task).
>
> **SHIPPED v0.1198.0: THE VANISH ROOT CAUSE + the 12d two-regime
> architecture.** The vanish was NOT the wx-floor handoff: cloud_carve
> divided by (1 - thr) while the body tops at CLOUD_BODY_TOP = 0.79,
> capping cores at carve ~0.68 (typical 0.2-0.4); the four erosion
> bands - calibrated for carve-1 cores - ground the entire from-below
> deck to ZERO (stage forensics at pinned coverage 1.0: pre-erosion
> carve max 0.23, post-erosion 0.000 on every sky ray). Fix: divide by
> (CLOUD_BODY_TOP - thr), the contract the constant's own comment
> documents. Plus 12d: NEAR regime (>= 1000 px) = half-res fullscreen
> per-pixel march with analytic pad-basis rays + screen reprojection
> (no direction cache -> the whole solitaire/ghost family structurally
> impossible near the planet); FAR keeps the octa map. Verified across
> 7 vantages, panics=0; cov100-underdeck is the permanent regression
> gate (coverage 1.0 = no legitimate gap, any blue zenith = defect).
>
> **SHIPPED v0.1199.0 (12e): the march/resolve split** - the operator's
> first-flight verdict on 12d ("still ghosting... way faster to
> disappear but still present" + "clouds look a lot like static, best
> on the cliff-like edge") traced to 12d's single blend constant, which
> cannot both converge the jittered march and kill stale history. 12e =
> quarter-res full-rate subpixel-jittered march + a standalone resolve
> with VARIANCE-CLIPPED reprojected history at base alpha 0.12: ghosts
> snap in one frame, static converges ~8 frames deep. Measured hf-noise
> -69% under-deck / -53% mid-alt at unchanged march cost. Adversarial
> review caught + fixed pre-ship: the history-drop flag was coupled to
> the octa cadence sentinel (~8-11 m/frame) and would have dropped
> accumulation every frame of ordinary fast flight; it now fires only
> on true teleports (delta > 0.25 x slab distance).
>
> **SHIPPED v0.1201.0 (12f): cloud underside relief.** The flat
> ceiling was arithmetic (every lighting term saturated at overcast
> tau + a constant warm bounce at 57-63% of base radiance, chroma sign
> inverted). Landed: LWP mottle field (solidity-gated density
> multiplier from existing taps), transmittance-scaled near-neutral
> bounce, vertical-tau split for the diffusion floor + CIE solar term,
> pouch shading. All four executable gates pass
> (scripts/cloud-underside-metrics.mjs): mottle 1.26x -> 1.91x, chroma
> sign corrected, gradation preserved, coverage unbroken. Tried and
> REVERTED: thr -> 0 at cov 1.0 (fills the slab vertically - scud).
>
> **SHIPPED v0.1208.0: procedural placement default + continuous mip
> dither; THE WHITE-CONTINENT CASE CLOSED** - it was a continent-sized
> STRATUS cell in the planet-fixed cloud-family field, frozen over
> North America because every rig boot re-seeds the same game minute.
> Every path A/B had compared different REGIONS. Rig-methodology
> lessons journaled (v0.1208 entry): pin the cloud TYPE when hunting
> coverage differences; same-region or it proves nothing.
>
> **SHIPPED v0.1214.0: THE APPROACH VANISH, ROOT-CAUSED AND CLOSED.**
> The atmosphere DOME was painting over the composited deck. The 12c
> order rule composited clouds BEFORE the transparent pass whenever the
> camera sat outside the atmosphere; over the disc that dome alpha is
> near-opaque, so it erased them. Measured: the composite wrote 1.2% of
> the disc under the old order, 99.9% under the new one, with every
> discard sentinel reading ZERO (drawn, then overdrawn). Ladder is now
> smooth 20,000 km -> 900 km (was a 60.9% -> 8.8% cliff). Physically
> correct too: the march already applies aerial perspective, so the old
> order applied the air column twice. It masqueraded as a terrain bug
> because the cliff sat exactly at the chunked-terrain trigger (1.5
> planet radii), which flips the shell lists and hence this ordering.
> ALSO: the Ultra eyeball rings - the v2 body is a DISTANCE field, so a
> footprint-derived rind is a metric radius and the 8-tap sun ladder
> shaded eight shrunken copies of each lobe; the rind is now frozen
> once per ray.
>
> **NEXT (TOP): operator verdict on v0.1214.1** (approach continuity
> orbit-to-ground, Ultra lobes ring-free), then by rank:
> 0. THE HORIZON LINE (operator, still open): a dead-straight line
>    through the whole cloud layer seen from inside/above the deck,
>    gone once underneath. Needs a NATURAL-weather repro - pinning
>    coverage 1.0 just buries the camera in dense cloud. Side finding
>    deserving its own item: inside a pinned-100% deck at noon the view
>    renders near-BLACK, where real in-cloud is bright white fog.
> **NEXT (TOP): operator verdict on v0.1208.1** (procedural-only sky
> feel, static-square death, approach continuity), then by rank:
> 1. THE SPHERE-BALL LOOK + per-lobe shelving (operator: "obviously
>    all balls/spheres... decimated spheres" + "weird shelving") - the
>    puff/cell lobe construction reads as uniform ball packs at close
>    range and the march's step ladder terraces each lobe. This is the
>    Ultra v2 constructed-body track (increments 14/15) pulled forward:
>    real cloud bodies are not sphere unions.
> 2. Stratus mesoscale structure: a 3,000 km featureless stratus sheet
>    is family-correct but visually dead - the increment-15 statistical
>    far field gives sheets their real broken texture.
> 3. Deck underside polish continues (chroma gate flaked warm under
>    changed weather - re-measure under pinned weather before chasing).
> 4. Black horizon hairline (chord-sag suspect), water F6/F4 A/Bs,
>    stars-below-cloud-top, cloud streets (13).
>
> (Superseded round, kept for the record: THE FAR/NEAR HANDOFF POP -
> with a live contradiction that turned out to be the regime field.) The operator's orbit-approach "huge patch of clouds
> just vanishes" is the regime switch (analyst: 9.4x footprint jump =
> +3.2 mips in ONE frame, carve compensator saturated at its 0.02 cap).
> BUT the derived fix (near march footprint = min(screen*4,
> cloud_pix_ang_map())) produced a WHITE CONTINENT at 4,500-6,700 km
> while the OCTA at a near-identical mip (2.8 vs 2.3) renders ~45%
> areal: HALF A MIP CANNOT DOUBLE COVERAGE, so footprint is NOT the
> whole octa-vs-near difference (v0.1204 journal has the full data).
> Suspects for the residual difference: the composite's Catmull-Rom of
> a 4096 map at a ~250 px disc (severe minification averaging), the 12e
> resolve's variance clip, the weather wlod delta, the octa's 2x
> spatial supersample. STEP 0 (instrument first): a 1 Hz [CloudRegime]
> log line in lib.rs printing px + near + altitude - THREE sweeps were
> confounded this round by guessing which regime a park ran.
> 2. Overcast-completion veil: engine coverage 1.0 = thr floor 0.347 =
>    ~87% areal; at >= 0.95 coverage add a thin base-level stratus veil
>    that closes the sky AREALLY without filling the slab vertically
>    (the round-5 scud lesson; gate4 recalibrated to 18% meanwhile).
> 3. Black horizon hairline (atmosphere shell chord-sag suspect), water
>    F6/F4 A/Bs, stars-below-cloud-top, cloud streets (13), 14/15.
> Operator watch items: 12e ghosting residue (RESOLVE_CLIP_GAMMA toward
> 0.75), 12f underside taste (LWP mix range; pouch 0.72), the new 1 km
> base feel, edge stipple on cloud silhouettes (march jitter at
> boundaries - if reported, widen the resolve's neighbourhood or add a
> spatial post-filter).
>
> (Superseded live-path protocol, kept for the confound catalog: the
> haze-corrected clouds-on/off design in journal 2026-08-22 - the
> wx-floor fade suspect is now largely moot post-carve-fix but the
> protocol remains the right instrument for any future live-vs-pinned
> coverage dispute.)
>
> (Superseded round below, kept for the confound catalog:)
> **SAME-REGION coverage-invariance measurement.** The
> night's final re-adjudication (environment-program.md 12c, READ IT
> FIRST - it names four instrument confounds that each looked like a
> rendering defect): the 15%/80%/60% triplet spread is dominated by
> WINDOW-SIZE SAMPLING VARIANCE of the ~2000 km-cell pinned field, and
> nothing yet cleanly demonstrates a rendering non-invariance. The
> clean protocol: one boot, orbit + nadir captures within a minute,
> CROP the orbit frame to the nadir frame's exact ground region,
> compare areal coverage in the crop (same field, same moment, same
> region, two footprints); several offsets for the under-deck rung.
> Only a spread surviving THAT is worth hunting. PROVEN NOT A TERM:
> the carve width table (three tables render identically). Rig debt
> blocking pinned captures: a showcase_request carrying cloud_cover
> resets the game clock ~8 h (chip task filed with the repro; send
> the camera request AFTER any showcase pin as the workaround).
> After that: 12c slice B (RG16F first_t + parallax-corrected history),
> descent-ladder re-run on the fixed map, then increments 13+ by rank.
> The ladder gates increments: node scripts/probe-sweep.js --ladder then
> scripts/ladder-score.mjs.
>
> Logged debt: cumulonimbus width capped at 8 km because the v2 cell grid
> (3.2 km, 3x3 neighbourhood) cannot host wider clouds; the permanent fix
> is a coarse cloud-grid tier for storm-scale systems - the cap must not
> silently become the design ceiling.

> **This is the TACTICAL backlog (what is next, right now).** Its strategic, themed,
> public-facing companion is **[ROADMAP.md](ROADMAP.md)** (the same to-do list, grouped
> by theme with status badges, rendered on the website). Use ROADMAP.md for "where are
> we going"; use this file for "what is the very next thing." Keep them consistent.
>
> **Read this file first if you're picking up work without context.** This is the strict-ranked backlog. The TOP item of TIER 0 is what gets worked on next; everything else waits.
>
> **Update rule:** every session that meaningfully changes scope updates this file before ending. The orchestrator_state.json journal records WHY a decision was made; this file records WHAT comes next. Don't mistake one for the other.

## Active focus

> **>>> TIER 0: SCHEDULED RE-VOTES (operator design, 2026-08-25). SPEC, NOT BUILT.**
>
> Operator: "I like the votes having the option to be final and being open to
> revision at a later time. Like maybe once a year we can revote on certain
> things. That way if something about our way of life changes we can actually
> vote to change things." His examples: laws that do not accommodate AI and now
> need to, or a technology that changes the paradigm for a lot of things.
>
> This resolves a real tension, and it resolves it BETTER than the obvious
> reading. The obvious reading was "let people edit a cast vote". That would
> fight the architecture: votes are Dilithium-signed objects, and the relay
> stores the first vote per identity with INSERT OR IGNORE
> (src/relay/storage/governance.rs:169). An editable signed object is a
> contradiction, and an audit trail you can rewrite is not an audit trail.
>
> **The design instead: never mutate a vote, SUPERSEDE a decision.**
> - A cast vote stays immutable and final. Unchanged from today.
> - A proposal carries a review date or cadence (for example one year). When it
>   arrives, the system OPENS A NEW PROPOSAL on the same question, linked to the
>   old one as its successor.
> - The standing answer is "the most recent closed decision in the chain". The
>   whole chain stays visible: what was decided, when, by what weight, and when
>   it is next up for review. That is a feature, not overhead, because seeing
>   that a rule was reaffirmed three times is itself information.
> - Early trigger: a petition threshold reopens a question before its scheduled
>   date, for exactly the case the operator described, where the world changes
>   faster than the cadence.
>
> Work required, roughly in order:
> 1. Schema: `review_after` / `review_cadence` and `supersedes` on proposal
>    objects (data/governance/proposal_types.ron + the signed-object shape).
> 2. Relay: resolve a chain to its current standing decision; expose the chain.
> 3. A scheduler that opens the successor proposal when a review date arrives.
> 4. UI on both clients: show the standing answer, the history, and the next
>    review date. Native first per the Rust-first rule, then web mirrors.
> 5. Petition threshold for early reopening.
>
> **Blocked-ish dependency worth knowing:** casting a vote is NATIVE ONLY today.
> Web's vote button is a stub because it needs canonical-CBOR signing in JS plus
> a cross-language KAT. Re-votes are worth little if most people land on the web
> and cannot vote, so web voting is arguably the prerequisite item.
>
> **Open, and the operator's to answer, not ours:** he holds that free speech and
> self-defense are universal rights that ought to be global. Asserting that
> belongs in the "base" set, which is distilled from the Humanity Accord. The
> Accord today protects "diversity of culture, belief, and expression"
> (humanity_accord.md:98) but does NOT name speech or self-defense as rights.
> Adding them is new Accord doctrine and needs his words, not ours. The
> descriptive half is already done: the real-law set now carries the 2nd
> Amendment beside the 1st, 4th, 5th, 6th and 14th, and Washington already
> covers reasonable force in self-defense plus firearm carry and storage.
>
> Already true and NOT to be rebuilt: the jurisdiction tree is Humanity > Earth
> > United States > Washington > Kitsap County > Silverdale, and rules are
> already tagged by jurisdiction and category (Rights, Speech and association,
> Privacy, ...). Location categorisation exists; it does not need inventing.


> **>>> TIER 0 NEWEST (2026-08-25): THE FRONT DOOR. Shipped v0.1212.0, ONE DECISION OPEN.**
>
> Real people handed the operator's phone could not say what the site was. Two
> investigations ran, and both found things worse than "the copy is vague".
>
> **SHIPPED in v0.1212.0** (all verifiable defects, not taste):
> - The FIRST CLICK WAS BROKEN sitewide. The onboarding tour auto-started 2s
>   after load behind a full-viewport pointer-events:auto overlay at z-index
>   10000; hit-testing confirmed the overlay, not the button, was the top
>   element at the centre of "Get the free app". The tour is now opt-in only.
> - "Runs on old computers" was FALSE and is gone. The renderer is
>   Backends::DX12 only on Windows, VULKAN|METAL elsewhere,
>   force_fallback_adapter:false, with a hard .expect panic BEFORE any window
>   is shown, so an unsupported machine shows the user nothing at all, forever.
> - "Say hello in chat" bounced first-time visitors back to the top of the page
>   they had just finished reading. Zero product imagery plus a caption
>   admitting it. "No accounts" three lines from explaining your account.
>   Favicons 404ing on every page. All fixed.
> - ONE anchored help button ("?" -> "X", same pixel both ways) on both
>   clients, replacing the popup. Per-page, data-driven, non-modal. Native F1
>   hold-to-glance is unchanged. See FEATURES.md "Anchored Help Button".
>
> **THE HERO DECISION: MADE AND SHIPPED (v0.1212.1). Tools-first.**
> Operator chose tools-first because it "more accurately portrays the holistic
> purpose of the software". Screens 1-3 rewritten as a set:
> 1. "Free tools for growing food, collecting water, and making power." A noun
>    in the first three words, concrete verbs, and the game arriving in the same
>    breath as a CONSEQUENCE rather than a second product.
> 2. SHOW it before describing it: the real capture with a caption.
> 3. Four things it does today, then the game as a consequence of the tools.
> The author is now named on the page (3 of 4 cold readers said the absent human
> was why they would not install). Title/description/og/twitter all match.
>
> Kept OUT of the hero on purpose, each measured a net negative with cold
> readers: the word "OS" (two thought they were being asked to replace Windows),
> "open source" unexplained, "no sign-up" (alarms rather than reassures), and
> the five-adversary list. The mission was NOT deleted, it moved down the page.
> If a future session is tempted to restore mission-first framing at the top,
> read the comprehension data in orchestrator_state.json 2026-08-25 first.
>
> **Known and NOT yet fixed** (from the same audit, ranked):
> - The share banner web/shared/social/og-banner.png leads with the RETIRED
>   "Project Universe" logo across the left half of every link preview anyone
>   has ever posted. Needs a rebuild at 1200x630. Highest impact per effort.
> - Desktop releases are UNSIGNED, so the updater refuses every one of them
>   (src/updater.rs) while download.html promises "It keeps itself up to date".
>   OPERATOR ONLY: `just sign-release vX.Y.Z`, needs the passphrase.
>    Operator's position 2026-08-25: will sign "when appropriate", because
>    releases currently ship faster than signing each one would justify.
>    DO NOT NAG about this. The half that does NOT need him: stop
>    download.html promising "It keeps itself up to date" until signing is
>    routine, since that sentence is false for every release today.
> - On an unsupported GPU the app panics before a window exists and the user
>   sees nothing. Wants a window-first "Starting HumanityOS" panel and a real
>   message box naming the requirement.
> - Stated download size understates macOS/Linux by ~2.7x (62 MB claimed, ~170
>   MB actual). nginx serves JS/CSS uncompressed (gzip_types is default).
> - Only ONE real product screenshot exists on the landing page. The other
>   candidates were unusable: garden_modal_tower.png has overlapping text,
>   chat_commons.png shows "Not connected" in red. Wants fresh captures.


> **>>> VPS: REBUILT, LIVE, AND HARDENED (2026-08-07 .. 08-12). DONE.**
> The server was null-routed by Namecheap for relaying attack traffic through
> an abusable coturn TURN relay (legacy static credential, public in the repo
> until v0.857, migration never run on the box). Full story:
> INCIDENT-PLAYBOOK.md "The TURN relay abuse". Fresh Debian 12, rebuilt
> entirely from scripts/provision-vps.sh, DB restored (zero loss). Six
> install-time landmines and three post-rebuild "quiet killers" all found,
> fixed IN the scripts, and guarded by assertions:
> - DB backups (were ZERO on the rebuilt box - units referenced but never
>   existed). Live now: humanity-backup-db.timer, every 30 min,
>   restart-independent. Asserted.
> - Cert renewal (was armed to fail ~Nov: standalone needs port 80, nginx
>   holds it). Fixed with renewal hooks, proven by --dry-run. Asserted.
> - Relay logging (headless relay initialized NO tracing subscriber - the
>   incident was investigated with zero relay logs). Fixed v0.1113.0.
> - provision-vps.sh is now DOMAIN-PARAMETRIC (node.env) - the first "boot
>   your own node" seam. Canonical byte-identical; a self-hoster sets 3 lines.
>
> STILL OPERATOR-ONLY (all cheap, none blocking):
> - **Off-box uptime monitor** (free UptimeRobot on /health). This outage was
>   invisible ~12 h because every monitor lived on the box it watched. TOP.
> - Confirm chat admin powers survived the DB restore; claim-code fallback in
>   docs/admin/SELF-HOSTING.md if not.
> - Reply to Namecheap NC-QTL-4184 declining the log offer (case closed).
>
> **>>> THE SELF-HOST / FEDERATION / MERCHANT VISION (mapped 2026-08-12 by a
> 4-investigation + adversarial-verify workflow, ~1M tokens - do NOT re-derive,
> read wf_679e408f-88b in the journal). Ranked by value x independence:**
>
> 1. **Merchant inventory - SHIPPED v0.1140.0, read side + vocabulary SHIPPED v0.1141.0** (v0.1140.0: validators for provider_v1/offering_v1 at the put_signed_object chokepoint covering REST+gossip; ownership rule; directory-only settlement enforced; bulk importer scripts/import-offerings.mjs + samples + docs. v0.1141.0: native Market page Directory tab - offering browse/storefronts/detail over GET /api/v2/objects with client-side CBOR decode + latest-revision resolution + TTL filtering + Real/Sim split; need-shaped categories.json ({id,label,desc}: food water shelter energy health care clothing tools materials repair transport growing education communication services emergency other) consumed by native + web selects + the relay validator (shared_categories at the chokepoint, fail-open); sample provider + 2 offerings PUBLISHED LIVE to united-humanity.us proving production ingest). v0.1143.0 closed the next two: IN-APP PUBLISHING (native Market > Directory > "+ Publish": shop + offering forms, locally validated with the exact relay validators, signed with the chat identity via pq_object_keypair + ObjectBuilder, POSTed off-thread; end-to-end proven against a throwaway relay incl. the foreign-key rejection; the proof caught a missing JSON content-type header) and the WEB DIRECTORY MIRROR (market page opens on Directory: browse/storefronts/detail modal, decodeCanonicalCbor extended for floats, encoder byte-lock re-proven by group-object-kat; also surfaced + fixed native reading "summary" where the schema says description). REMAINING for the merchant arc: group-backed provider member keys (a group_ref-backed provider accepting offerings signed by any active group member). TRADE-VS-MARKET MODEL (operator confirmed 2026-08-17): providers span neighbor sellers to big retail; the DIRECTORY listing stays free for everyone (the free-Etsy posture); commission generation is a per-provider AFFILIATE flag with disclosed commission, never a listing fee; Trade covers EXECUTION including barter (goods for goods/services, no money required); Trade folds into a Market tab when real execution ships. Original scope note:
>    Define a `listing_v1` signed-object payload (schemas/listing.toml: item
>    ref, title, desc, category, structured price {amount,currency}, stock,
>    condition, fulfillment, location; seller = signing key) + a validated BULK
>    IMPORTER, both riding the EXISTING signed-object pipeline (POST
>    /api/v2/objects + auto-gossip). Federation replication then comes free the
>    day a second peer works. Today /api/listings is a single-server classifieds
>    board owned by individual user keys - no bulk path, no settlement. A
>    group_v1 signed-object family already exists to build MERCHANT IDENTITY on.
>    Bug found in passing (fix cheap): web wallet USDC send is broken by a name
>    mismatch - wallet-app.js:394 calls sendToken; wallet.js exports sendSPLToken.
>
> 2. **White-label seams.** (a) DONE v0.1139.0: ALLOWED_ORIGINS single-source (WS-Origin + CORS + CSP; provisioner writes it; public.guide live-proven 101). Remaining seams below:
>    Clients are MOSTLY domain-relative already (web chat uses location.host;
>    native has a server picker). The real blockers, cited:
>    (a) THE hard one: the relay bakes its browser WS-Origin allowlist
>    (src/relay/mod.rs:840-846), CORS allow_origin (744-751) and CSP connect-src
>    (77-88) to the five united-humanity hostnames, so browser chat on ANY other
>    domain is refused. Fix: build all three from ONE source (ALLOWED_ORIGINS
>    env / a domain field in data/server-config.json), the five as default. One
>    file, three sites, plus a provision line - THE single change that unblocks
>    a self-hoster's web chat.
>    (b) DONE v0.1141.0: native TURN base now derives from the CONNECTED
>    server (WebrtcManager::start takes relay_base from the active ws URL;
>    HUMANITY_RELAY_BASE still overrides; dead TURN_SERVER const deleted).
>    gui/mod.rs streaming default is user-editable already.
>    (c) ~1004 branded literals (incl. "shaostoul") across ~30 data/ files,
>    dominated by data/announcements_archive - mostly content, not config.
>    (d) CLOSED v0.1141.0 by deletion: data/gui/navigation.json was consumed
>    by NOTHING and stale; removed (file + embedded_data registrations). A
>    future data-driven nav should be authored fresh against the live page
>    registry when actually wired.
>    (e) NEW SEAM SHIPPED v0.1141.0 - PER-NODE HOMEPAGE (operator direction
>    2026-08-15: "not everyone wants the same homepage"): nginx serves
>    /var/www/humanity-site/index.html over the repo default when present
>    (scripts/nginx/humanity.conf "Landing page" block; deploys never touch
>    the override tree; /site/ alias for custom assets; provisioner seeds
>    README). Repo ships flavors in web/home/; technical.html (engineer-
>    facing intro: architecture, crypto table, federation, self-hosting,
>    market objects) is LIVE as public.guide's homepage while
>    united-humanity.us keeps the mission page - two front doors, one
>    platform. Docs: SELF-HOSTING.md "Your own homepage"; in-app flavor
>    picker logged in docs/design/in-app-ops.md (Server Settings > Website).
>
> 3. **Federation - LIVE IN PRODUCTION, both directions proven (v0.1127.0,
>    2026-08-14).** Eight independent defects fixed (BUGS.md BUG-071, incl.
>    defect 8: outbound sockets registered by dialed URL while messages
>    identify by public key, so everything a peer relayed back was dropped;
>    the two-relay test now covers BOTH chat directions and reproduced the
>    live drop red before the fix). Live proof: bot probes crossed
>    public.guide <-> united-humanity.us both ways, persisting as
>    msg_type='federated_chat' with correct origin_server on the receiver.
>    THE WHOLE BUILD ORDER SHIPPED 2026-08-14 (v0.1128.0-v0.1130.0):
>    multi-connection foundation (park/unpark + bg_connections pump, all
>    saved servers live at once), the COMMONS sidebar section with the
>    merged cross-carrier view, carrier send routing with inherent
>    failover, and the Federation GUI (add-by-key Pair, bridged-with
>    labels, server-named admin header, Servers nav). Statuses in
>    docs/design/federation-ux.md build order. Field test 2 findings all
>    closed v0.1131.0-v0.1132.0 (Commons identity + carrier-only merge +
>    composer says where a send goes; carrier history depth; per-row
>    server cogs; host-node autostart; guaranteed local-only #local room,
>    toggleable, LIVE on both servers; all-shared labels; parallel dials).
>    REMAINING wants, ranked:
>    (a) operator field test 3 (Commons identity + #local + autostart);
>    (b) federation keepalive/ping so idle links do not silently rot
>    behind NATs; (c) per-channel per-peer bridge scoping; (d) channel
>    portable identity so Commons matching stops relying on names.
>
> 4. **BOOT TIME - measured, first win shipped (v0.1132.0-v0.1133.0).**
>    Probe profile ([BootPhase] in run.log + debug/boot_timing.json):
>    adapter_request 1.4-1.9 s, device 60 ms, shaders_and_pipelines
>    4.0 s (main-thread DXC compilation - THE target), ground bake was
>    1.0 s (now overlapped with shader compile, boot -0.65 s, v0.1133.0);
>    world entry adds planet_defs_bake 3.8 s + star_catalog 2.1 s.
>    NEXT, in order, each probe-gated (world entry, panics=0):
>    (a) DONE v0.1142.0: all seven PSO compiles in ONE thread scope
>    (was 3 parallel + 4 serial). Measured: pipeline_new 3894->2404 ms,
>    shaders_and_pipelines span 4177->2677 ms. Per-unit [BootPhase]
>    marks kept as the instrument. Floor is now the slowest single PSO;
>    the next boot rung there is a wgpu pipeline CACHE (descriptor
>    cache field is wired None today; DX12 supports PipelineCache).
>    (b) planet_defs_bake 3.8 s at world entry (cache or parallelize);
>    (c) adapter_request is wgpu-internal, likely not ours to fix.
>
> 5. **THE MAPS LADDER (operator direction 2026-08-16; full design in
>    docs/design/maps-ladder.md, read it, do not re-derive).** Rung 1
>    SHIPPED v0.1145.0: the Galaxy view draws the REAL HYG catalog
>    (data/stars-map.bin, 109,400 stars). Rung 3 increment 1 SHIPPED
>    v0.1146.0: fetch-osm-region.mjs (Overpass -> HOSMREG1, deterministic,
>    self-verifying) + the Maps Planet GPS view (Seattle Center region).
>    Increment 2 SHIPPED v0.1148.0: 3D extrusion in-world; roads draped +
>    buildings extruded on the chunked-LOD Earth, planet-fixed
>    (src/terrain/osm_region.rs parser/projection/mesher +
>    src/engine/region_meshes.rs progressive elevation grid + background
>    worker; Seattle Center + Silverdale WA shipped; probe vantages
>    silverdale-osm-ground + seattle-osm-5km). Water increment SHIPPED
>    v0.1149.0 (operator field report: Dyes Inlet + lakes missing):
>    HOSMREG2 water records + src/terrain/water_carve.rs terrain carve
>    (sea polygons press ground below sea level so the ocean shell fills
>    inlets; lakes get flat sheets; 2D Planet view fills the polygons).
>    NEVER hit live OSM tile servers from the app; regions ship like
>    star-catalog tiers. Remaining rungs ranked:
>    GRAPHICS FIELD-REPORT QUEUE (operator 2026-08-17, evidence in the
>    journal + docs/design/clouds-depth.md):
>    (i) TILED-LIGHTS NIGHT GLOW: FIXED v0.1155.0 (the tiled loop
>    ignored each pass declared light_count and smuggled interior
>    lights onto the terrain pass; one shader guard; night vantage now
>    pins lights_tiled ON as the permanent regression lock).
>    (ii) CLOUD DEPTH [phase 1 SHIPPED v0.1156: physical 0.4-12 km slab, metric ladder+extinction, orbit-proven; PHASE 2 NEXT: from-below presence calibration - carve/erosion/sampling were tuned for the deleted 51 km slab; harness is ready (junction heal + --reload-shaders + naga-safe diagnostics), gates in clouds-depth.md]: fenced 6-step increment in clouds-depth.md (deck
>    10-50x too high on a v0.883.2-stale constant; light march blind to
>    erosion; NOT texture resolution). Measured acceptance gates ready.
>    (a) Vegetation suppression inside region footprints (the probe
>    showed the procedural forest growing through Silverdale's streets;
>    placement should skip road ribbons + building footprints + water
>    polygons, the region file carries all three); region browser +
>    release-asset region downloads (increment 3); multipolygon
>    buildings (the fetcher's relation assembly now exists for water);
>    road smoothing + intersection blending (polish).
>    (b) Solar System view improvements (operator: "needs improvement";
>    candidates listed in the ladder doc, operator to rank).
>    (c) Cosmic web someday: 2MASS/SDSS galaxy catalog as a Universe
>    view above Galaxy (few MB, same brightest-first pattern).
>
> 6. **NAMED VISIONS captured 2026-08-16 (design items, not yet fenced):**
>    (a) STUDIO SIMULCAST: stream from PC -> own relay -> restream to
>    YouTube/Twitch/X/FB simultaneously; Studio page keeps a dedicated
>    chat pane so the streamer watches chat while operating the studio.
>    (b) WATCH AS UNIVERSAL VIEWER: watch HumanityOS streams AND
>    Twitch/YouTube embeds through one interface; someday synced watch
>    parties. Per-user URLs: /watch?u=<name-or-key> (the watch page
>    already takes a stream id).
>    (c) PLAY/CHARACTERS STRUCTURE: characters + worlds; Open Net
>    (character visits any server) vs Closed Net (server-held
>    characters, anti-cheat); Play = straight into the default; the
>    showroom is the character/world manager. Operator: "close but still
>    a bit weird", needs a design pass articulating the model before
>    restructuring.
>
> **>>> SECURITY follow-ups (post-incident, ranked):**
> - CI deploy key = the operator's personal root key and can do ANYTHING as
>   root. Split it: CI gets its own key with a forced command in
>   authorized_keys that can only trigger a deploy.
> - Deploy warned "humanity-relay-services failed visudo" - a sudoers file the
>   new box lacks; find what it is for (grep repo) and whether it is needed.
> - Deploy pipeline should run provision-vps.sh's assertions remotely after
>   each deploy, so drift is caught by CI, not by an abuse team.
>
> **>>> THE PLANET-PHYSICS / AGARTHA ARC (operator directive 2026-08-12;
> design + audit in docs/design/artificial-planet.md - read it, do not
> re-derive). Shipped v0.1118.0-v0.1119.0:** gravity_curve data model +
> walk-band sampling, per-body environment (weather gated by body, no rain
> on the Moon or Venus, breathability from planet data, temperature v1
> global/at-player split), sol.json disk-load + catalog_version gate +
> magnetic/pressure fields, ship-interior gravity from data/game.csv
> (live-tunable, first game.csv knob with a real reader), Maps page planet
> list rebuilt from the catalog (was silently empty), ron_edit.rs (the
> comment-preserving RON rewriter). NEXT concrete steps, in order:
> 1. DONE v0.1120.0: **Planet Tuner** shipped (Platform section, Dev-gated):
>    live readout + grouped PlanetDef editors + gravity-curve editor, saves
>    via ron_edit set_or_append (validated append was added for omitted
>    serde-default fields) with a final loads-as-a-PlanetDef check so a bad
>    value can never brick a file. Hot reload rebuilds the planet in a frame.
> 2. **F2 readout: current g** at player (one line, closes ladder item 2).
> 3. **Agartha authoring** (ladder item 6): destination-system data file,
>    agartha.ron with the hollow-shell gravity_curve, terrain seeds; the
>    interior rides the voxel-terrain arc when it starts.
> Deferred debt logged: web mirrors for subsection headers + Controls
> rebind UI (dual-UI parity); F1 overlay renders default keys not live
> binds (keymap.rs documents it); per-body gas composition as data;
> hemisphere seasons + axial tilt for non-Earth bodies.
>
> **>>> THEN the game work, unchanged from 2026-08-04:**
>
> **>>> TOP ITEM (2026-08-04): VIEW-FRUSTUM CULL THE NEAR TREES.** The largest
> verified unclaimed win on the board, and every constraint below is already
> MEASURED - do not re-derive them, they cost about 1M tokens of agent time.
>
> At the operator's own settings (121 deg horizontal FOV, 400 m model distance,
> 1024 budget), **62% of the drawn trees are outside the frustum**. Cross-checked
> two independent ways - an engine harness on real Earth data and a
> first-principles geometric calculation - agreeing within 1.5 points. The
> near-tree draw loop in `src/lib.rs` tests only distance and budget; the
> `frustum` is built in the same frame and handed to terrain selection and both
> water passes, never to the trees.
>
> THE PAYOFF IS THE RESPEND, not the saving. Today the nearest 1024 of ~5,000
> trees in range reach 181 m. Spend the same 1024 on the nearest VISIBLE trees
> and they reach 300 m: 1.7x the radius, **2.9x the area for the same draw
> count**, and the model/card handoff moves 173 m -> 293 m. The saving itself is
> real but smaller: ~2,000 draw calls and ~4.4 M vertex-stage triangles. A tree
> behind the camera is free to RASTERISE, not free to TRANSFORM.
>
> FOUR THINGS MUST BE RIGHT. All four were measured; none is optional.
>
> 1. **The coverage rule.** `ModelCoverage` derives the card-hide radius from the
>    nearest tree that got no model. Naively SKIPPING culled trees gives a hide
>    radius of 392 m against models stopping at 201 m - a 190 m bare ring
>    directly in front of the player. Naively reporting them as UNCOVERED gives
>    4.7-16.8 m, so nothing ever hides and every model double-draws with its
>    card. Only this works: **visible trees feed coverage, AND the radius is
>    additionally clamped by the harvest edge** (the distance to the last
>    harvested tree).
> 2. **The cull bound must enclose the CARD, not the model.** `sprite_card_frame`
>    frames a square card up to 1.365x tree height (acacia). A 0.6h model sphere
>    lets a card rasterise while its model is culled, and inside the hide radius
>    that card discards - so the tree disappears entirely.
> 3. **The shadow pass stays UNCULLED.** Near-tree models are the only off-screen
>    shadow casters in the scene. At an 8 degree sun with the sun behind you,
>    44-61% of culled trees cast into view; at 45 degrees or facing the sun, 0%.
>    Cull the colour pass only.
> 4. **Raise the harvest cap.** `near_tree_harvest_cap` = budget + 256 is not
>    enough once culling lands: the cap binds first and the budget only fills to
>    ~45%, so the reallocation never happens. Needs roughly 4x budget.
>
> Why this is newly possible: view-independence was load-bearing until v0.1110.2,
> because the hide radius derived from the drawn COUNT. It no longer does.
>
> **>>> THEN: MEASURE THE 103 ms FOREST FRAME BEFORE RANKING ANY PERF FIX.**
> Nobody knows where it goes, and one agent's answer ("CPU-submission-bound") was
> adversarially refuted for resting on zero measurement of the actual frame.
> Known: the named CPU is ~35 ms of 103; the only real GPU data says the pass
> CONTAINING grass is 68-91% of GPU time and it just received 10-30x more
> geometry (245,605 tufts, ~32 M triangles); and grass has no frustum test
> either, with ~66% of tufts off-screen. ALSO UNEXPLAINED, already on record: the
> same scene, camera PARKED on byte-identical content, swinging 44.3 -> 101.1 ms
> over 35 minutes. Get the real per-pass breakdown out of `frame_costs` first -
> grass culling vs blade LOD vs draw batching cannot be ranked until then.
> CAUTION reading fps: the operator's display is 119 Hz, so vsync quantises to
> 119/n (30 = 119/4, 23.8 = 119/5). Only unquantised numbers measure work.
>
> **>>> THEN, ranked:**
>
> 1. ~~Snap slider values to their displayed precision~~ **DONE v0.1112.0**
>    (2026-08-04). Drag snaps to the display step in `custom_slider_with_width`;
>    typed values stay full-precision. Gate:
>    `slider_snap_tests::a_dragged_value_survives_its_own_display_format`.
> 2. **Time control on the surface** (operator, 2026-08-04: "I don't want to
>    have to wait many minutes for the sunrise or sunset"). The machinery
>    already exists and is economy-safe: `systems/time.rs` has `time_scale` +
>    `set_time_scale`, and craft/drone/manufacturing timers all run on
>    `scaled_dt` - but `set_time_scale` has ZERO callers, so there is no way to
>    touch it in-game. Build: a dev-page time row (scale presets x1/x10/x60 +
>    "jump to sunrise/sunset/noon/midnight", which is a set of day-fraction
>    targets, not a new mechanic) AND the GUI-first rule means a player-facing
>    surface too (the ship bed / wait action is the natural fiction). Small: the
>    sim side is one call; the work is the two UI rows + persistence + a gate
>    that jumping time does not skip economy timers (they scale, so a JUMP must
>    either tick them forward or be documented as not doing so - decide, state
>    it, test it).
> 3. **The understorey / bushes** (operator asked twice). The 0.3-3 m band is
>    empty, which is most of why the ground reads as a lawn with trees stuck in
>    it. A previous plan was KILLED by both its reviewers: it mis-costed the draw
>    path (it assumed up to 4 card layers, but `ClusterLayer::ALL` has exactly 2)
>    and its "70% already covered" apportionment did not survive checking. Redo
>    the plan from the code. Honour the realistic-first rule: a basitonic
>    (basal-branching) growth form through the existing spline-lofted stem +
>    cluster-card pipeline, NOT billboard bushes.
> 4. **Tree identity, for felling.** Trees are a hash re-derived by two streams
>    that share no key, which is why they cannot be chopped down. A stable key
>    already exists structurally - (cell_x, cell_y, item_index) - it is just
>    never materialised. Full persistence is 19.4 TB and absurd; a sparse DELTA
>    store of only CHANGED trees is ~16 MB per million felled. Keep procedural
>    placement; record only what the player altered.
>
> **>>> OPERATOR-ONLY, waiting on you:**
> - **Sign the releases.** v0.1110.2 and v0.1111.1 are UNSIGNED, and the desktop
>   updater only offers signed releases - so v0.421+ users are still on an older
>   build. `export HUMANITY_SIGNING_PASSPHRASE=... && just sign-release v0.1111.1`.
> - **19 stale `agent-*` worktrees, ALL AUDITED AND ALL SAFE TO REMOVE**
>   (2026-08-04). Every one was checked properly rather than by mtime: take the
>   worktree's own change (diff against its merge-base with main, so committed
>   AND uncommitted work), extract the substantive lines it ADDED, and look for
>   each one in main. Result: **9 are byte-for-byte fully in main**, and the
>   other 10 differ ONLY in superseded formulations of code that did land - the
>   `0.80..=1.30` canopy band replaced by the per-species table, `GROUND_MACRO_AMP`
>   0.55 replaced by 0.30, the `dev_fly_mode` wiring replaced by `dev_hover`, and
>   so on. Every distinctive identifier in those 10 (`grass_detail_for`,
>   `ground_material_weights`, `UNCARDED_SPECIES`, `leaf_needle_len_frac`,
>   `reshape_blades`, `shadow_for`, `DrawnPatchSurface`, `flared_run`) exists in
>   main. The single exception, `TREE_MODEL_ENGINE_CLAMP_M = 400.0` in
>   `agent-a6723b9dda4c02ee6`, is a Settings warning about a renderer clamp that
>   has since been raised to `TREE_MODEL_MAX_M = 2000.0` - the warning is moot.
>
>   **NOTHING IS LOST EVEN IF THAT ANALYSIS IS WRONG.** Every worktree's own diff
>   was archived first, outside the repo, at
>   `%LOCALAPPDATA%\HumanityOS-worktree-archive\2026-08-04\` (20 patches, 1.2 MB,
>   plus an INDEX.txt naming each merge-base). Re-apply with
>   `git apply <name>.patch`.
>
>   REMOVAL NEEDS THE OPERATOR: the permission classifier blocks bulk worktree
>   deletion from a session, both as a `git worktree remove --force` loop and via
>   `just clean-worktrees --force-unmerged`. Run this yourself when convenient:
>
>   ```bash
>   just clean-worktrees --force-unmerged
>   ```
>
>   There WAS a 20th directory, `agent-a6824b91fc94231c0`, which git had no
>   record of at all - a completely empty husk, no `.git` link and no entry in
>   `.git/worktrees/`. Removed 2026-08-04, so the folder count and the worktree
>   count now agree at 19. Note the audit script initially reported it as
>   "EMPTY, base a1dd0b28": running `git -C <dir>` inside a directory with no
>   `.git` of its own makes git walk UP and answer about the PARENT repo, so
>   that base was main's own HEAD, not the directory's. Any per-worktree tool
>   should confirm a directory IS a worktree before believing what git says
>   about it.
> - **2.5 GB of stale probe rigs: DONE** (2026-08-04). The 15
>   `.probe-rig-{ab,bark,...}` directories from investigations that closed
>   2026-07-31..08-02 are removed, along with their 448 capture PNGs, whose
>   conclusions all live in git, docs/BUGS.md and the journal. `.probe-rig`, the
>   canonical rig `probe-sweep` uses, is untouched.
>
> The WATER ARC below is still open and still ranked, but it is not what is
> being worked on.

> **>>> WATER ARC (2026-07-27 evening field report, operator: "We need to do
> a lot of work on the water to make it look better. Maybe the water is one
> of the biggest performance hits since it's behaving so weird?"). STILL
> OPEN, NOT the current top item - see above. Punch list from the report
> + screenshots, roughly
> ranked (diagnose first - several may share a root cause):**
> 1. SEAM HOLES + unwelded look: "textures don't quite line up and it's
>    like the mesh don't weld seams. I can see a bunch of holes through
>    the water along the seams" (water patches have no skirts? border
>    verts not bit-identical across depths?).
> 2. HARD TILE LINES at altitude (screenshots: checkerboard-edged sheets
>    of white/dark at 0.7-2.1 km): per-patch shading/wave-amplitude
>    discontinuities + spec aliasing. Also "white splotches" band at low
>    altitude that vanishes higher up (grazing-angle spec aliasing -
>    normal-detail fade by distance is the standard fix).
> 3. STATIC waves at the surface: "water has height but... the waves
>    don't move" while standing at the shore (near-field Gerstner time
>    input? mesh vs shader displacement split?).
> 4. UNDERWATER: surface disappears from below (backface-culled shell) -
>    draw the underside so submerged players see the surface above them.
> 5. BLUE BLEED onto land: "land that is above water is blue" (coastal
>    tint mask leaking above sea level).
> 6. PERF pass 1 SHIPPED (v0.1020.1): backstop 1-octave hue early-out +
>    crest-warp footprint fade (up to 6 noise evals/pixel saved at
>    distance). REMAINING rungs: batch water shells through the patch
>    arena (still classic per-object, ~640 draws worst case); measure
>    the 4-layer transparent overdraw (backstop+shell+atmo+clouds).
>    PARTIAL 2026-07-31 overnight: full 24-vantage fps baseline recorded
>    (.probe-rig/sweeps latest manifest, all vantages at/above floor; slow end =
>    ground snow/storm 13-19 fps), and the CLOUDS layer measured ~free (domain-pass
>    interleaved ON/OFF A/B, no delta beyond noise). REMAINING: decompose
>    backstop/wave-shell/atmo contributions the same interleaved way.
>    RE-AIMED 2026-07-31 by the clouds domain pass: the CLOUDS quarter is
>    now measured and it is not the problem. Feature-off A/B on the probe
>    rig (RTX 4070, 3 captures per arm, >=15 s settle): WATER costs 7.8 ms
>    at land-sandstorm (31% of the frame, on a DESERT vantage) and 6.6 ms
>    at ground-storm-inslab, while clouds cost nil at 3 of 4 vantages
>    (only limb-400km shows +4.0 ms). Sun shadows are 2.8 ms at
>    ground-storm-inslab. So the remaining transparent-overdraw work
>    belongs to THIS water arc, which is already the top item. Two rig
>    caveats for whoever measures next: blue-marble-12000km is
>    present-capped at exactly 16.1 ms and can never show a delta, and an
>    8 s settle leaves streaming spikes inside the 120-frame ring (repeats
>    of one config disagreed by up to 15 ms). Resolving per-LAYER cost
>    properly needs wgpu timestamp queries around each transparent draw,
>    not frame-average differencing.
>    Original report: 16-18 FPS in ocean-heavy views vs 30-40 over land - profile
>    the water pass at budget defaults (operator suspects water is a top
>    perf hit; the checkerboard artifacts suggest overdraw or per-patch
>    waste).
> INCREMENT 3 SHIPPED (v0.1019.1): the BACKSTOP SHELL - long swells sag
> ~1.2 m over coarse patch edges (computed; they cannot be faded, they
> ARE the sea), so a coarse undisplaced deep-water layer ~4.7 m below
> the wave shell now water-colors every T-junction tear, above or below
> the waterline. Rig-verified: straight-edged pale polygon sheets GONE.
> INCREMENT 2 SHIPPED (v0.1018.1): underwater blend order (water sorts
> last submerged), foam lacework, surf direction sign flip, de-blue
> reworked to a blue-dominance clamp (purple triangles), tree floor 6 m
> (shoreline strip), W4 anchored (lambda 32), buoyancy DVec3 end-to-end
> (the operator-requested f64 audit find).
> INCREMENT 1 SHIPPED (v0.1017.1): items 1+2 fixed by per-train resolution
> fades (ocean_train_fade; holes + checkerboard verified gone at the rig);
> item 4 fixed (fully_covered all-or-nothing gate removed + below-view
> normal flip); item 5 fixed (grade_albedo coastal de-blue, shared with
> the bake); PLUS two live follow-ups: vertex JITTER (chop rephased in
> the camera-anchored 64m-modulus domain, axis-aligned dirs, f64 twin -
> the f32-at-scale class, GPU edition) and foam PULSING (train-beat
> threshold raise). STILL OPEN: item 3 static shore waves = designed
> shoal damping, wants a real breaking-wave rung; item 6 perf profile;
> operator re-verify of splotches/seams at play. Rig can now DIVE
> (camera_request negative altitude_km).
> DONE from the same report: FPS caps setting (foreground/background +
> unlimited/sync, v0.1016) + swim gear scaling + the rig focus-steal fix
> (the mouse-look freeze root cause). Deferred to reserved arcs: tree
> square/circle seams (billboard/instancing arc, analysis in its doc);
> cloud drift desync (structure pinned by live weather while texture
> drifts zonally - next clouds polish rung, noted below).**

> **>>> BEDTIME DIRECTIVE QUEUE (2026-07-26 late evening, operator verbatim
> list; worked overnight into 2026-07-27, releases v0.996.0-v0.1002.1).**
> Status per item:
> - DONE footsteps trio (v0.996): airborne-silent, 1.5 m stride, real
>   volume path (play_sound_vol; catalog volumes were structurally ignored).
> - DONE clouds visible from surface (v0.997): view-dependent transparent
>   ORDER (inside atmosphere = dome then deck; outside = deck then dome).
> - DONE night-lit trees (v0.998): fill light now scales with daylight in
>   the walk band (fill_scale poke at 652), rig-proven at local midnight.
> - DONE light-line glitch (v0.999): grass-field hard edge at the patch
>   boundary; bit-18 grass cards now Bayer-dissolve over 30-45 m.
> - DONE cloud subdivision + cliff edges (v0.1000.0): coverage octave
>   ladder 9/19/41/83 + cross 13 (mesoscale cells from orbit) + height
>   squash (weak columns top out low; NOTE the squash lives in the
>   Medium/Low cloud_density path; the High tier's height shaping is its
>   own towering mechanism in cloud_carve).
> - DONE cloud self-shadowing (v0.1003.0): "clouds don't block light from
>   each other" was a real BUG, not missing machinery - a tau heat-map
>   shader probe showed the High-tier light march reused the view-alpha
>   sigma, capping every shadow at ~e^-0.5 (tau ~0.1 across solid noon
>   overcast), and its first tap overshot thin stratus bands entirely.
>   Fixed with the standard view/light extinction split
>   (CLOUD_LIGHT_SIGMA_MULT 6x + halved first-tap step); undersides now
>   carry real gray weight. The pillowy/cauliflower STRUCTURE want remains
>   the VOLUMETRIC CLOUDS ARC below.
> - DONE clouds field report round 2 (v0.1014.1, operator photos: cliffs /
>   flat-sheet tops / straight hard lines): domed tops via height-rising
>   carve threshold (CLOUD_TOP_RISE) + geometric light ladder (first tap
>   0.9 km, deck-top relief finally shades) + crown channel (valley shade
>   at zenith sun; crown-weighted fine erosion = 3-13 km turrets) + fine
>   band de-stretched (the close-range slash artifact, same class as the
>   v0.1012 puff fix) + MODIS swath-seam data fixes (3-pass fraction blur,
>   chamfer validity feather ~500 km; live_weather.rs). Rig A/B: turreted
>   dome silhouettes side-on, mottled mounds from above, razor swath band
>   at 12N/122E gone. WATCH: double-lip cones at some mass leading edges
>   (rounded, borderline natural); cirrus slightly grayer via crown floor.
> - DONE terrain-gen proper fix increments 1+2 (v0.1001.0 + v0.1002.0):
>   patch mega-buffer arena + batch shader variant + real-byte cache
>   accounting + one multi_draw_indexed_indirect submit
>   (docs/design/terrain-draw-batching.md). MEASURED HONESTLY: at budget
>   12,288 the remaining frame cost is GPU vertex throughput (~9.2M
>   unshared flat-shaded verts), so the visible FPS gain rig-side was
>   small; CPU-side staging/bind churn is gone (bigger relief on the
>   operator's loaded session). NEXT LEVER (new arc, not a tweak): shared
>   terrain vertices with FS-derived per-face data, or bigger patches
>   (PATCH_TESS 16 -> 32 = 1/4 the draws at equal quality). Practical
>   note for the operator: budgets 2048-4096 run 60+ FPS today.
> - DONE increment 3 - shared terrain vertices (v0.1015.1): provoking-
>   vertex flat packs (shader groundwork v0.1013.1 + the
>   emit_shared_grid_faces builder). 258 grid verts vs 768 per patch =
>   2.98x fewer VS invocations on the measured vertex-bound frame; visual
>   pixel-identity rig-verified. Rig FPS A/B invalidated by the random
>   occlusion throttle (see terrain-draw-batching.md rig note; orbital
>   fps = the environment control for future pairs) - operator's live
>   HUD is the real bench. Remaining levers in the design doc: GPU
>   frustum culling, PATCH_TESS 32, tree instancing (reserved arc).
> - DONE voxel-terrain direction doc (docs/design/voxel-terrain.md):
>   Dover cliffs/overhangs/caves/digging as sparse ico-prism voxel
>   OVERLAY on the heightmap + impact-crater ladder; increment 1 =
>   data model + persistence, not yet started.
> - OPEN trenches at highest fidelity (character bobs): hunted at rig
>   vantages (alpine hillside, Seattle spawn at walking height, noon +
>   grazing) - terrain reads SMOOTH everywhere I sampled; cannot
>   reproduce blind. NEEDS the operator's location/screenshot; suspects
>   remain 1 m tile seams (bicubic overshoot) and physics-vs-drawn-mesh
>   divergence (bobbing = collider following the analytic curve between
>   drawn verts?).
> HOMESTEAD ARC COMPLETE (v0.1023.1-v0.1028.1, 2026-07-28): build order
> 1-6 done - the 10-room house is walled, lit, plumbed (first solo
> HotWater loop), furnished (34 pieces, typed-container storage, GLTF
> models), and light switches bill the real power ledger (~330 W with
> everything on vs the 4 kWh/day solo budget - the teaching point).
> Operator walk-through = the acceptance test. Deferred: wall-corner
> seam (own effort, blocks nothing); HvacSystem registration when heat
> becomes a loop.
> HYPER-REALISM ROADMAP (operator asked 2026-07-28 "what would we do to
> make these closer to hyper realistic"): three arcs, sequence water ->
> clouds -> plants. WATER: FFT ocean spectrum (Tessendorf) replacing the
> 6 hand trains + screen-space reflections/refraction with a depth
> buffer + shore-wave sim (breaking, foam advection). CLOUDS: volumetric
> froxel grid + temporal reprojection + wind-field advection of the
> weather map (auto-fixes fly-through clipping AND the "pinned" static
> feel - structures advect + evolve between MODIS refreshes). PLANTS:
> true GPU instancing + octahedral impostors (replaces the card sheet)
> + INSTANCED GRASS (operator 2026-07-28 "how do we get dense grass":
> per-patch instance buffers, crossed-quad blades from a CC0 realistic
> sheet, wind sway in VS, ~5-20/m2 inside ~150 m with distance fade -
> grass is instancing increment 1 since it needs no atlas bake).
> - WATER FFT INCREMENT 1 SHIPPED (v0.1029.1): ocean_fft.rs JONSWAP
>   128x128 on a 64 m tile (tile must DIVIDE the anchor modulus - doc
>   corrected), triplanar VS sampling, buoyancy reads the same array,
>   Settings > Graphics "FFT ocean (experimental)" default OFF. Next
>   rungs: choppy displacement + slope/Jacobian maps (then default ON),
>   weather-driven spectrum regen, second cascade, GPU compute, SSR,
>   shore sim. Operator fork points in docs/design/water-fft.md still
>   open (fetch default / calm residual / storm ceiling).
> - FAR-TREE CANOPY SHEET pulled to default OFF (v0.1029.1; Settings
>   toggle keeps it for A/B). Operator verdict twice: "black squares in
>   a grid" at altitude. Long-range trees now arrive ONLY via the
>   instancing/impostor arc - do not iterate the card sheet further.
> - OPEN varied tree species / bushes / denser grass: deliberately folded
>   into the reserved BILLBOARD-BAKE + TREE INSTANCING arc (fresh
>   session): species variety needs matching far-field cards + atlas
>   entries or the card handoff pops; models are already on disk
>   (bamboo/cactus/bushberries stages under assets/models/plants/).
> - NEXT VEGETATION RUNG (fenced 2026-08-01, wf_82974167 + operator field
>   report; critic-corrected plan in orchestrator_state recent_decisions):
>   [ALL RUNGS 0-6 SHIPPED v0.1086-v0.1093: conifer fallback, pre-wind
>   anchoring, wind class (whole stand leans; furniture guarded), real-
>   scale leaves -> cluster-card crowns (LAI 2.6-3.3), bark full PBR
>   bake, welded+tapered branches, grass strand mat with filler stubble.
>   Resource-budget PIES live (measure increment; allocate+govern fenced
>   in docs/design/resource-budgets.md). Prior partial-status note:]
>   [0/1/3/5 SHIPPED v0.1086-v0.1087: conifer fallback, pre-wind texture
>   anchoring, real-scale leaves at 96 percent budget, welded junctions.
>   NEXT = rung 2 (wind coverage w/ height normalization), then 4 (bark
>   PBR/POM via the UV+bake endgame), then 6 (grass strands + clutter).]
>   (0) SHIPPED-BUILD CONIFER HOLE - procedural fallback for model-backed
>   species, branch BEFORE scl at lib.rs:9633 or fir draws 381 m tall;
>   (1) leaf texture must ride the leaf (sample pre-wind position);
>   (2) wind coverage - type-19 gate + per-type height normalization,
>   card corner shear from the encoded lean value; (3) leaf-scale
>   elements - blades are 10x real size with 45-96% of MAX_TRIS unspent;
>   (4) bark procedural normal/roughness, then POM; (5) seam welding;
>   (6) grass strands + ground clutter. Storm lean gate is annotated
>   CURRENTLY FAILING in vantages.json until (2) lands.
> - ORGAN-TAG FIX shipped first (v0.1081, wf_11009ff9 domain pass): the
>   challenger STOPPED the atlas increment because the black-canopy crux
>   was a tag bug, not missing cards - blade() never set Organ::Leaf, so
>   5 of 8 species shaded foliage as BARK and the v0.1078 transmission +
>   v0.1080 flutter never ran on them. Fixed + gated (unit test counts
>   leaf-tagged geometry; fuji vantage carries a quantified no-black-
>   canopy regression). The ATLAS REGISTRY increment (bake tiles for the
>   6 procedural species; billboard-bake-generalization.md increment 1 +
>   feeding tree_mesh CPU buffers in as BakeParts) remains designed, NOT
>   built - it is the next vegetation rung, and the picket-fence far
>   field is its acceptance test. Tree seam welding rides with it.
> WEATHER ARC STATUS (2026-07-28 afternoon loop): advection (v0.1032),
> precipitation particles (v0.1033), extreme-weather events schema +
> LIVE consumption (v0.1034-v0.1035: eligibility rolls, rarity weights,
> Front gusts on exported wind, HUD event name, event emitters union
> into precipitation). Remaining weather rungs: Vortex spatial wind +
> hazard damage, precipitation streak sprites (particle shader),
> froxels + wind FIELD, rivers. (cloud coverage_boost/tint consumption
> SHIPPED v0.1037.1 per docs/history/2026-07-28.md, stale marker caught
> by the 2026-07-31 clouds domain-pass historian gate. Cloud tonal-range
> fidelity pass shipped v0.1069.0 and was REVERTED v0.1070.0 as BUG-049;
> it re-lands as rung 1 of the CLOUDS RUNGS block below.)

> **>>> CLOUDS RUNGS (2026-07-31 clouds domain pass; strict order, ONE
> commit each, each measured at the probe rig before the next starts).**
> RUNG 0 SHIPPED (v0.1074, this pass): the High-path slab-bounds dead
> write. `cloud_layer_volumetric` never consumed `material.params.w`, so
> below ~400 km altitude the deck marched at 76-128 km instead of its
> designed 25.5-76.5 km - above the visible atmosphere, three times its
> intended distance, at every ground and low-flight view. Fixed by
> `cloud_set_slab_bounds()` at the top of the High path only; the Medium
> path was deliberately NOT touched (Medium consuming params.w for the
> first time is exactly what caused BUG-049, and Medium has ZERO probe-rig
> coverage - no vantage selects it and probe-sweep never sets a cloud
> quality, so every sweep runs the "high" default from src/config.rs).
> The gate vantage `ground-storm-inslab` was repaired in the same commit:
> its global clock 12.0 at lon 138.8 was local ~21:00, so the permanent
> BUG-049 daylight gate had been silently judging cloud lighting in the
> dark since v0.1070.
> 1. WRENNINGE TONAL LADDER (the reverted v0.1069 Step B, on its own).
>    Reason: cloud undersides render white instead of grey because the
>    multiple-scattering third octave is a constant pedestal, flattening a
>    ~100:1 light-march range into ~1.3:1 of lit energy. MUST be
>    re-measured AGAINST THE MOVED DECK - the "19 -> 38 tonal spread"
>    number was measured on the misplaced 76-128 km slab, so it is not a
>    valid baseline any more, and the physical target at this exposure is
>    ~107.
> 2. AERIAL PERSPECTIVE ON THE DECK. Reason: the deck is the only surface
>    in the renderer that never gets it - `aerial_apply` already exists at
>    `assets/shaders/pbr/90-fragment-main.wgsl:11` with its uniforms live
>    in the celestial pass, and clouds are the third surface found
>    skipping it (the water shell was the last one, v0.1053). Measured
>    backwards today: clouds at the horizon are BRIGHTER than clouds
>    overhead. Includes retiring `cloud_low_cam_haze`'s alpha kill.
> 3. CLOUD GROUND SHADOWS. Reason: the shadow is sampled at the fragment's
>    own planet direction (so it never displaces with sun angle), uses the
>    uncarved weather blob instead of the cloud silhouette, and is capped
>    at 35% of albedo where a cumulus removes ~95% of the direct beam.
>    Gets MORE visible now that rung 0 removed the view parallax that was
>    faking the offset.
> 4. TWO-RATE MARCH. Reason: the visible optically-thick skin gets 2-3
>    view samples for a 0.5-1.8 km puff band, i.e. below Nyquist, so close
>    range reads as airbrushed cotton; coarse-then-fine stepping gives the
>    skin 8-12x resolution at roughly unchanged average cost.
> 5. PERF's FOUR SHADER EARLY-OUTS (cloud_weather octave skips, the
>    unreachable clear-sky gate, the cloud_carve pre-fetch threshold, the
>    low-camera haze bail). Reason: -2.2 ms measured at limb-400km with an
>    unchanged image - but RE-MEASURE AFTER THE DECK MOVES: that number
>    was taken on the pre-fix image, and PERF finding 1 rewrites
>    `cloud_weather`, which collides head-on with the ground-shadow rework
>    in rung 3. Land rung 3 first or plan the merge.
> The froxel arc stays RESERVED where it is (the HYPER-REALISM ROADMAP
> block below) - it is the vehicle for a troposphere-scale 0-14 km slab
> with 2-8 km cells, not a rung.

> **>>> BUG-052: Settings VSync OFF panics the app at boot (OPEN, found
> 2026-07-31, NOT a clouds bug).** With `vsync: false` the boot-frame
> settings-apply calls `Renderer::set_vsync` (`src/renderer/mod.rs:1305`),
> `surface.configure` fires while a swapchain image looks to still be
> acquired, and the app panics with "Invalid surface" during world entry.
> Reproduced deterministically twice on v0.1073.1. Two reasons it ranks:
> it is shipped and user-facing (anyone who turns VSync off), and it
> blocks the cleanest perf-measurement path we have - with vsync off,
> frame_ms stops being bounded by the refresh interval (which is why
> blue-marble-12000km reads exactly 16.1 ms in every configuration and
> must never be used for a perf claim). Full writeup + acceptance:
> docs/BUGS.md BUG-052. Likely fix shape: defer the reconfigure to the top
> of the next frame, before the surface texture is acquired.
> MODEL TIERING (operator 2026-07-28): docs/ai/model-tiering.md is the
> plan for spending the never-touched Opus/Sonnet 50% of the sub -
> Sonnet-ready data packages (full dictionary via Wiktionary/WordNet,
> real-plants data, i18n, glossary curation, tools/chemistry depth) with
> hard scope walls (no .rs/.wgsl/schemas). Fable orchestrates + reviews.
> GLOSSARY BLANK-ON-INSTALL fixed v0.1030.1: loader falls back to the
> embedded copy when data/glossary.json is missing next to the exe.
>
> **>>> OVERNIGHT BACKLOG (2026-07-25 ~01:00, operator heading to bed,
> given verbatim then ranked here): IN PROGRESS.** Standing frame: MAX
> GRAPHICS first, then optimize relentlessly without sacrificing fidelity;
> efficiency = more creatures/players on screen (goal: thousands of
> concurrent players in one scene, beating the ~100-player Fortnite bar).
> Ranked queue (status pass 2026-07-25 ~04:00, all shipped same night unless
> noted):
> 1. DONE v0.962.0 - LIFTOFF BUG (operator-experienced): spacebar ascent from the surface
>    does not leave perpendicularly - climbs a bit, flattens into
>    surface-flight-like lateral motion at constant altitude, then
>    suddenly resumes climbing. Unify the surface-to-space flight
>    transition (v0.923 blend-band suspect). Descent is fine.
> 2. DONE v0.963.0 (100 -> 800/cell, ~16k trees/km^2, ~free via sprite
>    cards) - DENSE FORESTS: raise tree density to find the ceiling ("I want to
>    see what we can get away with") - perf-verified via probe sweeps.
> 3. DONE v0.964.0 (depth 20, ~0.6 m verts) - WATER NEAR-FIELD LOD: raise WATER_MAX_PATCH_DEPTH toward depth 20
>    (0.6 m verts; ladder is 17=4.8m, 18=2.4m, 19=1.2m, 20=0.6m, 21=0.3m).
>    Selection is already pixel-driven like terrain - the cap is the limit.
>    Operator target: ~0.5 m near-field, no need below ~0.1 m ever.
> 4. DONE increment 1 v0.965.0 (Detail distances by item type block, live
>    water slider; generic registry continues as an arc) - LOD SETTINGS SECTION: per-ITEM-TYPE LOD registry (planet, tree,
>    furniture, animal, water...) with distance bands (LOD0 0-30 m, LOD1
>    30-100, LOD2 100-500, per-type-appropriate), data-driven
>    (data/vegetation/lod_categories.ron is the seed - generalize), full
>    Settings section rendering it. Tree OBJECTS become data too (fir +
>    variants 1-3 as entries, not code) until procedural plants arrive.
> 5. DONE v0.964.2 (docs/user/creating/, 10 guides + index) - CONTENT-CREATION DOCS, ZERO-PRIOR-KNOWLEDGE AUDIENCE: one doc per
>    content type (planet, vehicle, spaceship, furniture, plant, 3D model,
>    audio file, room/structure, quest, recipe...) - assume the reader has
>    never used a game, social media, or possibly a computer. Good
>    workflow fan-out candidate.
> 6. DONE v0.965.2 (53 docs / 12 categories) - LIBRARY = ALL DOCS: the in-app Library page (native + web mirror)
>    should carry the full docs tree, not just the Accord
>    (scripts/build-library.js currently syncs docs/accord only).
>    HumanityOS as the only app people need to open.
> 7. DONE v0.965.3 (docs/history/2026-07-25-archived-tasks-audit.md; two
>    stale TIER 2 claims corrected) - ARCHIVED-TASKS EVALUATION: find + review archived task lists
>    (journal archives, docs/history) for still-relevant work.
> 8. DONE v0.966.0 (leaf_drift + space_dust, billboard pipeline pair,
>    floating-origin ride) - PARTICLES: (a) leaves drifting through the air near trees; (b) space
>    motion-reference dust (velocity-streak particles so movement reads
>    against black space). particles.ron system exists, data-driven.
> 9. DESIGN ACCEPTED (docs/design/shader-organization.md, 2026-07-25):
>    split the SOURCE into assets/shaders/pbr/ numbered parts concatenated
>    at load, keep ONE module + hot-reload; implementation gets its own
>    fresh session (v0.782 unbootable-release verification bar applies).
>    MEGASHADER MODULARIZATION R&D: split pbr_simple.wgsl into per-domain
>    source files concatenated at load (naga has no include; one module
>    per pipeline is a wgpu reality, but file-level organization + the
>    hot-reload unit can both survive a build-time concat).
> 10. DESIGN ACCEPTED (docs/design/homestead.md, 2026-07-25): house-within-
>     the-greenhouse beside the corridor mouth, 10-room program on the
>     EXISTING InteriorWall/Opening model (equal 0.10 m thickness sidesteps
>     the deferred corner-seam bug), furniture = machine-catalog entries
>     (category Furniture; the layer already gives placement, persistence,
>     cards, containers, GLB slot), fixtures split the aggregate water/power
>     nodes (first HotWater user), ~20 PlacedLights. SIX increments,
>     1-4 data-only; INCREMENT 1 (shell walls in ship_structure.ron) is the
>     next build item. HOMESTEAD DESIGN ARC: fully-fledged player home - walls placed
>     properly, doors, windows, rooms, plumbing, electrical, lighting,
>     rugs, chairs, tables, desks, tools, machines - all data-driven.
>     AFTER it: NPC-AI stress testing (peaceful AI capability ceiling,
>     many NPCs completing real tasks at once) in public spaces
>     (mall/hangar).
>
> **>>> FIELD-REPORT QUEUE (2026-07-25, Fable loop): IN PROGRESS.** Operator
> evening reports rank the queue: [1] trees-near-render DONE v0.955.0 (root
> cause: the green-dominance biome gate rejected brown-green Blue Marble
> texels; shared veg_biome_ok() now gates bake + harvest identically, gate
> autopsy logging + 2 albedo regression tests added), [2] atmosphere limb
> hides terrain: SHIPPED v0.956 (surface-hitting rays keep pure
> transmittance alpha; land readable through blue haze). NOTE 2026-07-26:
> v0.986 misread this want and briefly ADDED a mid-disc veil; reverted
> v0.988 same-day - if a photo-style veil is ever wanted, ask the operator
> first. [3] ocean wave HEIGHT: SHIPPED v0.957 (real vertex displacement). THEN two operator-unblocked arcs (2026-07-25
> decisions): billboard sprite BAKE-OUR-OWN tool (automated 3D-model-to-card
> render, the alpha-card LOD rung; benefits modders, zero manual art per
> model) and the AUDIO ARC (CC0 Kenney.nl download APPROVED; kira engine has
> zero callers today; operator sourcing music separately). Parked: sky
> froxels stage 4. (The tiled-light dark-grid mystery is SOLVED, v0.976.0:
> the celestial pass zeroed light_count so terrain looped zero lights on
> both paths; lights_tiled stays EXPERIMENTAL default-off pending a
> high-count parity + perf pass, no longer mystery-blocked.)
>
> Prior arc: **>>> CLEANUP/STRUCTURE ARC (2026-07-24, Fable): IN PROGRESS.** Shipped this
> arc: v0.931 (ONE reaction palette to data/reactions.json, fixed the relay
> silently dropping native reactions + opening styles to RON), v0.932 (lib.rs
> tiers A+B to src/engine/: 1,040 lines), v0.933 (THE ADMIN MAP:
> data/admin/ops_registry.json, 70 code-verified server-owner actions rendered
> in Server Settings + web Ops; the in-app-ops registry north star READ half),
> v0.934 (the KEYSTONE: EngineState + tier D frame-lock math to
> src/engine/{state,frame_lock}.rs; lib.rs 22,638 -> 20,480 so far).
> NEXT, in order: [1] lib.rs tier C (IPC pollers -> src/engine/ipc.rs,
> manifest ready in scratchpad), [2] server-tools gap #1 from
> ops_registry.json planned[] (first-admin setup without hand-editing a server
> file), [3] remaining infinite-of-x queue (NPC crew/room equipment -> RON,
> web keybinds -> data/keymaps), [4] lib.rs tiers E/F/G. The graphics LOOP
> QUEUE below resumes after this arc. SIGNING: v0.931.0-v0.934.x await
> `just sign-release` (operator-only).
>
> Prior arc (for context): **ASSET ARC STATUS (2026-07-20): ground textures SHIPPED (v0.907.0);
> plants staged.** Done: ambientCG grass/dirt/rock/sand wired as triplanar
> tiling ground materials in type-12 (8-layer array, group-3 bindings 9/10,
> mean-normalized so imagery keeps owning color; neutral 1x1 fallback =
> exact pre-texture look), Settings sliders for sun shadows / god rays /
> SSAO, underwater depth tint + HUD readout, quest-id rewrite, sawmill +
> grain_mill stations, plant repack script (all 6 Poly Haven models merged
> to loader-compatible single-primitive *_merged.gltf).
> REMAINING, ranked (v0.911 shipped: home round-trip dock fix + label
> offsets, 64-light influence cull, real-tree groundwork behind the
> EXPERIMENTAL tree-model-distance slider (default 0; cutout alpha needs
> one operator screenshot at 120 to verify), 4 perf wins, docs/dev suite.
> LOOP MODE (operator-enabled 2026-07-21, one queue item per iteration;
> ranked queue + next pointer live in orchestrator_state.json entries):
> shipped so far v0.915 sun transmittance, v0.916 aerial perspective,
> v0.917 shoreline (depth-baked shallows/waterline/surf), v0.918 exposure
> calibration (three-tier dome + multiple scattering; killed the washed
> sky AND the grazing-white water) + BUG-047 (planet-detail setting sank
> the sky shell underground), v0.919 synthesized heightmaps (Moon/Mars/
> Pluto get real chunked-LOD cratered ground; the "icosphere stepping"
> was bodies with no heightmap riding the bare uniform sphere), v0.920
> geomorph fades (LOD swaps dissolve via complementary Bayer crossfade;
> RenderObject.fade rides the model matrix w-row - the tree/animal LOD
> ladder can reuse the channel). 2026-07-21 operator field-report batch
> shipped: v0.921 god rays respect planet occlusion, v0.922 ocean near-
> field rework (tiling mipped wave texture replaces aliasing analytic
> shading; sea-pin fix; 3ms terrain-build cap kills the descent hang),
> v0.923 planet-frame momentum on liftoff (radial Space thrust + full
> blend-band ride) + vegetation LOD stage 1 (trees DEFAULT ON at 120m,
> bare-forest guard, model+silhouette sliders). Queue next: bookmark
> studio arc, then LOD ladder rungs 2+ (billboard mid-stage, grass far
> cutoff needs a grass bit in packed UV, shrub/animal categories,
> per-stage crossfades), light clustering, audio engine.
> NEW TOP ITEMS:
> 0a. LOD ladder proper (operator design): per-SIZE-CATEGORY render
>     distance sliders (grass short, trees miles, ant vs beast same idea)
>     with stages billboard -> alpha-mapped card -> full model. The card
>     system + near_tree_instances are the first two rungs; needs the
>     alpha-card middle stage + per-category settings + animal hookup.
> 0b. DECOUPLING steps 3-5 (audit in journal): fold the ~15 home
>     singletons into StructureInstance, data/structures.ron frames
>     (orbit/surface/free), player as independent FrameRef entity.
>     Steps 1-2 (current-frame docking + label offsets) shipped v0.911.
> v0.909 items (ALL DISPATCHED as of v0.980, 2026-07-26 - kept for the
> paper trail; 1 stale-closed, 2 shipped-closed, 3 blocked on the operator
> GLB answer, 4 re-scoped post-BUG-048, 5-8 shipped v0.977-v0.980):
> 1. TELEPORT-OVER-DEEP-OCEAN: CLOSED as already-fixed (verified live
>    2026-07-26): placement parks on sea level since v0.896, the HUD Alt
>    + band reference since v0.909.x (alt = dist - max(ground, sea)).
>    Rig proof: mid-Pacific park at 10 m reads Alt 5 m - the km-scale
>    disagreement is gone; the residual is wave/lift convention (fine).
> 2. AUDIO ENGINE INTEGRATION: CLOSED as shipped (v0.960 CC0 sounds +
>    honest volume sliders synced to kira; v0.968 footsteps). Remaining
>    audio wants are new scope: broader SFX coverage + music playback
>    when the operator sources tracks.
> 3. Garden crop hero models: Quaternius Ultimate Crops (CC0, growth
>    stages) for the 134-crop coverage; potted_plant_01_v1 (176k tris)
>    as an interior hero pot. The type-19 + decorations.ron pipeline is
>    ready; crops need growth-stage swap wiring in the farming visuals.
> 4. Cloud raymarch polish: underside banding UNREPRODUCED as of
>    v0.974.0 - the "invisible underside" turned out to be BUG-048 (the
>    v0.958 absolute-slant fade hid the whole deck from the ground;
>    fixed with a grazing-ratio fade). Now that undersides render at
>    all, re-judge banding under a thick daylit deck before adding step
>    jitter. Deck-interior FPS dips to 10-16 (march cost) still stand.
> 5. Grazing-angle texture smear: explicit-LOD sampling bypasses the
>    aniso sampler; use textureSampleGrad or footprint-anisotropy bias.
> 6. Forage flora spawns: stationary-creature flag so berry-bush/wild-
>    flax rows work (2026-07-20 data-agent report, journal).
> 7. Quest Travel objective emitters (dated note in exploration.ron).
> 8. Settings duplicates cleanup: the Notifications card + Wallet network
>    selector edit dead fields (live paths live in chat DM cog / wallet
>    page state); either wire or remove (2026-07-20 audit, journal).
> Perf headroom: the operator is vsync-capped at 120 FPS - push quality.
>
> **>>> POST-AUDIT QUEUE (2026-07-19 late; from the 4-subagent audit wave).
> STATUS SWEEP 2026-07-26: 1 CLOSED (mostly stale; Talk emitter + id
> lockstep shipped v0.981), 2 CLOSED (resource nodes v0.982), 3 CLOSED
> (stale - shipped v0.905.0 same day as the audit), 4 BLOCKED on the
> operator GLB answer, 5 CLOSED (sawmill/grain_mill shipped v0.907;
> vehicle_assembler placed v0.982.2 - vehicle recipes now craftable),
> 6 PARTIAL (tint + HUD depth shipped v0.903/v0.907; residue = swim speed
> cap + bubbles, polish-tier). Lesson recorded: audit findings fixed
> same-day were never struck from this queue - strike on ship, always.**
> 1. QUEST REWRITE: tutorial/construction/exploration/farming.ron are
>    ~80% dead ids (items/recipes/blueprints that do not exist; the 20
>    Build objectives match none of the 12 real blueprints). Rewrite to
>    real ids; add Travel/Talk objective emitters (none fire today).
> 2. FORAGE FAUCET: wood_log/stone_raw/sand/clay/salt/oil/hides have NO
>    gather source - the whole tech tree hangs off the vendor. A chop/
>    forage/quarry verb on planet surfaces is the unlock (351/362
>    recipes currently unreachable without vendor purchases).
> 3. PLANET TEXTURES: albedo path is body-agnostic; bake Moon/Mars/Pluto
>    from the USGS PD maps (links + G1-G3 gotchas in
>    docs/reference/asset-and-map-sources.md + journal); Pluto needs a
>    PlanetDef first; gas giants get the type-18 procedural band shader
>    (also fixes uranus/neptune rendering gas-giant ochre).
> 4. GARDEN HERO MODELS: consider the CC0 Quaternius Ultimate Crops pack
>    (growth-staged GLTF) to replace procedural plants for hero crops -
>    operator reviews links in docs/reference/asset-and-map-sources.md.
> 5. Missing stations: sawmill + grain_mill machine types do not exist
>    (2 recipes uncraftable); vehicle_assembler defined but unplaced.
> 6. Underwater polish: depth-graded tint/fog, swim speed cap, bubbles;
>    HUD depth readout (Alt currently shows height above SEAFLOOR when
>    submerged).
>
> **>>> FABLE FINAL SPRINT (2026-07-19 day; v0.897-v0.900).** Morning field
> reports answered: FLICKER ROOT-CAUSED AND KILLED in v0.898 (the 256 MB
> patch cache was sized for 640-leaf budgets - at 6144 the needed set
> outgrew it and every build evicted a still-needed patch; now 1.5 GB + a
> recency guard + drawn-keyed split hysteresis + a committed-split budget
> tier; probe-proof: five seconds of byte-identical selections parked at
> 6144). v0.899 shipped the first REAL SUN SHADOW MAP (4096 near-field
> ortho, texel-snapped, PCF; terrain/trees/ocean-waves/home all cast and
> receive; probe-verified tree shadows at Oahu). v0.897 made vegetation
> PLANET-FIXED (same plants at every LOD - splits no longer reshuffle the
> forest) and screen-blended the god rays (no more cloud blowout) with
> live-overcast dimming. v0.898 also: cloud ground shadows (terrain darkens
> under the sky-drawn coverage field) + land detail octaves to 8 m.
> REMAINING environmental-graphics wants, in rough order: SSAO-on-ambient
> (v0.901 shipped SSAO; v0.1100 rebuilt the estimator normal-aware after
> BUG-062's tree aura — the remaining rung is applying AO to the ambient
> term INSIDE the PBR shader instead of multiplying the tone-mapped frame,
> which needs a depth prepass so the main pass can read the AO texture;
> until then AO attenuates direct sun too);
> tangent-space ground detail TEXTURE below 8 m (unit-dir noise quantizes
> below that); geomorph/fade at LOD swaps if any residual pop bothers;
> cloud-shadowed god rays; Settings toggles for sun shadows +
> godray_intensity (renderer fields sun_shadows / godray_intensity exist,
> default on).
>
> **>>> OVERNIGHT LOOP RESULT (2026-07-19, operator asleep; v0.889-v0.896, 8 feature releases).**
> The whole bedtime list shipped, then self-directed polish. SHIPPED: terrain
> prefetch (move-flicker fix) + surface-clamp LOD slop (see-through-Earth fix)
> (v0.889); F6 location bookmarks (exact pose to debug/bookmarks.json, restore
> via camera_request {"bookmark":"bm-N"}) + Q/E flight roll + 100-1000 km
> partial co-rotation velocity blend (v0.890); 4x FRAME RATE from draw
> submission batching, 19.8 -> 78.6 FPS measured same-scene (v0.891); patch
> budget ceiling 6144, default 3072 (v0.892); 7 cloud families adding
> altocumulus/cumulonimbus/nimbostratus (v0.893); trees at every LOD depth
> (they used to vanish up close) at constant area density, 4x tree density,
> ocean camera parking (v0.894); GOD RAYS (depth-marched shafts) + camera
> {"aim":"sun"} staging rig (v0.895); imagery-green biome gate (no more
> Sahara trees), vegetation lit like the ground, radius-based ocean-park
> backstops (v0.896). NEW DEV RIGS: portable perf probe (scratchpad copy of
> the exe + junctions, offline autopilot world - measures FPS via
> screenshot_done.json), aim-at-sun captures. FOR THE OPERATOR TO JUDGE
> (morning taste pass): tree/grass density + card look up close, god-ray
> strength (godray_intensity 0.55), 7-family cloud variety, silver-lining
> strength. STAGING NOTE (corrected): the sun light is constant-intensity
> with the REAL astronomical direction (no hour-based dimming - an earlier
> overnight diagnosis was wrong). Staged local noon = game hour 12 -
> east_lon/15 (UTC-independent; sun_az cancels in planet_spin_from_time).
> Verified bright at Sahara/Iceland; Oahu/Rainier staged shots still read
> dusk-dim for unpinned reasons (suspect high-latitude sun elevation +
> tone curve, or a spin-model detail + the probe 77x clock racing). The
> {aim:sun} rig is verified exact (Everest god-ray capture).
>
> **>>> TIER 0 NEWEST (operator, 2026-07-14 eve): ALL-IN-ONE + INLINE-FIRST + WEB PARITY.**
> A direction statement while asking how to SSH (he'd let the AI do all VPS ops for
> months). Full context in memory `feedback_all_in_one_inline_first.md`. The stance:
> the app must be genuinely all-in-one because "loading any other app is a potential
> failure point" (a SAFETY argument), everything a 5-year-old could use, explained
> inline on the thing, not in manuals/modals. Concrete queue, ranked:
> 1. **SHIPPED (v0.858.0):** in-app VPS Console in the Relay Control Center - runs
>    server commands over SSH from the app (one-click status/restart/logs/disk/memory
>    + a free command box), so admin no longer means a second terminal. Shells to the
>    OS `ssh` with the `humanity-vps` alias + the operator's existing key.
> 2. **SHIPPED (v0.859, persistence gap closed v0.1066) - header word-wrap +
>    hint-display modes.** (a) DONE both sides: the top nav wraps overflowing buttons
>    into a second row instead of hiding them (native `horizontal_wrapped` in
>    escape_menu.rs; web `flex-wrap` on `.hub-nav` with a ResizeObserver keeping the
>    spacer in sync). (b) DONE both sides: the icon+text / icon-only / text-only cycle
>    (native `NavDisplayMode`, web `data-navmode`), default icon+text as the operator
>    wanted for screenshots. It lives as a cycle button in the nav itself rather than a
>    Settings toggle, on both sides. **v0.1066 closed a real parity gap found on
>    re-audit:** web had persisted the choice since v0.859 but native cycled it
>    in-memory only, so it silently reset to icon+text on every restart. Now stored in
>    `AppConfig.nav_display_mode` and saved on click, guarded by
>    `nav_display_mode_survives_a_config_round_trip`.
> 3. **In-app documentation / tutorials / walkthroughs / AI guides.** Ship the docs
>    INSIDE the app (data-driven, like the Library already does for the Accord), so
>    there is no external manual to lose or fail to find. Tech-illiterate-first.
>    **Head start (audit 2026-07-30): `data/onboarding/core_pages.json` (8 plain-language
>    page descriptions) and `core_concepts.json` (4 concepts) already exist and are
>    exactly this content, but NOTHING renders them on either client.** A comment in
>    gui/mod.rs claimed the web /onboarding page read them; it does not, it fetches
>    quests.json only. Wire them into a surface rather than rewriting the content.
>    Note core_pages.json predates the v0.1063 page split, so it lists 8 pages and
>    does not mention Library, Tools or Platform. Also unrendered:
>    `data/onboarding/sim_guides.json` (27 in-game guides, 20 of them still unwritten),
>    split out of the old resources.json in v0.1064.
> 4. **Inline-first hints everywhere.** Put the instruction ON the widget/machine/
>    button, controlled by the hint-display modes from (2). Get close, not absolute.
> 5. **Web near-pixel parity with the app.** He shared a side-by-side and wants the
>    website chrome (nav, chat layout) to mimic the native app as closely as possible.
>    PROGRESS (2026-07-16, largely DONE): v0.861.4 removed the web Real/Sim toggle
>    (native killed it in v0.197.0; separation is by navigation, docs/design/
>    two-realities.md). v0.861.5-8 aligned the accent (#FF8811 -> #ed8c24 sitewide)
>    and migrated nav-legacy hex to theme tokens. v0.861.9 OPERATOR-GREENLIT black:
>    presets.json themes.dark now carries native's exact neutrals, plus the sitewide
>    galactic-core space background (our own 25M-star bake, faint, drift + scroll
>    parallax, Settings toggle). v0.861.10 app-style nav pills (category borders per
>    the native nav_group recipe, 5 categories). v0.861.11 landing redesign: 7-screen
>    picture-book scroll, real screenshots, mission essay moved verbatim to /mission.
>    REMAINING: (a) swap landing screen 2's cosmos stand-in for a composed live 3D
>    capture (needs the operator in-game for 60 seconds); (b) chat-page layout parity
>    (the deepest surface, untouched this pass); (c) consider the faint galaxy bake
>    behind native egui pages so native mirrors the web treatment back.
>
> 6a. **TARGET MARKERS (operator design, 2026-07-18; v1 SHIPPED v0.885).**
>    DONE: home-station ring + look-label + distance in-world, Cosmos
>    "Stations" section with a persisted Track toggle, tile-aware altitude
>    parking. REMAINING: planets/asteroids/ships/enemies selection, the
>    construction-mode respawn/teleport-point marker, web-maps parity.
>    ORIGINAL DESIGN: Construction mode gets a
>    respawn/teleport-point marker (Stargate-style teleporters MUCH later).
>    Maps page: click-select the station / planets / asteroids / spaceships /
>    detected enemies -> in game a RING encapsulates the selected object and
>    looking at it shows its label (generalize the machine Tab-label pattern).
>    Everything selectable must appear on the Maps page too. Station glint +
>    HUD marker is the v1 slice.
> 6. **PLANET + OCEAN arc (operator, 2026-07-16 night; heavy progress 2026-07-17).**
>    SHIPPED so far: v0.871 streamed 460 m ETOPO tiles (real mountain shapes, Fuji
>    is a cone); v0.872 surface-lock jitter fix (f64 spin) + graduated altitude
>    bands (walk/co-rotate/blend/inertial, fixes the 10-mile desync + dead FTL
>    wheel); v0.873 true-3D cloud noise (seam fix) + async 192^3/128^3 volumes +
>    Planet LOD settings (Settings > Planets: split px / patch budget / stream
>    speed); v0.874 LIVE WEATHER - NASA GIBS MODIS cloud fraction fetched in the
>    background (30 min refresh, APPDATA cache, offline fallback), decoded via the
>    official palette to an RG8 mask the megashader blends as cloud PLACEMENT
>    (procedural octaves carve structure inside real masses; validity channel
>    falls back to procedural; Settings toggle). Verified against the satellite
>    reference: clear Sahara/Arabia, ITCZ band, Europe cloud all match in-game.
>    v0.875 shipped the 1 m TERRAIN LADDER - PatchId.path u32 -> u64,
>    tile-tier depth cap 16 -> 20 (~0.42 m triangles within ~30 m of the
>    ground; depth 19 / 0.84 m at the default patch budget, the full tier at
>    the Settings 768 ceiling), fine-octave ladder extended 125 m -> 1 m
>    wavelengths (Nyquist gates 14..20, amplitudes tapering to rock-scale
>    wrinkle), regression tests pin the descent and u64 path integrity.
>    v0.876 shipped OCEAN Stage 1 (the split): terrain = true bathymetry
>    under a translucent water shell (material type 16, vertex wave
>    displacement + Fresnel/glitter shading), CPU wave twin locked to the
>    shader by a constants guard test, player floats on sea + wave height.
>    Tuning debt: shallow-shelf banding through the alpha, patch-edge
>    shading steps, underwater tint (Stage 3 diving).
>    NEXT (in order): (a) REAL OCEANS remaining stages - design in docs/design/ocean.md: ocean mask
>    (flood fill; keeps Death-Valley-type below-sea-level basins dry), Gerstner
>    wave surface drawn == sampled, swimming, Archimedes buoyancy (sail ships),
>    depth pressure + hull ratings (submarines), analytic impact displacement
>    (spaceship crashes, asteroid drops). (c) DONE v0.881: HOMESTEAD DECOUPLED into a real 400 km LEO orbit (ISS-like; player frame rides it aboard; scene renders at the orbital offset when away; no more orbit-screenshot photobomb). Follow-ups: Return-home targets the station, ground-visible glint, Map orbit display. Previously the
>    known surface bug (home wheels around the player frame on the ground; it
>    photobombs every orbit screenshot too). (d) DONE v0.878: sun-frame lighting unified (game
>    clock vs subsolar longitude drifts with spin - hard to stage lit captures).
>
> The production-readiness + go-live focus below is DONE for this pass (streaming
> shipped v0.853-857, UI-audit closed v0.855-856, TURN + watch v0.857). This
> all-in-one arc is the new top of TIER 0.

> **>>> TIER 0 PRIOR (operator, 2026-07-13/14): PRODUCTION READINESS + GO LIVE.**
> The operator redirected: "We really need to start getting it production ready.
> Make it all beautiful. Everything easily described, accessible." And: "It'd be
> cool if I could start using the streaming software and chat." This serves the
> funding goal directly (his own framing: shipping content earlier means social
> posts earlier means donations earlier, under a $1k/mo income cap).
>
> WORK ORDER:
> 1. **SHIPPED** the production-polish pass: web-chat security-modal CSS fix
>    (v0.844.1), native User-Profile modal redesign (v0.845.0), donate reframed
>    around the maintainer + a curated charities list (v0.845.1-v0.847.0), Relay
>    Control Center (v0.846.0), Notes/Calendar rescued + dead code removed
>    (v0.848.0), crafting.html real recipe browser (v0.848.1), 8-page tokenize +
>    accessibility sweep (v0.848.2), civilization Sim fake-data killed (v0.848.3),
>    the long-standing modal-backdrop click-steal bug fixed (v0.849.0), relay +
>    profile pages to one scroll (v0.849.0), crafting condensed to columnar rows +
>    Studio live-chat panel (v0.850.0), relay admin-stats 414 fixed via POST body
>    (v0.851.0).
> 2. **SHIPPED (v0.855.0)** most of the docs/UI-AUDIT.md backlog: native + web
>    profile editors each collapsed to one; the orphaned Civilization dashboard
>    rescued into the Humanity tab; bug reports wired to the relay (they were NOT
>    persisting); the two web identity restore flows folded into one tabbed modal;
>    ~62 theme literals migrated (LEGACY_OFFENDERS 17 -> 12). STILL OPEN: UI-AUDIT
>    s6 (web civilization.html Sim mode still shows fake colony stats) and s7
>    (remaining literal tokenization: showUserContextMenu, planet tooltip,
>    calendar/onboarding). LEGACY_OFFENDERS is now 12 files, keep shrinking to zero.
> 3. **THE BIG ONE - STREAMING TRANSPORT: SHIPPED (v0.853-0.855) and LIVE.** Studio's
>    "Go Live" now really broadcasts: non-blocking GPU capture -> downscale + JPEG on
>    a worker thread -> binary WebSocket -> relay fanout -> the public /watch page.
>    Self-hosted to the operator's own relay, no third party. MJPEG for v1 (zero new
>    deps); an adversarial review found + fixed 8 bugs. Full design + the encoder
>    tradeoff table in docs/design/streaming.md. NEXT RUNGS (not yet built):
>    Rung 2 = hardware H.264 via the `windows` crate + Media Foundation MFT (NVENC on
>    the operator's RTX 4070, no C toolchain, Windows-only first) + real screen/camera
>    capture; Rung 3 = HLS, because VPS egress (not the protocol) is the scaling
>    ceiling. Rung 2 is the highest-value streaming follow-up.
>
>    OPERATOR-ONLY pending for streaming: (a) sign releases v0.853-v0.855 so desktop
>    auto-update offers them; (b) rotate the TURN credential committed in plaintext at
>    src/net/webrtc.rs + web/chat/chat-voice-rooms.js and move to short-lived TURN
>    creds (found during the streaming research); (c) OPTIONAL nginx one-liner for a
>    /watch pretty-URL (works today as /watch.html; the sed edit was blocked by the
>    auto-mode classifier and needs the operator to run it or approve it).
>
> The co-presence pivot below REMAINS the strategic bar (playable multiplayer with
> a second real human); it is paused, not cancelled, while production-readiness and
> the ability to broadcast land.

> **>>> TIER 0 - THE PIVOT (operator, 2026-07-08, Opus era): VALIDATION +
> REAL CO-PRESENCE, not more breadth.** After a long build streak of
> deep-but-unwitnessed features (the ship corridors the operator found
> visually broken on first real play are the canary - tests cannot catch
> what needs eyes), the focus is now: get a SECOND real human into the
> world and prove what exists holds up when a person touches it. The
> mission is uniting humans; the near-term bar is playable multiplayer on
> the VPS; everything built (mall, trade, chat, ship) only matters with
> someone else present.
> WORK ORDER:
> 1. UNIFIED CHAT - same relay chat in-game / Chat tab / website /
>    livestream. SHIPPED: 1a (v0.771) read-only bottom-left in-game feed;
>    1b (v0.772) the panel is now INTERACTIVE - Enter opens a compact
>    bottom-left chat panel that frees the cursor + disables look/move,
>    with a channel switcher, active-channel message list, and a focused
>    input that sends via the same path the Chat page uses. The v0.771
>    feed now FOLLOWS chat_active_channel (fixes the field-report bug where
>    switching to #announcements never updated the header or loaded its
>    messages) and hides while the interactive panel is open. NEXT: 1c view
>    modes inside the in-world panel (all-chat / DMs / group chats /
>    options - today the full Chat page nav tab is still the home for those),
>    then the server-join flow so the VPS appears in the launcher Servers list.
> 2. SHARED-WORLD CO-PRESENCE - actually see other avatars. FINDING (v0.774):
>    the mechanism is ALREADY BUILT END-TO-END. In-world + connected, the
>    client auto-sends game_join over the chat socket, streams position 15Hz,
>    and net_sync renders remote avatars; the relay validates + broadcasts
>    welcome/joined/position/left. There is NO launcher join step - it is
>    automatic (the "needs join flow wired" note was stale). What was missing:
>    it was INVISIBLE. v0.774 made it legible - a HUD top-left indicator
>    ("Shared world - <host>" + a live roster of who else is present) + real
>    names both directions. REMAINING = the actual TWO-CLIENT LIVE TEST: two
>    native instances with DISTINCT identities, both in-world + connected to
>    united-humanity.us, should see each other's avatars and the roster count
>    tick up (web chat is NOT a co-presence participant - no game_join/position).
>    PROVEN LIVE 2026-07-10 (v0.794.1): two autopilot-driven instances joined
>    united-humanity.us simultaneously; game_players: 2 on /api/server-info,
>    each client's HUD roster showed the other, logs show bidirectional
>    position streams. The v0.793 dev AUTOPILOT (debug/autopilot_request.json:
>    zero-click ephemeral identity -> connect -> enter world) makes this
>    repeatable without a second human; it also CAUGHT + fixed a real race
>    (v0.794.0: game_join sent before the identify handshake bound the socket
>    was silently discarded - any fast-loading client could hit it). A second
>    HUMAN witness test remains wanted (operator-gated). Follow-up: the roster
>    shows "Player" instead of the game_join player_name.
> 3. FIELD-REPORT CADENCE - operator plays, reports what is broken/ugly, I
>    fix. DONE from this cadence: dev spawn + walk-up creature editor
>    (v0.777-778), the 15-fix review sweep (v0.779), lighting arc (v0.780-781:
>    glass night-glow fix, caps 500/200m, physical fixtures, real strip lights
>    with sharp/smooth corner paths). DONE: the corridor rework (v0.788) -
>    corridors OWN their door mouths (from_zone/to_zone/lat/width/door_w/h/
>    glass_top; cuts its own apertures; no authored-door references; BOTH the
>    move-the-door-desync AND the coincident-wall z-fight/walk-through fixed
>    at the root; shipped RON migrated; regression tests encode both bugs).
>    DONE (2026-07-10, the v0.789-799 loop day): corridor pass-through +
>    zone-drag guard (v0.789); build gizmos (v0.790); SAVE SAFETY +
>    Settings > Gameplay + orbit alignment (v0.791); strip corner
>    subdivision + emission along the curve, rotation-only rings + icosphere
>    radius, Dev teleport/FTL (v0.792); docs sweep + LICENSE (v0.792.2); dev
>    AUTOPILOT zero-click world entry (v0.793); game_join identify-race fix
>    (v0.794); corridor pocket DOORS with live colliders (v0.795); roster
>    lazy-spawn name fix (v0.796); NPC walk-up dialogue (v0.797); stars.bin
>    1ms parse, rung 1 (v0.798); in-world chat VIEW MODES + the
>    Dev/Creative/Normal PLAY-MODE system (v0.799, task #50 closed).
>    NEXT QUEUE: star rung 2 (ATHYG 2.5M in-app download); NPC dialog lines
>    to data files (infinite-of-X - they live in relay populate_ship);
>    corridor door glass strips; first-contact funnel polish; server-side
>    play-mode permissions for shared worlds. Earlier cadence DONE: lights
>    UNCAPPED (v0.782), real constellations (v0.783), palette lights + RGB
>    launcher cards (v0.784).
>    QUEUED (renderer/data): the BIG STAR CATALOG arc. Key analysis
>    (2026-07-10): Gaia DR3's terabytes are ~99% metadata; the 4 render
>    fields (position/mag/color) pack to ~8-16 bytes/star, and ESA's TAP
>    service does COLUMN SELECTION server-side, so we never download the
>    terabytes. But 1.8B individual GPU points is pointless: below ~mag
>    13-14 stars are sub-pixel and merge into diffuse glow - the RIGHT end
>    state is points + a BAKED all-sky glow map (that glow IS the visible
>    galaxy). Ladder: (1) binary star format + single parse (34 MB HYG CSV
>    parses TWICE at startup today); (2) ATHYG ~2.5M stars (same schema,
>    in-app "extended catalog" download, ~40 MB binary); (3) Gaia extract
>    G<14 (~25M points, ~300-500 MB, chunked TAP pulls of 4 columns) + a
>    baked HDR glow cubemap integrating the remaining ~1.77B faint stars -
>    visually equivalent to rendering all 1.8B at ~1/50th the size.
>    STATUS 2026-07-11: THE WHOLE LADDER SHIPPED (v0.789-v0.817). Point
>    tiers 120k/2.5M/16.8M with in-app downloads; glow baked from the REAL
>    Gaia census - level-10 8192x4096 in-repo default (v0.810.2) + level-11
>    16384x8192 Ultra as an in-app download tier (v0.817.0, assets-glow-1);
>    star halos; packed 12B StarVertex; 16K/8K/4K wallpapers published
>    (assets-wallpapers-1). Level 11 is the deliberate END of the ladder
>    (finer cells go Poisson-noisy; 16384 is the GPU texture ceiling).
>    SAME ARC (v0.810-v0.816): planet visuals field reports closed -
>    per-pixel Blue Marble Earth (first texture bind group), volumetric
>    cloud raymarch, atmosphere close-range exposure fix, the asin LOD
>    root-cause fix (chunks could NEVER activate before), ocean/land
>    grading, hi-res screenshot capture (4K/8K while playing 1440p), water
>    shader (v0.818, 6-octave waves + fresnel + sun sparkle + land detail),
>    and
>    the camera_request.json dev tool (scripted look-anywhere captures).
>    ** THE REAL NEXT UNLOCK (finding 2026-07-11): the water shader is
>    shipped and correct but its wave detail is UN-WITNESSABLE from any
>    reachable viewpoint - every octave fades by pixel footprint (keeps
>    orbit smooth) and the lowest the camera reaches (~9 km) still views the
>    sea from km away. The ocean is a smooth sphere, no walkable surface,
>    flight stays far above wave height. So "stand on the beach and watch
>    waves/tides" is a GET-TO-THE-SURFACE problem, not a shader problem:
>    (a) sub-orbital descent / low-altitude flight to near sea level, (b) a
>    near-surface ocean LOD that keeps wave + foam detail alive close up,
>    (c) eventually a walkable shoreline + tide sim. This is higher-leverage
>    than more water/cloud shader tuning and gates the operator's biggest
>    dream. ** Lower-priority polish (task #75): cloud sub-135km octaves +
>    Nubis-style volumetric clouds (a renderer agent's cloud_noise.rs +
>    bind-group work is PRESERVED unmerged in worktree agent-a1a5aa9f96a972f77,
>    died mid-wiring at the Fable spend cap - revive in a fresh budget
>    window), ocean/land close-range content, cloud ground shadows. LATER
>    R&D: sky FILTER modes (UV / H-alpha / infrared layers for gas clouds +
>    nebulae); per-point light sampling along strip paths.
>    ** OVERNIGHT REALISM ARC (operator mandate 2026-07-11 "get the
>    environment as close to real looking as possible"; Opus, 2026-07-12): **
>    THE "GET-TO-THE-SURFACE" UNLOCK ABOVE IS SHIPPED - **v0.829.0 surface
>    mode** (task #76 DONE): within ~10 km of a planet the camera flips to a
>    radial-up tangent basis (down = planet centre, level horizon), gravity
>    settles the eye to standing height on the real heightmap ground, WASD
>    walks the tangent plane; above the engage altitude orbit is INERTIAL
>    (starfield fixed, planet turning - ISS view). src/surface_walk.rs (pure
>    glam, 8 tests) + Camera.surface_mode/surface_up. VERIFIED: level sea
>    horizon over Oahu; marble + Milky Way from orbit.
>    **v0.830.0 wispier clouds** (salvaged the paused a202 clouds agent, re-
>    verified myself): ridged-Perlin cirrus filament octave + four cloud-type
>    regimes (cirrus/cumulus/stratus/stratocu); earth.ron base coverage 0.55
>    -> 0.42 so Earth reads as partly-cloudy blue, not a white shroud.
>    **FOV-collapse guard** (v0.829.0): camera fov clamped 60..120 on apply -
>    a "fov": 0.0 config used to black out the whole 3D scene (root-caused a
>    poisoned portable config that made every verify capture black; see
>    journal 2026-07-12 + memory dev_workflow_tooling).
>    REALISM QUEUE (ranked, all fresh renderer features - do with full
>    attention + boot-verify EACH, the device-limit gotcha is real):
>    (a) SUN corona/flare - the Sun is a flat white disc with a hard edge
>        ("looks like the moon"); needs a soft additive corona shell (mirror
>        the atmosphere-shell pattern near lib.rs:13510) + stronger bloom
>        pickup. Celestial draw uses state.sun_material (lib.rs:13473);
>        sun_halo_material exists (lib.rs:6065) but is NOT drawn in that pass.
>    (b) MOON surface realism - currently a flat grey ball, fully lit; wants
>        maria (dark basalt patches), cratering, and a real terminator/phase.
>    (c) WEATHER (procedural, math-first per operator): sunny/overcast/light+
>        heavy rain/snow/ash x wind (none/low/high/extreme) + changeable wind
>        direction -> foliage/particle motion, fog, lightning (ground strike +
>        sky spider). Scoped OUT for now: hurricanes/tornadoes. Big feature -
>        design a WeatherState + particle + shader pass; start with 1-2 states.
>    (d) PLANTS - free 3D real-plant catalogs OR procedural L-system; Silverdale
>        WA evergreens/pine/douglas-fir first. Research + a spawn/scatter system.
>    (e) TERRAIN icosphere-triangle mosaic still faintly visible - smooth/subdiv.
>    ** DEV TOOLING SHIPPED (operator, 2026-07-12, "build the dev tools to make
>    your development as easy as possible"): ** v0.831.0 STAR-CATALOG TIER TOGGLE
>    (Settings > Graphics > Render tier + HUMANITY_STAR_TIER=standard env -> dev
>    boots load the 120k stars.bin in ~1 ms instead of the 252 MB / 25M Ultra
>    catalog; ~5 s/boot). v0.831.0 BOOT ANALYTICS (src/boot_timing.rs -> log
>    summary + debug/boot_timing.json). v0.832.0 Dev Travel "LAND ON SURFACE"
>    (per-body button -> drop to the surface + surface mode; fixes the GUI-
>    teleport-parks-in-orbit / planet-spins-underneath report).
>    ** BOOT SPEED (found by the analytics, NOT a guess): ** renderer_init was
>    ~32 s of the ~40 s cold boot = Naga compiling the pbr_simple.wgsl MEGASHADER
>    into 3 PSOs ~10 s each, SEQUENTIALLY. v0.834.0 PARALLELIZED those 3 compiles
>    (std::thread::scope; wgpu Device is Send+Sync) -> renderer_init 32 s -> 19 s
>    (~40% cut, measured). Further boot gains are bigger + riskier (diminishing
>    returns): (a) wgpu PIPELINE_CACHE disk cache = Vulkan/Metal ONLY, needs a
>    whole-renderer DX12->Vulkan backend switch; (b) split the megashader so each
>    PSO compiles less; (c) async pipeline compile (show world while compiling).
>    Also queued: renderer_init sub-spans in boot_timing (shader vs device), a
>    headless click-test for the "Land on surface" + new Dev-Travel buttons.
>    ** SURFACE LOCK (operator field reports, v0.833.0): ** the surface-mode
>    co-rotation was a per-frame NO-OP (re-captured the anchor + re-placed it at
>    the same spin = identity), so the planet slid out from under a standing
>    player ("the Earth spins without me... clips through me"). FIXED: capture
>    the anchor ONCE on engage, co-rotate the persisted anchor thereafter;
>    verified at Puget Sound (land features held still across frames). LESSON:
>    re-verify surface work at spots with LAND, never featureless ocean (the
>    Oahu ocean verify hid the slide).
>    ** SURFACE FIELD REPORT 2 (operator, 2026-07-12): ** investigated (workflow
>    wf_c420bd2e). SHIPPED v0.835.0: SEE-THROUGH GROUND (the eye clamp used the
>    coarse base heightmap but the drawn mesh adds ~4x-exaggerated detail up to
>    ~120 m, so the 1.7 m eye sank below the surface + backface-cull -> see-
>    through; fixed via a shared planet_chunks::drawn_elevation_normalized;
>    verified over the Rockies) + SPEED CLIFF (wheel did nothing on the ground
>    then one notch flung you to orbit; fixed by gating the FTL fly-integration
>    off in surface_mode + folding a bounded wheel mult into surface speed).
>    ** NEXT (headline, PLANNED, not yet done): HOMESTEAD DECOUPLING. ** The home
>    has no world position of its own - every home mesh draws at Vec3::ZERO in the
>    floating-origin frame, and ship_world_pos is BOTH the player frame origin AND
>    the home position, so the home is glued to the player and (post the v0.833
>    co-rotation) appears to spin on the surface. PLAN: add EngineState.home_world_pos
>    (init it + ship_world_pos to the GEO vector at lib.rs:6054; field ZERO at
>    ~7859); compute home_off=(home_world_pos-ship_world_pos).as_vec3() before the
>    home render block (~11430); add home_off to EVERY home-group draw position -
>    structure Vec3::ZERO sites (11438/11596/11607/11622/11634), placeholders
>    (11651), machines (11667), pipes (11679) + the transparent glass list; TP-home
>    /Return-home (~9938) sets ship_world_pos=home_world_pos. At home home_off==0
>    (unchanged); away the home recedes to its fixed orbital spot. DEFER collision
>    (home-local, off in fly mode). VERIFY: at-home identical; dev-travel away ->
>    home recedes with NO piece following; TP-home returns to orbit; build mode
>    intact. Multi-site render refactor - fresh focus, not rushed.
>    ** ALSO STILL WANTED: ** surface-relative FREE FLIGHT (fly AROUND the planet
>    co-rotating; today free flight above SURFACE_ENGAGE_ALT=10 km goes inertial)
>    + a scale/altitude/ground-distance HUD readout (operator can't judge the
>    distance to the shore, has no size reference).
>    ** CROSS-DEVICE IDENTITY (operator, 2026-07-12, "get that fixed before we do
>    any more gameplay stuff"): ** he posted from his phone but couldn't upload -
>    the phone auto-generated its OWN unverified identity with no in-app way to
>    adopt his existing Shaostoul identity. SHIPPED v0.836.2 (web): the TARGET-side
>    chooser openLinkThisDeviceModal (chat-profile.js) - 4 methods with plain pros/
>    cons (Scan QR / Paste code / Enter seed phrase / Encrypted backup file), the
>    counterpart to the existing Link-New-Device QR SOURCE modal; a real camera QR
>    scanner (scanQrWithCamera via BarcodeDetector, graceful fallback where
>    unsupported); a sidebar "Link this device to me" button; and an onboarding
>    step-0 escape hatch ("Already have an identity on another device? Link it").
>    His working path TODAY: phone chat -> identity sidebar -> Link this device to
>    me -> Enter seed phrase -> type his 24-word backup -> phone becomes Shaostoul
>    (verified, can upload). Also fixed the sync-web recipe (it hard-rsynced the
>    deleted web/activities/ -> set -e aborted every web deploy before the data
>    rsync tail; now guarded). ** NATIVE FOLLOW-UP SHIPPED v0.837.1: ** the native
>    Account settings (Settings > Account > Identity & Seed Phrase, behind the same
>    passphrase-gated seed reveal) now show a "Show device-link QR" button that
>    renders a scannable QR of the identity backup JSON. The operator's PC can now
>    be the scan source: reveal seed > Show device-link QR, then on the phone chat
>    > your identity > "Link this device to me" > "Scan a QR code". Encodes the
>    exact {name, publicKey(64-char Ed25519), privateKey(seed)} JSON the web
>    importer accepts (unit-test-locked in net::identity). New native-only qrcode
>    crate (matrix-only, gated so the relay stays lean). Cross-device identity is
>    now COMPLETE end-to-end: seed-phrase from anywhere, QR scan phone<-web-or-
>    native, paste code, encrypted file. (Caveat: v0.837.1 release is UNSIGNED
>    until the operator signs it, so desktop auto-update won't offer it yet.)
>    ** /LINK PAIRING CODE - found existing + fixed + surfaced, v0.838.0: ** the
>    operator recalled a /link -> short-code -> enter-on-device-2 flow; it ALREADY
>    EXISTED and is fully backed (create_link_code/redeem_link_code in
>    storage/messages.rs, /link handler relay.rs:3695, device_* handlers wired).
>    8-char hex code, 5-min, one-time; registers device 2's OWN key under your
>    name. REAL BUG FIXED: capability gates read get_role(public_key) per-KEY (95
>    callsites) but the roster aggregates role by NAME, so a linked device showed
>    under your name yet was silently unverified + could not upload; redeem_link_code
>    now copies the creator's role to the redeeming key (one-time/5-min/private = a
>    deliberate this-is-my-device grant; 3 tests). SURFACED in the web chooser as
>    two labeled groups: "Fully become this identity" (seed/QR/paste/file = SAME
>    key, gets DMs) vs "Companion device" (/link code = own key, posts+uploads, no
>    DMs since DMs are E2EE to a specific key). Cross-device identity now closed
>    across BOTH models (full-identity transfer + companion multi-device).
> 4. FEDERATION - LATER. REALITY CHECK: our.universe is Namecheap SHARED
>    cPanel hosting (plan EXPIRING Jul 14 2026); it CANNOT run the Rust relay
>    (no root / persistent process / custom ports). Do NOT renew it for a
>    relay. The $0 test = a SECOND relay process on the SAME VPS on another
>    port + a subdomain via nginx, federate the two. our.universe is only a
>    domain name / static web mirror. <<<**

> **>>> CLOSURE LADDER: FIFTEEN INCREMENTS SHIPPED (v0.745-v0.759, started
> Fable final day 2026-07-07). Every unblocked rung has now shipped its
> planned increments.** Shipped, each with tests + full battery:
> 1. **v0.745 death & recovery** (rung 1): Dead at 0 HP with cause tracking,
>    death screen + respawn at the spawn room, EffectTick (poison bites,
>    regeneration heals), attack-pulse on damage.
> 2. **v0.746 construction entry point** (rung 2): Crafting page > Structures
>    section, materials consumed backpack-first, scaffold-rises render,
>    build_<id> quest events + shelter_building XP.
> 3. **v0.747 credits + first vendor** (rung 3): Wallet (persisted, HUD
>    readout), EconomySystem registered with REAL passive income,
>    TradeGoodsRegistry, Trading Post machine + vendor modal (buy 1.25x /
>    sell 0.5x base).
> 4. **v0.748 quest repair** (rung 4): QuestTracker persists, Available +
>    Accept UI (4 unreachable authored quests now live), xp_rewards granted.
> 5. **v0.749 environment matters** (rung 6): ALL 132 plants harvest
>    (harvest_item column + 114 new produce items + a forever-test), field
>    crops face season/weather while indoor grows stay climate-controlled,
>    station-gated crafting with 8 new craft-station machines.
> 6. **v0.750 gear is real** (rung 8, first increments): equipment.csv over
>    the existing clothing family, Equip MOVES items onto the persisted
>    Outfit (slot-validated, swap-back refuses rather than loses),
>    cold/heat_resist scale the temperature drain, speed + carry_capacity
>    fold through ONE stat grammar.
> 7. **v0.751 passive livestock** (rung 7): CreatureRegistry over all 92
>    creatures.csv species + renewable_product column, starter herd
>    (chickens/goats/sheep via data/entities/livestock.ron) grazing by the
>    fields, walk-up [E] collect (egg/milk/wool) volume-gated with farming
>    XP + harvest_<creature> quest events, block bodies sized from mass.
> 8. **v0.752 native market is live** (rung 5, first increment): the
>    Market page speaks the relay marketplace protocol (browse on view,
>    broadcast-synced list, Publish via listing_create, Delete on own
>    listings), GuiListing mirrors ListingData outright, wire contract
>    pinned by a frame round-trip test.
> 9. **v0.753 abilities are castable** (rung 8, abilities increment):
>    spells.csv renamed abilities.csv + flavor column, AbilityRegistry
>    loads all 110 rows, AbilitySystem validates gate/cost/cooldown
>    (energy pays; level-1 gates baseline-open), v1 self-scoped healing
>    casts (first_aid, cauterize, repair, heal), honest Abilities panel
>    on Profile > Skills.
> 10. **v0.754 HUD ability bar** (rung 8 CLOSED): first nine castable
>    abilities above the inventory hotbar, digit keys 1-9 cast, cooldown
>    sweep + seconds per slot, 4s-fading cast feedback, decorative
>    inventory slot numbers removed (keys that cast must not be numbered
>    on a row that ignores them).
> 11. **v0.755 native reviews + threads** (rung 5, trade-flow increment):
>    Market detail view gains reviews (REST list + WS create/delete,
>    star rows, live average) and the buyer-seller message thread
>    (Contact Seller pulls history, broadcasts append live, draft box
>    sends). Wire frames pinned by tests.
> 12. **v0.756 native escrow trades** (rung 5 CLOSED - T3 fully native):
>    the Trade page rebuilt from the hardcoded mock to the real relay
>    flow (request by key, accept/reject, per-side offer editing, dual
>    confirm, cancel); private-wrapper delivery routed to the page (was
>    destined for chat as noise); relay fix removed the untargeted
>    TradeData broadcast that sent every trade to every client.
> 13. **v0.757 guilds are real** (rung 10, first increment): the Guilds
>    page live against the relay REST guild API - merged list+membership
>    fetch, real join/leave/create/delete (owner), member roles, fake
>    guild-chat echo box removed with an honest pointer to Chat groups.
> 14. **v0.758 real Community Dashboard** (rung 10, civilization
>    increment): GET /api/civilization aggregates rendered live (members
>    + real weekly trend, messages, market, task completion, follows,
>    activity); the fabricated tech/food/water/happiness metrics and the
>    Charts stub removed until real sims feed them.
> 15. **v0.759 vote rules are data** (rung 10, governance increment):
>    proposal_types.ron finally loaded - the relay tally endpoint returns
>    data-driven quorum/pass verdicts (hot-editable per request), the
>    native governance page shows them per proposal (abstain counts
>    toward quorum, not the pass ratio).
> PROCEDURAL PLANETS SHIPPED (v0.763, operator-directed): screen-size
> icosphere LOD (d20 when tiny, doubling ladder, level-7 cap, cached
> meshes), seeded FBM fractal surfaces (smooth oceans, dry-world basins,
> polar caps), fresnel atmosphere shells, per-planet RON params, LIVE
> Settings sliders (Graphics > Planets). v0.764 adds planet-def
> HOT-RELOAD: save a data/planets/*.ron mid-game and the sky updates
> within a frame - the visual tuning loop needs no relaunch. Follow-ups
> documented: 1m chunked landing subdivision, rivers, real craters,
> biomes.ron wiring, mesh-cache eviction.
> SHIP SUPERSTRUCTURE ARC COMPLETE (v0.766-v0.769, operator screenshot
> to vessel in one day; docs/design/ship-superstructure.md is the
> record, absorbs Brief 1): A zones (multi-zone ShipStructure + editor
> selector), B corridors (one row generates the tube + cuts walkable
> shell apertures), C THE COMMONS (the operator mall as pure data:
> glass-roofed hall, tower grove, Trading Post stalls, glass gallery),
> D THE HULL WRAP (lofted plating through data-driven silhouette
> stations, taper clamps that never slice zones, cutouts over glass,
> greebles as data in data/blueprints/hull_profile.ron, live regrow,
> H-key/Settings toggle). The ship's exterior is screenshot-tunable
> data. FOLLOW-UP SEAMS: curved lofts, hull windows, bay doors, profile
> hot-reload, interior liner, multi-deck (design section E).
> OPERATOR FIELD TESTS PENDING (one relaunch covers all): planets in
> the sky + slider/RON tuning, the hull from outside, the glass gallery
> walk to the Commons, wolves + melee. NEXT ARCS (pick per operator
> direction or field reports): hull/planet taste tuning per screenshots,
> combat taste items, planet LANDING arc (1m chunked subdivision),
> guild production pools. Journal follow-up still open: guild REST auth
> should ride the signed-auth helper before real users. <<<**

> **>>> FABLE FINAL DAY (2026-07-07): THE GAMEPLAY DESIGN BIBLE. Fable's last
> session produced the decided-design handoff so Opus executes against specs
> instead of re-deriving taste. READ THESE BEFORE ANY GAMEPLAY WORK:**
> 1. **`docs/design/gameplay-loop-map.md`** - every loop's current state vs
>    designed state (verified against lib.rs register calls by a 7-agent
>    survey), the tier stack (BODY -> HABITAT -> PRODUCTION -> EXCHANGE ->
>    COMMUNITY -> EXPANSION + RISK/PROGRESSION cross-cuts), and THE CLOSURE
>    LADDER: a strict 10-rung order where each rung is one shippable session.
>    Rung 1 = death & recovery. Rung 2 = construction entry point (queue_build
>    has zero callers). Rung 3 = credits + first NPC vendor (economy.ron +
>    trade_goods.ron are authored but unloaded). Rung 4 = quest repair.
> 2. **`docs/design/progression-skills-gear.md`** - skills/abilities/gear with
>    CONCRETE schemas (skills.csv scales+perk_levels columns, spells.csv ->
>    abilities.csv with flavor real|tech|fantasy, new equipment.csv joining
>    items.csv, ONE stat grammar through net_stat_multiplier, the Equipped
>    component unifying the two equip forks) + a 9-rung ladder where rungs
>    1-3 need no new systems at all.
> 3. **`docs/design/decision-briefs.md`** - 5 operator taste calls framed with
>    recommendations (vehicle bay ZONES, unified map scale ladder, Studio =
>    OBS companion, browser = no-JS readable web, crew client-side). Each
>    needs one operator line to green-light; move decided ones into design
>    docs and delete the brief.
> Key survey facts Opus should not rediscover: 19 systems are registered
> (Solar/Electrical/PlayerController were previously undocumented); dormant-
> but-complete systems are allowlisted in tests/engine_wiring_lint.rs
> DEFERRED_SYSTEMS with reasons; creatures.csv (92) / spells.csv (110) /
> enchantments.csv (107) are fully authored with ZERO loaders; the relay
> holds two extra progression forks (game_state JSON XP + skill_dna
> reality/fantasy XP) that the progression doc's unification section owns;
> HydrologySystem has a registration-blocking Mutex-type bug (hydrology.rs:323).
> Operator-blocked items unchanged: vehicle-kit GLB exports, release signing
> (v0.678+ unsigned). <<<**

> **>>> FIELD-FEEDBACK LOOP DAY 2 (2026-07-07): FIELD REPORT 2 FULLY SHIPPED
> (v0.735 -> v0.740).** The operator's hands-on report on v0.731.1 drove five
> releases, all synced + exe-archived:
> - v0.735: HOLD-ALT frees the FPS cursor at machine cards; "Save home" now
>   saves MACHINE placements too (was structure-only -- data-loss grade);
>   smelter status says where it looks; volume-legal adds grow slots.
> - v0.736: inventory transfer AT the tile -- right-click Stash/Move/Take
>   context menu + full drag & drop (tiles drag onto container headers,
>   accent highlight, floating label). No modal.
> - v0.737: auto machines draw inputs from HOME STORAGE backpack-first
>   (home_stock mirror bridge) -- "iron in my inventory and/or garage" now
>   genuinely feeds the smelter.
> - v0.738: THE GRAIN LOOP -- beds/trays/fields are plantable garden groups
>   (Plant button, default_crop in grow_media.ron, seeds consumed, idempotent
>   refill); 8 grain_<plant>_0 harvest items + sunflower + 6 seed packets in
>   items.csv (dry_goods -> silo routing engages); per-bed irrigation.
> - v0.739: garden slot TILE GRID replaces scroll+expand rows; "Harvest N
>   ready" bulk button (harvest_many_request channel).
> - v0.740: CI Verify fix -- em dashes purged from rendered GUI strings
>   (no_emdash lint had gone red at v0.738.1; local lint routine now runs
>   emdash + theme + glyph before every push).
> NEXT: operator field report 3 (re-test list in orchestrator_state
> current_focus); vehicle-kit GLB models BLOCKED on operator Blender exports
> (pipeline ready, docs/game/model-pipeline.md + the Model test crate).
> Release-signing backlog (operator-only): v0.678+ unsigned. <<<**

> **>>> MODEL HANDOFF: FABLE -> OPUS (operator, 2026-07-06). Fable 5 access ends
> ~July 7; then Opus for weeks/months with less compute headroom. Use the
> remaining Fable time to FINISH the launch-critical, everyday-use features and
> leave gameplay for Opus. Operator priority order + current state:**
> 1. CHAT for daily use: direct messages, groups, and connecting to a server all
>    smooth on native. DONE in code, wants the operator field test. Saved
>    servers: add (bare host OK, v0.714) -> switch on click (v0.712, lands on
>    general v0.713) -> forget. Unread across the whole sidebar: DM previews +
>    unread (v0.715), group dots (v0.717), channel dots (v0.718); P2P-group
>    unread correctly WAITS for native P2P push (closed groups only poll their
>    list, there is no message event to flag).
> 2. The operator can point his native PC app at his live server (the VPS relay
>    at united-humanity.us) and use it; mod/admin controls should feel complete.
>    VERIFIED + AUDITED (2026-07-06): all 40+ documented commands have working
>    relay handlers (scout-mapped); 3 defects fixed in v0.716 (the dot-gate bug
>    that made /server-add + dotted /report post PUBLICLY, missing /friend-code
>    + /redeem text handlers, stale /dm docs); operator IS admin on the VPS
>    (ADMIN_KEYS, journaled 2026-05-21). Remaining idea for Opus: surface
>    mod/admin actions as BUTTONS (user context menu / server settings), not
>    just slash commands.
> 3. Universal widgets reviewed for consistency + theme use. DONE (v0.711: 5 dead
>    widgets removed; the ~17 in use are theme-token compliant + editable in
>    Settings, enforced by theme_token_lint + theme_editor_coverage).
> 4. FILES page add/remove files on the server for others to download. DONE
>    (v0.709 signed delete endpoint owner-or-admin + v0.710 native shared-files
>    manager: list/upload/remove on the Files page via the in-app file browser).
> 5. GAMEPLAY HOLD LIFTED (operator, 2026-07-06 afternoon: "focus on non-game
>    stuff first and then move forward with game stuff"). Shipped same day:
>    machine info-windows DONE (live walk-up cards v0.724: cistern litres +
>    battery kWh from the running sims; assembler infinite-of-X vehicle
>    selector + buildable Nova v0.725), material-storage Stage A slice 1 DONE
>    (v0.726: volume_l on all 496 items via scripts/gen-item-volumes.js +
>    Inventory volume tracking + Inventory-page Volume tile; densities patched
>    v0.726.2) AND slice 2 ENFORCEMENT DONE (v0.727: add_item_volume_gated caps
>    by remaining litres on transfers/crafting/harvest/compost; outputs_fit
>    volume headroom pauses auto-machines; mining drone delivery deliberately
>    ungated per the 2026-07-04 never-vanish-a-haul ruling — a documented
>    tension for the operator to reconcile). STAGE A COMPLETE. REMAINING game
>    queue (fresh-session arcs): container-wiring design pass (the typed
>    Container system in containers.rs has ZERO runtime spawns — decide which
>    home.ron machines are containers + how MachineDef declares container_type,
>    then the "containers show contents" card stat), vehicle BAY redesign,
>    texture bug (INVESTIGATED 2026-07-06: a scout's "noise interpolation
>    axis-collapse in 11 shaders" claim was ADJUDICATED AND REFUTED — the
>    shaders are canonical bilinear noise, do NOT change the mix() factors;
>    real hypotheses + repro plan in orchestrator_state decision #89, lead
>    suspect = f32 precision collapse on world-space UVs far from origin),
>    GLB pipeline (guide SHIPPED 2026-07-06: `docs/game/model-pipeline.md` —
>    verified authoring rules from the real loader + the model: field wiring
>    plan + the replace_mesh/shared-cache hazard the wiring session must
>    dodge; remaining = the model: field wiring itself + a viewer).
> FOR THE FIRST OPUS SESSION: read data/coordination/orchestrator_state.json
> (running journal, newest at bottom), this file (top of TIER 0 = next up), and
> CLAUDE.md START HERE. The Fable stretch v0.677 -> v0.712 shipped economy phase
> 2, the page-access + fresh-install reviews, the first-boot storage chooser +
> portable mode, chat markdown/links + 1:1 voice call answer/place + mute, the
> in-app file browser + native shared-files manager (Files add/remove), the
> widget review, and saved-server switch/forget. FIELD TESTS the operator still
> owes: storage chooser (fresh + USB), voice calls (two clients), chat attach +
> shared-files upload/remove, saved-server switching against the live VPS.
> RELEASE SIGNING BACKLOG (operator-only): v0.678 -> v0.712 are unsigned; run
> `just sign-release vX.Y.Z` so desktop auto-update offers them. <<<**


> **>>> ALL-IN-ONE APP + FRESH-INSTALL ACCESSIBILITY (operator, 2026-07-06). Two
> directives:**
> **(A) ALL-IN-ONE APP.** Embed as MANY tools as possible directly into the native
> app so nobody needs external programs: file browsing, file uploads/downloads,
> modding/dev tooling, and whatever else we can. This is WHY maps, notes, tasks,
> calendar, calculator, chat, market, etc. are already in-app. It is also why the
> non-negotiables matter and are universal: infinite-of-X (tools are data, not
> hardcode), ONE theme source, universal widgets + gizmos -- they are what make an
> ever-growing tool suite maintainable. The 3D GAME WORLD is the TECH DEMO showcasing
> the engine; the accessible all-in-one app is the product everyone gets. DECISION
> resolved: native file attach = an IN-APP file browser (NOT the rfd OS dialog),
> extending the existing Files page (files.rs) concept into a real file picker that
> feeds chat upload + downloads. Same in-app browser serves modding/dev.
> **(B) FRESH-INSTALL ACCESSIBILITY (must work for EVERYONE, not just the operator
> PC).** Under investigation via the 2026-07-06 fresh-install-audit workflow:
>   1. BLANK WORLD ON ESC: a fresh user in Chat pressed Esc to go in-game and got a
>      blank skybox + no world, because world/skybox init is gated behind clicking
>      Play or Characters first. Fix: initialize the scene regardless of entry path.
>   2. CWD FILE LITTER: running the exe from the wrong folder (not a dedicated dir)
>      dumps a pile of writable files into that folder. Fix: force ALL writable state
>      to the OS data dir (%APPDATA%HumanityOS) regardless of CWD; keep read-only
>      game-data loading working. Open Q for operator: also an installer step?
>   3. FIRST-BOOT COMPLETENESS: what only works because of state on the operator PC
>      that a fresh user lacks (identity, data/, assets, config defaults).
> AUDIT DONE + adversarially verified (2026-07-06 fresh-install-audit workflow).
> Two fixes SHIPPED v0.706.0: (1) exe-litter -- extract_data_if_needed now writes
> editable modding data to %APPDATA%HumanityOSdata (find_data_dir reads it),
> NOT beside the exe, so running from Downloads no longer dumps ~70 files there;
> reads fall back to embedded so a zero-file install still runs. (2) avatar/blank:
> the avatar + showroom setup was gated on a room id "respawner" the default home
> never emits, so no avatar body loaded on ANY path and the Play/Characters
> showroom orbited an empty point; now falls back to the spawn room. FINDINGS that
> CHANGED the picture: the operators hypothesis (world/skybox gated behind Play)
> was REFUTED -- load_world fires on ANY Esc-to-None and DOES render world+skybox;
> the 3D scene is deferred until first Enter World BY DESIGN (chat-first instant
> startup), and the ECS logic-world IS populated at boot. So the exact "blank
> skybox on Esc" could not be reproduced from static code (world renders on Esc);
> most likely the no-avatar impression (now fixed) or a stale build -- OPERATOR:
> re-verify Esc-from-chat on the v0.706 exe. SECONDARY litter as FOLLOW-UPS (not
> first-boot, lower impact): debug/home_snapshot.json + debug/screenshot_* write
> CWD-relative (construction editor / screenshot dev tool -- keep repo/debug for
> dev, redirect only for distributed); theme.rs save() tries CWD-relative data/gui
> first (harmless for distributed since candidates coincide; in a dev repo run it
> overwrites the tracked theme.ron). INSTALLER: a dedicated-folder installer is a
> good COMPLEMENT but no longer required to prevent litter -- the app is now
> CWD-independent for writes. v0.707.0 ANSWERED the external-drive question with
> the operator-designed FIRST-BOOT STORAGE CHOOSER + full portable mode: a fresh
> machine picks "My user folder (recommended)" (APPDATA) or "Next to the app
> (portable)" (portable.txt marker; data + saves + config incl. identity + logs
> ALL beside the exe, USB-drive friendly) BEFORE identity creation; existing
> installs auto-detect (LegacyBesideExe keeps the dads setup byte-identical;
> deleting portable.txt reverts to per-user). Storage-mode logic lives in
> src/storage.rs (4 unit tests). FOLLOW-UPS: an in-app "move my files" tool
> (Settings > Data) for switching modes later; surface the current mode +
> open-folder buttons on the Data settings page. All-in-one FILE BROWSER for
> chat attach SHIPPED v0.708.0: universal widgets/file_browser.rs (in-app, NOT an
> OS dialog; quick roots Home/Downloads/Documents/Desktop/Game data/App folder;
> type filter + 6MB cap surfaced in-UI; 5 unit tests) + Attach button in the chat
> composer -> upload on a worker thread -> routed send. ARCHITECTURE: native now
> has send_composed_content as THE single content-routing authority (mirrors the
> web v0.698.2 fix): composer, clipboard paste, and file attach all flow through
> it -- which FIXED a pre-existing native privacy bug found during wiring: the
> clipboard-paste flow sent a raw chat message with the active channel, BYPASSING
> DM encryption and the scratchpad local-only promise. Next file-browser
> consumers: Files page upgrade, download destinations, the move-my-files tool.
> <<<**


> **>>> LOOP MODE OUTPUT (2026-07-05 -> 2026-07-06, operator engaged loop mode:
> "focus on obvious stuff you dont need me to decide"). SHIPPED the no-decision
> items: v0.704 Home fully-exposed (no expandables), v0.705.0 native-INITIATED
> 1:1 calls + mute (completes voice-call parity both directions), v0.705.1 dead
> web-chat code removed (chat-voice.js monolith + style.css, 5642 lines, verified
> unloaded). Loop WOUND DOWN because every remaining backlog item carries a
> decision that is genuinely yours, not obvious:
> - NATIVE FILE ATTACH beyond clipboard images: needs a new dependency (rfd, the
>   standard cross-platform file picker; egui has none built in). Your call on
>   adding the crate vs an in-app file browser. Web has ~20-type attach today.
> - STALE app/web/ BUNDLE (294 tracked files, frozen v0.414): it is the output of
>   scripts/bundle-web.js, an in-app OFFLINE web bundle that is currently consumed
>   by nothing. Decision = do you still want the offline-bundle feature (then
>   regenerate + wire it) or drop it (git rm + gitignore)? Not obvious cleanup
>   because it is a half-built feature artifact, not junk.
> - INCOMING-CALL RING SOUND: needs a ring audio asset (assets/audio/ is empty)
>   AND the GUI audio path wired (AudioManager::play_sound exists but is not
>   reached from the chat/render loop). Small arc, but a binary asset + wiring,
>   not a one-liner.
> Other parity tail still open (each a real increment, not loop-obvious): native
> threads panel, presence status picker, friend-code UI, the Add Server stub
> (saved servers render unclickable). And the gameplay arcs (machine info-windows
> + vehicle selector, volume containers) wait for a directed session. <<<**


> **>>> STRATEGIC DIRECTION (operator, 2026-07-05): ONE COHESIVE END-APP, TRIM
> THE FLUFF. Guiding rules for all page/UI work from here:**
> 1. **The NATIVE app is the product; the website MIRRORS it in HTML/CSS.** The
>    native egui GUI is the source of truth for every UI pattern; the web
>    version reflects it. Not the other way around. "Just the GUI of the app is
>    meant to inspire the website." Serve what we can via the web, but native is
>    the focus.
> 2. **No tech-demo / fluff pages.** "Any fluff we add now is fat we have to trim
>    later." Prefer ONE fully-developed thing over two partial ones (the two-
>    gardening-games lesson). Before adding a web-only page, ask: does it serve
>    the end app, or is it a demo? DONE 2026-07-05 (v0.699.3): deleted audit,
>    ai-usage, dashboard, the activities hub, the orphaned gardening game.
>    DONE 2026-07-05 (v0.699.4): deleted `game.html` -- it was just another
>    "download the app" page; the game IS the downloaded native app, so the
>    Download page's "Humanity: The Game" module card now reads "Included". The
>    whole `web/activities/` directory is gone.
> 3. **REMAINING WEB-ONLY CANDIDATES (operator to confirm each):**
>    - `data.html` -- BROKEN (its save/backup/USB actions call removed Tauri
>      commands; dead since we left Tauri). Strong DELETE, or rebuild later as a
>      faithful mirror of the native Data settings.
>    - `projects.html` -- static marketing showcase; the public Roadmap already
>      covers "what we're building". Trim candidate.
>    KEEP -- NOT trim candidates (operator 2026-07-05):
>    - `dev.html` + ALL dev/debug tooling -- PERMANENT (forever-development
>      directive; never trim debugging/diagnostics/testing as "launch cleanup").
>    - `home.html` -- REBUILT v0.700.0 per the operator's direction: Home now
>      outlines the perfect ideal closed-loop homestead (one person, six loops,
>      honestly sized) AND doubles as the game's Home requirements list. One data
>      file (data/home_outline.json, distilled from homestead-solo-design.md,
>      every game_id unit-test-verified real) rendered by BOTH the native Home
>      page panel and the web page -- web mirrors native, zero divergence. The old
>      localStorage room-decorator is gone. NEXT for Home (in the data file's
>      in_game_next): play-load the solo home; live balances track YOUR build
>      against the outline; long-term real-home import.
>    - Essential public/site + functional: index, download, onboarding,
>      wallet-guide, roadmap, accord, shared-files, ops, admin, agents, + every
>      native-mirror page (chat, inventory, tasks, ...). The mirrors ARE the
>      app-as-website; the work there is FAITHFULNESS to native, not deletion.
> 4. **ONE web-browsing page = our OWN lightweight browser (NOT Chromium).** The
>    seed is `web.html` (bookmarks) + the native Browser page. North star: browse
>    real sites (e.g. Google) inside the game on in-game monitors, without the
>    bloat of embedding a full browser engine. Avoid Chromium/CEF. This is real
>    R&D -- non-Chromium web rendering in a Rust/wgpu app (candidates: Servo/Verso,
>    Blitz, a limited custom renderer for cooperating content, or an OS webview
>    with the WebKit caveat). Needs a dedicated research + design pass before any
>    build; consolidate web.html + native Browser into this single surface. <<<**


> **>>> OPERATOR FIELD SESSION 3 DIRECTIVES (2026-07-04 late, journaled in
> full in orchestrator_state):** quick batch SHIPPED v0.693.0 (graphite from
> C-class asteroids answers coal-in-space; live V badges; friends-list role
> badges scoped out; P2P test tucked under Dev tools). v0.703.0 shipped the worst cross-client bug: native now ANSWERS 1:1 voice
> calls (web callers used to ring forever). Ring anywhere (modal overlays every
> page, not just Chat), Accept/Decline, in-call bar with Hang up, busy auto-
> reject; audio rides the proven voice-room str0m path (a reserved __call__
> pseudo-room whose signaling wears the web webrtc_signal envelope). CALL
> FOLLOW-UPS: native-initiated calls (Call button in the user modal), mute
> button on the call bar, ring sound, and the known edge where a live P2P
> DataChannel to the same peer refuses their call offer (fix = renegotiate a
> voice m-line onto the existing Rtc). REMAINING, in rough
> order: (1) FOLLOW-DIRECTION badges (you-follow / follows-you; needs the
> relay to expose both directions to the client). (2) STARTER 1975 CHEVY
> NOVA: the operator's first real-life recreation target; prebuilt in the
> default home (kits.ron entry + starter-vehicle spawn) so driving needs no
> factory chain. (3) TEXTURE BUG: surfaces render as colored LINES not
> splotches/grain -- suspect procedural noise collapsing on one axis;
> investigate shaders. (4) MACHINE INFO-WINDOW OVERHAUL: every walk-up card
> shows relevant LIVE info; assembler gets an infinite-of-X vehicle SELECTOR
> (fixed auto_recipe in RON is an infinite-of-X violation); containers show
> contents; cistern shows volume. (5) VEHICLE BAY redesign: justify every
> machine; bay = dedicated standard-vehicle-sized area (gravity-safety
> justification), select the held vehicle; ties into hangar/mech ZONES;
> 3D printer more justified than an assembler. (6) VOLUME-BASED CONTAINERS =
> material-storage Stage A is GO (slots only for bandolier-likes). (7) GLB
> model pipeline guide (in-app + GitHub) + viewer; GLB for game, STL stays
> for print. (8) STUDIO CHAT LAYERS: HOS channel view on the Studio page,
> then merged YouTube/Twitch/Rumble layers, resizable/collapsible. (9) REAL
> STREAMING PIPELINE: Studio is UI-only today -- capture/encode/RTMP is the
> gap for relay + multistream. (10) STEERING-MODE
> setting: mouse-look / keys-only / hybrid toggle for driving (v0.697 ships
> hybrid: mouse looks, A/D turn the same heading). <<<**

> **>>> PAGE-ACCESS + CHAT AUDIT RESULTS (2026-07-04, 6-auditor workflow; the
> operator's "hidden pages" hunch CONFIRMED). SHIPPED v0.698.0: POST
> /api/v2/agents/override was ANONYMOUS since v0.118.0 (any visitor could rewrite
> data/coordination/overrides.ron + spam #announcements via unrestricted scope_id);
> now Dilithium-admin-signed (same scheme as /api/admin/stats) + scope_id validation.
> NEW TIER-0, ranked: (A) WEB CHAT DM ATTACHMENT PRIVACY BUG -- FIXED v0.698.2.
> file attach / clipboard paste / drag-drop while viewing a DM or group used to
> post the upload as a PUBLIC channel `chat` message while echoing into the DM
> pane (looked private, was not). Fix: a single routing authority
> `window.sendComposedContent(content)` in chat-ui.js now handles a typed message
> AND an attachment URL identically -- group_msg / Kyber-E2EE DM (fail-closed) /
> public channel -- and sendMessage's DM+group branches now DELEGATE to it, so
> the seal logic lives ONCE and can't drift from the attachment path again.
> REMAINING CAVEAT (follow-up, not a launch blocker): a DM image encrypts the
> URL, but the file BYTES sit in the relay's public upload store fetchable by
> URL -- true DM-attachment confidentiality needs encrypted blob storage.
> (B) /download SERVES A STALE FORK -- REPO FIXED v0.698.3, needs ONE operator
> VPS action to go live. Was: nginx humanity.conf:164 pointed at
> web/activities/download.html (frozen at v0.36 "Launcher") while bump-version.js
> stamped BOTH copies (hiding the drift). Done in repo: nginx route now
> `/download.html` (the maintained web/pages/download.html); the fork deleted;
> bump-version.js legacy block removed; PAGES.md + SYNC.md updated. LIVE FIX
> APPLIED 2026-07-05 (AI did it over SSH `humanity-vps`): targeted-edited
> /etc/nginx/sites-enabled/humanity (/download -> /download.html; removed the
> dead /landing route), backed up first, `nginx -t` + graceful `systemctl reload
> nginx`, verified curl /download=200 serving v0.699.2 (was the stale v0.36 fork).
> Also moved the orphaned /var/www/humanity/landing.html (stale April page, no
> inbound links) to /root backup so /landing now 404s. FOLLOW-UP: the web deploy's
> page-copy loop has NO --delete, so every page ever removed from web/pages still
> lingers on the VPS web root (landing.html, activities/download.html, ...) --
> worth a --delete or a periodic orphan sweep. And nginx config is still NOT in
> the deploy pipeline (hand-applied); consider a `just apply-nginx` recipe. <<< (C) NAV EXPOSURE -- CORE DONE v0.699.2 (web/shared/shell.js):
> the hamburger drawer (which holds every page that doesn't fit the 14-tab
> app-mirror row) is now a "More" button visible on DESKTOP too, not just <=768px,
> so the ~17 drawer-only pages (Wallet, Market, Governance, Civilization, Calendar,
> Notes, ...) are reachable by click. Added the 6 working pages that weren't even in
> the drawer: Trade + Guilds (Community group), Calculator + Files + Bookmarks(/web)
> + Roadmap (Tools/system group). Every WORKING orphaned page is now clickable.
> REMAINING (operator taste calls, NOT auto-done): (1) the primary 14-tab desktop
> row is unchanged -- if any drawer page deserves promotion to a real top-nav tab,
> that's your call. (2) DELETE-CANDIDATES left out of nav pending your decision: the
> dead stubs dashboard ("Coming Soon"), agents + ai-usage (native twins removed
> v0.197), and the fully-orphaned legacy web/activities/ hub (~17.5k lines, every
> tab 301s to modern pages) + its gardening page. Confirm delete and I'll remove
> them + their nginx routes + commands.json entries. (D) NATIVE DEAD WEIGHT -- DONE v0.699.0: deleted
> the 17 unreachable variants (5 Overview* landings + 12 Settings* sub-pages) and
> their whole category-browse subsystem (category_overview + settings_pages modules,
> escape_menu's top_categories/sub_pages_for/category_pages/category_meta); rehomed
> the stranded working pages -- Calculator + Files into the Platform tab, Trade +
> Guilds into the Real tab; fixed the "Get oriented" deep-link (was an unknown Real
> section id -> silently landed on Body & Measurements; now opens GuiPage::Quests).
> 36 variants remain (was 53). REMAINING D-tail: (1) GuiPage::Civilization is still
> unreached -- its stats page overlaps the Humanity tab's Mission Dashboard, so it's
> a page-uniqueness call: wire civilization.rs as a distinct "Community Stats" Humanity
> section, OR retire it and fold anything unique into the dashboard. (2)
> theme.nav_dev_visible still gates nothing reachable (Testing/Bugs ship inside
> Platform ungated; Files now in Platform too) -- decide if the dev-visibility toggle
> should hide those or be removed. (E) NATIVE CHAT PARITY -- STARTED (v0.702.0 shipped the top item: markdown
> **bold** *italic* `code` ~~strike~~ + clickable http(s) links render in native
> chat via widgets::msg_format (10-unit-test pure parser) + message_row styled
> spans; links open the OS browser like the Browser page. ALSO fixed: the
> scratchpad was labeled local-only but posted channel:"scratchpad" to the relay
> when connected -- WS send now gated, truly local). REMAINING,
> ranked by impact: markdown/links/link-preview rendering (help modal already
> advertises markdown!); file attach beyond clipboard images; 1:1 voice-call answer
> (a web caller rings a native user FOREVER -- native discards voice_call); threads
> panel; Go Live / Watch Stream are placeholder-only; Add Server stub renders
> unclickable saved servers; scratchpad labeled local-only but posts to the relay.
> (F) WEB CHAT CLEANUP: 6 command-palette commands nothing handles (/redeem /bio
> /social /mypin /search /profile-with-arg) + relay /help advertises unhandled
> commands; voice moderation buttons stubbed-disabled; ~180KB dead weight
> (chat-voice.js 2663-line unloaded monolith + unloaded style.css); app/web/
> offline bundle frozen at v0.414 (~284 releases stale) -- regenerate or untrack.
> (G) SMALLER: data.html calls removed Tauri commands (stub -- rebuild or delete);
> audit.html shows hardcoded seed data (fails its transparency purpose);
> /api/server-info reports accord_compliant:false (investigate why); nginx /landing
> route 404s; preview-server.js lacks the pages-flattening + $uri.html fallback so
> local preview 404s every standalone page; PAGES.md lists a web chat.html mirror
> that does not exist. Chat verdicts: WEB substantially finished for daily use
> (~12k LOC, zero TODOs, all handlers real, E2EE DMs fail-closed, voice
> calls/rooms/PTT real) EXCEPT (A); NATIVE solid B+ core loop with (E) gaps. <<<**


> **>>> SHARED-FILE LIBRARY SHIPPED (v0.675.0, 2026-07-02, Fable 5): the
> operator's "share my .blend phone case / car bushings from my PC" request,
> end to end. `POST /api/upload?share=1` publishes; NEW `GET /api/uploads`
> lists (search + limit); shared files are EXEMPT from the per-user media
> FIFO so a shared .blend never vanishes under later chat photos; chat
> auto-shares ONLY 3D/model formats (.blend .stl .obj .gltf .glb -- photos
> stay private); `original_name` preserved for display. NEW web page
> `shared-files.html` (browse/search/download, in the nav). Smoke-tested
> against a live local relay. page_registry_lint earned its keep on day 2:
> caught `accord.html` missing from PAGES.md. **v0.676.0 HOTFIX rode right
> behind (BUG-046):** v0.675.0's relay crashed at startup on the LIVE DB --
> the new index sat in the schema batch, before the ALTER block adds the
> `shared` column on pre-existing tables; fresh-DB tests/smoke structurally
> can't see this. Fixed + regression-locked with a pre-migration-shape
> `Storage::open` test; ~25 min relay downtime; `/api/uploads` verified
> live on united-humanity.us. **Native follow-up tracked in
> PAGES.md:** in-app shared-files browsing + native chat file-attach parity.
> **Next up (operator's staged vehicle-pipeline decision, logged 2026-07-01):**
> economy Phase 2 -- purchased vehicle arrives as a kit ITEM first (fast to
> test), then factory world-SPAWN after a job finishes, then physical
> transport the player can follow or take over. Before that, the smaller
> remaining threads below are fair game. <<<**

> **>>> ECONOMY PHASE 2 STAGE 1 SHIPPED (v0.677.0, 2026-07-02, Fable 5):
> vehicle KITS. Craft a Pickup Truck Kit / Rover Kit at the workbench
> (steel+iron+rubber, feedable by the Phase 1 drone->smelter chain), click
> Deploy on the item card, and a real Vehicle entity assembles 6 m in front
> of you: body/cabin/4-wheel primitives from data/vehicles/kits.ron
> proportions, persistent across world re-entry AND app restart
> (WorldSave.deployed_vehicles). VehicleSystem registered for the first
> time (deploy arm live; enter/exit/mech dormant until Stage 3). All
> data-driven: a new deployable vehicle = rows in kits.ron + items.csv
> (+ recipe). 8 tests incl. one-kit-cannot-become-two-vehicles + save
> round-trip. Adversarially reviewed pre-commit (2-lens + verifier).
> Operator visual check pending next play session (3D primitives).
> **STAGE 2 SHIPPED (v0.679.0, 2026-07-03):** factory world-spawn. The new
> Vehicle Assembler machine (build palette) auto-runs assemble_rover: home
> stock ingots + rubber become a REAL rover on the pad 3 m in front of the
> machine -- drone -> smelter -> assembler = mine-ore-to-vehicle untouched.
> Vehicle-class recipe outputs world-spawn via CraftingSystem::
> deliver_outputs (shared timed+instant path); full backpack can't stall
> the line; mid-batch machine despawn still delivers at the captured pad;
> machines now carry a Transform (their world pose). NOT the
> ManufacturingSystem route -- one job engine (CraftingSystem) with the
> Phase 1 hardening beats activating a second parallel one. 5 tests +
> data lint. **NEXT: Stage 3 transport** -- the produced/purchased vehicle
> physically travels factory -> buyer (DroneSystem phase-machine is the
> template, camera tp_target for follow, VehicleSystem enter/exit for
> take-over). Also queued: operator visual check of Stage 1+2 primitives;
> the buy-side (market Buy -> factory job) needs the wallet/currency
> decision. **FIELD-TEST FOLLOW-UPS (operator screenshots 2026-07-03,
> partially fixed v0.681.0):** crew grounded client-side -- the REAL fix
> is relay/client LAYOUT ALIGNMENT (relay simulates its multi-deck ship;
> client renders the flat homestead; chore sites need to come from the
> actual home layout); drone dock POPS on launch/return -- wants a real
> docking/undocking sequence; machine labels are static authored strings
> -- consider live label stats fed from auto_craft_status. <<<**

> **>>> OPERATOR DESIGN DIRECTION (2026-07-04 field session 2): UNIFIED MAP.**
> One map to rule them: the main Maps/Cosmos page should show the PLAYER'S
> location (marker next to Earth), and located asteroids should appear on
> that same map -- "everything synced to one thing instead of separate
> systems" (today the mining mini-map on the Inventory page and the Cosmos
> page are disjoint). Design sketch: Cosmos System view gains (a) a player/
> home marker at Earth, (b) the live AsteroidBody entities plotted near it,
> (c) drone-in-flight dot reusing GuiDrone. The Inventory mini-map then
> becomes a shortcut INTO the Cosmos page. ALSO from the session: the
> Garden section of the Inventory page needs a design pass (operator:
> "improve the garden section" -- unspecified, gather requirements next
> play session), and the broader inventory-page restructure remains open
> (nested-container tiles memory has the earlier direction). <<<**

> **>>> FLEET MODE COMPLETE (2026-07-01/02 night, Fable 5): 8 more releases
> in one evening, v0.663.0 -> v0.669.0, built by parallel worktree agents +
> the orchestrator, every branch reviewed/merged/re-verified on main (709
> lib tests green, up from 659 at loop start). Shipped: economy automation
> Phase 1 (ONE drone commission becomes a hammer untouched -- the
> living-ecosystem loop; 5 adversarial-review defects fixed pre-commit);
> web Laws mirror; homestead data gaps #3-#4 (85-crop nutrition bridge +
> component-output/location tables + loaders); WEB governance voting REAL
> (canonical-CBOR JS byte-locked vs Rust via `just vote-kat`); NPC crew
> chores + the first-ever native crew rendering (crew were NEVER visible
> before); the cannot-close civilization panel; the grow-light honesty
> meter (+ real bug fix: batteries counted as 48 kWh/day phantom demand
> EACH); Studio Program/Preview split. **Remaining non-gated:** snapshot
> QA sweep findings (agent still rendering), crop-nutrition Home-page
> integration (compute the food loop from the new data), chore-label
> nameplates, saffron fractional-yield parser bug, studio.rs 13-literal
> theme migration, Studio real transport (multi-cycle). **Gated on
> operator:** Donate payment methods, Mute Server scope, dead-code
> deletion, economy Phase 2 (truck = Item or Structure?).** <<<**

> **>>> AFTERNOON LOOP continued (2026-07-01 evening, Fable 5): four more
> releases shipped, each adversarially reviewed pre-commit where substantive.
> v0.657.0 homestead gaps #1-2 (edible mushrooms in plants.csv, tilapia/
> catfish in creatures.csv). v0.658.0 Studio real mic meter
> (net::voice::mic_level) + FIRST-EVER help_modal adoption (3 topics; the
> native help plumbing had zero call sites until now). v0.659.0 Donate page
> fetches the connected server's REAL funding info (was native-blind,
> web-only); review caught a money-routing bug pre-commit (stale server-A
> addresses shown as server-B's) -- fixed + regression-locked; the fake
> "$350/$1000" progress bar is gone. v0.660.0 native GOVERNANCE GOES LIVE:
> real proposal feed with weighted tally bars + Dilithium-signed vote_v1/
> proposal_v1 submission built with the in-crate ObjectBuilder the relay
> verifies with (7 regression tests incl. relay-storage round-trip);
> review found + fixed 6 defects incl. cross-server stale-proposal voting.
> **Next up:** (a) Laws quick wins (surface the loaded-but-unused
> `categories` as filter chips; BASE/REAL as a real chip), (b) Humanity
> page visual pass, (c) economy automation Phase 1 (time-scale fix first,
> then drone auto-relaunch -> auto-smelt -> craft_hammer proof -- the
> operator's living-ecosystem vision), (d) homestead Phase B gaps #3/#4
> (crop-calorie bridge, component_outputs.ron) + Phase C (grow-light meter,
> "what this cannot close" Home panel), (e) NPC task-AI minimal step.
> Web governance voting = its own tracked item (needs canonical-CBOR JS +
> KAT). <<<**

> **>>> AFTERNOON LOOP, Phase A of the homestead design SHIPPED (v0.656.1,
> 2026-07-01): `data/machines/home_solo.ron` -- the one-person self-sufficient
> homestead from `docs/design/homestead-solo-design.md` (4 solar/2 battery/1
> wind/1 generator, 1 cistern/pump/purifier/tap, 1 air recycler, 2 composters,
> 9 nutrition towers + 1 apothecary + 8 potato beds + 3 oilseed + 2 grain
> trays + 2 mushroom racks + 1 aquaponic tank + 1 grain field + 1 legume
> field + 1 silo + 1 irrigation -- ~2,078 kcal/day indoors alone, ~94% of one
> person's need). Discovered `MachineHome::load` was hardcoded to always read
> `home.ron`, so built the missing selector plumbing too:
> `AppConfig.home_variant` + `machines::home_ron_path()` + a Settings -> Data
> -> "Home Design" (Family/Solo) radio-button UI. 2 new regression tests;
> full verify pass (both cargo checks, 659 lib tests, all 5 lints, doc-links);
> versioned exe built. **Next up (Phase B per the design doc + the loop plan):
> author the 4 flagged content gaps in priority order -- (1)
> `oyster_mushroom`/`shiitake` in `plants.csv` (unblocks `mushroom_rack`'s
> honesty), (2) `tilapia`/`channel_catfish` in `creatures.csv` (unblocks the
> aquaponic B12/omega-3 claim), (3) calorie/macro columns on `plants.csv` or a
> new `data/food/crop_nutrition.ron` (lets the food loop compute from crops
> instead of hand-typed catalog strings), (4)
> `data/self_sufficiency/component_outputs.ron` + `location.ron` + a
> household-size selector data table (turns the design into a computed
> per-loop score). Then Phase C (grow-light meter + a "what this cannot
> close" Home-page panel), then the loop's remaining priorities: Studio
> streaming pipeline, Humanity/Governance/Laws/Donate pass, registering the
> disconnected systems, economy automation Phase 1, NPC task-AI.** <<<**

> **>>> AFTERNOON LOOP RUNNING (2026-07-01, operator AWAKE and actively
> testing HumanityOS live -- different from the earlier overnight loop) --
> see [`docs/history/2026-07-01-afternoon-loop-plan.md`](history/2026-07-01-afternoon-loop-plan.md)
> for the full backlog + safety rules. Read that file FIRST every wake-up.
> Triggered by the operator directly: "enable loop mode to work what's been
> discussed" + "dedicate a subagent to designing a fully fledged
> self-sustaining homestead." A `self-sustaining-homestead-design` Workflow
> (3 research agents + 1 synthesis) is in flight -- its result becomes
> priority #1 once delivered (see the plan doc). Also fixed this turn before
> the loop started: BUG-045 (cloned/mirrored homes in a residential zone had
> no floor/ceiling/trim, only walls -- operator screenshot report) and the
> manual sun-angle override for the construction editor (v0.653.0, operator
> was stuck with unfixably bad lighting since the mothership has no
> orbital rotation simulated at all yet -- a real, separate, larger project
> per the cosmos-architecture design doc). Priority order for the loop:
> homestead design implementation, Studio streaming pipeline, Humanity/
> Governance/Laws/Donate pass, registering 4 disconnected-but-valuable
> systems (ConstructionSystem/ManufacturingSystem/AISystem/OfflineSystem),
> economy automation Phase 1, NPC task-AI minimal step. Explicitly NOT in
> scope without operator input: Donate payment-method list (Patreon
> discrepancy unresolved), Mute Server scope, the ~15+1 dead-code files
> (cleanup opportunity, not yet greenlit for deletion), full ship orbital
> mechanics.** <<<**

> **>>> DAYTIME SESSION (2026-07-01, operator awake, following up on the overnight
> loop's open questions): (1) SkyRenderer REMOVED (v0.651.0) -- operator confirmed
> deletion once told the code had zero external callers already; no visual change
> possible since it was never invoked. (2) Storage architecture / SurrealDB question
> RESOLVED (v0.651.1, docs-only) -- verdict: not a hybrid DBMS, SQLite does 100% of
> real database work, RON/CSV/TOML is a content layer not a second engine; SurrealDB
> evaluated on current facts (BSL 1.1 license, not OSI open source; RocksDB backend
> is a risky C++ dependency given this repo's known Windows linker issues; young 3.x
> line with an open perf-regression issue) and NOT adopted for now -- full reasoning
> in docs/design/storage-architecture.md's new "Is this a hybrid DBMS?" section, so
> this doesn't need re-litigating. The ~133 .surql files the operator recalled were
> confirmed via git history to be pre-rename "project_universe" speculative
> world-knowledge schemas, never wired to any code -- not a prior backend plan. (3)
> Mute Server DESIGN RESEARCHED, not yet built -- see
> `open_questions_for_human` in orchestrator_state.json + the operator conversation
> for the two-phase proposal (build native notification primitives first, since none
> exist; then build tiered mute on top). Awaiting operator's steer on scope before
> writing any code for this one.** <<<**

> **>>> OVERNIGHT AUTONOMOUS LOOP RUNNING (started 2026-07-01, ~8h unattended,
> operator asleep) -- see [`docs/history/2026-07-01-night-loop-plan.md`](history/2026-07-01-night-loop-plan.md)
> for the mission, safety rules, and full backlog. Read that file FIRST at the
> start of every wake-up iteration tonight; it's the durable source of truth
> across context resets. Priority order: (1) chat feature completeness, DONE
> as of cycle 4, (2) livestreaming end-to-end verification, DONE (backend)
> as of cycle 5, (3) a broader stub-completion sweep (now active). Docs
> sync every cycle. On stop: write `docs/history/2026-07-01-night-loop-results.md`.
> **Progress: chat backlog fully shipped (v0.641.0-v0.644.0, see git log /
> journal for detail). Livestreaming backend verified live end-to-end
> (cycle 5, v0.645.0) -- start/stop/viewer-join-leave/chat all confirmed
> correct against a real local relay, EXCEPT a real bug found + fixed:
> BUG-043, `viewer_peak` was fed the live viewer count at leave/stop time
> (only ever highest right at a join, decreasing from there) instead of a
> tracked historical high-water mark -- proved live (2 viewers peak, both
> leave, stream stops, old code would've recorded 0) and with 4 tests
> proven via revert-and-retest. NOT verified: the WebRTC signaling relay
> (simple pass-through, read as correct but not live-tested) and the
> client-side scene-management UI -- logged as a real follow-up in the
> plan doc if time remains later. **Priority #3 (broader stub sweep,
> cycle 6, v0.645.1): two candidates turned out bigger than estimated and
> were NOT force-built** -- `SkyRenderer` (`src/renderer/sky.rs`) is fully
> dead code (never instantiated anywhere; the real sun lighting already
> uses astronomically-real Earth-Sun vectors) and its intended future role
> is a genuine product question, logged in
> `orchestrator_state.json::open_questions_for_human`. `EconomySystem`'s
> deferral is already correctly documented in the lint itself ("needs
> market/credits entities") -- not a quick win, left alone. That
> investigation surfaced a real, high-confidence doc-accuracy fix instead:
> 4 stale "NOT registered, never ticks" claims in FEATURES.md (Weather,
> Atmosphere, Skills, Quests are all actually registered and ticking;
> STATUS.md already had it right), fixed. **Cycle 7 (v0.646.0):**
> `src/systems/navigation/orbital.rs`'s Kepler stub is dead code (zero
> callers anywhere) -- left alone, not deleted. But checking for the
> real math's home found `src/ecs/cosmos.rs`'s
> `body_position_in_system_meters` (the Phase-2 cosmos position
> resolver's `ContainerRef::Body` case) was ALSO a `DVec3::ZERO` stub,
> and unlike orbital.rs this one is real, documented, currently-inert
> infrastructure (no live caller yet -- Phase 3's Cosmos page / Phase
> 4's ship containers aren't built) waiting on exactly the Kepler math
> that already shipped separately in `src/cosmos.rs` (Maps page /
> Sol-system model, v0.262.8). Wired it: now calls
> `crate::cosmos::find_body` + `body_world_position_3d_au` for the
> `"sol"` system and converts AU to meters; unknown system/body still
> falls back to zero (documented). 4 new tests, proven via
> revert-and-retest. No user-visible behavior changed tonight (nothing
> calls this path in the live game loop yet) but it's real progress
> banked for Phase 3+. **Cycle 8 (v0.647.0, BUG-044):** food spoilage's
> data model + tick logic already worked correctly -- the real gap was
> narrower: the EAT handler never checked the `spoiled` flag, so a
> spoiled item could be eaten with full nutrition and zero risk forever
> (cooked/canned/preserved food all has raw_consumption_risk 0). Fixed:
> spoiled food now grants 25% nutrition + guaranteed food_poisoning. 1
> new test, proven via revert-and-retest. **Cycle 9 (v0.648.0):**
> `learning.rs`'s practice-hours `Skill` confirmed DEAD (superseded by
> the real XP-based `SkillSystem` in `skills/mod.rs`) -- left alone. A
> fresh full-repo TODO grep (not just the original list) found 2 more:
> chat's "Mute Server" button needs notification infrastructure that
> doesn't exist yet, logged as a real open question rather than wiring
> a hollow flag; Cosmos page's "Track" button (disabled stub) WAS
> self-contained (the orbital math already existed from cycle 7) --
> implemented continuous camera-follow, 4 new tests via
> revert-and-retest, plus a new `snapshot_cosmos` headless screenshot
> test (the page had none before). Bonus: found `src/gui/pages/maps.rs`
> (591 lines) is ALSO fully dead code -- `GuiPage::Maps` has forwarded
> to `cosmos::draw` since v0.203.2 -- 4th instance this session of
> "superseded file left in place, docs still point at it." Fixed the
> stale FEATURES.md/PAGES.md file pointers. **Cycle 10 (v0.648.1,
> docs-only):** re-checked the plan doc's own "larger/riskier, needs a
> design decision" bucket (8 files) for external callers instead of
> taking the original filing at face value -- ALL of them are ALSO
> zero-caller dead scaffolding (autonomy.rs, blueprint.rs, csg.rs, the
> whole logistics/ and navigation/ trees, physics/fluid.rs,
> physics/collision.rs, psychology.rs, input/{mod,bindings}.rs -- 11
> files, ~250 lines total). None of these needed a design decision at
> all, unlike SkyRenderer/Mute Server; they're just confirmed-dead, a
> safe cleanup opportunity for later, left in place tonight (same
> conservative call made for the other 4 dead-file finds). This closes
> out the ENTIRE original backlog list, both buckets. Only 2 genuinely
> open product questions remain (SkyRenderer, Mute Server), both
> already logged. **Cycle 11:** live-verified the WebRTC signaling
> relay pass-through (`stream_offer`/`stream_answer`/`stream_ice`) --
> 3 bot connections (streamer/viewer/bystander) against a fresh local
> relay confirmed correct unicast routing (bystander got nothing),
> server-authenticated `from` (not client-spoofable), and no
> self-echo. This closes the relay-side half of livestreaming's
> remaining follow-up. What's left (the actual WebRTC media handshake
> + the client-side scene-management UI) needs a real browser/str0m
> peer or the live production relay -- out of scope for the loopback
> harness, flagged for the operator rather than attempted against
> production tonight. This effectively completes priorities #1 and #2
> in full, plus the entire #3 backlog (both original buckets). Next:
> if runway remains, look for genuinely new ground (e.g. a web/
> frontend TODO sweep, since tonight's work was almost entirely
> Rust-side) rather than re-covering closed backlog.**
> **Cycle 12 (v0.649.0, v0.650.0): self-improvement pass.** The web/
> frontend TODO sweep turned up nothing actionable (1 hit total, a
> Tauri-era dead-code TODO in `shell.js` guarded behind a
> `window.__TAURI__` check that's never true post-Tauri-deprecation --
> not worth fixing code that never runs). Instead dispatched an
> independent adversarial-review agent over the whole night's diff
> (`cb089287..HEAD`) before wrapping up -- and it found a REAL bug in
> this session's OWN BUG-044 fix (cycle 8): the spoiled-food slot
> lookup used forward search (`position`) while `Inventory::remove_item`
> actually consumes from the LAST matching slot backward, so a
> fresh+spoiled pair of the same item in different slots could silently
> defeat the whole fix. Fixed (v0.649.0) with a matching reverse search
> + a new multi-slot regression test, proven via revert-and-retest. The
> other 6 reviewed areas were confirmed correct, no changes needed.
> Also fixed a stale v0.283.0 comment in `lib.rs` claiming native has no
> WebRTC stack (it does, shipped in the v0.485-495 arc) -- found while
> cross-referencing STATUS.md (v0.650.0, comment-only). **This is a
> genuinely good stopping point**: both explicit priorities done, the
> full stub backlog closed or correctly reclassified, and a
> self-review pass caught + fixed the one real regression from
> tonight's own work. Next: write
> `docs/history/2026-07-01-night-loop-results.md` summarizing the
> whole night, then stop the loop (~8h target reached; see the
> timestamps in git log from v0.640.1 onward).** <<<**

> **SONNET 5 SESSION CONTINUED (2026-07-01) -- recovered from a repeat clean-worktrees
> incident, shipped all 3 previously-lost features.** `just clean-worktrees` destroyed
> ALL THREE in-flight diffs a second time mid-review (spotlight-cone rendering, the web
> Accord doc browser, and the live screenshot command), this time simultaneously, because
> the first fix was doc-only (a CLAUDE.md warning) and a subagent told to "read CLAUDE.md
> first" read Step 0 literally and ran the destructive cleanup itself. Real fix this time:
> `scripts/clean-worktrees.sh` now structurally refuses to remove a worktree/branch with
> uncommitted changes or commits not merged into main, even under `--yes`; only an explicit
> `--force-unmerged` can destroy real work. All 3 features were rebuilt and SHIPPED: **v0.639.0**
> spot-light cone rendering (real cones, not the point-light placeholder -- `RoomLight`
> carries an optional cone, `CameraUniforms` grew to 672 bytes, every hardcoded buffer
> offset recomputed, verified via a real release-build launch confirming every shader
> compiles clean). **v0.640.0** live in-game screenshot command (drop
> `debug/screenshot_request.json`, get `debug/screenshot_N.png` back within a frame --
> verified end-to-end with a real capture of the live chat UI). **v0.640.0** Humanity Accord
> in-app doc browser (17 governance docs, fixed-allowlist backend verified against 6
> malicious-shaped slug attacks with a real running relay, two-pane web browser at
> `/accord`, the 3 dead GitHub-blob links repointed) -- this one survived a mid-session
> internet outage that killed the harness process; the hardened script protected its
> worktree through the resume, and its solid partial backend work was completed rather
> than redone from scratch. Full verification suite green on the merged result: both
> cargo checks, 624 lib tests, 5 lints, 0 broken doc links. See
> `data/coordination/orchestrator_state.json` recent_decisions for the full incident
> writeup and the CLAUDE.md "known gotchas" entry for the script's new safety model.

> **FIRST SONNET 5 SESSION (2026-06-30) -- docs cleanup + M2c zone population shipped.**
> The three construction forks below are STILL open and unresolved, nothing about them
> changed today. What did happen: (1) a 13-agent reacquaintance assessment; (2) a large
> doc-hygiene + cleanup pass (ROADMAP/STATUS/PAGES re-synced to reality, the OpenClaw
> personal-assistant template deleted a third time from repo root + docs/ai/ +
> docs/reference/ + docs/design/ + docs/network/, 133 dead SurrealDB `.surql` files
> removed, ~180 dead/duplicate files total removed or archived, a live-site OpenClaw
> config leak found and fixed on the public Jekyll site); (3) operator resolved the
> multi-crate question, single crate is final, docs corrected to match; (4) a
> no-backwards-compatibility-debt directive (CLAUDE.md Working norm); (5) the
> game/simulator toggle idea was REJECTED and replaced with a real/fake multi-save
> model + real-life-first boot (TIER 2 item 9 below); (6) **v0.638.0 SHIPPED**:
> mothership zone interior population, residential zones clone the player's home into
> every slot, every other zone type gets a generic tiled filler, two new zone types
> (armory, arena). Not yet visually confirmed in the live 3D viewport, operator should
> eyeball a populated zone next launch. Full narrative in `docs/history/2026-06-30.md`.

> **>>> AUTONOMOUS BULK RUN PAUSED (v0.637, 2026-06-29 night) -- AWAITING OPERATOR BULK-TEST + STEER. <<<**
> Loop mode shipped **9 verified construction/superstructure releases** (all compile relay+native, lib
> tests, 5 lints, snapshots; exes archived): **v0.629** in-view conduit-node placement + drag-port-to-node;
> **v0.630** per-utility usage meters + home self-sufficiency (non-punitive); **v0.631** mothership ZONES
> (M1) -- zone_types registry + wireframe district boxes; **v0.632** conduit node TIERS + service-entrance
> grid-tie; **v0.633** machine ROTATION (yaw); **v0.634** zone interactivity (click/drag/duplicate);
> **v0.635** mothership RAIL node graph (M2); **v0.636** viewport HIDE-per-type (declutter); **v0.637** RAIL
> CARS (animated). The loop then **stopped adding features by design** -- the contained editor backlog had
> thinned to padding (more transit graphs = the same pattern repeated; toggles = trivial), and the genuinely
> valuable next work needs YOUR steer. **Three open forks for the operator:**
> 1. **M1 zone-editor architecture** -- one editor with a zoom/scale switch (mothership <-> zone <-> room)
>    vs separate editors? (`docs/design/mothership-superstructure.md`). Blocks growing the zone editor.
> 2. **M3 civic MALL / meeting zone** -- the social heart: shop stalls (owner + market listing), plaza,
>    transit-hub access. Needs a design pass (ties the market + guild systems).
> 3. **grid S3 multi-home tiers** -- substations aggregating homes -> the fleet grid + zone-level metering
>    (`docs/design/grid-hierarchy.md`). Needs the home->fleet aggregation model decided.
> **Bulk-test the 9 releases when you launch; your feedback (visual tweaks + which fork to take) sets the
> next direction.** The loop is on a long heartbeat (30 min) until you steer; interrupt anytime to redirect.

> **UTILITY TRIO + TELECOM + CONDUIT DEBUG-VIZ ALL SHIPPED (v0.604-623, 2026-06-29).** Power, water,
> air are real at design-time AND runtime with consequence chains (power->water->food->vitals,
> power->air->vitals); the telecom/data utility teaches real media tradeoffs (Cat6 / fibre / WiFi, with
> WiFi RF harming nearby grows); and the build editor now has colour-coded conduit flow visualization
> (v0.622) refined in v0.623/v0.624 (selected-machine-only rainbow flow, static per-utility pipe colours,
> smaller readable beads). **v0.624 fixed the two bugs the operator caught on visual-verify at root:**
> the missing CISTERN TOPS (Mesh::cylinder_capped wound both caps inward -> back-face-culled) and the
> CAN'T-CLICK-MACHINES regression (build-mode entry never rebuilt `machine_pick`; now it does). All
> verified (relay+native compile, 33 machines tests, 5 lints, snapshot).
>
> > **CONSTRUCTION VIEWPORT-FIRST PUSH (operator: "every object needs a proper gizmo; do it in the view;
> > fix the conduit overlap"). Phase 1 shipped (v0.626):** pipes/wires are now CLICKABLE (select a routed
> > connection -> it highlights + the panel gives Remove); conduit support brackets are DEDUPED by
> > position (fixes the overlapping-bracket polygon waste); port handles are bolder. **v0.627: port NODE
> > gizmo redesign** -- the in/out rings became a solid sphere + 4 cardinal arrows (in=input, out=output),
> > and the GRID HIERARCHY vision is captured in `docs/design/grid-hierarchy.md` (home->substation->
> > generator->fleet, non-punitive metering to teach supply/demand). **v0.628: pipes TERMINATE at the
> > matching-utility port nodes.** **v0.629 (build Phase 2):** the pipe GRAPH is built in-view -- "Place in
> > view" drops a conduit node on a floor click, and a dragged machine port can land ON a node (branches
> > onto the main line). **LOOP MODE ENGAGED (operator, 2026-06-29 eve):** keep shipping the backlog
> > autonomously. **Backlog order:** (1) ~~grid S2 metering~~ DONE (v0.630: `utility_meters` per-utility
> > generation/demand/self-sufficiency in the Buildability panel, non-punitive). (2) **MOTHERSHIP SUPERSTRUCTURE**
> > (`docs/design/mothership-superstructure.md`): ~~M1 Zone primitive~~ DONE (v0.631: zone_types registry +
> > Zone on HomeStructure + editor + wireframe render) -> **M2 transit node graphs (NEXT)** (rail multi-
> > stop / elevator shafts / teleporter / cargo tunnels) -> M3 civic MALL/meeting zone -> M4
> > industrial+cargo -> M5 hangar/mech bays. **OPEN FORK (operator):** M1 used a panel+wireframe; the
> > "one editor with a zoom/scale switch vs separate mothership/zone/room editors" question
> > (mothership-superstructure.md) is deferred for your steer before the zone editor grows. (3) **Phase 3
> > trunk hierarchy** -- `ConduitNode.tier` ROUTING (`conduits-node-graph.md` Stage 2; tier EDITING +
> > grid-tie node shipped v0.632, routing still TODO). (4) BULK nice-to-haves: ~~conduit tier editing~~ +
> > ~~service-entrance node~~ (v0.632) + ~~machine rotation~~ (v0.633) + ~~zone
> > select/drag/duplicate gizmo~~ (v0.634) DONE; ~~viewport hide-per-type~~ DONE (v0.636).
> > **Superstructure M2: rail NODE GRAPH shipped (v0.635)** -- topology + editor + gizmo (cars/routing =
> > M2b). **M2b rail CARS shipped (v0.637)** -- animated cars along rail
> > edges. **NEXT loop:** more M2 transit (elevator-shaft node / teleporter edge / cargo tunnel), a zone
> > Hide toggle in the Zones panel, OR M3 civic-mall prototype. Watch the ~8 HomeStructure positional
> > literals on any new serde-default vec field (done 3x: zones, rail). Journal ~134 KB -- rotate near 150.
> >
> > **VIEWPORT DRAG-TO-CONNECT shipped (v0.625):** wiring is now a 3D gesture -- select a machine, drag
> > one of its coloured port handles onto another machine to wire them (the confusing from/to dropdowns
> > are now just a fallback). Array-member machines (e.g. a grain tray) are now movable too (first drag
> > explodes the array into instances).
> >
> > **Build-editor NEXT = conduit TRUNK HIERARCHY (Stage 2 of `conduits-node-graph.md`).** The operator's
> > "moveable main lines + machines branch to them, some paths look wrong" is the Stage-1 node-graph
> > (shipped) limited by per-edge Manhattan routing; Stage 2 (tier 0/1/2 main/sub/subsub + routing that
> > follows the parent line before dropping to the child) is the realism fix. No new data model -- `tier`
> > already exists on `ConduitNode`. Drag-to-connect could also extend to dropping a port on a conduit
> > NODE (not just another machine) once the trunk hierarchy lands.
>
> > **NEXT (open forks -- operator steer, or take the reasonable one):**
> > 1. **detection-sensing implementation** (`docs/design/detection-sensing.md`) -- the big combat-adjacent
> >    multi-modal stealth system (sight/light/RF/smell+wind/sound/seismic). BLOCKED on two operator calls:
> >    the MMO **performance approach** (coarse tick + spatial buckets + analytic falloff vs per-frame
> >    physics) and **HUD-first vs enemy-reactions** scope. The v0.620 `RfEmitter` is one ready channel.
> > 2. **superconductor upgrade MISSION** -- the cable type + bulk-upgrade button exist (v0.616); gate the
> >    room-temp superconductor behind a research/quest so it's earned, not free.
> > 3. **sim-realism-roadmap primitives** (`docs/design/sim-realism-roadmap.md`) -- the remaining gaps from
> >    the 12-agent realism audit.
> > 4. The deferred build-editor polish (rotation gizmo for primitives, viewport hide-per-type).
>
> **BUILD-EDITOR BACKLOG CLEARED (v0.612-614, operator-picked after the wiring arc, 2026-06-29).** The
> object-management trio: **multi-select** + group delete/nudge (Ctrl+click rows, v0.612), **alignment
> snap guides** while dragging (v0.613), and **lock-per-type** (fat-finger protection, v0.614). Deferred
> as low-value/high-effort (logged): a rotation gizmo (machines are primitive shapes, no rot field;
> structures already rotate via `[`/`]`) + viewport hide-per-type (fiddly multi-site render filtering).
>
> > **NEXT (open forks -- operator steer or pick the reasonable one):** the **water->FOOD** chain shipped
> > (v0.611), so power->water->food->vitals runs end to end. Remaining big threads: AIR/atmosphere
> > life-support utility (the 3rd of the energy/water/air trio; integrate the existing AtmosphereSystem),
> > INTERNET/data utility, the **superconductor upgrade mission** + a wire-A-to-B gizmo + per-cable type
> > picker (the `spec` field exists, no UI yet), or the deferred build-editor polish above.
>
> **UTILITY-WIRING + LIVE WATER SIM ARC COMPLETE (v0.604-611, operator "do the wiring; no magic
> transmission; spin up subagents", 2026-06-29).** Power + water are now REAL at design-time AND runtime:
> - **Power (v0.604-607):** `src/utilities.rs` cable physics + `conduits.ron` registry (real NEC copper,
>   superconductor as the upgrade target); machine `ports` + `storage`; buildability **Conduits** check
>   (auto-sizes cheapest copper per run) + **Power-circuit** connectivity check (union-find, every load
>   must reach a generator); **runtime per-island power-flow gating** (`PowerCircuit`, ElectricalSystem
>   sheds per island, no magic transmission).
> - **Water (v0.608-610):** a live **PlumbingSystem** (`WaterTank`/`WaterProducer`/`WaterConsumer`/
>   `PlumbingCircuit`) coupled to power -- the FIRST power->water consequence chain (cut the grid, the
>   well pump stops, the cistern drains). A "Live water" Home-page card. An adversarial review caught the
>   seed topology was inert; v0.610 fixed it (verified fill-when-powered / drain-when-cut).
> - **Docs:** FEATURES.md (was stale since v0.496) + ROADMAP.md + utility-wiring.md brought current.
>
> > **NEXT (the consequence-chain thread, sim-realism-roadmap gap #2):** water->FOOD -- the FarmingSystem
> > already models crop water + dehydration + a `garden_irrigation` top-up that is currently a FREE GUI
> > slider; gate it on actual cistern availability (dry cistern -> crops stop being watered -> wilt) to
> > complete power->water->food. Then: a data/internet utility, the superconductor upgrade mission, and
> > the build-editor backlog (multi-select, rotation gizmo). (`register PlumbingSystem` is DONE -- it ticks
> > against WaterTank/WaterProducer/WaterConsumer, not the old WaterFixture scaffold, which was deleted.)
>
> **STRUCTURAL BACKLOG WAVE FULLY COMPLETE (v0.583-592, operator "proceed until caught up" + "enable
> loop mode", 2026-06-27).** The v0.582 "keep working" feedback wave's structural list AND its
> deferrals, all cleared as one data-driven system (see `docs/design/structure-pieces.md`). The
> autonomous loop (operator away) added the deferrals on top of the directed v0.583-587:
> - **v0.588** -- multi-level foundation: a `Deck` piece + "Place at height" so a deck lands as an
>   upper landing atop stairs; footing sampler uses the player's live height (gated) so it's reachable.
> - **v0.589** -- LADDER CLIMB (hold Space at a ladder, gravity suspended, clamped to span).
> - **v0.590** -- ELEVATOR RIDE (a moving car carries the rider; step on to ride, wait in-shaft to recall).
> - **v0.591** -- CURVED ROADS (Catmull-Rom splines bending through degree-2 nodes, straight at junctions).
> - **v0.592** -- RAIL LINE between paired train platforms (steel rails + ties, deduped).
> - Movement-touching releases (ladder/elevator) each got an adversarial review that caught + fixed real
>   bugs (a blocking deck-rejection, a clamp teleport-snap, a wall-flush drop-out, a jump-cheese regression).
> The home tech demo is now buildable end to end + multi-level THREE ways (stairs / ladder / elevator).
>
> > **NEXT CANDIDATES = the structural REFINEMENTS (operator's pick; not started -- new work, awaiting
> > direction; docs/design/structure-pieces.md):** a ridable moving TRAIN CAR (horizontal
> > elevator-equivalent) + platform-beside-track placement; a glassy elevator DESCENT + a distance CALL;
> > road FOOTING (walk/drive on the surface; marginal on a flat floor); auto-stacking PLACEMENT
> > (click-to-place-on-the-surface-under-the-cursor vs the manual height field); solid-body collision for
> > tall structure pieces. All cosmetic/nice-to-have, none blocking. The directed backlog is DONE.
>
> The directed-then-deferred structural list:
> - **v0.583** -- data-driven `StructurePiece` registry (`structure_types.ron`: wall/stairs/ramp/
>   ladder/elevator/teleporter/train/road) + a "Structure" footer palette (leftmost; "Add wall" moved
>   there) + viewport placement/ghost/bounds-gizmo/select + console `add_structure`/`rm_structure`.
> - **v0.584** -- WALKABLE stairs/ramps/platforms (the first-person ground sampler raises the player's
>   floor to the structure surface under them, step-up capped) + working TELEPORTERS (pair jump + cooldown).
> - **v0.585** -- material LAYERING: `SurfaceLayer` stack on walls (exposed top layer drives colour +
>   `total_thickness`) + `road_types.ron` fixed stacks (footpath/residential/highway/runway) + editor + `add_layer`.
> - **v0.586** -- ROADS as a node+edge GRAPH (`RoadNode`/`RoadEdge`; ribbon mesh per edge coloured by
>   the class top layer; editor + node-ring/edge-line gizmo + `add_road*` console).
> - **v0.587** -- helper widgets on EVERYTHING: machine bounds cubes + conduit-node markers + a master
>   "Helper gizmos" toggle gating the passive overlays (interactive handles always shown).
> - Each release: native+relay compile, lib tests, 3 lints, archived exe, + an adversarial subagent
>   review that caught/fixed 3 real bugs (boxes rendered inside-out; a road-list panic; an untracked-file CI break).
>
> > **NEXT CANDIDATES = the honest deferrals (operator's pick; docs/design/structure-pieces.md):**
> > elevator RIDE + ladder CLIMB (a moving/animated structure-state increment + a destination floor);
> > multi-level landings (upper storeys the stairs connect to); curved road SPLINES + road FOOTING
> > (walk/drive on the surface); the RAIL LINE between train platforms; solid-body collision for tall
> > structure pieces. Each is a focused next increment, none blocking.

> **HOME-CONSTRUCTION REDESIGN -- MAJOR PIECES COMPLETE (v0.532-0.537, operator-directed "build it
> all" push, 2026-06-25).** Node/wall construction: a FIXED outer box (55 x 89 x 3 m steel allotment)
> + freely-designed INTERIOR WALLS placed as segments between corner nodes; same tools for any
> structure; edited equally by an AI (the RON file) and a human (the editor). What shipped:
> - **Stage 1 (data model) v0.532** -- `src/ship/home_structure.rs` (`HomeStructure` + RON load/save +
>   `generate_meshes` -> `HomesteadMeshes`). `data/blueprints/home_structure.ron` = the steel box.
> - **Stage 2a (openings) v0.533** -- doors/windows on walls with data-driven animation STYLE + mesh
>   cutting (piers/header/sill).
> - **Stage 2b (render + editor) v0.534** -- HomeStructure wired into the LIVE render (load_world +
>   rebuild_homestead) + the node/wall editor in `construction.rs` (`draw_wall_editor`): draw walls by
>   clicking corner nodes (chained, snapped, translucent preview), edit corners/height/openings, Save.
> - **Stage 2c (rooms-from-walls) v0.535** -- `HomeStructure::detect_rooms()` flood-fills the floor
>   plan into rooms, live. Plus tested foundations `systems/door_anim.rs` + `ship/conduits.rs`.
> - **Stage 3 (plumbing loop) v0.536** -- `rebuild_connection_objects` routes every connection via
>   `conduits.rs` (up/across/down, copper-potable vs flexible, ceiling hangers + material-aware
>   passthrough gaskets); the wall editor gained Machines + Connections panels.
> - **Stage 4 (animated doors) v0.537** -- `ship/door_panels.rs` + `render_door_panels`: each opening's
>   panel animates by its style (swing/slide/iris/energy/nanowall...), doors ease open on approach,
>   windows are fixed glass.
> - **Stage 5 (position-based machines) v0.538** -- machines in a HomeStructure home position by ABSOLUTE
>   world coords (clamped into the box), never skipped on a stale room id, so they survive wall-edit
>   room-id churn AND the old home.ron machines render (visible at box edges, draggable). `load_world` +
>   `placements()` box-mode branches kept in sync; HUD occlusion -> geometric; garden count -> by stat.
>   Found + scoped by a 5-agent discovery workflow; legacy ship layout untouched.
> - **Stage 6 (clear glass roof) v0.539** -- HomeStructure `roof_material` (default 4 = glass); the
>   ceiling renders translucent in the see-through pass, always visible (a sealed clear roof you see
>   the stars through). Data-driven; opaque roof = roof_material 1.
> - Adversarial reviews v0.534 / v0.535-536 / v0.537 / v0.538 ALL CLEAN (v0.538 verified the two
>   placement copies byte-identical); v0.539 = a low-risk material choice.
>
> > **REDESIGN COMPLETE through v0.539 -- every operator-named item shipped.** Remaining is all
> > operator-gated (a launch-check or a data call), none blocking:
> > - **Operator data decision (OPEN, the one real fork):** the old home.ron machines render but the
> >   v0.538 review confirmed many shipped offsets are negative, so they STACK at the box corner (a
> >   pile, overlapping, near-zero-length conduits) -- visible but poor. Pick: keep-and-drag / CLEAR
> >   for a fresh box (archive first) / I re-author home.ron into a clean positive-coord layout.
> > - **Door-animation FEEL tuning** (open distance / easing / hinge side) after the launch-check.
> > - **Deferred v0.531 review follow-ups (minor, dormant):** object-cap reorder+warn (hologram
> >   truncates before machines when >1024), sphere ghost floor-lift, ghost-over-panel gate.

> **EDITOR-POLISH + MATERIALS + MACHINE-SELECT BATCH SHIPPED (v0.540-553, operator launch-test
> feedback waves, 2026-06-26).** A full wave of build-mode polish:
> - v0.543-548: double-sided walls (kill see-through), CAD dimension overlay (wall lengths + corner
>   angles + feature gaps), door interaction rings + a dev-overlay toggle, native-chat reconnect-loop
>   fix, and THE editor-clickability fix (the full-screen dimension Area was swallowing panel clicks ->
>   rewrote as `ctx.layer_painter`; see memory `feedback_ui_interactability`).
> - v0.549: TAP-VS-DRAG on the corner orbs (click = select + show on the right panel, click-and-HOLD =
>   move); orbs shrunk + dropped to the wall base, clickable through the floor.
> - v0.550: round CORNER COLUMNS fill the cube wall joins (a slim double-sided cylinder of the wall's
>   half-thickness at each >=2-wall join, in the most-opaque meeting material).
> - v0.551: per-pie-slice corner ANGLES on a ground circle (each slice labelled at its midpoint on the
>   floor, raised 10cm; a 2+-wall join shows all its angles).
> - v0.552: WALL MATERIAL picker (pick + render + learn). `data/blueprints/wall_materials.ron` = 8 real
>   materials (steel/concrete/oak/tempered-glass/aluminum/pine/granite/HDPE, real density/tensile/cost/
>   renewable); the wall re-colors per material (per-material meshes; glass -> transparent pass); the
>   panel shows the real properties. Adversarially reviewed (clean).
> - v0.553: MACHINE-IN-VIEWPORT selection -- click a machine in the 3D view (or the list) to select +
>   inspect it on the right panel (type/room/position/power/stats/connections); a ground ring
>   highlights it. Adversarially reviewed; 3 findings fixed (stale pick on the move-fast-path, wall-draw
>   selection exclusivity, array-machine Remove no-op).
> - All native+relay green, lints green, versioned exes archived.
> > **DOOR VISUALS SHIPPED v0.554** -- Opening gained a `locked` state (serde-default; a locked door
> > stays shut, with a "Locked" editor checkbox per door); ENERGY doors are a glowing transparent field
> > (green unlocked / red locked) instead of an opaque slab; NANOWALLS are metallic semi-transparent
> > with a time-driven shimmer (see-through as they dissolve open); each opening's style + lock state
> > floats as build-mode TEXT. Doors now route energy/nanowall/windows through the transparent pass (the
> > panel_motion alpha is finally used). Verified green; the look is operator-confirm (native 3D).
> > **home.ron machine pile RESOLVED v0.554.1 (re-authored, the v0.538-open fork closed):** the box
> > migration read the old room-relative offsets as absolute, piling every machine at the (0,0) corner.
> > A constant per-room shift lifted the 3 clusters into distinct in-box areas (garage x[3,28] z[5,30];
> > garden x[4.5,34] z[37,65]; study x[39.6,43.2] z[24.6]) preserving each cluster's internal layout;
> > buildability + load/round-trip tests pass. Refinable live now that machines are editor-selectable
> > (v0.553), so re-author was the clear best of keep / clear / re-author.
> > **REMAINING (all optional, none blocking, want operator eyes on v0.549-554 first):**
> > - Deeper door polish if wanted: a per-door alpha gradient as a door opens (needs per-door materials,
> >   currently shared), in-PLAY door text (currently build-mode only), an in-world lock toggle.

> **WALL-PHYSICS + EDITOR wave (v0.555-557, operator launch-test feedback 2026-06-26).** Grounded by a
> 6-agent design workflow (corners/physics/thickness/destructibility) + an adversarial critique; the
> implementation-ready plan + critique live in this session's workflow transcript.
> - **v0.555 hull angle** -- a wall ending on the box perimeter now shows its angle vs the hull.
> - **v0.556 COLLISION + per-wall THICKNESS (the run-through fix).** The player IS the camera (rapier is
>   DORMANT -- never stepped), so collision is a geometric SLIDE, not a rapier rewrite: `src/ship/
>   wall_collision.rs` builds thin 2D segments (perimeter + each wall's solid pier spans, DOOR apertures
>   cut so doorways are gaps, WINDOWS stay solid) + `resolve()` pushes the camera XZ out, SUBSTEPPED so a
>   sprint/frame-hitch can't tunnel a 1mm wall (the review HIGH, fixed + tested). Doors collide live
>   (closed/locked block; open+unlocked pass). Per-wall `thickness: Option<f32>` + `shell_thickness` +
>   `default_thickness_m` per material in wall_materials.ron (resolved override->material->0.15), threaded
>   into mesh+collider+room-detect; a Thickness control (down to 1mm) + "auto" in the wall editor.
>   FirstPerson only; third-person + furniture/machine colliders are tracked follow-ups.
> - **v0.557 build-mode AVATAR** -- a draggable teal figure + pyramid gizmo; leaving build mode spawns
>   you at it (seeded at your current spot, clamped to the box).
> > **WALL-MODEL STAGED PLAN (from the workflow, the corner answer to "what better way to fill corners"):**
> > - **Stage 2 SHIPPED v0.558 -- MITER corners.** 2-wall joins cut each end to the bisector (wall_end_miter
> >   intersects the offset edges; a/b-end side flip; degenerate -> square; 3+ joins keep the cylinder;
> >   free/hull ends square). wall_piece builds the prism from the 4 footprint corners; wall_with_openings
> >   lerps the side edges so a mitred end carries through piers/sill/header. 3 geometry tests + an
> >   adversarial review (flushness 0.00000 m all angles, no bugs). Follow-ups (minor): perimeter/hull
> >   corners still square (interior-only); opening jamb skews slightly if placed hard against a mitred
> >   corner; the per-face wall_piece normals point inward (benign via double-siding, commented).
> > - **Stage 3 DEFERRED -- destructibility HP.** Do NOT build until the formula is re-derived against the
> >   REAL 8 materials + proven by an ordering test: the critique caught the draft formula's own numbers
> >   off ~2.3x, "paper" isn't in the DB, and tensile-as-HP scores granite(15MPa) weaker than oak(90) --
> >   needs a toughness_factor/hardness blend (add a column to wall_materials.ron) so brittle-thick stone
> >   resists blunt impact. HP is DERIVED (K*tensile*thickness*area*clamped-density), never serialized.
> >   Gate damage behind an explicit source (weapon/tool), NOT movement collision (a sprint bump must not
> >   delete a wall). Mid-span T-junctions (a wall ending on another wall's FACE) are an unhandled join
> >   class for the miter pass -- resolve before committing the endpoint-snap join model.
> > **WAVE v0.559-567 SHIPPED (operator launch-test feedback, 2026-06-26).** v0.559 miter no longer
> > deforms door/window frames (mitre ONLY true wall ends; opening cuts square); v0.563 fixed "flipped"
> > gizmo normals (overlay pass clears depth + depth-sorts) + smaller orbs; v0.564 door AUTO/MANUAL-open
> > states + window-glass z-fight inset; v0.565 constant-width auto-open LINE ring (drawn like orbit
> > paths); v0.566 mid-span T-junction CLIP (the deferred join class -- a thick wall T-ing into another no
> > longer spears through); **v0.567 door CONTROL PANELS** (a MANUAL door, inert before, gets a
> > wall-mounted panel; walk up in first person + press E to open/close; green/red glowing box; HUD
> > "[E] open/close door"; manual door's open target reads a per-door flag, collision follows it). All
> > native+relay green, ship/door tests + lints green, exes archived; v0.567 adversarially reviewed (4
> > fixes: flag-reset-on-rebuild, panel wall-end fallback, manual-only + no-menu gating).
> > **WAVE v0.568-571 SHIPPED (the big multi-topic feedback batch, 2026-06-27).** v0.568 all gizmo
> > BOUNDS are constant-width LINE circles via a reusable `line::push_circle`/`push_polyline` primitive
> > (docs/design/line-overlays.md saves the idea for grenade-arc / laser reuse) + orbs to 0.05 m; v0.569
> > collapsible nested left panel (walls / machines / utility-lines-by-kind) with Save/Close PINNED +
> > gizmo HOVER states (idle->hover->active) on orbs/cubes/pyramid; v0.570 data-driven door LOCKS Stage 1
> > (lock_types.ron key/keypad/knob/crank/biometric; Opening.locks generalizes locked:bool; control-panel
> > unlock; docs/design/locks.md staged plan); v0.571 data-driven local LIGHTS Stage 1 (light_types.ron,
> > HomeStructure.lights, GI-off toggle) + a fix to a pre-existing renderer CLOBBER (point lights never lit
> > the interior; now the Renderer stores live light state + injects via lit_uniform). All adversarially
> > reviewed; native+relay+lints+tests green; exes archived.
> > **THE remaining big wave item -- NODE-BASED CONDUITS (grounded, deferred, ranked LAST):** the operator
> > wants pipes/conduits as a node GRAPH (main/sub/subsub lines; edit nodes, software auto-routes the mesh)
> > replacing the delete-only connection list. Plan in the ground-construction-systems workflow output;
> > synthesis ranked it last (biggest system, most borrow churn, no urgent pull now the collapsible panel
> > shipped). Build when the operator calls it.
> > **STILL OPEN (operator-requested, none blocking):**
> > - **Locks Stages 2-5** (knob/crank + key-item gating; lockpick/hack; wall-mounted locks; shoot/blow ->
> >   needs destructibility). docs/design/locks.md.
> > - **Lighting Stages 2+** -- real spot CONES + bar AREA shading (shader maths), emissive-surface-as-light
> >   (harvest the energy-door / lock-indicator glow), click-to-place, >8-light culling. NOTE: the v0.571
> >   renderer fix makes point lights + the real sun direction reach the interior for the FIRST time --
> >   launch-test the home still looks right with GI ON.
> > - **Door-content system** -- data-driven multi-PART doors for a premade catalog, custom/stained-glass,
> >   REAL iris doors (sliding petals -- operator: the current iris is "totally wrong"), revolving doors.
> > - **Control-panel actions beyond open/close** -- emergency power, hack (lock/unlock now ships in v0.570).
> > - **nanowall = animated water CAUSTICS** (not the current uniform pulse; shader, ref image given).
> > - **door/wall LABELS** with on-door / on-wall / both placement + a draggable position gizmo.
> > - **Destructibility Stage 3** (HP/physics) -- re-derive the formula first; Locks Stage 5 hooks it.

> **ACTIVE 2026-06-23: HOME-DESIGN AI/PLAYER PARITY arc (operator-directed).** Make the AI's
> home designs use the SAME machinery players build with, so they're inherently player-workable
> + real-world-valid (steel-primary + wood; the homestead enclosed in a steel ship where Earth
> and ship share identical plumbing/electrical). North star + staged plan: `docs/design/home-design.md`.
> - **Stage 1 (machine placement) SHIPPED + HARDENED:** v0.519 place a machine in the selected
>   room; v0.521 x/z/y offset editing; v0.522 a 4-dimension adversarial review fixed 5 real bugs
>   (room-delete orphan cleanup, machine-remove connection pruning, deterministic BTreeMap save,
>   room-aware offset ranges, array-id collision guard) — all in testable `MachineHome` methods
>   reusable by the AI too.
> - **Stage 2 (connections) SHIPPED v0.523:** the per-room panel gains a Connections section —
>   wire a machine in this room to any machine by kind (power/water/nutrient/fuel/air/waste), list
>   + Remove, validated `add_connection`/`remove_connection`. Verified by a `construction` UI
>   snapshot (the panel actually renders) + a unit test.
> - **Stage 3a (buildability validator: power + wiring) SHIPPED v0.524:** the editor's whole-home
>   "Buildability" section computes real kWh/day from the placed machines — is there a power source
>   for the load, does energy balance over a representative day with the battery carrying the night,
>   is the wiring intact — each as a green/amber/red verdict. `MachineHome::buildability_report` is
>   pure + AI-callable. 5 unit tests + the snapshot show it. Seed home reads all-pass.
> - **Live editor 3D preview SHIPPED v0.525:** machine edits (offset drag / add / remove /
>   connect / room move) now refresh the machine meshes LIVE in the editor instead of only on world
>   entry. Fixed the operator's "I can't move the objects in the weird list" -> the offset fields
>   always worked, but nothing rebuilt the mesh, so dragging looked broken. Machines got their own
>   `machine_objects` render list + `MachineHome::placements` (pure, tested position resolver) +
>   `rebuild_machine_objects` on a `construction_machines_dirty` flag. (Live power ECS + connection
>   pipes still resolve on next world entry -- a follow-up.)
> - **home.ron RESTORED + save() hardened (v0.525-0.526):** an in-game "Save machines" had degraded
>   the shipped home.ron (lost 56/59 design comments + ~12 machines + 11 connections) and it got
>   committed incidentally. Restored the authored design from 652bff60; save() now preserves the
>   leading comment header so future saves do not re-strip it. (serde can't keep interspersed
>   comments -- design rationale belongs in the leading header or docs/design/.)
>
> **ACTIVE NEXT -- BUILD MODE (operator-directed 2026-06-24, from launch testing v0.525-ish).** The
> operator wants the construction editor to feel like a game build mode, not a numeric list:
>   (1) **Footer placement palette SHIPPED v0.527** -- a bottom bar with category tabs (Power / Water
>       / Food / Production / Defense / Logistics, with counts) + a 10-wide grid of the types in the
>       selected category; collapsed by default, Expand for more. Data-driven: `category` field on
>       MachineDef + `MachineHome::palette_categories()` (the 26 seed types are tagged). Click an item
>       -> placed in the SELECTED room (lands at center, appears live since v0.525). Snapshot-verified.
>   (2) **Ghost placement SHIPPED v0.529** -- click a palette item to HOLD it (it gets an accent
>       fill + border); the editor renders it as a semi-transparent ghost on the room floor under the
>       cursor; left-click DROPS it exactly where you click (offset from that room center, not the
>       center). Stays held for multi-place; right-click / re-click cancels. Reuses the room
>       floor-raycast (`cursor_floor_hit`). Ghost mesh cached (no per-frame leak). Also removed the
>       legacy garden markers (the 2 non-responsive sphere-towers). Launch-verify (3D).
>   (3) **Live connection lines SHIPPED v0.530** -- connections render as live colored cylinders that
>       follow rooms (replaced the static routed pipes). Trade-off surfaced to operator: simple lines
>       vs realistic routed pipes; awaiting their read on the look.
>   (4) **GPU-leak + stale-held-item fixes SHIPPED v0.531** (from an adversarial review): a HIGH
>       per-frame room-drag leak (renderer was append-only; now has a replace_mesh/update_material
>       reuse API used by the shell/machine/ghost rebuilders), per-edit machine+ghost leaks, and the
>       stale-held-item-across-editor-close wrong-context placement.
>   (5) **NEXT -- EASY PLUMBING step 2 (operator ask):** click machine A then machine B to draw a
>       connection (machine-pick raycast + connect mode + a Wiring palette category), on top of the
>       v0.530 live lines. Pending the operator's read on the line look.
>   (6) **Click a machine to select + drag to move it** in the viewport (machine-pick raycast).
> - **Build-mode review follow-ups (deferred, from the v0.531 adversarial review):**
>   - 1024 object cap can still be exceeded by a very dense garden (a big MachineArray); worse, the
>     truncation drops the hologram/remote-players (pushed AFTER machines) not the excess machines.
>     Fix: reorder the all_objects pushes so the unbounded machines are last + a one-shot overflow warn,
>     or a growable object buffer (infinite-of-X).
>   - The placement ghost isn't floor-lifted for a sphere shape (dormant -- no sphere in the catalog).
>   - The ghost previews even when the cursor is over a side panel (gate on egui pointer-over-area).
>   (7) Future categories beyond machines: Structure (place a room/wall), Furniture -- the palette
>       framework already supports any category; these need their own placement actions wired in.
> - **Deferred (operator's pick when build-mode lands):** Stage 3b validator (water/structure/
>   materials -- a data-model step); Stage 4 unify the model (home-design.md open questions);
>   the v0.522 save-success-toast polish.

> **FOLLOW-UP (from the 2026-06-23 "Too many connection attempts" incident): GRACEFUL
> RELAY RESTART.** Every deploy restarts the relay, which drops ALL client WebSockets at
> once -> a reconnect storm. v0.520.0 raised the per-IP identify limit 10 -> 30/min to make
> that survivable, but the real fix is a relay that hands off / drains connections on
> restart so a deploy never blips active users (and ideally preserves in-memory voice_rooms
> -- see the older voice note below). Also a follow-up: the NATIVE ws_client should back off
> past the 60s window on a "Too many connection attempts" message (the web client now does;
> the native client's x2-from-5s backoff is ~3/min so it's under 30/min, but explicit
> respect-the-throttle is more robust). Until then: avoid pushing many releases in a short
> window (each one restarts the relay).

> **ACTIVE 2026-06-22: INVENTORY REDESIGN (operator-directed) — nested-container TILES.**
> The operator specified a spatial inventory: every container is a card holding its items
> as evenly-sized TILES with its sub-containers nested inside (person -> shirt -> pocket ->
> {pen, keychain, wallet}; house -> rooms -> containers; car -> trunk), with MULTIPLE
> inventories visible so items can be transferred between them and inspected. Full spec in
> memory `design_nested_container_inventory`.
> - **SHIPPED v0.512.0:** `draw_container` recursive renderer (items = tiles, sub-containers
>   = nested cards), tile selection + inspect card.
> - **SHIPPED v0.513.0:** header contents counts + whole-row click to toggle a container.
> - **SHIPPED v0.514.0 — item TRANSFER (organize layer, the operator's chosen model):**
>   `PlacedItem { key, name, qty, container-path }` pool on GuiState, seeded from the places
>   spine (`flatten_placed_items`); non-backpack tiles source from the pool so moves are
>   live; selecting an item shows a "Move to" combo (`collect_containers`) that re-tags its
>   container. Serialize-ready for a save.
> - **SHIPPED v0.516.0 — backpack <-> container transfer** (the ECS boundary): the
>   `inventory_transfer_ops` channel + InventorySystem application; "Stash to" (backpack
>   -> container) and "Take to backpack" (container -> backpack).
> - **SHIPPED v0.517.0 — persistence:** `placed_items` ride in `WorldSave`, so transfers
>   survive a restart (serde-default for old saves).
> - **ARC COMPLETE (organize layer).** Per-container **capacity/weight** is the operator's
>   explicit "later" (they chose organize-layer-first) — do NOT build it unasked. The
>   container-header click is covered by the headless interaction harness; the combo/button
>   interactions are best confirmed once in `just launch`.

> **Dev-experience tooling LANDED (v0.515.0 + follow-ups, 2026-06-22..23):** headless egui
> interaction harness, `verify.yml` CI (green), `just brief`, Windows-safe git hooks,
> `engine_wiring_lint` fixed, agent-status staleness, journal rotation (`just rotate-journal`,
> the journal went 513 KB -> 70 KB). See memory `dev_workflow_tooling`. Deferred audit items
> the operator should weigh in on: a one-command `just release` wrapper (fork-test the
> create-vs-auto-publish race first); slimming CLAUDE.md's finished PQ changelog into
> `docs/history`.
>
> **SHIPPED 2026-06-22 (v0.511.0): MINING MAP reads as a journey** — route line in accent +
> drone labelled mid-trip ("drone · outbound"), fixed the "Hom drone" label collision. The
> "see your little drone going off to mine, with real distance" view.

> **SHIPPED 2026-06-22 (v0.508-0.510): GARDEN EDIT SLIDERS ARE NOW FUNCTIONAL (autonomous loop, native).**
> The garden edit modal was cosmetic -- the per-medium form existed but nothing consumed
> its values. Now: v0.508.0 moved the grow-media into `data/garden/grow_media.ron` (a
> plot-type is a data edit, infinite-of-X); v0.509.0 wired the **water** slider to crop
> survival (a configured tower keeps crops topped up -> healthy/grows, low -> wilts);
> v0.510.0 wired the **nutrient** slider to growth speed (0.5x..1.5x). Pattern: the GUI
> publishes a neutral `HashMap<tower_id,f32>` per frame (`garden_irrigation` /
> `garden_nutrient`), lib.rs bridges it to the DataStore, FarmingSystem reads it -- no GUI
> type leaks into the sim layer. Proven by `per_area_irrigation_keeps_configured_crops_watered`
> + `per_area_nutrient_speeds_growth`.
>
> **NEXT (top of the loop queue): LIVE HOME SIM.** `homes.rs` still shows AUTHORED
> self-sufficiency strings from `home.ron` loops, not the running sim. Make it read live
> `PowerStatus` (ElectricalSystem/SolarSystem/Battery already publish it). The blocker:
> home machines + those systems only spawn/tick after `load_world` (Enter World), not in
> MENU mode -- so the Home page is frozen. Fix = spawn home machines + tick the power
> systems at startup (a startup-ordering change at ~lib.rs:2000/2050). Needs careful
> verify (snapshots + the operator's eye -- 'shows != works' for egui), so it is a focused
> pass, not a fire-and-continue step. Then: grow-light-vs-power meter (depends on this);
> extend per-area sim to soil-bed/field crops (no tower_id link yet).

> **SHIPPED 2026-06-18..21 (v0.485-0.495): NATIVE VOICE CHAT, end to end.** Mic
> capture + Opus + RNNoise + transmit modes (Phase A + input stack), str0m WebRTC
> Opus media (Phase B), per-channel voice rooms interoperable with web over
> `voice_room_signal` (Phase C), and live two-way audio (Phase D). Voice is now
> **per text channel** (the voice room IS the channel, keyed by its id + the
> `voice_enabled` flag), which fixed "clicking the mic does nothing". Defaults:
> Noise suppression + Push-to-talk on CapsLock. Full reference:
> [native_voice.md](network/native_voice.md). ALSO shipped: **headless UI
> snapshots** (`just snapshots` renders egui pages to PNGs so the native UI can be
> reviewed without launching the app) + `just verify` / `lints` / `preflight`.
>
> **NEXT (voice + dev-infra, agreed with operator):**
> 1. ~~In-process WebRTC test harness~~ **DONE (v0.496):**
>    `inproc_webrtc_tests::two_str0m_opus_roundtrip` in src/net/webrtc.rs drives two
>    str0m instances through ICE + DTLS in-process and asserts an Opus frame
>    round-trips. Voice/net media changes are now CI-verifiable.
> 2. **Native per-peer voice controls** UI (volume / mute / squelch), mirroring
>    the web's `web/chat/chat-voice-modal.js`, plus a visible in-call indicator.
> 3. **Web transmit-mode parity** (the web has no open-mic / PTT / VAD UI yet).
> 4. Graceful relay restart so a deploy does not drop active voice (a deploy
>    currently clears the in-memory `voice_rooms` + drops all client WS).
>
> Older queued items (launcher visual verify, two-player co-presence test,
> server-side character persistence) remain below, unchanged by the voice arc.

> **The strategic, public planning doc is now [ROADMAP.md](ROADMAP.md)** (it renders to the app +
> website via `data/roadmap.json`). PRIORITIES stays the tactical "very next thing"; the roadmap is
> "where we are going" + the full themed backlog. Keep them consistent.
>
> **Signing P0 RESOLVED (2026-06-16):** desktop auto-update was dead since v0.421 (no signed
> release); the operator signed v0.470.0 and `just check-signing` confirms it green. Every release
> from v0.421 onward MUST be signed; `scripts/check-release-signing.js` (in `just status`) flags it.
>
> **SHIPPED 2026-06-16:** website parity (v0.469.x), the security patch + signing pipeline verified
> (v0.470.0), multistory editor + structural de-risk spike + web accessibility (v0.471.0),
> **multiplayer co-presence client wiring (v0.472.0)** (NetSyncSystem reuses the authenticated chat
> WS; remote players render as teal avatars), **battery state-of-charge sim (v0.473.0)**, and
> **the character launcher + Game Admin page (v0.474.0)**: Play opens a character/home picker
> (Homes wired; Open-Net/Closed-Net placeholders) with a persisted default that skips the picker and
> a Customize-Look button; Game Admin issues game-world bans that are STRUCTURALLY SEPARATE from chat
> bans (free speech is a right, MMO play is a privilege).
> **NEXT (top of the queue):**
> 1. Operator visually verifies the v0.474.0 launcher + Game Admin layout (run `just launch`; egui
>    can't be auto-verified). 2. The scheduled two-player co-presence test on the VPS (operator's
>    tester, later today). 3. Multiplayer polish for that test: remote-player NAMEPLATES +
>    WORLD-SNAPSHOT PREFILL (a joiner should see already-present players immediately, not only on
>    their next move). 4. Then server-side character persistence (the Open-Net/Closed-Net launcher
>    sections + `character_v1` signed object + `character_policy` per server), and GitHub branch
>    protection. See [characters-and-servers.md](design/characters-and-servers.md) +
>    [first-playable.md](design/first-playable.md).

**ACTIVE 2026-06-15: CONSTRUCTION EDITOR arc (operator-directed detour, paused the LIVE HOME
SIM arc below).** The operator is building the in-app homestead editor by rapid screenshot
feedback. SHIPPED: v0.463 top-down canvas -> v0.466 3D room grab -> v0.467 3-column layout ->
v0.468 door/window slide gizmo -> **v0.469 OPENINGS AS PLACED OBJECTS** (the v0.468 wall-kind
model was unintuitive; redesigned). Now: a door/window/airlock is an additive `Opening` object
(`RoomConfig.openings: Vec<Opening>`) placed on a still-solid wall -- add multiple per wall,
move via a 3D wall-plane handle (door snaps to floor; window free up/down/left/right), RESIZE via
edge handles or numeric W/H, every value clamped to the real wall (the 20m-vs-2m bug is gone).
Also fixed the garden-wall regen bug (Open self-heals to Solid; `neighbor_owns` never hides a
wall that owns a placed opening). Legacy WallKind windows still work + a `promote_walls_to_openings`
path. **NEXT construction slice = LEVEL selector + STACKED-room multistory** (build-order step 2
in `docs/design/construction-architecture.md` (g)) - the big multistory unlock the operator
flagged as a hard requirement (homes several stories; the mall is multiple stories aboard the
ship). Then: Stage 2 SVG/curved cutouts (`Opening.profile` hook reserved), click-on-wall-to-place,
multi-level rooms (the mall). Resume the LIVE HOME SIM arc after the editor reaches multistory.

**ACTIVE 2026-06-13: LIVE HOME SIMULATION arc (operator pick from the content-strategy
survey).** The survey's finding: the home is a beautiful diorama, not a sim. ~270 LOC of
real simulation logic (ElectricalSystem, PlumbingSystem, AtmosphereSystem, VehicleSystem,
ConstructionSystem) is written but ticks for nobody because the entities are never spawned.
The arc: make the home a LIVE engineering sim. **Increment 1 SHIPPED (v0.437.0):** machines
declare an optional `power` role in home.ron (Solar/Generator/Consumer); load_world spawns
them as live ECS entities (PowerGenerator/PowerConsumer/SolarPanel, tagged HomeMachine for
idempotent re-spawn); a new SolarSystem scales solar by sun_factor(hour) and the now-
registered ElectricalSystem sums supply/demand + publishes a live PowerStatus to the
DataStore; the HUD shows a live "Power: gen/use/net W" line that climbs from ~1750 W at 8am
to ~3350 W at noon and falls to the wind baseload at night. **NEXT increments (this arc):**
(1b) battery state-of-charge integrating surplus/deficit over time (the "2.8 days autonomy"
becomes live + the generator kicks in on deep deficit); (1c) the live Home-page loop summary
reads PowerStatus instead of authored strings; (1d) per-machine-card live stats (solar card
shows live watts); (1e) register PlumbingSystem against WaterTank/WaterFixture the same way.
Then arcs 2-6 from the survey: survival stakes, operable machines ([E] runs them), build
mode, enclosed-space climate, vehicles/drone fleet. Full ranked roadmap in the journal
(orchestrator_state.json, 2026-06-13 content-strategy decision).

**(Prior groundwork, v0.427-0.436, all shipped):** 3D home populated with data-driven
machines + LOD labels + [E] interaction; honest closed-loop homestead design + Home-page
closure summary; realistic orthogonal pipe routing with elbows/collars/brackets/valves/wall-
penetration-sleeves (data/routing_rules.ron); mouse-sensitivity boot fix (BUG-038). Operator: populate the home with primitives for ALL machines +
connections, then make it a demonstrable self-sufficient homestead anyone can load and
learn from. **Shipped v0.427 -> v0.433:** data-driven machine layout (`home.ron` +
`src/machines.rs`, infinite-of-X catalog/instances/connections) with `box_xyz`/`pyramid`/
`segment` primitives (v0.427); floating LOD machine labels with status icons + occlusion +
Tab-reveal (v0.428-0.430); walk-up [E] interaction cards (v0.431); closed-loop stats +
Home-page closure summary (v0.432); **v0.433 the honest indoor garden:** a 4-lens design
workflow proved LIGHT (not floor area) caps indoor food, so 1156 m2 of sun-lit garden ~=
a ONE-person diet, and grow-lights to feed 3 would draw 2.5-12x the energy budget. Built
sun-lit: closes B12+omega-3 via aquaponics, honestly offloads ~half the calories + most
fat to outdoor fields. New `arrays` data feature fills any room with one RON line. Lesson
baked into the loop notes + `docs/design/self-sufficiency.md`. **NEXT (operator-flagged by
the design pass, high teaching value): a live GROW-LIGHT-DRAW vs power-budget meter** that
turns red the instant an LED is added past the free pump headroom, the single most honest
teaching artifact in the build. Then: live-sim wiring (loop numbers move with day/night
play), an in-world closure HUD, or the expedition/stealth layer (design-doc filed).

**Sign step pending:** the v0.426.0 release is built + waiting for `just sign-release
v0.426.0` (operator passphrase). Security sprint code-complete; only the GitHub
branch-protection click remains (docs/admin/security-hardening-tasks.md item 2).

---

**DONE 2026-06-13: SECURITY SPRINT TAIL (v0.423.0->.426.0).** Continued + largely
closed the 2026-06-12 audit. A scoping workflow (9 agents) re-verified ALL prior shipped
fixes still hold (SVG-XSS, vault-replay, object/announce quotas, headers, twemoji,
gossip-amplification, future-timestamp, all SOLID by file:line) and surfaced + planned
the open items. Shipped: **(1) object storage hardening v0.423.0** (per-author quota
capped count but not bytes -> MAX_SIGNED_OBJECT_PAYLOAD 256KiB at the put_signed_object
chokepoint covering REST + gossip, 413 on REST; + a size cap on the federation_rate
map); **(2) hard-delete remnants v0.424.0** (PRAGMA secure_delete=ON + wal_checkpoint
TRUNCATE after bulk wipes; honest backup-residual documented; byte-scan test proves
scrub); **(3) member-directory opt-out, FULLY DONE v0.425.0 backend+web + v0.426.0
native** (profiles.privacy directory:"unlisted" honored in get_members/count/get_member
via a shared LEFT JOIN + json_extract; web checkbox + native checkbox & the first-ever
native profile_update send). **REMAINING = operator-only tasks** (no code): /api/send
nginx edge rate-limit (paired VPS zone-def + conf block; LOW value, API_SECRET-gated)
and GitHub branch/tag protection + deploy approval gate, both with exact steps in
**docs/admin/security-hardening-tasks.md**; plus the ongoing `just sign-release vX` duty.
The audit's CODE surface is now closed. **NEXT (operator pick from ROADMAP.md "Right
now"): the next gameplay arc (garden plot-types registry OR First Playable).**

---

**DONE 2026-06-12: DOCS REFACTOR (v0.422.1->.3, docs-only patches on main).** The whole
docs folder was refactored to the operator's brief: (1) all em dashes removed from active
docs (they read as AI-written); (2) stale crypto fixed (Ed25519->Dilithium3 identity,
ECDH->Kyber768 DM, per the CLAUDE.md table); (3) the roadmap is now the to-do list,
`docs/ROADMAP.md` is the single canonical, themed, status-badged roadmap that the website
renders from `data/roadmap.json` (via `scripts/roadmap-to-json.js`); (4) audience-first
structure, `docs/README.md` is the router, with `user/ admin/ ai/ contributor/` folders
each having a who-this-is-for README + clear onboarding (`user/getting-started.md` covers
who/what/when/where/why/how for non-technical readers). docs root went 50->17 files; stale
indexes fixed; `scripts/check-doc-links.js` holds the tree at 0 broken links. **NEXT
(operator pick from ROADMAP.md "Right now"):** the next gameplay arc (garden plot-types
registry OR First Playable arc), or the security-sprint tail (sign each release
[operator-only], member-directory opt-out UI, GitHub branch protection). Going forward,
maintain `docs/ROADMAP.md` as the strategic to-do and regenerate the JSON after edits.

---

**ACTIVE: INVENTORY styling batch (operator on v0.404 screenshot, 2026-06-08), happy with direction, wants polish.** Items: (1) slim ALL buttons, even padding, font-driven height [**DONE v0.405**, universal Button is font-driven + tokenized button_pad_y; steppers slimmed to compact_button_height]; (2) slim the mining +/- [**DONE v0.405**]; (3) "Start collapsed" → a proper bordered button (full area clickable) [**v0.406 NEXT**]; (4) every section collapsible + more defined, with the standard Collapse/Expand/Start-collapsed on ALL nested lists [**v0.406**]; (5) row striping #000000 / #020202 (subtle) [**DONE v0.406**, row_stripe color token → egui faint_bg_color]; (6) move location text inline after the title in parens [**DONE v0.406**, place_to_tree puts it in the label]; (7) INLINE ROW EXPANSION, click an item row to expand in place (picture/3d + full details over rows), click to collapse, instead of a popup/top detail [**v0.408, the big one**]. **v0.407, "Start collapsed" → bordered Button DONE** (widgets::Button.active(); full-area clickable, shows on-state). **v0.408, stripe → #040404 + animated 1px RGB section dividers DONE** (new `widgets::rgb_section_divider` reusing `row::rgb_from_time`, placed between the 4 section boundaries). **NEW BIG ASK (operator on v0.407 screenshot):** a **UNIVERSAL NESTED-EXPANDABLE WIDGET** with configurable columns, to be reused across HumanityOS. **Garden redesign on it:** tower title row = "Aeroponic Tower make/model/version" + row widgets → expand to **SLOT rows** (slot 1..N, plant + simple stats) → click a slot → expand THAT row to a multi-row **card** with image/3D-model placeholder + a **description** + details. Plus: **fix the "Plant this tower" STACKING bug** (replant kept adding 33 more → 66 → 99; the slot model makes it idempotent), and the **garden section gets collapse/expand** (falls out of the widget). **Sequenced:** **v0.409 DONE**, `widgets::expandable_row` + `row_cell` (the universal nesting primitive + configurable-column cell) + TowerConfig make/model/version (in the RON) + the garden tower groups refactored onto expandable_row with a make/model/version title row + Plant button in the header. **v0.410 DONE**, SLOT model: CropInstance.tower_slot; the tower handler fills fixed slots idempotently (despawn-dead-then-refill), killing the 33→66→99 stacking bug (proven by tower_replant_fills_slots_idempotently); threaded through GuiCrop + sync. **v0.411 DONE** (the visible payoff): SLOT ROWS + click-to-expand plant CARD, each tower renders slot 1..N (plantings flattened so slot index == crop.tower_slot) as nested `widgets::expandable_row`s with aligned `widgets::row_cell` columns (Slot# | name | status | growth bar); click a slot → multi-row CARD = new `widgets::placeholder_tile` (colour from new `widgets::swatch_color`) + name + role + description (planting note) + details grid (Stage/Growth/N·P·K/Water·day/Temp/Reservoir/Health) + Harvest/Water/Fertilize; empty/planned slots show tile+name+description + "Not yet planted". New universal widgets `swatch_color` + `placeholder_tile`. Verified: cargo check clean, `cargo test --lib` 398/0, all 4 gui lints pass. (Also added `test = false` to the bin so `cargo test --lib` skips the native bin link, dodges the Windows LNK1318 PDB-limit; full gotcha + the standalone-`rustc --test` lint workaround in CLAUDE.md.) **v0.412 DONE**: all five inventory MAIN sections (Status/Equipment/You&places/Garden/Mining) are now collapsible via new closure-free `widgets::section_disclosure` (painted triangle, memory-persisted, honors the global force); the Collapse-all/Expand-all/Start-collapsed control bar was hoisted to a GLOBAL top bar that drives sections + nested trees + tower groups in one click. Verified: check clean, lib 398/0, all 4 gui lints. **v0.413 DONE** ("the big one"): INLINE item-detail expansion. The tree widget (`container_node`/`tree_list_ex`) now takes an `inline: &mut dyn FnMut(&mut Ui, &str)` that renders an expand-in-place body under the SELECTED leaf; the backpack passes a closure that draws the new `draw_item_card` (placeholder image tile + category badge + details grid + description + Eat/Drink/Plant/Equip/Drop) from an inventory snapshot. Deleted the old top-of-panel detail + its 5 handlers; item actions apply after the panel. Verified: check clean, lib 398/0, all 4 gui lints. **v0.414 DONE**, Mining + You&places on the universal row: asteroids (expand → per-ore composition bars) / drone manifest (expand → per-asteroid availability; steppers stay in the title row) / active drone (expand → fetching+cargo) all on `expandable_row`+`row_cell`; the places tree's BRANCHES now render THROUGH expandable_row (one nesting primitive app-wide) and item leaves get a fixed name column; 3 new theme tokens `cell_narrow_width`/`cell_short_width`/`cell_name_width` (Settings sliders + css regen; garden literals migrated). **v0.415.0 = a STRAY TAG, skipped** (a quoting-mangled commit tagged the wrong commit; per the never-delete/re-tag SOP we incremented past it, no release exists for it, don't create one). **v0.416.0 DONE, retired-page cleanup + THE RELAY-BUILD FIX:** `save_load` was ungated while importing native-gated `persistence`, so EVERY relay build, CI Deploy to VPS AND `just sync`, had been red for 25 straight releases (v0.381→v0.414) and the live relay binary never rebuilt; one-line native gate fixes it (lesson in CLAUDE.md gotchas: check `--features relay --no-default-features` before pushing Rust). Cleanup: play.rs + resources.rs DELETED (with GuiPage::Play/Resources + ResourceCategory plumbing), the standalone onboarding page renderer deleted (quest-chain machinery kept; first boot now lands on the **Humanity Mission Dashboard**; boot-page picker = Humanity + Library; legacy config strings migrate), and Profile's Quests section retired, live sim quests now render ON the top-level Quests page above the learn-by-doing chains (the operator's "one page, two kinds" model realized). Verified per increment: native+relay check clean, lib 398/0, all 4 gui lints + engine_wiring_lint. **v0.417 NEXT (operator-picked 2026-06-11):** the long-deferred garden PLOT-TYPES arc (soil/sand/pots/trays/direct-sow, infinite-of-X registry) and/or the **First Playable arc** (persistence depth → 3D HUD vitals → walk-up stations → death/respawn → guided first day; proposed + plan-filed this session), operator chose "v0.414 + hygiene first", so the next arc pick is theirs; the web Mission-Dashboard mirror stays queued behind "app first".

**INVENTORY one-panel redesign (operator feedback on v0.397 screenshot, 2026-06-08), DONE v0.400-v0.404.** The 3-panel split (left nav / center Mining / right detail, built v0.390 + v0.396) reads as messy/stair-stepped; the operator wants **back to ONE panel** with a clean rows/columns "Excel spreadsheet" layout (aligned widgets), **status bars capped (~200px)**, and **every width a theme token** (universal, editable in Settings, nothing hardcoded). **v0.400, step 1/2 DONE:** 3 theme tokens (status_bar_width 200, stat_name_width 86, stat_value_width 82) wired through theme.rs + theme.ron + Settings sliders + theme.css regen; `stat_row` caps the bar at status_bar_width (was uncapped = the main stair-step); old STAT_NAME_W/STAT_VALUE_W consts migrated to tokens. theme_editor_coverage passes (100% token coverage). **v0.401, ONE PANEL DONE:** reverted the 3 SidePanels into a single CentralPanel via boundary-checked wrapper surgery (1166→1136 lines, no content moved); the default (nothing-selected) view is a clean single column with the v0.400 capped bars. **v0.402, SPREADSHEET GARDEN DONE:** the garden is now a grouped TABLE (one collapsible group per tower with a Plant button; columns Plant|Stage|Growth|N·P·K|Water/day|Temp|State|Actions, growth bar capped at status_bar_width, Harvest/Water/Fertilize inline; respects Collapse/Expand-all). Detail block simplified to item-only; dead tree helpers removed. **The operator's full inventory feedback (one panel + spreadsheet + capped bars + tokenized widths) is addressed across v0.400–v0.402.** **v0.403, GARDEN ROW POLISH DONE** (operator happy with v0.402, asked to tighten): new `widgets::compact_button` (short, tight inline button, fixes the overlapping Harvest/Water/Fertilize; buttons were taller than the tight rows), `status_bar_height` token (5px thin bars for vitals + garden growth), `compact_button_height` token (18px), tighter grid spacing; both tokens editable in Settings. **v0.404, MINING ALIGNED DONE:** asteroids → Grid [Asteroid|Type|Ores]; drone manifest → Grid [Ore|Available|In-manifest with the [-] qty [+] steppers]; active drones → Grid [Drone|Status|Fetching|Cargo] + a thin capped progress bar. **The operator's full v0.402-screenshot polish message is complete (garden rows + compact buttons + thin bars + mining alignment).** **Queued minor:** the selected-item detail still renders at the TOP on item-click (relocate inline if wanted). **Then the big one:** the deferred garden PLOT-TYPES arc (infinite-of-X gardens: aeroponic/soil/sand/pot/direct-sow/trays, data-driven) the operator opened.

<!-- Set this to the single most important thing right now. -->
**ACTIVE: UI / merge-pages arc (2026-06-02).** Operator design-collab from live play, shipping universal widgets + a page consolidation toward a slim **Real | Play | Platform** top nav (operator confirming the bucket mapping before pages move). LANDED: compact `stat_row` table (v0.345), drone capacity manifest + styled square `stepper_button` (v0.346–0.348), **inventory as a uniform container-node `tree_list`** (v0.349, operator greenlit; the SAME widget scales from a toothbrush to a planet), **Studio quick-access in the chat right rail** (v0.350, native mirror of the website's studio widget; survives the nav condense so livestreaming is never stranded), **`section_nav` universal sidebar/TOC widget** (v0.351, generalized Profile's grouped switcher sidebar into `widgets::section_nav` + `SectionNavItem` and refactored Profile onto it to prove universality + DRY out the duplicate helpers; click-action-agnostic so the one widget serves both switcher pages and infinite-scroll TOCs), **chat composer fix** (v0.352, the "… is typing" row no longer shoves the text field under the taskbar in snapped-fullscreen), and the **Earth-rooted container hierarchy** (v0.353, data-driven `Place` spine in `data/places/seed.json`; the inventory now shows your backpack INSIDE Earth → WA → Silverdale → home → You → Backpack via the same `tree_list` rooted at the planet, with Silverdale's real lat/long shown as the terrain bridge; the seed is per-user personal data, deliberately not embedded, distributed builds fall back to flat), and **inventory = entity-first + colour-coded + populated** (v0.354, top level is now ENTITIES [You, your home, your 1975 Chevy Nova] with `location` as an attribute rather than the deep Earth chain; `tree_list` nodes get colour swatches by kind so "what is where" reads at a glance; the home is seeded with a realistic 58-item Mt. Rainier 3-day summit kit in nested containers). and the **`lockable_gate` widget** (v0.355, operator-specified `[title] [show/hide] [unlock] [passphrase]` private-section lock: collapsed + locked by default, real vault-passphrase verification via `decrypt_private_key`, in-memory-only so a restart re-locks; demoed gating the entire Wallet page). and applied to the **seed-phrase reveal** in Settings (v0.356, re-enter the passphrase to show the 24 words; was previously a one-click reveal with no passphrase). Verified that Identity (public DID lookup) + Recovery (social-recovery setup) are NOT private data and correctly left them unlocked. NEXT: **(a)** the **Profile PRIVATE group** (Body & Measurements, Private Notes) is the remaining lock candidate, pending the operator's call on what they consider private; **(b) container arc increment 2** = in-app place editing (add/rename/move rooms + containers + worn-item slots under You, GUI-first) so each user defines their own; **(c)** the **Real | Play | Platform** page consolidation (recommendation delivered to the operator); **reconcile the two tree widgets**, NOT a straight migration (scoped v0.351): the closure-based `widgets/tree.rs` (sole consumer = the Files page) supports **lazy children** (a file browser can't build the whole filesystem eagerly), **per-node icons**, and **colored leaves**, all of which the eager data-driven `tree_list` lacks. So "one universal tree" = unify the ROW STYLING into one shared renderer + co-locate both as two documented MODES of one tree module (eager `tree_list` for bounded trees like inventory; lazy closure `tree_node` for unbounded/dynamic trees like the filesystem), retiring the divergence, NOT forcing Files onto a crippled eager API. **CONFIRMED + IN PROGRESS**, the page carve is **Humanity · Chat · Real · Play · Platform** (operator 2026-06-03; the H button now opens Humanity, not chat). Step 1 (nav regroup into the five + H→Humanity + all bucket decisions) shipped v0.357. Step 2 = fold each tab into ONE `section_nav` page, tab by tab. **REAL FOLDED v0.358** (6 buttons → 1 "Real" tab, Profile's sidebar flattened in + Possessions/Wallet/Tasks/Map/Market). KEY unlock: the **delegate pattern** (the tab renders a SidePanel `section_nav` + delegates the content to the sub-page's existing `draw`, whose CentralPanel renders beside it) means **no per-page rewrite**, so Play/Platform/Humanity fold the same way. **ALL FOLDED v0.360**, the top nav is now the clean **6: H · Humanity · Chat · Real · Play · Platform · Settings** (was ~20 buttons; H opens Humanity; **Settings pulled to its own top-level tab v0.361**, the most-accessed page, never buried). **Design rule** (operator, load-bearing for the rest of the carve): most-used → top-level (1 click); rich-feature pages may keep their own 2nd-level sidebar (the double-sidebar is fine for Map/Crafting, NOT for Settings); big single pages → internal scroll + section TOC; keep top-level tabs ≤~10. NEXT: **Humanity Mission Dashboard built v0.362** (mission + the three scopes + live scoreboard + CTAs + the AI-as-Humanity line, from `docs/design/humanity-page.md`; landing-page quality). **Polished v0.363:** zero em-dashes (now BANNED in all user-facing copy, they read as machine-written), leads with the personal "why" (one family's survival is everyone's, hence CC0), and a single **H/Humanity** tab (the brand H became the tab's icon; the duplicate brand button is gone). **v0.364, the em-dash ban is now enforced APP-WIDE:** the whole native UI, all of web, and the user-facing data files were swept (~1500 occurrences across ~140 files; code comments, dev docs, vendored/generated files, historical message archives, and the backend relay strings were intentionally left), and `tests/emdash_lint.rs` (the `theme_token_lint` analog) now FAILS the build on any new em-dash inside an `src/gui/` string. The web is em-dash-clean too, so the web-landing mirror inherits a clean base. NEXT for it: **(i)** wire the platform-wide **scoreboard metrics** (relay fetch → GuiState: humans/AI onboarded, donations, federation totals, only online-now is live so far); **(ii)** **mirror it to the WEB landing**, the operator's actual goal is for this page to *replace the website's landing page*, so port the same structure/copy to `web/`). **UPDATE (operator 2026-06-05): app FIRST, web later.** Do NOT touch the live landing yet. Get the mission right IN THE APP, then mirror so the experience is consistent (first visit or millionth, web or app). FINDING: the live `index.html` is already rich + mission-led (hero "End poverty. Unite humanity." + a full vision section + the concrete "free water/energy/food" hook), so naively replacing it would REDUCE content; when the app is right, lead the landing with the mission while keeping its concrete hooks. DURABLE PRINCIPLES: the individual ("what I'm doing") AND the civilization ("what we're doing together") must both be instantly identifiable to anyone of any age or state of mind; consistent + predictable + easy to follow; every added PAGE is friction, minimize sprawl ("avoid another dysfunctional US government"). v0.365 sharpened the native Humanity hero to name BOTH scopes (For you / For all of us) as the page spine. **v0.366** (operator feedback on the live build) added the MECHANISM he asked for: a **"Why it's built this way"** card mapping each system (quests+sim, encrypted chat, tasks/notes, maps, marketplace+trust, owned-identity+federation, CC0) to its poverty-ending design, the way the Onboarding page is concrete about it. And **fixed the Resources page** (operator: "barely populated... maybe because we don't have an all category") with an **All** default view + a dense wrapping link grid + a populated catalog (6→10 categories, 14→37 links, led by Water/Energy/Food/Housing). **v0.367** made the onboarding QUESTS universal (operator: the "Lake Tahoe" line is an unverifiable place/event claim with no link, and too US-specific; each quest must read as something that applies *everywhere*, Sahara to tundra) by rewriting `data/onboarding/quests.json` (native + web both load it) to strip the place/event claim, nation stats, and currency, reframing around universal principles + climate-spanning options. And added a **"What it protects"** Mission Dashboard section (the liberty half beside the poverty mechanism): free speech first (operator: "loss of free speech is a death sentence for a free and independent people"), then privacy, ownership, and self-governance, each showing how the *design* defends it. **v0.368** mined the docs (the Humanity Accord + two Explore agents) to deepen the pitch so it shows the platform helps in EVERY scenario: enriched "Why it's built this way" (data-driven/moddable: the tool bends to any place) and "What it protects" (post-quantum + zero-knowledge "even we can't read your DMs", never-locked-out via social recovery, and the keystone **"Bound by a constitution, not a promise"** = the Accord as CC0 law), and added a new **"Built for every situation"** section (no internet / server-down / device-loss / no-money-or-papers / any-language-or-ability / disaster). All claims SHIPPED except the radio mesh, flagged "(in progress)" to stay honest. The Mission Dashboard is now a comprehensive, scenario-spanning manifesto. **v0.369** added an in-app **Humanity Accord viewer** (embedded via `include_str!`, a reusable lightweight markdown reader + "Read the Humanity Accord" button) and an **"Early days, built in the open"** early-access section (states plainly the app is early, flags "in progress", reframes bugs as building-in-public participation). **NEXT TWO INCREMENTS (operator-chosen 2026-06-06 via AskUserQuestion):** **(1) LIBRARY = a new top-level tab** holding all the docs in a nested tree like the inventory (reuses the v0.369 doc viewer; data-driven manifest so docs are added by file, not code). **(2) QUESTS = one page, two kinds**, a single Quests page with BOTH gameplay quests (auto-track + XP) AND learn-by-doing chains (manual check, deeper per-method, e.g. a chain for each way to collect/purify/store water), and **retire the standalone onboarding page** (the Mission Dashboard now covers that ground). **Library SHIPPED v0.370** (new top-level tab; a data-driven docs tree + in-app markdown reader holding the Humanity Accord + 16 companion docs, curated by `scripts/build-library.js` into `data/library/`). **v0.372 expanded it** to a 3-level tree: the Accord docs nest under a **HumanityOS** section, and a **Tools and Websites** section lists the external links (sourced from the shared `resources/catalog.json`, so links live once). Doc entries render inline; link entries open the browser. **Open:** the Resources page now overlaps the Library's links (consolidate, or keep both? operator's call). **QUESTS UNIFIED (v0.373):** one learn-by-doing **Quests** section under **Real** (LIFE group), onboarding chain (First Steps) pinned top, the sim woven into the chain steps as practice (no game/real duplication). The Profile game-quests panel + the standalone onboarding page are retired (the page's hero/concepts overlap the Mission Dashboard; "Get oriented" now jumps to Real › Quests). **Cleanup follow-up:** delete the now-dead `profile::draw_quests` + `onboarding::draw` (and decide whether the in-game quest-registry/XP still surfaces anywhere). **v0.374 batch:** Resources **retired into the Library** (Humanity sidebar item removed; quest text repointed); Library **sections collapsible** + a **search filter** (so the websites scale to 1000+); **Quests** + **Studio** promoted to **top-level tabs** (Quests = the learn-by-doing path; Studio = livestreaming, right of Chat). Nav is now Humanity · Chat · Studio · Real · Quests · Play · Platform · Library · Settings (9 tabs). **Cleanup list grows:** `resources::draw` joins `profile::draw_quests` + `onboarding::draw` as now-dead page renderers to delete in one pass. **DIRECTION SHIFT (2026-06-06):** remove the **Real/Play distinction**; make the survival/production systems (garden, inventory, crafting, market) first-class top-level features that just WORK; add **Crafting** top-level. **NORTH STAR (keep in mind throughout):** every in-game system maps to a real-world system a person can actually build → a **parts list** (3D-print / buy / trade) → and LAST, the app **automates + monitors** those real systems (control layer for your real homestead, e.g. real aeroponics). Design the game systems NOW as real buildable things. (This supersedes the old "Real vs Sim = separate pages" firewall for now; real/sim separation returns last.) **REORG PENDING:** operator unhappy with the cramped Real page; I proposed Profile | Home(homestead) | Crafting and asked him to pick the granularity before building. **v0.375** did the **Library refinement** he asked for: websites are now a single **External Resources** card list (full-width cards, search + tag-filter chips, scales to thousands) and clicking one opens an **in-app detail page** with a "Load website" button instead of launching the browser immediately (an in-app browser is the noted future enhancement). **Decided next:** **Inventory** must be a **top-level page** and **Crafting** its own page "for now". **Still designing with the operator:** the inventory page layout (he wants nested-tree-left + details-right; unsure 2 vs 3 panels), what combines elegantly into inventory (crafting GUI is nearly identical), and the full Real/Play removal + Home/Profile grouping. **v0.376 shipped NAV REORG step 1:** Inventory + Crafting are now top-level tabs (the one-item Play fold is retired; nav = Humanity · Chat · Studio · Real · Quests · Inventory · Crafting · Platform · Library · Settings, 10 tabs). Kept nav-only/low-risk: dead Play left for the cleanup pass, Real keeps its Possessions shortcut for now, and the inventory internals (the 2-vs-3-panel rebuild) untouched pending the operator's panel call. **NEXT for the reorg:** the inventory-internals rebuild (a shared tree→contents→detail widget Crafting reuses; my recommendation = 3-pane file-manager style, garden as a container node) once the panel direction is set, then the full Real/Play removal + Home/Profile grouping. **v0.377 shipped the PLAY button** (operator: "add a dedicated button for the FPS game part ... Click Play to start FPS game mode"), FPS mode IS `GuiPage::None` (nav + pages hide, cursor grabs), so Play is a nav button → `None`, leading the game cluster (Play · Quests · Inventory · Crafting), with a play-triangle icon; Esc still toggles back. **HOMES-AS-PROFILES model ADOPTED (operator, 2026-06-07; new `docs/design/homes-as-profiles.md`):** each **home is a save profile** rendered by ONE shared homestead UI, differing only by a **kind** toggle, **Offline** (local sim; the first offline home = the 100%-self-sustaining **Fibonacci** design = the gamified blueprint others copy), **Server** (multiplayer relay), **Real** (bound to real monitoring/automation hardware; the north-star control layer, built LAST). Homes are built from **designs** (first = Fibonacci). This **dissolves the Real/Play firewall**, real-vs-game becomes a property of the home you're in, not a page split; Play enters the world for the *selected* home. **Sequenced:** Play DONE → home model + home-select → offline Fibonacci design → server homes → parts-list-from-design → real homes LAST. The save-model rework is NOT built yet (open forks in the doc: can a home change kind? design-vs-home split? where home-select lives? what a Real home shows, operator's architecture call). **v0.378 shipped a big operator batch (from the live build):** Play moved leftmost; **Real renamed to Profile**; **Tasks** + **Map** promoted to top-level tabs (Tasks right of Quests; Map = Cosmos); nav is now **Play · Humanity · Chat · Studio · Profile · Quests · Tasks · Inventory · Crafting · Map · Platform · Library · Settings** (13). Profile's sidebar trimmed to Profile-sections + Wallet + Market; **Possessions/Tasks/Map removed** (now top-level), **Streaming moved into Studio** (public channel URL + LIVE flag). Added a **profile selector** (one "Base" character for now). **Library External Resources cards are now self-contained:** a **"Load website" button at the TOP** of each card + all the data (title/tag/desc/url) below; the click-to-detail gate is gone (only the explicit button launches). **Chat/Studio persist-load:** documented as already-true (state lives in GuiState; nothing is torn down on nav; the forward rule, real stream pump runs in the engine, not the page, is in studio.rs's header). **CHARACTER MODEL adopted** (homes-as-profiles.md new Characters section): one **base character** (real self, typed once) + per-server **augmented** versions (shared base look) + **shared-vs-locked inventory** (Diablo II offline/open-bnet vs closed/ladder; servers can force starter gear). Character (who you are) enters a Home (where/what world). The character data-model (multi-character saves, shared/locked stash) is captured with open forks, NOT built. **v0.379, HOMES increment 1 (operator: build it + "I like your suggestions" + "keep developing offline, multiplayer after singleplayer works"):** an Explore survey found the Fibonacci homestead ALREADY exists as rich data (`data/blueprints/fibonacci_homestead.ron`, 13 rooms F1→F233 with materials + power + water), so I **surfaced** it rather than authoring it. New `GuiPage::Homes` + `pages/homes.rs` (a "Home" top-level tab, paint_house icon, by Profile): a read-only **Design browser** with a **scale selector** (Solo/Family/Community/Colony) showing, per scale, the total power + water **demand**, the aggregated **bill of materials** (#3), a **self-sufficiency summary** (solar/wind/water/greenhouse/composter counts = #4 partial), and rooms by tier. Structs + `load_homestead_design` (RON) in `gui/mod.rs`. **Improvements ADOPTED** (operator-approved) as the direction: #1 base character = the existing crypto identity (no second "you"); #2 home kind = who owns the truth (you/server/sensors); #3 BOM-in-blueprint (done); #4 closure score (started; exact output-vs-demand needs generation-capacity data = next layer); #5 forkable/signed CC0 designs; #6 Real home = digital twin; #7 progressive disclosure. **NEXT (offline):** increment 2 = the Home **save-wrapper** (`WorldSave` gains `kind=Offline` + `design`; minimal home list; Play enters the selected home) + ground the base character in identity (#1); then the closure-score data layer (#4). Server/Real/character-augmentation stay deferred until offline singleplayer is solid. **v0.380, self-sufficiency model doc + save-wrapper model (operator: "what variables are there to consider?" + "start the save wrapper"):** (1) new `docs/design/self-sufficiency.md` breaks down homestead self-sufficiency as **coupled loops** (energy/water/food/waste/air/thermal/materials), each supply/demand/storage/loss over time, gated by **location+climate, scale, autonomy margin, time horizon, loop-coupling**; key truths: energy is **Wh+storage not nameplate**, self-sufficiency = the **limiting loop** (Liebig's minimum); proposes a buildable **score** (per-loop supply/demand ratio + autonomy-days + overall = the weakest loop, gated by place+household, same metric sim/real). (2) **Save-wrapper MODEL**, finding: the game has **no working save/load** (persistence is test-only; entering regenerates the homestead fresh). So built the model now, lifecycle next: `WorldSave` gains `kind` (default "offline") + `design` (default "fibonacci") via serde defaults (zero migration), a `new_offline` constructor, 3 passing tests (incl. legacy-JSON-defaults). Home page frames the design as **your offline home** (one home; progressive disclosure). **NEXT (offline):** increment 3 = the **save/load lifecycle**, extract live state (inventory/skills/time/crops/constructions) ↔ WorldSave, load the active home on world-enter, auto-save on exit (the game persists *nothing* between sessions today, so this is real value); then the self-sufficiency **score** data layer once the operator reacts to the variables doc + sets the editable component-output numbers. **v0.381, SAVE/LOAD LIFECYCLE (homes increment 3):** offline progress now **persists between sessions** (it persisted *nothing* before, `persistence` was test-only). An Explore survey found the tick is **ungated** (systems run in menu loops + 3D), so the ECS player is authoritative from startup. New `src/save_load.rs`: extract/apply player **inventory + skills** ↔ a single `offline_home.json`; **applied at startup** (which also makes exit-save safe, the player carries the loaded state, so a no-play session round-trips instead of overwriting empty); **saved** on window-close + a **periodic** self-throttling save (robust to in-app-quit/crash). Scope = inventory + skills only (lowest coupling, no glam/schema change); **deferred** health/position/game_time (TimeSystem owns its clock)/vitals/crops/quests → reload = "wake rested at home with your stuff + skills intact". 2 tests pass (round-trip + offline-kind guard). **NEXT (offline):** extend persistence to health/position/game_time, then vitals + crops (needs a no-double-spawn guard) + quests; then the self-sufficiency score data layer. **v0.382, AEROPONIC TOWERS increment 1 (operator new task):** two curated **50-plant aeroponic tower** configs as data (`data/towers/aeroponic_configs.ron`), the homestead's **food loop** made concrete. An Explore found every target species already in `data/plants.csv` (128 plants) → zero new plant data, pure curation; and nutrition isn't a runtime sim (satiation/hydration only) → the towers document their design. **Tower 1 "Daily Greens and Beans"** (nutrition): 17 species (greens/brassicas/legumes incl. soybean protein/fruiting/roots/allium) with a `covers` list + an HONEST `gaps` list (bulk calories, fats, B12, D, omega-3 = field crops/ranch/sun/animals, not a tower). **Tower 2 "Remedy and Flavor"** (apothecary): 21 herbs with culinary + **traditional/folk** medicinal notes + a disclaimer (not medical advice; per-plant cautions: comfrey topical-only, valerian sedative, st_johns_wort interactions, feverfew not-in-pregnancy). New `TowerConfig`/`TowerPlanting` + loader + a collapsible **Aeroponic towers** section on the Home page (per-tower covers/gaps/disclaimer + the 50-slot planting list). Test asserts RON parses + both sum to 50. **NEXT:** increment 2 = the **3D placeholder** (cylinder Mesh helper + a placed tower entity + simple plant markers), then increment 3 = **farming integration** (plant a tower → crops grow → harvest → nutrition). **v0.383, increment 2 DONE: MAX-VARIETY reframe + 3D placeholder.** Operator: "max variety, one of each (1 lettuce + other types), make sure they grow together, excited for the placeholder." Reframed both towers to **maximum variety** (33 distinct food + 24 distinct herbs, one of each; capacity stays 50, room for the community). Captured the **compatibility insight**: aeroponics shares a reservoir + air, NOT soil, so soil companion/adverse rules relax; the real constraint is a shared **reservoir pH + temp/humidity window** (which widens the variety), a check computing it from `plants.csv` is the next feature. Built the **3D placeholder**: `Mesh::cylinder` + `Mesh::sphere` helpers + a grey cylinder per tower + a green sphere per variety in a helix, placed on the **garden** floor (markers capped at 12/tower; render is crash-safe via the 256 soft-cap). Proposed improvements: tie the tower to the self-sufficiency loops (sum `water_liters_per_day` + harvest timeline from `growth_days`), community tower "recipes", staggered planting. **v0.384, increment 2b: fixed inverted cylinder normals + DATA-DRIVEN geometry.** Operator (screenshot): "normals inverted, seeing the inside; design towers + amount dynamically; wide diameter + adjust helix density like coarse/fine bolt threads." Fixed `Mesh::cylinder` to use **row-major rings + the same winding as the working sphere** (now outward-facing; dropped the inverted caps, it's a solid-walled open tube). `TowerConfig` gains **`diameter_m` / `height_m` / `helix_turns`** (serde defaults); the RON sets them per tower to show it off (nutrition wide+fine 0.6/2.4/6, apothecary narrow+coarse 0.32/1.9/2.5). `load_world` reads each tower's geometry → per-tower cylinder + one marker per **curated variety** (dynamic count) along the helix. **NEXT (increment 3):** the **compatibility check** (plant pH/temp/humidity shared window + outlier flags) + the **farming hookup** (plant a tower → CropInstances grow → harvest). Deeper capacity-from-geometry math waits for the planting loop. **v0.385, towers in the inventory (operator confirmed the winding fix worked + flagged "not seeing the towers in the inventory").** The towers now appear in the inventory's **"You & your places"** tree under **Home** (matching the container model: a tower is a structure holding plants), `inventory.rs::towers_tree_node` builds an "Aeroponic Towers" node → each tower (name + plant count + height) → its planted varieties. Display-only for now. **v0.386, increment 3: FARMING HOOKUP (plant a tower → crops grow → harvest).** Operator: "keep developing the rest of the features." The towers are now **functional**, reusing the existing gardening loop. An Explore found the GUI→ECS bridge (GuiState flag → DataStore Mutex channel → FarmingSystem drains) + CropInstance growth/water/harvest + the Garden render are all reusable. Built: `GuiState.pending_plant_tower` → a new `plant_tower_request` channel → a FarmingSystem handler that spawns one CropInstance per tower planting (dev-friendly: **no seed cost** yet, to get it working) → the crops auto-mirror to `GuiState.crops` and render in the inventory **Garden** with the existing Water/Harvest/Dev-grow controls. A "**Plant a tower:**" row of buttons in the Garden section drives it. So: plant tower → ~33 crops appear → grow → harvest into inventory + Farming XP, with zero new growth/harvest code. 4 farming tests pass (incl. the full loop). **NEXT:** the **compatibility check** (plant pH/temp/humidity shared window); a `tower_id` on CropInstance so the inventory tower nodes show "N/M ready"; then the seed economy + the real-world parts list. **v0.387, FIX: plant-a-tower did nothing.** The `plant_tower_request` channel got registered only in the **test** `make_store` (the `data.insert` anchor matched the `#[cfg(test)]` helper first), not the **runtime** (lib.rs ~984). At runtime the channel was absent → nothing spawned. Fixed by registering it in the runtime next to `plant_request`. The rest of the loop is ungated (the menu garden works without entering 3D). **Lesson logged:** a new DataStore channel must be added to BOTH the runtime (`lib.rs` resumed) and the test `make_store`; `replace_all:false` hits the test first. **v0.388, GARDEN crop list = compact table (first slice of the operator's inventory redesign).** Operator confirmed the farming loop works + asked to redesign the crop list: nested/collapsible-by-tower, single row not three, fixed columns, ~200px progress bar (full width is rough), upgrade the inventory page to left+right panels, maybe add NPK/nutrient/water/temp columns. Shipped the highest-value low-risk slice: the Garden crops are now a compact **egui::Grid table** (one row each, fixed aligned columns Plant/Stage/Growth/Water+Health/Actions, 200px bar). **v0.389, GROUP BY TOWER + COLLAPSIBLE (the operator's lead request done).** Added a crop→tower linkage (`tower_id: Option<String>`) to CropInstance + GuiCrop, threaded end-to-end (`pending_plant_tower`→`(tower_id, plant_ids)` tuple + the `plant_tower_request` channel [registered in BOTH lib.rs runtime AND farming make_store, the v0.387 lesson] + the FarmingSystem tower handler tags each spawned crop + lib.rs sync mirrors it). The Garden render now groups crops by tower into an `egui::CollapsingHeader` per tower (title = tower-config name + "N/M ready", default_open, id_salt) each holding the v0.388 compact Grid; seed-planted crops fall under "Other crops". cargo check clean, release exit 0, emdash 2/2, farming 4/4. **v0.390, INVENTORY = LEFT nav + RIGHT detail + CENTRAL workspace (the operator's "at least left + right panel" done).** The vitals/equipment/tree/garden/mining were all crammed into one shared CentralPanel+ScrollArea (one tall column); rewrote the panel boundaries into a resizable left `SidePanel` (status + equipment + the "You & your places" tree) | the existing right item-detail panel | a central workspace (Garden + Mining). File-manager shape: structure left, act in the center, selected-item detail right. All FOUR gui lints green, farming 4/4, release exit 0; inventory.rs diff a tight 30 lines. **LESSON (load-bearing):** do NOT run `cargo fmt` in this repo, it is not maintained rustfmt-clean, so a whole-crate fmt churned 242 files AND moved a trailing `// theme-exempt:` comment off its line (line >100 cols), silently breaking theme_token_lint; caught via `git diff --stat` before pushing, reverted all fmt-only files, re-applied by hand. Match surrounding style manually. And when touching `src/gui/` or `src/renderer/`, run ALL FOUR style lints (emdash, theme_token, theme_editor_coverage, icon_glyph), not just emdash. **v0.391, NPK + Water/day + Temp COLUMNS on the crop table (the redesign's last "maybe" done).** The data was in plants.csv but dropped at parse; extended the farming data model (PlantRow + PlantDef + from_csv) to parse nutrient_n/p/k, ph_min/max, temp_min_c/max_c, humidity_min/max (all serde-default), surfaced N/P/K + water_per_day + temp into GuiCrop (via the lib.rs sync's existing PlantDef handle), and rendered three compact columns ("N·P·K", "Water/day", "Temp") in the grouped Garden Grid. ph/humidity parsed too (not shown yet) for the compatibility check. farming tests 5/5 (added a full-row parse lock), all 4 gui lints green. Width watch: 8 columns + 200px bar is wide; fits on a large monitor + resizable left panel, move needs→tooltip/detail if it clips. **The operator's entire inventory-redesign message is now fully shipped (v0.388 table → v0.389 group-by-tower → v0.390 left+right panels → v0.391 needs columns).** **v0.392, TOWER COMPATIBILITY CHECK done (the "make sure they grow together" feature).** Aeroponics shares one reservoir + air (not soil), so the constraint is a common pH/temp/humidity window. New `TowerCompat` + `compute_tower_compat` (gui/mod.rs) intersect each axis across a tower's species (Some window = shareable; None = conflict, naming the binding plants), cached in `GuiState.tower_compat` (computed once in the lib.rs crop sync), rendered per tower on the Home page (green ✓ shared-window line, or ⚠ per-conflict lines + a split hint). Tests 2/2, farming 5/5, all 4 gui lints green. **The aeroponic-tower feature is now end-to-end: curate → Home browser + compatibility → 3D placeholder → inventory tree → plant/grow/harvest loop → grouped crop table with NPK/water/temp.** **v0.393, TOWER REAL-WORLD PARTS LIST done (the north-star game→real bridge).** Each tower now carries a data-driven `parts` bill of materials (TowerPart {name, qty, source, note} on TowerConfig; ~9 standard tower-garden parts per tower in the RON, scaled by geometry), shown on the Home page under its plantings, framed as a refinable starting list (the operator/community tune the values via the data file). Tests assert ≥5 parts each with a source; all 4 gui lints green. **v0.394, TOWER SELF-SUFFICIENCY NUMBERS:** each tower shows its total daily water draw + harvest window on the Home page (folded into compute_tower_compat; uses PlantDef water_per_day + growth_days). **The aeroponic-tower feature is now COMPREHENSIVELY end-to-end: curate → Home (browse + compatibility + water/harvest + parts list) → 3D placeholder → inventory tree → plant/grow/harvest → grouped crop table with NPK/water/temp.** **OPERATOR FEEDBACK BATCH (2026-06-08, with screenshot of v0.394 inventory):** three directed asks, **(1) Creative/Survival mode**, default Creative during early dev, so the seed economy can be built without needing seeds yet [**DONE v0.395**, GuiState.creative_mode default true, bridged to a DataStore flag farming+crafting read to skip resource requirement+consumption; toggle in the inventory left panel]; **(2) the GARDEN belongs in the LEFT nav tree, not the central panel**, I put it wrong; clicking any entry (inventory item OR garden plot/crop) should show its details in the RIGHT panel [**DONE v0.396**, garden is now a "Garden" tree section in the left panel (towers=plots → crops); the right panel renders crop/tower/item detail by selection; the central panel is Mining-only; the tree widget now allows selectable container headers so a planted tower can be picked; the 8-column table is retired]; **(3) collapse/expand-all buttons + a "default collapsed" checkbox** for the nested lists (inventory + garden) [**DONE v0.397**, `widgets::tree_list_ex(default_open, force)` + a Collapse-all / Expand-all / "Start collapsed" control bar driving both trees; trees_start_collapsed defaults true so they start collapsed every launch (no GUI-pref persistence exists yet, so collapsed-by-default is how "start collapsed on first load" is honored)]. **The operator's three-ask batch is COMPLETE.**

**SEED ECONOMY arc (operator-directed via AskUserQuestion, 2026-06-08).** **v0.398, step 1 DONE:** 47 new seed items in items.csv (one per distinct tower variety; generated from plants.csv names); SURVIVAL-mode planting consumes seeds (tower handler consumes one seed/variety, skips unseeded; individual plant already did); CREATIVE (default) is free; a "Dev: stock seeds" button grants the **one-seed-of-each starter set** so survival is testable. Seeds are **plot-agnostic**. **★ BIG OPERATOR VISION (durable, drives the next arc): GARDENS ARE INFINITE-OF-X.** Aeroponic towers are ONE plot type. He wants soil beds, **sand** (he has sand at home, wants to recreate it), pots "made out of anything", direct-sow (no tray), **optional** seedling/sprout trays, and "all possible options, including those we haven't thought of yet" = a **data-driven plot / growing-method registry**. Seed acquisition = ALL of harvest-yields-seeds + optional tray + buy/trade. Starter set = one seed of each. **v0.399, (a) harvest yields seeds DONE:** a survival harvest returns produce + 2 seeds of that plant (creative stays clean), so the loop is self-sustaining (plant 1 → harvest → 2 back → replant + surplus). **The basic seed economy LOOP is now complete: seed items + survival-consume + starter-grant (v0.398) + harvest-regenerates (v0.399).** **NEXT (seed economy, sequenced):** (b) **data-driven garden PLOT TYPES**, generalize "tower" into a plot/method registry (aeroponic / soil / sand / pot / raised-bed / direct-sow / …), each data-defined and moddable (the larger architectural arc he just opened, infinite-of-X applied to gardens); (c) optional **seedling-tray** sub-system; (d) on-new-game starter grant. Keep all of it plot-agnostic. Future from this batch: seedling/sprout harvesting trays; the player starts with a base plant set when the game is ready; the **seed economy** is now UNBLOCKED (build it gated behind survival mode). **Still-standing backlog:** the 8-column-table width watch, tower follow-ups (deepen compat / parts→BOM), the web Mission-Dashboard mirror. (The dead-renderer cleanup SHIPPED v0.416.0, resources/play/onboarding-page deleted, game quests folded into the Quests page.) **(Older, now-stale) NEXT: the QUESTS rework** (one page, two kinds: gameplay quests [auto-track + XP] + learn-by-doing chains [manual, deeper per-method, e.g. each way to collect/purify/store water], and retire the standalone onboarding page). Dedupe DONE (v0.371): `render_markdown` now lives once in `widgets::markdown`, used by both `library.rs` + `humanity.rs`. **Open follow-ups on the mission page:** keep iterating the Mission Dashboard copy/flow to the operator's taste; the real scoreboard metrics stay deferred (placeholders are honestly framed for this early stage); the web-landing mirror still waits until the app reads right. **Condensation pass** (runs alongside the folds): each section should FILL its row cleanly like the Settings page rather than leave a small section + 98% blank, Skills done (v0.359, `egui::Grid` one-row), Settings de-stepped (v0.359, 170px label column); next, spread the loved `tree_list` nested-list to **Notes / Tasks / Quests** (operator's ask, "a great way to condense a ton of information", but "don't overdo it"). The switch-one-section model (the Settings model the operator likes) stays, infinite-scroll is NOT required; condensing sections is the fix. Separate pages stay whole (delegate model) for **web parity**; the landing tab on launch becomes a setting (VR "boot into Play" still works). Decisions: **2 maps** (the real map carries toggleable Humanity LAYERS, donation/pothole/member pins, opt-in coarse location like "Silverdale, WA"; the sim map lives in Play); Recovery→Platform; Resources+Identity→Humanity; **Civilization → the Humanity Community/Mission Dashboard** (repurpose its empty sim metrics to real collective ones). **DESIGN THREADS the operator set this arc** (full detail in `orchestrator_state.json` v0.350 entry): **container model extends to the PLANET**, "mark Earth as my container"; the real-self root chain is Earth → region (Washington/Kitsap) → Silverdale WA → ~1-acre property → 2-story home → rooms → containers → items (uniform tree, just a higher root; the player is a node). **Real-terrain world-gen north star**, in-game terrain heightmaps should semi-match real life (source real DEM/elevation data, USGS/SRTM/Copernicus, keyed to the container's lat/long; a teaching/familiarity feature). **REAL vs SIM = SEPARATE PAGES, never a toggle** (a forgettable toggle is a trust risk for the mission, the page itself is the firewall). Tracked deferrals: deep container nesting needs ECS containers-within-containers (inventory is flat slots today); variable-icon detail-level + compact toggle for `tree_list`; draggable segmented-divider drone manifest; the empty 3D home (no placed crafting stations, world-content arc).

**COMPLETE: Gameplay-loop arc (2026-05-30).** Operator vision brain-dump + "develop as if the user unlocked everything 100%", wire the actual play loops on top of the now-wired engine. Holistic map in `docs/design/gameplay-loops.md` (survival needs → production chain → connective systems → threats → progression-LAST). ~40 systems already exist in code (the engine_wiring_lint deferred list), so this arc is WIRING loops + spawning content + GUI→ECS glue, NOT writing new systems. **Build order: (1) full-unlock dev provisioning ✅ + (2) real crafting loop ✅ (SHIPPED v0.329.0); (3) cooking + nutrition ✅ (SHIPPED v0.330.0, Vitals + StatusEffects + FoodSystem registered; eat/decay/conditions/poisoning/well_fed); (4) gardening ✅ (SHIPPED v0.331.0, plant/water/harvest + Garden panel + dev-grow; closes garden→cook→eat); (5) drone↔asteroid mining ✅ (SHIPPED v0.332.0, AsteroidBody + Drone + DroneSystem; commission→trip→mine finite asteroid→deliver→delete-when-empty; Mining panel); (6) refining-chain depth ✅ (SHIPPED v0.333.0, nickel/platinum/stainless ingots + smelt recipes; 2-tier ore→ingot→alloy; closes mine→refine→craft); (7) survival systems online [IN PROGRESS], #7a energy/rest ✅ (v0.335.0); #7b environment-coupled oxygen/temperature ✅ (v0.336.0, homestead-AABB context → oxygen drain/hypoxia/suffocation + body-temp/hypothermia when exposed; hunger now tangible too); WeatherSystem registered ✅ (v0.337.0, drives the exposed-environment temperature; first deferred sim system live with a real consumer); #7c sanitation ✅ (v0.338.0, waste→compost→fertilizer→Fertilize crops; **all 5 listed survival needs now live**); remaining #7 = register the heavier sim systems (atmosphere/hydrology/ecology/disasters) **when they gain real consumers** (the weather precedent, not cosmetic un-deferral); (8) progression layer [✅ DONE], #8a skills + XP foundation ✅ (v0.340.0, the built-but-unwired `skills/` scaffold now wired end-to-end: SkillRegistry loads skills.csv, PlayerSkills on the player, SkillSystem registered LAST drains a shared `xp_grants` channel the action systems push to; XP from **craft→recipe skill / harvest→farming / mine→mining**; live levels+XP in the profile Skills panel; **data-integrity fix**, recipes.csv `skill_required` reconciled from a non-canonical vocabulary to real skill ids [235 rows, category-aware] so XP can't silently no-op, locked by a new drift lint); **#8b tech-unlock** ✅ (v0.341.0, skills GATE crafting: CraftingSystem authoritatively rejects under-level crafts, the crafting UI shows "Requires {skill} Lv N (you: Lv M)" + locks the button, a **Dev: max skills** button preserves the 100%-unlocked testing posture) + **#8c quests** ✅ (v0.342.0, `quests/` scaffold wired: `QuestRegistry::from_ron_dir`, QuestSystem registered, player auto-accepts the **Getting Started** chain, Craft/Harvest objectives advance via a `quest_events` channel + Gather via live inventory, prerequisite chaining, a profile **Quests** panel; also fixed a #8b fresh-player **deadlock**, level-1 recipes are the free starter tier, gating begins at level 2). **★ GAMEPLAY-LOOP ARC COMPLETE, build order #1–#8 all shipped (v0.329→v0.342, 21 commits): the full production + survival + progression sandbox is delivered.** Next arc is the operator's call. **TEST & HARDEN underway (operator's first live play):** v0.343.0 fixed round-1 findings, the linchpin was a stale `target/release/data` shadowing live repo data (`find_data_dir` now prefers the repo's own `data/`), which had made dev-stock no-op + the quest show its raw id + skills stay empty when running the build exe directly; plus a 3-stage drone visual (+ progress bar), clearer mining labels, and a real skill sheet replacing placeholders. Known gap (future arc): the 3D player home is empty, loops are menu-driven, no walk-up crafting stations yet. #3b: SPEED modifiers ✅ (v0.334.0), Drink action ✅ (v0.339.0, hydration symmetric with Eat); still tracked: stamina/vision modifiers, spoilage→nutrition.** #1+#2 shipped the reusable GUI→ECS command bridge (GuiState flag → main-loop writes a DataStore Mutex channel → the owning System drains + acts in its tick) + a "Dev: stock all materials" button (one stack of every recipe input, inventory auto-grown via `Inventory::ensure_slots`) + a real Craft button doing consume/produce on the player's ECS inventory (proven by `crafting_bridge_tests`). Proposed concise complete-nutrition plant set: potato/soybean/leafy-green/tomato/sunflower/carrot, **operator to finalize**.

**FOUNDATION COMPLETE, engine-wiring arc (2026-05-29):** native P2P NAT-traversal (TURN, v0.320.0); typed containers + content-class compatibility (v0.321.0); server game-world + player-progress persistence (v0.322.0); ENGINE WIRING, item/recipe/plant registries load into the runtime DataStore so crafting crafts + inventory resolves real names (v0.323.0); game_time export + the engine-wiring ENFORCEMENT LINT (v0.324.0, the `theme_token_lint` analog: every `impl System` must be registered OR deferred-with-reason, 7 registered, the deferred list shrinks as the gameplay-loop arc wires systems). This is the platform the gameplay-loop arc builds on. **DATA EXPANSION is now unblocked** (crafting/inventory exercise the item/recipe schema with working code) and proceeds organically as each loop lands, refining chains, components, tools, plants, recipe byproducts, chemistry→crafting links.

**PAUSED -- superseded by the game-dev arc (resume after, or per operator):**
**ACTIVE: Clean web chat VIEW rebuild (Track W, pivoted 2026-05-26).** Operator's call: stop incrementally patching the tangled web view, rebuild it from scratch to mirror native 1:1, **keep the proven JS engine** (WS/crypto/WebRTC), and make sync *mechanical*. Live chat is non-precious (no users) so we rebuild in place. **Spec + sync backbone: `docs/design/chat-layout.md`**, one web `view/*` module per native `draw_*` (same names), engine↔view boundary via the `hos` event bus, DOM stays (accessibility improves, never canvas; WASM considered + rejected for canvas/a11y reasons). Build order: scaffold + constants + event-bus boundary → engine extraction from app.js → centerPanel/messageRow/timestampPill → leftRail → rightRail → composer/header → modals → sweep old view files + dead CSS. Incremental-patch history (now superseded): left+right rails done (v0.287.4-.9), nav labels (v0.287.7), message-row flatten + grouped pill rows (v0.287.10/.11), these stay live as the clean rebuild replaces them section by section. Native WebRTC transport remains the committed parallel next-major-effort (unblocks native voice + streaming).

## TIER 0: pre-public launch blockers
Items here are mandatory before inviting public users. Operator-attended where noted. **Order matters within the tier.**

0. **★ SECURITY AUDIT 2026-06-12, the CRITICAL is a launch blocker.** A 10-dimension multi-agent audit (51 agents, every finding adversarially verified; Fable 5 re-verified the top items by hand) produced 30 verified findings. The **no-fork quick-wins shipped v0.417.0** (stored-XSS via uploaded SVG → blocked + `Content-Disposition: attachment` on `/uploads`; missing security headers on the live `/` and `/chat` entry pages → re-listed + X-Frame-Options added, applied live; twemoji un-SRI'd CDN script → vendored same-origin; profile-gossip future-timestamp lockout → 24h bound). **STILL OPEN, ranked:**
   - **(a) CRITICAL, auto-update RCE: ACTIVATED + crypto-proven 2026-06-12 (v0.421.0).** The operator generated the hybrid keypair (`just gen-release-key`), the PUBLIC keys are committed in `data/release/signing_pubkeys.json` + compiled into v0.421.0, the private `release-signing-key.enc` is gitignored + externally backed up (passphrase recorded non-digitally), and a local self-verify test confirmed the private key ↔ embedded public keys are a matching pair ("Signed + self-verified OK"). So v0.421.0+ builds ENFORCE signatures end-to-end. **Remaining: sign each published release** (`just sign-release vX` after CI uploads binaries), v0.421.0 is the first to sign once its Build-Desktop run finishes; from here on an unsigned release is invisible to auto-update for v0.421+ users (CLAUDE.md SOP step 5 + docs/admin/release-signing.md). NOTE: legacy (v0.420-and-earlier, empty embedded keys) users still auto-update normally to v0.421.0, then enforce. [original code-shipped detail below]
   - **(a-orig) CODE SHIPPED v0.418.0 (the updater half) + v0.419.0 (find_newer_exe).** The updater now verifies a **hybrid Ed25519 + Dilithium3 signed manifest** (both must verify) + the artifact SHA-256 before installing, and only OFFERS releases that carry a signed manifest, so a GitHub/release compromise or a stray/malicious tag can no longer push code. New `src/release_update.rs` (sign/verify/keygen + 8 tests, smoke-tested incl. wrong-passphrase reject), CLI `--gen-release-key`/`--sign-release`, `just gen-release-key`/`just sign-release`, `docs/admin/release-signing.md`. Operator decided: hybrid scheme, dedicated key, passphrase-encrypted file (Argon2id+AES-GCM), signed LOCALLY (never CI). **TO ACTIVATE (operator, one-time): `export HUMANITY_SIGNING_PASSPHRASE=... && just gen-release-key`, commit `data/release/signing_pubkeys.json`, ship a release, then `just sign-release vX` per release.** Until then the embedded pubkeys are empty → updater warns + legacy behaviour (so nothing breaks pre-activation). **find_newer_exe DONE v0.419.0:** `src/main.rs::find_newer_exe` now verifies each candidate `vX_HumanityOS.exe` against the embedded keys (detached `.sig.json` sidecar, hybrid sig over the file's SHA-256) before launching, an unsigned/tampered local build is skipped, not exec'd. New `--sign-file` CLI + `verify_file_against_sidecar`; `scripts/archive-build.js` opt-in-signs each dev build when the key+passphrase are present (unprovisioned → legacy, so the dev flow isn't blocked). 10 release_update tests. **So the only thing left for this CRITICAL item is the operator ACTIVATION** (`just gen-release-key` → commit pubkeys → ship → `just sign-release`), after which the whole updater + launcher chain is fail-closed.
   - **(b) DONE v0.422.0, vault-sync replay.** Added a relay-side anti-replay cache: after a signed request verifies, the relay records the (key, purpose, timestamp) tuple and rejects a second sighting within the window (409). No client or protocol change. `RelayState.auth_nonce_fresh` + `seen_auth_nonces`; applied to vault_sync PUT/GET/DELETE (the dangerous DELETE/PUT replays + the GET). Reusable for the other signed endpoints if needed.
   - **(c) DONE v0.420.0, `POST /api/v2/objects` per-author quota.** Added a sliding-window per-author submission cap (30/60s, keyed by a short hash of the author key, map-size-bounded) → 429 over the cap. The signature already authenticates the author; the quota closes the storage-exhaustion vector (flooding distinct objects under one valid key). `api_v2_objects.rs::post_object` + `RelayState.object_submit_rate`. (A sustained-flood daily quota is a possible future tightening; the per-minute cap kills the 1000s/sec exploit by 4 orders of magnitude.)
   - **(d) LARGELY MITIGATED by the active release signing; residual documented.** A compromised CI can no longer push a malicious DESKTOP release that users will install, because CI cannot sign (the key is operator-local, never in CI) and v0.421+ updaters reject unsigned releases. The residual is the VPS RELAY deploy (a compromised CI could deploy a malicious relay binary via deploy.yml) + GitHub branch/tag protection (operator GitHub-settings, not code). Recommended (operator): enable branch protection + required signed commits on `main` in GitHub settings; the relay-binary-verification on the VPS is a future hardening. Lower priority now that the desktop-RCE path is closed.
   - **(e) DEFERRED to the UI/onboarding work, public `/api/members` directory** exposes name/key/last_seen with no opt-out. Design settled: reuse the existing `profiles.privacy` JSON with a `directory: "unlisted"` key (no schema change), honored in `get_members` (+ `get_member_count` for pagination) and `get_member_by_key` (404 for unlisted). Deferred because (1) a backend opt-out flag is useless without a user-facing toggle to set it, which belongs in the privacy/settings UI the operator is restructuring, and (2) the `get_members` filter wants a `json_extract` subquery on the hot member-list query and json1 isn't currently used in storage (verify it's compiled in first). Build the toggle + filter together in the UI increment. It is a documented design choice (you appear in a server's directory when you JOIN it), MEDIUM, not a launch blocker.
   - **(f) PARTLY DONE v0.420.0.** Federation gossip amplification (HIGH) FIXED: a per-SOURCE inbound rate limit (50/s, reusing `federation_rate` with an `:inbound` key) drops + doesn't re-emit when a source floods, so 1×rate inbound can no longer become N×rate outbound. Announce-flood FIXED: a global cap on `POST /api/v2/announce` (20/60s) bounds the blast radius if `API_SECRET` leaks. STILL OPEN: `/api/send` per-IP rate limit (needs X-Real-IP plumbing; the bot path is the trusted API_SECRET path, low value), and message hard-delete leaves WAL/backup remnants (no retention/secure-erase policy, needs a retention design).
   - Full detail + the correctly-REFUTED findings (home coords = demo data, avatar/banner/link `data:` XSS = server-blocked, `unsafe-inline` CSP = `esc()`-neutralized, etc.) are in the session transcript + the operator's private memory note `security_audit_2026_06_12.md`. **Deliberately NOT committed to the public repo** (disclosure hygiene). The operator chose "quick-wins now, sprint later", items (a)-(f) are the candidate security sprint.

1. **DONE: nginx `/health` routing.** Verified live 2026-06-11: `https://united-humanity.us/health` returns 200 JSON. The fix has been in `scripts/nginx/humanity.conf` (`location = /health { proxy_pass ... }`, v0.285.x), this entry was doc lag. Off-site monitoring can use the public endpoint.

2. **DONE: GitHub webhook deleted + endpoint fail-closed (v0.285.0).** The stale webhook (pointed at a dead ngrok URL, 404 for months) was deleted from the GitHub repo. The relay's `/api/github-webhook` endpoint now FAILS CLOSED, rejects when `WEBHOOK_SECRET` is unset (was fail-open, a forged-announcement spoof vector). Note: this webhook was NEVER the update-autoposter, that's the CI Deploy Bot via `/api/send` + `API_SECRET`, a separate path that's unaffected and healthy.

3. **DONE: off-site backup (stopgap).** 2026-05-20: `scripts/backup-relay-from-vps.ps1` + a Windows scheduled task ("HumanityOS Relay Backup Pull", every 6h) now pull the live relay DB from the VPS to the operator's PC, genuine 3-2-1 backup (live DB / VPS-local 30-min snapshots / off-site PC). This is the "immediate" half of the device-mesh vision (`docs/design/device-mesh.md`); the full in-app version is TIER 2. NOTE: the PC backup is off-site but a SINGLE off-site copy. A second target (phone, NAS, or a cheap second VPS) would make it 3-2-1-with-redundancy. Phase B of the device mesh generalizes this.

4. **DONE: 2026-05-21 release-mirror cleanup + retention automation.** Cleaned 277 old release dirs from `/var/www/humanity/releases/` (freed 91 GB; 91% → 13%). v0.283.4 extends `scripts/humanity-disk-guard.sh` to enforce 10-version retention automatically on every 20-min cycle + regenerate the manifest. Cascade is structurally prevented from recurring.

5. **DONE: backup script repaired + in-repo.** The pre-v0.90.0 path bug was silently backing up an empty fossil DB for over a month. v0.283.4 ships `scripts/humanity-backup-db.sh` as the source of truth, the `deploy.yml` workflow now copies it to `/usr/local/bin/humanity-backup-db` on every deploy. Fossil backups moved to `backups/fossil-pre-v0.90/` for historical interest only.

6. **DONE: Orphan Ed25519 admin rows cleanup.** 2026-05-21: ADMIN_KEYS env updated to Shaostoul's Dilithium hex (3904 chars), 4 orphan rows DELETEd, relay restarted, verified `user_roles` is Dilithium-only.

7. **DONE: Inc6 attended wipe.** Verified 2026-05-20 by direct SQL.

8. **DONE: TLS auto-renew sanity check.** certbot.timer runs on a 12h cycle; last run 2026-05-20 16:42, next 2026-05-21 06:15. All 3 certs valid 50-68 days out. No action needed.

9. **DONE: API_SECRET length audit.** 64 chars (above 32-char threshold). No action needed.

## TIER 1: hardening before invites scale beyond known group
Items here protect against the realistic adversary (script kiddie, opportunistic abuser, eager fan with sticky fingers). Order within tier is flexible; pick what's cheapest first.

**TIER 1 is effectively closed.** All code-actionable items shipped; the two decision-gated items were decided by the operator 2026-05-20 (fail2ban over Cloudflare; skip off-box monitor; plan federation). Remaining federation *implementation* is tracked in TIER 2.

1. **DONE: DDoS protection, fail2ban (v0.286.x).** Operator chose self-hosted fail2ban over Cloudflare. nginx jails added (`scripts/fail2ban/nginx.local`): `nginx-limit-req` (bans IPs repeatedly tripping nginx rate limits) + `nginx-botsearch` (bans exploit-path scanners), conservative thresholds + `ignoreip` for loopback/private. sshd jail was already active. Installed live + version-controlled (deploy.yml installs + reloads). Composes with the in-app gates (v0.279/v0.280).

2. **DONE (VPS-side): Monitoring + alerting (v0.286.2).** Watchdog (2-min liveness + self-heal) + `scripts/humanity-alert.js` configurable multi-channel external alerting (ntfy/Discord/Telegram/webhook), wired into watchdog + disk-guard. Admin opt-in via `data/alert-channels.secrets.json`. **Off-box monitor (whole-VPS-down) explicitly SKIPPED per operator 2026-05-20** ("not too concerned"). If revisited: a free uptime service or PC scheduled task can reuse the same alert channels.

3. **DONE: SQLite corruption recovery (v0.286.0).** `Storage::open_resilient`, boot integrity check + restore-newest-healthy-backup or refuse-to-start. 4 tests.

4. **Federation: design DONE, implementation in TIER 2.** Operator chose "plan activation." Design + vetting + abuse model + 4-phase plan in `docs/design/federation-activation.md`. Key finding: federation is already fail-closed (trust_tier 0 default; unknown peers can't connect), so dormant = safe; the implementation phases (admin UI, profile-gossip rate limit, second-VPS end-to-end test, then third-party peers) are the work. Moved to TIER 2 #1.

5. **DONE (via watchdog, v0.285.2): crash-loop detection.** Watchdog self-heals + alerts (chose this over systemd StartLimit, which would give up + leave the relay dead, bad for unattended).

## TIER 2: big-feature gaps
Items here are real features the system promises but doesn't deliver on every platform. Weeks of work each.

> **Cross-cutting mandate (CLAUDE.md non-negotiable rule, 2026-05-20): GUI-first configurability.** Every ops/config capability must be reachable in-app, not CLI-only. The recent TIER 0/1 ops work (alerts, backups, fail2ban, watchdog, secrets) is all CLI/SSH today, that's tracked debt. See `docs/design/in-app-ops.md` for the audit + the north-star admin action registry (GUI renders it AND an AI can enumerate it) + the build order. NEW features with an ops dimension build their in-app control in the same increment.

1. **Web-mirrors-native parity (Track W, ACTIVE).** Full divergence map + migration order in `docs/design/web-native-parity.md`. Native chat is the parent; web is the old UI being rebuilt to mirror it, incrementally (web stays usable throughout; theme tokens already shared). Migration order: (1) left-rail tabs→stacked-collapsible-sections ✅ + 1b studio→right ✅ + 1c scratchpad top-row ✅ + 1d identity→account-menu ✅, (2) right-rail Friends/Members ✅, (3) message rows + timestamp pill + inline reactions **[NEXT]**, (4) header + composer, (5) top-nav alignment (labels ✅ v0.287.7; native tiering pending), (6) spacing sweep + dead-CSS removal (`style.css`, `chat-voice.js` are dead). Each step = its own increment + version bump.

2. **Studio + streaming (Track S, phased, dependency-ordered).** Full vision in `docs/design/studio-streaming.md`. Right-rail studio widget (top, for streamers) + full Studio modal + docked inverted chat + per-friend viewer widgets + multi-stream viewer modal + **persistent stream across all pages** + **privacy guard** (auto-hide on sensitive pages/buttons). KEY CONSTRAINT STALE-CORRECTED 2026-07-25: native capture/encode/stream SHIPPED v0.853-0.854 (src/renderer/stream_capture.rs, src/net/live.rs, web/pages/watch.html). So build the widget on web first (functional), mirror to native once native transport exists. Order: S0 persistent session (gate for "always stream" + viewers) → S1 web studio widget+modal → S2 viewer widgets+modal → S3 privacy guard (can land early, independent) → S4 native mirror. Native transport = the same weeks-long WebRTC lift as native voice (#4). **v0.350.0: a native Studio quick-access section now sits at the top of the chat right rail (above Friends), mirroring the website's studio-widget placement, a Go Live/End Stream toggle + Open Studio launcher. This preserves Studio access through the Real/Play nav consolidation (the top-nav Studio button is being folded away); it is ACCESS only, the transport-bearing native mirror is still S4, gated on native WebRTC.**

3. **In-app ops console (phased, pays down the CLI debt).** Per `docs/design/in-app-ops.md`. Slice 1 (System/Health dashboard) SHIPPED v0.287.0 (web) + **native parity SHIPPED v0.720.0** (Server Settings → admin tab → "System health": status / deployed build / uptime / messages / peers from the connected server's public /health + /api/stats, worker-thread fetch + Refresh). Remaining: (2) Alert-channels editor (first write panel), (3) Backups panel, (4) Federation panel (= #5 Phase 1), (5) fail2ban/relay-control/secrets (need a sudo-gated relay→system bridge), (6) factor out the action registry + AI-facing list/run endpoints + a coverage test.

4. **Native voice.** STALE-CORRECTED 2026-07-25 (archived-tasks audit): native voice SHIPPED in the v0.485-0.495 str0m arc (STATUS.md:543; str0m in src/net/, chat.rs). Remaining tail only: per-peer volume/mute/squelch UI, web transmit-mode UI, a two-str0m CI harness, graceful relay restart (docs/history/2026-06-21.md Still TODO).

5. **Federation activation (phased).** Design done, `docs/design/federation-activation.md`. Phase 1: Server Settings → Federation admin UI (list/add/trust/defederate peers + per-channel federation toggle), native + web. **NATIVE Phase 1 UI SHIPPED v0.722.0** (Server Settings → admin → Federation: list from GET /api/federation/servers, Add server, per-row trust-tier dropdown + confirmed Remove, Connect-all; per-channel federation toggle already existed in the Channels grid). Remaining Phase 1: web mirror. Phase 2: per-peer profile-gossip rate limit. Phase 3: second operator-controlled relay, federate the two, verify end-to-end, esp. whether moderation propagates to federated content (load-bearing test). Phase 4: open to vetted third-party peers. Fail-closed default = safe to build incrementally.

6. **Native streaming viewer.** Subsumed into Track S (S4 native mirror).

7. **Native trade UI completion.** Trade page exists in `src/gui/pages/`. Trade events (`trade_response`, `trade_confirm`, etc.) aren't dispatched. Either wire them up or remove the page until ready.

4. **Litestream / continuous backup.** Beyond the nightly rsync floor in TIER 0, set up real continuous replication. SQLite WAL → S3-compatible blob storage. RPO ~1 minute, RTO ~10 minutes from cold.

5. **Mobile clients.** Android (JNI bridge for keyring + AndroidKeyStore; new keychain backend), iOS (Keychain Services already works via `keyring` crate, needs only an iOS build target). Big effort either way.

6. **Device mesh** (design doc: `docs/design/device-mesh.md`). The operator's vision: your devices back up each other + the relay; review all devices' system-info (hardware, storage, health) from any one device; device roles (battle-station / accessory / relay / archive). Phased: A) system-info reporting + "My Devices" dashboard, B) backup designation + pull + encryption-at-rest (subsumes the shipped PowerShell stopgap), C) restore flow, D) LAN direct-sync + mobile mesh members + remote wipe. The VPS-as-rendezvous architecture (devices report up, read all-devices down) fits the existing federation model. On-mission sovereignty tooling, give it to every user, not just the operator.

7. **Library, federated file/media catalog (NEW, designed 2026-05-26).** Full design in `docs/design/library.md`. One "free public access" page, tabbed by consume-mode: **Files** (federation-hosted media/art/3D models, download in, upload, pin) + **Software** (folds in the Tools page) + **Web** (folds in Browser + Resources). Files engine = trust-tiered LRU cache (unverified shared pool + per-user sub-cap; verified+ per-user quota → **bounded disk by construction**) + curated permanent tier (roled pin → permanent + quota refund → routed to the existing torrent seeder + Internet Archive). Identity by **content hash (SHA-256)**: exact dupes auto-link; near-dupes (image perceptual hash) trigger a side-by-side **preview-confirmation dialog** (3D/binaries: exact-hash only). Rule: **ephemeral = server-local; pinned = federated**; catalog aggregates lightweight metadata across `/api/federation/servers`, grouped by source server. Extends `assets.rs`/`uploads.rs`/`roles.rs`/`pins.rs`/`server_settings.rs` + `docs/admin/torrent-infrastructure.md`. Phased: Files engine → Library/Files UI (web→native) → pin/torrent → perceptual dedup → federation aggregation → fold Tools/Browser/Resources in. Seed content: the 187 archived Project Universe media files. GUI-first quota/cap config per server admin.

8. **P2P Groups, relay-independent groups (NEW, designed 2026-05-27; operator chose "true P2P" over relay-mediated/federated-fallback).** Full design + phased plan in `docs/design/p2p-groups.md`. Today groups are 100% relay-mediated (`handle_group_create/join/msg` → relay SQLite), so a relay outage breaks create/join/messaging and the invite URL 404s, contradicts "no single point of failure." Target: a group is a **signed object + append-only signed membership/message logs** replicated peer-to-peer over the existing WebRTC DataChannels (`web/chat/chat-p2p.js`); relays are **optional accelerators** only. Invite = **signed connection ticket** (not a URL). E2EE via a per-epoch group key (generalize the Kyber768 dual-seal in `src/net/dm_pq.rs`), re-keyed on membership change. **Core gap = relay-independent signaling** (today `webrtc_signal` rides one relay) → solved by multi-relay failover + peer-assisted signaling (+ TURN/peer-relay for NAT). Phased: **P1** sovereign data + working signed-ticket invite (fixes the 404; relay still signals) → **P2** signed + E2EE messages → **P3** P2P transport (relay = signaling-only) → **P4** relay-independence (the payoff: kill the home relay, a group with ≥1 reachable peer still works) → **P5** serverless discovery (mDNS/DHT). **Decisions settled 2026-05-27** (operator): TURN = operator-run **+** peer-as-TURN; encryption = per-epoch group key; relay = optional accelerator (see doc). Builds on the signed-object/gossip model (`storage-architecture.md`) + signed-log governance (`signed_moderation_logs.md`).
   - **P1 sub-steps:** (a) **DONE v0.292.0**, relay sovereign data model: `group_v1`/`group_member_v1` signed-object types projected into `p2p_groups` + `p2p_group_roster` (the membership-log fold) via `src/relay/storage/groups_p2p.rs`, wired into `put_signed_object`, additive (old relay-mediated path untouched), 3 tests incl. "unauthorized admit rejected". Object-format spec captured as the module doc-comment. (b) **DONE v0.292.1**, cross-language signed-object construction. Built the missing web primitive: `web/shared/canonical-cbor.js` (byte-exact canonical CBOR matching `src/relay/core/encoding.rs`, length-first key sort, shortest-int, definite-length) + `web/shared/pq-object.js` (`buildSignedObject`/`buildGroupV1`/`buildGroupMemberV1` → the `POST /api/v2/objects` submission). **KAT-locked**: `scripts/group-object-kat.mjs` (`just group-kat`) ↔ `groups_p2p.rs::group_v1_canonical_kat` assert identical payload hex + object_id, web builds objects byte-identical to what the relay verifies. Native already builds objects via `ObjectBuilder`. This unblocks ALL web signed objects (votes/vouches/recovery), not just groups. (c) **DONE v0.293.0**, invite + admission model. Capability design (offline-joinable): creator posts a `group_invite_v1` committing to `BLAKE3(secret)` + expiry; the ticket carries the secret out-of-band; a joiner posts `group_join_v1` revealing it; the roster fold admits the join author iff the secret matches + not expired, **no creator online needed**, randos rejected. Relay: `index_group_invite`/`index_group_join` + `p2p_group_invites` table (`groups_p2p.rs`), 7 tests (incl. wrong-secret/expired/non-creator-invite rejection). Web: `buildGroupInviteV1`/`buildGroupJoinV1` + `encodeInviteTicket`/`decodeInviteTicket` + `randomInviteSecret` (`pq-object.js`). (d) **DONE v0.294.0**, chat UI create+join flow (**the 404 is fixed**). New relay read endpoint `GET /api/v2/groups?pubkey=<hex>` (`api_v2_objects::my_p2p_groups` off the projection). New `web/chat/chat-groups-p2p.js` (lazy-imports the ESM object layer + vendored blake3; uses the chat's own Dilithium signer): create group → `buildGroupV1` → POST /api/v2/objects; per-group "create invite" → copyable ticket (`buildGroupInviteV1` + `encodeInviteTicket`, 7-day); join → paste ticket → `buildGroupJoinV1`. `chat-social.js` `renderGroupList` now renders P2P groups (click → roster + invite dialog) above legacy ones; `promptCreateGroup`/`promptJoinGroup` repointed to the P2P flow. **No browser e2e run yet** (preview unavailable), relay compiles, all KATs/tests pass, submission fields + signable bytes match Rust by construction; operator to visually verify. (e) legacy-path retirement, operator **deleted** the live test group (no migration needed); retire the old relay-mediated group code AFTER the operator browser-verifies the (d) flow (kept as fallback until then).
   - **Phase 2 (E2EE group messages), COMPLETE both clients + the conversation is a CHANNEL, not a modal (v0.295.0 / v0.297.1 / v0.299.0 / v0.300.0):**
   - **v0.301.0, delete groups (Leave + Disband).** Operator: the group list only grew, no way to remove. Built sovereign-signed-object style (matches join/invite): **Leave** (anyone) relaxes `index_group_member` to authorize self-removal (`group_member_v1 {action:"remove", subject:self}`), reuses the existing object type. **Disband** (creator only) is a new `group_disband_v1` that sets a `p2p_groups.disbanded` flag (guarded idempotent ALTER for the live DB); `p2p_groups_for_member` filters disbanded, so it drops off everyone's list (the signed object is the durable tombstone; `index_group` INSERT OR IGNORE never un-disbands). `GET /api/v2/groups` now returns `is_creator` per group (server-computed) so the UI shows **Disband** only to the creator. Web: right-click → Leave / Disband (confirm). Native: header **Leave** + **Disband** buttons. 4 new relay tests (13/13 groups_p2p). Both clients return to #general + refresh on action. **Epoch-key bootstrap note** (operator hit "No epoch key yet" sending into a freshly-joined group): rekey-on-join is creator-driven, only the creator's client seals an epoch to a new joiner, on next open; it self-heals. Future polish: rekey from the group LIST, not just on open.
   - **v0.300.0, group = channel, no modal (operator UX fixes).** Operator tested v0.299.x and required: the group conversation must be the SAME interface as server channels (switching to a group should feel like #general → #announcements), not a bolt-on modal, and the modal kept landing behind its own darkening backdrop. WEB (v0.299.2): killed the dialog; `openP2pGroup` switches the main chat center panel like `switchChannel` (`window.activeP2pGroup`, standard `addChatMessage` renderer, composer monkey-patch, 4s poll); invite via header link / right-click popover (no full-bleed backdrop). WEB first-load bug (groups only appeared after interacting) fixed: `loadP2pGroups` owns the fetched flag, `connect()` proactively loads after identity. NATIVE (v0.300.0): removed `draw_p2p_group_modal`; a P2P group is a new `chat_active_channel` prefix `p2pgroup:<id>` joining the `dm:/group:/scratchpad` dispatch; `enter_p2p_group`/`poll_p2p_group` project decrypted `GroupMessage`s into `chat_messages` so the standard renderer handles them; header has Back + name + "Copy invite"; composer routes `group_msg_v1` over HTTP. Removing every full-bleed overlay eliminates the modal-behind-backdrop bug CLASS. KNOWN LIMITATION: native `enter_p2p_group` does ~4-6 blocking ureq calls on the click frame → brief hitch on a high-latency relay; thread it / dedupe redundant epoch+members fetches if noticeable. P2-relay shipped v0.295.0 (`group_epoch_key_v1` + `group_msg_v1` projections; relay stores/serves opaque ciphertext only; read endpoints `/groups/{id}/{members,messages,epoch}`; 9 tests). P2-web shipped v0.297.1 (web crypto helpers in `pq-object.js`: `buildGroupEpochKeyV1` / `buildGroupMsgV1` / open helpers + `decodeCanonicalCbor`; chat UI rewritten as a real E2EE chat view with 4s polling; initial epoch key issued on group create). **P2-cross-identity + native shipped v0.299.0:** (a) **Web rekey-on-join** (`chat-groups-p2p.js` `rekeyIfCreatorNeeds`), when the creator opens a group dialog and the roster has new members not covered by the current `epoch_key`, auto-mint a fresh epoch sealed to the full roster (forward secrecy on rotation). (b) **Native chat-in-groups**, new `src/net/group_e2ee.rs` (epoch-key sealing + AES-GCM under it, byte-identical to web; 2 tests pass); `src/net/api_v2.rs` adds submit/fetch helpers + `rekey_if_creator_needs` + initial epoch on create; `draw_p2p_group_modal` gains a full chat view (message list, compose + Enter-to-send, refresh button). The two clients now interop end-to-end on the same group object. **Phases 3-5 remain:** P3 P2P transport (relay = signaling-only) → P4 relay-independence (multi-relay signaling + peer-assisted + TURN; the actual payoff, the group survives a dead home relay) → P5 serverless discovery (mDNS/DHT). **Polish queued for after operator browser-verifies cross-identity chat:** right-click context menu on P2P groups; signed `group_leave_v1` + `group_disband_v1` objects.

9d. **DONE (2026-08-24): encrypted DM attachments (the last genuine hole).** A file/photo shared in a DM used to sit as readable bytes at a public /uploads URL; only the LINK was hidden in the E2EE message. So a "private" DM photo was exposed to the operator, to anyone who got the URL, and to metadata leakage. Fixed to match the message promise: the file is encrypted client-side with a fresh AES-256-GCM key BEFORE upload; the relay stores only opaque ciphertext via a new ?encrypted=1 mode (inert .enc blob, skips format/EXIF/magic validation on ciphertext); the key+nonce+metadata ride inside the sealed envelope as a [[hum:file:v1]] marker (native net::dm_pq + web crypto.js, shared format). Recipient: decrypt envelope -> fetch ciphertext -> decrypt locally. Web renders encrypted images inline + a decrypt-and-download for other files; native encrypts on send (both the file picker AND clipboard-paste-into-DM paths) and shows a labeled card on receive (full native inline decrypt is a tracked follow-up). Public-channel uploads unchanged (public is public). Sidebar/notify previews collapse the marker to "Photo"/filename, never raw base64. Tests: attachment_encrypt_decrypt_and_marker (roundtrip, wrong-key-fails, no-plaintext-in-ciphertext, marker parse). Full suite 1607/0. FOLLOW-UPS: full native inline image decrypt+render; group-chat attachments (same pattern, not yet done); cert expiry/revocation; native relay-calls TURN toggle. These plus mixnet/traffic-analysis are the honest remaining set; everything else is fundamental limits (device compromise, public-is-public, replication-no-global-undo, transport IP which the Tor onion service opts around).

9c. **DONE (2026-08-24): privacy MAXIMIZATION arc, the follows graph removed + the last leaks closed.** Operator: keep going to full privacy maximization, then one cohesive devlog post. Shipped: (1) FOLLOWS GRAPH GONE, the last server-side social graph. The follows table is dropped by migration. Following is now sealed client-to-client control messages ([[hum:follow]]/[[hum:unfollow]] over the DM mailbox); each client keeps its own following/followers in its local encrypted store; multi-device sync rides the self-copies. Friendship is a client-held Dilithium CERTIFICATE (recipient authorizes sender, hum/friend/v1 preimage) verified STATELESSLY at dm_put, so no friends table exists. Certless DMs to strangers are knocks (sealed, delivered, capped 20/sender/day, never per-pair so no graph reforms). Friend codes now hand the redeemer the owner_key and the clients complete the friendship; profile friends-visibility unlocks by presenting the owner cert. All three surfaces done (relay verify + knock budget, native engine/dm.rs social layer + dm_store sets, web chat-social.js social layer + chat-dm-store.js sets). (2) DM SIZE PADDING: sealed plaintext padded to buckets [256,1024,4096,16384] so ciphertext length stops leaking message length; native+web bucket-matched, test proves equal ciphertext for short vs long. (3) MESSAGE RETENTION: message_retention_days server setting (default 0=forever) auto-expires public messages past the window on the maintenance sweep, pins always kept; native Server Settings row. (4) FEDERATION GOSSIP respects unlisted: Private/Balanced users no longer replicate their profile across federated servers (gossip + signed_profile cache gated on privacy.directory; going unlisted retracts the local copy via new delete_signed_profile). (5) TOR ONION SERVICE: opt-in operator infra (scripts/tor-onion-setup.sh + docs/admin/tor-onion-service.md) so users reach the relay without revealing their IP, the application-layer answer to the wiretap-class exposure. Tests: friend-cert roundtrip + PINNED preimage (web must match), knock-budget + forged-cert + valid-cert-bypasses-budget, padding equal-length, message retention window+pins. Full lib suite 1606/0; all 4 gui lints green. Deferred honestly: cert expiry/revocation (v1 certs are permanent, unfriend is client-side), native relay-calls toggle (str0m TURN still None), mixnet against clearnet traffic analysis (the onion service sidesteps it for opt-in users). Cohesive devlog/RSS post covering the whole v0.1197-v0.120x privacy arc is the closing step.

9b. **DONE (2026-08-23, same session): the privacy-hardening sweep + privacy tiers.** Follow-through on the sealed-sender cutover, closing every stored-data class the audit found, plus the operator-directed onboarding privacy chooser. Shipped: (1) EXIF/XMP/IPTC stripping on every image upload, lossless, before bytes touch disk (relay/core/strip_metadata.rs; decode-after-strip integrity tests) - a phone photo no longer publishes GPS coordinates. (2) Marketplace buyer-seller threads DELETED as a data class (listing_messages stored plaintext AND broadcast every message to all clients); Contact = a sealed-sender DM with the seller, table dropped by migration. (3) Legacy relay groups fully retired (relay protocol + tables dropped + native/web UI cleaned); groups are exclusively the E2EE P2P signed-object system. (4) Backups encrypted at rest: 6h in-process snapshots AES-256-GCM (.db.enc), VPS 30-min script openssl (.db.aes), key at data/backup.key OUTSIDE the backups dir, created at boot; crash recovery decrypts transparently and now scans BOTH backup dirs (the in-process dir was never consulted before - its relay_ underscore names failed the old filter); scripts/decrypt-backup.sh for manual restores; sealed-backup recovery test. (5) Presence privacy, server-enforced: privacy_update hides online status, join/leave, typing, and last_seen is NEVER WRITTEN while hidden (and scrubbed on enable); new members join hidden (fail-private); roster keeps hidden members listed-but-masked so DM keys distribute. The old Settings "Show Online Status" toggle - which was persisted and read by NOTHING - is now the real switch. (6) PRIVACY TIERS (operator direction): data-driven presets in data/gui/privacy_tiers.json - Private (maximum privacy, THE DEFAULT), Balanced, Open, Spotlight (maximum publicity for streamers by explicit choice); first-connect chooser modal in BOTH clients (native global modal + web overlay), Settings > Privacy section with per-switch overrides, choice persisted and re-asserted per server. (7) Account sovereignty: self-service account_export (everything the server stores, as JSON download/file) + account_delete (typed-name confirm; erases messages, uploads+files, profile, follows, mailbox, vault, listings, membership; admins must hand off first); export-then-erase-leaves-no-trace test. (8) Web key-protection nudge: after the tier chooser, an unwrapped seed walks straight into the existing passphrase-wrap flow. (9) Relay-my-calls toggle (web): iceTransportPolicy relay, fail closed, persisted - callers cannot learn your IP. (10) VPS nginx log retention cut 14 days -> 2 and accumulated IP history purged (fail2ban keeps the live log it needs). Follow-ups deliberately NOT done: native relay-calls toggle (str0m TURN client is still None - wire when P3 TURN lands); follows-graph minimization (mutual-friendship certificates so the server stores no follow edges - REAL design work, the last server-side social graph); DND windows stay server-side (they gate server push at night - moving them client-side breaks quiet hours; documented, accepted); mixnet/padding against live traffic analysis (long horizon).

9a. **DONE (2026-08-23, same day it was logged): DM metadata minimization, sealed sender + expiring mailbox.** The relay no longer stores WHO a DM is from, ever: the `direct_messages` graph table (from_key/from_name/to_key/timestamp in the clear) was replaced by `dm_mailbox (id, to_key, sealed_envelope, received_day)` and the old table is DROPPED by migration (secure_delete zeroes the pages). The sender's identity now travels Dilithium3-SIGNED inside the Kyber768-sealed envelope (v2: inner `{v:2,from,to,ts,text,sig}`, preimage `hum/dm/v2\n...`), so DM authenticity became end-to-end instead of relay-vouched, and a spoofed sender fails signature verification client-side. One DM = two `dm_put` deposits (recipient copy + self copy into the sender's own mailbox so their other devices fetch sent history). Mail EXPIRES after `dm_mailbox_ttl_days` (new server setting, default 30, editable in native Server Settings) and users can scrub instantly ("Delete my server mailbox" in both clients → `dm_purge`). Long-term history lives ONLY client-side, encrypted under seed-derived keys: native `src/net/dm_store.rs` (AES-256-GCM file), web `chat-dm-store.js` (encrypted IndexedDB). The native "send unencrypted anyway" modal and all plaintext DM paths were deleted; the v2 protocol has no field that can carry plaintext. Chose TTL-expiry over ack-deletion deliberately: ack-deletion breaks the same-identity-on-two-devices flow (first device to ack starves the second); the TTL window preserves it while still bounding what any subpoena/breach can ever collect to N days of sender-less ciphertext. Tests: v2 envelope suite (spoof/tamper/replay-dedupe), dm_store roundtrip + wrong-seed + no-plaintext-on-disk, mailbox storage suite + no-sender-column schema guard + legacy-drop migration test, and 5 handler-level lifecycle tests (gates, targeted delivery, paging, purge). Residuals documented honestly in `docs/reference/retention_and_deletion_semantics.md` (pre-cutover backups until rotation; wiretap-class live observation; mixnet work out of scope). Follow-ups if ever needed: bg parked-server mailbox fetch reuses the per-connection `history_fetched` arm (no re-fetch on bg reconnect until unpark, matches existing channel-history behavior); web wrapped-key users get at-rest IndexedDB encryption that is genuinely independent of localStorage.

9. **Real-life-first boot + real/fake multi-save model (REVISED 2026-06-30, was "game/simulator opt-out toggle," operator rejected the toggle framing: "too confusing from the start").** The actual direction: HumanityOS has multiple game saves, and each house/character in a save is flagged real or fake. Real means it maps to the operator's (eventually any user's) actual life: their real house, and with it their real resources (clothing, car, furniture, etc.) entered as data. Fake means the sim/game sandbox as it exists today. The app's default state is real-life, with sim/game content loaded secondarily, not the reverse. Today the app boots straight into the game unconditionally (an early-dev shortcut: Esc from the chat page drops you straight into the loaded world); the target is that the sim/game world does NOT eager-load at all unless the active save/character is flagged fake OR the user explicitly opens it from Settings. This needs, in order: (1) the real/fake flag itself, likely a field on the save/character model (`src/persistence.rs`'s `WorldSave` or a new per-character record, nothing like this exists in code yet, confirmed by grep 2026-06-30) (2) a way to author "real" resource data (starting with the operator's own house/car/furniture/clothing as the first real content) (3) the boot-sequence change so `load_world` in `src/lib.rs` only fires for a fake-flagged save or an explicit Settings opt-in, replacing today's unconditional Esc-from-chat path. Scope this properly (it touches persistence + character system + boot sequence) before starting; don't rush a shim.

## TIER 3: UX accessibility (the ELI5 mandate)
The platform's mission requires this layer. Not optional, just sequenced after the load-bearing security/feature work.

1. **Tooltip pass on every interactive element.** Every button, every input, every icon: short tooltip explaining what it does in plain language. Audit pages one at a time.

2. **"First 5 minutes" onboarding flow.** New user opens the app, what do they see? Today: a chat with no context. Build a guided tour: identity → seed backup → join your first channel → send your first message → set your status → done. The Onboarding page exists but needs flow polish.

3. **Localization expansion.** 5 languages today (en, es, fr, ja, zh). Add: ar, hi, pt, ru, de, sw at minimum. Existing infrastructure (`data/i18n/`) supports it; the work is translation, not code.

4. **Full accessibility audit.** High-contrast, screen-reader, colorblind, reduced-motion modes already in code (`src/gui/theme.rs` has the tokens). Audit every page against WCAG 2.1 AA. Fix violations. Document the audit in `docs/accessibility-audit.md`.

5. **Glossary integration on every page.** 150+ terms in `data/glossary.json`. Right-click any unfamiliar term → glossary popup. Native widget doesn't exist yet; web has it.

## TIER 4: long horizon
Don't touch these until TIERs 0-3 are mostly done. Listing them so they're not forgotten.

1. **LoRa mesh hardware integration.** Roadmap item. Requires actual radio hardware on hand.
2. **STARK selective disclosure.** Scaffold exists; circuit design deferred.
3. **Game-world depth.** The simulation/educational gameplay loop. Big. Cosmos Phase 4d shipped; ship-at-origin world exists; voxel asteroids exist. Lots of content + system work left.
4. **AI agent governance.** First-class AI participation is in `docs/ai/onboarding.md`. As more AI participants connect, governance protocols (Article 14 of the Humanity Accord) need to evolve from "documented intent" to "enforced rules with appeals."
5. **Distribution layer beyond GitHub.** Forgejo mirror exists. BitTorrent + IPFS scaffolded. Codeberg + Software Heritage + WinGet manifest still pending per `docs/admin/distribution-mirrors.md`.

## Recent shipped work

This file lists only NOT-yet-done items. For what shipped, the live sources are:
`git log`, `data/coordination/orchestrator_state.json` `recent_decisions` (the why),
the GitHub releases, and `docs/history/<date>.md`. (A hand-maintained "last 30 days"
list lived here but rotted to v0.283.0 while the project shipped past v0.515 -- a third
competing "what's done" list is worse than none. Don't reintroduce it: SHIPPED recaps go
in the journal, this file stays forward-looking.)

## Tier criteria: how to decide where something goes

- **TIER 0**: "We can't credibly invite strangers until this is done." Operator-attended OK.
- **TIER 1**: "We can invite known people but not unknown people until this is done." Self-service operator can fix.
- **TIER 2**: "Feature is promised but doesn't fully work." Multi-week effort.
- **TIER 3**: "Real users can use the app but they need help understanding it." Mission-critical for accessibility.
- **TIER 4**: "Nice eventually; don't let it crowd out the load-bearing work."

When adding an item, pick the LOWEST tier it could justifiably go in (i.e., the most urgent). Tier-up is rare; tier-down is normal as we discover things are less critical than they felt.
