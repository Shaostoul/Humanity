# The HDR scene target (PRIORITIES TIER 0 item 3b)

Written 2026-09-27 from a read-only planning pass over the code. It is the
build order for replacing the 8-bit scene with a linear `Rgba16Float` target,
one tonemap and one dither. Line numbers are as of v0.1384.0; re-grep before
trusting one.

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

1. **Scene target plus a final pass in the same format (bit-exact; proves the architecture).** A new `renderer/scene_target.rs` holds `SceneTarget` (texture, view, format) and `PresentPass`: its own bind group layout (one `texture_2d` and a uniform) with ONE `create_bind_group` site shared by init and `resize`, and a fullscreen `textureLoad` passthrough that copies rgba including alpha, built for `config.format`. Add `scene_format()`, returning the surface format for now, and route every builder listed above through it. In lib.rs: `let swap_view = view; let view = state.renderer.scene_view();` at about 14909, then `present_scene(&swap_view)` before returning `swap_view`. Delete `render_instanced` and retire `gpu.instanced` (frame_costs.rs 104). Register `gpu.present` in frame_costs.rs 96-118 and a row in data/performance/budget_systems.ron. Tests: a GPU unit test (the device pattern at frame_costs.rs 1490) renders a gradient and asserts a byte-exact round trip; a lint asserts no scene-PSO builder takes `surface_format()` or `config.format` (which covers the hot-reload site). Measure: `probe-sweep --only sahara-noon-ground,limb-400km,space-50000km,home-clock-night,sunset-over-water,ground-snow-gpu`, then `cloud-diff.js` before and after: max 0.
2. **Off-screen views through the scratch and the final pass.** Still bit-exact. Vantage `console-face-3` (the camera wall), and a 3840x2160 hi-res capture.
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
