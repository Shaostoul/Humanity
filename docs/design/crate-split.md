# Splitting the crate: what goes where, and in what order

Status: **the split is decided (operator, 2026-10-30): "The split makes sense. Saves us compute.
Saves electricity. Makes development a little bit faster."** It reverses the 2026-06-30 one-crate
decision (CLAUDE.md, Architecture: "one `Cargo.toml` at the repo root, no workspace, no
sub-crates"), which changes when rung 1 lands, not before. **The ten crates and the names are his
decisions too, the same day:** "I like the idea of going the full 10. It sounds like it'd be the
most beneficial over a long period of time. Feels kind of small now but, huge a year from now."
and, of the names, "Let's go with humanity." **Rung 0 is done (2026-10-31):** parts A and B took
the wrong-way references in code from 157 to 0, and the ratchet holds every edge at zero; rung 1 is
next (Order of work, below).

The plan's one source is `scripts/crate-layers.json`: which crate every part of `src/` goes to and
which crates each may use. `node scripts/crate-graph.js --crates` prints each crate's size, and
`--edges` lists every reference that points the wrong way under the plan (the rung-0 work list).
Every number here comes from those, from `git log` over the 30 days to 2026-10-30, or from
`docs/ai/builds.md` ("Measured 2026-10-24"). Re-run before acting: the code moves daily.

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
| `humanity-relay` | `relay`, `how_to`, `release_update`: the server | 110,645 | 142 | 60% | core, protocol, ship, systems |
| `humanity-net` | `net`: the app's side of the network | 30,691 | 58 | 47% | core, protocol |
| `humanity-renderer` | `renderer`, `assets`: uploading and drawing | 68,820 | 37 | 53% | core, terrain |
| `humanity-gui` | `gui`, `config`, `games`, `software`, `media`, `web_reader`, `updater` (since rung 0 part B), and the chat-safety code now in `engine/` (protected mode, warnings, DMs, blocking, reports, reach, key cards, group reports, file transfer, friend codes; since part B also the friend choices, the pass answers, household permits and low impact) | 177,268 | 255 | 42% | all of the above |
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
  (`src/systems/farming/golden_tests.rs`), the lib tests, the relay battery and a
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
   **Not counted, and needed before each crate is cut (rung 1 and after):**
   - **The tests** hold 125 wrong-way references of their own (2026-10-31, after part B; 139 after
     part A): systems -> engine 27, gui -> engine 20, ship -> systems 19, systems -> gui 12, ship ->
     engine 8, net -> relay 7, systems -> relay 7, ship -> renderer 5, systems -> renderer 5, and
     smaller ones. A test moves up with the crate it reaches, or reaches through the crate under it.
   - **`crate::data_dir()`** (lib.rs, the engine) is read by about 50 files under the engine (the
     GUI 22, the simulation 20, the ship, the relay, net, media, the word source, cosmos, and the
     engine-folder files the GUI owns, such as low impact); no path counts it, but it must move to
     the core at rung 1.
   - **The engine-folder files the plan gives the GUI** (the chat-safety files, `choice`,
     `put_answer`, `household`, `low_impact`) move into `crates/humanity-gui/` when that crate is
     cut (rung 4); their `crate::engine::` paths change then.
   - **`pub(crate)`** items now used across a planned boundary (most of the moved items kept their
     visibility) become `pub` as each crate is cut.
   - A small cleanup the moves made visible: `ship::levels::LevelCut` and `ship::storey_edits::CutAway`
     are the same box (min, max, cut height), and engine/map_cut.rs converts one to the other.
1. **The workspace, `humanity-core` and `humanity-protocol`** (the smallest and lowest). Measure a
   cold build and a one-line edit against 241 s and 56 s.
2. **`humanity-ship`, `humanity-terrain`, then `humanity-systems`.** Measure.
3. **`humanity-relay`, `humanity-net` and `humanity-renderer`.** Measure the server build on
   GitHub.
4. **`humanity-gui`**, leaving **`humanity-engine`**, the app and the binary. Measure an engine
   edit against the 8 s estimate.
