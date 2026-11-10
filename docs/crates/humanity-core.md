# humanity-core

## What it is for

The bottom of the stack: the small pieces every other crate leans on. It holds the ECS world
(`ecs`), the data files' parsers and their built-in copies (`data_parse`, `embedded_data`), hot
reload, where a person's files live (`storage`), the Sol system's bodies and orbits (`cosmos`), the
word source (`i18n`), the cost store every system records into (`frame_costs`), the one switch
that turns off all reuse (`reuse`), and the one material catalog of what a run (a pipe, hose or
cable) is made of (`run_materials`, with `row_source`, where each row's numbers come from), here
because `utilities`' cable checks read it. It may use no other crate and every crate may use it, so an
edit here re-checks all of the code: keep it small, stable and free of anything that belongs higher.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-core/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-core/src/ecs/`, `crates/humanity-core/src/physics/`, `crates/humanity-core/src/audio/`, `crates/humanity-core/src/input/`, `crates/humanity-core/src/data_parse.rs`, `crates/humanity-core/src/vertex.rs`, `crates/humanity-core/src/skin_data.rs`, `crates/humanity-core/src/look_data.rs`, `crates/humanity-core/src/frame_costs.rs`, `crates/humanity-core/src/build_permission.rs`, `crates/humanity-core/src/atomic_write.rs`, `crates/humanity-core/src/weather_map.rs`, `crates/humanity-core/src/civil_date.rs`, `crates/humanity-core/src/embedded_data.rs`, `crates/humanity-core/src/platform.rs` + `crates/humanity-core/src/platform/`, `crates/humanity-core/src/showroom.rs`, `crates/humanity-core/src/utilities.rs`, `crates/humanity-core/src/run_materials.rs`, `crates/humanity-core/src/row_source.rs`, `crates/humanity-core/src/cosmetics.rs`, `crates/humanity-core/src/cosmos.rs`, `crates/humanity-core/src/player_look.rs`, `crates/humanity-core/src/stance.rs`, `crates/humanity-core/src/dev_travel.rs`, `crates/humanity-core/src/credits.rs`, `crates/humanity-core/src/camera_angles.rs`, `crates/humanity-core/src/turning.rs`, `crates/humanity-core/src/scenic_views.rs`, `crates/humanity-core/src/reactions.rs`, `crates/humanity-core/src/lod_registry.rs`, `crates/humanity-core/src/boot_timing.rs`, `crates/humanity-core/src/reuse.rs`, `crates/humanity-core/src/hot_reload/`, `crates/humanity-core/src/storage.rs` + `crates/humanity-core/src/storage/`, `crates/humanity-core/src/debug.rs`
  - `feature = "native"`: `crates/humanity-core/src/play_journal.rs`, `crates/humanity-core/src/raw_frame.rs`, `crates/humanity-core/src/i18n.rs`, `crates/humanity-core/src/mods/`, `crates/humanity-core/src/auto_unlock.rs`
  - `any(test, feature = "test-support")`: `crates/humanity-core/src/test_clock.rs`, `crates/humanity-core/src/test_temp.rs`, `crates/humanity-core/src/test_log.rs`
  - `all(any(test, feature = "test-support"), feature = "native")`: `crates/humanity-core/src/test_gpu.rs`
- **Size:** 19,009 lines in 64 files, 922 of them in 4 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 39, `test-support` 9, `wasm` 4.
- **May use** (the plan): nothing; it is the bottom of the stack. **Uses today** (references in non-test code): nothing.
- **May be used by** (the plan): protocol, ship, terrain, systems, music, animation, relay, net, renderer, gui, engine. **Used today by:** engine 651, gui 505, systems 464, renderer 68, ship 57, net 51, relay 32, animation 13, terrain 5.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 216 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-core
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_core::`):

| Item | References | From |
|---|---:|---|
| `hot_reload::data_store::DataStore` | 122 | systems 81, engine 39, net 1, ship 1 |
| `debug::push_debug` | 82 | gui 42, net 23, engine 17 |
| `ecs::components` | 77 | systems 58, engine 18, ship 1 |
| `storage::data_dir` | 76 | gui 40, systems 21, engine 7, ship 5, relay 2, net 1 |
| `i18n::t` | 73 | gui 66, engine 6, systems 1 |
| `embedded_data::note_builtin_copy` | 60 | ship 15, systems 14, relay 13, gui 9, renderer 6, engine 1, net 1, terrain 1 |
| `ecs::components::Transform` | 59 | engine 40, systems 15, gui 1, net 1, relay 1, ship 1 |
| `i18n` | 56 | gui 56 |
| `frame_costs::stage` | 51 | engine 30, systems 16, gui 4, renderer 1 |
| `frame_costs::span` | 49 | engine 49 |
| `ecs::components::Controllable` | 46 | systems 26, engine 19, gui 1 |
| `ecs::systems::System` | 43 | systems 41, engine 1, net 1 |
| `vertex::Vertex` | 34 | engine 15, renderer 9, ship 7, terrain 3 |
| `cosmos::find_body` | 32 | engine 26, gui 4, renderer 2 |
| `embedded_data::load_data_or_embedded` | 32 | systems 13, gui 8, net 6, engine 3, relay 1, ship 1 |

And 259 more items, 954 references (`node scripts/crate-cards.js --interface humanity-core` lists them all).
<!-- end generated -->

## Rules

- **Nothing here reaches up.** Code in core may name no other crate; `node scripts/check-crate-layers.js`
  (in `just lints`) fails on any reference that points upward. When core needs something from
  above, the caller passes it in, or that thing moves down here. Source:
  [crate-split.md](../design/crate-split.md), rung 0, and [crate-layers.json](../../scripts/crate-layers.json).
- **Only what several crates need goes here,** and nothing that holds GPU, GUI or server state:
  every edit in core re-checks every crate above it
  ([crate-split.md](../design/crate-split.md), "How a split pays, and when it does not").
- **Most of it compiles into the server build too.** A module that touches the GUI, the renderer,
  persistence or save state gets `#[cfg(feature = "native")]`, and `just verify-relay-compiles`
  passes before a push. Source: [CLAUDE.md](../../CLAUDE.md), Known gotchas (the ungated
  `save_load` that kept the deploy red for 25 releases).
- **No caches kept across runs; in-memory reuse only when it cannot go stale, and it asks
  `reuse::enabled()`.** The switch itself lives here ([reuse.rs](../../crates/humanity-core/src/reuse.rs): "Turn off all
  reuse" in the Dev tab's Performance section, `HUMANITY_NO_REUSE=1` for a rig). A new reuse that
  does not ask is a bug. Source: [CLAUDE.md](../../CLAUDE.md), "No caches kept across runs during development".
- **Data, not code.** Anything that can exist more than once is a data file under `data/`, read
  through `data_parse` and `embedded_data`. Source: [infinite-of-x.md](../design/infinite-of-x.md).
- **Words come from the word source by key.** `i18n` reads `data/i18n/en.json`; new words are added
  with `node scripts/i18n-add.js`, never typed into a UI call. `i18n` is native-only (see the gates
  above), so code that also compiles into the server hands back a word key, not a word. Source:
  [CLAUDE.md](../../CLAUDE.md), "One word source", and [ui-system.md](../design/ui-system.md), "Words".
- **The material catalog is the core's, its rules and parts are the ship's.**
  `run_materials` (data/piping/pipe_materials.ron) holds plain data and its loader only, because
  `utilities` reads its cable specs and the core may not reach up; what the rules, fittings,
  anchors and benders say about a material lives in `ship::run_*`. Every row carries a `source`
  (`row_source`). Source: [spline-routing.md](../design/spline-routing.md), sections 5.3 and 17.
- **Planet-scale numbers stay in f64** in `cosmos`, `camera_angles` and `dev_travel`, all the way
  to the sampler that reads them. Source: [renderer.md](../ai/renderer.md), "f32 at planet scale".
- **`auto_unlock` keeps the identity's secret in the OS keychain, optionally behind a PIN.** Read
  [cryptography.md](../reference/cryptography.md) (its "Auto-unlock" row) before changing it or
  naming an algorithm, and update the table in the same commit. Source: [CLAUDE.md](../../CLAUDE.md),
  Cryptography.

## How to test it

- **Its lib tests:** `just test-lib --crate humanity-core`, the core's own test program (it is a
  crate of its own since rung 1). Since every crate uses core, a change to a widely used item (the
  interface table) also wants `just verify`, the static gate over the whole workspace, and
  `just test-lib`, every member's tests.
- **Its test helpers cross to other crates through `test-support`:** `test_temp`, `test_clock`,
  `test_log`, `test_gpu`, the frame-cost store's `reset_for_test` and `test_lock`, and the
  `fixtures` modules (`storage::fixtures`, `dev_travel::fixtures`) compile in this crate's own
  tests and in a dependent's test build, which turns the feature on in its `[dev-dependencies]`;
  never in a product build. A new helper another crate's tests need goes behind the same gate
  (`#[cfg(any(test, feature = "test-support"))]`), in a `fixtures` module, never inside a test
  module (whose private helpers would warn as unused in a non-test build).
- **After editing anything under `data/`:** `just validate-data`, every data loader and data-wiring
  test (the parsers are here; the registries that use them are mostly in systems).
- **The server build:** `just verify-relay-compiles` (core is mostly ungated, so the relay build
  compiles almost all of it), and `just verify-relay`, whose battery runs the core's tests too.
- **`just lints`:** the layer ratchet, the word-source lint (`tests/i18n_lint.rs`) and the rest.
- Nothing in core needs a booted game.

## Open work

- **Cut in rung 1 (2026-11-01):** a crate of its own, `crates/humanity-core/`, with the build times
  measured before and after in [crate-split.md](../design/crate-split.md) (rung 1). Callers name it
  `humanity_core::...`. Its version is the workspace's (`[workspace.package]`): its own code reports
  the app's version (boot timings, frame costs, the play journal, the refresh check), so a version
  bump rebuilds it like every other member. Decoupling that is open work if a bump's rebuild ever
  matters.
- **A stale header:** [data_store.rs](../../crates/humanity-core/src/hot_reload/data_store.rs) says the DataStore "wraps
  AssetManager"; it is a plain map of parsed registries by key, and it cannot reach the asset
  manager (which is the renderer's). Found 2026-10-31 writing this card.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
