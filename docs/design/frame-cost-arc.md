# The frame cost arc: where the frame goes, and the order to take it back

> Design of record, written 2026-09-18 against v0.1314.1 from the clouds-off
> measurement at the operator's live settings (2560 x 1387, real timestamp
> queries, RTX 4070; 21 boots, 72 cost files; the capture set lives in the
> 2026-09-18 session scratchpad, run1-operator-cloudsoff through
> run9-tree-grass, and its summary in the 2026-09-18 journal entries and
> `docs/PRIORITIES.md`). Every mechanism below was re-read in the tree.
> Numbers are measured or arithmetic on measured numbers; a hypothesis says
> so and carries the experiment that kills it. Companions:
> `docs/design/cloud-far-rung.md` (the contract style this follows),
> `docs/design/cloud-sun-shadow-cache.md` (the increment pattern that
> worked), the v0.1282 to v0.1288 blocks of `docs/PRIORITIES.md`.

## 0. The finding that reorders the arc: a per-pixel floor that belongs to the shader, not to the work

The three cost centres were briefed as separate. Two of them are the same
defect.

**The arithmetic.** 2560 x 1387 = 3,551,720 pixels. Per-pixel costs from this
machine, this frame, this resolution:

| pass | shader | measured ms | shaded px | ns per pixel |
|---|---|---|---|---|
| `gpu.ssao` | `assets/shaders/ssao.wgsl`, 10 taps, fullscreen triangle (`src/renderer/ssao.rs:182`) | 0.01 to 0.18 | 3.55 M | 0.003 to 0.051 |
| `gpu.godrays` | `assets/shaders/godrays.wgsl:76`, 40 dependent `textureLoad` taps, fullscreen triangle (`src/renderer/godrays.rs:255`) | 0.42 (max, console-face-3) | 3.55 M | 0.118 |
| `gpu.stars` | `assets/shaders/stars.wgsl` | 0.9 to 1.2 | sky only | 0.25 to 0.34 upper bound |
| `gpu.celestial`, `moon-surface-200m` | megashader `fs_main` | 44.5 | about 3.05 M (ground from y = 192 down, full width) | 14.6 |
| `gpu.scene`, `console-face-6` | megashader `fs_main` | 82.8 | 3.55 M at one layer (derived in section 2) | 23.3 |
| `gpu.scene` delta, 2.2 m to 0.3 m from the wall screen | megashader `fs_main`, type 24 | +62.0 | at most 3.55 M | at least 17.5 |

The moon row is the one that cannot be argued with. The run log says
`Planet chunks 'moon': ACTIVE (6 patches drawn, 27 cached, 0.6 MB)`. Six
patches at `PATCH_MESH_BYTES` (`src/terrain/planet_chunks.rs:198`) is about
6,300 vertices. One material, untextured fallback, no clouds, no ocean shell,
no atmosphere shell, `light_count.x = 0` in the celestial pass so no point
lights at all (`assets/shaders/pbr/90-fragment-main.wgsl:2076`, and the guard
comment at 2096 to 2103). Six non-overlapping patches sorted front to back
cannot overdraw. That is 44.5 ms of GPU for 6,300 vertices and one layer of
the cheapest type-12 fragment in the engine.

**The residual, and why it is not ALU.** Price the two toggles that do move:

- Surface detail. `planet_surface_detail off + terrain_detail_distance 1`
  removes 8.8 ms at Fuji and 11.2 ms at ocean-storm-low. That path is
  `land_detail_factor` plus three camera-relative micro octaves plus
  `ground_detail` (`90-fragment-main.wgsl:1559` to `1620`). `ground_detail`
  runs four loops over `GROUND_MAT_COUNT = 5` (`20-surface-detail.wgsl:806,
  860, 875, 889`) and each present layer takes a `ground_tri4`, three
  `textureSampleGrad` on a filtered array texture (`20-surface-detail.wgsl:330`
  to `332`). Call it 30 to 60 anisotropic taps. Over roughly 3.0 Mpx of
  terrain that is 2.9 ns per pixel, which at 50 to 100 ps per filtered array
  tap is exactly right. The part the toggle can switch off costs what the
  hardware says it should cost.
- The light loop. From run5-console-fill: `c6-lights200` minus
  `c6-control-2m2` is `gpu.scene` +33.90 and `gpu.transparent` +5.93 for 200
  camera-pinned in-range lights (`[lights-diag] assembled=412`). If the
  per-light per-pixel cost is the same in both passes, the ratio 33.90 / 5.93
  = 5.72 fixes the coverage ratio; taking `gpu.scene` at one full screen gives
  the glass ceiling 0.62 Mpx (17.5 percent of the frame, which is what the
  capture shows) and k = 47.7 ps per in-range light per pixel. Independently,
  `lights_tiled` on saves 8.0 ms with 211 mostly out-of-range lights, which is
  10.7 ps per rejected light per pixel. Both are believable issue-rate
  numbers. The whole 211-light untiled loop is therefore 2.25 ns per pixel,
  9.7 percent of the interior's 23.3.

So on the moon, 2.9 of 14.6 ns per pixel is explained and 11.7 is not. In the
console room, 2.25 of 23.3 is explained and about 21 is not.

11.7 ns per pixel on a 4070 (5888 lanes at 2.48 GHz = 1.46e13 lane-cycles per
second) is 171,000 lane-cycles per pixel; 21 ns per pixel is 307,000.
`fs_main` on the interior and moon paths is a few hundred instructions. ALU is
off by three orders of magnitude. It is also not geometry (6,300 vertices on
the moon), not draw submission (the 2.2 m to 0.3 m delta changes no draw and
no vertex, only screen coverage), and not early-z failure (see the refutation
in section 2). It scales with covered pixels and with nothing else, in two
different pipelines, in two different passes.

The one thing those two pipelines share is the module. `Pipeline::new`
compiles seven PSOs from one 14,296-line megashader assembled by
`src/renderer/shader_loader.rs:18` `PBR_PARTS`: opaque, transparent, overlay,
two shadow, patch render, patch shadow (`src/renderer/pipeline.rs:876` to
`1013`). Four of them use `fs_main` as the fragment entry (`pipeline.rs:780`
and `920`). The comment at `pipeline.rs:894` measures the consequence: each
bake compiles the WHOLE fragment into a backend PSO, about 10 s of
Naga-to-DXIL work apiece on this GPU.

**Leading hypothesis (H1): the per-fragment floor is the megashader's
per-invocation private storage frame, allocated and initialised for every
fragment of every pipeline that uses `fs_main`, including terrain and interior
walls that never touch it.**

The prime suspect is named and dated in the repo's own record.
`assets/shaders/pbr/41-cloud-bodies.wgsl:618`:

```wgsl
var<private> g_bc_lc: array<vec4<f32>, 180>;
```

2,880 bytes per invocation, module scope, no initialiser (so WGSL requires
zero-initialisation), dynamically indexed at lines 654, 659, 679, 698, 700,
722, 729 (so no backend can keep it in registers), plus `g_bc_key` (144 B) and
`g_bc_width` (36 B) at 612 and 617, plus the `var<private>` `GROUND_*` tables
at `20-surface-detail.wgsl:396` to `409`. It is reachable from `fs_main`
through `cloud_layer` (`90-fragment-main.wgsl:927`), so a backend cannot
dead-strip it from that entry point. It is NOT reachable from `fs_shadow`, and
the far-rung arrays (`45-cloud-temporal.wgsl:671` to `1012`, up to about
1.3 KB) live in `fs_cloud_profile_bake`, a separate entry point, so those do
not bear on `fs_main`.

`docs/PRIORITIES.md:1297` already recorded the mechanism in the pass it was
built for: "the per-invocation slot arrays spill to local memory and a ray
that crosses many cells amortises few builds, so the closeup is the case for
the per-cell cluster TABLE". What has never been asked is whether that spill
is also being charged to every terrain patch and every interior wall in the
game.

The bandwidth arithmetic fits. 11.7 ns per pixel over 3.05 Mpx is 35.7 ms; at
the 4070's roughly 504 GB/s that is 18.0 GB, which is 5.9 KB per fragment. The
declared private frame is about 3.1 KB and a spilled register frame on top of
it puts 5.9 KB in exactly the right place.

**Circumstantial support, offered as circumstantial only.** The sweep
`.probe-rig/sweeps/20260821-043333` was taken at
`graphics_source: rig-config-matches-operator`, 2560 wide, real timestamps,
before any of the cloud-arc growth: `ladder-0_3km` `gpu.celestial` 6.6 ms,
`ladder-400km` 17.1 ms, whole-ladder max 18.4 ms. The same families now read
42.9 (Sahara ground) and 61 to 64.6 (limb-400km). Confounded by settings drift
(`terrain_split_px` was 2 in the 09-15 baseline and is 10 now) and by vantage
differences, so it proves nothing on its own. It is consistent with H1 and
with nothing else that was done to terrain in that window.

**Alternative hypothesis (H2):** the floor is somewhere outside the fragment
shader entirely (a per-pass or per-draw driver stall that happens to correlate
with coverage). H2 predicts the increment-1 experiment below moves nothing.

Everything in sections 1 and 2 is written so that it is correct under H1 and
under H2, because increment 1 decides between them in one boot.

## 1. Cost centre one: the planet pass, `gpu.celestial`

### (a) Mechanism as read

`Renderer::render_celestial_onto` (`src/renderer/mod.rs:3121`) opens one render
pass at `mod.rs:4523` labelled "Celestial Pass", timed as `gpu.celestial` at
`mod.rs:4526`, which draws, in order:

1. the classic opaque list (bodies, near-tree models, props) one
   `draw_indexed` each, `mod.rs:4570` to `4595`, material binds skipped when
   unchanged;
2. the batched terrain patches on `patch_render_pipeline` (`mod.rs:4606` to
   `4640`), one `multi_draw_indexed_indirect` when `patch_indirect`, otherwise
   one draw per patch;
3. the grass sward, ONE instanced `draw_indexed(0..gm.index_count, 0,
   0..self.grass_n)` (`mod.rs:4673`).

Then a separate pass `gpu.celestial_t` at `mod.rs:4714` draws the transparent
list (atmosphere shells, cloud shell, the water shell).

The terrain fragment is `fs_main` type 12 (`90-fragment-main.wgsl:1351` to
`1741`): terminator gate and per-fragment `frag_up` (1365 to 1375), the
sprite-card sub-branch (1397 to 1433) with two eye-distance `discard`s, the
packed or textured albedo (1435 to 1541), the footprint-faded detail stack
(1542 to 1621), cloud ground shadows (1622 to 1640), ocean glint and
`water_shade` (1641 to 1728), then the shared tail: `evaluate_light` for sun
times `sun_shadow_offset` (2048 to 2053), `evaluate_light` for fill (2068), the
point-light loop (2092 to 2154), `sky_ambient` (2165), `aerial_apply` (2199),
`underwater_apply` (2206), ACES (2209 to 2214).

Which of those actually run per pixel regardless of the CPU-side toggles:

- `sun_shadow_offset` (`00-bindings-vertex.wgsl:227`) returns at 229 when
  `shadow_u.params.x < 0.5`, and at 240 for any fragment outside the 1500 m
  ortho box; beyond the box the 3x3 PCF is skipped. The measurement agrees:
  `sun_shadows off` moves the frame by 1.0 ms at Fuji, 2.1 at the ocean, 0.1
  at the limb. Not a suspect.
- `aerial_apply` / `aerial_transmittance` return at `90-fragment-main.wgsl:122`
  to `129` when sigma is zero. `planet_atmo_scatter off + aerial 0` moves +0.6,
  -0.3, +1.5, which is noise. Not a suspect.
- `sky_ambient` (`90-fragment-main.wgsl:70`) returns zero at 73 when the
  sky-view table was not rendered and at 79 when there is no local up, but on
  a planet surface both gates are open, so every terrain fragment pays two
  `water_sky_lut` calls (`20-surface-detail.wgsl:1206`), each an `asin`, an
  `acos`, a `sqrt` and one `textureSampleLevel`. Real, unconditional,
  untoggleable, and small: under 0.5 ns per pixel at the godray pass's
  measured tap rate.
- Cloud ground shadows with clouds off. `90-fragment-main.wgsl:1630` gates on
  `has_tex && camera.light_count.w > 0.5`; `light_count.w` is poked by
  `render_celestial_onto` after the bulk uniform write. It does not run with
  clouds off (no cloud key ever appears in the clouds-off cost files).
  Refuted as a running cost. It remains a suspect for H1 for a different
  reason: `cloud_weather` is the doorway into `40-clouds.wgsl`, which is what
  makes `g_bc_lc` live in `fs_main`.
- The point-light loop is dead in this pass: `light_count.x` is zero for the
  celestial camera and the tiled path re-checks it at 2104.

So is the measurement consistent with the mechanism? Only partly, and the
inconsistency is the finding. `terrain_split_px 20` moving ocean-storm-low by
-17.3 ms (patches 3621 to 1333) and the limb by -6.9 ms (796 to 285) is
per-pixel cost times overdraw at grazing views, which the front-to-back patch
sort (`src/lib.rs:10032`, whose comment says the type-12 fragment is the
frame's most expensive shader and overdraw was eating multiple ms) mitigates
but cannot remove for grazing sightlines. The detail octaves at 8.8 to 11.2 ms
are priced correctly. Everything else is a constant times the pixel count, at
a per-pixel rate 124 times the engine's own 40-tap fullscreen pass. That is
not the terrain shader's arithmetic. It is the module.

### (b) The 2030 technique

Material shader permutations: one authored source, many specialised pipeline
variants, each containing only the code its material class can reach,
generated at pipeline build. Unreal calls them material permutations, Unity
shader variants, Godot shader specialisations. Nobody ships one fragment entry
point that contains a volumetric cloud raymarcher, an ocean FFT shading model,
an atmosphere integrator and a wall panel pattern, and hands it to every draw
in the frame.

Two ways to get there in this tree, in increasing order of commitment:

1. Pipeline-overridable constants. wgpu 24.0.5 / naga 24.0.0 (`Cargo.lock`)
   support WGSL `override` declarations and
   `wgpu::PipelineCompilationOptions::constants`, which every
   `create_render_pipeline` site already passes as `default()`. Declare
   `override HAS_CLOUD_BRANCH: bool = true;` and friends, guard the three
   heavyweight early-return branches with them, and compile
   `patch_render_pipeline` with them false. Naga substitutes and folds before
   the backend, so the cloud march and its `var<private>` tables never reach
   DXIL for that PSO. One source, no duplication, no drift risk.
2. Split entry points (`fs_terrain`, `fs_interior`, `fs_veg`, `fs_water`) if
   the override route does not prune the private globals. The repo already has
   the drift-guard pattern for this: `pipeline.rs:1066` to `1160`, the
   `fs_shadow_mirrors_the_fs_main_cutouts` test that reads the assembled
   source and requires the same literals on both sides.

Cheaper terrain fragment work is the SECOND rung and stays on the list, but it
is second because it is 2.9 ns per pixel against an 11.7 ns floor: the six
octaves are already LOD-faded by footprint (`detail_octave_fade` per octave,
window at `90-fragment-main.wgsl:1575`), the right technique, correctly
applied. A G-buffer is the wrong answer here and is recorded as rejected: this
renderer's value is a very wide material vocabulary at planet scale with
alpha-cutout vegetation, the case deferred shading handles worst.

### (c) First increment (one worktree agent)

**Increment P1: decide H1 against H2, then ship the permutation.**

Phase A, the discriminating measurement, throwaway, no commit. In a worktree,
replace the bodies of `atmosphere_scattering`, `cloud_layer` and `ocean_shell`
with `return vec4<f32>(0.0);` so that nothing in `40-clouds.wgsl`,
`41-cloud-bodies.wgsl` or the type-16 path is reachable from `fs_main`. Build,
boot the rig, capture `moon-surface-200m` only.

- Vantage: `moon-surface-200m` (`tests/visual/vantages.json`, `perf_floor_fps`
  25, `settle_s` 15).
- Cost key that must move: `gpu.celestial` 44.5 ms. Under H1 it falls below
  10 ms. Under H2 it does not move.
- Negative proof: the moon draws no atmosphere shell, no cloud shell and no
  ocean, so the capture must be pixel-identical to the clouds-off baseline
  `moon-surface-200m.png` outside the HUD text. The vantage's own `expect`
  ("Continuous grey cratered lunar regolith ... under a black star sky. NOT
  bare flat facets.") and both `regressions` entries must still hold. That is
  the whole reason the moon is the fixture: it is the only ground vantage
  where deleting those three branches is provably a no-op on the image.
- Second cell, to separate "the cloud code" from "any big branch": re-run
  phase A stubbing ONLY `cloud_layer` (leave the atmosphere and the ocean
  intact) and capture `sahara-noon-ground` (floor 20, `gpu.celestial` 42.9).
  Sahara draws the atmosphere shell, so that capture must also be
  pixel-identical, and it tests the fix on a textured, fully-detailed planet.

Phase B, the shipped change, only if phase A is green. Add the override
constants, compile `patch_render_pipeline` and `patch_shadow_pipeline` with the
cloud, atmosphere and ocean branches off, keep `render_pipeline` /
`transparent_pipeline` as they are for this increment (the atmosphere and
ocean shells draw through them). Add a Rust test that asserts each PSO's
constant map, so a future site cannot silently re-enable a branch.

- Gate: `gpu.celestial` at `moon-surface-200m` and `sahara-noon-ground` down
  by the phase-A factor, both captures pixel-identical to the pre-change
  baseline, `blue-marble-12000km` and `limb-400km` above their floors (30 and
  25), and the whole `just perf-sweep` with `--operator-config` showing no
  vantage newly below its floor.

Rig recipe:

```bash
node scripts/probe-sweep.js --operator-config --only moon-surface-200m,sahara-noon-ground --exe <worktree>/target/release/HumanityOS.exe --out <scratch>/p1-<arm>
```

Discipline, all four of which have burned this repo before: mirror the
operator's config rather than the rig defaults (the defaults understate the
planet pass threefold, `PRIORITIES.md:1224`); the rig freezes the clock with
`time_scale 0` after every park (v0.1287) and sends a discarded first pass
(`probe-sweep.js:707` to `721`), so take the `-costs.json`, not the
`-costs-postB.json`, consistently; check `tasklist //FI "IMAGENAME eq
HumanityOS.exe"` before booting, because one GPU means one instance; boot only
through the rig, never `HumanityOS.exe` by hand.

#### P1 outcome (2026-09-18, worktree agent; phase A and phase B both green)

**Verdict: H1.** The floor is the megashader's per-invocation frame, and the
cloud branch alone carries it. Every number below is a GPU timestamp query at
the operator-mirrored config, 2560 x 1387, RTX 4070, clouds off; captures,
cost files, diff tool and heatmaps live in the 2026-09-18 session scratchpad
(`p1-ab/`, `p1-b0-control/`, `p1-b1-permutation/`, `p1-diff.js`,
`diff-*.png`).

Phase A ran as a SAME-BOOT A/B rather than one boot per arm: the rig boots
the shader parts from disk, so the driver (`p1-ab.js`) captured the control,
stubbed the function bodies on disk, waited for the megashader hot-reload
(`[HotReload] megashader reassembled`, 6.3 s), captured again, then restored
the parts with `git checkout` and waited for the reload back. Same frozen
clock, same streamed patches, same GPU thermal state on both sides. The moon
cell pins its local hour (camera `time: 10.2`) because an unpinned moon lands
wherever the preceding cells left the global clock (the first control boot
captured it at night, gpu.celestial 45.6 ms even in the dark, which is itself
evidence the cost is not the lit arithmetic).

| cell | made unreachable | `gpu.celestial` control | stubbed | `gpu.celestial_t` control | stubbed |
|---|---|---|---|---|---|
| `moon-surface-200m` | `atmosphere_scattering` + `cloud_layer` + `ocean_shell` | 44.11 | 7.43 | 0.00 | 0.00 |
| `sahara-noon-ground` | `cloud_layer` only | 31.38 | 5.43 | 10.44 | 0.19 |

Against the operator baseline (run1-operator-cloudsoff): moon 44.46, so the
control reproduces it within 1 percent. The Sahara control read 31.4 in every
boot of this session (three boots, two harnesses) against the baseline's
42.9: the baseline's Sahara cell followed the limb descent, this session's
followed a warm-up park at the same coordinates, so the streamed patch set
differed. The comparison that decides is same-boot control against stub.

Pixel proof, and a correction to the contract above. "Pixel-identical" is not
a property this rig has even for an unchanged shader: two captures 1.5 s apart
in one boot with the clock frozen differ on 20 to 35 percent of pixels by 1 to
3 levels (max 12 at the Sahara), and the terrain's per-frame dither flips more
pixels at a higher frame rate (the stubbed moon at 30 fps: 85 percent at mean
3.1). Read "pixel-identical" everywhere in this document as "inside the rig's
same-boot repeat floor, with no spatially coherent difference". Against those
floors: Sahara control vs stub 34 percent, mean 1.33, max 14, and the sky rows
exactly 0.00 (the atmosphere shell was intact and is bit-identical while 50x
cheaper); moon control vs stub 87 percent, mean 3.41, block for block the
same as the stubbed build's own frame-to-frame floor (mean 3.14), and the two
heatmaps carry the same speckle and the same faint patch-seam lines. Nothing
coherent anywhere.

Bonus finding: the Sahara's atmosphere shell (type 14, classic transparent
PSO) fell 10.44 to 0.19 ms the moment the cloud branch was unreachable. The
floor sits under EVERY PSO that compiles `fs_main`, as section 0 argued, and
that is the next increment (below).

Phase B shipped the permutation: `assets/shaders/pbr/05-overrides.wgsl`
(three `override` switches defaulting to true), the guards at the three
dispatch sites in `90-fragment-main.wgsl` (switch first in the `&&`), and in
`src/renderer/pipeline.rs` the `PSO_DEAD_BRANCHES` registry plus
`pso_constants(label)`, which the four megashader builders call: the patch
render and patch shadow PSOs compile all three branches off, the classic five
keep everything. `permutation_tests` (five tests) pins the declarations, the
guard shape, the registry contents and the builder wiring against each
other. Gate, `probe-sweep --operator-config`, B0 = pristine tree and pristine
exe, B1 = this change:

| vantage | `gpu.celestial` B0 | B1 | fps B0 | B1 | floor | pixels B0 vs B1 |
|---|---|---|---|---|---|---|
| `moon-surface-200m` | 45.38 | 8.31 | 19.9 | 30 | 25 | 53 percent at 1 level, max 4 |
| `sahara-noon-ground` | 31.37 | 5.32 | 24 | 30 | 20 | 36 percent, mean 1.38, max 14, sky rows 0.00 |
| `blue-marble-12000km` | 4.99 | 1.08 | 30 | 30 | 30 | 18 percent, mean 2.8; the pixels over 32 are star and constellation dots, the disc carries only sparse 1-level dither |
| `limb-400km` | 63.45 | 13.25 | 10.9 | 24.1 | 25 | 26 percent, mean 1.16; 43 pixels over 32, all limb-edge stars |
| `fuji-forest-ground` | 85.33 | 69.06 | 10.9 | 12 | 9 | ground and sky black; every tree and its cast shadow differs, because the sway runs on a live clock (`sin(t * sway_hz + phase)`, `00-bindings-vertex.wgsl`) and the two captures are seconds apart |

30 fps is the vsync cap (60 Hz, every second vblank). Zero panics in all
four boots of the increment, and the B1 log shows both modules booted from
disk with the override declarations present. That the constants actually
BOUND is evidenced by the measurement itself, not by any log line: naga
substitutes an override only when the module declares it (a module with no
overrides comes back unchanged), and wgpu 24 raises no error for a constant
key the module does not declare, so a source whose switches were ignored
would have left every branch reachable and read B0's numbers, where B1 reads
the phase-A stub numbers. The shipped exe compiles the same embedded parts
(`PBR_PARTS`) the disk boot read. Since the review of this increment,
`validate_wgsl` (the gate the hot reload, the from-disk boot and the
embedded test all pass through) also REFUSES any megashader that lacks one
of the three `override HAS_*_BRANCH: bool = true;` declarations, naming the
missing switch. The shape it exists for is a tree from before the
permutation, with no switch declared and no guard using one (a stale
checkout under `HUMANITY_SHADERS_FROM_DISK`, a `--shipped-assets` mirror
that predates P1): naga accepts that source, wgpu binds the constants to
nothing, and it would boot, render correctly and silently restore the 45 ms
terrain cost. (Deleting `05-overrides.wgsl` alone is already a parse error,
since the guards then name undefined identifiers.)
Fuji's costs-file `frame_ms` sample read 109 against 97, but probe-sweep
copies that file immediately after the screenshot readback (perf-drive waits
3 s), so the GPU timestamp (down 16 ms) and the fps ring (10.9 to 12) are the
readings to trust there.

The one gate not met: the limb is at 24.1 fps against a floor of 25 (it was
10.9 before this change, so the floor was already missed at these settings).
Its remaining cost is `gpu.celestial_t` 26 ms, the atmosphere shell drawn
through the classic transparent PSO, which this increment deliberately left
with every branch on because the cloud shell draws through the same
pipeline. Phase A proved that shell drops 50x once the cloud branch is gone.

**Next increment, P2: a shell permutation of the transparent draw.** Give the
atmosphere shell (type 14) and the water shell (type 16) a transparent PSO
compiled with `HAS_CLOUD_BRANCH` off and route only the cloud shell (type 15)
through the one that keeps it; the registry, the switch and the test already
exist, so it is one more `PSO_DEAD_BRANCHES` row, one more pipeline field,
and a draw-list split at `mod.rs:4714` onward. Cost keys that must move:
`gpu.celestial_t` at `limb-400km` (26 ms) and `sahara-noon-ground` (10.4
ms), toward the 0.2 ms the stub measured. Then the same for the opaque
classic PSO once the interior draw lists prove no shell goes through it,
which is what re-prices the console-room wall pixel in section 2.

### (d) Instrumentation this cost centre needs

The celestial pass is one timestamp pair covering bodies, terrain, near trees
and grass together, and one render pass can carry only one
`beginning_of_pass_write_index` / `end_of_pass_write_index`
(`src/renderer/frame_costs.rs:600` to `604`). `MAX_TIMED_PASSES = 48`
(`frame_costs.rs:524`) with 22 ids in use, so there are 26 free slots.

Split `mod.rs:4523` into three consecutive render passes over the same
attachments with `LoadOp::Load` and `StoreOp::Store` on colour and depth (the
pass already loads colour; only the first keeps `Clear(0.0)` on depth). On a
desktop GPU with no tiler this costs pass setup and nothing else. New ids:
`gpu.celestial_bodies` (the classic loop, 4570 to 4595, where near-tree
models land), `gpu.celestial_patches` (4606 to 4640), `gpu.celestial_grass`
(4653 to 4676). That alone splits the Fuji 144.7 ms into its three real
components and ends the current situation where "near-tree models cost 68 ms"
is an inference from a config bisect rather than a reading.

Also missing: `gpu.sky_view` (`src/renderer/sky_view.rs`, the LUT pass,
`timestamp_writes: None`). Registry: `data/performance/budget_systems.ron` has
no row whose `sources` lists `gpu.celestial_t` or the four cloud keys, so
46.5 ms of ocean at `ocean-grazing-calm` lands in the Performance page's
"Elsewhere" remainder. Add a Water row and a Clouds row.

## 2. Cost centre two: the interior opaque pass, `gpu.scene`

### (a) Mechanism as read, and two suspects refuted

`Renderer::render_scene_onto` (`src/renderer/mod.rs:2771`) opens one pass timed
`gpu.scene` (2793) and iterates `objects` in list order (2823 to 2855):
dynamic-offset bind, vertex buffer, index buffer, `draw_indexed`, material
binds skipped when unchanged. There is no sort and no culling of any kind at
this site. The list is built in `src/lib.rs` from `state.homestead_floors`
(7266), the wall / trim / mirror shell (7401), the ceiling (7408 to 7414), the
hull when `show_hull` (7421), material walls (7435), windows (7450) and the
machines, all unconditionally, for a homestead the run log reports as
`Homestead: 24 rooms, 3 floors, walls: false, 211 lights` with `[Screens] 6
in-world screen(s) placed`.

**Refuted suspect 1: discard defeating early-z.** At 0.3 m from the wall
screen, with the screen quad filling the view, `gpu.transparent` reads 0.01
ms, down from 13.72 at 2.2 m (`c6-wallfill-0m3-costs.json` against
`c6-control-2m2-costs.json`). The glass ceiling was rejected before shading,
by a pipeline whose fragment entry is the same `fs_main` that contains the
Bayer `discard` at `90-fragment-main.wgsl:868` and `870`. Early depth
rejection is working. Do not spend an increment on moving `discard` to a
masked variant on this evidence.

**Refuted suspect 2: heavy overdraw.** Derived from the +200-light probe rather
than assumed. With k = 47.7 ps per light per pixel fixed by the two passes
jointly (section 0), the scene pass's shaded area comes out at 3.55 Mpx, one
screen, one layer. That is what the front-to-back accident of the authored
object order plus working early-z buys. The room at 2.2 m (97.7 ms total),
outside at 6 m (70.5) and at 0.3 m (161.3) all move with coverage rather than
with the object count.

**What the per-pixel cost is made of**, best current split for
`console-face-6`, 82.84 ms over one 3.55 Mpx layer = 23.3 ns per pixel (the
45 ns figure in the measurement divides the 160 ms wall-fill frame by one
screen; that frame draws the room AND the fullscreen quad, so 23.3 is the
per-layer rate and 17.5 the isolated type-24 layer):

| term | ns per pixel | how known |
|---|---|---|
| 211-light untiled loop | 2.25 | `lights_tiled` 1 vs 0 saves 8.0 ms, measured in one boot |
| one screen texture fetch (type 24 only) | about 0.1 | one `textureSampleGrad`, `90-fragment-main.wgsl:993` |
| sun + fill `evaluate_light`, ambient, ACES, material branch | under 0.5 | instruction count against the godray pass's measured tap rate |
| `sun_shadow_offset` 3x3 PCF where in box | under 0.3 | 9 comparison taps |
| `sky_ambient`, `aerial_apply`, `underwater_apply` | 0 | all three early-return for interiors (`sky_ambient` at 79, aerial at 122; `gpu.ssao` at 0.01 to 0.02 ms in the room confirms the interior pays no post terms) |
| residual | about 20 | not attributed; H1 |

Note the type-24 path specifically: the 211-light loop at
`90-fragment-main.wgsl:2092` runs for screen fragments, and then line 2185
throws the entire lit result away (`if (screen_emitter) { color = albedo *
max(emissive_strength, 0.0); }`). That is a real, findable waste of 2.25 ns
per pixel on every screen pixel in the game, cheap to fix by hoisting the
`screen_emitter` test above the loop. It is 10 percent of the problem, not
the problem.

Two more things read in the code that the measurement is consistent with:

- The camera screen's re-render is invisible. It goes through the same
  `render_scene_onto` / `render_transparent_onto` with the same ids, and
  `publish_gpu_frame` sums same-id samples (`frame_costs.rs:195` to `201`).
  The measured 16.9 ms it costs when its wall is in frame is hidden inside
  `gpu.scene`.
- The light tiler is a 2D screen binner, not a cluster.
  `src/renderer/light_tiles.rs`: 16 x 9 tiles (17, 18), `TILE_CAP = 64` (21),
  CPU-side, no depth slicing, and a light whose influence sphere contains the
  camera is conservatively pushed into all 144 tiles (62 to 68). In a room
  where the player stands inside several bulbs' ranges that path floods the
  lists and the cap at line 48 silently drops lights. That is why tiling saves
  only 8 of 82.8 ms, and why `lights_tiled` is off in the operator's config: a
  binner that drops lights past 64 is a look risk, not just a perf knob.

### (b) The 2030 technique

Three things, in this order.

1. The permutation from section 0, which is the same fix. An interior wall
   must not compile the cloud raymarcher.
2. Clustered forward shading, the froxel grid (Olsson, Billeter and Assarsson,
   "Clustered Deferred and Forward Shading", HPG 2012), replacing the 2D
   binner: a 3D grid over the view frustum with depth slices on an exponential
   distribution, built per frame, so a light is assigned only to the froxels
   it actually reaches instead of to a screen column that spans the whole
   depth range. This removes the camera-inside-sphere all-tiles path, removes
   the drop-past-64 correctness hazard, and makes tiling safe to turn on by
   default. Build it on the GPU in a compute pass once the CPU binner's cost
   shows up; at 412 lights the CPU binner is 0.05 ms (`cpu.lights`), so CPU is
   fine for now and the geometry is what needs fixing.
3. Frustum and portal culling of the interior draw list, which does not exist
   at all today. `show_hull` defaults true and the hull, all 24 rooms' shells
   and the machines draw every frame from inside one room. A depth prepass is
   the standard partner, but the evidence says early-z already gives roughly
   one layer, so a prepass here would buy vertex and draw submission, not fill.
   Rank it after culling.

### (c) First increment (one worktree agent)

**Increment I1: give the interior a real vantage, its own scopes, and the
screen-emitter hoist.** Deliberately small and instrumental, because the
interior's headline number belongs to increment P1 and measuring it twice is
waste.

- `console-face-6` and `console-face-3` do not exist in
  `tests/visual/vantages.json` (343 vantages, 48 with `perf_floor_fps`, range 5
  to 30). They were driven ad hoc through the dev IPC. Add both as real
  vantages with `expect` and `regressions` lines describing the six live
  screens, and a `perf_floor_fps` of 10, where they measure today. Without
  this the interior arc has no repeatable fixture.
- Give the camera screen's re-render its own ids by passing an id parameter
  into `render_scene_onto` (`mod.rs:2771`) and `render_transparent_onto`
  (`mod.rs:2865`): `gpu.screen_scene`, `gpu.screen_transparent`. Cost key that
  must appear: about 16.9 ms at `console-face-6` facing the west wall, moving
  out of `gpu.scene` into `gpu.screen_scene`; `gpu.scene + gpu.screen_scene`
  must sum to the old `gpu.scene` within the rig repeat floor.
- Hoist the `screen_emitter` test above the point-light loop at
  `90-fragment-main.wgsl:2092`. Cost key that must move: `gpu.scene` at the
  0.3 m wall-fill cell, 160.1 ms down by about 8 ms.
- Negative proof: the type-24 result at 2185 is already `albedo *
  emissive_strength` with the lit result discarded, so the hoist is a provable
  no-op on the image. The `console-face-6` capture must be pixel-identical,
  and the pages on all six screens must still render (`just verify-screens`).

Rig recipe: `node scripts/verify-screens.js` for the screen content gate, plus
`node scripts/probe-sweep.js --operator-config --only
console-face-6,console-face-3,home-clock-noon`. `home-clock-noon` carries no
`perf_floor_fps`, so `perf-report.js` prints a dash for it; a dip there is
worth investigating and is not a reportable regression.

### (d) Instrumentation this cost centre needs

- `cpu.frame_total`: a `frame_costs::stage` spanning the
  `WindowEvent::RedrawRequested` arm at `src/lib.rs:3150`. At `home-clock-noon`
  the frame is 33.5 ms with 19.5 ms of GPU and 8.5 ms of instrumented CPU; 13
  to 20 ms per frame is unaccounted at every light vantage and nothing can say
  whether it is work or waiting.
- `cpu.present_wait`: a stage around `self.surface.get_current_texture()`
  (`mod.rs:2633`, `2681`, `5271`, `5292`). With vsync on and `frame_ms`
  quantised to 8.33 ms, this is the only way to tell an egui or ECS stall from
  a vblank wait.
- `gpu.screen_surface` at `src/gui/screen_surface.rs:1044`, currently
  `timestamp_writes: None`. `gpu.view_star` at `src/engine/ipc.rs:1082`, also
  untimed. `gpu.particle_sim` for the GPU particle compute pass in
  `src/renderer/particles_gpu.rs` (compute passes take
  `ComputePassTimestampWrites`, a different type from the render-pass one at
  `frame_costs.rs:592`, so this needs a second helper).
- `cpu.patch_select` and `cpu.mesh_upload` are listed in
  `data/performance/budget_systems.ron` and recorded by nothing. Wire them or
  delete the rows; a registry entry that can never move is a check that
  cannot fail.
- `cpu.chunk_veg_and_draws` (0.5 to 7.7 ms at Fuji) is still a bucket, per the
  comment at `src/lib.rs:9883`. Split it at least into harvest, card emission
  and draw building.

## 3. Cost centre three: vegetation and the water shell

### (a) Mechanism as read

**Near-tree models, 68 ms of 156 at Fuji.** `src/lib.rs:10229` onward. The
comment at 10197 states the asset fact: the photoscans are 120 to 190k
triangles each. `near_tree_budget` is 260 (operator config) and it caps DRAWN
trees, not harvested ones; the log reads `[NearTree] recompute: 516 trees
within 460 m`. Each drawn tree emits two `RenderObject`s (stem plus `_bark` or
`:wood`, the `suffixes` loop at 10329) and a procedural species emits up to
four more cluster-card layers (10353 to 10381). So 260 trees is up to 1,560
draws and, at 150k triangles a pair, on the order of 39 million triangles
submitted into `gpu.celestial` every frame, and again into `gpu.shadow` for
the casters. Two properties of that loop matter more than its size:

1. There is no LOD ladder. The mesh chosen at
   `decoration_mesh_cache.get(&key)` (10333) is the same mesh at 3 m and at
   400 m. A photoscan subtending twelve pixels still rasterises 150k
   triangles.
2. There is no frustum test. The only rejection is `if d2 > td2 { continue; }`
   (10240), a sphere distance test against `tree_model_distance` squared. At
   fov 90.05 and 2560 x 1387 the view frustum subtends roughly one eighth of
   the sphere around the player, so on the order of 80 percent of the
   near-tree geometry submitted at Fuji is behind or beside the camera.

The measurement is consistent: `tree_model_distance 0` removes 67.5 ms and the
whole `NearTree` harvest with it, and nothing else in the Fuji bisect comes
close.

**Cluster cards, about 38 ms at Fuji.** Type 21, `90-fragment-main.wgsl:1000`
to `1085`: one `textureSampleGrad` on the baked atlas, an alpha cutout at
1024, `crown_depth_shade`, a cuticle-sheen roughness override, then the shared
tail. Alpha-cutout quads at forest density are a fill-bound stack; the figure
is the residual of `tree_density 0 + grass_density 0` (-55.5) minus the grass
part (-17.3).

**Grass, 17.3 ms.** `mod.rs:4653` to `4676`: ONE instanced draw of the shared
tiller mesh for 83.6k strands, 7.5 M triangles, on the opaque pipeline after
the ground so most buried fragments are z-rejected. This is already the 2030
technique. 17.3 ms for 7.5 M triangles is 2.3 ns per triangle, a normal
number for a full vertex stage plus tiny fragments. The only thing wrong with
grass is that `grass_far_m` clamps to 13 m at the low end (`GRASS_MID_M + 1`),
so "grass off" is not reachable from Settings.

**The water shell, 42 to 49 ms at low ocean eyes.** `[WaterDiag] draws=1024
... budget=1024 sat=true`, hitting `WATER_MAX_LEAVES = 1024`
(`src/terrain/planet_chunks.rs:2469`; `MAX_CHUNK_LEAVES = 640` at line 154 is
the land patch cap, a different number). Those 1024 patches are drawn in the
transparent pass at `mod.rs:4775` to `4825`, on `overlay_pipeline` when
`water_depth_write` is set, which is alpha blend, `cull_mode: None`, depth
write TRUE (`pipeline.rs:955` to `959`). The list is sorted only so that water
sinks to the end of the transparent list (`src/lib.rs:18333`, stable); within
the water set the order is the LOD heap's, uncorrelated with distance, which
the v0.1060 comment at `mod.rs:4749` to `4772` says explicitly. The fragment
is `ocean_shell` (`90-fragment-main.wgsl:386` to `832`, about 450 lines with
FFT tile sampling, `water_shade`, the sky LUT mirror and Fresnel).

Two consequences, both visible in the measurement. With depth write on and an
arbitrary order, a far patch drawn first paints and a near patch then blends
over it, so the pixel is shaded twice; drawn the other way round the far one
is rejected. Average overdraw is the harmonic-series expectation over the
overlap depth, and the result is order dependent wherever alpha is below 1
(the code says 0.93 to 1.0). And `cull_mode: None` on a closed shell
rasterises the underside triangles of every patch. The measurement agrees
that this is geometry and depth, not shading parameters: sea state pinned
glassy moves `gpu.celestial_t` 48.3 to 47.3, `water_fft off` moves 1.0, but
`water_detail_depth` (the mesh depth cap) moves 5.8.

### (b) The 2030 technique

- Near trees: an LOD ladder plus octahedral impostors. Three decimated levels
  per species (the standard 100 / 25 / 6 percent triangle chain) chosen by
  projected screen height, then a baked octahedral impostor beyond the
  distance where a billboard is indistinguishable from the mesh. The repo
  already has the impostor machinery: `src/renderer/billboard_bake.rs` bakes
  the tree atlas and the cluster cards, and `tree_card_hide_m` is the existing
  handoff radius. What is missing is the middle of the ladder: today it is
  150k triangles or a flat card, with nothing between.
- Frustum culling before the budget, which is a bug fix more than an
  optimisation, with one trap. The budget must keep being spent nearest-first,
  but `ModelCoverage` must keep being fed by the UNCULLED set, because
  `cov.hide_radius_m()` is a promise to the terrain shader's card discard
  (`90-fragment-main.wgsl:1401` and `1467`). This exact coupling has broken
  three times: v0.995 used the view-culled draw count as the proxy, v0.1107
  used the budget-th tree, v0.1110.1 used the farthest drawn tree; the stories
  are in `docs/BUGS.md`. Cull the DRAW, never the coverage arithmetic.
- Water: a depth prepass, then one shaded pass. Draw the 1024 water patches
  depth-only with `fragment: None` in any order (the engine already has such a
  PSO shape at `pipeline.rs:988` to `997`), then draw them again with
  `depth_compare: Equal` and `depth_write_enabled: false`. Exactly one
  `ocean_shell` invocation per pixel, the nearest one, deterministically.
  Cheaper and more correct than today's heap-order blend. Set `cull_mode:
  Back` on the shaded pass.
- Grass is already right. Leave it alone except for the Settings clamp.

### (c) First increment (one worktree agent)

**Increment V1: frustum-cull the near-tree draw list, coverage arithmetic
untouched.** Chosen first among the vegetation work because it is the only
one of the four that removes work which by construction contributes no
pixels, so its negative proof is exact rather than perceptual.

- Change: in the loop at `src/lib.rs:10229`, after the `d2 > td2` range test
  and before the budget test, reject trees whose bounding sphere (position,
  plus the species height as radius) is outside the camera frustum. Keep
  `cov.uncovered(d2)` / `cov.drew(d2)` fed exactly as today from the pre-cull
  decision, so `hide_radius_m()` is bit-identical. Spend the 260 budget on
  visible trees only.
- Vantage: `fuji-forest-ground` (floor 9).
- Cost key that must move: `gpu.celestial` 144.7 ms, expected down 30 to
  50 ms. The frustum holds roughly one eighth of the sphere, so most of the
  67.5 ms attributable to near-tree models is off-screen work; it will not be
  the full 67.5 because the budget will now be filled with visible trees that
  were previously not drawn at all. If `gpu.celestial` falls and the count of
  drawn trees stays at the budget, that is the expected result; if the count
  falls too, the budget is not being refilled and the change is wrong.
- Negative proof, three parts. First, `state.renderer.tree_card_hide_m` must
  be identical frame for frame before and after (log it at the existing 1 Hz
  handoff diag at `lib.rs:10412` onward and diff). Second, the
  `fuji-forest-ground` capture must be pixel-identical: culled trees are
  off-screen, and the card-hide radius is unchanged, so no card may appear or
  disappear. Third, the vantage's `regressions` list must still hold. A pixel
  diff mean above the rig repeat floor at this vantage means the coverage
  arithmetic moved and the increment is red.
- Second cell for the turning case: capture `fuji-forest-ground` twice with a
  small heading change between them and check the card-hide radius does not
  oscillate. A frustum cull that leaks into coverage shows up as cards popping
  when you turn, which is precisely the failure mode of the three previous
  attempts.

Rig recipe:

```bash
node scripts/probe-sweep.js --operator-config --only fuji-forest-ground,fuji-grass-underfoot,silverdale-osm-ground --exe <worktree>/target/release/HumanityOS.exe --out <scratch>/v1-<arm>
```

`silverdale-osm-ground` (floor 10, 139 near trees) is the low-density control:
it must not get slower.

**Increment W1 (queued behind V1, same shape):** the water depth prepass.
Vantage `ocean-grazing-calm` (floor 20) and `ocean-storm-low` (floor 18); cost
key `gpu.celestial_t`, 46.5 and 48.6 ms, expected to fall to roughly the
single-layer cost of `ocean_shell`. This one is NOT a provable no-op: it
replaces an order-dependent blend with a deterministic nearest-fragment
result, which will differ on pixels whose alpha is below 1. Report it as a
fidelity change with a pixel diff and let the operator judge, rather than
folding it into a perf claim. Do the backface cull in the same increment.

### (d) Instrumentation this cost centre needs

- The three-way celestial split from section 1(d) is the prerequisite:
  without `gpu.celestial_bodies` there is no key that reads the near-tree cost
  directly, and V1's gate is a config bisect rather than a reading.
- A `[NearTree]` counter line carrying harvested / in-range / frustum-passed /
  drawn, at the existing 1 Hz cadence. The current `recompute` line prints
  every frame while parked (84 lines in boot 1), which is a log flood and,
  worse, a per-frame recompute worth chasing on its own.
- A `gpu.celestial_t` registry row so the ocean stops landing in the
  Performance page's remainder.

## 4. Build order, ranked by milliseconds saved per unit of risk

1. **P1, the megashader permutation.** Reach: it is the floor under both
   `gpu.celestial` (44.5 ms at the moon, 42.9 at Sahara, 61 at the limb, 144.7
   at Fuji) and `gpu.scene` (82.8 in the console room), so it is the only item
   on this list that can move every vantage in the game at once. Risk: the
   lowest of the three, because phase A is a throwaway build whose negative
   proof is exact (the moon draws none of the three stubbed branches) and
   whose failure mode is "the number did not move", which costs one boot and
   rules out an entire hypothesis. Even under H2 the increment is worth its
   cost, because it converts the largest unexplained number in the report
   into a decided question. Do this first, alone, before anything else
   touches the shader.
2. **V1, near-tree frustum culling.** Reach: 30 to 50 ms at the forest
   vantages, the worst frames measured (156 and 175 ms). Risk: moderate and
   well understood, because the trap is named, dated and has broken three
   times, so the gate is written against it. Second rather than first only
   because P1 may change what a tree fragment costs, and measuring V1 against
   a moving floor wastes the measurement.
3. **I1, interior instrumentation plus the screen-emitter hoist.** Reach:
   about 8 ms directly, plus it is the prerequisite for every later interior
   claim (two real vantages, and the camera screen's 16.9 ms pulled out of
   `gpu.scene`). Risk: near zero; the hoist is provably a no-op and the rest
   is measurement.
4. **W1, the water depth prepass plus backface cull.** Reach: a large fraction
   of 42 to 49 ms at two ocean vantages. Risk: the highest of the four,
   because it is the only increment that changes the image, and it changes it
   on the sea, the surface the operator has given the most feedback about
   (the v0.1060, v0.1055 and increment-7 notes are all operator-driven). Ship
   it with a pixel diff and a look call, never as a silent perf win.
5. **Clustered light grid, replacing `light_tiles.rs`.** Reach: 8 ms today,
   and the ability to turn `lights_tiled` on by default without the
   drop-past-64 hazard. Risk: real, because it changes lighting. Deferred
   behind all four above, and behind a decision the measurement cannot make:
   whether the interior should have hundreds of lights at all once P1 has
   repriced a wall pixel.
6. **Interior frustum and portal culling, and a near-tree LOD ladder with
   impostors.** Both larger pieces of work that want P1's answer first,
   because the right amount of geometry to cull depends on what a fragment
   costs.

Not on this list, deliberately: moving `discard` to a masked variant (refuted
in 2(a) by the transparent pass reading 0.01 ms when occluded), a depth
prepass for the interior (the light probe shows roughly one layer already),
and a G-buffer (wrong architecture for a renderer whose value is a wide
material vocabulary with alpha-cutout vegetation at planet scale).

## 5. Two defects found while reading, neither a performance item

- `vsync: false` panics deterministically, two boots of two, at the first
  settings apply after world entry: `lib.rs:19496 state.renderer.set_vsync(...)`
  leads to `mod.rs:2001-2010 surface.configure`, which fails with
  `ResizeBuffers ... 0x887A0001` then `surface configuration failed: window is
  in use` then a panic in `Surface::configure`. The operator has vsync on and
  never hits it; toggling it off in Settings should. Repro log in the
  measurement's run7-vsync-repro.
- Settings silently rewrites config values mid-run: tree and grass density 0
  become 0.1 (`TREE_DENSITY_MIN`), `water_detail_depth` 5 becomes 14
  (`src/config.rs:1400`), `grass_far_m` 0 becomes 13 (`GRASS_MID_M + 1`).
  "Vegetation off" is therefore not reachable from the GUI, which matters for
  this arc specifically, because the operator cannot reproduce the bisect that
  found vegetation is 80 percent of the worst frame.

## 6. Confidence

The design pass booted nothing; every GPU number here is the measuring
agent's, re-derived rather than re-taken. H1 is a hypothesis with a good
arithmetic fit (5.9 KB per fragment of implied traffic against a declared
3.1 KB private frame plus spills) and a documented precedent in this repo's
own record (`docs/PRIORITIES.md:1297`), but it is not proven; increment P1
phase A exists to prove or kill it in one boot, and phase B does not ship on
the strength of the reasoning alone.
