# humanity-ship

## What it is for

The home and the ship as places a person stands in. It holds the home's walls, doors, storeys and
rooms (`home_structure`, `rooms`, `room_types`), the ship's zones, corridors and hull
(`ship_structure`, `hull`), where a person can stand, see and walk aboard (`standing`, `sight`,
`wall_collision`, `ship_space`, `moves`), the starts a new player can choose (`starts`), the
machines a home holds and what they drew over a day (`machines`), the rules, fittings, anchors and
benders that runs (pipes, hoses, cables) are checked and built by (`run_rules`, `run_fittings`,
`run_anchors`, `run_benders`, data/piping/; spline routing rung 0), and the homestead station's orbit
(`station`). It is plain data and geometry with no GPU in it, and the server compiles it too, so the
shared ship's rules are the same on both sides.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-ship/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-ship/src/assembly.rs`, `crates/humanity-ship/src/build_frames.rs`, `crates/humanity-ship/src/conduits.rs`, `crates/humanity-ship/src/pipe_materials.rs`, `crates/humanity-ship/src/run_rules.rs`, `crates/humanity-ship/src/run_fittings.rs`, `crates/humanity-ship/src/run_anchors.rs`, `crates/humanity-ship/src/run_benders.rs`, `crates/humanity-ship/src/pipe_marking.rs`, `crates/humanity-ship/src/door_panels.rs`, `crates/humanity-ship/src/door_points.rs`, `crates/humanity-ship/src/fibonacci.rs`, `crates/humanity-ship/src/home_structure.rs`, `crates/humanity-ship/src/levels.rs`, `crates/humanity-ship/src/lifts.rs`, `crates/humanity-ship/src/hull.rs`, `crates/humanity-ship/src/layout.rs`, `crates/humanity-ship/src/lock_types.rs`, `crates/humanity-ship/src/moves.rs`, `crates/humanity-ship/src/neighbours.rs`, `crates/humanity-ship/src/ship_space.rs`, `crates/humanity-ship/src/transit.rs`, `crates/humanity-ship/src/rooms.rs`, `crates/humanity-ship/src/ship_structure.rs`, `crates/humanity-ship/src/standing.rs`, `crates/humanity-ship/src/storey_edits.rs`, `crates/humanity-ship/src/starts.rs`, `crates/humanity-ship/src/structure.rs`, `crates/humanity-ship/src/sight.rs`, `crates/humanity-ship/src/wall_collision.rs`, `crates/humanity-ship/src/room_types.rs`, `crates/humanity-ship/src/machines.rs` + `crates/humanity-ship/src/machines/`, `crates/humanity-ship/src/station/`
  - test builds only: `crates/humanity-ship/src/homestead_layout_tests.rs`, `crates/humanity-ship/src/crossing_tests.rs`
- **Size:** 33,195 lines in 41 files, 1,430 of them in 4 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `crates/humanity-ship/src/levels.rs` 184 of 200.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `test-support` 5, `native` 3.
- **May use** (the plan): core. **Uses today** (references in non-test code): core 57.
- **May be used by** (the plan): systems, relay, gui, engine. **Used today by:** engine 330, gui 162, relay 39, systems 26.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 382 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-ship
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_ship::`):

| Item | References | From |
|---|---:|---|
| `ship_structure::ShipStructure` | 25 | engine 16, gui 4, systems 3, relay 2 |
| `machines::MachineHome` | 20 | engine 15, gui 4, systems 1 |
| `room_types::RoomBounds` | 16 | gui 12, engine 3, systems 1 |
| `lock_types::LockState` | 13 | engine 8, gui 5 |
| `machines::PlacedMachine` | 12 | engine 12 |
| `machines::ConduitEnd` | 11 | gui 7, engine 4 |
| `ship_structure` | 10 | engine 8, gui 1, relay 1 |
| `structure::structure_type` | 10 | engine 5, gui 4, relay 1 |
| `door_panels::PanelPlacement` | 9 | engine 9 |
| `home_structure::HomeStructure` | 9 | engine 8, gui 1 |
| `machines::MachinePower` | 9 | gui 6, systems 2, engine 1 |
| `machines::RoomGeom` | 8 | engine 8 |
| `starts::Starts` | 8 | engine 4, relay 4 |
| `station::to_hull` | 8 | engine 8 |
| `structure::zone_type` | 8 | gui 7, engine 1 |

And 192 more items, 381 references (`node scripts/crate-cards.js --interface humanity-ship` lists them all).
<!-- end generated -->

## Rules

- **It may use only core.** The simulation (systems) sits above it and reaches down, never the
  other way: the machines' day record and `GrownFood` moved down here in rung 0 for that reason.
  Source: [crate-split.md](../design/crate-split.md), rung 0, and the ratchet in `just lints`.
- **Data, not code.** Room types, locks, structural pieces, pipe materials, the starts and the
  machines are data files (`data/blueprints/lock_types.ron`, `data/homes/starts.ron`,
  `data/machines/ship.ron`, `data/stations/home.ron`, ...); a new kind of thing is a new row, not a
  new arm in a `match`. Source: [infinite-of-x.md](../design/infinite-of-x.md).
- **The server compiles it,** so nothing here may need a native-only dependency outside a
  `#[cfg(feature = "native")]` gate, and a change here wants `just verify-relay`, whose battery runs
  this crate's tests in the server's feature set. Source: [CLAUDE.md](../../CLAUDE.md), Quick
  orientation (`verify-relay`) and Known gotchas.
- **A saved shape changes outright.** Home designs and saves are persisted formats: no migration
  code before v1.0, except version-tagged blueprint schemas that must keep loading old player saves.
  Source: [CLAUDE.md](../../CLAUDE.md), "No backwards-compatibility debt before launch".
- **Build the real thing.** Walls are real assemblies (`assembly.rs`), pipes carry real marking
  bands (`pipe_marking.rs`), the station keeps a real orbit. Source: [CLAUDE.md](../../CLAUDE.md),
  "Realistic-first".
- **Every row of a run catalog names its source** (data/piping/run_rules.ron, fittings.ron,
  anchors.ron, benders.ron, and the core's pipe_materials.ron): a finding R1..R44 or D1..D19 of the
  2026-11-01 findings, or `Ours("why")`; each rule also says how binding it is where its profile
  applies (`Standing`), and the editor calls only `Law` "code". The tests read the findings
  documents' numbered paragraphs, so a row naming a finding that does not exist fails. Source:
  [spline-routing.md](../design/spline-routing.md), sections 5.3 and 7.1.
- **The station's orbit is planet-scale math:** positions and directions stay in f64 to the sampler.
  Source: [renderer.md](../ai/renderer.md), "f32 at planet scale".
- **Moving aboard is checked on the state a player reaches,** in a booted game, one game at a time
  and never taking focus. Source: [CLAUDE.md](../../CLAUDE.md), "The operator's machine", and
  [game-boots.md](../ai/game-boots.md).

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance; the structure's own test files are
  `home_structure_mesh_tests.rs`, `homestead_layout_tests.rs` and `levels_tests.rs`.
- **The server's battery:** `just verify-relay`.
- **After editing its data files:** `just validate-data`.
- **In the running game** (each boots one game in its own sandbox; `--dry-verdict <manifest.json>`
  re-judges a run without booting): `just verify-storeys` (the upper floor, the stairs, elevator-1,
  ladder-1), `just verify-falls` (the stairs' top and the ladder, with the simulation's falls),
  `just verify-starts` (the Cabin start, `starts.rs`), and `just verify-household` and
  `just verify-shared-build` (plots, permits and building only on your own plot). Their judges'
  tests run in `just rig-tests`:
  [storeys-judge.test.js](../../scripts/tests/storeys-judge.test.js),
  [falls-judge.test.js](../../scripts/tests/falls-judge.test.js),
  [starts-judge.test.js](../../scripts/tests/starts-judge.test.js),
  [household-judge.test.js](../../scripts/tests/household-judge.test.js),
  [shared-build-judge.test.js](../../scripts/tests/shared-build-judge.test.js). Which rig fits a
  change: the `game-checks` skill.

## Open work

- **Increment 7's follow-ups still open** ([PRIORITIES.md](../PRIORITIES.md), RESUME HERE,
  "Increment 7 follow-ups"): `moves::on_my_storey` and `moves::stands_in_storey_of` (used by the
  relay's reach and the quests) assume 3 m storeys, since the relay holds no slabs; pieces built with
  the construction system (the engine's `build_place::built_piece_boxes`) are unknown to
  `ship::standing`; in the ground storey's cut-away the greenhouse risers to g1 and g2 are cut at
  3.0 m, 0.24 m short of the ports drawn over them; a jump from the stair's seventh step can slip
  past a slab edge.
- **Spline routing rung 1 next** ([spline-routing.md](../design/spline-routing.md), section 18):
  the `Run` model replaces `connections` and `conduit_edges`; `ConduitKind`, the catalog's `kinds`
  rows and `pipe_materials.rs`'s view go with it. Rungs 3, 7 and 8 read `run_benders`,
  `run_fittings`, `run_anchors` and `run_rules`, which nothing evaluates yet.
- **A crate of its own since rung 2 (2026-11-02):** `crates/humanity-ship/`, the old `ship` module
  as the crate root, with `machines` and `station` under it. The crates above it (relay, net,
  renderer, gui) are cut in rungs 3 and 4. Source: [crate-split.md](../design/crate-split.md),
  "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
