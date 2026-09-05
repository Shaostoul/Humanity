# The cloud sun-shadow cache (perf arc increment 1), interface contract

Written 2026-09-05 from the performance panel design (journal v0.1284) and the code as read that night; the two worktree implementers of v0.1286 built to this text. Kept as the design of record; update it when the cache changes.

Source: performance panel wf_de1c02dc synthesis, increment 1, adjusted to the
code as read tonight. Two implementers work in separate worktrees on DISJOINT
files; this contract is the only thing they share. No em dashes anywhere.

## What it is

Sun optical depth `tau_sun(p)` (the thing `cloud_sun_tau` returns after its
rung ladder) becomes a planet-fixed cached quantity, baked into an R16F slice
atlas over two nested 3D windows around the camera's ground point, and read
by the march with one manual-trilinear tap. Each pixel keeps rungs 0 and 1
of the ladder on its own axis (the 30 m and 57 m local self-shadow the A2
split already isolates as `g_sun_tau01`); rungs 2 to 11 move into the bake.
Beyond both windows the march uses the analytic column `g_sun_tau_col` it
already computes. Nothing view-dependent enters: the bake evaluates the same
profile density at planet-fixed lattice points, with the regime read at the
lattice point's OWN direction (BUG-074 stays dead by construction).

Dev pad bit 16 (65536.0 in `light7_color.w`, free since v0.1283) = cache ON.
Off = today's ladder, the A/B twin. Showcase key `cloud_light` "0"/"1",
F10 checkbox "Sun shadow cache (off = 12-rung ladder per pixel, for A/B)",
renderer field `cloud_light: bool`, GuiState `cloud_dev_light: bool`, the
lib.rs mirror line beside `cloud_ms`.

## Windows and atlas (Rust owns the numbers; WGSL reads them from the pads)

Two windows, each an axis-aligned box in a LOCAL planet-fixed frame:
origin = the window anchor (a point on the planet at the camera's ground
lat/lon, re-anchored with hysteresis), axes = local east `e`, local up `u`,
local north `n` at the anchor (u = normalize(anchor), e = normalize(cross(Y,
u)) with Y the planet spin axis in planet-local space (0,1,0), n = cross(u,
e)). A lattice point (i, j, k) sits at
`anchor + e * ((i + 0.5) * cell_h - half_w) + n * ((j + 0.5) * cell_h - half_w) + u * (k * cell_v + z0)`
where `half_w = nx * cell_h / 2`, `z0` = the slab base height above the
anchor's sphere radius (planet radius * rb, minus the anchor radius).

| window | nx = ny | nz | cell_h (m) | cell_v (m) | extent |
|---|---|---|---|---|---|
| fine   | 256 | 48 | 190 | 240 | 48.6 km square, 11.5 km tall |
| coarse | 128 | 24 | 760 | 480 | 97 km square, 11.5 km tall |

(The panel's 195 km coarse extent used 128 cells of 1520 m; keep 760 m and 97
km for the first cut: the coarse window only has to cover the fine window's
re-anchor hysteresis and the mid distance; beyond it the analytic column
takes over. Widen later if the diff radial profile asks for it.)

Atlas: ONE R16F texture_2d, slices side by side along x: fine slices 0..47
each 256x256, then coarse slices 0..23 each 128x128, packed as
`fine: x = k * 256 + i, y = j` (width 48 * 256 = 12288) and
`coarse: x = 12288 + k * 128 + i, y = j`. Height 256. Total 15360 x 256
R16F = 7.9 MB. (A 2D atlas so it rides the EXISTING group 3 binding 0
`albedo_texture: texture_2d<f32>` through `build_albedo_group_from_view`
(materials.rs 192), the way the octa map did: NO bind-group-layout change.)
Sampler: the group's own `albedo_sampler` is linear; the WGSL side does the
trilinear itself (two bilinear taps on adjacent k slices, lerp in k); it must
clamp i, j taps inside a slice (never bleed into the neighbour slice: sample
at texel centres with coordinates clamped to [0.5, n-0.5] per slice).

Stored value: `tau_far` = the ladder's optical depth from rung 2 onward
(`cloud_sun_tau_far`), starting 87 m sunward of the lattice point, clamped to
[0, 64], as f16.

## Pads (camera uniform, written per frame by Rust in f32 after f64 math)

Unread today by every shader (grep verified): `light3_color` at byte offset
256 and `light4_color` at 272.

`light3_color` = (fine_anchor_x, fine_anchor_y, fine_anchor_z, fine_cell_h)
`light4_color` = (coarse_anchor_x, coarse_anchor_y, coarse_anchor_z, coarse_cell_h)

where anchor_xyz is the window anchor in the SAME planet-local frame `p` the
march uses (the frame of `ro` and `p` in `cloud_march_core`: planet-centred,
planet-local, the unit is the drawn-shell unit, 1 unit = `g_cloud_upkm` km
... use exactly the units of `p` in `cloud_density_hi`; the Rust side gets
them from the same `cloud_composite_frame` basis lib.rs already stashes, so
both sides agree by construction). cell_h is in the same unit. cell_v, nx,
nz, z0 are CONSTANTS in both languages (`CLOUD_LC_*` in WGSL, `CLOUD_LC_*`
in cloud_temporal.rs) with a unit test asserting they match by reading the
shader text (the pattern of `wgsl_reference_constants_stay_in_sync`).

Sun: the bake reads `camera.sun_direction` like the march does. The cache is
re-referenced (full re-bake) when the sun moves more than 2 degrees from the
direction it was baked with; Rust tracks that.

## Bake pass (Rust owns the pass, WGSL owns the fragment)

Fragment entry `fs_cloud_light_bake` in 45-cloud-temporal.wgsl (replacing the
dead `fs_cloud_octa`), fullscreen over the atlas target (the existing
`vs_cloud_screen` vertex entry is fine). From `@builtin(position)` derive
(window, k, i, j) by the packing above, the lattice point, then
`tau = cloud_sun_tau_far(point, sun_local, ...)` with the regime from
`cloud_regime(cloud_type_coord(normalize(point), t, seed))` and the weather
at the point's own direction. Output `vec4(tau, 0, 0, 1)`.

Time slicing: Rust sets a scissor rect covering one EIGHTH of each window's
slices per frame in a fixed order (8 frames per full refresh); on a re-anchor
or sun re-reference the window is baked fully in one frame (1 to 2 ms).

Rust: `CloudLightCache` in cloud_temporal.rs next to `CloudScreen`: the atlas
texture + view + `AlbedoBindGroup` via `build_albedo_group_from_view`,
anchors (f64), the bake phase counter, `plan(camera_ground_point_f64,
sun_dir)` returning whether a re-anchor happened (re-anchor when the ground
point leaves the inner HALF of the fine window; 8-cell hysteresis), the pad
values. `pipeline.rs`: generalize `build_cloud_screen_pipeline` over
(fragment entry, targets) and add `cloud_light_bake_pipeline` (one R16F
target, same pipeline layout). `mod.rs`: a "Cloud Light Bake Pass" with
`pass_timer("gpu.cloud_light")` before the Cloud March Pass, bound like the
march pass (groups 0..3); the MARCH pass binds the cache group at group 3
instead of `default_texture_bind_group` when the cache is on. `lib.rs`:
feed the ground point and sun each frame near the cloud fill block; a
once-a-second `[CloudLight]` log line. `ipc.rs`: `cloud_light` key.
`cloud_dev.rs`: the checkbox and a bisect channel 9 "Sun source" (map_diag 9:
fine = white, coarse = grey, analytic = dark).

## March side (WGSL)

In `cloud_sun_tau`, at the existing `i == 1` split (line ~2667): when the
cache bit is on and the sample lies inside a window,
`tau = g_sun_tau01 + light_cache_tau(p + sun_local * 87 m)` and return; the
fallback (outside both windows, or bit off) runs the ladder as today. Factor
rungs 2..11 into `cloud_sun_tau_far(p_start, sun_local, t, seed, weather_a,
reg, detail_amt, puff_amt, cell_amt, lodb)` so the bake and the fallback are
one code path. `light_cache_tau(p)`: pick fine if inside its inner 80%,
blend fine to coarse across the fine window's outer 20%, coarse to
`g_sun_tau_col` across the coarse window's outer 20% (world distance from
each anchor along e and n, and k range). The regime lookup and everything
else in the march is untouched.

## Gates (rig, half res, Ultra, clock and cover/type pinned, cache on vs off in ONE boot)

Look: masked mean within 3 levels and direct-sun channel grain not up at
operator-bm12, sc-top-3p0km, rain-26km-nadir, cumulus-closeup-ultra;
sc-inside-top mean over 170 with the in-cloud light on; the pinned-clock
DIFF image through cloud-radial-profile.js and cloud-lum-bands.js shows no
ring at either window edge (the Sun-source channel shows where the edges
are). Prove RED first: double the cell size and confirm the direct channel
and the diff profile flag it. Performance: `gpu.cloud_screen` from the day-0
table, in-deck down 40% or more. Panics 0, relay check green, no
bind-group-layout change (count the group-3 layout entries: unchanged).

## Units of `p` (read from the code, 2026-09-05)

`cloud_march_core` builds `ro = (inv_model * vec4(camera.view_pos, 1)).xyz`
with `inv_model = transpose(obj_normal_matrix())` of the CLOUD SHELL render
object (lib.rs ~11937: position = planet centre in the render frame,
rotation = the planet basis, scale = `visual_scale * shell_ratio` with
`shell_ratio = slab_rt + 0.0006`, lib.rs 11690). So `p` is in the shell's
object space: origin at the planet centre, axes = the planet basis columns,
1 unit = the drawn shell radius = `visual_scale * shell_ratio` render units.
`g_cloud_upkm` is units per km in that space (set in 40-clouds.wgsl ~427 from
material params: planet radius km and the slab ratios).

Rust therefore computes a window anchor in p-units as
`anchor_p = basis_transposed * (anchor_world - center) / (visual_scale * shell_ratio)`
in f64, where `center`, `basis` come from the same values lib.rs stashes
into `cloud_composite_frame` (lib.rs 12254; note it is `None` when the
temporal map is not armed, so the cache must take its frame from the cloud
fill block directly, not from that Option), and `visual_scale`, `shell_ratio`
from the cloud fill block (lib.rs ~11690 to 11940). `cell_h` in p-units =
cell_h_m * 0.001 * upkm with upkm computed the same way the shader does (the
Rust side already mirrors g_cloud_upkm somewhere in clouds.rs; grep upkm).
A unit test: a point 1 km east of the anchor at the equator maps to
anchor_p + e * (1 km in p-units) within 1e-6 of the shader's own value
computed from the same inputs (port the two lines of WGSL into the test).


## As built (v0.1286): deviations from the contract and open questions

The two worktree implementers reported these; kept verbatim so the contract above is read WITH them.

### Rust side

```
DEVIATIONS (cumulative; 1-9 from the first report stand except as amended):
- 5 AMENDED: no z0 constant; z0 is per planet via `cloud_lc_z0_m` and `slab_rb` in the frame.
- NEW 10: sun re-reference orders no full bake (above).
- NEW 11: `CLOUD_LC_START_M` removed; the bake starts at the lattice point with rungs 0-1 at depth 0, matching the WGSL half's D1.

OPEN QUESTIONS:
1. Unchanged: the merged tree (Rust + WGSL) must be boot-verified via the rig by the orchestrator; this worktree alone panics at pipeline creation because `fs_cloud_light_bake` is not in main's shader.
2. Unchanged: coarse == fine anchor means every 13.68 km re-anchor full-bakes both windows (8x a partial frame). Measure `gpu.cloud_light` on the first cache-on capture; if the spike matters, decouple the coarse anchor (its own hysteresis) as a follow-up.
3. The 1 Hz `[CloudLight]` log does not print z0; add `slab_rb` to it if a per-planet sanity read is wanted (mod.rs, one format arg).
```

### WGSL side

```
DEVIATIONS FROM THE CONTRACT (D1-D7 unchanged from the first report; D8 updated):
D1 cloud_sun_tau_far takes the ladder origin, rungs 0-1 advance inside it (bit-identity; the contract's "+ 87 m" would double-count). D2 extra `tau_in` parameter, returns the cumulative (cap breaks at the same value). D3 light_cache_tau returns vec3 (tau_far, w_far, src). D4 coarse outer band blends to the per-pixel far ladder; beyond the coarse window the ladder runs as today; CLOUD_LC_FAR_ANALYTIC = 0.0 is the switch to the analytic column. D5 no vertical fade (clamped nearest slice). D6 smoothstep blend bands. D7 bake pins g_v2_disp_lod = CLOUD_V2_SHAPE_LOD_WORLD, g_v2_allowed = true, g_lod_jitter = 0, bisect index applied, wlod = 0. D8 (updated): the source code is 0.15 (fallback) or 0.35 (decided by rungs 0-1) whenever the cache was not read, with the bit on or off. NEW D9: `cloud_sun_tau_far` has no lodb parameter (the contract's signature sketch listed the march's parameter set; lodb was never read, v0.1264).

OPEN QUESTIONS:
Q1 (unchanged): shipped default for the far fallback - the ladder (current, CLOUD_LC_FAR_ANALYTIC = 0.0) or the analytic column (1.0)? The critic's horizon-vantage note applies: measure one horizon look before declaring the perf gate met, or set 1.0 for that measurement.
Q2 (unchanged): g_lod_jitter = 0 in the bake removes the lateral cone dither for rungs 2-11; expected invisible after trilinear filtering.
Q3 RESOLVED: names converged on CLOUD_LC_FINE_NX / CLOUD_LC_COARSE_NX (the sibling moved; I stayed). Remaining risk is only that the sibling's rename is uncommitted at the time of this report.
Q4 NEW (gate notes for whoever runs the A/B): (a) run the look gate with dev pad bit 3 (cloud_world_shape_lod) ON, otherwise cache-on and cache-off shade different displacement mips and the delta is not the cache's; (b) cache-on vs cache-off legitimately diverge where a ray crosses a regime boundary (march: one regime at the ray midpoint; bake: the lattice point's own regime, BUG-074 rule); (c) channel 9 dark now means 0.15 (outside / bit off) OR 0.35 (decided in-window) - read the value, not just "dark".
```
