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
- **Owns** all of `crates/humanity-systems/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-systems/src/surface_walk.rs`, `crates/humanity-systems/src/surface_move.rs`, `crates/humanity-systems/src/persistence.rs` + `crates/humanity-systems/src/persistence/`, `crates/humanity-systems/src/time.rs`, `crates/humanity-systems/src/notices.rs`, `crates/humanity-systems/src/farming/`, `crates/humanity-systems/src/construction/`, `crates/humanity-systems/src/door_anim.rs`, `crates/humanity-systems/src/inventory/`, `crates/humanity-systems/src/encumbrance.rs`, `crates/humanity-systems/src/machine_levels.rs`, `crates/humanity-systems/src/combat/`, `crates/humanity-systems/src/quests/`, `crates/humanity-systems/src/crafting/`, `crates/humanity-systems/src/vehicles/`, `crates/humanity-systems/src/livestock.rs`, `crates/humanity-systems/src/animal_care.rs`, `crates/humanity-systems/src/abilities.rs`, `crates/humanity-systems/src/ai/`, `crates/humanity-systems/src/skills/`, `crates/humanity-systems/src/ecology.rs`, `crates/humanity-systems/src/economy/`, `crates/humanity-systems/src/player.rs`, `crates/humanity-systems/src/interaction.rs`, `crates/humanity-systems/src/weather.rs`, `crates/humanity-systems/src/weather_events.rs`, `crates/humanity-systems/src/hydrology.rs`, `crates/humanity-systems/src/atmosphere.rs`, `crates/humanity-systems/src/body_environment.rs`, `crates/humanity-systems/src/env_layer1.rs`, `crates/humanity-systems/src/precipitation.rs`, `crates/humanity-systems/src/disasters.rs`, `crates/humanity-systems/src/electrical.rs`, `crates/humanity-systems/src/plumbing.rs`, `crates/humanity-systems/src/solar.rs`, `crates/humanity-systems/src/fire.rs`, `crates/humanity-systems/src/status_effects.rs`, `crates/humanity-systems/src/flight.rs`, `crates/humanity-systems/src/food.rs`, `crates/humanity-systems/src/body_heat.rs`, `crates/humanity-systems/src/illness.rs`, `crates/humanity-systems/src/treatment.rs`, `crates/humanity-systems/src/wounds.rs`, `crates/humanity-systems/src/falls.rs`, `crates/humanity-systems/src/sleep.rs`, `crates/humanity-systems/src/death_pack.rs`, `crates/humanity-systems/src/fluids.rs`, `crates/humanity-systems/src/mining.rs`, `crates/humanity-systems/src/governance.rs`, `crates/humanity-systems/src/docking.rs`, `crates/humanity-systems/src/aging.rs`, `crates/humanity-systems/src/creative_arts.rs`, `crates/humanity-systems/src/geology.rs`, `crates/humanity-systems/src/oceanography.rs`, `crates/humanity-systems/src/astronomy.rs`, `crates/humanity-systems/src/manufacturing.rs`, `crates/humanity-systems/src/waste.rs`, `crates/humanity-systems/src/genetics.rs`, `crates/humanity-systems/src/transportation.rs`, `crates/humanity-systems/src/offline.rs`, `crates/humanity-systems/src/self_sufficiency.rs`, `crates/humanity-systems/src/grow_machines.rs`, `crates/humanity-systems/src/life_support.rs`, `crates/humanity-systems/src/ship_power.rs`, `crates/humanity-systems/src/ship_meals.rs`
  - test builds only: `crates/humanity-systems/src/day_energy_tests.rs`, `crates/humanity-systems/src/crossing_tests/`
- **Size:** 80,375 lines in 148 files, 12,384 of them in 35 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `crates/humanity-systems/src/surface_walk.rs` 462 of 474.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `test-support` 8, `native` 7.
- **May use** (the plan): core, ship, terrain. **Uses today** (references in non-test code): core 466, ship 26, terrain 7.
- **May be used by** (the plan): relay, gui, engine. **Used today by:** engine 576, gui 163, relay 10.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 865 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-systems
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_systems::`):

| Item | References | From |
|---|---:|---|
| `inventory::ItemRegistry` | 25 | engine 25 |
| `inventory::placed::PlacedItem` | 21 | engine 13, gui 8 |
| `weather::WeatherCondition` | 15 | engine 12, gui 3 |
| `persistence::live_save::active_home` | 13 | engine 9, gui 4 |
| `construction` | 11 | engine 11 |
| `construction::BlueprintRegistry` | 11 | engine 10, relay 1 |
| `persistence::saves_dir` | 11 | engine 6, gui 4, relay 1 |
| `inventory::Inventory` | 10 | engine 10 |
| `inventory::containers::Container` | 9 | engine 9 |
| `persistence::slot_path` | 9 | engine 8, gui 1 |
| `inventory` | 7 | engine 6, gui 1 |
| `inventory::TransferOp` | 7 | gui 5, engine 2 |
| `persistence::live_save::active_home_machines_path` | 7 | engine 7 |
| `time::GameTime` | 7 | engine 7 |
| `construction::uses::planet_store_paths` | 6 | engine 6 |

And 367 more items, 580 references (`node scripts/crate-cards.js --interface humanity-systems` lists them all).
<!-- end generated -->

## Rules

- **It may use core, ship and terrain only** (the ratchet in `just lints`). It speaks in word keys
  and arguments, never English, because the word source is native-only and this crate also compiles
  into the server. Source: [PRIORITIES.md](../PRIORITIES.md), TIER 0 NOW item 3 ("systems speak in
  keys and arguments"), and [CLAUDE.md](../../CLAUDE.md), "One word source".
- **Every system runs, or says why not.** Each `impl System` in this crate is registered in the
  engine's `src/lib.rs` or listed in `DEFERRED_SYSTEMS` with a reason. Source:
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
- **A system says whether a line is the world's or a reply,** by the channel it writes to, never
  by its words: `notices::world` for what the world says on its own (a bed flowering, a craft
  finishing; held until the person enters a world), `notices::reply` for the answer to a request a
  page sent (a craft refused, a Plant press with no seed; shown at once). Source:
  [BUGS.md](../BUGS.md), BUG-258 and BUG-253; `crates/humanity-systems/src/notices.rs`.
- **A step never depends on the frame rate.** Doses, needs and falls are exact per tick, the same at
  30 and 240 frames a second. Source: [BUGS.md](../BUGS.md), BUG-164 and BUG-209.
- **The farming golden state stays bit for bit** through every speed change and every rung of the
  split (`farming/golden_tests.rs`). Source: [crate-split.md](../design/crate-split.md),
  "Costs and risks", and [sim-frame-cost.md](../design/sim-frame-cost.md), "Proof first".
- **The save shape changes outright** (no migration code before v1.0, except version-tagged
  blueprint schemas that must keep loading old player saves), and every save is written crash-safe
  (`atomic_write`). Source: [CLAUDE.md](../../CLAUDE.md), "No backwards-compatibility debt before
  launch", and [BUGS.md](../BUGS.md), BUG-129.
- **Walking on a planet is planet-scale math:** f64 to the sampler. Source:
  [renderer.md](../ai/renderer.md), "f32 at planet scale".

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance. The golden state alone:
  `just test-lib --crate humanity-systems farming::golden_tests::`.
- **The server's battery:** `just verify-relay`, which runs this crate's tests in the server's
  feature set (no `native`).
- **Where its tests start:** in `crates/humanity-systems/`, under `cargo test` and `just test-lib`
  alike, where no data/ sits. Name the data folder `humanity_core::storage::data_dir()` (a test
  build resolves it to the repo's) or `env!("HUMANITY_REPO_ROOT")`, never a bare `"data"`.
- **After editing `data/`:** `just validate-data`; then `just lints` for the wiring, recipe-source
  and byproduct lints.
- **In the running game:** `just verify-falls` (falls hurt as `falls.rs` and
  `data/medical/falls.ron` say;
  [falls-judge.test.js](../../scripts/tests/falls-judge.test.js)), `just verify-shared-build`
  (building in the shared world). One game at a time: [game-boots.md](../ai/game-boots.md).
- **Doors a hand opens (2026-11-10):** `door_anim`'s tests (the hand target, the swing away from
  the opener, the push ahead of a body walking through, each seen red); in the running game
  `anim-door-hand` and its red twin `anim-door-hand-hand-ik-off` (character-animation.md section 17,
  "Built after the answers").

## Open work

- **Pulleys** (`pulley/`): rung 1 (the model, the configurator, the block geometry and collision)
  is done; next the web mirror and synthetic rope rows, then the gear in the world (equipment rows,
  anchors, the meshes drawn), lifting through the carry rules, and failure. Source:
  [pulleys.md](../design/pulleys.md), section 8.
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
- **A crate of its own since rung 2 (2026-11-02):** `crates/humanity-systems/`, the old
  `systems` module as the crate root, with `persistence`, `surface_walk` and `surface_move` under it.
  The domains inside it (farming, construction, crafting, food) as crates side by side: not now,
  revisited with rung 2's measurements. Source: [crate-split.md](../design/crate-split.md), "The
  questions the operator asked" and "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
