# humanity-engine

## What it is for

THE FRAME: the main loop, and everything that wires the other crates into one running app.
`lib.rs` is engine start-up and the main loop; `engine/` holds the frame's parts (world loading,
the home's meshes, the in-world screens, the dev IPC and the dev MCP endpoint, the `frame_*` steps,
launch focus, the build editor); `save_load` applies a save to the running world; `main.rs` is the
binary. It may use every crate and no crate uses it, so the most-edited code sits on top, where an
edit re-checks the least.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - `feature = "native"`: `engine/`, `save_load.rs`
  - `feature = "wasm"`: `wasm_entry.rs`
  - the crate root: `lib.rs`
  - the binary: `main.rs`
  - test builds only: `save_load_body_tests.rs`, `save_load_slot_tests.rs`
- **Size:** 86,090 lines in 99 files, 6,512 of them in 17 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `lib.rs` 16,668 of 16,700, `engine/frame_shells.rs` 1,296 of 1,300, `engine/map_rig.rs` 242 of 250, `engine/map_view.rs` 303 of 315, `engine/frame_water.rs` 716 of 750, `engine/frame_near_trees.rs` 668 of 700, `engine/map_rig_ship.rs` 109 of 115, `engine/map_feed.rs` 188 of 200, `engine/map_cut.rs` 69 of 75, `engine/frame_ws_poll.rs` 1,626 of 1,800, `engine/map_tiles.rs` 365 of 450.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 31, `relay` 5, `wasm` 1.
- **May use** (the plan): core, protocol, ship, terrain, systems, relay, net, renderer, gui. **Uses today** (references in non-test code): core 572, systems 568, gui 487, ship 322, renderer 310, terrain 131, net 95, relay 7.
- **May be used by** (the plan): nothing; it is the top of the stack. **Used today by:** nothing.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 0 in test code.
- **Its lib tests:** 584 tests, and this runs exactly them, checked against every test's name (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter):

```
just test-lib engine:: save_load:: wasm_entry:: --skip engine::block:: --skip engine::choice:: --skip engine::dm:: --skip engine::file_transfer:: --skip engine::friend_code:: --skip engine::group_report:: --skip engine::household:: --skip engine::key_cards:: --skip engine::low_impact:: --skip engine::protected:: --skip engine::put_answer:: --skip engine::reach:: --skip engine::report:: --skip engine::warnings::
```

**Interface:** no other crate references anything in it.
<!-- end generated -->

## Rules

- **`lib.rs` is the merge funnel: the fewer reasons to touch it, the fewer collisions.** Loop code
  goes into a file under `src/engine/`; the size ratchet fails a monolith that grows past its budget
  (At a glance) and asks for the budget to come down when it shrinks. Source:
  [CLAUDE.md](../../CLAUDE.md), Architecture, and
  [file_size_ratchet.rs](../../tests/file_size_ratchet.rs).
- **Stage your own files by name** (`just mine`, `just ship`), and a shared file that holds another
  session's hunks (`lib.rs` most of all) is staged hunk by hunk and the staged tree compiled before
  pushing. Source: [CLAUDE.md](../../CLAUDE.md), "Sharing this checkout", and
  [orchestrator.md](../ai/orchestrator.md), section 8.
- **Never set `HUMANITY_TAKE_FOCUS`;** a game started by a script opens in the background and silent
  by itself. At most one booted game machine-wide; close the operator's with `taskkill //PID <pid>`
  (no `//F` first, so it saves). Source: [CLAUDE.md](../../CLAUDE.md), "The operator's machine",
  held by `tests/focus_optin_lint.rs`.
- **`save_load` stays behind `#[cfg(feature = "native")]`:** an ungated `save_load` broke every relay
  and CI build from v0.381 to v0.414. Source: [CLAUDE.md](../../CLAUDE.md), Known gotchas.
- **Every system the simulation defines is registered in `lib.rs`,** or listed in
  `DEFERRED_SYSTEMS` with its reason. Source: [engine_wiring_lint.rs](../../tests/engine_wiring_lint.rs).
- **Every reuse in the frame asks `reuse::enabled()`** (the draw lists, the dev request reader, the
  Garden mirror): a new one that does not ask is a bug. Source: [reuse.rs](../../src/reuse.rs) and
  [CLAUDE.md](../../CLAUDE.md), "No caches kept across runs during development".
- **The dev tooling is permanent:** the dev IPC, the dev MCP endpoint (on 127.0.0.1,
  [mcp.md](../ai/mcp.md)), the play journal's frame half and the overlays are load-bearing, never
  cleanup. Source: [CLAUDE.md](../../CLAUDE.md), "Forever development".
- **The rigs' determinism pins cannot be bypassed.** Source: [rig_pin_lint.rs](../../tests/rig_pin_lint.rs).
- **A change to start-up or world entry is proven by booting** (`just verify-runtime`). Source:
  [renderer.md](../ai/renderer.md), "Verify by booting and entering the world".

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance (it skips the GUI's files that sit in
  `engine/`).
- **Boot:** `just verify-runtime`; for a change a person walks into, the rig for that state:
  `just verify-falls`, `just verify-storeys`, `just verify-copresence` (with `--build`, `--starts`
  and `--household`, or their own recipes), `just verify-screens`, `just verify-live-screen`; and
  `just mcp-smoke --boot --close` for the dev MCP end to end. Which one fits: the `game-checks`
  skill; how to boot without disturbing the operator: [game-boots.md](../ai/game-boots.md).
- **`just lints`:** `engine_wiring_lint`, `focus_optin_lint`, `rig_pin_lint`, `file_size_ratchet`.
- **The rigs' own tests, in `just rig-tests`:** [rig-boot.test.js](../../scripts/tests/rig-boot.test.js),
  [game-launch.test.js](../../scripts/tests/game-launch.test.js),
  [boot-lock.test.js](../../scripts/tests/boot-lock.test.js),
  [verify-runtime-gate.test.js](../../scripts/tests/verify-runtime-gate.test.js),
  [check-fresh-exe.test.js](../../scripts/tests/check-fresh-exe.test.js) (a rig refuses a build of
  another tree, BUG-133), [rig-needs.test.js](../../scripts/tests/rig-needs.test.js),
  [mcp-server.test.js](../../scripts/tests/mcp-server.test.js),
  [play-journal-bridge.test.js](../../scripts/tests/play-journal-bridge.test.js).

## Open work

- **NEXT in TIER 0: the headless, GPU-free sim runner.** Its increments 1 to 5 move code out of
  `lib.rs` and need quiet windows, as do FarmingSystem's F1 and F2. Source:
  [PRIORITIES.md](../PRIORITIES.md), TIER 0 NEXT, and
  [headless-sim-runner.md](../design/headless-sim-runner.md).
- **Stutter:** every periodic job gets a per-frame budget; the home autosave (2.3 to 3.2 ms every
  120 s, on the main thread) is a candidate for writing off it. Source: TIER 0 NOW item 12 and
  [sim-frame-cost.md](../design/sim-frame-cost.md), Survey 1.
- **From TIER 0 NOW items 5 and 8:** teleports and a hitch's slowest pass are not yet kinds in the
  play journal; low impact does not lower the CPU per frame (the wall screens' UI 10 to 13 ms,
  terrain streaming); the Maps tile bake (`engine/map_tiles.rs`) is over its 2 ms budget in the
  frames that bake.
- **`just launch-bg` and hand-typed boots skip the rigs' boot lock.** Source: PRIORITIES, RESUME
  HERE, "Increment 7 follow-ups".
- **BUG-217, open:** `frame_ws_poll.rs` replaces the desktop's task list with the server's, so a
  task made in the app is lost (the gui card has the fix's shape). Source: [BUGS.md](../BUGS.md).
- **For the split:** `crate::data_dir()` moves from `lib.rs` to core at rung 1; `engine/map_cut.rs`
  converts `LevelCut` to `CutAway`, the same box under two names; rung 4 leaves the engine on top
  once gui is cut. Source: [crate-split.md](../design/crate-split.md), rung 0, "Not counted".
- **The counting cannot see `main.rs`:** the binary reaches the library through
  `humanity_engine::` paths, not `crate::`, so its uses (of `release_update`, for one) are missing
  from every interface table. Found 2026-10-31 writing this card.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
