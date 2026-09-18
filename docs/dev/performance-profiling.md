# Performance Profiling and Debug Instrumentation

The permanent dev rigs for measuring the running game: overlays, the debug
file protocol, the portable probe, and the log lines that matter. Per the
forever-development rule (CLAUDE.md), none of this is launch-day fat, it is
load-bearing infrastructure. Findings from specific profiling sessions live
in dated notes (e.g. `docs/dev/performance-findings-2026-07-20.md`, written
in a parallel session as of this doc's creation).

## In-app overlays (verified in `src/lib.rs`, v0.482)

- **F2** - performance overlay: fps + a 120-frame frame-time ring buffer
  sparkline (the same buffer `screenshot_done.json` averages, so on-screen
  and scripted measurements agree).
- **F3** - network overlay.
- **F4** - system overlay.
- Toggle on press; they stack in the top-right corner. Diagnostics sampling
  only runs while an overlay is open.
- **F6** - saves a camera location bookmark to `debug/bookmarks.json`;
  restore via the camera protocol below (`{"bookmark":"bm-N"}`).

## The debug file protocol (poll files in `debug/` under the process CWD)

The render loop polls for request files each frame, consumes them, and
answers with a `*_done.json`. This is how AI agents and scripts see and
drive the live 3D game (2D egui pages have `just snapshots` instead).

| Drop this | What happens | Answer |
|-----------|--------------|--------|
| `debug/screenshot_request.json` (any content, or `{"width":3840,"height":2160}` for hi-res) | Captures the current viewport to `debug/screenshot_N.png` (N monotonic per session; hi-res renders one offscreen frame, no HUD) | `debug/screenshot_done.json`: `{"ok":true,"path":...}` **plus `fps` and `frame_ms_avg`** (mean of the 120-frame ring buffer), which is what makes A/B perf measurement a file drop |
| `debug/camera_request.json` | Places the camera: `{"body":"earth","lat":...,"lon":...,"altitude_m":...}`, or `{"view":"Oahu Coast"}` (named entries in `data/scenic_views.ron`), or `{"bookmark":"bm-N"}` (F6 bookmarks), plus `{"aim":"sun"}` to face the sun from any vantage | `debug/camera_done.json` |
| `debug/showcase_request.json` | Frames a named subject (e.g. `{"tower":"nutrition"}`) so a follow-up screenshot captures it; also drives staged time-of-day | `debug/showcase_done.json` |
| `debug/autopilot_request.json` | Drives an instance into a world with zero human input (v0.793), e.g. `{"server_url":""}` for the OFFLINE world. Guard: refuses to run against a real installed identity, run it in a portable scratch install only | `debug/autopilot_done.json` |

All requests are consumed (deleted) whether they succeed or fail; failures
report `{"ok":false,"error":...}`.

## The portable perf probe (the A/B measurement rig, v0.891+)

Proven pattern for measuring renderer changes without touching your real
install (this rig measured the 4x draw-submission win and root-caused the
terrain flicker):

1. Copy `target/release/HumanityOS.exe` into a scratch dir with an empty
   `portable.txt` beside it (portable mode: all state lives next to the exe,
   so the throwaway identity never touches your real one).
2. Junction the repo's data + assets in (PowerShell:
   `New-Item -ItemType Junction data -Target C:\Humanity\data`, same for
   `assets`), so the probe runs the repo's current content without copying.
3. Launch, then drop `debug/autopilot_request.json` with `"server_url": ""`
   (offline world, no relay noise, satisfies the identity guard).
4. Stage the shot: `debug/camera_request.json` (scenic view, bookmark, or
   lat/lon + `"aim":"sun"`), optionally showcase time-of-day.
5. Drop `debug/screenshot_request.json`; read `fps` + `frame_ms_avg` from
   `debug/screenshot_done.json`, and the PNG for the visual.
6. A/B = repeat with the other exe build in a second scratch dir, same
   requests.

Probe gotchas (journaled 2026-07-19): a fresh world's clock runs ~77x so
staged lighting expires fast; local solar noon = 12 - east_longitude/15;
scene brightness follows the global game hour while the sun's screen position
is longitude-aware.

## Frame costs: the per-pass and per-stage breakdown (`renderer::frame_costs`)

The Performance page (Platform > Performance) and the `HUMANITY_FRAME_COSTS=1`
JSON drop (`debug/frame_costs.json`, rewritten once a second while the
process runs with that variable set) read one process-global store that
the engine writes every frame:

- **`gpu.*`**: milliseconds per render or compute pass, from wgpu
  `TIMESTAMP_QUERY`, resolved one frame late (never a GPU stall). Every pass
  opens its scope through `Renderer::pass_timer("gpu.x")` (render) or
  `compute_pass_timer` (compute); passes sharing an id in one frame are
  SUMMED, and a pass that stops running decays to zero. On an adapter with
  no timestamp queries the store falls back to CPU submission time and the
  page says so (`gpu_timing: "cpu_fallback"`); a `gpu.x` row then reads
  `cpu.x`, which is why every timed pass has a same-named CPU stage.
- **`cpu.*`**: milliseconds per frame stage, `frame_costs::stage("cpu.x")`
  as a scope guard or `record_cpu` for an explicit span; several calls per
  frame sum.
- **`vram.*` / `ram.*`**: byte inventories, exact, sampled once a second
  while somebody is looking.

The registry that turns ids into pie slices is
`data/performance/budget_systems.ron`; an id that is not a row's source
folds into the pie's "Elsewhere" wedge. Two tests keep the join honest
(`src/gui/pages/performance.rs`): every registered source must be an id the
source tree actually records, and every id the source tree records must be
a row or be listed as remainder WITH A REASON. Add a scope, add a row (or a
reason), or the test names the orphan.

### Pass ids (as of 2026-09-18)

| id | pass | since |
|----|------|-------|
| `gpu.clear` | the UI-only frame's surface clear | |
| `gpu.stars` | the star sky (live frame and hi-res screenshot) | |
| `gpu.celestial` | terrain patches, planets, sky, one megashader | |
| `gpu.celestial_t` | the transparent celestial pass: ocean shells + water depth prepass | |
| `gpu.sky_view` | the distant-sky lookup table refresh (near an atmosphere only) | 2026-09-18 |
| `gpu.cloud_light`, `gpu.cloud_screen`, `gpu.cloud_resolve`, `gpu.cloud_composite` | the cloud deck in frame order | |
| `gpu.cloud_profile`, `gpu.cloud_profile_calib` | the far rung's calibration passes (only when armed) | |
| `gpu.shadow` | the sun shadow depth pass | |
| `gpu.scene`, `gpu.transparent`, `gpu.overlay`, `gpu.instanced` | the live frame's homestead/prop passes | |
| `gpu.screen_ui` | the in-world screens' egui-to-texture pass, one per framed screen, summed | 2026-09-18 |
| `gpu.screen_sky`, `gpu.screen_scene`, `gpu.screen_transparent` | the camera wall's 10 Hz re-render of the world, under its own keys (it used to sum into `gpu.scene`, so a 17 ms re-render was invisible) | 2026-09-18 |
| `gpu.particles`, `gpu.gpu_particles` | billboard and GPU particle draws | |
| `gpu.particles_sim` | the GPU particle compute step | 2026-09-18 |
| `gpu.godrays`, `gpu.ssao`, `gpu.bloom` | post-process (bloom reads zero: dormant) | |
| `gpu.lines`, `gpu.celestial_lines` | guide and orbit lines | |
| `gpu.ui` | the main egui pass | |

The hi-res screenshot renders its one extra view under the MAIN ids
(`gpu.stars`, `gpu.scene`...) on purpose: it is a one-off render of the
player's own view, its other passes are main-keyed already, and it decays
out of the pie within a second; keying it `screen_*` would misattribute a
screenshot to the camera wall. Which key a scene pass uses is the
`frame_costs::SceneView` argument of `render_scene_onto` /
`render_transparent_onto`; `render_view_onto` picks it from `ViewPasses`.

### CPU stages that are not passes

- **`cpu.frame_total`**: `RedrawRequested` entry to the end of the last
  submit (the egui pass), world frames only. UI-only frames are discarded
  at their acquire, so nothing is recorded for them.
- **`cpu.present_wait`**: around `get_current_texture` (in
  `Renderer::acquire_surface`) plus around `present()` in the frame loop,
  summed. The CPU blocked on the display: vsync and the driver's frame
  queue.
- **`cpu.fps_cap_sleep`**: the Settings frame-rate cap's deliberate sleep at
  the top of the frame. Inside `frame_total` (the timer starts at entry),
  recorded separately so it is never read as work.
- **`cpu.systems`** (the ECS tick, with `cpu.system.<slug>` per system),
  **`cpu.patch_build`**, **`cpu.near_tree_harvest`**, **`cpu.grass_harvest`**,
  **`cpu.chunk_veg_and_draws`** (a bucket that contains the two harvests;
  listed as remainder, not a row, or the harvests would count twice), the
  upload stages (`cpu.patch_upload`, `cpu.grass_upload`, `cpu.water_upload`,
  `cpu.weather_upload`, `cpu.atmo_luts`, `cpu.lights`, `cpu.light_tiles`),
  the submission twins of every pass (`cpu.scene`, `cpu.celestial`,
  `cpu.screen_ui`, `cpu.screen_scene`, `cpu.particles_sim`, ...).

### Reading the numbers

`frame_ms` is wall time between frame starts. For a world frame:

```
frame_ms  ~=  cpu.frame_total  +  cpu.present_wait(present half)  +  time outside the handler
cpu.frame_total  =  cpu.fps_cap_sleep  +  instrumented CPU stages  +  UNTIMED CPU work
```

So **`cpu.frame_total` minus the instrumented sum** (every `cpu.*` stage
except `frame_total` itself, `present_wait`'s acquire half is inside the
total too) is the CPU work that has no stage yet: egui layout of the main
UI, input, the screens' page logic outside `run_and_render`, the ECS parts
not under `cpu.systems`, draw-list assembly. When that number is large the
frame is CPU-bound in untimed code and the next step is a stage around the
suspect; when `cpu.present_wait` is large the frame is GPU- or
display-bound and the GPU pie says which pass. Before 2026-09-18 neither
number existed, and 13 to 20 ms per frame at light vantages were
unaccounted with no way to tell the two cases apart.

The `gpu.*` sum against `frame_ms` works the same way on the GPU side: the
difference is the GPU's idle time plus passes nobody has scoped. As of
2026-09-18 every pass the frame submits has a scope except the live
broadcast copy (`stream_capture.rs`) and the one-time billboard bake.

## Log-line telemetry

Logs land in `%APPDATA%/HumanityOS/logs/run.log` (portable: `logs/` next to
the exe). The panic hook always writes the cause to run.log/crash.log, read
them FIRST on any boot failure.

- **`[ChunkDiag]`** (`src/lib.rs`) - the terrain streaming heartbeat: draw
  count, max depth, budget saturation, build requests, cache size, resident
  tiles, altitude, refused-split telemetry, hot counters
  (vis-empty/budget/missing/split), max leaf error, and a depth histogram.
  This line is how the patch-cache thrash and the split/collapse oscillation
  were proven; watch it whenever touching terrain LOD.
- **`[Godray]`** - 1 Hz sun-NDC diagnostic for the god-ray pass.
- **`Planet chunks '<body>': build budget saturated (N requested, M per
  frame)`** - at most every 300 frames while more patches are requested than
  the live per-frame build budget (the `terrain_builds_per_frame` setting,
  clamped to 1..64; before 2026-09-18 the line printed the retired
  `PATCH_BUILDS_PER_FRAME` constant, 24, whatever the setting was).
- **`[NearTree] recompute (<reason>)`** - DEBUG level since 2026-09-18 (it
  was an info line firing every frame while parked). The reason is one of
  `moved`, `depth changed`, `density changed`, with the before/after values
  and the seconds since the last harvest; the gate is
  `terrain::near_tree_gate` (movement eager, the two change terms limited to
  one harvest per 0.25 s). If a parked camera still logs this several times
  a second, the reason says which input is oscillating.
- Boot cost: `debug/boot_timing.json` records startup phase timings (the
  parallel PSO compile work was measured from it).

## Where the frame budget goes (shape as of v0.901)

Rough guide for where to look first, from the 2026-07-18/19 marathon:

- **Draw submission** used to dominate at high patch counts until the v0.891
  4x batching win; regressions here show as frame_ms rising with draw count
  in ChunkDiag.
- **Terrain patch builds** are background-threaded but the cache must be big
  enough (the 256 MB cache at 6144-leaf budgets caused permanent
  build/evict/rebuild waves; now 1.5 GB + never-evict-recent).
- **The megashader PSO compiles** (~10 s each on the dev GPU) are BOOT cost,
  compiled in parallel; they do not affect steady-state frames but explain
  slow first launches.
- Full-screen passes (bloom, SSAO, god rays, clouds High) scale with
  resolution; the cloud quality setting (`Settings > Graphics`) is the big
  fragment-cost lever.
- When in doubt: two screenshot drops (feature on/off via its Settings
  toggle) give you frame_ms_avg deltas in under a minute.

## Ground rules

- Measure on the RELEASE exe (`cargo build --features native --release`);
  debug builds lie.
- Never re-run a battery that already passed on unchanged code; never assess
  a perf change without an A/B pair from the same rig.
- Keep every new diagnostic behind this protocol pattern (request file in,
  done file out) so agents can use it; document it here when you add one.
