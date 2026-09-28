# The HDR scene target (PRIORITIES TIER 0 item 3b)

Written 2026-09-27 from a read-only planning pass over the code. It is the
build order for replacing the 8-bit scene with a linear `Rgba16Float` target,
one tonemap and one dither. Line numbers are as of v0.1384.0; re-grep before
trusting one.

**Status (2026-09-28): increments 1 to 4 are BUILT.** Section 6 covers 1
and 2, section 7 covers 3 and 4 (the float target and the one dither, which
close the banding report), with what was measured. Sections 1 to 3 describe
the tree as it was before them; the "Where things stand" paragraph is history
now. Increment 5 (linear radiance behind a runtime flag) is next.

## Where things stand

Every pass draws straight into the swapchain `view` (lib.rs about
14908-15411). A `scene_texture` exists (renderer/mod.rs about 355,
surface.rs about 176) but nothing draws into it, and bloom is dormant
because nothing calls `BloomPass::apply`. Each shader tonemaps inline, so a
slow dark gradient quantises into flat rings one display level apart.

## 1. Every writer of the scene target

| Pass | Where (pass / PSO) | Blend | Reads the scene | Assumes 0..1 |
| --- | --- | --- | --- | --- |
| Stars: glow, points, constellations, halos | lib.rs 14971, ipc.rs 1286 / stars.rs 1125, 404, 471, 598 | additive, alpha-over, alpha-over, additive | no | yes: display-referred (galaxy_glow.wgsl 92, stars.wgsl 80, star_halo.wgsl 134); additive sums above 1 clamp per write today |
| Celestial opaque and patches | celestial.rs 1476 / pipeline.rs 1324, 1145 | replace | no | ACES tail |
| Celestial transparent (air, cloud shells, water) | celestial.rs 1669 / pipeline.rs 1441 | alpha | no | ACES output blended over display values |
| Emission (aurora) | emission_pass.rs 319 / pipeline.rs 748 | One+One | depth | `srgb_dither` at 95-emission-pass.wgsl 143 |
| Cloud composite | cloud_composite.rs 284 / 183 | premultiplied over | depth, plus its own Rgba16Float map | the map holds ACES output |
| Celestial and ring lines | overlay_draw.rs 362, 81 / line.rs 219 | alpha | no | display colours (line.rs 142) |
| God rays | godrays.rs 246 / 127 | SCREEN (OneMinusDst, One) | depth | **yes, and it breaks above 1** |
| SSAO | ssao.rs 169 / 89 | multiply (Dst, Zero) | depth | safe in HDR |
| Scene opaque, transparent, overlay | scene_draw.rs 302, 369, 472 / pipeline.rs 1440-1442 | replace, alpha, alpha | no | ACES tail |
| Particles, CPU and GPU | overlay_draw.rs 180, 300 / particles.rs 602 | alpha, SrcAlpha+One | no | display-referred (particles.wgsl 92) |
| Bloom (dormant) | bloom.rs 154 | replace | would read the scene | later it consumes the HDR target |
| `render_instanced` (no callers) | mod.rs 2675-2799 | writes the swapchain with scene PSOs | | delete it |
| egui | lib.rs 16112, built at lib.rs 1476 | | | must stay in display space |

Scene pipelines are also built at: the shader hot reload (mod.rs 2153,
`self.config.format`), particles (mod.rs 1250), lines (mod.rs 1269), stars
(lib.rs 1567 and world_load.rs 912), and the passes (mod.rs 1196-1203).
Off-screen views go through `render_view_onto` (ipc.rs 1236): the hi-res
capture (capture.rs 76) and camera screens (camera.rs 253 puts the screen
texture in the scene format, via screens.rs 541).

Not touched: the billboard bake and tree atlas (billboard_bake.rs 656, 719;
mod.rs 1646), sky_view.rs 151, cloud_resolve.rs 182, and the shadow passes.

## 2. Inline tonemaps, and what each becomes in the linear increment

- **Main tail** (80-fragment-shared.wgsl 436-442): return `max(color, 0)`, no clamp.
- **Water backstop, wave shell, glow shell** (90-fragment-main.wgsl 516-535, 862-874, 1086-1090): output linear.
- **Atmosphere** (30-atmosphere.wgsl 1008-1019): output linear in-scatter, keeping a local ACES only to compute `sky_lum`, which drives the star-occlusion alpha just after it.
- **Clouds** (40-clouds.wgsl 1715, 1861, 2054, 6489, `cloud_march_core`): output linear premultiplied radiance. The history clamp at cloud_resolve.wgsl 199 gets 1/(1+luma) weighting so HDR highlights do not become fireflies.
- **Aurora** (95-emission-pass.wgsl 141-143): drop the `1-exp` shoulder and the dither; delete `srgb_dither` (30-atmosphere.wgsl 752-777) once nothing calls it.
- **Display-referred writers** (stars, galaxy glow, halos, particles, `LINE_SHADER`): wrap the output in `aces_inverse()` so each lands where it was tuned. The FALLBACK copies embedded in stars.rs change in lockstep (star_halo.wgsl 23).
- **God rays**: SCREEN gives src + dst(1-src), which pulls HDR highlights down to 1. Change it to One+One, let the ACES shoulder guard it, retune the intensity.
- **CPU mirrors** (cloud_reference.rs 555 and 1527): apply ACES after compositing, not per sample.
- **Exposure constants** (SKY_LUT_EXPOSURE, ATMO_EXPOSURE*, 90-fragment-main.wgsl 32-47): unchanged. It is the same curve, applied once.
- **Not compiled** (no `include_str`): pbr.wgsl 193-208, procedural_material.wgsl 313, and the `pow(1/2.2)` material shaders. Leave them.

## 3. Captures and the pixel-measuring scripts

- **The probe rig's window grab** (`{}` in screenshot_request.json; probe-sweep.js 661-853, then lib.rs 16163, ipc.rs 1026, capture.rs 52) reads the swapchain after egui. The final pass writes the swapchain, so this path and its BGRA swizzle are unchanged, as is stream_capture.rs 115.
- **The hi-res capture** (ipc.rs 1115) and **camera screens** render into a view-sized HDR scratch, then run the final pass into a display-format target. The scratch follows the view_depth.rs Parked/Active rule and is dropped after a one-off capture (an 8192 square capture needs 512 MB for a moment). Camera screens stay in the display format (pass `surface_format()`). Update the notes at ipc.rs 1226-1229 and camera.rs 22-25 and the test at camera.rs 512-515.
- **The pixel scripts** all read 8-bit PNGs through sharp. Increments 1 and 2 must give byte-identical captures, increment 3 differs by about one code at most. After the dither (increment 4), mean-based metrics should not move (aurora-gate.js, night-side-mean.js, measure-sky.mjs, cloud-brightness.js), but the high-frequency ones will read the dither (cloud-grain-metric.js, speckle-census.js, terminator-grain.js, aurora-comb.js, glint-autocorr.mjs, blackline-census.mjs). Add a showcase key `present_dither:0` (precedent: `cloud_dither`, 45-cloud-temporal.wgsl 222-233) and pin it in those gates' vantages. The linear increment moves every number: re-baseline from same-boot A/B pairs and update the `expect` fields in tests/visual/vantages.json.

## 4. The increments, each one bootable

1. **Scene target plus a final pass in the same format (bit-exact; proves the architecture).** **BUILT 2026-09-27, see section 6.** A new `renderer/scene_target.rs` holds `SceneTarget` (texture, view, format) and `PresentPass`: its own bind group layout (one `texture_2d` and a uniform) with ONE `create_bind_group` site shared by init and `resize`, and a fullscreen `textureLoad` passthrough that copies rgba including alpha, built for `config.format`. Add `scene_format()`, returning the surface format for now, and route every builder listed above through it. In lib.rs: `let swap_view = view; let view = state.renderer.scene_view();` at about 14909, then `present_scene(&swap_view)` before returning `swap_view`. Delete `render_instanced` and retire `gpu.instanced` (frame_costs.rs 104). Register `gpu.present` in frame_costs.rs 96-118 and a row in data/performance/budget_systems.ron. Tests: a GPU unit test (the device pattern at frame_costs.rs 1490) renders a gradient and asserts a byte-exact round trip; a lint asserts no scene-PSO builder takes `surface_format()` or `config.format` (which covers the hot-reload site). Measure: `probe-sweep --only sahara-noon-ground,limb-400km,space-50000km,home-clock-night,sunset-over-water,ground-snow-gpu`, then `cloud-diff.js` before and after: max 0.
2. **Off-screen views through the scratch and the final pass.** **BUILT 2026-09-27, see section 6.** Still bit-exact. Vantage `console-face-3` (the camera wall), and a 3840x2160 hi-res capture.
3. **`scene_format` = `Rgba16Float`, shaders untouched**, with a clamp in the final pass. Only the intermediate quantisation disappears. `cloud-diff`: at most 2 codes; any pixels over 8 codes should be star cores only (additive sums above 1 under the atmosphere). Run the scripts' A/B deltas.
4. **One dither**: triangular noise, spatial only so captures stay deterministic, the `srgb_dither` step math moved into the final pass; remove the aurora's own dither in the same commit. This is the increment that closes the banding report. GPU test: a linear 0 to 0.02 ramp has no run of equal codes longer than N, and the mean moves by under 0.1 code. New `scripts/band-census.js` (mean run length of equal codes in dark crops). Vantages: aurora-limb-1200, aurora-polar-1500, limb-400km, orbit-terminator-3000km, desert-night, land-fog, night-horizon, shore-dawn-0600, home-clock-dawn, coast-night-2000.
5. **Linear radiance behind a runtime `hdr_linear` flag, default off.** The flag rides spare uniform lanes (the camera pad bit field at 45-cloud-temporal.wgsl 230) and the final pass's own uniform, so no existing bind group layout changes. Every change in section 2 lands here. Same-boot A/B at the increment-4 set plus sun-behind-deck, cumulus-closeup-ultra, approach-2000km-high, ocean-storm-glitter and the aurora-over-land and -dark pairs.
6. **Flip the default, re-baseline, then delete the old tails, the flag and the clamps.**

## 5. Constraints and cost

- **Line budgets** (tests/file_size_ratchet.rs): renderer/mod.rs sits at its 2,800; the field swap (`scene_texture` and `scene_view` into `scene`) nets about zero and deleting `render_instanced` frees 128 lines. lib.rs was at 16,679 of 16,700, so the three lines fit. celestial.rs (1,907 of 1,950) is untouched.
- **wgpu limits**: Rgba16Float as a blendable, filterable render target is core, so `Limits::default()` is enough.
- **Bind group layouts**: one new layout, no existing layout changes. A lint should count exactly one `create_bind_group` for it.
- **egui** draws after the final pass and is never tonemapped or dithered.

Estimated GPU cost, assuming about 450 GB/s of memory bandwidth (worse if this GPU blends 64-bit formats at half rate; watch `gpu.cloud_composite`, `gpu.ssao` and `gpu.celestial_t`, and measure at space-50000km, fuji-forest-ground and approach-2000km-high):

| | 1600x900 (1.44 MP) | 4K (8.29 MP) |
| --- | --- | --- |
| Target memory at 8 B a pixel | 11.5 MB (5.8 MB more than today's unused 8-bit texture) | 66 MB (33 MB more) |
| Final pass (12 B a pixel) | about 0.05 ms | about 0.25 ms |
| Doubled traffic on about six full-screen layers | about 0.15 ms | about 0.85 ms |
| **Total** | **about 0.2 to 0.4 ms** | **about 1 to 2 ms** |

## 6. Built: increments 1 and 2 (2026-09-27)

### What landed

- **`renderer/scene_target.rs`** (new): `SceneTarget` (texture, view, format, and its own present bind group, made once with the texture) and `PresentPass` (its own layout: one `texture_2d` read with `textureLoad` plus a 16-byte uniform whose `flags.x` is the clamp increment 3 needs). The one `create_bind_group` site is `PresentPass::bind`, shared by init, `resize` and the view scratch. `scene_format_for(display)` is the single line increment 3 changes; `Renderer::scene_format()` reads the target's format. Shader: `assets/shaders/present.wgsl` (fullscreen triangle, rgba including alpha, no blend).
- **Every scene-PSO builder takes the scene format**: the megashader PSOs, the shader hot reload (was `self.config.format`), particles (was `config.format`), lines, stars (the lib.rs preload thread and world_load), bloom, god rays, SSAO, the cloud composite. Their `surface_format` parameters are renamed `scene_format`.
- **lib.rs**: `Ok((output, swap_view))`, `let view = state.renderer.scene_view_for(&swap_view)`, and `present_scene(&swap_view)` after the last scene pass. egui, the window grab and the live broadcast copy still work on the swapchain, after the present.
- **Increment 2**: `render_view_onto` swaps in a view-sized scratch (`begin_view_scene`) and presents it into the caller's display target (`end_view_scene`), under the depth rule: kept for camera screens, dropped after a screenshot. Camera screens stay in the display format; the trait parameter is now `display_format`, and the camera notes and test say why.
- **Deleted**: `render_instanced`, `InstanceBatch`, `create_scene_texture`, `gpu.instanced`.
- **Costs**: `gpu.present` / `cpu.present` (the live frame and the hi-res screenshot) and `gpu.screen_present` / `cpu.screen_present` (a camera screen's copy). New budget row "Scene present"; the screen ids join the two screens rows.
- **Tests**: `present_pass_is_byte_exact_on_a_real_device` (skips with a note without an adapter): in all four 8-bit formats a surface can pick, every code of every channel (alpha included) goes through the present pass unchanged, and an alpha-blended gradient drawn through the target and presented equals the same draw straight into the display. Proven red by a shader that drops alpha. Plus a naga validation of present.wgsl. **`tests/scene_format_lint.rs`** (in `just lints`): every builder call passes a `scene_format` expression and never `surface_format` or `config.format`; every other display-format read carries an inline `display-format:` note; the present layout has one bind group site. On the pre-change tree it named exactly the builder list of section 1, the hot reload included.

### Where it departs from the plan

1. `gpu.present` has a CPU twin (`cpu.present`) instead of a place on the no-twin list: the Performance page's fallback test holds that every screen pass has one, and a twin is one stage guard.
2. A camera screen's present is its own id, like every other camera-wall pass.
3. **The A/B switch** was not in the plan: the showcase key `present_direct` ("1" draws straight into the display, the old path) exists so a same-boot A/B can compare the two paths and read the pass's cost. It only acts while the formats are equal; from increment 3 it is ignored and the log says so. Delete it at increment 6 at the latest. `scripts/probe-hot-ab.js` arms can now carry a `showcase` object for it, and a plan can ask for a hi-res `shot`.
4. The scene target carries COPY_DST (the GPU test uploads into it).
5. The billboard bake and the tree atlas keep the display format, each read marked `display-format:` (section 1 said not touched).

### Measured

**Cost** (RTX 4070, timestamp queries, 2560x1387, the final exe): `gpu.present` 0.050 to 0.067 ms across the seven vantages (space-50000km 0.067, home-clock-night 0.062, sahara 0.060, sunset 0.058, limb 0.053, console-face-3 0.051, snow 0.052); `gpu.screen_present` about 0.001 ms for the camera wall. `cpu.present` (the EMA of encoder, pass and submit) 0.15 to 0.45 ms, which is the price of a separate submit; folding the present into the egui encoder would recover it if it ever matters. Memory: none new for the window (the unused 8-bit scene texture it replaced was already allocated); a camera screen's parked scratch is its own size at 4 B a pixel.

**Bit-exactness.** The rig cannot give two identical consecutive window frames at most of these vantages in EITHER build, so a window capture alone cannot show max 0. Same park, consecutive captures, pins `anim_clock 300, wind 0, weather clear, time_scale 0`:

| Vantage | baseline build, capture vs next capture (its own floor) | new build, switch flip (direct vs target) | pixels that change on BOTH flips and on neither same-arm pair |
| --- | --- | --- | --- |
| console-face-3, **3840x2160 hi-res** | **0 px** | **0 px, in three boots** | 0 |
| console-face-3 window | 394 px, max 2 (then 1,475 px, max 135: the camera feed's plant sways) | 378 to 1,436 px, max 1 to 135 | 0, 0, 1 (max 1) |
| home-clock-night | 317 px, max 184 (sparse speckles on the hull) | 211 to 283 px | 1, 0, 0 |
| ground-snow-gpu | 9,306 px (the flakes) | 10,108 to 11,270 px | 2 to 11 (flake coincidences) |
| space-50000km | 7,435 px, almost all on the Earth disc | 7,656 to 7,780 px, same disc | 285 to 521, max 2 to 4 |
| limb-400km, sahara, sunset | 250k to 630k px, ground and sky moving by up to 141 codes (sunset) (clouds off does not stop it: the terrain moves too) | same magnitude | not separable: the scene alternates |

And the literal before/after: the **3840x2160 capture at console-face-3 is byte-identical between the baseline build and the new build** (different boots, 0 of 8,294,400 pixels differ). That capture runs every pass the live frame runs except particles (stars, planet and clouds, orbit lines, god rays, SSAO, opaque, transparent, overlay, ring lines) through the view scratch and the present pass, with the camera wall's own texture in view, which the camera-screen present wrote. The window captures differ only where the scene itself moves: particles (drawn in the window path, never in a hi-res capture), the camera wall's plant sway (the `anim_clock` pin does not reach a camera screen's view, a rig determinism gap worth closing), and outdoor terrain and sky that change between consecutive parked frames. Everywhere the scene holds still (the star field around the Earth disc, 99.99 % of the home frames, the whole hi-res frame) the switch changes nothing. panics 0 at world entry in every boot.

One caveat on the 0 px row: a fifth boot (the check of `same_park` mode itself) parked console-face-3 in a state where surfaces sparkle frame to frame, and its 3840x2160 direct/target pair differed in 1,166 scattered pixels (max 10 codes), the same speckle class the home-clock-night frames show between two captures of the baseline build. The 0 px rows are the three boots where that park held still.

How the table was taken: park once, then capture direct, target, direct, target at that one park (`scripts/probe-hot-ab.js` with `same_park: true` does this now; re-parking per arm, its old and default protocol, moved 18 to 33 % of pixels between two captures of the SAME arm). The last column is the switch's own signature: a difference caused by the switch lands on the same pixels at both flips and never between two captures of one arm, while animation lands somewhere new each frame. The captures themselves stayed in the session scratchpad.

### For increment 3

Change `scene_format_for` to `Rgba16Float` and write `flags.x = 1` into the present uniform when the formats differ. The lint then guards every builder; the billboard bake keeps the display format. The A/B switch becomes a logged no-op. Re-measure `gpu.present` (the read doubles to 8 B a pixel) and the passes section 5 names.

## 7. Built: increments 3 and 4 (2026-09-27, measured 2026-09-28)

### What landed

- **Increment 3.** `scene_format_for` returns `Rgba16Float`; the present pass
  clamps (and scrubs NaN) whenever the scene and output formats differ.
  `PresentPass::new(device, output_format, scene_format)` writes its own
  uniform. The billboard bake keeps the display format.
- **Increment 4.** ONE triangular spatial dither in `assets/shaders/present.wgsl`,
  exact in code space (encode, add noise, decode), with the amplitude narrowed
  to `c + 0.4` within 0.6 of a code of either end, so black and white stay
  exact (the space background is 0 and stays 0). The aurora's own dither
  (`95-emission-pass.wgsl`) and `srgb_dither` (`30-atmosphere.wgsl`) are
  deleted.
- **Showcase keys** for same-boot A/B: `present_dither` (0 or 1) and
  `scene_format` (display or hdr). The latter rebuilds the scene target, every
  scene PSO (`renderer/scene_format_ab.rs`) and the star sky
  (`world_load::build_star_sky`, extracted for it); a rebuild takes 13 to 27 s
  on the rig. probe-sweep and probe-hot-ab reset `present_dither` to 1 for
  every cell that does not pin it.
- **`scripts/band-census.js`**: the mean run of identical codes in the dark
  part of a capture (checked on synthetic ramps: banded 318, dithered 2.2).
- **GPU tests** (`src/renderer/scene_target_tests.rs`, all 7 pass): the
  display arm is byte-exact, the float target presents every code and clamps,
  and a dark ramp (0 to 0.02 over 8,192 pixels) has no run of one code longer
  than 64 with every block mean within 0.1 code.

### Measured: increment 3 (the float target alone, dither off)

Same boot, same park, captures display, hdr, display, hdr (plan
`scripts/hdr-ab/planA.json`, pins `anim_clock 300`, `wind 0`,
2560x1387). A pixel the switch changed differs at both flips and at neither
same-arm pair; the "floor" is what two captures of the SAME arm differ by.

| Vantage | same-arm floor | switch pixels | switch max (codes) | over 2 codes |
| --- | --- | --- | --- | --- |
| console-face-3 | 0 | 17,156 | 2 | 0 |
| home-clock-night | 109k | 283,793 | 2 | 0 |
| space-50000km | 26 | 31,809 | 2 | 0 |
| ground-snow-gpu | 7.6k | 4,931 | 1 | 0 |
| sahara-noon-ground | 748k | 480,274 | 3 | 4 |
| aurora-limb-1200 | 4.3k | 61,275 | 6 | 2 |
| limb-400km | 280k | 1,620,540 | 5 | 20 |
| approach-2000km-high | 670k | 513,789 | 7 | 1,342 (1,264 in dark star neighbourhoods) |
| sunset-over-water | 312k | 904,176 | 21 | 40 (36 star-like) |
| fuji-forest-ground | 308k | 809,756 | 104 | 9 (3 over 32) |

The plan's bar was "at most 2 codes, anything over 8 on star cores only". Where
the scene holds still (the console room, the home at night, space, snow) the
switch moves pixels by 1 or 2 codes and nothing more: the intermediate 8-bit
rounding is gone. The larger values come from vantages whose floor is hundreds
of thousands of pixels. At sunset 36 of the 40 sit in dark neighbourhoods (the
script's star-like test; the largest, 21 codes, is a small cluster near
(2369, 1020) that reads darker in the float arm, and its cause was not
isolated). The three Fuji pixels over 32 codes are most likely motion that
happened to alternate with the arms: with 30 % of the frame moving between two
captures of one arm, a few such coincidences are expected. Nothing over 8
codes in any still scene.

### Measured: increment 4 (the one dither), the banding report closed

Same boot, same park, four arms (plan `scripts/hdr-ab/planB.json`):
`before` is the old build's picture (display target, the aurora's own dither
patched back in through the hot reload), `inc3` the float target with no dither
at all, `after` the float target with the present dither, and `before2` a
repeat of `before` last, whose distance from the first is the same-boot floor.
`scripts/band-census.js` counts runs of one identical code among the dark
pixels (brightest channel at most 64, not pure black), along rows and columns:
the mean run is the band width, `long` the share of dark pixels in a row run of
16 or more.

| Vantage | before (mean run / long) | inc3 | after |
| --- | --- | --- | --- |
| aurora-over-land-dark | 7.1 / 53 % | 7.1 / 53 % | **2.3 / 10 %** |
| ocean-grazing-calm | 5.6 / 64 % | 5.6 / 64 % | **3.1 / 52 %** |
| night-horizon | 4.8 / 70 % | 4.7 / 58 % | **1.8 / 27 %** |
| aurora-polar-1500 | 3.3 / 59 % | 4.4 / 63 % | **1.9 / 12 %** |
| shore-dawn-0600 | 2.6 / 54 % | 2.7 / 54 % | **1.7 / 9 %** |
| coast-night-2000 | 2.6 / 54 % | 2.6 / 54 % | **1.7 / 28 %** |
| aurora-limb-1200 | 2.5 / 50 % | 2.5 / 50 % | **1.7 / 16 %** |
| desert-night | 2.4 / 42 % | 2.4 / 41 % | **2.0 / 36 %** |
| orbit-terminator-3000km | 2.2 / 42 % | 2.2 / 42 % | **1.6 / 17 %** |
| aurora-over-land | 1.9 / 17 % | 4.1 / 36 % | **1.7 / 9 %** |
| home-clock-dawn | 1.7 / 25 % | 1.7 / 25 % | **1.4 / 16 %** |
| cloudlum-2000-high | 1.7 / 32 % | 1.7 / 31 % | 1.6 / 30 % |

`before2` matched `before` to within 0.1 of a run everywhere except
night-horizon (4.8 against 5.2). land-fog and limb-400km are bright daytime
scenes whose only dark pixels are the HUD (239,456 of them, identical in every
arm and in both vantages, because egui draws after the present pass): the
census has nothing to read there, which is the expected answer, not a
measurement.

Read together: the float target alone (`inc3`) changes little where the
aurora is absent (night-horizon's long share, 70 to 58 %, is the one real
move), and makes the aurora vantages WORSE (1.9 to 4.1, 3.3 to 4.4),
because the branch deleted the aurora's own dither; the present dither then
beats both. The flat runs that remain after it (desert-night, cloudlum) are
genuinely flat surfaces at one code, which the census counts before and after
alike.

**Means held.** Mean brightness per capture moved by at most 0.1 of a code
between `before` and `after` everywhere the same-boot floor allows a reading
(night-horizon 2.58 to 2.67 against a floor of 0.04; shore-dawn's own floor is
1.5 codes, the scene moves). **The aurora comb held** (`scripts/aurora-comb.js`,
the aurora's own dither against the present dither): 8.5 against 8.7 % at
aurora-limb-1200, 4.8 against 4.8 at aurora-polar-1500, 2.9 against 2.9 at
aurora-over-land. The float target does find about 4.6 % more aurora pixels
over land (153,853 to 161,105 above the script's threshold, mean green 60.1
to 58.3): faint curtain edges that the 8-bit intermediate rounded away.

**Gate pins.** The vantages of the high-frequency gates (glint-autocorr,
blackline-census, speckle-census, cloud-speckle-census, cloud-grain-metric,
terminator-grain: 35 vantages) pin `present_dither: "0"`, with the reason in
the `_dither_note` at the top of tests/visual/vantages.json; the mean-based
gates and aurora-comb run dithered.

### Measured: cost

Same boot with `HUMANITY_FRAME_COSTS=1`, same park, 45 s settle per arm
(plan `scripts/hdr-ab/planC.json`), arms display, hdr, hdr with the
dither, display, hdr; RTX 4070 at 2560x1387, timestamp queries, ms:

| Pass | space-50000km display / hdr / hdr+dither | fuji-forest-ground | approach-2000km-high |
| --- | --- | --- | --- |
| `gpu.present` | 0.067 / 0.093 / 0.094 | 0.055 / 0.072 / 0.075 | 0.055 / 0.070 / 0.074 |
| `gpu.ssao` | 0.026 / 0.048 / 0.048 | 0.122 / 0.127 / 0.127 | 0.106 / 0.107 / 0.107 |
| `gpu.stars` | 0.065 / 0.083 / 0.083 | (not drawn) | 0.059 / 0.072 / 0.071 |

Every other pass (`gpu.scene`, `gpu.celestial`, `gpu.celestial_t`,
`gpu.cloud_composite`, the cloud passes, `gpu.shadow`, `gpu.aurora`) moved by
no more than its own display-to-display floor (for example `gpu.celestial_t`
at space read 0.363 and 0.462 in the two display arms). The float target
costs about 0.02 ms in the present pass (it reads 8 B a pixel instead of 4)
and 0.01 to 0.02 ms in SSAO and the stars; the dither costs under 0.004 ms.
**In all, about 0.03 to 0.06 ms at 2560x1387**, against the plan's estimate of
0.2 to 0.4 ms at 1600x900: the bandwidth estimate in section 5 assumed six
full-screen layers doubling their traffic, and on this GPU most of them do not
show it.

### Checked at world entry

panics 0 in every boot: the three A/B plans (27 vantages between them), the
rig boot at space-50000km, and `photograph-home.js` at 00-overview and
25b-mushroom-racks. The release build and the relay check are clean.

### Next: increment 5

Linear radiance behind a runtime `hdr_linear` flag, default off (section 4).
With the target in float, a value above 1 now survives from one pass to the
next, so the inline tonemaps can move into the present pass one at a time
behind that flag.
