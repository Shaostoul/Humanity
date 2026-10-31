# humanity-ship

## What it is for

The home and the ship as places a person stands in. It holds the home's walls, doors, storeys and
rooms (`home_structure`, `rooms`, `room_types`), the ship's zones, corridors and hull
(`ship_structure`, `hull`), where a person can stand, see and walk aboard (`standing`, `sight`,
`wall_collision`, `ship_space`, `moves`), the starts a new player can choose (`starts`), the
machines a home holds and what they drew over a day (`machines`), and the homestead station's orbit
(`station`). It is plain data and geometry with no GPU in it, and the server compiles it too, so the
shared ship's rules are the same on both sides.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - no gate (every build): `ship/`, `machines.rs` + `machines/`, `station/`
- **Size:** 32,680 lines in 36 files, 1,387 of them in 3 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `ship/levels.rs` 190 of 200.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 3.
- **May use** (the plan): core. **Uses today** (references in non-test code): core 34.
- **May be used by** (the plan): systems, relay, gui, engine. **Used today by:** engine 322, gui 159, relay 38, systems 26.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 32 in test code, to systems 19, engine 8, renderer 5: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 394 tests, and this runs exactly them, checked against every test's name (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter):

```
just test-lib ship:: machines:: station:: --skip engine::map_rig_ship:: --skip systems::grow_machines::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `ship::ship_structure::ShipStructure` | 23 | engine 14, gui 4, systems 3, relay 2 |
| `machines::MachineHome` | 19 | engine 14, gui 4, systems 1 |
| `ship::room_types::RoomBounds` | 14 | gui 11, engine 2, systems 1 |
| `ship::lock_types::LockState` | 13 | engine 8, gui 5 |
| `machines::ConduitEnd` | 11 | gui 7, engine 4 |
| `machines::PlacedMachine` | 10 | engine 10 |
| `ship::ship_structure` | 10 | engine 8, gui 1, relay 1 |
| `ship::structure::structure_type` | 10 | engine 5, gui 4, relay 1 |
| `machines::MachinePower` | 9 | gui 6, systems 2, engine 1 |
| `ship::door_panels::PanelPlacement` | 9 | engine 9 |
| `ship::home_structure::HomeStructure` | 9 | engine 8, gui 1 |
| `machines::RoomGeom` | 8 | engine 8 |
| `ship::home_structure` | 8 | gui 5, engine 3 |
| `ship::starts::Starts` | 8 | engine 4, relay 4 |
| `ship::structure::zone_type` | 8 | gui 7, engine 1 |

And 191 more items, 376 references (`node scripts/crate-cards.js --interface humanity-ship` lists them all).
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
- **One box, two names:** `levels::LevelCut` and `storey_edits::CutAway` are the same box (min, max,
  cut height), and `engine/map_cut.rs` converts one to the other. Source:
  [crate-split.md](../design/crate-split.md), rung 0, "Not counted".
- **Rung 2 cuts ship and terrain, then systems;** its test code's wrong-way references (At a
  glance) move first. Source: [crate-split.md](../design/crate-split.md), "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
