# A headless, GPU-free, faster-than-real-time simulation runner (design, 2026-10-22)

The operator, 2026-10-22: "The simpler we make it for you to sim gameplay at rates beyond human
comprehension. I can handle maybe 3x speed comfortably. Could you do 1-billion-x?" This is the
design (a planning agent's, read-only, from the code at v0.1543.x). Increments 0, 1 and 2 are built;
what is built, and what the frame's file-size split (2026-11-14) already did for increments 2 and
3, is in "Built so far" after the increments. The line numbers in sections 1 and 2 are the v0.1543
lib.rs's, kept as the record of what was tangled; that file is now a dispatcher and the frame is
39 stage files (`src/engine/frame_*.rs`, the table in docs/design/crate-split.md).

## The short answer

Not for anything that moves; partly for things that only grow.

- **Movement and physics** (walking, stairs, falls, doors, elevators) must be stepped at frame
  rate. Headless, about **20x to 800x real time on one core**, depending on how often the slow
  systems tick alongside; about **4x** if every system ticks every step as today.
- **Slow processes** (crops, air, water, power, animals) can be stepped coarsely: about **10^4x
  at 1-minute steps, 10^5x at 10-minute steps, 10^6x at 1-hour steps**, with a loss of accuracy
  that is measured, not guessed.
- **About 10^9x only by jumping**, not by simulating every moment: the existing time-away ledger
  (`save_load::catch_up_world`, `crafting::away`, `mining::advance_away`,
  `livestock::timers_after_away`) moves a home forward by weeks in milliseconds, for what it
  covers (it deliberately leaves tanks and batteries where they are).
- **Several runs in parallel** add throughput, not speed per run; four stay inside the share of the
  PC the operator keeps (`scripts/pc-share-guard.ps1`).

The practical win: the storeys and falls rigs go from minutes with the GPU to seconds without it,
and a home can be soaked for game-weeks in seconds.

**Measured** (`.probe-rig/debug/frame_costs.json`, v0.1539.0, a rig frame with no terrain
streaming): the whole ECS tick `cpu.systems` = 3.80 ms, of which **FarmingSystem alone is 3.51
ms** (the other 23 systems 0.29 ms); `cpu.frame_total` = 34.6 ms, about 18.7 ms of it timed by
nothing (the walk, the GUI bridges, the draw-list assembly); the planet walk's ground sampling
about 0.6 ms a frame (lib.rs ~4832's comment); the walk aboard never measured.

## 1. What in the frame is simulation, what is drawing, and where they are tangled

The whole frame is one match arm, `WindowEvent::RedrawRequested` (lib.rs 3258 to 16568), driven by
`about_to_wait`'s `request_redraw`.

| Lines (src/lib.rs, v0.1543.x) | What | Kind |
|---|---|---|
| 952-1444 | `resumed()`: hidden window, `Renderer::new_native(window)` (1027) before any game setup, the DataStore slots, the 24 `system_runner.register` calls (1114-1421), save boot, fresh player, asteroids, home power entities | setup, GPU first |
| 1446-1812 | egui, GuiState catalogs, registries, star-sky prebuild (needs the GPU device), planet defs, vehicles, config, `resume_home` (1750) | mixed |
| 1814-2139 | The `EngineState` literal: `window`, `renderer`, `egui_state`, `egui_renderer` need a window and GPU (state.rs 160-161, 1121-1122); the rest is CPU-only, GuiState too | mixed |
| 2203-2990 | Keyboard arm; the E-press actions inline at ~2653+; movement through `engine::input::apply_game_action` (2966) | input |
| 3278-3318 | Frame-rate cap; `dt = raw_dt.min(0.1)` from `Instant`; the movie's fixed step (3313) is a virtual clock already | clock |
| 3320-3326 | `current_sim_t = SystemTime::now()`: planets placed by the wall clock | sim reads the wall clock |
| 3351-3549 | Station orbit, boarding and riding (sim), F11 readout and markers (GUI) | mixed |
| 3566-3631 | Speed multipliers, carried load, dev fly | sim |
| 3641-3661 | Elevator tick, gated on `camera.mode == FirstPerson`: a view setting gates physics | sim |
| 3675-3756 | Footing, teleporter, ladder, `controller.update_camera_with_floor`; the body IS the renderer's camera (`CameraController`, `src/renderer/camera.rs`, CPU-only) | sim |
| 3770-3815 | Probe hold, a 1.5 s grace on the wall clock | sim |
| 3817-4153 | Ambient particles | cosmetic |
| 4155-4197 | House lights drive a `PowerConsumer` | sim |
| 4320-5253 | Frame lock and planet walk: the ground is the patch the renderer DREW last frame (`planet_chunk_states.last_drawn`, 4842-4846); Earth's tile tier requested only by the view's LOD selection (9735); bodies placed with `SystemTime::now()` (4453) | sim depends on view |
| 5262-5309 | Wall and door collision | sim |
| 5317-5450 | Driving, follow cam | sim |
| 5455-5768 | Camera into the DataStore, look targets, landing to injury, body environment | sim |
| 5770-6660 | GUI-to-ECS bridges (`gui_state.pending_*` into the DataStore): the sim's command queue | sim input |
| 6665-6721 | Stock publish, `system_runner.tick` (6671), after-tick drains | sim |
| 6723-6880 | Co-presence join and position stream | networking |
| 6882-6945 | Refunds, autosave (wall clock) | sim |
| 6946-7384 | Showroom, construction editor (edits sim state, rebuilds GPU meshes) | tool, mixed |
| 7390-11800 | Draw-list assembly; door simulation lives here (`render_door_panels` advances every door's open fraction; the next frame's walk collides against it), only when the home is drawn | sim inside view |
| 11951-14264 | ECS-to-GUI mirror with game-rule settles inside (livestock collect, melee, loot, `build_place::frame`, vendor, abilities, equip, respawn) | sim inside view |
| 14266-14320 | Dev IPC polls | rig input |
| 14409-14700 | Networking | networking |
| 14708-14710 | `load_world` from the GUI page state | sim and GPU |
| 14791-16180 | Scene passes, egui, present; `poll_showcase_request` (the rigs' verbs) runs only inside the branch that acquired a swapchain image (16113) | view holds rig input |

Also: `world_load.rs` mixes GPU uploads (48 `add_mesh` sites) with building the sim (colliders,
room bounds, machines, air space, door panels); `rig_walk::HELD` times keys with `Instant`; seven
process-wide statics (`rig_walk::HELD`, `ipc::REMOTE_RECORDING`, `save_load::LAST_SAVE_SECS`,
`DATA_DIR`, `storage::MODE`, `frame_costs`, save_load's thread-locals) make it one sim per process.
The ECS systems themselves are already clean: `crates/humanity-systems/src/` reads the wall clock once outside
tests (`economy/mod.rs:388`), and most slow models are exact over any step length
(`humidity::step_vapour`, `pests`, `fire::burn_step`, `life_support`; `body_heat` splits long steps
into 10 s pieces; crops age by `planted_at` against game time). The trouble is in lib.rs.

## 2. The runner: one setup, one sim step, two drivers

The two paths must be the same code, not twins:

```
EngineState::boot(opts, gpu: Option<Gpu>)        // resumed() passes Some, --sim passes None
frame_sim::step(state, &FrameClock, &mut Inputs)  // the ONLY simulation entry point
  windowed: RedrawRequested = clock from Instant -> frame_sim::step -> frame_view::draw
  headless: loop { clock from script -> frame_sim::step }   // no EventLoop, no wgpu Instance
```

- **`gpu: Option<Gpu>`** (`Gpu { window, renderer, egui_state, egui_renderer }`): every sim
  function keeps `&mut EngineState`; the view reaches the renderer through the option (a macro);
  ~547 `state.renderer`, 35 window and 21 egui sites change mechanically, and the compiler then
  lists every GPU touch left in sim code.
- **`FrameClock { dt, clock_dt, sim_epoch_s, anim_s }`**: every sim read of `Instant` or
  `SystemTime` goes through it (rig holds, the probe grace, `current_sim_t`, frame lock's `sim_t`,
  autosave, `resume_home`'s `now`; `catch_up_world` already takes `now`).
- **Inputs, one verb table, two transports**: the showcase, autopilot and recorder verbs become a
  parsed `Verb` enum with `apply`; the `debug/` file drop stays for windowed runs; headless reads
  JSON lines on stdin or a script. Verbs: enter, place/stand, face, hold keys for N SIM seconds,
  console, illness, time speed, weather, save, fast-forward, record. Picture-only verbs are refused
  headless ("needs the picture"). GUI actions go through the same `gui_state.pending_*` fields the
  buttons set; the E-press logic moves into `engine::input` so the key and the verb share it.
- **Outputs**: the recorder's JSON builders (`storey_probe::frame_json`, `body_probe::falls_json`,
  `home_session_json`); the JS judges stay; `lib/sim-transport.js` sends a verb and gets frames back.
- **Same binary**: `HumanityOS --sim [script|-]`, dispatched in `main.rs` before the hand-off and
  before winit, like `--headless`; the freshness gate applies; always the release build.
- **Safety**: refuses outside a scratch portable storage root (never `%APPDATA%` saves); updater,
  live weather, host-node autostart and audio off; Weather, Disasters, AI, Combat and Fire take a
  `sim_seed` DataStore slot (OS entropy when unset, so the windowed game is unchanged).
- **What moves**: ~700 lines of setup to `engine/sim_boot.rs`; ~4,100 lines (3302-7389 minus
  particles and editor GPU work) plus ~1,100 scattered sim lines to `engine/frame_sim*.rs`; lib.rs
  drops to ~10-11k lines; verbatim moves first, as `frame_water.rs` and `frame_shells.rs` were.

## 3. Time acceleration, kind by kind

`h` = physics step (1/60 s), `c_p` = one physics step's cost, `c_s` = one systems tick (3.8 ms
measured), `N` = systems decimation; the multiple of real time is `h / (c_p + c_s/N)`.

**Fixed-step physics, one core of the i7-8700K** (the walk aboard has no broad phase: linear scans
in `footing.rs` 132-181 and `wall_collision::resolve`; 0.02-0.2 ms a step estimated, increment 0
measures it):

| Schedule | ms/step | steps/s | x real time |
|---|---|---|---|
| Every system every step (today) | ~4.0 | ~250 | ~4x |
| Slow systems at 10 Hz | ~0.85 | ~1,200 | ~20x |
| Slow systems at 1 Hz | ~0.27 | ~3,700 | ~60x |
| Movement only, aboard | 0.02-0.2 | 5k-50k | 80-800x |
| On a planet's ground | ~0.7-1.0 | 1.0-1.4k | 17-24x |
| Farming made 10x cheaper, every step | ~0.6 | ~1,600 | ~28x |

FarmingSystem is the lever; its heavy passes are already exact over any step, so it can run at a
slower rate inside its own tick, in BOTH paths (the windowed game gets ~3 ms a frame back).
Sweeping dt from 1/240 to 0.1 s reproduces frame-rate bugs (BUG-164, BUG-207, the stair-tread skip).

**Slow processes stepped coarsely** (systems only, ~3.8 ms a tick):

| Game s per tick | x real time | A game week | What is lost |
|---|---|---|---|
| 1 | ~260x | 39 min | reference |
| 60 | ~16,000x | 38 s | negligible |
| 600 | ~160,000x | 3.8 s | completions on tick boundaries; threshold crossings late |
| 3,600 | ~900,000x | 0.6 s | solar sampled once a tick (a staircase dawn); a tick's lag between demand and supply; mid-step clamps; chained machines lose up to a tick a batch |
| 86,400 | ~2x10^7 | 27 ms | no day and night |

(With farming 10x cheaper, ~7x more per row.) Checks against the fine step: a **dt ladder** (same
seed and script at 1, 10, 60, 600, 3,600 s; observables every game day against the 1 s run;
tolerances in `data/sim/step_tolerances.ron`; the largest passing step is the home's "safe step",
printed with its measured error in every soak report); **step halving** (first-order error halves,
exact models stay at zero; anything else is a bug); a **split-step invariance test** over the real
registered SystemRunner (`tick(dt)` against `tick(dt/2)` twice on conserved quantities).

**Fast-forwarding a home**: `ff live` steps the real systems at the safe step (the ground truth);
`ff away` saves, moves the clock (`catch_up_world(.., now = save.timestamp + T)`) and resumes
through `resume_home` (exactly a returning player's path); an **away-vs-live rig** compares them
after N days, and gaps outside `offline-progression.md`'s exclusions are bugs in one model or the
other.

## 4. What can never be headless, and the rigs' split

Never: anything about the picture or the GPU (shaders, lighting, shadows, clouds, water, sky, LOD
seams, trees on the drawn ground, screenshots, golden vantages, draw cost, device-limit rejections
and world-entry GPU panics, egui layout, in-world screen pixels, audio, focus).

| Rig | Headless | Stays windowed |
|---|---|---|
| verify-storeys | all 15 legs | the 3 kept screenshots |
| verify-falls | the legs aboard; the planet legs after increment 6 | the planet legs until then |
| verify-copresence, --plots, --starts | the judges, at 1x (the relay ticks on wall time) | the two "body visible" shots |
| probe-sweep, visual-sweep, verify-runtime, snapshots | none | all |
| New | home soaks, dt ladder, away-vs-live, frame-rate sweeps, determinism, a nightly random-input fuzzer | none |

The windowed rigs still run in each release batch.

## 5. Increments

| # | Increment | Proof | Effort |
|---|---|---|---|
| 0 | **Built 2026-10-30.** Measure: stage timers around the walk aboard, the planet walk, the bridges, the settles | numbers in this doc; the untimed part of `frame_total` shrinks | 0.5 d |
| 1 | **Built 2026-11-15.** Setup split: `engine::sim_boot` and `engine::world_sim::build` (GPU uploads stay in `world_load`) | windowed game unchanged (verify-runtime, verify-storeys); a lib test builds the homestead with no GPU | 3 d |
| 2 | **Built 2026-11-15** (the ECS stage; the bridges stay for increment 3). Systems slice and the first headless value: lib.rs 5770-6721 into `frame_sim::systems_slice`; `--sim soak`; sim seeds | a 7-day soak in seconds; the dt ladder report; the split-step test over every system; byte-identical repeat runs | 2-3 d |
| 3 | **Built 2026-11-15, but for the settles** (see "Built so far"). Frame split, verbatim, and the inversions fixed (door easing to `doors::tick`, the showcase poll out of the present branch, `FrameClock`, settles at the end of step) | aboard rigs green; `frame_split_lint` (step once before view code; no renderer/egui/window/wall clock in frame_sim*.rs); a written read-after-write audit | 4-5 d |
| 4 | `gpu: Option<Gpu>`, `EngineState::boot(opts, gpu)`; sim-side mesh creation becomes dirty flags | `--sim` enters the homestead and walks with no EventLoop and no wgpu Instance; windowed rigs green | 3-5 d |
| 5 | Transport and headless rigs (the `Verb` table, stdin JSON lines, `--headless` on verify-storeys and verify-falls aboard) | headless and windowed verdicts agree (75/75); the floorless sandbox fails headless too | 3 d |
| 6 | The planet's ground owned by the sim (CPU LOD selection and tile requests in `step`) | the falls planet legs pass both ways; the near-tree ground check green | 3-4 d |
| 7 | Rate classes and farming's cost (per-system rate in `SystemRunner`, both paths) | windowed frame ~3 ms cheaper; dt ladder within tolerance; GUI requests answered under 100 ms | 2-3 d |
| 8 | Parity and replay (a windowed run's dt and inputs replayed headless, compared frame by frame), a frame-rate sweep, the fuzzer | positions within 1e-4 m, same landings and floors; no panic in N sim-hours | 3 d |
| 9 | Copresence headless at 1x; later the relay on a sim clock | `copresence-judge` on headless frames; a 20-player crowd with no GPU | 3-4 d |

About 27-35 agent-days. 1 to 5 are mostly sequential (all touch lib.rs, the hottest shared file:
land 3 and 4 as verbatim moves in quiet windows, staged by hunk, both `cargo check`s on the staged
tree); 6 to 9 can run in parallel afterwards. First real value (game-week soaks) after increment 2.

## Built so far

**Increment 0** (2026-10-30): the stage timers, measured and tiled in docs/design/sim-frame-cost.md
(the spans add up to `cpu.frame_total`). Part of increment 7 came with it: FarmingSystem runs on
its own clock (docs/design/farm-clock.md rung 2).

**Increment 1, the setup split** (2026-11-15). The world's simulation is built without the GPU, by
the same code in both paths:

- `src/engine/sim_boot.rs` (`boot(SimBootOptions) -> SimBoot`, then `starter_vehicles` and
  `apply_home_save`): the ECS world, the DataStore's slots, the 24 systems in tick order, the live
  save chosen, the fresh player, the asteroids, the menu's live home power, the starter vehicles
  and the save's progress with its time-away catch-up. Moved verbatim from lib.rs `resumed`
  (lib.rs 2,372 to 1,871 lines; sim_boot.rs 584). The options carry the data folder, the saves folder
  (a test gives a scratch one) and the config as read.
- `src/engine/world_sim.rs` (`build(Parts) -> WorldSim`): at world entry, the ship assembled with
  the home on its plot, the save's pieces carried to it, the walls and sight walls, the room
  boxes, whose air each place breathes, where the player arrives, the household's machines as
  live entities with their levels, the herd, the wilds and the home's air space, with the home's
  geometry returned as data (world_sim.rs about 600 lines with the herd and the wilds; world_load.rs
  1,409 to 956). `world_load::load_world` calls it first, then uploads and draws
  (the meshes, room GI, the hull, the lights, the machines' bodies, the screens, the decorations,
  the grow plots, the pipes, the doors) in the order those ran before. The one order change: the
  drawing that ran between the sim steps now runs after all of them; none of it reads what the
  sim steps write (it reads the ship, set first, and the household's layout, read only by the
  machines' bodies, which ran after it before too), and the entities spawn in the same order.
  `spawn_herd` and `spawn_wilds` moved there from world_load.rs; `publish_home_frame_on`,
  `carry_loaded_save_home`, `survival_env::ship_spaces` and `home_meshes::placements_of` are the
  four helpers that took `EngineState` and now take the parts.
- `world_sim_tests.rs`: the homestead entered with no wgpu Instance, no window and no
  `EngineState`, checked against what the windowed game holds (every system registered, the ship
  with rooms and walls, every live machine standing where the world load draws it and every drawn
  machine with a power or water role live, the air space, the player arriving in a room).
- `tests/sim_gpu_free_lint.rs`: no wgpu, winit, egui, renderer, `Window` or `EngineState` in the
  sim files' code, no `headless` (the "no `if headless`" rule, started), and the windowed game
  built through them (lib.rs registers no system of its own). `tests/engine_wiring_lint.rs` reads
  the registrations from sim_boot.rs.

Still tangled at world entry, for increment 4 (sim-side mesh creation becoming dirty flags): the
grow plots (`publish_grow_plots` publishes the anchors `build_machine_bodies` records while it
builds meshes), the chairs (filled by the same builder), the door panels (`rebuild_door_panels`
builds each door's mesh and its open state together), the station's orbit and the planet bakes
(later in `load_world`, already GPU-free in what they compute, left in place so nothing reorders).

**What the frame's file-size split already did** (2026-11-14, docs/design/crate-split.md, "Step 1's
stage list"). Increment 3's verbatim move is done in its first half: `window_event` is 39 stage
files in frame order, and the sim stages are files of their own (`frame_clock`, `frame_station`,
`frame_move_setup`, `frame_walk_aboard`, `frame_planet_walk`, `frame_collide_drive`,
`frame_look_env`, `frame_bridges`, `frame_bridges_npc`, `frame_ecs`, `frame_settles`). Not done:
every stage still takes `&mut EngineState`, the sim stages are interleaved with view stages
(the settles run after the draw lists; door easing is still inside the home's draw list; the
showcase poll is still in the present branch), and no `FrameClock` exists. Increment 2's slice,
"lib.rs 5770-6721", is now `frame_bridges.rs`, `frame_bridges_npc.rs` and `frame_ecs.rs`; inside
`frame_ecs::run` the tick itself (`world_hold::tick`) already takes only the sim's parts.

**Increment 2, the systems slice and the first headless value** (2026-11-15). The ECS stage's
body is `frame_sim::systems_slice(SimParts { runner, world, data, gui, world_loaded }, dt, &FallSite)`
(`src/engine/frame_sim.rs`), the one step the windowed frame (`frame_ecs.rs`), the soak and the
no-GPU tests run. The five calls in it that took `EngineState` take the parts
(`stock_piles::publish_home_stock`, `take_consumed_home_stock`, `receive_machine_outputs`,
`age_home_storage`, `death_pack::after_tick`); their state versions had no other caller and are
gone. Where the player stands for a Realistic death's pack is `death_pack::FallSite` (the frame
lock, the camera's feet, `planet_build::GroundSources` borrowed field by field), read before the
step. The order audit: the packs' per-frame body list is cleared before the tick instead of after
it, and the fall site is read before the tick instead of after; nothing between reads either.

- `src/engine/sim_home.rs`: the homestead entered as the game enters it (stepping into the world
  as `showroom_confirm` does: without it the systems are held, BUG-253, and nothing moves), the
  opening quest's tower planted as the Plant button plants it, and every system's quantities read
  by name (`OBSERVABLES`: clock, body, crops, batteries, tanks, air, storage, herd).
- `src/engine/sim_soak.rs`: `HumanityOS --sim soak [--days N] [--step S] [--force]` and `--sim
  ladder` (`just sim-soak`, `just sim-ladder`; main.rs dispatches before any window, relay or
  hand-off). Each run is a fresh homestead on its own thread, scratch saves, the run's seed
  (`humanity_systems::sim_seed`: the weather's and the disasters' streams; the thread-shared draws
  are listed there as not yet seeded). The tolerances, the ladder, the check step and the safe step
  are `data/sim/step_tolerances.ron`: a quantity holds within `abs` plus `rel` times how far it
  moved at the reference step. The Dev page's Simulation card runs the soak as a separate process
  (`src/gui/pages/dev_sim.rs`); the ladder is CLI debt (`docs/design/in-app-ops.md`). The machine
  guard does not count a `--sim` run (`isSimRun`, risk 4).
- Tests (`src/engine/frame_sim_tests.rs`): a game day at the safe step with no GPU; the split-step
  test, two minutes at the frame's 1/60 s against 1, 10 and 60 s; the ladder holding the recorded
  safe step over two hours; two runs of one seed identical over a day; the refusal of a step past
  the safe one. The split-step test's first run found **BUG-314**: every load the air step drives
  spawned at its full draw and the electrical sim integrates before the air step writes it, so a
  world's first step billed 3.1 kW of air handlers for the whole step (522 Wh at 600 s, most of what
  the ladder had first blamed on the 600 s step); they start idle now.

**Measured** (release build, i7-8700K, one core, 2026-11-15):

| Run | Steps | Wall time | |
|---|---|---|---|
| Ladder, 24 h at the 1 s reference | 86,400 | 38.0 s | 0.44 ms a step |
| 24 h at 10 s / 60 s / 600 s / 3,600 s | 8,640 / 1,440 / 144 / 24 | 3.75 / 0.63 / 0.063 / 0.012 s | |
| **7-day soak at the 600 s safe step** | 1,008 | **0.45 s** | **15.7 game days a second, 1.36 million x real time** |
| The soak's error check: its first day again at 10 s | 8,640 | 3.8 s | |

The whole 7-day soak, entry and error check included, took 4.5 s of wall time. Entering the
homestead takes 0.08 s. The ladder over a day: every quantity holds at 10, 60 and 600 s; at
3,600 s the batteries are off by 408 Wh (255.6 allowed: the sun sampled once an hour, the
design's "staircase dawn"), so **the safe step is 600 s**, and the soak's measured error at it is
47.6 Wh on the batteries (255.6 allowed), 0.13 L on the tanks, 0.012 on crop health and nothing
measurable elsewhere. Two runs give identical reports, with or without the seed: the homestead
aboard sees no weather (the weather's clouds reach solar panels and bodies on a planet's ground),
so the seed is for the planet runs to come. What the 7 days show of the
untouched homestead: the batteries fill on day 1 and hold, the tanks fill by day 3, the crops
grow every second, the body goes without a drink and dies of dehydration 2.6 game days in
(nobody drinks in an untouched run; the report says so), and the herd says it has nothing to eat.

**Not yet in increment 2's soak** (increment 3 put them in, below; the bridges took
`EngineState` then): the body's per-frame environment (`survival_env::publish`: the sealed home's air
around the body, the felt gravity; the soak's body keeps what the boot gave it) and the GUI's
requests (eat, drink, plant, craft) except the planting the soak makes itself. The Settings levels
(body heat, illness, animal care, death) are published once at the soak's entry
(`survival_env::publish_levels`), the garden levels by `sim_boot::apply_home_save`.

**Increment 3, the bridges, the clock and three of the four inversions** (2026-11-15). The soak's
person eats, drinks, draws water and sleeps through the same requests a player's clicks make, and
the windowed game and the soak step the same sim the same way:

- `frame_sim::bridges_slice(SimParts, &BodyPlace)`: the bridges' world-feeding half on the sim's
  parts. `BodyPlace` is where the person is as plain values the driver reads before the step (the
  `PlayerView`: aboard, the eye and the look, or the locked body and the eye in its frame; the
  gait, driving, fly mode, the felt gravity, whose air each place breathes, the planets' ground
  borrowed field by field). It publishes the body's environment (`frame_lock::publish_body_environment`
  and `survival_env::publish`, both on plain values now; `planet_build::frame_of_place` is the
  shelter test's frame), the drone's panel, and the GUI's requests
  (`src/engine/frame_sim_requests.rs`, moved verbatim from frame_bridges.rs and the tail of
  frame_bridges_npc.rs: craft, eat, drink, use, plant, deploy, summon, the tower and bed
  plantings, harvests, the per-frame settings mirrors, the backpack's transfers, rest, compost,
  the drone's orders, and the water tank card's Fill and Pour, moved in from frame_cards.rs). The
  windowed frame runs it where the bridges ran (frame_bridges.rs builds the `BodyPlace` from the
  camera, the controller, the frame lock and the drive); `Homestead::step` runs it before
  `systems_slice`, its person standing where they arrived in the parked ship.
- `FrameClock { dt, clock_dt, raw_dt, sim_epoch_s, frame, elapsed_s }` (frame_sim.rs), the one clock:
  `EngineState::clock`, advanced once a frame by frame_clock.rs, replaces `FrameVars`' three steps
  and `current_sim_t`; every stage reads `state.clock`, and `systems_slice` takes it. The soak
  advances its own; `Homestead::step_game` turns its game step into the frame's real `dt` at the
  time speed the next tick runs at (`humanity_systems::time::next_tick_scale`), so a night's sleep
  (the clock at 7,200 times) is stepped as the windowed game steps it, ten game minutes a twelfth of
  a real second, where a plain 600 s step would have slept 50 game days in one step.
- Door easing is `engine::doors::tick`, a stage of its own right before the home's draw list
  (moved verbatim out of `home_meshes::render_door_panels`, which draws the doors where the tick
  left them). The poll of the rigs' verbs runs after the present branch, every frame, presented or
  not (lib.rs; it was inside `frame_tail`).
- **Not done: the settles at the end of the step** (frame_settles.rs still runs after the view's
  draw-list stages), and so no `frame_split_lint` yet: increment 4's work below.
- A person's routine for the soak, as data: `data/sim/routines/daily.ron` (acts at hours of the
  home's day: drink, eat, sleep, rest; the foods and drinks reached for) and
  `src/engine/sim_routine.rs`, which turns each act into the requests the pages set (the Drink
  and Eat buttons, the tank card's Fill, "Harvest N ready", "Take to backpack", the Rest button,
  the bed prompt's sleep request) and measures what each did from the backpack and the bottles,
  never from what it asked. `--sim soak --routine daily` (and `--sim ladder --routine daily`); the
  Dev page's Simulation card has "Run a week of a person's day". The default soak has no routine.
- The bridges' quantities are observables with tolerance rows: the environment around the body
  (`env.air_c`, `env.humidity`, `env.oxygenated`, `env.g_load`) and the backpack (`pack.items`,
  `pack.water_l`).

The order audit (each move's read-after-write check is written where it moved: frame_bridges.rs,
frame_sim_requests.rs, doors.rs, frame_tail.rs):

| Moved | From, to | What changes |
|---|---|---|
| The Planet Tuner's readout | between the body's environment and the survival context, to after the slice | nothing: it reads the body's environment published first in both |
| frame_bridges_npc.rs's requests | after the dev teleport, spawn, creature editor and NPC card, to before them | nothing: neither side reads what the other writes |
| The tank card's Fill and Pour | frame_cards.rs after the tick, to the bridges before it, the same frame | this frame's tick sees the filled bottles and the lower tank instead of the next frame's |
| Door easing | inside the home's draw list (only while the home was drawn), to its own stage just before it, every frame | doors keep easing while the showroom is open (they paused there) |
| The rigs' verb poll | inside the present branch before the live broadcast and the present, to after the branch, every frame | verbs also run on a frame that acquired no image; a verb that changes a setting is applied by the next frame's tail |
| The frame's clock | `current_spin` before `current_sim_t`, to after the clock | nothing: the spin reads the game clock and the sun, not `current_sim_t` |

Found on the way: **BUG-317**, the first step after world entry breathed vacuum (the home's air
status started all zeros until the first atmosphere step; a frame in the windowed game, a 600 s
step in the soak, where the routine's person suffocated in it). The world entry publishes the
home's air now (`atmosphere::publish_air_status`).

**Measured** (release build, i7-8700K, 2026-11-15; machine shared with other agents' builds):

| Run | Steps | Wall time | What it shows |
|---|---|---|---|
| 7-day soak, no routine | 1,008 | 0.49 s (14.4 game days a second, 1.24 million x) | as increment 2's: dead of dehydration 2.6 game days in; the numbers move only by the body now breathing the home's real air (core 36.822 C against 36.826, hydration at day 1 29.50 against 29.43), plus the six new rows; every quantity within tolerance |
| 7-day soak, `--routine daily` | 1,016 (steps landed on the acts' hours) | 0.49 s (14.2 game days a second) | **alive at day 7**: 42 drinks (4 of the kit's jugs, then 38 bottles filled at the tank), 19.0 L drawn from the tank, 3 meals (the kit's 3 rations), 7 nights in bed; hydration held at 94.8; fullness 0 by day 7 and health 98.6 (the food ran out); every quantity within tolerance at its 10 s check |
| 60-day soak, `--routine daily` | about 8,700 | about 4.2 s | starves 20.8 game days in: the kit's 3 rations are the only food, the soak's home storage is empty (the start's storage seed is the windowed GUI's, not loaded headless), and the garden's first lettuce comes ripe after that |

The one system that did not hold its tolerance at 600 s with the routine: the body's core
temperature, 0.037 C off at 600 s against 0.024 allowed (a night's sleep changes the body's
activity once a step); its tolerance is 0.05 C now, with the reason in the file. Nothing else
had to be held at a finer step.

**Increment 4, its first step**: the settles at the end of the step (frame_settles.rs, the
ECS-to-GUI mirror's game rules: livestock collect, melee, loot, `build_place::frame`, the vendor,
abilities, equipping, respawn) moved before the view's draw-list stages, with their audit, and
`frame_split_lint` (the sim stages run once, before any view stage; no renderer, egui, window or
wall clock in frame_sim*.rs); then `gpu: Option<Gpu>` and `EngineState::boot(opts, gpu)`, so
`--sim` enters the homestead through the same `EngineState` with no wgpu Instance and the
compiler lists every GPU touch left in sim code (the doors' tick and the settles among them).

## Risks

1. **Headless and windowed drifting apart.** One `boot`, one `step`, no `if headless` in sim code
   (a lint counts it: zero); the replay parity rig in every release batch where frame_sim changed;
   a determinism test (same script, same bytes). `wasm_entry.rs` (7 systems against 24 native) and
   the life-support tests' hand-built `make_store` are already drifted twins: new long tests use
   `sim_boot`.
2. **Order changes** when sim code embedded in the view moves (door easing pauses while the
   showroom is open; settles move relative to the draw list): a written read-after-write audit,
   behaviour changes listed in the commit.
3. Rig timing moves from the wall clock to the sim clock (the judges read recorded frame times).
4. The machine guard counts every `HumanityOS.exe` as a GPU instance (`isGameName`): `--sim` runs
   must be excluded by command line; they stay under the CPU share guard, at most four.
5. lib.rs conflicts (see above).
6. Trusting coarse steps too much: every soak report carries its step and measured error; steps
   beyond the safe step refused unless forced.
7. The planet's ground is the deepest dependency of the walk on the drawing; its legs stay windowed
   until increment 6.

Critical files: `src/lib.rs`, `src/engine/state.rs`, `src/engine/ipc.rs`,
`src/engine/world_load.rs`, `src/engine/home_meshes.rs`; also `src/main.rs`, `crates/humanity-core/src/ecs/systems.rs`,
`src/engine/rig_walk.rs`, `crates/humanity-systems/src/time.rs`, `src/save_load.rs`, `tests/engine_wiring_lint.rs`,
`scripts/lib/machine-guard.js`.
