# humanity-systems

## What it is for

The simulation: what happens in the world, and why. Farming, building, crafting, food, the body
(needs, illness, wounds, falls, heat), weather and the environment, creatures, the economy and the
quests each live in a folder or file under `systems/`, as the `System`s the engine runs every tick.
`persistence` is the save's shape, the same for the desktop app and for a server that holds a
shared world, and `surface_walk` and `surface_move` are walking and moving on a planet. It stands on
core, ship and terrain, draws nothing, and the server compiles it too.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - no gate (every build): `systems/`, `persistence.rs` + `persistence/`, `surface_walk.rs`, `surface_move.rs`
- **Size:** 82,151 lines in 137 files, 15,196 of them in 28 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `surface_walk.rs` 965 of 1,150.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 5.
- **May use** (the plan): core, ship, terrain. **Uses today** (references in non-test code): core 410, ship 26, terrain 7.
- **May be used by** (the plan): relay, gui, engine. **Used today by:** engine 568, gui 157, relay 10.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 51 in test code, to engine 27, gui 12, relay 7, renderer 5: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 896 tests, and this runs exactly them, checked against every test's name (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter):

```
just test-lib systems:: persistence:: surface_walk:: surface_move:: --skip ecs::systems:: --skip relay::storage::game_persistence::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `systems::inventory::ItemRegistry` | 25 | engine 25 |
| `systems::inventory::placed::PlacedItem` | 16 | engine 8, gui 8 |
| `systems::weather::WeatherCondition` | 15 | engine 12, gui 3 |
| `persistence::live_save::active_home` | 13 | engine 9, gui 4 |
| `persistence::saves_dir` | 11 | engine 6, gui 4, relay 1 |
| `systems::construction` | 11 | engine 11 |
| `systems::construction::BlueprintRegistry` | 10 | engine 9, relay 1 |
| `systems::inventory::Inventory` | 10 | engine 10 |
| `persistence::slot_path` | 9 | engine 8, gui 1 |
| `systems::inventory::containers::Container` | 9 | engine 9 |
| `systems::time::GameTime` | 8 | engine 8 |
| `persistence::live_save::active_home_machines_path` | 7 | engine 7 |
| `systems::inventory` | 7 | engine 6, gui 1 |
| `systems::inventory::TransferOp` | 7 | gui 5, engine 2 |
| `persistence::now_ms` | 6 | gui 5, engine 1 |

And 366 more items, 571 references (`node scripts/crate-cards.js --interface humanity-systems` lists them all).
<!-- end generated -->

## Rules

- **It may use core, ship and terrain only** (the ratchet in `just lints`). It speaks in word keys
  and arguments, never English, because the word source is native-only and this crate also compiles
  into the server. Source: [PRIORITIES.md](../PRIORITIES.md), TIER 0 NOW item 3 ("systems speak in
  keys and arguments"), and [CLAUDE.md](../../CLAUDE.md), "One word source".
- **Every system runs, or says why not.** Each `impl System` under `src/systems/` is registered in
  `src/lib.rs` or listed in `DEFERRED_SYSTEMS` with a reason. Source:
  [engine_wiring_lint.rs](../../tests/engine_wiring_lint.rs) (in `just lints`).
- **Teaching data is checked like code.** Recipes, food handling and plant facts teach real skills:
  "Every fact here still needs the teaching-data refutation pass before the chains that use it
  ship." Source: [BUGS.md](../BUGS.md), BUG-216.
- **A recipe is real only if a player can gather every input,** and a byproduct nothing uses is a
  defect. Source: [recipe_sources_lint.rs](../../tests/recipe_sources_lint.rs) and
  [byproduct_use_lint.rs](../../tests/byproduct_use_lint.rs) (both in `just lints`).
- **Every deep system ships two modes,** full realism and a simplified one (general play defaults to
  softened), and a gear-gated feature also gets a plain toggle. Falls are the worked example:
  Forgiving and Realistic in `data/medical/falls.ron`. Source: [CLAUDE.md](../../CLAUDE.md), "Dual
  modes, open source, and a compute budget".
- **Compute is a budget.** Cosmetic simulation runs on each client and is never networked; only
  gameplay state syncs, at a low rate; each step states its per-frame cost. Source: the same section.
- **A step never depends on the frame rate.** Doses, needs and falls are exact per tick, the same at
  30 and 240 frames a second. Source: [BUGS.md](../BUGS.md), BUG-164 and BUG-209.
- **The farming golden state stays bit for bit** through every speed change and every rung of the
  split (`systems/farming/golden_tests.rs`). Source: [crate-split.md](../design/crate-split.md),
  "Costs and risks", and [sim-frame-cost.md](../design/sim-frame-cost.md), "Proof first".
- **The save shape changes outright** (no migration code before v1.0, except version-tagged
  blueprint schemas that must keep loading old player saves), and every save is written crash-safe
  (`atomic_write`). Source: [CLAUDE.md](../../CLAUDE.md), "No backwards-compatibility debt before
  launch", and [BUGS.md](../BUGS.md), BUG-129.
- **Walking on a planet is planet-scale math:** f64 to the sampler. Source:
  [renderer.md](../ai/renderer.md), "f32 at planet scale".

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance. The golden state alone:
  `just test-lib systems::farming::golden_tests::`.
- **The server's battery:** `just verify-relay` (CLAUDE.md names `src/systems/` as shared code whose
  tests run there too).
- **After editing `data/`:** `just validate-data`; then `just lints` for the wiring, recipe-source
  and byproduct lints.
- **In the running game:** `just verify-falls` (falls hurt as `falls.rs` and
  `data/medical/falls.ron` say;
  [falls-judge.test.js](../../scripts/tests/falls-judge.test.js)), `just verify-shared-build`
  (building in the shared world). One game at a time: [game-boots.md](../ai/game-boots.md).

## Open work

- **FarmingSystem's cost:** F1 (per-tick lookup tables) and F2 (no allocations per crop), about 2.5
  to 3 ms back, then gating the Garden mirror. Source: [PRIORITIES.md](../PRIORITIES.md), TIER 0 NOW
  item 4, and [sim-frame-cost.md](../design/sim-frame-cost.md).
- **The farm on its own clock:** rung 1 (every step exact) is done; next the farm's own interval,
  time-sliced by area, then events and closed-form crops, then lazy homes on the server. Source:
  [PRIORITIES.md](../PRIORITIES.md), TIER 0 NOW item 11, and [farm-clock.md](../design/farm-clock.md).
- **The closed-loop test, phase 2:** the loader, the report, the home-day cases on the life-support
  harness, then the aggregate tier. Source: [PRIORITIES.md](../PRIORITIES.md), TIER 0 NOW item 7,
  and [closed-loop-test.md](../design/closed-loop-test.md), section 13.
- **English left in shared code:** the crafting system's other notices. Source: TIER 0 NOW item 3.
- **BUG-216, partly fixed:** canning low-acid food should need a pressure canner, which the game
  does not have yet (the Food chain's Can branch); `purify_water` and `water_tester_0` wait for the
  Water chain's water-contents model; whether the cold zone should end at 4 C goes with the Keep It
  branch; and every fact still needs the refutation pass. Source: [BUGS.md](../BUGS.md), BUG-216,
  and [quest-chains.md](../design/quest-chains.md).
- **BUG-153's last part:** the campfire's warmth works, its light does not (the renderer's point
  lights). Source: [BUGS.md](../BUGS.md), BUG-153, "Still open".
- **The headless sim runner** will run this crate faster than real time without a GPU. Source:
  [headless-sim-runner.md](../design/headless-sim-runner.md) (PRIORITIES TIER 0, NEXT).
- **Rung 2 cuts systems** after ship and terrain; its test code's wrong-way references (At a
  glance) move first. Source: [crate-split.md](../design/crate-split.md), "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
