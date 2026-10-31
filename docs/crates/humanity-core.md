# humanity-core

## What it is for

The bottom of the stack: the small pieces every other crate leans on. It holds the ECS world
(`ecs`), the data files' parsers and their built-in copies (`data_parse`, `embedded_data`), hot
reload, where a person's files live (`storage`), the Sol system's bodies and orbits (`cosmos`), the
word source (`i18n`), the cost store every system records into (`frame_costs`) and the one switch
that turns off all reuse (`reuse`). It may use no other crate and every crate may use it, so an
edit here re-checks all of the code: keep it small, stable and free of anything that belongs higher.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - no gate (every build): `input/`, `audio/`, `utilities.rs`, `physics/`, `debug.rs`, `platform.rs` + `platform/`, `turning.rs`, `reactions.rs`, `lod_registry.rs`, `boot_timing.rs`, `credits.rs`, `scenic_views.rs`, `showroom.rs`, `player_look.rs`, `cosmetics.rs`, `embedded_data.rs`, `data_parse.rs`, `vertex.rs`, `camera_angles.rs`, `frame_costs.rs`, `build_permission.rs`, `atomic_write.rs`, `weather_map.rs`, `civil_date.rs`, `ecs/`, `hot_reload/`, `storage.rs` + `storage/`, `cosmos.rs`, `dev_travel.rs`, `reuse.rs`
  - `feature = "native"`: `mods/`, `auto_unlock.rs`, `play_journal.rs`, `raw_frame.rs`, `i18n.rs`
  - test builds only: `test_temp.rs`, `test_log.rs`, `test_clock.rs`
- **Size:** 14,579 lines in 50 files, 859 of them in 4 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 26, `wasm` 4.
- **May use** (the plan): nothing; it is the bottom of the stack. **Uses today** (references in non-test code): nothing.
- **May be used by** (the plan): protocol, ship, terrain, systems, relay, net, renderer, gui, engine. **Used today by:** engine 572, systems 410, gui 300, renderer 61, net 45, ship 34, relay 27, terrain 4.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 4 in test code, to terrain 3, ship 1: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 172 tests, and this runs exactly them, checked against every test's name (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter):

```
just test-lib input:: audio:: utilities:: physics:: debug:: platform:: turning:: reactions:: lod_registry:: boot_timing:: credits:: scenic_views:: showroom:: mods:: player_look:: cosmetics:: auto_unlock:: embedded_data:: data_parse:: vertex:: camera_angles:: frame_costs:: play_journal:: raw_frame:: i18n:: build_permission:: atomic_write:: weather_map:: civil_date:: ecs:: hot_reload:: storage:: cosmos:: dev_travel:: test_temp:: test_log:: test_clock:: reuse:: --skip engine::input:: --skip gui::pages::showroom:: --skip media::audio:: --skip relay::storage::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `hot_reload::data_store::DataStore` | 101 | systems 68, engine 31, net 1, ship 1 |
| `debug::push_debug` | 83 | gui 42, net 23, engine 18 |
| `ecs::components` | 69 | systems 51, engine 17, ship 1 |
| `ecs::components::Transform` | 57 | engine 38, systems 15, gui 1, net 1, relay 1, ship 1 |
| `embedded_data::note_builtin_copy` | 53 | systems 14, relay 13, ship 9, gui 8, renderer 6, engine 1, net 1, terrain 1 |
| `frame_costs::span` | 49 | engine 49 |
| `ecs::components::Controllable` | 45 | systems 25, engine 19, gui 1 |
| `ecs::systems::System` | 41 | systems 39, engine 1, net 1 |
| `frame_costs::stage` | 38 | engine 21, systems 13, gui 3, renderer 1 |
| `vertex::Vertex` | 34 | engine 15, renderer 9, ship 7, terrain 3 |
| `cosmos::find_body` | 32 | engine 26, gui 4, renderer 2 |
| `embedded_data::load_data_or_embedded` | 32 | systems 13, gui 8, net 6, engine 3, relay 1, ship 1 |
| `i18n` | 29 | gui 29 |
| `cosmos::body_world_position_3d_au` | 26 | engine 23, renderer 3 |
| `i18n::t` | 25 | gui 22, engine 2, systems 1 |

And 211 more items, 739 references (`node scripts/crate-cards.js --interface humanity-core` lists them all).
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
  `reuse::enabled()`.** The switch itself lives here ([reuse.rs](../../src/reuse.rs): "Turn off all
  reuse" in the Dev tab's Performance section, `HUMANITY_NO_REUSE=1` for a rig). A new reuse that
  does not ask is a bug. Source: [CLAUDE.md](../../CLAUDE.md), "No caches kept across runs during development".
- **Data, not code.** Anything that can exist more than once is a data file under `data/`, read
  through `data_parse` and `embedded_data`. Source: [infinite-of-x.md](../design/infinite-of-x.md).
- **Words come from the word source by key.** `i18n` reads `data/i18n/en.json`; new words are added
  with `node scripts/i18n-add.js`, never typed into a UI call. `i18n` is native-only (see the gates
  above), so code that also compiles into the server hands back a word key, not a word. Source:
  [CLAUDE.md](../../CLAUDE.md), "One word source", and [ui-system.md](../design/ui-system.md), "Words".
- **Planet-scale numbers stay in f64** in `cosmos`, `camera_angles` and `dev_travel`, all the way
  to the sampler that reads them. Source: [renderer.md](../ai/renderer.md), "f32 at planet scale".
- **`auto_unlock` keeps the identity's secret in the OS keychain, optionally behind a PIN.** Read
  [cryptography.md](../reference/cryptography.md) (its "Auto-unlock" row) before changing it or
  naming an algorithm, and update the table in the same commit. Source: [CLAUDE.md](../../CLAUDE.md),
  Cryptography.

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance runs exactly core's (Windows needs
  `just test-lib`, never a bare `cargo test --lib`: BUG-174). Since every crate uses core, a change
  to a widely used item (the interface table) also wants `just verify`, the static gate over the
  whole crate.
- **After editing anything under `data/`:** `just validate-data`, every data loader and data-wiring
  test (the parsers are here; the registries that use them are mostly in systems).
- **The server build:** `just verify-relay-compiles` (core is ungated, so the relay build compiles
  almost all of it).
- **`just lints`:** the layer ratchet, the word-source lint (`tests/i18n_lint.rs`) and the rest.
- Nothing in core needs a booted game.

## Open work

- **`crate::data_dir()` moves down to core at rung 1.** It lives in `lib.rs` (the engine) and about
  50 files under the engine read it; the counting above does not see it because it names no planned
  module. Source: [crate-split.md](../design/crate-split.md), rung 0, "Not counted".
- **Rung 1 cuts core first,** with protocol: the Cargo workspace, then a cold build and a one-line
  edit measured against 241 s and 56 s. Before that, the test-code references in At a glance move
  with their tests, and `pub(crate)` items used across the new line become `pub`. Source:
  [crate-split.md](../design/crate-split.md), "Order of work".
- **A stale header:** [data_store.rs](../../src/hot_reload/data_store.rs) says the DataStore "wraps
  AssetManager"; it is a plain map of parsed registries by key, and it cannot reach the asset
  manager (which is the renderer's). Found 2026-10-31 writing this card.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
