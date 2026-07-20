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
