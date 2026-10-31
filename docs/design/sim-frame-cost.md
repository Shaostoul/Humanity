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
headless soaks only. (2026-10-30: docs/design/farm-clock.md rung 1 made those four exact, and found
more; the `health_seconds` sum was in fact a right one, the end health.)

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

### Survey 1, 2026-10-31 (a busy machine: an agent was compiling throughout)

A rig at home-overview-noon, clock released to normal speed, a 30 s walk past the wall screens at
the start, then ten minutes standing; `game_frame_costs` every 5 s (each sample's worst is the
worst frame of the last 5 to 10 s), then a minute walking back past the screens every 2 s. The
background rig runs at 30 fps. With the machine busy, wall-clock worsts are inflated by preemption
and only the spans' own thread CPU time (`thread_worst_ms`) is trusted below; the "few ms" verdict
needs a quiet re-run.

| What | Average | Worst (own CPU) | When | Verdict |
|---|---|---|---|---|
| **Wall screens** (`cpu.span.screens`, nearly all `cpu.screen_ui`) | 0 out of view; **5 to 11 ms a frame while four page screens are in view** | 35 to 50 ms the first time, 10 to 16 ms on every later return | every frame they are in view | **The biggest.** Each of up to four page screens re-ran its whole page every frame. FIXED 2026-10-31: page screens take turns, one a frame (`screens::PAGE_FRAMES_PER_TICK`, `page_turns`); the hovered screen, a dev-IPC target and video/camera/web screens still draw every frame |
| The Inventory wall screen's Garden data (`cpu.page_data.garden`) | small | 10.3 ms (13.6 the first time) | each time that screen comes back into view after half a second away | Every row was rebuilt at once. FIXED 2026-10-31: from a wall screen it keeps rotating through its rows (`garden_mirror::build_for_wall_screen`, `plan`); the main page still never opens on old rows |
| The home autosave (`cpu.span.save_editor`) | 0 | 2.3 to 3.2 ms | every 120 s (t = 20, 140, 260, 381, 501) | Under the goal; a candidate for writing off the main thread later |
| Plant mesh rebuilds (`cpu.span.hot_reload`, `cpu.plant_rebuild`) | 0.2 ms | 2 to 4.3 ms | 16 windows in ten minutes, irregular | The main-thread part of the rebuild; measure on a quiet machine before slicing |
| The dev IPC (`cpu.span.dev_ipc`) | 0.16 ms | 1.7 to 2.2 ms | each `game_frame_costs` request (the survey's own) | Dev tooling measuring itself; only when a tool asks |
| The world passes and the ECS (`world_passes`, `ecs`) | 6.8 and 4.0 ms | window worsts about twice the average in EVERY window (13 to 17 and 7 to 9 ms) | always | Not periodic: the spread of a loaded machine (shared cores slow the thread itself). Re-read quietly |

**After the two fixes** (the same walk back past the screens, the same busy machine, sampled every
2 s): the screens' own CPU while in view averaged **2.4 to 2.9 ms a frame, from 5.9** (the same
window at 6 s: 3.2 from 8.7), and the Inventory screen's cold Garden rows **1.9 to 2.8 ms, from
7.2**. The WORST frame did not move (16.4 to 19.9 ms, from 16.6): it is the Inventory screen's own
turn, where its Garden views (about 4.5 ms: pH, pollination, picking, air, weeds, outlook) and the
page drawing its rows (`cpu.garden.page`, 6.5 ms) land on one frame. Next for the screens: the
Garden page lays out only the rows on screen (about 2,000 crops in the showcase), and the views
follow the rows' rotation. `just verify-screens` 13/13 after the change, and its viewport shows the
page screens drawn.

**The next step, built 2026-10-31: the Garden page lays out only what is on screen**
(`gui/widgets/offscreen.rs`, used by `draw_garden_section` in `gui/pages/inventory.rs`). A group,
or a row of slot tiles in an open group, that lies wholly outside the view takes the space it took
the last time it was laid out instead of being laid out again, so the scroll bar and everything
below stay where they were. Each size is kept under a key of everything that decides it (each
tile's word and outlook line, a group's title row, open state and the lines above its tiles, where
the block starts and how wide it may be, the text sizes, the font's measurements, the spacing and
the screen width), so a change lays the block out again the same frame, and the Dev page's "Turn
off all reuse" turns all of it off. A block never laid out before is laid out (nothing estimated),
so the first draw of a page still costs what it always did. Measured headless
(`garden_section_timings` in `gui/pages/inventory_garden_tests.rs`: the section's own draw, what
`cpu.garden.page` times in the game, the median of 40 frames in a 1280 x 800 window, a busy
machine). A test build is unoptimised, so these milliseconds are many times the release game's (the
2,000-crop "before" reads 107 ms here against 5.4 to 6.5 ms in the game); before against after, and
2,000 against 10,000, is what they are for:

| Garden | Before (top / middle) | After (top / middle) | Tiles laid out after | After, first frame |
|---|---|---|---|---|
| 2,000 crops in 90 groups (about the showcase) | 107 / 107 ms | **8.0 / 8.0 ms** | 85 / 82 of 2,000 | 122 ms |
| 10,000 crops in 450 groups (more towers) | 543 / 543 ms | **18.3 / 17.8 ms** | 69 / 66 of 10,000 | 605 ms |
| 10,000 crops in 90 groups (bigger towers) | about 590 ms (switch off) | **15.6 / 17.0 ms** | 96 / 127 of 10,000 | 538 ms |

"Before" is the test binary built from the code before the change (it had no 10,000-in-90 garden;
that row's "before" is the new build with the Dev page's switch off). The new build with the switch
off measured the same as the old one (104 / 105 and 575 / 579 ms), so the switch restores the old
behaviour. Five times the crops cost about five times as much before and about twice as much after,
the same whether they come as more towers or bigger ones: what is left grows with the crops but is
no longer layout, it is the key every tile of an open group needs every frame (its word and its
outlook line, so that a change re-lays it out the same frame) and the grouping, roughly 1 microsecond
a crop in an unoptimised build. The first frame of a page
still lays everything out once. Not yet re-read in the game: `cpu.garden.page` on the main screen
and on the Inventory wall screen's turn wants a quiet-machine rig run (this change booted no game).

### FarmingSystem on its own clock, 2026-10-31 (docs/design/farm-clock.md rung 2)

FarmingSystem averaged 3.6 ms of EVERY frame in Survey 1, its worst frames about 9 ms of its own
CPU, though at real time a crop changes by nothing anyone could see in a second. Rung 2 advances
each grow area once an interval (Settings > Gameplay > Garden updates, every second by default),
in 8 turns spread over the interval, and the air on its own turn (`systems/farming/clock.rs`).

**Headless, a test build (unoptimised: its milliseconds are many times the release game's, and a
release build and a check of another tree were compiling beside it, so read the ratios):**
`clock_tests.rs` `farm_cost_per_frame`, 2,000 crops of every plant in 90 areas (about the
showcase's 1,981 in 91 groups), 30 frames a second of real-time play, ten seconds after one to
settle. The code before the change (f06c0129e, same harness, a quieter moment): average 29.3 ms,
median 27.7, worst 56 to 69 ms. After (the first build, 8 turns), in one run:

| Garden updates | Average | Median | Worst |
|---|---|---|---|
| no interval published (every frame: the farm as before, bit for bit) | 59.1 ms | 57.0 ms | 104.7 ms |
| every second (the default) | 3.14 ms (5.3%) | 0.073 ms | 34.0 ms (32%) |
| every minute | 0.141 ms (0.24%) | 0.038 ms | 12.6 ms |

A frame with no turn costs the clock's own bookkeeping (the median). A turn frame costs its share
of the crops plus the body's setup and its passes over every crop to pick out the turn's (marks
read one by one), which in an unoptimised build are a large share of the turn.

**In the game** (release build, the showcase's 1,981 crops at `home-overview-noon`, the rig's
clock released to real time with the showcase verb `time_scale 1`, 30 fps in the background; each
row 12 readings of the rig's `debug/frame_costs.json` (the shape `game_frame_costs` returns) 5 s
apart, the average being frame_costs' own
running average and the worst the largest single frame of the last 5 to 10 s; no build running
except where said):

| Garden updates | FarmingSystem average | Worst frame |
|---|---|---|
| everything every frame (`HUMANITY_NO_REUSE=1`: the farm as before; a build shared the machine) | 3.51 ms (3.40 to 4.11) | 5.1 to 7.5 ms |
| every second, 8 turns (the first build) | 0.19 ms (0.12 to 0.26) | 0.75 to 1.41 ms |
| every second, 16 turns (shipped) | 0.24 ms (0.22 to 0.27) | 0.63 to 0.94 ms |

At 16 turns, per frame: crops 0.08, pest_tally 0.055, setup 0.033, pollination 0.017,
crop_setup 0.016, picking and weeds 0.009 each, the clock's own bookkeeping 0.006, the air 0.001
to 0.007 ms. 8 turns had the lower average and the higher worst; the worst is what is felt, so 16
shipped. On board the plant meshes keep the garden within a second whatever the setting, so this
is also the cost at home of the longer choices; off the station every minute costs about a
sixtieth of it.

**The budget (0.1 ms average, 0.5 ms worst) is not met yet.** What stands in the way, measured:
- **One whole advance of the showcase's crops is about 3.5 to 4 ms of CPU** (crop loop 1.95 ms,
  pest tally 1.0 ms with everything every frame), and at every second the farm does one each
  second however it is sliced: 0.12 to 0.14 ms a frame at 30 fps by itself, 0.06 to 0.07 at 60.
  The per-crop cost is next: the per-area and per-(area, plant) tables of F1 (not built), and the
  pest tally's per-crop conditions (each crop asks every pest under its own conditions).
- **Each turn has a fixed cost of about 0.16 ms** whatever its share: the setup (the rooms' air
  map, `AirMap::new`, matches every grow machine against every room box; the water, the light,
  the nutrient data), the pollinators' reach (`pollination::Help::now`, every machine), and a pass
  over every crop's mark in each of six passes. F3's "cache AirMap's structure until rooms or
  plots change" and the same for the pollinators, kept for a round like `pest_hosts`, is the next
  cut; with it a turn is about its share, under 0.5 ms.
- What was cut on the way: the pest host lists rebuilt every turn (now kept for a round,
  `FarmingSystem::pest_hosts`), the tally reading every skipped crop's record before its mark.

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
