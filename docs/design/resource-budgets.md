# Resource budgets: allocate your machine

Status: DESIGN (2026-08-01, operator idea, verbatim below). Increment 1 dispatched.

> "Pull the CPU/GPU data and have like a 'GPU bar' where there's a section for
> skybox, one for textures, grass, trees, particles, whatever. I allocate 10%
> to this and 20% to that, and the game performs within expected parameters.
> Do that with CPU and RAM too. Pie charts - each hand is in reference to
> another. Lock certain ones at a base. Add 'extra' for rain since they have
> the G-RAM. Single stacking the pie charts on mobile; on PC it keeps the
> widget small."

## Why this is the right shape

- Settings today are N independent sliders whose COST the user cannot see.
  A budget pie shows where the frame actually goes and makes the trade
  explicit: more grass IS less rain, on your hardware, visibly.
- Accessibility: "give trees a quarter of my graphics card" is understandable
  by someone who has never heard of a draw call. GUI-first norm satisfied by
  construction.
- It generalizes: GPU time, CPU time, RAM, VRAM are four pies over the same
  registry of systems.

## Architecture (three increments)

### Increment 1 - MEASURE (dispatched)

You cannot allocate what you cannot measure. Per-system live cost:

- GPU: wgpu TIMESTAMP_QUERY around each pass family (terrain batch, celestial
  objects, water, clouds/sky, particles, shadow pass, bloom/post, egui).
  Timestamp queries are cheap and supported on the primary adapter; feature-
  gate off when absent (downlevel), pie falls back to CPU-side pass timing.
- CPU: per-System tick times already exist in the SystemRunner; add the
  frame-loop stages (harvest, patch build, upload) via the existing
  boot_timer pattern.
- VRAM: an allocation inventory - patch arena (exact), texture pools (sum of
  known uploads), particle pool, mesh buffers. Not driver-perfect; honest
  about being "tracked allocations", displayed with a "driver overhead" slice.
- RAM: process working set (sysinfo-free: read from the OS directly).

Surface: a PIE WIDGET (new universal widget, src/gui/widgets/pie.rs) on the
Performance page: four pies (GPU / CPU / VRAM / RAM), each slice a system,
live-updating. Mobile/narrow layout stacks them vertically; wide shows a 2x2
grid. Read-only in this increment - it is ALREADY worth shipping as dev
tooling (which is permanent infrastructure per the forever-dev norm) and as
the "where does my machine go" answer for players.

Registry: data/performance/budget_systems.ron - id, display name, category,
which timestamps/counters feed it, LOCKED flag, base floor. Infinite-of-X:
adding a system to the pie is a data row.

### Increment 2 - ALLOCATE

Slices become draggable (egui pointer math on the pie; also plain numeric
entry for accessibility). An allocation is a CEILING as a fraction of frame
budget (the user also picks a target: 30/60/120 fps). Locked slices (UI,
world core) have floors. Allocations persist in AppConfig; the settings
round-trip lint applies.

### Increment 3 - GOVERN

A feedback controller per system: when a system's measured cost exceeds its
allocation for N consecutive seconds, its quality knobs step DOWN (each
registry row names its knobs and their step order - e.g. grass: density then
FAR_M then blades-per-tiller); under-budget systems with "extra allowed" may
step UP to spend headroom ("extra for rain since they have the G-RAM").
Hysteresis so it never oscillates; every change logged and visible in the
pie ("grass stepped down: over budget 3s"). The controller is the piece that
makes "the game performs within expected parameters" TRUE rather than hoped.

## Honest constraints

- Slice-to-knob calibration is per-hardware; the controller (inc 3) sidesteps
  needing a calibration table by measuring and stepping, not predicting.
- GPU timestamps measure PASSES; a system spread across passes (trees are in
  terrain batch AND shadow AND celestial) needs its slices summed - the
  registry maps timestamps to systems many-to-one.
- Increment 2 without 3 is honest sliders with a better UI; say so in the UI
  ("allocations enforce when the governor ships").

## Web mirror

Native egui pie first (canonical), then a matching CSS/SVG pie for the web
Performance page reading the same registry via the relay stats endpoint.
