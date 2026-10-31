# Where the CPU frame goes: FarmingSystem and the untimed 19 ms

Status: proposal, measured from reading on 2026-10-22 (perf-expert, read-only; nothing booted or
built). Line numbers are at lib.rs 16,614 lines and will drift: grep before relying on one. Every
item is a timer or a result-identical change except F4. Companion to
`docs/design/headless-sim-runner.md` (its increments 0 and 7).

**2026-10-30: built and MEASURED (the section below this one; the proposal follows it as it was).**
S1, increment 0, F1, F2, S3 and S6 are done; S5 is not worth doing.

## Measured, 2026-10-30

The frame at `home-overview-noon` with the showcase's 1,981 crops, five samples averaged, on a busy
machine (the farm system read 3.5 ms quiet earlier the same day, 7.3 ms here: compare passes with
each other, not with the 2026-10-22 numbers).

- **Increment 0 works:** the spans add up to 29.16 ms against `cpu.frame_total` 29.08 (no cap
  sleep), so the whole handler is tiled; the farm passes add up to `cpu.system.farming` exactly.
- **The spans, largest first:** world_passes 12.98 (the render passes, mostly timed already),
  ecs 7.95 (farming 7.28 of it), lists_sky 1.86, settles 1.55, mirror_player 1.52, ui_paint 0.64,
  dev_ipc 0.55, lists_world 0.41, lists_home 0.35, hot_reload 0.33, cards 0.25, the other 21 under
  0.2 each. **Next places to look:** lists_sky, settles and mirror_player (about 5 ms between them,
  with no stage inside).
- **The farm passes:** crops 3.98, pest_tally 2.00, apply 0.59, setup 0.21, pollination 0.18,
  picking 0.15, air 0.15, the rest about 0.
- **S1** (the Garden data, done first): 16-20 ms a frame, see S1 below.
- **S6 was underestimated:** one `Path::exists` on a missing file is about 60 microseconds on the
  operator's PC (Windows and its antivirus filter), so the eleven request checks cost 0.65-0.7 ms
  a frame (timed with node on the rig folder), matching the dev_ipc span. Done: one watcher on
  `debug/` (`engine/ipc.rs` `RequestWatch`).
- **S5 is not worth doing:** abilities are read by the HUD's hotbar every frame (not a page-only
  mirror), the home stock feeds the ECS tick itself, machine card stats are already gated on a
  card being shown, and skills and the herd scale with a few dozen skills and a handful of animals.
- **S3** was estimated at 1-3 ms, but every home and world list together builds in 0.76 ms
  (lists_home plus lists_world), so its share is a fraction of that; done anyway because it is
  cheap (`engine/state.rs` `DrawBufs`).
- **F1 and F2:** done (one plant table per tick with each plant's definition, stage list and the
  pests that eat it; each crop changed in place, only new soil records inserted after the loop).
  The farming golden state is bit-identical, and the test was seen red with the soil write-back
  removed.

**After S1, F1, F2, S3 and S6, on a quiet machine** (the same vantage and crops; `cpu.scene`, which
none of this touched, read 3.33 ms against 3.09 in the quiet run of the old build that afternoon,
so the two are comparable): **`cpu.frame_total` 22.4 ms before, 12.9 ms after.** FarmingSystem 3.5
-> 2.4 ms (crops 1.18, pest_tally 0.80, the apply pass gone); dev_ipc 0.15 ms (the rest of it is
the other per-frame polls); the Garden data is no longer built on world frames. The CPU now waits
on the GPU and the 30 fps background cap (`cpu.present_wait` about 20 ms). F1 and F2 gave about a
third of FarmingSystem, not the two-thirds estimated; the rest of its time is in the crop loop's
per-area lookups (the doc's per-area and per-(area, plant) tables, not built) and the pest tally's
per-crop conditions.

## What the data on disk says

- `.probe-rig/debug/frame_costs.json` (v0.1539.0): `cpu.frame_total` 34.64 ms, `frame_ms` 36.30.
  Timed CPU stages sum to 15.68 ms, so **19.0 ms is untimed**. GPU passes sum to 14.25 ms and
  `cpu.present_wait` is 0.22 ms: **the limit is the CPU main thread**, and over half of it has no
  timer.
- The rig's homestead has **1,981 crops** (`[Showcase] auto-seeded 1981 crops` in run.log), so
  FarmingSystem's 3.51 ms is 1.77 us per crop per frame.
- Chunked terrain was not running at this vantage (its stages had decayed to ~0).

## FarmingSystem (3.51 ms)

`FarmingSystem::tick` (`src/systems/farming/mod.rs:954`) walks every crop several times. The cost is
string-keyed SipHash lookups and small heap allocations repeated per crop, not arithmetic.

| Pass | Lines | Est. ms | Why |
|---|---|---|---|
| Pest tally | 1976-2043 | 0.8-1.1 | ~20 lookups per crop, 10 `pests::favour` HashSet<String> lookups |
| Main growth loop | 2056-2471 | 1.8-2.4 | 40-45 lookups (the plant registry 7 times for one crop), ~7 allocations (`def.stages()` Vec, `crop.clone()`, `area.to_string()` map keys) |
| Apply pass | 2605-2619 | 0.15-0.25 | two `world.get` per crop, old crop's Strings freed |
| Pollination, picking, weeds walks | 1631, 1657, 2047 | 0.2-0.3 | `in_flower.insert(area.to_string())` per flowering crop |
| Per-tick setup | 1056, 1068, 1184 | 0.05-0.15 | `AirMap::new` clones every room |

**F1. Per-tick lookup tables (saves ~1.5-2.2 ms, identical results).** Resolve once per plant
(definition, stages slice, growth days, light need, water, `exchange_for`, fungus CO2, windows, weed
loss, a host mask sized from `pests.ron`, never a hardcoded 64 bits), once per area (field flag,
room, `rh_for`, `known_rh`, `known_co2_ppm`, `warmed_k`, `cover_warming`, pests/weeds entries,
supply, nutrient slider, `lamp_cover`, mulch rate), once per (area, plant) (`plants_here`, need,
pest/humidity/CO2 ceilings, light rate, climate factor). All read-only during the loop. **Do NOT
cache** soil pH per area (`soil_ph::add_n` writes it inside the loop), the `organic` pools, or
`crop_soil.store`. Owned-String keys kept in `FarmingSystem` across ticks plus a last-key memo.

**F2. No allocations per crop (saves ~0.6-0.9 ms, identical).** Iterate
`query_mut::<(&mut CropInstance, Option<&mut CropSoil>)>()` and mutate in place (removes
`crop.clone()`, the `updates` Vec and the apply pass); accumulate per-area sums by area index and
build the maps once, in the same order.

**F3. The other walks (~0.1-0.3 ms).** `get` before `insert` in pollination; cache `AirMap`'s
structure until rooms or plots change; cache `light_at` per area at night; keep `need_cache` and
`fertilizer_doses` across ticks, invalidated on hot reload.

After F1-F3: FarmingSystem ~0.5-1.0 ms. **F4 (lower tick rate) changes results** (left Riemann
`health_seconds`, the 0.2 water-stress crossing, dawn/dusk light gating, feeder steps): keep it for
headless soaks only.

**Proof first:** a lib test that runs the 1,981-crop showcase ~600 ticks and snapshots the bits of
every `CropInstance`, `CropSoil` and `SoilMemory`; bit-identical after the change is the proof.
Sub-stage timers (children of `cpu.system.farming`, kept out of sums): `cpu.farm.setup` 954-1209,
`.pollination` 1631, `.picking` 1657, `.pest_tally` 1976-2043, `.weeds` 2046-2047, `.crops`
2056-2471, `.air` 2480-2554, `.apply` 2605-2619.

## Increment 0: tile the untimed 19 ms

Non-overlapping `cpu.span.*` timers (`let t = frame_costs::stage("cpu.span.x"); ... drop(t);`) so
frame_total ~= `cpu.fps_cap_sleep` + sum of spans. Spans after the acquire record only when
`!page_active` (like frame_total at lib.rs:16157). Update `docs/dev/performance-profiling.md`'s
subtraction rule and keep spans out of `data/performance/budget_systems.ron`. Overhead ~0.02 ms.

Span boundaries (lib.rs): station 3302-3553, hot_reload 3559, move_setup 3561-3632, walk_aboard
3633-3756, particles 3758-4153, house_power 4155-4318, planet_walk 4320-5253, collide_drive
5255-5451, look_env 5452-5769, bridges 5770-6659, ecs 6660-6721, copresence 6723-6877,
save_editor 6878-7388, lists_home 7390-8214, lists_fixtures 8215-8662, lists_gizmos 8663-9137,
lists_people 9138-9338, lists_sky 9339-11800, hud_voice 11802-11950, settles 11951-12589,
mirror_player 12590-12796, garden_mirror 12797-12945, mirror_world 12946-13292, env_publish
13293-13697, cards 13698-14264, dev_ipc 14266-14408, network 14409-14703, load 14704-14746, screens
14749-14790, world_passes 14791-15333 (sub-span point_lights 14885-~15040), ui_run 15336-15816,
audio 15818-15973, ui_paint 15975-16083, tail 16085-16148.

**The Garden view, part by part (quiet machine, camera on the Inventory wall screen):** building
the data 8.86 ms (the crop rows 5.91, the per-area part 0.94, the five views 1.6 together), the
page's Garden section drawing itself 4.68 ms (`cpu.garden.*`). Done the same evening: the rows are
**time-sliced** (`garden_mirror::SLICES`, every 8th crop's row a frame in rotation, every row at
once when the page was not drawn in the last half second or the reuse switch is off), and the page
**groups the crops in one pass** and reads each crop's outlook from the one map instead of copying
it (each of the 91 groups walked all 1,981 crops). Measured only on a busy machine so far (every
unchanged stage read about 2.25 times its quiet value): rows about 2.3 ms and the section about
3.4 ms in quiet terms. Less than slicing alone should give: confirm on a quiet run, and find what
in the rows is per frame rather than per row.

**A worst frame on a loaded machine is mostly the OS, not the code.** With the game below normal
priority (the share guard) on a busy PC, nearly every span showed a worst of 5-20 ms in the same
samples: the thread was paused mid-span. So the worst-frame readings need a quiet machine, and the
next instrument step is to time each span in the thread's own CPU time as well as clock time
(`QueryThreadCycleTime` on Windows), so a span's worst is the code's and a pause shows as the gap.
**Built the same night:** `thread_ms` / `thread_worst_ms` (`frame_costs::close_span`), with a test that a
sleeping span records almost no CPU time (seen red recording the clock time instead).

## Stutter: spread every spike across frames (operator, 2026-10-30)

The operator, on time-slicing the Garden data: "We should consider everywhere we should implement
this for the best MMO experience. I imagine it'll help lower end hardware a bit too. We want to
avoid stuttering." A frame that runs long is felt as a hitch even when the average is fine, and on
slower hardware the same spike is bigger.

1. **Measure the worst frame, not only the average.** Every `cpu.*` id and span is an exponential
   average, which hides a 17 ms job that runs once a second. **Built 2026-10-30:** `frame_costs`
   keeps, beside each average, the largest single-frame value over the last 5 to 10 s
   (`note_worst`, `worst`), and the JSON drop and `game_frame_costs` report both (`cpu_worst_ms`,
   `frame_ms_worst`); a span whose worst is far above its average is a periodic spike. Read it on
   a quiet machine (see above), until the spans also count thread CPU time.
2. **The known periodic or bursty work to check first:** the autosave of the offline home every
   120 s (`save_load::periodic_save_due`, written on the main thread) and the ship-structure
   autosave; the plant mesh rebuilds (the log reports their worst frame); the shader-change poll
   (once a second, a file-time read per megashader part, about 60 microseconds each on this PC);
   the hot-reload watcher's poll; the room light probes; the terrain patch builds (already given a
   per-frame budget); the Performance page's once-a-second memory walk; the request watcher's
   once-a-second listing; the chat history pump and the background server connections.
3. **Each one gets a per-frame budget** and is spread across frames (a slice of its work per frame,
   the way the terrain patch builds already are) or moved off the main thread (the way the plant
   meshes' build already is). Every slice that keeps data across frames follows CLAUDE.md's reuse
   rule: rebuilt on a schedule, and off with the Dev page's "turn off all reuse" switch.
4. **The goal is stated as a worst frame,** not an average: on the operator's PC, no frame more
   than a few milliseconds over the average in a ten-minute rig run that includes an autosave, a
   harvest, a build edit and a walk past the wall screens.

## Suspects in the untimed time, ranked

- **S1. DONE 2026-10-30 (engine/garden_mirror.rs). Measured 16-20 ms a frame, not 4-9,** with the
  showcase's 1,981 crops: `cpu.page_data.garden` with the camera parked on the Inventory wall screen
  (`{"station":"home","screen":"wall_screen_1","distance_m":2.2}`), on a busy machine (FarmingSystem
  read 5-9 ms in the same samples against 3.5 ms quiet, so perhaps half that when quiet). Every world
  frame paid it before; now only a frame that draws the Inventory page does, and with no such page
  the timer never appears. A page open on the main screen hides the world and its frames are not
  measured (`frame_costs::discard_frame`), so measure it through the wall screen. In the same
  samples the wall screen's own page draw, `cpu.screen_ui`, was 10-14 ms: S1b below is the next cut.
  The original reading follows. **The Garden mirror (lib.rs:12797-12945), est. 4-9 ms every world frame.** ~15 String
  allocations, 6-8 formats and ~20 lookups per crop (uncached `crop_season_need` through
  `harvest_item_for`, `air_view.crop_row` formats, `outlook()`), plus five view constructors that
  each re-walk every crop. Its only consumer is the Inventory page (inventory.rs:2154-2600). Fix,
  identical image: build it only when the Inventory page will be drawn this frame (active page, or an
  in-world screen sourcing `Page(Inventory)`, e.g. home.ron's `wall_screen_1`); that choice is made
  at 14762-14783, so move the mirror into a function called just before `frame_surfaces` and
  `egui_ctx.run`. Verify with `just snapshot inventory` and `just verify-screens`.
- **S1b. Inside `cpu.screen_ui` (6.23 ms):** the Inventory Garden section filters all crops once per
  group (inventory.rs:2255) and clones every `CropOutlook` per group per frame (2394). Group once,
  borrow. Confirm with a per-source id at screen_surface.rs:1101.
- **S2. The main egui pass, est. 1.5-4 ms, no CPU timer.** The HUD runs `sight_blockers.blocked()`
  before `world_to_screen` (hud.rs:562/565); project first.
- **S3. Draw-list assembly (7390-11800), est. 1-3 ms.** Seven lists built with `Vec::new()` each
  frame; keep capacity.
- **S4. Plant-mesh signature (home_meshes.rs:1091-1104), 0.1-0.2 ms, untimed.** SipHash over every
  crop before its own guard; publish a crop-change counter from FarmingSystem instead.
- **S5. Page mirrors rebuilt for pages that are not open** (skills 12673, abilities 12336, machine
  card stats 13756, `publish_home_stock` triple clone, `herd_rows` 6682): gate on visibility.
- **S6. Dev IPC polls, 0.05-0.4 ms:** ~11 `Path::exists` per frame (ipc.rs:548); one `notify`
  watcher on `debug/` instead.

## Order of work

1. Timers (`cpu.span.*`, `cpu.farm.*`) and the farming golden-state test.
2. F1 + F2, then the S1 gate.
3. `just perf-sweep` against the v0.1539 baseline with `just perf-diff`, plus `just verify-screens`.
