# Splitting the crate: what goes where, and in what order

Status: **the split is decided (operator, 2026-10-30): "The split makes sense. Saves us compute.
Saves electricity. Makes development a little bit faster."** It reverses the 2026-06-30 one-crate
decision (CLAUDE.md, Architecture: "one `Cargo.toml` at the repo root, no workspace, no
sub-crates"), which changes when rung 1 lands, not before. **The ten crates and the names are his
decisions too, the same day:** "I like the idea of going the full 10. It sounds like it'd be the
most beneficial over a long period of time. Feels kind of small now but, huge a year from now."
and, of the names, "Let's go with humanity." **Rung 0 is done (2026-10-31):** parts A and B took
the wrong-way references in code from 157 to 0, and the ratchet holds every edge at zero. **Its
backlog is cleared the same day** (rung 0, "Not counted", below): the tests' 127 wrong-way
references are 0 outside six farm test files another session was editing (15 there), and the
ratchet now holds test code too, file by file; the data folder's path is the core's; `release_update`
is the GUI's; main.rs and the crate root's own items are counted; one cut-away box. **Rung 1 is
done (2026-11-01):** the Cargo workspace, with `humanity-core` and `humanity-protocol` cut out into
`crates/` (Order of work, below, with the build times before and after: unchanged, as a small
piece off one end of the chain predicts). **Rung 2 is done (2026-11-02):** `humanity-ship`,
`humanity-terrain` and `humanity-systems` cut out, about 138,000 lines from under the engine (Order
of work, below, with the build times). Rung 3 is next.

The plan's one source is `scripts/crate-layers.json`: which crate every part of `src/` goes to and
which crates each may use. `node scripts/crate-graph.js --crates` prints each crate's size, and
`--edges` lists every reference that points the wrong way under the plan (the rung-0 work list).
Every number here comes from those, from `git log` over the 30 days to 2026-10-30, or from
`docs/ai/builds.md` ("Measured 2026-10-24"). Re-run before acting: the code moves daily.

**Each crate has a one-page card** ([docs/crates/](../crates/README.md), 2026-10-31; the operator,
2026-10-30: "Sounds like something we gotta do"): what it owns, its interface and rules, how to test
it, its open work and the requests other crates have made of it, so an agent dedicated to one crate
reads one page. Each card's numbers are generated from `scripts/crate-layers.json` and the counting
above, and `just brief` refreshes them.

## Why

- **One compile of our crate, cold: 241 s** on the operator's PC. Type checking 46 s and borrow
  checking 50 s run on ONE core inside one crate (about 40% of every compile); code generation
  105 s is already spread across the cores; linking 5 s (`docs/ai/builds.md`).
- **A one-line edit re-checks all of it: 56 s.** The crate is about 640,000 lines of Rust in 1,037
  files (the biggest single file is `src/lib.rs`, 16,672 lines), and an edit anywhere re-checks
  everything.
- **The server build** (the relay, `--features relay`) compiles the same crate: about 9.5 minutes
  on a GitHub runner (`.github/workflows/deploy.yml` since 2026-10-30), and its length is the
  crate's.

## How a split pays, and when it does not

Two facts decide every boundary:

1. **An edit re-checks the crate it is in and every crate that uses that crate.** So code that is
   big and rarely edited belongs at the bottom, where every edit above it skips it, and the code
   edited most belongs at the top, where an edit re-checks the least.
2. **Crates that do not use each other compile at the same time, on different cores.** Crates
   stacked in a chain do not.

And the reasons not to split:

- **Each boundary is work once:** every reference that crosses it the wrong way has to move first
  (the list below).
- **And a little friction after:** an item used across the line must be `pub`; a change that spans
  two crates edits both.
- **A small piece split off one end of a chain saves almost nothing**, because the chain still
  re-checks above it and nothing gains a core.
- **Speed across crates:** a small function in one crate is only inlined into another with
  `#[inline]` or link-time optimisation; release builds may want `lto = "thin"`, measured with
  `just perf-sweep` before and after.

## The ten crates

| Crate | What is in it | Lines | Commits in 30 days | An edit re-checks | May use |
|---|---|---|---|---|---|
| `humanity-core` | the small leaves, the ECS, data loading, hot reload, storage, cosmos | 11,374 | 50 | 100% | nothing |
| `humanity-protocol` | `relay/core`: keys, signatures, sealed messages, signed objects, encodings | 12,621 | 21 | 66% | core |
| `humanity-ship` | `ship`, `machines`, `station`: the home's structure, its machines, the orbit | 31,980 | 81 | 77% | core |
| `humanity-terrain` | `terrain`: planets, heights, oceans, chunk meshes as plain data | 23,731 | 2 | 86% | core |
| `humanity-systems` | `systems`, `persistence`, `surface_walk`, `surface_move`: the simulation | 78,798 | 108 | 72% | core, ship, terrain |
| `humanity-relay` | `relay`, `how_to`: the server | 110,645 | 142 | 60% | core, protocol, ship, systems |
| `humanity-net` | `net`: the app's side of the network | 30,691 | 58 | 47% | core, protocol |
| `humanity-renderer` | `renderer`, `assets`: uploading and drawing | 68,820 | 37 | 53% | core, terrain |
| `humanity-gui` | `gui`, `config`, `games`, `software`, `media`, `web_reader`, `updater` (since rung 0 part B), `release_update` (since its backlog), and the chat-safety code now in `engine/` (protected mode, warnings, DMs, blocking, reports, reach, key cards, group reports, file transfer, friend codes; since part B also the friend choices, the pass answers, household permits and low impact) | 177,268 | 255 | 42% | all of the above |
| `humanity-engine` | `engine` (THE FRAME), `lib.rs`, `save_load`, the binary | 94,250 | 242 | 15% | everything |

The lines, commits and shares are the 2026-10-30 reading. After rung 0 (2026-10-31,
`node scripts/crate-graph.js --crates`): core 14,577, protocol 12,645, ship 32,680, terrain 25,638,
systems 80,491, relay 110,833, net 31,064, renderer 65,877, gui 183,772, engine 91,183.

"An edit re-checks" is the crate plus every crate that may use it, as a share of all the lines.
Weighted by how often each is edited, **an average edit would re-check about half the code instead
of all of it, and an engine edit (242 commits) about 15%: on the 56 s measured today, roughly 27 s
on average and 8 s for an engine edit. Estimates, to be measured at each rung** (incremental
compilation inside the crates above an edit may make them faster still). Work on the server alone
can check `humanity-relay` by itself (`cargo check -p humanity-relay`, about a quarter of the code);
the server build on GitHub compiles core, protocol, ship, terrain, systems and relay, with no GPU
or GUI code in it at all. In a cold build, ship, terrain, protocol and net compile side by side,
then systems beside the renderer.

## The questions the operator asked (2026-10-30), and the answers

- **Split the simulation into systems, machines and ship?** ship is 24k lines, machines 6k,
  systems 61k; only about 10 references hold them in a loop. Split into a chain they save little on
  their own (a systems edit skips 30k lines), so **ship, machines and the station become one
  crate, `humanity-ship`, beneath `humanity-systems`**: cheap, and it is a real line (what the
  simulation runs on, against the simulation). **The domains inside systems** (farming 15k lines,
  construction 6.6k, crafting 4.5k, food 2.8k, ...) could later become crates side by side, which
  is where the simulation's real gain is, but they reach into each other (food uses 13 other
  domains); **not now**: revisit with the rung-2 measurements.
- **Split terrain from the renderer?** **Yes, as its own crate under both the renderer and
  systems.** For: 23k lines edited twice in 30 days compile once and stay cached, so every edit
  above it skips them; and the line between "the planet's shape and heights" and "drawing it" becomes
  real. Against: 17 references from terrain into the renderer move first (the vertex type, the
  patch slot, the tree card footprint, the projection math), and `planet_chunks.rs` (4,621 lines)
  mixes height math with mesh building and splits in two. (The first draft of this plan cut terrain
  in half between the simulation and the renderer; one crate under both is simpler.)
- **engine, gui, net?** **Yes, all three.** net uses gui once, engine never, config three times: it
  is nearly separate already, and its use of the server is almost all the protocol core
  (`relay::core`, 50 references), which becomes `humanity-protocol` so net and the server compile
  side by side. gui and engine are the most-edited code (255 and 242 commits): gui reached into
  engine 196 times, but most of that was the chat-safety code that sits in engine's folder and
  works on the GUI's state; it moves into the gui crate (measured: 205 wrong-way references with it
  in net, 155 with it in gui). **gui pages as separate crates: not now:** every page uses
  `GuiState` (`gui/mod.rs`, itself edited 92 times in the 30 days).
- **The names** (below).

## The names

**`humanity-` and the module's current name**: `humanity-systems`, `humanity-renderer`,
`humanity-gui`, and so on; the top crate keeps today's package name, `humanity-engine` (the log
targets already read `humanity_engine::`). Why: one concept, one word (CLAUDE.md, the 2026-10-19
naming rule): a crate called `sim` holding a module called `systems` is two names for one thing.
It matches the existing package name and the `HUMANITY_` settings. The `hos-` of the first draft
was short but cryptic. The longer name costs nothing in code: a file imports a crate once.
The two new groupings are named for what they hold: `humanity-core`, `humanity-protocol`.

The crates live in `crates/<name>/` (the Rust convention), with `src/` emptying into them crate by
crate. CLAUDE.md's warning that an agent reporting edits under `crates/` was in a stale worktree
(the v0.90 layout) is rewritten in the same rung.

## The 155 references to re-home first (`node scripts/crate-graph.js --edges`, 2026-10-30)

The list as it was planned. The ratchet's corrected reading (rung 0, below) counted 157 in 19 edges
on the same tree; part A's cuts, where they differ from this table's, are in rung 0's status.

| Edge | Refs | What crosses | The cut |
|---|---|---|---|
| gui -> engine | 58 | page buttons that call the engine directly: save and load (`active_home`, `extract_world_save`, `apply_save_to_world`, save slots), the map feed and tiles, the editor's storey names, the play journal, household, choices, net routes | The page posts a request in `GuiState` and the engine carries it out (the pattern the dev IPC already uses), or a pure helper moves down (the storey names to ship) |
| systems -> renderer | 20 | `assets::loader::parse_ron`, `parse_csv` (18); `Camera::new`, `celestial_projection_f64` | Move the CSV/RON loaders down into core (pure parsing; also fixes ship's and core's uses); the projection math to terrain |
| terrain -> renderer | 17 | `patch_arena::PatchSlot`, `mesh::Vertex`, `tree_mesh::registry`, `CardFootprint`, the projection | Move the plain data types down (the vertex to core, the patch slot and card footprint to terrain); split `planet_chunks.rs` |
| renderer -> systems | 12 | the surface look angles (`surface_walk`), `WeatherCondition` | Move the look math to terrain, the weather condition to core |
| ship -> systems | 8 | `DayEnergy`, `GrownFood`, `DAY_HOURS` (machines) | Move the day-energy and grown-food types down to ship |
| ship -> renderer | 8 | `mesh::Vertex` (7), `parse_ron` | The vertex to core (as above) |
| core -> engine | 6 | hot reload calls the engine (`EngineState`, `reload_planet_defs`, ...) | Invert it: hot reload reports what changed; the engine reacts |
| core -> renderer | 4 | `frame_costs`, the asset loader and watcher | Move `frame_costs` and the file watcher to core |
| net -> gui | 4 | `config::pbkdf2_sha256` (the vault key derivation), a part-file widget | The key derivation to protocol (crypto belongs there); the helper down |
| the rest | 18 | one or two each (`gui::i18n::t` and `RoomBounds` from ship and systems, `frame_lock::ground_radius_m`, `MET_STANDING`, `ShowcaseGarden`, `how_to`'s page list, `games::registry`, `net::live::RawFrame`, `epoch_days_to_ymd`, `WEATHER_MAP_W/H`, `active_home` in machines) | Each moves to the lowest crate that uses it, or the caller passes it in |

## Costs and risks

- **A Cargo workspace** replaces the one `Cargo.toml` (`[workspace]` with ten members). CLAUDE.md's
  Architecture section, the build slot (`rustc-workspace-wrapper` already wraps every member), the
  lint and test recipes, CI's three workflows and the release scripts all change in step.
- **Feature flags become crate choices:** the server build is `humanity-relay` and what it uses,
  instead of `--no-default-features` on one crate.
- **Visibility:** `pub(crate)` items used across a new boundary become `pub`; the move makes those
  uses visible, which is a review, not a risk.
- **Tests** move with their code; a test that reaches across layers becomes a test of the crate
  above.
- **A catch for CI:** a fresh checkout gives every file a new timestamp, so Cargo rebuilds path
  crates whatever the cache holds. The CI gain needs `git restore-mtime` before building, or Cargo's
  content-hash freshness (still unstable). Locally the gain is immediate.
- **No runtime change** is intended anywhere. Each rung keeps the farming golden state
  (`crates/humanity-systems/src/farming/golden_tests.rs`), the lib tests, the relay battery and a
  `just verify-runtime` boot exactly as they were.

## Order of work (each rung shippable alone, and measured)

0. **Inside the one crate, re-home the 155 references** (the table above), a few edges per commit,
   with no Cargo change. Then a layer lint (`tests/layer_lint.rs`) that reads
   `scripts/crate-layers.json` the way `crate-graph.js --edges` does and fails on any new
   wrong-way reference, so the cycle cannot grow back while the rest is done. This rung is worth
   doing even if a later rung stalls: it untangles the dependency structure.
   **The ratchet is built (2026-10-30):** `scripts/check-crate-layers.js`, in `just lints`, holds each
   wrong-way edge to its budget in crate-layers.json's `budgets` (today's 155 in 22 edges): an edge
   over its budget or a new one fails; one under asks for its budget to come down in the same commit
   (`--write` sets them all to today's counts).
   **Part A is done (2026-10-31): 62 references in 3 edges remain, all part B's.**
   First the ratchet's reading was corrected (`scripts/crate-graph.js`): it had stopped reading a
   file at its first `#[cfg(test)] mod`, so a test file declared near the top of a file
   (systems/farming/mod.rs) hid eleven real `frame_costs` calls, and it counted tests gated
   `cfg(all(test, ...))` and files named plain `tests.rs` as code. Read as rustc reads them, the
   same tree held 157 references in 19 edges (not 155 in 22). Then, a few edges per commit, each
   reference went to the lowest crate that uses it, or the caller now passes the value in:
   - **into the core:** the data parsers (`data_parse`, was `assets::loader`), the mesh vertex
     (`vertex`, its wgpu layouts staying in the renderer as `mesh::vertex_layout` and
     `instance_layout`), the camera's aim math (`camera_angles`, was in `surface_walk`), the fall
     physics (`physics::landing`, was in `systems::falls`), the cost store (`frame_costs`; its GPU
     half is `renderer::gpu_costs`), the file watcher (`hot_reload::watcher`), the met table, the
     eye height and a structure's box (`ecs::components`), the household permit and the refusal
     codes (`build_permission`), the crash-safe write (`atomic_write`), the weather map's size
     (`weather_map`), the civil date (`civil_date`), the stream's frame (`raw_frame`), the Windows
     registry reader (`platform::registry`) and the word source (`i18n`, was `gui::i18n`);
   - **into terrain:** the plant geometry builder (`plant_mesh`), the tree species and card atlas
     tables (`tree_registry`), the patch slot, Earth's scale-height ratio and the celestial
     projection (`celestial_projection`);
   - **into the ship:** the machines' day record (`machines::day_energy`), `GrownFood`, the door
     storey band (`door_panels::feet_reach_door`) and `RoomBounds` (`room_types`);
   - **into the protocol:** the vault's key derivation (`relay::core::kdf::pbkdf2_sha256`, the
     algorithm and its 600,000 rounds unchanged); **into net:** the part file and the voice modes;
   - **the caller passes it in:** the camera's fall step takes numbers (`RadialStep::landing`), the
     weather region takes any comparable condition, `home_ron_path` takes the start and household
     (`save_load::active_home_machines_path`, `persistence::live_save`'s since part B),
     `MachineHome::load` no longer fills the food
     figures (`systems::grow_machines::load_home` does, for every caller above the ship, and a
     test holds them to it), and hot reload's `poll` returns the changed files instead of
     invalidating the asset manager itself.
   **Part B is done (2026-10-31): 0 references in 0 edges** (gui -> engine 55, core -> engine 6,
   core -> ship 1, all gone; the budgets are empty, so any wrong-way reference now fails
   `just lints`). Every reference was a direct call and stays one: no request was turned into a
   next-frame action, so nothing a person sees moved by a frame. The cuts, a theme per commit:
   - **hot reload's gates to the engine:** `poll_and_apply` takes the engine's state, so it is
     `engine::hot_reload_gates`; the core's `hot_reload` keeps the coordinator and the watcher.
   - **files that work on the GUI's state, listed under the GUI** (crate-layers.json): the friend
     choices (`engine/choice`), the server's answers to the passes (`engine/put_answer`), the
     household permits (`engine/household`, its running-world half split out to the engine as
     `engine/household_world`: the open plots and the dev IPC's report), low impact
     (`engine/low_impact`, the rig's overlay on config.json) and the updater (`updater`, the
     Settings > Updates backend that GuiState holds; the launch policy's answer is now handed to it,
     `Updater::relaunch_behind`, and to the host node's autostart);
   - **into the core:** the play journal's store and pure watcher (`play_journal`; the mark's
     picture and the frame watcher stay in `engine::play_journal_frame`);
   - **into the ship:** the elevators as data (`ship::lifts`: `Lift`, `LiftKey`, `lifts`) and what
     a storey edit does to a home (`ship::storey_edits`: `EditStorey`, the storey and slab names,
     a new wall, guard or floor slab, `SlabDraw`, the cut-away's box `CutAway`);
   - **into the GUI's own pages:** the plan tiles' grid (`gui::pages::maps::tile_grid`:
     `TileKey`, the zoom, which tiles cover a view), the build editor's choices on the GUI state
     (`gui::pages::construction::storeys`: the storey, the slab, the tool, `editor_cut_of`), the
     picker's questions (`where_you_live`: `server_slot`, `ask_where_you_live`,
     `choose_on_server`), Settings' Restore (`restore_snapshot_into_game`), the connected row's one
     constant (`showroom::CONNECTED_SERVER_ID`) and the server the game talks to
     (`GuiState::active_server_key`);
   - **into the simulation, beside the save files:** which save is live and what waits for which
     save (`persistence::live_save`: `ActiveHome`, the restore hold, "Snapshot now", the choices
     waiting for a fresh save, `slot_for`, an erased account's save set aside), from save_load.rs
     and engine/save_slot.rs.
   **Not counted, and needed before each crate is cut (rung 1 and after).** The backlog as rung 0
   left it, and what became of each item (2026-10-31, the same day):
   - **The tests' 127 wrong-way references: 0.** (The count was 125
     after part B, then 127 when the crate cards gave a `#[path]` file to the module declaring it:
     the GUI's eight test files in `engine/` had been counted as the engine's.) Each test that made
     one moved WHOLE to the lowest crate that may use everything it reaches, into one
     `crossing_tests` folder or file per destination crate, declared once in that crate's root
     module, a file per file the tests came from: src/engine/crossing_tests/ (19 files, two of them
     whole files moved with git mv: `engine/parity_tests.rs`, `systems/quests/opening_tests.rs`),
     src/gui/crossing_tests/ (13), crates/humanity-systems/src/crossing_tests/ (7), src/relay/crossing_tests/ (2)
     (the engine's 25 files since the farm's six joined it, below)
     and one file each in net, renderer, terrain and the ship. Nothing in a test changed but its
     paths; the how-to checker's nine page-reading tests run without the app's page list in the
     relay's crate and again with it in the GUI's (`gui/crossing_tests/how_to.rs`), and the station
     tests take their own 24-hour day instead of the simulation's constant. **The farm's last 15**
     (crates/humanity-systems/src/farming/heat_tests.rs 6, life_support_tests.rs 3, mod.rs 3, humidity_tests.rs 1,
     soil_ph_tests.rs 1, weed_tests.rs 1; all systems -> engine, the engine's home spawn and save and
     load) moved the same way the same evening, once the farm's session finished with those files:
     to src/engine/crossing_tests/farming_heat.rs (eight tests, with the stove and heat-pump helpers
     only they use), farming_life_support.rs (the `shipped` module's four, with `shipped_home` and
     the day it settles), farming_gardening.rs (two of mod.rs's `gardening_tests`),
     farming_humidity.rs, farming_soil_ph.rs and farming_weeds.rs (one each); the helpers they share
     with the tests that stayed are the origin files', `pub(crate)`. The same test names before and
     after (`node scripts/lib-tests.js --list`). **The ratchet holds test code**, file by file
     (crate-layers.json "testBudgets", empty now), and code file by file too; `node
     scripts/crate-graph.js --tests` lists each reference with its line.
   - **`crate::data_dir()` is the core's: `storage::data_dir()` and `storage::DATA_DIR`**, byte for
     byte, every caller changed (52 files), and the 11 files fenced for the farm and Garden session
     (engine/home_meshes.rs, gui/pages/inventory.rs, nine under systems/farming/) the same evening, so
     lib.rs's temporary `pub(crate) use crate::storage::data_dir;` is gone and crate-layers.json's
     "budgets" is empty. Its ten callers below the engine had been counted and held to one each
     until then, because **the counting reads the crate root's own items** (`rootItems`: a path to a
     fn, static, const, type or re-export of lib.rs is a reference into the crate owning lib.rs).
     Until then `crate::data_dir()`, `crate::DATA_DIR` and a test calling the engine's
     `crate::drone_dock_visible` from the ship named no module and went uncounted.
   - **main.rs is counted:** the binary's `humanity_engine::x` is read as the library's `crate::x`,
     from the engine (crate-graph.js `cargoTargets`), so its uses of `release_update` and the relay's
     restore are in the interface tables.
   - **`release_update` is the GUI's, not the relay's:** native-only, never called by the server,
     used by the updater and the binary.
   - **One cut-away box:** `ship::levels::LevelCut` is gone; `level_cuts` returns
     `ship::storey_edits::CutAway`, and engine/map_cut.rs no longer converts.
   - **Still open: the engine-folder files the plan gives the GUI** (the chat-safety files,
     `choice`, `put_answer`, `household`, `low_impact`) move into `crates/humanity-gui/` when that
     crate is cut (rung 4); their `crate::engine::` paths change then.
   - **Still open: `pub(crate)` items used across a planned boundary** become `pub` as each crate is
     cut. The test moves added about 90 (mostly test helpers, a few private constants and
     `storage::extract_embedded_to`).
   **New, found clearing it, and needed for rung 1 itself:**
   - **Test helpers across a boundary.** A cut crate's `#[cfg(test)]` items are not compiled into
     another crate's test build. 412 test references reach another crate's test-only module: 305 of
     them the core's own `test_temp`, `test_clock` and `test_log`, used by every crate's tests, so
     the core needs a `test-support` feature (or a dev-only crate) before rung 1 can cut it; the
     other 107 are module fixtures (the put_answer, trade, wounds, spoilage, reports, profile-copy
     and household test helpers the moved tests share with the tests that stayed, the Maps camera's
     `feed_at`, ...), which become a `fixtures` feature of their crate at its cut.
     `node scripts/crate-graph.js --fixtures` lists them, by crate and module.
   - **The repo root, found from a crate.** 565 `env!("CARGO_MANIFEST_DIR")` in 251 files (most of
     them tests reading data/) mean the repo root today; in a workspace each member's is its own
     folder. And 251 `include_str!`/`include_bytes!("../...")` paths are relative to the file, so
     each breaks when its file moves into `crates/<name>/src/`. Rung 1 needs one way to name the repo
     root before it moves the first file.
   - **Eight tests left the relay battery** (3,194 then 3,186, measured from the battery's test
     list): checking a lower crate against the renderer, whose modules the server build still
     compiles today, is now the GUI's (surface_move's five wish-direction tests, env_layer1's shader
     twin, the ship's lighting watts), and the drone dock test reads the engine's
     `drone_dock_visible` (lib.rs). `just test-lib` still runs all eight; in the plan neither the
     renderer nor the engine is under the relay, so none could stay.
1. **The workspace, `humanity-core` and `humanity-protocol`** (the smallest and lowest). Measure a
   cold build and a one-line edit against 241 s and 56 s.
   **Done (2026-11-01), in six steps, a commit each:**
   - **The test helpers across the boundary: a `test-support` feature** of each cut crate, not a
     dev-only crate. crate-layers.json already filed `test_temp`, `test_clock`, `test_log` (and
     since BUG-222 `test_gpu`) under the core, so the plan stays true; one mechanism serves the
     core's helpers and every later crate's fixtures (one pattern, one name: later rungs use
     `test-support` too, not a separate `fixtures` feature); and it adds no eleventh crate. A
     dependent turns it on in its `[dev-dependencies]` only, which resolver 2 never unifies into a
     product build. The fixtures other crates' tests shared moved out of their test modules into
     `fixtures` modules beside them (`storage::fixtures`, `dev_travel::fixtures`,
     `pq_crypto::fixtures`, `profile_copy::fixtures`; the protocol's `strip_heif_metadata::build`
     already was one): a whole test module behind the feature would leave its private helpers
     unused, and warning, in a non-test build. Of the 409 cross-crate test-helper uses
     (`--fixtures`, 2026-11-01), the 307 uses of the three helpers and the 14 of the core's and the
     protocol's fixtures are behind the feature; the other 88 are between crates cut later.
   - **One way to name the repo root: `env!("HUMANITY_REPO_ROOT")`,** set by each member's build
     script (the root's `build.rs`; `crates/repo_root.rs`, which both crates share, finds the
     nearest folder whose Cargo.toml holds `[workspace]`). 566 `env!("CARGO_MANIFEST_DIR")` in 255
     files and 253 `../` include paths in 77 files changed before any file moved;
     `tests/repo_root_lint.rs` holds it (819 places red on main's tree, red again on a plant).
     Not `[env]` with `relative = true` in .cargo/config.toml: that file is machine-local and in
     the main checkout only, so every worktree would have read the main checkout's data/,
     silently, and CI has none. Not a const in the core: `include_str!` takes a literal. One path
     the lint does not refuse broke in the move and was fixed by hand: an include relative to its
     file but into ANOTHER module's folder (`relay/api_v2_objects.rs` read
     `core/pq_kat_vote_submission.json`). Rung 2 should look for those (`include_str!("x/...")`
     whose folder is another crate's) before it moves.
   - **The workspace:** `[workspace]` with the root package (still `humanity-engine`) and the two
     crates, files moved with `git mv` (history follows: `git log --follow`). `[workspace.package]`
     holds the one version (the core's own code reports the app's version, so every member carries
     it; a bump rebuilds every member, as before) and `[workspace.dependencies]` the version and
     features of the 40 dependencies more than one member uses. The root's `native` forwards to both
     crates' `native`, `relay` turns the protocol on, `wasm` the core's; the server build gets the
     core with no feature and the protocol without `native` (the core then depends on blake3,
     bytemuck, csv, glam, hecs, log, rapier3d, ron, serde, serde_json and toml only; egui is not in
     the server's graph; wgpu still is, through the root package, as before). The protocol's code
     uses no crate of ours (only its tests use the core), so it compiles BESIDE the core. Callers
     name the crate (3,080 paths in 577 files); no re-export keeps a `crate::` path alive. Made
     `pub` for another crate: `frame_costs::{publish_gpu_frame, publish_gpu_harvest,
     set_gpu_untimed, dump_enabled}` (the renderer's half reads them through a glob import) and
     `storage::extract_embedded_to`, plus the test helpers and fixtures. Moved because a crate may
     define methods only on its own types: `Npk`'s arithmetic, from farming's soil.rs to the core
     beside the type. The same 5,586 lib-test names as main (c8f05d953): engine 5,256, core 171,
     protocol 159, the protocol's with their `relay::core::` prefix dropped and nothing else.
   - **Every tool that knows where code lives** (the step's commit lists them): the lib-test runner
     builds and runs every member's program, the warnings ratchet counts every member, the source
     stamp, the deploy plan, the layer tools (which now also hold each member's Cargo.toml to the
     plan), the cards, the code map, the include classifier, the build slot's names, the release
     scripts, and every lint that walks the source, each seen red on a violation planted under
     crates/ (four of them passed the same plants before the step: blind). Three lib tests that
     read source paths failed the first full run and were fixed with them.
   - **The docs that describe the layout:** CLAUDE.md (Architecture: a workspace; the stale-worktree
     warning kept for `native/src/`, `server/src/` and any `crates/` folder the workspace does not
     list), builds.md ("The workspace"), the cards, STATUS and the contributor and AI onboarding
     docs, and every path to a moved file (link targets only in the records).
   - **Measured: no change worth the name, as "How a split pays" predicts for a small piece off
     one end of a chain.** Main (c8f05d953) and the branch in the same worktree the same evening,
     the machine quiet (no other compile, no game, about 23 GB of RAM free), `CARGO_BUILD_JOBS=4`,
     each compile timed by the build slot's own line (so no memory wait is counted), the same
     comment line appended to `civil_date.rs` (core) and `engine/launch_focus.rs` (engine);
     `node scripts/build-ab/split-measure.js` repeats it for each rung:

     | Build | main, one crate | rung 1, the workspace |
     |---|---|---|
     | `cargo check --features native`, cold | 74 s (wall 77 s) | core 3 s and protocol 1 s side by side, then the engine 73 s (wall 79 s) |
     | the same, one line in the core | 18 s (wall 21 s) | core 0 s, engine 18 s (wall 21 s) |
     | the same, one line in the engine | 18 s (wall 21 s) | engine 18 s (wall 21 s) |
     | the lib test build, cold | 237 s (wall 241 s) | core and protocol 5 to 8 s, then the engine 242 s (wall 252 s) |
     | the same, one line in the core | 53 s (wall 56 s) | core and protocol 2 s, engine 55 s (wall 59 s) |
     | the same, one line in the engine | 53 s (wall 56 s) | engine 55 s (wall 59 s) |

     The engine is still about 95% of the lines, and every edit re-checks it; the two crates
     (about 27,000 lines) compile in seconds. The test build's 2 s per edit are the engine's test
     program now linked with its debug file (one `cargo test --no-run` for every member, in place of
     `cargo rustc ... /DEBUG:NONE`): the old command on the split tree took 53 s, the same as main.
     Cold, the core's library now sits before the engine on the critical path (6 s). Against the
     plan's figures: 241 s cold and 56 s for an edit (2026-10-24) read 237 s and 53 s on main a week
     later. **Compare quiet-machine numbers only:** the first "before" run, beside other sessions'
     compiles, read 86 and 88 s for the same edits main compiles in 53 s. The gains start at rung 2,
     which takes ship, terrain and systems (about 137,000 lines) out from under the engine.
2. **`humanity-ship`, `humanity-terrain`, then `humanity-systems`.** Measure.
   **Done (2026-11-02), in seven steps, a commit each:**
   - **The fixtures first** (step 1). Every helper another planned crate's tests took from a test
     module of the three moved out of it into a `fixtures` module beside it, `pub`, before any file
     moved: `standing`, `sight`, `moves` and `door_panels` in the ship; `far_trees` and
     `drawn_surface` in the terrain; `surface_move`, `env_layer1`, `food::fixtures::{wounds,
     spoilage}` and `farming::fixtures::{gardening, heat, humidity, life_support, soil_ph, weeds}`
     in the simulation. At the cut each is gated `any(test, feature = "test-support")`, rung 1's
     mechanism and name. `crate-graph --fixtures` listed 44 of these references; it did not list
     two more kinds, found by compiling the moved tree: test-gated ITEMS in non-test modules
     (`ShipStructure::load_and_assemble_shipped`, `MealRules::built_in`,
     `status_effects::with_old_dispel_type_column`, `near_trees::frame_loop_source`,
     `water_carve::PublishedForTest`) and a test module reached through a `use` group
     (`use farming::{self, step_tests, ..}`: the step tests' garden and its comparison, and the golden
     garden they start from, now `farming::fixtures::{step, golden}`).
   - **What would break, fixed on the old layout** (step 2): the move was first made once, compiled
     in both feature sets, and every fix replayed before the move, so steps 3 and 4 are the moves.
     46 items (fields and methods counted) became `pub` because another planned crate uses them
     (the build editor's pure half, `storey_edits`; the plant mesh builder's primitives;
     `HomeStructure::material_color`;
     `persistence::{civil_from_days, days_from_civil}`; `quests::{PLANT_EVENT, counted}`;
     `farming::{SECONDS_PER_DAY, stage_index}`, `FarmingSystem::reuse_override`; the test-only items
     above; the full list is the step's commit). The relay's two methods on the ship's `ShipPlots`
     (E0116) became the relay's functions of the same names. Two dead-code allows that only held
     while caller and callee shared a crate went. No `include_str!` reached into another module's
     folder this time, and no inherent impl outside the relay's.
   - **The cut** (steps 3 and 4): `git mv`, each root module's folder the crate's `src/` and its
     `mod.rs` the `lib.rs`, so the modules are the crate's (`humanity_ship::ship_structure`,
     `humanity_terrain::planet`, `humanity_systems::farming`) and the extra modules keep their names
     (`humanity_ship::machines`, `humanity_ship::station`, `humanity_systems::persistence`,
     `::surface_walk`, `::surface_move`); 1,050 + 388 + 1,968 paths rewritten, no re-export. The
     spline routing's rung 0 files that landed on main that day moved with the ship. Each crate's
     `native` (its default) forwards to the crates under it; the root's `native` turns on all
     three; nothing in them names `relay` or `wasm`, so the server build compiles them whole but
     their `native` parts (the ship's `RoomBounds` and `machines::home_ron_path`, the terrain's OSM
     mesher, the simulation's live save, server save names, named rooms and home-store words). None
     of them depends on egui, winit or wgpu. noise and rand joined `[workspace.dependencies]`.
   - **What the moves broke that compiling did not show.** The full lib-test run on the moved tree
     failed six: a ship test that searched the engine's lib.rs for `crate::ship::sight::...`, which
     the path rewrite had turned into `crate::sight::...` (it now looks for
     `humanity_ship::sight::...`), the terrain's one-reader-of-the-density test, which read its own
     file at the old path, and four how-to tests (below). Reading every source walk in the lib tests
     found two that passed blind, walking `src/` alone: the simulation's no-bare-`MachineHome::load`
     test and the net's no-age-API scan (blind to the core and the protocol since rung 1); they and
     the density test walk every crate now. **And where a test starts:** cargo starts each test
     program in its own package's folder, where no data/ sits, while lib-tests started every
     program from the repo root. So the full lib-test run passed and the relay battery failed four
     simulation tests whose notices the server build reads from data/i18n/en.json, unseen at first
     because cargo stopped at main's own failing core test; 20 more had been handing
     `ElectricalSystem` a bare `"data"` and passing on an empty table. Now a test build's
     `storage::data_dir()` falls back to the repo's data/, those tests name it, and lib-tests starts
     each program in its member's folder, as cargo does. Paths that followed the files: the golden test's recorded state,
     the grass coverage scan's files, and the how-to whose source cites `src/persistence.rs` (the
     checker refuses a source the repo does not have; the one changed path rebinds its review hash,
     `node scripts/build-how-to.js`).
   - **Every tool that knows where code lives** (step 5), each source-walking check seen red on a
     violation planted under the new crates (the step's commit lists them with their red lines).
   - **The docs** (step 6): CLAUDE.md's Architecture paragraph, builds.md, the cards, the code map,
     and every path to a moved file across docs/, data/, web/, scripts/, schemas/ and the code's own
     comments (link targets only in the records).
   - **Measured: an edit compiles about a quarter faster, with 138,000 lines out from under the
     engine.** `cargo check` of an engine edit went from 18 s to 14 s, the test build from 55 s to
     42 s, the cold test build from 264 s to 227 s; the cold check stayed at about 82 s, because the
     crates sit on its critical path in front of the engine (core 3 s, ship 5 s beside the
     terrain's 1 s, systems 9 s, then the engine 61 s). Main (36f20b942) and the branch in the same
     worktree the same day, two and a half hours apart, `CARGO_BUILD_JOBS=4`, each compile timed by
     the build slot's own line (so no memory wait is counted), the same comment line appended to the
     simulation's `time.rs` and to `engine/launch_focus.rs`:

     | Build | main, rung 1 | rung 2 |
     |---|---|---|
     | `cargo check --features native`, cold | core 3 s and protocol 1 s, then the engine 74 s (wall 83 s) | protocol 1 s, core 3 s, terrain 1 s, ship 5 s, systems 9 s, then the engine 61 s (wall 82 s) |
     | the same, one line in the simulation | engine 18 s (wall 21 s) | systems 2 s, engine 14 s (wall 19 s, leaving out a 38 s wait for memory behind another session's compile) |
     | the same, one line in the engine | engine 18 s (wall 21 s) | engine 14 s (wall 17 s) |
     | the lib test build, cold | core and protocol 5 to 8 s, then the engine 254 s (wall 264 s) | core, protocol and terrain 4 to 12 s, ship 17 and 21 s, systems 22 and 35 s (library, then test program), the engine 182 s (wall 227 s) |
     | the same, one line in the simulation | engine 56 s (wall 59 s) | systems 3 and 6 s, engine 42 s (wall 48 s) |
     | the same, one line in the engine | engine 55 s (wall 59 s) | engine 42 s (wall 45 s) |

     The engine went from about 95% of the code to 74.9% (496,000 of 662,000 lines: ship 33,000,
     terrain 25,600 and the simulation 79,500 out), and every edit still re-checks it: a
     simulation edit now costs its own crate's few seconds plus the engine's 14 s, where it cost
     the engine's 18 s. The crates' test programs compile beside the engine's. **The conditions**
     (compare quiet-machine rows only): main's rows ran beside the operator's open HumanityOS
     (about 0.8 of a core), no other compile, 21.4 to 21.8 GB free; the branch's check rows began
     with one or two other sessions' compiles on the machine (16.9 to 21.8 GB free) and its test
     rows with none and no game (22.1 to 22.3 GB free). A first branch attempt, beside another
     agent's lib test build, read 76 s for the cold engine check and is not used. The next gains
     are rung 3's: the relay, the net and the renderer are about 207,000 of the engine's 496,000
     lines (the 2026-10-31 reading above).
3. **`humanity-relay`, `humanity-net` and `humanity-renderer`.** Measure the server build on
   GitHub.
4. **`humanity-gui`**, leaving **`humanity-engine`**, the app and the binary. Measure an engine
   edit against the 8 s estimate.
