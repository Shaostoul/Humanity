# A headless, GPU-free, faster-than-real-time simulation runner (design, 2026-10-22)

The operator, 2026-10-22: "The simpler we make it for you to sim gameplay at rates beyond human
comprehension. I can handle maybe 3x speed comfortably. Could you do 1-billion-x?" This is the
design (a planning agent's, read-only, from the code at v0.1543.x). Nothing here is built yet;
the increments are at the end.

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
The ECS systems themselves are already clean: `src/systems/` reads the wall clock once outside
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
| 0 | Measure: stage timers around the walk aboard, the planet walk, the bridges, the settles | numbers in this doc; the untimed part of `frame_total` shrinks | 0.5 d |
| 1 | Setup split: `engine::sim_boot` and `engine::world_sim::build` (GPU uploads stay in `world_load`) | windowed game unchanged (verify-runtime, verify-storeys); a lib test builds the homestead with no GPU | 3 d |
| 2 | Systems slice and the first headless value: lib.rs 5770-6721 into `frame_sim::systems_slice`; `--sim soak`; sim seeds | a 7-day soak in seconds; the dt ladder report; the split-step test over every system; byte-identical repeat runs | 2-3 d |
| 3 | Frame split, verbatim, and the inversions fixed (door easing to `doors::tick`, the showcase poll out of the present branch, `FrameClock`, settles at the end of step) | aboard rigs green; `frame_split_lint` (step once before view code; no renderer/egui/window/wall clock in frame_sim*.rs); a written read-after-write audit | 4-5 d |
| 4 | `gpu: Option<Gpu>`, `EngineState::boot(opts, gpu)`; sim-side mesh creation becomes dirty flags | `--sim` enters the homestead and walks with no EventLoop and no wgpu Instance; windowed rigs green | 3-5 d |
| 5 | Transport and headless rigs (the `Verb` table, stdin JSON lines, `--headless` on verify-storeys and verify-falls aboard) | headless and windowed verdicts agree (75/75); the floorless sandbox fails headless too | 3 d |
| 6 | The planet's ground owned by the sim (CPU LOD selection and tile requests in `step`) | the falls planet legs pass both ways; the near-tree ground check green | 3-4 d |
| 7 | Rate classes and farming's cost (per-system rate in `SystemRunner`, both paths) | windowed frame ~3 ms cheaper; dt ladder within tolerance; GUI requests answered under 100 ms | 2-3 d |
| 8 | Parity and replay (a windowed run's dt and inputs replayed headless, compared frame by frame), a frame-rate sweep, the fuzzer | positions within 1e-4 m, same landings and floors; no panic in N sim-hours | 3 d |
| 9 | Copresence headless at 1x; later the relay on a sim clock | `copresence-judge` on headless frames; a 20-player crowd with no GPU | 3-4 d |

About 27-35 agent-days. 1 to 5 are mostly sequential (all touch lib.rs, the hottest shared file:
land 3 and 4 as verbatim moves in quiet windows, staged by hunk, both `cargo check`s on the staged
tree); 6 to 9 can run in parallel afterwards. First real value (game-week soaks) after increment 2.

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
`src/engine/world_load.rs`, `src/engine/home_meshes.rs`; also `src/main.rs`, `src/ecs/systems.rs`,
`src/engine/rig_walk.rs`, `src/systems/time.rs`, `src/save_load.rs`, `tests/engine_wiring_lint.rs`,
`scripts/lib/machine-guard.js`.
